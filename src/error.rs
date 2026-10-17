//! Tipe error terpusat untuk seluruh aplikasi.
//!
//! Semua pesan yang ditampilkan ke pengguna harus ramah dibaca dan tidak boleh
//! membocorkan API key / data sensitif (lihat `privacy::redact`).

/// Error aplikasi.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("konfigurasi tidak valid: {0}")]
    Config(String),

    #[error("kesalahan I/O: {0}")]
    Io(#[from] std::io::Error),

    #[error("kesalahan database: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("kesalahan JSON: {0}")]
    Json(#[from] serde_json::Error),

    #[error("gagal membaca TOML: {0}")]
    TomlDe(#[from] toml::de::Error),

    #[error("gagal menulis TOML: {0}")]
    TomlSer(#[from] toml::ser::Error),

    #[error("audio: {0}")]
    Audio(String),

    #[error("speech-to-text: {0}")]
    Stt(String),

    #[error("AI provider: {0}")]
    Ai(String),

    #[error("API key tidak tersedia — set environment variable {0} atau simpan via halaman Settings")]
    MissingApiKey(String),

    #[error("rate limit dari provider AI — coba lagi sekitar {retry_after} detik")]
    RateLimited { retry_after: u64 },

    #[error("masalah jaringan: {0}")]
    Network(String),

    #[error("timeout saat menunggu respons layanan")]
    Timeout,

    #[error("izin perekaman belum dikonfirmasi pengguna")]
    ConsentRequired,

    #[error("fitur belum tersedia di sistem ini: {0}")]
    Unavailable(String),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, AppError>;
