//! The single source-of-truth SQLite database, `~/.agentflare/agentflare.db`.
//! New relational state adds a table + migration here rather than a new file
//! (see #138 — gateway secrets fold in later). The rebuildable caches under
//! `~/.local/share/agentflare/` (skills index, gateway tool-index) stay
//! separate: they belong in the data dir, not next to source-of-truth state.
use rusqlite::Connection;
use std::path::PathBuf;

pub fn agentflare_db_path() -> PathBuf {
    crate::paths::home().join(".agentflare").join("agentflare.db")
}

/// Opens (creating if absent) `agentflare.db` and applies every table's
/// migration. Each subsystem owns its own `CREATE TABLE IF NOT EXISTS` so
/// this stays a thin dispatcher.
pub fn open() -> rusqlite::Result<Connection> {
    let path = agentflare_db_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let conn = Connection::open(path)?;
    crate::claims::migrate(&conn)?;
    Ok(conn)
}
