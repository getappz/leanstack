use rusqlite::Connection;
use std::path::{Path, PathBuf};

const SCHEMA_V1: &str = "
CREATE TABLE session_files (
    file_path   TEXT PRIMARY KEY,
    mtime_secs  INTEGER NOT NULL,
    size_bytes  INTEGER NOT NULL,
    indexed_at  TEXT NOT NULL
);

CREATE TABLE file_rollup (
    file_path                 TEXT NOT NULL,
    date                      TEXT NOT NULL,
    project                   TEXT NOT NULL,
    model                     TEXT NOT NULL,
    input_tokens              INTEGER NOT NULL,
    output_tokens             INTEGER NOT NULL,
    cache_read_tokens         INTEGER NOT NULL,
    cache_creation_tokens     INTEGER NOT NULL,
    cache_creation_5m_tokens  INTEGER NOT NULL,
    cache_creation_1hr_tokens INTEGER NOT NULL,
    cost_usd                  REAL NOT NULL,
    has_unpriced_usage        INTEGER NOT NULL,
    PRIMARY KEY (file_path, date, model)
);
CREATE INDEX file_rollup_date ON file_rollup(date);
";

fn db_path() -> PathBuf {
    crate::state::state_dir().join("analytics.db")
}

fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    let version: i32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version < 1 {
        conn.execute_batch(SCHEMA_V1)?;
        conn.pragma_update(None, "user_version", 1)?;
    }
    Ok(())
}

fn try_open(path: &Path) -> Option<Connection> {
    let conn = Connection::open(path).ok()?;
    migrate(&conn).ok()?;
    Some(conn)
}

/// Opens the analytics cache database, creating and migrating it if needed.
/// The database is a pure cache over the JSONL session transcripts, so any
/// corruption is recovered by deleting and recreating it — never by partial
/// repair. If the filesystem is entirely unusable, falls back to an
/// in-memory database so `agentflare cost` still works for this run (without
/// persisting the cache).
pub(crate) fn open_or_rebuild() -> Connection {
    let path = db_path();
    let _ = std::fs::create_dir_all(crate::state::state_dir());

    if let Some(conn) = try_open(&path) {
        return conn;
    }
    let _ = std::fs::remove_file(&path);
    if let Some(conn) = try_open(&path) {
        return conn;
    }
    Connection::open_in_memory().expect("sqlite failed to open even an in-memory database")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::test_support::with_temp_home;

    #[test]
    fn open_or_rebuild_creates_expected_schema() {
        with_temp_home(|| {
            let conn = open_or_rebuild();
            let mut stmt = conn
                .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
                .unwrap();
            let names: Vec<String> = stmt
                .query_map([], |row| row.get(0))
                .unwrap()
                .map(|r| r.unwrap())
                .collect();
            assert_eq!(names, vec!["file_rollup", "session_files"]);

            let version: i32 = conn
                .pragma_query_value(None, "user_version", |row| row.get(0))
                .unwrap();
            assert_eq!(version, 1);
        });
    }

    #[test]
    fn open_or_rebuild_is_idempotent() {
        with_temp_home(|| {
            let _ = open_or_rebuild();
            let conn = open_or_rebuild();
            let version: i32 = conn
                .pragma_query_value(None, "user_version", |row| row.get(0))
                .unwrap();
            assert_eq!(version, 1);
        });
    }

    #[test]
    fn open_or_rebuild_recovers_from_corrupt_db_file() {
        with_temp_home(|| {
            let path = db_path();
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, b"not a sqlite database").unwrap();

            let conn = open_or_rebuild();
            let version: i32 = conn
                .pragma_query_value(None, "user_version", |row| row.get(0))
                .unwrap();
            assert_eq!(version, 1);
        });
    }
}
