//! Akses database SQLite (schema + migrasi).
//!
//! Database disimpan di `~/.local/share/niri-ai-copilot/copilot.db`.
//! Audio tidak pernah disimpan di sini — hanya metadata, transkrip, dan
//! saran jawaban. API key tidak pernah masuk database.

use std::path::Path;
use std::sync::Mutex;

use rusqlite::Connection;

use crate::error::Result;

pub struct Db {
    conn: Mutex<Connection>,
}

impl Db {
    /// Buka (atau buat) database di path tertentu.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        Self::init(conn, true)
    }

    /// Database sementara di memori (untuk test).
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        Self::init(conn, false)
    }

    fn init(conn: Connection, on_disk: bool) -> Result<Self> {
        conn.pragma_update(None, "foreign_keys", "ON")?;
        if on_disk {
            conn.query_row("PRAGMA journal_mode=WAL", [], |_row| Ok(()))?;
        }
        Self::migrate(&conn)?;
        Ok(Db {
            conn: Mutex::new(conn),
        })
    }

    fn migrate(conn: &Connection) -> Result<()> {
        let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if version < 1 {
            conn.execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS sessions (
                    id            INTEGER PRIMARY KEY AUTOINCREMENT,
                    kind          TEXT NOT NULL,
                    title         TEXT NOT NULL,
                    started_at_ms INTEGER NOT NULL,
                    ended_at_ms   INTEGER,
                    summary       TEXT
                );

                CREATE TABLE IF NOT EXISTS transcripts (
                    id         INTEGER PRIMARY KEY AUTOINCREMENT,
                    session_id INTEGER NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
                    ts_ms      INTEGER NOT NULL,
                    source     TEXT NOT NULL,
                    text       TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_transcripts_session ON transcripts(session_id);

                CREATE TABLE IF NOT EXISTS questions (
                    id         INTEGER PRIMARY KEY AUTOINCREMENT,
                    session_id INTEGER NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
                    ts_ms      INTEGER NOT NULL,
                    text       TEXT NOT NULL,
                    category   TEXT,
                    keywords   TEXT
                );
                CREATE INDEX IF NOT EXISTS idx_questions_session ON questions(session_id);

                CREATE TABLE IF NOT EXISTS suggestions (
                    id          INTEGER PRIMARY KEY AUTOINCREMENT,
                    question_id INTEGER NOT NULL REFERENCES questions(id) ON DELETE CASCADE,
                    style       TEXT NOT NULL,
                    content     TEXT NOT NULL,
                    created_at_ms INTEGER NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_suggestions_question ON suggestions(question_id);

                CREATE TABLE IF NOT EXISTS profile (
                    id         INTEGER PRIMARY KEY CHECK (id = 1),
                    data       TEXT NOT NULL,
                    updated_at_ms INTEGER NOT NULL
                );

                CREATE TABLE IF NOT EXISTS kv (
                    key   TEXT PRIMARY KEY,
                    value TEXT NOT NULL
                );

                PRAGMA user_version = 1;
                "#,
            )?;
        }
        Ok(())
    }

    /// Jalankan closure dengan akses ke koneksi (di-lock).
    pub fn with<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| crate::error::AppError::Other("database lock poisoned".into()))?;
        f(&conn)
    }
}

/// Millis sejak Unix epoch (UTC).
pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Format millis epoch menjadi `YYYY-MM-DD HH:MM:SS` (UTC naif, cukup untuk UI).
pub fn format_ts(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let (y, mo, d) = civil_from_days(days);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{m:02}:{s:02}")
}

/// Format durasi millis menjadi `HH:MM:SS` untuk transkrip.
pub fn format_duration_ms(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let (h, m, s) = (secs / 3600, (secs % 3600) / 60, secs % 60);
    format!("{h:02}:{m:02}:{s:02}")
}

/// Algoritma konversi hari-sipil (Howard Hinnant), tanpa dependency chrono.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_epoch() {
        assert_eq!(format_ts(0), "1970-01-01 00:00:00");
        assert_eq!(format_ts(1_700_000_000_000), "2023-11-14 22:13:20");
    }

    #[test]
    fn formats_duration() {
        assert_eq!(format_duration_ms(0), "00:00:00");
        assert_eq!(format_duration_ms(3_723_000), "01:02:03");
    }

    #[test]
    fn opens_memory_db_and_migrates() {
        let db = Db::open_in_memory().expect("db");
        db.with(|c| {
            let v: i64 = c.query_row("PRAGMA user_version", [], |r| r.get(0))?;
            assert_eq!(v, 1);
            Ok(())
        })
        .expect("migrated");
    }
}
