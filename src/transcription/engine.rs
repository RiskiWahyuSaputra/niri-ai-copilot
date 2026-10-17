//! Abstraksi engine speech-to-text.
//!
//! Implementasi saat ini: `whisper-cli` (whisper.cpp lokal). Trait ini
//! memisahkan UI/pipeline dari engine konkret sehingga engine lain dapat
//! ditambahkan tanpa mengubah bagian lain aplikasi.

use crate::error::Result;

/// Engine STT lokal.
pub trait SttEngine: Send + Sync {
    /// Nama engine untuk ditampilkan di UI/log.
    fn name(&self) -> String;

    /// Periksa kesiapan (binary ada, model ada). Error harus ramah pengguna.
    fn check_ready(&self) -> Result<()>;

    /// Transkripsi satu potongan WAV (s16le mono 16 kHz).
    ///
    /// `language`: kode ISO-639-1 ("id", "en") atau "auto".
    fn transcribe(&self, wav: &[u8], language: &str) -> Result<String>;
}

/// Engine no-op untuk test / kondisi model belum terpasang.
///
/// Selalu mengembalikan teks kosong; tidak pernah dipakai untuk klaim fitur.
pub struct NullEngine;

impl SttEngine for NullEngine {
    fn name(&self) -> String {
        "null".into()
    }

    fn check_ready(&self) -> Result<()> {
        Ok(())
    }

    fn transcribe(&self, _wav: &[u8], _language: &str) -> Result<String> {
        Ok(String::new())
    }
}
