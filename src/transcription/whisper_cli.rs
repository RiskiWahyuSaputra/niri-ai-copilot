//! Backend STT: whisper.cpp (`whisper-cli`) dipanggil sebagai subprocess lokal.
//!
//! Audio diproses potong-per-potong sehingga tidak perlu menunggu rekaman
//! selesai. Semua pemrosesan berlangsung lokal — tidak ada audio yang dikirim
//! ke jaringan.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::Settings;
use crate::error::{AppError, Result};
use crate::transcription::engine::SttEngine;

pub struct WhisperCli {
    binary: String,
    model: PathBuf,
    threads: u32,
    translate: bool,
}

impl WhisperCli {
    /// Bangun dari settings. Model boleh belum ada saat konstruksi —
    /// `check_ready` yang memvalidasi.
    pub fn from_settings(settings: &Settings) -> Self {
        let binary = settings.resolved_whisper_binary();
        let model = settings
            .resolved_whisper_model()
            .unwrap_or_else(|| PathBuf::from("<model belum diatur>"));
        let threads = resolve_threads(settings);
        Self {
            binary,
            model,
            threads,
            translate: settings.stt.translate,
        }
    }

    pub fn model_path(&self) -> &Path {
        &self.model
    }

    /// Temukan binary di PATH bila bukan path absolut.
    fn resolve_binary(&self) -> Option<PathBuf> {
        let b = self.binary.trim();
        if b.is_empty() {
            return None;
        }
        let p = Path::new(b);
        if p.is_absolute() || b.contains('/') {
            if p.is_file() {
                return Some(p.to_path_buf());
            }
            return None;
        }
        find_in_path(b)
    }
}

fn resolve_threads(settings: &Settings) -> u32 {
    if settings.stt.threads > 0 {
        return settings.stt.threads;
    }
    let cores = std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(2);
    if settings.performance.cpu_friendly {
        2.min(cores)
    } else {
        4.min(cores.max(1))
    }
}

/// Cari binary di PATH (tanpa dependency tambahan).
pub fn find_in_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

impl SttEngine for WhisperCli {
    fn name(&self) -> String {
        format!("whisper-cli ({})", self.binary)
    }

    fn check_ready(&self) -> Result<()> {
        if self.resolve_binary().is_none() {
            return Err(AppError::Unavailable(format!(
                "binary `{}` tidak ditemukan. Pasang paket whisper-cpp (Arch: `sudo pacman -S whisper-cpp`) \
                 atau atur path binary di Settings → Audio.",
                self.binary
            )));
        }
        if !self.model.is_file() {
            return Err(AppError::Unavailable(
                "model whisper belum diatur atau file model tidak ditemukan. \
                 Unduh model GGML (mis. ggml-base.bin) dan atur path-nya di Settings → Audio."
                    .into(),
            ));
        }
        Ok(())
    }

    fn transcribe(&self, wav: &[u8], language: &str) -> Result<String> {
        let binary = self
            .resolve_binary()
            .ok_or_else(|| AppError::Stt(format!("binary `{}` tidak ditemukan", self.binary)))?;

        let tmp = temp_wav_path();
        std::fs::write(&tmp, wav)?;

        let mut cmd = Command::new(&binary);
        cmd.arg("-m")
            .arg(&self.model)
            .arg("-f")
            .arg(&tmp)
            .arg("-l")
            .arg(language)
            .arg("-t")
            .arg(self.threads.to_string())
            .arg("-nt"); // tanpa timestamp di output; timestamp kita kelola per-chunk
        if self.translate {
            cmd.arg("-tr");
        }

        let output = cmd.output().map_err(|e| {
            AppError::Stt(format!("gagal menjalankan whisper-cli: {e}"))
        })?;
        let _ = std::fs::remove_file(&tmp);

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            let tail: String = err.lines().rev().take(3).collect::<Vec<_>>().join(" | ");
            return Err(AppError::Stt(format!(
                "whisper-cli keluar dengan status {}: {}",
                output.status,
                crate::privacy::redact_all(tail.trim())
            )));
        }

        Ok(clean_transcript(&String::from_utf8_lossy(&output.stdout)))
    }
}

/// Bersihkan output whisper: buang log, marker non-ucapan, dan rapikan spasi.
pub fn clean_transcript(raw: &str) -> String {
    let mut pieces: Vec<String> = Vec::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // Buang baris log whisper/ggml yang kadang ikut ke stdout.
        let lower = line.to_ascii_lowercase();
        if lower.starts_with("whisper_")
            || lower.starts_with("[whisper") 
            || lower.contains("ggml_")
            || lower.starts_with("main:")
            || lower.starts_with("system_info")
        {
            continue;
        }
        // Buang marker non-ucapan, mis. [BLANK_AUDIO], (music), [ Pause ].
        let stripped = line.trim_matches(|c| c == '[' || c == ']' || c == '(' || c == ')').trim();
        if stripped.is_empty() || is_non_speech_marker(stripped) {
            continue;
        }
        pieces.push(line.to_string());
    }
    pieces.join(" ").trim().to_string()
}

fn is_non_speech_marker(s: &str) -> bool {
    const MARKERS: [&str; 9] = [
        "blank_audio",
        "music",
        "inaudible",
        "silence",
        "noise",
        "applause",
        "laughter",
        "pause",
        "typing",
    ];
    let lower = s.to_ascii_lowercase();
    MARKERS.iter().any(|m| lower == *m || lower.starts_with(m))
}

fn temp_wav_path() -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("niri-ai-copilot-stt-{}-{nanos}.wav", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_whisper_output() {
        let raw = "\
\n[00:00:00.000 --> 00:00:02.000]  halo selamat pagi\n\
whisper_print_timings: total time = 200 ms\n\
[BLANK_AUDIO]\n\
(inaudible too)\n\
kita mulai wawancara\n";
        let cleaned = clean_transcript(raw);
        assert!(cleaned.contains("halo selamat pagi"));
        assert!(cleaned.contains("kita mulai wawancara"));
        assert!(!cleaned.contains("whisper_print_timings"));
        assert!(!cleaned.contains("BLANK_AUDIO"));
        assert!(!cleaned.contains("inaudible"));
    }

    #[test]
    fn missing_binary_is_unavailable() {
        let mut s = Settings::default();
        s.stt.binary = "whisper-cli-tidak-ada".into();
        s.stt.model = "/tmp/model-tidak-ada.bin".into();
        let engine = WhisperCli::from_settings(&s);
        let err = engine.check_ready().unwrap_err();
        assert!(matches!(err, AppError::Unavailable(_)));
    }

    #[test]
    fn find_in_path_does_not_find_nonsense() {
        assert!(find_in_path("definitely-not-a-real-binary-xyz").is_none());
    }

    #[cfg(unix)]
    #[test]
    fn transcribe_with_stub_binary_end_to_end() {
        use std::os::unix::fs::PermissionsExt;

        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!(
            "niri-ai-copilot-stub-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let bin = dir.join("fake-whisper");
        std::fs::write(
            &bin,
            "#!/bin/sh\necho 'halo dari stub whisper'\necho '[BLANK_AUDIO]'\necho 'whisper_print_timings: total time = 5 ms'\n",
        )
        .unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        let model = dir.join("ggml-stub.bin");
        std::fs::write(&model, b"stub model").unwrap();

        let mut s = Settings::default();
        s.stt.binary = bin.to_string_lossy().to_string();
        s.stt.model = model.to_string_lossy().to_string();
        let engine = WhisperCli::from_settings(&s);
        engine.check_ready().unwrap();

        let text = engine.transcribe(&[0u8; 64], "auto").unwrap();
        assert_eq!(text, "halo dari stub whisper");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
