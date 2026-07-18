use rusqlite::{Connection, OpenFlags};
use std::path::Path;

/// The ONLY way the dashboard opens a database — read-only, so a write can't slip in.
pub fn open_readonly(path: &Path) -> rusqlite::Result<Connection> {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
}

/// Open the PM backend database (`~/.agentflare/backend.db`) read-only.
pub fn pm_db_readonly() -> rusqlite::Result<Connection> {
    let path = crate::paths::home().join(".agentflare").join("backend.db");
    open_readonly(&path)
}

/// Live claims as a JSON array string; reuses `crate::claims::list`. "[]" on error.
pub fn claims_json() -> String {
    let path = crate::db::agentflare_db_path();
    let result = open_readonly(&path).and_then(|conn| {
        crate::claims::list(&conn, None, true, crate::claims::now(), crate::claims::ttl_secs())
    });
    match result {
        Ok(claims) => serde_json::to_string(&claims).unwrap_or_else(|_| "[]".into()),
        Err(_) => "[]".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn open_readonly_rejects_writes() {
        let dir = std::env::temp_dir().join("agentflare-test-dash-ro");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.db");
        {
            let w = Connection::open(&path).unwrap();
            w.execute_batch("CREATE TABLE t (x INTEGER);").unwrap();
        }
        let ro = open_readonly(&path).unwrap();
        let err = ro.execute("INSERT INTO t (x) VALUES (1)", []).unwrap_err();
        assert!(format!("{err}").contains("read"), "must reject writes: {err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn pm_db_readonly_rejects_writes() {
        crate::paths::test_support::with_temp_home(|| {
            let agentflare_dir = crate::paths::home().join(".agentflare");
            std::fs::create_dir_all(&agentflare_dir).unwrap();
            let path = agentflare_dir.join("backend.db");
            {
                let w = Connection::open(&path).unwrap();
                w.execute_batch("CREATE TABLE t (x INTEGER);").unwrap();
            }
            let ro = pm_db_readonly().unwrap();
            let err = ro.execute("INSERT INTO t (x) VALUES (1)", []).unwrap_err();
            assert!(format!("{err}").contains("read"), "must reject writes: {err}");
        });
    }
}
