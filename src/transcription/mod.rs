pub mod engine;
pub mod pipeline;
pub mod whisper_cli;

pub use engine::SttEngine;
pub use pipeline::{SttEvent, SttJob, SttWorker};
pub use whisper_cli::WhisperCli;

use crate::config::Settings;
use std::sync::Arc;

/// Bangun engine STT sesuai settings.
///
/// Saat ini hanya "whisper-cli" yang diimplementasikan; engine tak dikenal
/// jatuh kembali ke whisper-cli dengan peringatan di UI nanti.
pub fn build_engine(settings: &Settings) -> Arc<dyn SttEngine> {
    Arc::new(WhisperCli::from_settings(settings))
}

/// Deskripsi status STT untuk UI (tanpa menjalankan transkripsi).
pub fn status_text(settings: &Settings) -> String {
    let engine = build_engine(settings);
    match engine.check_ready() {
        Ok(()) => format!("STT siap: {}", engine.name()),
        Err(e) => format!("STT belum siap: {e}"),
    }
}
