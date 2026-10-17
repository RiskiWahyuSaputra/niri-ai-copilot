//! Model konfigurasi aplikasi (TOML) beserta load/save dan resolusi API key.
//!
//! API key **tidak pernah** disimpan di source code, log, atau database.
//! Prioritas resolusi: environment variable (termasuk `.env` lokal) →
//! file kredensial dengan permission 0600 di direktori config.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::config::paths;
use crate::error::{AppError, Result};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl Default for Corner {
    fn default() -> Self {
        Corner::TopRight
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AnswerStyle {
    Concise,
    Professional,
    Technical,
    Star,
    BeginnerFriendly,
    EnglishInterview,
}

impl Default for AnswerStyle {
    fn default() -> Self {
        AnswerStyle::Professional
    }
}

impl AnswerStyle {
    pub fn label(&self) -> &'static str {
        match self {
            AnswerStyle::Concise => "Concise",
            AnswerStyle::Professional => "Professional",
            AnswerStyle::Technical => "Technical",
            AnswerStyle::Star => "STAR Method",
            AnswerStyle::BeginnerFriendly => "Beginner Friendly",
            AnswerStyle::EnglishInterview => "English Interview",
        }
    }

    pub fn all() -> [AnswerStyle; 6] {
        [
            AnswerStyle::Concise,
            AnswerStyle::Professional,
            AnswerStyle::Technical,
            AnswerStyle::Star,
            AnswerStyle::BeginnerFriendly,
            AnswerStyle::EnglishInterview,
        ]
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AnswerLength {
    Short,
    Medium,
    Detailed,
}

impl Default for AnswerLength {
    fn default() -> Self {
        AnswerLength::Medium
    }
}

impl AnswerLength {
    pub fn label(&self) -> &'static str {
        match self {
            AnswerLength::Short => "Short",
            AnswerLength::Medium => "Medium",
            AnswerLength::Detailed => "Detailed",
        }
    }

    pub fn hint(&self) -> &'static str {
        match self {
            AnswerLength::Short => "1-2 kalimat, langsung ke inti",
            AnswerLength::Medium => "1 paragraf singkat + key points",
            AnswerLength::Detailed => "STAR lengkap dengan detail dan contoh",
        }
    }

    pub fn all() -> [AnswerLength; 3] {
        [AnswerLength::Short, AnswerLength::Medium, AnswerLength::Detailed]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    /// Opacity widget (0.55 – 1.0).
    pub opacity: f64,
    /// Lebar widget compact dalam pixel.
    pub compact_width: i32,
    /// Ukuran font dasar dalam pixel.
    pub font_size: f64,
    /// Sudut layar tempat widget hinggap.
    pub corner: Corner,
    /// Margin dari tepi layar.
    pub margin: i32,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            opacity: 0.94,
            compact_width: 330,
            font_size: 14.0,
            corner: Corner::TopRight,
            margin: 24,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Audio {
    /// Node PipeWire/PulseAudio untuk microphone. `None` = default source.
    pub input_device: Option<String>,
    /// Monitor source untuk menangkap audio meeting (opsional).
    pub meeting_device: Option<String>,
    /// Tangkap microphone + audio meeting sekaligus.
    pub capture_both: bool,
    /// Bahasa STT: "auto", "id", atau "en".
    pub language: String,
    /// Tampilkan indikator level input.
    pub show_level_meter: bool,
    /// Durasi potongan audio sebelum dikirim ke STT (detik).
    pub chunk_seconds: u32,
    /// Jumlah maksimum potongan audio dalam antrean STT (backpressure).
    pub max_chunk_queue: usize,
}

impl Default for Audio {
    fn default() -> Self {
        Self {
            input_device: None,
            meeting_device: None,
            capture_both: false,
            language: "auto".into(),
            show_level_meter: true,
            chunk_seconds: 5,
            max_chunk_queue: 4,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Stt {
    /// Nama engine STT. Saat ini: "whisper-cli".
    pub engine: String,
    /// Binary whisper.cpp. Bisa dioverride lewat env NIRI_AI_COPILOT_WHISPER_CLI.
    pub binary: String,
    /// Path model GGML/GGUF whisper.cpp.
    pub model: String,
    /// Jumlah thread. 0 = otomatis (CPU-friendly: min(4, cores/2)).
    pub threads: u32,
    /// Terjemahkan hasil ke bahasa Inggris.
    pub translate: bool,
}

impl Default for Stt {
    fn default() -> Self {
        Self {
            engine: "whisper-cli".into(),
            binary: "whisper-cli".into(),
            model: String::new(),
            threads: 0,
            translate: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Ai {
    /// Provider AI. Saat ini: "groq".
    pub provider: String,
    /// Nama environment variable yang menyimpan API key.
    pub api_key_env: String,
    /// Model Groq (dapat dikonfigurasi).
    pub model: String,
    /// Base URL OpenAI-compatible API.
    pub base_url: String,
    /// Bahasa jawaban: "auto" (ikuti bahasa pertanyaan), "id", atau "en".
    pub response_language: String,
    pub answer_length: AnswerLength,
    pub answer_style: AnswerStyle,
    /// Timeout request (detik).
    pub timeout_secs: u64,
    /// Maksimum retry untuk status 429/5xx (tidak pernah retry tanpa batas).
    pub max_retries: u32,
    /// Jumlah segmen transkrip terakhir yang dikirim sebagai konteks.
    pub context_segments: usize,
    /// Minimum jumlah kata agar sebuah kalimat dianggap kandidat pertanyaan.
    pub question_min_words: usize,
}

impl Default for Ai {
    fn default() -> Self {
        Self {
            provider: "groq".into(),
            api_key_env: "GROQ_API_KEY".into(),
            model: "llama-3.3-70b-versatile".into(),
            base_url: "https://api.groq.com/openai/v1".into(),
            response_language: "auto".into(),
            answer_length: AnswerLength::default(),
            answer_style: AnswerStyle::default(),
            timeout_secs: 30,
            max_retries: 2,
            context_segments: 12,
            question_min_words: 3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Privacy {
    /// Wajibkan dialog konfirmasi izin sebelum merekam.
    pub confirm_recording: bool,
    /// Tampilkan timer berjalan pada indikator. Indikator REC selalu tampil
    /// selama perekaman aktif dan tidak dapat dimatikan sepenuhnya.
    pub show_recording_timer: bool,
    /// Simpan file audio ke disk. Default: false.
    pub save_audio: bool,
    /// Retensi file audio (hari). 0 = simpan selamanya.
    pub audio_retention_days: u32,
    /// Tampilkan disclosure sebelum mengirim data ke layanan AI eksternal.
    pub disclose_external_ai: bool,
}

impl Default for Privacy {
    fn default() -> Self {
        Self {
            confirm_recording: true,
            show_recording_timer: true,
            save_audio: false,
            audio_retention_days: 7,
            disclose_external_ai: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Performance {
    /// Mode hemat CPU: thread STT lebih sedikit, chunk lebih besar.
    pub cpu_friendly: bool,
    /// Maksimum segmen transkrip yang disimpan di memori untuk UI.
    pub max_transcript_context: usize,
    /// Debounce request AI (milidetik).
    pub ai_debounce_ms: u64,
    /// Kurangi pemrosesan latar belakang saat tidak ada sesi aktif.
    pub reduce_background: bool,
}

impl Default for Performance {
    fn default() -> Self {
        Self {
            cpu_friendly: false,
            max_transcript_context: 500,
            ai_debounce_ms: 2500,
            reduce_background: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Practice {
    /// Practice Mode adalah mode default.
    pub default_mode: bool,
    /// Kategori latihan: "mixed", "hr", "behavioral", "technical", "english".
    pub category: String,
}

impl Default for Practice {
    fn default() -> Self {
        Self {
            default_mode: true,
            category: "mixed".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Settings {
    pub appearance: Appearance,
    pub audio: Audio,
    pub stt: Stt,
    pub ai: Ai,
    pub privacy: Privacy,
    pub performance: Performance,
    pub practice: Practice,
}

impl Settings {
    /// Muat konfigurasi dari disk; dibuat dengan nilai default jika belum ada.
    pub fn load() -> Result<Self> {
        let path = paths::config_file();
        Self::load_from(&path)
    }

    pub fn load_from(path: &Path) -> Result<Self> {
        if !path.exists() {
            let s = Settings::default();
            s.save_to(path)?;
            return Ok(s);
        }
        let raw = std::fs::read_to_string(path)?;
        let mut s: Settings = toml::from_str(&raw)?;
        s.normalize();
        Ok(s)
    }

    pub fn save(&self) -> Result<()> {
        self.save_to(&paths::config_file())
    }

    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let raw = toml::to_string_pretty(self)?;
        std::fs::write(path, raw)?;
        Ok(())
    }

    /// Bersihkan nilai di luar rentang yang wajar.
    pub fn normalize(&mut self) {
        self.appearance.opacity = self.appearance.opacity.clamp(0.55, 1.0);
        self.appearance.compact_width = self.appearance.compact_width.clamp(260, 520);
        self.appearance.font_size = self.appearance.font_size.clamp(11.0, 22.0);
        self.appearance.margin = self.appearance.margin.clamp(0, 200);
        self.audio.chunk_seconds = self.audio.chunk_seconds.clamp(2, 30);
        self.audio.max_chunk_queue = self.audio.max_chunk_queue.clamp(1, 16);
        self.ai.context_segments = self.ai.context_segments.clamp(2, 40);
        self.ai.question_min_words = self.ai.question_min_words.clamp(2, 20);
        self.performance.ai_debounce_ms = self.performance.ai_debounce_ms.clamp(500, 15_000);
        self.performance.max_transcript_context =
            self.performance.max_transcript_context.clamp(50, 5000);
        if !["auto", "id", "en"].contains(&self.audio.language.as_str()) {
            self.audio.language = "auto".into();
        }
        if !["auto", "id", "en"].contains(&self.ai.response_language.as_str()) {
            self.ai.response_language = "auto".into();
        }
    }

    /// Resolusi API key: env var → file kredensial → None.
    pub fn resolve_api_key(&self) -> Option<String> {
        if let Ok(v) = std::env::var(&self.ai.api_key_env) {
            let v = v.trim().to_string();
            if !v.is_empty() {
                return Some(v);
            }
        }
        self.credential_from_file()
    }

    /// Baca API key dari file kredensial lokal (permission 0600).
    pub fn credential_from_file(&self) -> Option<String> {
        let raw = std::fs::read_to_string(paths::credentials_file()).ok()?;
        let value: toml::Value = toml::from_str(&raw).ok()?;
        let key = value
            .get("api_keys")?
            .get(&self.ai.provider)?
            .as_str()?
            .trim()
            .to_string();
        if key.is_empty() {
            None
        } else {
            Some(key)
        }
    }

    /// Simpan API key ke file kredensial dengan permission 0600.
    pub fn store_api_key(&self, api_key: &str) -> Result<()> {
        let path = paths::credentials_file();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut table: toml::value::Table = if path.exists() {
            std::fs::read_to_string(&path)
                .ok()
                .and_then(|raw| toml::from_str::<toml::Value>(&raw).ok())
                .and_then(|v| v.as_table().cloned())
                .unwrap_or_default()
        } else {
            Default::default()
        };
        let keys = table
            .entry("api_keys".to_string())
            .or_insert_with(|| toml::Value::Table(Default::default()));
        if let Some(keys) = keys.as_table_mut() {
            keys.insert(
                self.ai.provider.clone(),
                toml::Value::String(api_key.trim().to_string()),
            );
        }
        std::fs::write(&path, toml::to_string_pretty(&toml::Value::Table(table))?)?;
        restrict_permissions(&path)?;
        Ok(())
    }

    /// Hapus API key tersimpan dari file kredensial.
    pub fn clear_api_key(&self) -> Result<()> {
        let path = paths::credentials_file();
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }

    /// Lokasi model whisper yang efektif (config → env → None).
    pub fn resolved_whisper_model(&self) -> Option<PathBuf> {
        if let Ok(p) = std::env::var("NIRI_AI_COPILOT_WHISPER_MODEL") {
            if !p.trim().is_empty() {
                return Some(PathBuf::from(p));
            }
        }
        if self.stt.model.trim().is_empty() {
            None
        } else {
            Some(PathBuf::from(self.stt.model.trim()))
        }
    }

    /// Binary whisper.cpp yang efektif (env → config).
    pub fn resolved_whisper_binary(&self) -> String {
        if let Ok(p) = std::env::var("NIRI_AI_COPILOT_WHISPER_CLI") {
            if !p.trim().is_empty() {
                return p;
            }
        }
        self.stt.binary.clone()
    }
}

#[cfg(unix)]
fn restrict_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let perm = std::fs::Permissions::from_mode(0o600);
    std::fs::set_permissions(path, perm)?;
    Ok(())
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

/// Parser `.env` minimalis (tanpa dependency tambahan).
///
/// Menerima baris `KEY=VALUE`, komentar `#`, dan nilai berkutip sederhana.
/// Variabel yang sudah ada di environment tidak ditimpa.
pub fn load_dotenv_file(path: &Path) -> Result<()> {
    let raw = match std::fs::read_to_string(path) {
        Ok(r) => r,
        Err(_) => return Ok(()),
    };
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let mut value = value.trim().to_string();
        if (value.starts_with('"') && value.ends_with('"') && value.len() >= 2)
            || (value.starts_with('\'') && value.ends_with('\'') && value.len() >= 2)
        {
            value = value[1..value.len() - 1].to_string();
        }
        if key.is_empty() || std::env::var_os(key).is_some() {
            continue;
        }
        std::env::set_var(key, value);
    }
    Ok(())
}

/// Muat `.env` dari cwd (development) dan direktori config (installed).
pub fn load_dotenv() {
    let cwd = PathBuf::from(".env");
    let _ = load_dotenv_file(&cwd);
    let _ = load_dotenv_file(&paths::config_dir().join(".env"));
}

/// Baca nilai environment non-kosong.
pub fn env_nonempty(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

/// Error helper untuk pengaturan yang tidak valid.
pub fn invalid_setting(msg: impl Into<String>) -> AppError {
    AppError::Config(msg.into())
}
