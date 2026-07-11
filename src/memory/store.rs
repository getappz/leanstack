use rusqlite::Connection;
use std::path::PathBuf;
use std::time::Duration;

pub fn brain_db_path() -> PathBuf {
    crate::paths::home().join(".agentflare").join("memory").join("brain.db")
}

pub fn open() -> rusqlite::Result<Connection> {
    let path = brain_db_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700));
        }
    }
    let conn = Connection::open(&path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    tune(&conn)?;
    super::schema::migrate(&conn)?;
    Ok(conn)
}

fn tune(conn: &Connection) -> rusqlite::Result<()> {
    conn.busy_timeout(Duration::from_secs(5))?;
    let _: String = conn.query_row("PRAGMA journal_mode=WAL", [], |r| r.get(0))?;
    conn.execute_batch("PRAGMA synchronous=NORMAL;")?;
    Ok(())
}
