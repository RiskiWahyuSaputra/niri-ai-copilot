//! Utilitas privasi: redaksi rahasia pada log/pesan error, gerbang izin
//! perekaman, dan retensi file audio.

pub mod consent;
pub mod redact;

pub use consent::ConsentGate;
pub use redact::redact_all;

/// Teks disclosure yang ditampilkan sebelum data dikirim ke layanan AI eksternal.
pub fn external_ai_disclosure() -> &'static str {
    "Bagian transkrip terbaru (dan konteks profil/CV yang relevan) akan dikirim \
     ke layanan AI eksternal yang dikonfigurasi (Groq) untuk menghasilkan saran \
     jawaban. Audio tidak dikirim — hanya teks. Lanjutkan hanya jika Anda setuju \
     dan sudah memiliki izin dari peserta sesi."
}

/// Teks konfirmasi izin perekaman.
pub fn recording_consent_text() -> &'static str {
    "Saya mengonfirmasi bahwa saya telah mendapatkan izin dari semua peserta \
     untuk merekam dan memproses audio percakapan ini secara lokal."
}

/// Hapus file audio rekaman yang lebih tua dari `days` hari.
/// `days == 0` berarti "jangan simpan audio" — seluruh direktori dibersihkan.
/// Mengembalikan jumlah file yang dihapus.
pub fn enforce_audio_retention(dir: &std::path::Path, days: u32) -> std::io::Result<usize> {
    if !dir.exists() {
        return Ok(0);
    }
    let cutoff = if days == 0 {
        None
    } else {
        Some(
            std::time::SystemTime::now()
                - std::time::Duration::from_secs(u64::from(days) * 86_400),
        )
    };
    let mut removed = 0;
    remove_old_files(dir, cutoff, &mut removed)?;
    Ok(removed)
}

fn remove_old_files(
    dir: &std::path::Path,
    cutoff: Option<std::time::SystemTime>,
    removed: &mut usize,
) -> std::io::Result<()> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .collect();
    entries.sort_by_key(|e| e.path());
    for entry in entries {
        let path = entry.path();
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_dir() {
            remove_old_files(&path, cutoff, removed)?;
            let _ = std::fs::remove_dir(&path);
        } else {
            let expired = match cutoff {
                None => true,
                Some(cutoff) => meta
                    .modified()
                    .map(|m| m < cutoff)
                    .unwrap_or(false),
            };
            if expired && std::fs::remove_file(&path).is_ok() {
                *removed += 1;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retention_skips_nonexistent_dir() {
        let dir = std::env::temp_dir().join("niri-ai-copilot-tidak-ada-xyz");
        assert_eq!(enforce_audio_retention(&dir, 7).unwrap(), 0);
    }

    #[test]
    fn retention_keeps_fresh_files_and_removes_all_when_zero() {
        let dir = std::env::temp_dir().join(format!(
            "niri-ai-copilot-retensi-{}",
            std::process::id()
        ));
        let nested = dir.join("session-1");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("00001-mic.wav"), b"RIFF").unwrap();

        assert_eq!(enforce_audio_retention(&dir, 7).unwrap(), 0);
        assert!(nested.join("00001-mic.wav").exists());

        assert_eq!(enforce_audio_retention(&dir, 0).unwrap(), 1);
        assert!(!nested.join("00001-mic.wav").exists());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
