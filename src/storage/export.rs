//! Ekspor transkrip sesi ke TXT atau Markdown.

use crate::error::Result;
use crate::storage::db::{format_duration_ms, format_ts, Db};
use crate::storage::session;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Txt,
    Markdown,
}

impl ExportFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            ExportFormat::Txt => "txt",
            ExportFormat::Markdown => "md",
        }
    }
}

/// Bangun konten ekspor lengkap (metadata, ringkasan, transkrip, pertanyaan).
pub fn export_session(db: &Db, session_id: i64, format: ExportFormat) -> Result<String> {
    let info = session::get_session(db, session_id)?
        .ok_or_else(|| crate::error::AppError::Other(format!("sesi {session_id} tidak ada")))?;
    let transcript = session::transcript_for(db, session_id)?;
    let questions = session::questions_for(db, session_id)?;

    let started = format_ts(info.started_at_ms);
    let ended = info
        .ended_at_ms
        .map(format_ts)
        .unwrap_or_else(|| "(belum selesai)".into());

    let mut out = String::new();
    match format {
        ExportFormat::Txt => {
            out.push_str(&format!("Sesi: {}\n", info.title));
            out.push_str(&format!("Jenis: {}\n", info.kind.label()));
            out.push_str(&format!("Mulai: {started}\nSelesai: {ended}\n\n"));
            if let Some(summary) = &info.summary {
                out.push_str("== Ringkasan ==\n");
                out.push_str(summary.trim());
                out.push_str("\n\n");
            }
            out.push_str("== Transkrip ==\n");
            for seg in &transcript {
                out.push_str(&format!(
                    "[{}] [{}] {}\n",
                    format_duration_ms(seg.ts_ms),
                    seg.source.label(),
                    seg.text.trim()
                ));
            }
            if !questions.is_empty() {
                out.push_str("\n== Pertanyaan Terdeteksi ==\n");
                for q in &questions {
                    out.push_str(&format!("- ({}) {}", q.category.as_deref().unwrap_or("-"), q.text));
                    if !q.keywords.is_empty() {
                        out.push_str(&format!(" [kata kunci: {}]", q.keywords.join(", ")));
                    }
                    out.push('\n');
                }
            }
        }
        ExportFormat::Markdown => {
            out.push_str(&format!("# {}\n\n", info.title));
            out.push_str(&format!("- **Jenis:** {}\n", info.kind.label()));
            out.push_str(&format!("- **Mulai:** {started}\n"));
            out.push_str(&format!("- **Selesai:** {ended}\n\n"));
            if let Some(summary) = &info.summary {
                out.push_str("## Ringkasan\n\n");
                out.push_str(summary.trim());
                out.push_str("\n\n");
            }
            out.push_str("## Transkrip\n\n");
            for seg in &transcript {
                out.push_str(&format!(
                    "- `{}` **{}** — {}\n",
                    format_duration_ms(seg.ts_ms),
                    seg.source.label(),
                    seg.text.trim()
                ));
            }
            if !questions.is_empty() {
                out.push_str("\n## Pertanyaan Terdeteksi\n\n");
                for q in &questions {
                    out.push_str(&format!(
                        "- **{}** — {}\n",
                        q.category.as_deref().unwrap_or("pertanyaan"),
                        q.text
                    ));
                }
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::session::{self, SegmentSource, SessionKind};

    #[test]
    fn exports_markdown() {
        let db = Db::open_in_memory().unwrap();
        let id = session::create_session(&db, &SessionKind::Live, "Interview Backend").unwrap();
        session::insert_transcript(&db, id, 1500, &SegmentSource::Meeting, "Tell me about SQL")
            .unwrap();
        session::insert_question(&db, id, 1500, "Tell me about SQL", Some("technical"), &["SQL".into()])
            .unwrap();
        let md = export_session(&db, id, ExportFormat::Markdown).unwrap();
        assert!(md.contains("# Interview Backend"));
        assert!(md.contains("00:00:01"));
        assert!(md.contains("Tell me about SQL"));
    }
}
