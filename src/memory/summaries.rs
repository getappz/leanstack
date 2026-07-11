use rusqlite::{params, Connection};

#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionSummary {
    pub id: i64,
    pub project: String,
    pub session_id: Option<String>,
    pub seq: i64,
    pub summary: String,
    pub searchable_text: String,
    pub created_at: String,
}

pub fn append(
    conn: &Connection,
    project: &str,
    session_id: Option<&str>,
    summary: &str,
) -> rusqlite::Result<i64> {
    let now = now_iso();
    let next_seq: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(seq), 0) + 1 FROM session_summaries WHERE project = ?1",
            params![project],
            |r| r.get(0),
        )?;
    let searchable = format!("{}: {}\n{}", project, summary, now);
    conn.execute(
        "INSERT INTO session_summaries (project, session_id, seq, summary, searchable_text, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![project, session_id, next_seq, summary, searchable, now],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn list_recent(conn: &Connection, project: Option<&str>, limit: usize) -> rusqlite::Result<Vec<SessionSummary>> {
    let mut stmt = conn.prepare(
        "SELECT id, project, session_id, seq, summary, searchable_text, created_at
         FROM session_summaries
         WHERE (?1 IS NULL OR project = ?1)
         ORDER BY created_at DESC
         LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![project, limit as i64], |r| {
        Ok(SessionSummary {
            id: r.get(0)?,
            project: r.get(1)?,
            session_id: r.get(2)?,
            seq: r.get(3)?,
            summary: r.get(4)?,
            searchable_text: r.get(5)?,
            created_at: r.get(6)?,
        })
    })?;
    rows.collect()
}

fn now_iso() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}
