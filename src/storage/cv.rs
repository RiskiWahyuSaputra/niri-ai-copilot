//! Ekstraksi teks CV dari file (PDF via `pdftotext`, atau teks langsung).
//!
//! PDF hanya diproses bila `pdftotext` (paket `poppler`) tersedia; jika tidak,
//! pengguna diminta menempel teks secara manual — aplikasi tidak mengklaim
//! bisa membaca PDF tanpa alat tersebut.

use std::path::Path;
use std::process::Command;

use crate::error::{AppError, Result};

/// Batas karakter CV yang disimpan (mencegah database membengkak).
pub const MAX_CV_CHARS: usize = 120_000;

/// Ekstrak teks dari file .pdf, .txt, atau .md.
pub fn extract_text(path: &Path) -> Result<String> {
    if !path.is_file() {
        return Err(AppError::Other(format!(
            "file tidak ditemukan: {}",
            path.display()
        )));
    }
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let text = match ext.as_str() {
        "pdf" => extract_pdf(path)?,
        "txt" | "md" | "text" | "" => std::fs::read_to_string(path)?,
        other => {
            return Err(AppError::Other(format!(
                "format CV `.{other}` belum didukung — gunakan PDF, TXT, atau MD"
            )))
        }
    };
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err(AppError::Other(
            "tidak ada teks yang dapat diekstrak dari file tersebut (mungkin PDF hasil scan — \
             tempel teks secara manual)"
                .into(),
        ));
    }
    if text.chars().count() > MAX_CV_CHARS {
        return Ok(text.chars().take(MAX_CV_CHARS).collect());
    }
    Ok(text)
}

fn extract_pdf(path: &Path) -> Result<String> {
    let output = Command::new("pdftotext")
        .args(["-layout", "-enc", "UTF-8"])
        .arg(path)
        .arg("-")
        .output()
        .map_err(|_| {
            AppError::Unavailable(
                "`pdftotext` tidak ditemukan — pasang paket `poppler` \
                 atau tempel isi CV secara manual di kolom teks CV"
                    .into(),
            )
        })?;
    if !output.status.success() {
        return Err(AppError::Other(format!(
            "pdftotext gagal: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_txt_file() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("cv.txt");
        std::fs::write(&p, "  Nama: Riski\nSkill: Rust  ").unwrap();
        assert_eq!(extract_text(&p).unwrap(), "Nama: Riski\nSkill: Rust");
    }

    #[test]
    fn missing_file_errors() {
        assert!(extract_text(Path::new("/tmp/tidak-ada-cv-xyz.txt")).is_err());
    }

    #[test]
    fn unsupported_extension_errors() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("cv.docx");
        std::fs::write(&p, "x").unwrap();
        let err = extract_text(&p).unwrap_err();
        assert!(err.to_string().contains("belum didukung"));
    }
}
