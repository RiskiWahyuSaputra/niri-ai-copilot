//! Profil konteks AI: data nyata pengguna yang dipakai untuk menghasilkan saran.
//!
//! Prinsip: saran jawaban tidak boleh mengarang pengalaman. Bagian kosong akan
//! ditandai untuk dilengkapi pengguna.

use serde::{Deserialize, Serialize};

use crate::error::{AppError, Result};
use crate::storage::db::{now_ms, Db};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Profile {
    pub name: String,
    pub target_position: String,
    pub job_description: String,
    pub experience: String,
    pub skills: String,
    pub projects: String,
    pub education: String,
    pub certifications: String,
    /// Bahasa interview target: "id", "en", atau "auto".
    pub interview_language: String,
    /// Teks CV hasil ekstraksi (PDF → teks) atau tempel manual.
    pub cv_text: String,
    /// Path file CV asli (untuk referensi pengguna, tidak wajib ada).
    pub cv_path: String,
}

impl Profile {
    /// Daftar field penting yang masih kosong (ditampilkan sebagai peringatan).
    pub fn missing_fields(&self) -> Vec<&'static str> {
        let mut missing = Vec::new();
        if self.name.trim().is_empty() {
            missing.push("Nama");
        }
        if self.target_position.trim().is_empty() {
            missing.push("Posisi yang dilamar");
        }
        if self.experience.trim().is_empty() && self.cv_text.trim().is_empty() {
            missing.push("Pengalaman / CV");
        }
        if self.skills.trim().is_empty() {
            missing.push("Skill");
        }
        missing
    }

    pub fn is_empty(&self) -> bool {
        self == &Profile::default()
    }

    /// Ringkasan konteks untuk prompt AI. Hanya memuat field yang terisi.
    pub fn as_prompt_context(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        let mut push = |label: &str, value: &str| {
            let v = value.trim();
            if !v.is_empty() {
                parts.push(format!("{label}: {v}"));
            }
        };
        push("Nama", &self.name);
        push("Posisi yang dilamar", &self.target_position);
        push("Deskripsi pekerjaan", &self.job_description);
        push("Pengalaman", &self.experience);
        push("Skill", &self.skills);
        push("Project portofolio", &self.projects);
        push("Pendidikan", &self.education);
        push("Sertifikasi", &self.certifications);
        if !self.cv_text.trim().is_empty() {
            parts.push(format!("CV (teks): {}", truncate_chars(self.cv_text.trim(), 6000)));
        }
        if parts.is_empty() {
            "PROFIL KOSONG — semua bagian harus ditandai butuh dilengkapi, jangan mengarang.".into()
        } else {
            parts.join("\n")
        }
    }
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max).collect();
    out.push_str("\n[...dipotong...]");
    out
}

/// Simpan profil (single-row).
pub fn save_profile(db: &Db, profile: &Profile) -> Result<()> {
    let data = serde_json::to_string(profile)?;
    db.with(|conn| {
        conn.execute(
            "INSERT INTO profile (id, data, updated_at_ms) VALUES (1, ?1, ?2)
             ON CONFLICT(id) DO UPDATE SET data = excluded.data, updated_at_ms = excluded.updated_at_ms",
            rusqlite::params![data, now_ms()],
        )?;
        Ok(())
    })
}

/// Muat profil; profil default (kosong) jika belum pernah disimpan.
pub fn load_profile(db: &Db) -> Result<Profile> {
    db.with(|conn| {
        let mut stmt = conn.prepare("SELECT data FROM profile WHERE id = 1")?;
        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            let raw: String = row.get(0)?;
            let p: Profile = serde_json::from_str(&raw)
                .map_err(|e| AppError::Other(format!("profil korup: {e}")))?;
            Ok(p)
        } else {
            Ok(Profile::default())
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_profile() {
        let db = Db::open_in_memory().unwrap();
        let mut p = load_profile(&db).unwrap();
        assert!(p.is_empty());
        p.name = "Riski".into();
        p.skills = "Rust, Linux".into();
        save_profile(&db, &p).unwrap();
        let loaded = load_profile(&db).unwrap();
        assert_eq!(loaded.name, "Riski");
        assert_eq!(loaded.skills, "Rust, Linux");
    }

    #[test]
    fn prompt_context_marks_missing() {
        let p = Profile::default();
        assert!(p.missing_fields().contains(&"Nama"));
        let ctx = p.as_prompt_context();
        assert!(ctx.contains("PROFIL KOSONG"));
    }
}
