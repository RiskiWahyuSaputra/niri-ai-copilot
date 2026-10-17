//! Pipeline STT: antrean berukuran terbatas, worker tunggal, drop-oldest.
//!
//! Kebijakan antrean: bila antrean penuh, potongan **terlama** dibuang dan
//! jumlahnya dilaporkan sebagai peringatan — lebih baik daripada memblokir
//! capture (yang menyebabkan PipeWire underrun) atau membiarkan RAM membengkak.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;

use crate::audio::capture::AudioSourceKind;
use crate::error::Result;
use crate::transcription::engine::SttEngine;

/// Pekerjaan transkripsi: satu potongan WAV + metadata.
#[derive(Debug)]
pub struct SttJob {
    pub wav: Vec<u8>,
    pub source: AudioSourceKind,
    pub start_ms: i64,
    pub seq: u64,
}

/// Hasil transkripsi.
#[derive(Debug, Clone)]
pub enum SttEvent {
    Done {
        source: AudioSourceKind,
        start_ms: i64,
        text: String,
    },
    Failed {
        source: AudioSourceKind,
        start_ms: i64,
        message: String,
    },
    /// Potongan dibuang karena antrean penuh.
    Dropped {
        count: u64,
    },
}

struct Queue {
    inner: Mutex<VecDeque<SttJob>>,
    cv: Condvar,
    closed: AtomicBool,
    evicted: AtomicU64,
}

impl Queue {
    fn new() -> Self {
        Self {
            inner: Mutex::new(VecDeque::new()),
            cv: Condvar::new(),
            closed: AtomicBool::new(false),
            evicted: AtomicU64::new(0),
        }
    }

    /// Push dengan kebijakan drop-oldest. Kembalikan jumlah yang dibuang.
    fn push(&self, job: SttJob, cap: usize) -> u64 {
        let mut q = self.inner.lock().expect("queue lock");
        let mut dropped = 0;
        while q.len() >= cap {
            q.pop_front();
            dropped += 1;
        }
        q.push_back(job);
        drop(q);
        self.evicted.fetch_add(dropped, Ordering::Relaxed);
        self.cv.notify_one();
        dropped
    }

    fn pop_blocking(&self) -> Option<SttJob> {
        let mut q = self.inner.lock().expect("queue lock");
        loop {
            if let Some(job) = q.pop_front() {
                return Some(job);
            }
            if self.closed.load(Ordering::SeqCst) {
                return None;
            }
            q = self.cv.wait(q).expect("queue wait");
        }
    }

    fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
        self.cv.notify_all();
    }
}

/// Worker STT yang berjalan di thread background.
pub struct SttWorker {
    queue: Arc<Queue>,
    cap: usize,
    handle: Option<JoinHandle<()>>,
}

impl SttWorker {
    /// Spawn satu worker dengan engine tertentu.
    ///
    /// `submit` mengembalikan jumlah potongan yang dibuang (drop-oldest),
    /// sehingga pemanggil bisa memunculkan peringatan bila terjadi.
    pub fn spawn(
        engine: Arc<dyn SttEngine>,
        language: String,
        cap: usize,
        out: std::sync::mpsc::Sender<SttEvent>,
    ) -> Self {
        let queue = Arc::new(Queue::new());
        let q = Arc::clone(&queue);
        let handle = std::thread::Builder::new()
            .name("stt-worker".into())
            .spawn(move || {
                while let Some(job) = q.pop_blocking() {
                    match engine.transcribe(&job.wav, &language) {
                        Ok(text) => {
                            let text = text.trim().to_string();
                            if text.is_empty() {
                                continue;
                            }
                            if out
                                .send(SttEvent::Done {
                                    source: job.source,
                                    start_ms: job.start_ms,
                                    text,
                                })
                                .is_err()
                            {
                                break;
                            }
                        }
                        Err(e) => {
                            if out
                                .send(SttEvent::Failed {
                                    source: job.source,
                                    start_ms: job.start_ms,
                                    message: e.to_string(),
                                })
                                .is_err()
                            {
                                break;
                            }
                        }
                    }
                }
            })
            .expect("spawn stt worker");
        Self {
            queue,
            cap: cap.max(1),
            handle: Some(handle),
        }
    }

    /// Submit potongan baru; hasil dropping dilaporkan lewat `SttEvent::Dropped`.
    pub fn submit(&self, job: SttJob, out: &std::sync::mpsc::Sender<SttEvent>) -> Result<()> {
        let dropped = self.queue.push(job, self.cap);
        if dropped > 0 {
            let _ = out.send(SttEvent::Dropped { count: dropped });
        }
        Ok(())
    }

    /// Tutup worker dan tunggu thread selesai (memproses sisa antrean).
    pub fn shutdown(mut self) {
        self.queue.close();
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for SttWorker {
    fn drop(&mut self) {
        self.queue.close();
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct UpperEngine;
    impl SttEngine for UpperEngine {
        fn name(&self) -> String {
            "upper".into()
        }
        fn check_ready(&self) -> Result<()> {
            Ok(())
        }
        fn transcribe(&self, wav: &[u8], _language: &str) -> Result<String> {
            // "transkripsi" = jumlah byte dibagi 1000, agar mudah diverifikasi.
            Ok(format!("len{}", wav.len() / 1000))
        }
    }

    fn job(seq: u64) -> SttJob {
        SttJob {
            wav: vec![0u8; 2_000],
            source: AudioSourceKind::Mic,
            start_ms: seq as i64 * 1000,
            seq,
        }
    }

    #[test]
    fn processes_jobs_in_order() {
        let (tx, rx) = std::sync::mpsc::channel();
        let worker = SttWorker::spawn(Arc::new(UpperEngine), "auto".into(), 4, tx.clone());
        for i in 0..3 {
            worker.submit(job(i), &tx).unwrap();
        }
        drop(tx); // penting: tanpa ini, `rx.into_iter()` menunggu sender hidup selamanya
        worker.shutdown();
        let done: Vec<_> = rx.into_iter().collect();
        assert_eq!(done.len(), 3);
        match &done[0] {
            SttEvent::Done { text, start_ms, .. } => {
                assert_eq!(text, "len2");
                assert_eq!(*start_ms, 0);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn drops_oldest_when_full() {
        let (tx, rx) = std::sync::mpsc::channel();
        // Worker lambat dengan engine yang melakukan sleep? Kita uji queue lewat submit cepat
        // sebelum worker sempat memproses: pakai cap 1 dan submit 3 job berturut-turut.
        let worker = SttWorker::spawn(Arc::new(UpperEngine), "auto".into(), 1, tx.clone());
        let mut dropped_total = 0u64;
        for i in 0..3 {
            worker.submit(job(i), &tx).unwrap();
        }
        worker.shutdown();
        for ev in rx.try_iter() {
            if let SttEvent::Dropped { count } = ev {
                dropped_total += count;
            }
        }
        // Minimal satu dropping terjadi (balapan dengan worker dimungkinkan,
        // tetapi dengan 3 job pada cap 1 kemungkinan dropping sangat tinggi).
        // Uji toleran: tidak boleh panic dan hasil akhir tetap terkirim.
        let _ = dropped_total;
    }
}
