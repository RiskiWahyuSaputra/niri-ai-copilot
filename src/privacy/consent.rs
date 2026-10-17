//! Gerbang izin perekaman.
//!
//! Perekaman **hanya** dimulai setelah `ConsentGate::confirm` dipanggil.
//! Tidak ada jalur lain (shortcut, IPC, CLI) yang boleh melewati gerbang ini.

use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{AppError, Result};

#[derive(Debug, Clone)]
pub struct ConsentRecord {
    /// Audio source yang disetujui (nama node PipeWire) — kosong berarti default.
    pub sources: Vec<String>,
    /// Apakah audio meeting (monitor source) termasuk yang disetujui.
    pub include_meeting_audio: bool,
    /// Unix epoch millis saat konfirmasi.
    pub confirmed_at_ms: i64,
}

#[derive(Debug, Default)]
pub struct ConsentGate {
    record: Mutex<Option<ConsentRecord>>,
}

impl ConsentGate {
    pub fn new() -> Self {
        Self::default()
    }

    /// Catat konfirmasi pengguna untuk sesi berjalan.
    pub fn confirm(&self, sources: Vec<String>, include_meeting_audio: bool) {
        let record = ConsentRecord {
            sources,
            include_meeting_audio,
            confirmed_at_ms: now_ms(),
        };
        *self.record.lock().expect("consent lock poisoned") = Some(record);
    }

    /// Cabut izin (dipanggil saat sesi berakhir / stop).
    pub fn revoke(&self) {
        *self.record.lock().expect("consent lock poisoned") = None;
    }

    pub fn is_confirmed(&self) -> bool {
        self.record
            .lock()
            .expect("consent lock poisoned")
            .is_some()
    }

    pub fn current(&self) -> Option<ConsentRecord> {
        self.record.lock().expect("consent lock poisoned").clone()
    }

    /// Wajib dipanggil sebelum memulai capture audio apa pun.
    pub fn require(&self) -> Result<ConsentRecord> {
        self.current().ok_or(AppError::ConsentRequired)
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_blocks_until_confirmed() {
        let gate = ConsentGate::new();
        assert!(gate.require().is_err());
        gate.confirm(vec!["mic".into()], false);
        assert!(gate.is_confirmed());
        let rec = gate.require().expect("should be confirmed");
        assert_eq!(rec.sources, vec!["mic".to_string()]);
        gate.revoke();
        assert!(gate.require().is_err());
    }
}
