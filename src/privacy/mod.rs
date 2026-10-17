//! Utilitas privasi: redaksi rahasia pada log/pesan error, gerbang izin
//! perekaman, dan retensi file audio.

pub mod consent;
pub mod redact;

pub use consent::{ConsentGate, ConsentRecord};
pub use redact::{redact_all, redact_secrets};

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
