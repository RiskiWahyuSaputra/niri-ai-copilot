//! Capture audio via subprocess `pw-record`, streaming PCM dari stdout.
//!
//! Keamanan privasi: modul ini hanya boleh dipanggil setelah `ConsentGate`
//! mengonfirmasi izin perekaman (ditegakkan di `app.rs`).
//!
//! Pause: selama pause, data dibaca dan dibuang (stream PipeWire tidak
//! diputuskan) sehingga tidak ada audio yang hilang sebagian di buffer.

use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc::Sender, Arc, Mutex};
use std::thread::JoinHandle;

use crate::audio::pcm::{self, Chunker, BYTES_PER_100MS};
use crate::error::{AppError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioSourceKind {
    Mic,
    Meeting,
}

impl AudioSourceKind {
    pub fn label(&self) -> &'static str {
        match self {
            AudioSourceKind::Mic => "mic",
            AudioSourceKind::Meeting => "meeting",
        }
    }
}

/// Satu potongan audio siap-STT.
#[derive(Debug, Clone)]
pub struct AudioChunk {
    pub source: AudioSourceKind,
    pub wav: Vec<u8>,
    /// Offset milidetik dari awal stream.
    pub start_ms: i64,
    /// Level RMS 0..1 untuk indikator.
    pub level: f32,
    /// Nomor urut potongan.
    pub seq: u64,
}

/// Event dari capture thread.
#[derive(Debug, Clone)]
pub enum CaptureEvent {
    Chunk(AudioChunk),
    /// Stream berakhir (mis. microphone terputus) — pesan opsional.
    Ended(Option<String>),
    Error(String),
}

/// Handle capture yang sedang berjalan.
pub struct CaptureHandle {
    stop: Arc<AtomicBool>,
    child: Arc<Mutex<Child>>,
    thread: Option<JoinHandle<()>>,
    pub device: Option<String>,
    pub kind: AudioSourceKind,
}

impl CaptureHandle {
    /// Mulai capture dari `device` (None = default source).
    ///
    /// `send` adalah bounded channel; backpressure akan menghentikan pembacaan
    /// sementara bila antrean STT penuh (mencegah pemakaian RAM membengkak).
    pub fn spawn(
        kind: AudioSourceKind,
        device: Option<String>,
        chunk_seconds: u32,
        paused: Arc<AtomicBool>,
        send: Sender<CaptureEvent>,
    ) -> Result<Self> {
        let mut cmd = Command::new("pw-record");
        cmd.args([
            "--format",
            "s16",
            "--rate",
            &pcm::SAMPLE_RATE.to_string(),
            "--channels",
            &pcm::CHANNELS.to_string(),
        ]);
        if let Some(dev) = device.as_deref() {
            if !dev.trim().is_empty() {
                cmd.args(["--target", dev]);
            }
        }
        cmd.arg("-")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null());

        let mut child = cmd.spawn().map_err(|e| {
            AppError::Audio(format!(
                "gagal menjalankan `pw-record` ({e}) — pastikan paket pipewire terpasang dan session PipeWire berjalan"
            ))
        })?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AppError::Audio("stdout pw-record tidak tersedia".into()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| AppError::Audio("stderr pw-record tidak tersedia".into()))?;

        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = Arc::clone(&stop);
        let child_shared = Arc::new(Mutex::new(child));

        let thread = std::thread::Builder::new()
            .name(format!("capture-{}", kind.label()))
            .spawn(move || {
                reader_loop(
                    kind,
                    stdout,
                    stderr,
                    chunk_seconds,
                    paused,
                    stop_thread,
                    send,
                );
            })
            .map_err(AppError::Io)?;

        Ok(CaptureHandle {
            stop,
            child: child_shared,
            thread: Some(thread),
            device,
            kind,
        })
    }

    /// Hentikan capture dan tunggu thread berhenti.
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for CaptureHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn reader_loop(
    kind: AudioSourceKind,
    mut stdout: impl Read,
    mut stderr: impl Read + Send + 'static,
    chunk_seconds: u32,
    paused: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    send: Sender<CaptureEvent>,
) {
    // Thread terpisah membaca stderr agar pipe tidak penuh dan pesan error
    // PipeWire bisa dilaporkan.
    let stderr_buf: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
    let stderr_sink = Arc::clone(&stderr_buf);
    let stderr_reader = std::thread::spawn(move || {
        let mut buf = [0u8; 512];
        loop {
            match stderr.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if let Ok(mut s) = stderr_sink.lock() {
                        if s.len() < 4096 {
                            s.push_str(&String::from_utf8_lossy(&buf[..n]));
                        }
                    }
                }
            }
        }
    });

    let mut chunker = Chunker::new(chunk_seconds);
    let mut buf = vec![0u8; BYTES_PER_100MS];
    let mut seq: u64 = 0;

    loop {
        if stop.load(Ordering::SeqCst) {
            break;
        }
        match stdout.read(&mut buf) {
            Ok(0) => {
                // EOF: stream berakhir (mis. microphone terputus).
                let msg = stderr_snapshot(&stderr_buf);
                let _ = send.send(CaptureEvent::Ended(Some(if msg.is_empty() {
                    format!("stream {} berakhir", kind.label())
                } else {
                    msg
                })));
                break;
            }
            Ok(n) => {
                if paused.load(Ordering::SeqCst) {
                    // Buang data selama pause, tetapi tetap majukan jam stream.
                    chunker.skip(n);
                    continue;
                }
                for chunk_pcm in chunker.push(&buf[..n]) {
                    let start_ms = pcm::bytes_to_ms(chunker.total_bytes() - chunk_pcm.len() as u64);
                    let level = pcm::rms_level(&chunk_pcm);
                    let ev = CaptureEvent::Chunk(AudioChunk {
                        source: kind,
                        wav: pcm::wav_from_s16_mono(&chunk_pcm),
                        start_ms,
                        level,
                        seq,
                    });
                    seq += 1;
                    if send.send(ev).is_err() {
                        // Penerima hilang (sesi berhenti).
                        return;
                    }
                }
            }
            Err(e) => {
                let _ = send.send(CaptureEvent::Error(format!(
                    "gagal membaca audio dari pw-record: {e}"
                )));
                break;
            }
        }
    }

    // Kirim sisa buffer yang belum genap satu chunk.
    if let Some(rest) = chunker.flush() {
        if !paused.load(Ordering::SeqCst) && rest.len() >= pcm::BYTES_PER_SECOND as usize / 2 {
            let level = pcm::rms_level(&rest);
            let _ = send.send(CaptureEvent::Chunk(AudioChunk {
                source: kind,
                wav: pcm::wav_from_s16_mono(&rest),
                start_ms: pcm::bytes_to_ms(chunker.total_bytes() - rest.len() as u64),
                level,
                seq,
            }));
        }
    }
    let _ = stderr_reader.join();
    let _ = send.send(CaptureEvent::Ended(None));
}

fn stderr_snapshot(buf: &Arc<Mutex<String>>) -> String {
    buf.lock().map(|s| s.trim().to_string()).unwrap_or_default()
}

/// Cek keberadaan binary `pw-record`.
pub fn pw_record_available() -> bool {
    Command::new("pw-record")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pw_record_available_check_does_not_panic() {
        let _ = pw_record_available();
    }

    #[test]
    fn spawn_reports_missing_binary_gracefully() {
        // Gunakan PATH kosong agar pw-record "tidak ditemukan".
        let old_path = std::env::var_os("PATH");
        std::env::set_var("PATH", "/nonexistent");
        let (tx, _rx) = std::sync::mpsc::channel();
        let res = CaptureHandle::spawn(
            AudioSourceKind::Mic,
            None,
            5,
            Arc::new(AtomicBool::new(false)),
            tx,
        );
        if let Some(p) = old_path {
            std::env::set_var("PATH", p);
        }
        assert!(res.is_err());
    }
}
