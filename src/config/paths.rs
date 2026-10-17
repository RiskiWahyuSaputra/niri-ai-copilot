//! Lokasi file & direktori mengikuti spesifikasi XDG.
//!
//! Semua path dapat dioverride lewat environment variable untuk kebutuhan
//! testing maupun instalasi non-standar.

use std::path::PathBuf;

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).map(PathBuf::from).filter(|p| !p.as_os_str().is_empty())
}

/// Direktori config aplikasi: `~/.config/niri-ai-copilot`.
pub fn config_dir() -> PathBuf {
    if let Some(p) = env_path("NIRI_AI_COPILOT_CONFIG_DIR") {
        return p;
    }
    env_path("XDG_CONFIG_HOME")
        .unwrap_or_else(|| home().join(".config"))
        .join("niri-ai-copilot")
}

/// Direktori data aplikasi: `~/.local/share/niri-ai-copilot`.
pub fn data_dir() -> PathBuf {
    if let Some(p) = env_path("NIRI_AI_COPILOT_DATA_DIR") {
        return p;
    }
    env_path("XDG_DATA_HOME")
        .unwrap_or_else(|| home().join(".local/share"))
        .join("niri-ai-copilot")
}

/// Direktori penyimpanan audio opsional (hanya dipakai jika diaktifkan pengguna).
pub fn recordings_dir() -> PathBuf {
    data_dir().join("recordings")
}

/// Lokasi database SQLite.
pub fn db_path() -> PathBuf {
    data_dir().join("copilot.db")
}

/// Lokasi file konfigurasi TOML.
pub fn config_file() -> PathBuf {
    config_dir().join("config.toml")
}

/// Lokasi file kredensial lokal (chmod 0600, tidak pernah masuk database).
pub fn credentials_file() -> PathBuf {
    config_dir().join("credentials.toml")
}

/// Socket IPC untuk integrasi shortcut Niri.
pub fn socket_path() -> PathBuf {
    env_path("XDG_RUNTIME_DIR")
        .unwrap_or_else(std::env::temp_dir)
        .join("niri-ai-copilot.sock")
}

fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

/// Pastikan direktori config & data ada.
pub fn ensure_dirs() -> std::io::Result<()> {
    std::fs::create_dir_all(config_dir())?;
    std::fs::create_dir_all(data_dir())?;
    Ok(())
}
