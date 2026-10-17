//! Enumerasi sumber audio (microphone & monitor sink) via `pactl` JSON.
//!
//! `pactl` tersedia bersama paket `pipewire-pulse` di Arch Linux. Bila PipeWire
//! tidak aktif, fungsi mengembalikan error yang jelas untuk ditampilkan UI.

use std::process::Command;

use serde::Deserialize;

use crate::error::{AppError, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioDevice {
    /// Nama node PipeWire (dipakai sebagai `--target` pada pw-record).
    pub name: String,
    /// Deskripsi ramah manusia.
    pub description: String,
    /// True jika sumber ini monitor dari sebuah sink (audio keluar/meeting).
    pub is_monitor: bool,
    pub is_default: bool,
}

#[derive(Debug, Deserialize)]
struct PactlSource {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    monitor_of_sink: Option<serde_json::Value>,
}

/// Cek apakah PipeWire/PulseAudio berjalan.
pub fn pipewire_available() -> bool {
    Command::new("pactl")
        .arg("info")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Daftar seluruh sumber audio (microphone + monitor sink).
pub fn list_sources() -> Result<Vec<AudioDevice>> {
    let output = Command::new("pactl")
        .args(["-f", "json", "list", "sources"])
        .output()
        .map_err(|e| {
            AppError::Audio(format!(
                "tidak dapat menjalankan `pactl` ({e}) — pastikan paket pipewire-pulse terpasang"
            ))
        })?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::Audio(format!(
            "PipeWire/PulseAudio tidak aktif atau tidak dapat diakses: {}",
            err.trim()
        )));
    }
    parse_sources_json(&String::from_utf8_lossy(&output.stdout))
}

/// Daftar default source (untuk auto-confirm saat dialog izin dimatikan).
pub fn default_source() -> Result<Option<AudioDevice>> {
    let sources = list_sources()?;
    Ok(sources.into_iter().find(|s| s.is_default && !s.is_monitor))
}

/// Parser JSON `pactl -f json list sources` — dipisah agar bisa diuji.
pub fn parse_sources_json(raw: &str) -> Result<Vec<AudioDevice>> {
    let sources: Vec<PactlSource> = serde_json::from_str(raw)
        .map_err(|e| AppError::Audio(format!("format keluaran pactl tidak dikenal: {e}")))?;
    let default_name = default_source_name();
    let mut out: Vec<AudioDevice> = Vec::new();
    for s in sources {
        let is_monitor = s.monitor_of_sink.is_some() || s.name.ends_with(".monitor");
        let description = s
            .description
            .clone()
            .unwrap_or_else(|| s.name.clone());
        out.push(AudioDevice {
            is_default: default_name.as_deref() == Some(s.name.as_str()),
            name: s.name,
            description,
            is_monitor,
        });
    }
    Ok(out)
}

fn default_source_name() -> Option<String> {
    let output = Command::new("pactl").arg("get-default-source").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"[
        {"index": 40, "name": "alsa_input.pci-0000_00_1f.3.analog-stereo",
         "description": "Built-in Audio Analog Stereo"},
        {"index": 41, "name": "alsa_output.pci-0000_00_1f.3.analog-stereo.monitor",
         "description": "Monitor of Built-in Audio Analog Stereo", "monitor_of_sink": 0}
    ]"#;

    #[test]
    fn parses_mic_and_monitor() {
        let devices = parse_sources_json(SAMPLE).expect("parse");
        assert_eq!(devices.len(), 2);
        assert!(!devices[0].is_monitor);
        assert!(devices[1].is_monitor);
        assert!(devices[1].name.ends_with(".monitor"));
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_sources_json("bukan json").is_err());
    }
}
