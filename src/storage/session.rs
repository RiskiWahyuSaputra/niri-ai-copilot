//! Repositori sesi, transkrip, pertanyaan, dan saran jawaban.

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, Result};
use crate::storage::db::{now_ms, Db};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionKind {
    /// Latihan interview dengan AI interviewer (mode default).
    Practice,
    /// Sesi interview langsung dengan bantuan AI.
    Live,
    /// Sesi bebas tanpa AI (hanya transkripsi).
    Transcription,
}

impl SessionKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            SessionKind::Practice => "practice",
            SessionKind::Live => "live",
            SessionKind::Transcription => "transcription",
        }
    }

    pub fn from_str_lossy(s: &str) -> Self {
        match s {
            "practice" => SessionKind::Practice,
            "live" => SessionKind::Live,
            _ => SessionKind::Transcription,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            SessionKind::Practice => "Latihan",
            SessionKind::Live => "Interview Langsung",
            SessionKind::Transcription => "Transkripsi",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SegmentSource {
    Mic,
    Meeting,
}

impl SegmentSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            SegmentSource::Mic => "mic",
            SegmentSource::Meeting => "meeting",
        }
    }

    pub fn from_str_lossy(s: &str) -> Self {
        match s {
            "meeting" => SegmentSource::Meeting,
            _ => SegmentSource::Mic,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            SegmentSource::Mic => "Mic",
            SegmentSource::Meeting => "Meeting",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: i64,
    pub kind: SessionKind,
    pub title: String,
    pub started_at_ms: i64,
    pub ended_at_ms: Option<i64>,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptSegment {
    pub ts_ms: i64,
    pub source: SegmentSource,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionRow {
    pub id: i64,
    pub session_id: i64,
    pub ts_ms: i64,
    pub text: String,
    pub category: Option<String>,
    pub keywords: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestionRow {
    pub id: i64,
    pub question_id: i64,
    pub style: String,
    pub content: String,
    pub created_at_ms: i64,
}

/// Buat sesi baru, kembalikan id.
pub fn create_session(db: &Db, kind: &SessionKind, title: &str) -> Result<i64> {
    db.with(|conn| {
        conn.execute(
            "INSERT INTO sessions (kind, title, started_at_ms) VALUES (?1, ?2, ?3)",
            params![kind.as_str(), title, now_ms()],
        )?;
        Ok(conn.last_insert_rowid())
    })
}

/// Tutup sesi dengan timestamp selesai dan ringkasan opsional.
pub fn end_session(db: &Db, session_id: i64, summary: Option<&str>) -> Result<()> {
    db.with(|conn| {
        conn.execute(
            "UPDATE sessions SET ended_at_ms = ?1, summary = COALESCE(?2, summary) WHERE id = ?3",
            params![now_ms(), summary, session_id],
        )?;
        Ok(())
    })
}

/// Perbarui judul sesi.
pub fn rename_session(db: &Db, session_id: i64, title: &str) -> Result<()> {
    db.with(|conn| {
        conn.execute(
            "UPDATE sessions SET title = ?1 WHERE id = ?2",
            params![title, session_id],
        )?;
        Ok(())
    })
}

/// Ambil metadata satu sesi.
pub fn get_session(db: &Db, session_id: i64) -> Result<Option<SessionInfo>> {
    db.with(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, kind, title, started_at_ms, ended_at_ms, summary FROM sessions WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![session_id])?;
        match rows.next()? {
            Some(row) => Ok(Some(SessionInfo {
                id: row.get(0)?,
                kind: SessionKind::from_str_lossy(&row.get::<_, String>(1)?),
                title: row.get(2)?,
                started_at_ms: row.get(3)?,
                ended_at_ms: row.get(4)?,
                summary: row.get(5)?,
            })),
            None => Ok(None),
        }
    })
}

/// Daftar sesi terbaru, opsional difilter dengan kata kunci judul/ringkasan.
pub fn list_sessions(db: &Db, search: Option<&str>, limit: usize) -> Result<Vec<SessionInfo>> {
    db.with(|conn| {
        let like = search.map(|s| format!("%{}%", s.replace('%', "\\%")));
        let mut stmt = conn.prepare(
            "SELECT id, kind, title, started_at_ms, ended_at_ms, summary
             FROM sessions
             WHERE (?1 IS NULL OR title LIKE ?1 OR summary LIKE ?1)
             ORDER BY started_at_ms DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![like, limit as i64], |row| {
            Ok(SessionInfo {
                id: row.get(0)?,
                kind: SessionKind::from_str_lossy(&row.get::<_, String>(1)?),
                title: row.get(2)?,
                started_at_ms: row.get(3)?,
                ended_at_ms: row.get(4)?,
                summary: row.get(5)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    })
}

/// Hapus sesi beserta seluruh data terkait (transkrip, pertanyaan, saran).
pub fn delete_session(db: &Db, session_id: i64) -> Result<()> {
    db.with(|conn| {
        let tx = conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM suggestions WHERE question_id IN
                (SELECT id FROM questions WHERE session_id = ?1)",
            params![session_id],
        )?;
        tx.execute("DELETE FROM questions WHERE session_id = ?1", params![session_id])?;
        tx.execute("DELETE FROM transcripts WHERE session_id = ?1", params![session_id])?;
        tx.execute("DELETE FROM sessions WHERE id = ?1", params![session_id])?;
        tx.commit()?;
        Ok(())
    })
}

/// Hapus SELURUH riwayat sesi (dipakai tombol "Delete all session data").
pub fn delete_all_sessions(db: &Db) -> Result<usize> {
    db.with(|conn| {
        let n = conn.execute("DELETE FROM sessions", [])?;
        conn.execute("DELETE FROM suggestions", [])?;
        conn.execute("DELETE FROM questions", [])?;
        conn.execute("DELETE FROM transcripts", [])?;
        Ok(n)
    })
}

/// Simpan satu segmen transkrip.
pub fn insert_transcript(
    db: &Db,
    session_id: i64,
    ts_ms: i64,
    source: &SegmentSource,
    text: &str,
) -> Result<i64> {
    db.with(|conn| {
        conn.execute(
            "INSERT INTO transcripts (session_id, ts_ms, source, text) VALUES (?1, ?2, ?3, ?4)",
            params![session_id, ts_ms, source.as_str(), text],
        )?;
        Ok(conn.last_insert_rowid())
    })
}

/// Ambil seluruh transkrip sebuah sesi, terurut waktu.
pub fn transcript_for(db: &Db, session_id: i64) -> Result<Vec<TranscriptSegment>> {
    db.with(|conn| {
        let mut stmt = conn.prepare(
            "SELECT ts_ms, source, text FROM transcripts WHERE session_id = ?1 ORDER BY ts_ms, id",
        )?;
        let rows = stmt.query_map(params![session_id], |row| {
            Ok(TranscriptSegment {
                ts_ms: row.get(0)?,
                source: SegmentSource::from_str_lossy(&row.get::<_, String>(1)?),
                text: row.get(2)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    })
}

/// Simpan pertanyaan yang terdeteksi.
pub fn insert_question(
    db: &Db,
    session_id: i64,
    ts_ms: i64,
    text: &str,
    category: Option<&str>,
    keywords: &[String],
) -> Result<i64> {
    let keywords_json = serde_json::to_string(keywords)?;
    db.with(|conn| {
        conn.execute(
            "INSERT INTO questions (session_id, ts_ms, text, category, keywords)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![session_id, ts_ms, text, category, keywords_json],
        )?;
        Ok(conn.last_insert_rowid())
    })
}

/// Simpan saran jawaban untuk sebuah pertanyaan.
pub fn insert_suggestion(db: &Db, question_id: i64, style: &str, content: &str) -> Result<i64> {
    db.with(|conn| {
        conn.execute(
            "INSERT INTO suggestions (question_id, style, content, created_at_ms)
             VALUES (?1, ?2, ?3, ?4)",
            params![question_id, style, content, now_ms()],
        )?;
        Ok(conn.last_insert_rowid())
    })
}

/// Daftar pertanyaan sebuah sesi.
pub fn questions_for(db: &Db, session_id: i64) -> Result<Vec<QuestionRow>> {
    db.with(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, session_id, ts_ms, text, category, keywords
             FROM questions WHERE session_id = ?1 ORDER BY ts_ms, id",
        )?;
        let rows = stmt.query_map(params![session_id], |row| {
            let kw: Option<String> = row.get(5)?;
            Ok(QuestionRow {
                id: row.get(0)?,
                session_id: row.get(1)?,
                ts_ms: row.get(2)?,
                text: row.get(3)?,
                category: row.get(4)?,
                keywords: kw
                    .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
                    .unwrap_or_default(),
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    })
}

/// Cari pertanyaan di semua sesi (untuk halaman History).
pub fn search_questions(db: &Db, query: &str, limit: usize) -> Result<Vec<QuestionRow>> {
    db.with(|conn| {
        let like = format!("%{}%", query.replace('%', "\\%"));
        let mut stmt = conn.prepare(
            "SELECT id, session_id, ts_ms, text, category, keywords
             FROM questions WHERE text LIKE ?1 ORDER BY ts_ms DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![like, limit as i64], |row| {
            let kw: Option<String> = row.get(5)?;
            Ok(QuestionRow {
                id: row.get(0)?,
                session_id: row.get(1)?,
                ts_ms: row.get(2)?,
                text: row.get(3)?,
                category: row.get(4)?,
                keywords: kw
                    .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
                    .unwrap_or_default(),
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    })
}

/// Saran terakhir untuk sebuah pertanyaan.
pub fn suggestions_for(db: &Db, question_id: i64) -> Result<Vec<SuggestionRow>> {
    db.with(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, question_id, style, content, created_at_ms
             FROM suggestions WHERE question_id = ?1 ORDER BY created_at_ms, id",
        )?;
        let rows = stmt.query_map(params![question_id], |row| {
            Ok(SuggestionRow {
                id: row.get(0)?,
                question_id: row.get(1)?,
                style: row.get(2)?,
                content: row.get(3)?,
                created_at_ms: row.get(4)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    })
}

/// Helper validasi sesi ada.
pub fn ensure_session(db: &Db, session_id: i64) -> Result<()> {
    match get_session(db, session_id)? {
        Some(_) => Ok(()),
        None => Err(AppError::Other(format!("sesi {session_id} tidak ditemukan"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seeded() -> (Db, i64) {
        let db = Db::open_in_memory().expect("db");
        let id = create_session(&db, &SessionKind::Practice, "Latihan 1").expect("session");
        (db, id)
    }

    #[test]
    fn session_lifecycle() {
        let (db, id) = seeded();
        insert_transcript(&db, id, 1000, &SegmentSource::Mic, "halo, tes mikrofon").unwrap();
        insert_transcript(&db, id, 2500, &SegmentSource::Meeting, "can you tell me about yourself?")
            .unwrap();
        end_session(&db, id, Some("ringkasan singkat")).unwrap();

        let info = get_session(&db, id).unwrap().unwrap();
        assert_eq!(info.kind, SessionKind::Practice);
        assert!(info.ended_at_ms.is_some());
        assert_eq!(info.summary.as_deref(), Some("ringkasan singkat"));

        let tr = transcript_for(&db, id).unwrap();
        assert_eq!(tr.len(), 2);
        assert_eq!(tr[1].source, SegmentSource::Meeting);
    }

    #[test]
    fn delete_cascades() {
        let (db, id) = seeded();
        let qid = insert_question(&db, id, 0, "Q?", Some("introduction"), &["a".into()]).unwrap();
        insert_suggestion(&db, qid, "professional", "jawaban").unwrap();
        insert_transcript(&db, id, 0, &SegmentSource::Mic, "x").unwrap();

        delete_session(&db, id).unwrap();
        assert!(get_session(&db, id).unwrap().is_none());
        db.with(|c| {
            let n: i64 = c.query_row("SELECT COUNT(*) FROM suggestions", [], |r| r.get(0))?;
            assert_eq!(n, 0);
            let n: i64 = c.query_row("SELECT COUNT(*) FROM questions", [], |r| r.get(0))?;
            assert_eq!(n, 0);
            let n: i64 = c.query_row("SELECT COUNT(*) FROM transcripts", [], |r| r.get(0))?;
            assert_eq!(n, 0);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn search_finds_question() {
        let (db, id) = seeded();
        insert_question(&db, id, 0, "Tell me about backend experience", Some("experience"), &[])
            .unwrap();
        insert_question(&db, id, 0, "Kenapa memilih perusahaan ini?", Some("motivation"), &[])
            .unwrap();
        let found = search_questions(&db, "backend", 10).unwrap();
        assert_eq!(found.len(), 1);
        assert!(found[0].text.contains("backend"));
    }
}
