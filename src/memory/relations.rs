use rusqlite::{params, Connection, OptionalExtension};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Relation {
    pub id: i64,
    pub source_id: i64,
    pub target_id: i64,
    pub relation: String,
    pub judgment_status: String,
    pub reason: Option<String>,
    pub evidence: Option<String>,
    pub confidence: Option<f64>,
    pub marked_by_actor: Option<String>,
    pub marked_by_kind: Option<String>,
    pub marked_by_model: Option<String>,
    pub session_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

pub fn create(
    conn: &Connection,
    source_id: i64,
    target_id: i64,
    relation: &str,
    session_id: Option<&str>,
    reason: Option<&str>,
    evidence: Option<&str>,
    confidence: Option<f64>,
) -> rusqlite::Result<i64> {
    let now = now_iso();
    conn.execute(
        "INSERT INTO memory_relations
            (source_id, target_id, relation, judgment_status, reason, evidence, confidence, session_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, 'judged', ?4, ?5, ?6, ?7, ?8, ?8)
         ON CONFLICT(source_id, target_id, relation) DO UPDATE SET
            judgment_status = 'judged',
            reason = COALESCE(?4, reason),
            evidence = COALESCE(?5, evidence),
            confidence = COALESCE(?6, confidence),
            updated_at = ?8",
        params![source_id, target_id, relation, reason, evidence, confidence, session_id, now],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn get(conn: &Connection, id: i64) -> rusqlite::Result<Option<Relation>> {
    conn.query_row(
        "SELECT id, source_id, target_id, relation, judgment_status, reason, evidence,
                confidence, marked_by_actor, marked_by_kind, marked_by_model,
                session_id, created_at, updated_at
         FROM memory_relations WHERE id = ?1",
        params![id],
        map_relation,
    ).optional()
}

pub fn list_for_observation(conn: &Connection, obs_id: i64) -> rusqlite::Result<Vec<Relation>> {
    let mut stmt = conn.prepare(
        "SELECT id, source_id, target_id, relation, judgment_status, reason, evidence,
                confidence, marked_by_actor, marked_by_kind, marked_by_model,
                session_id, created_at, updated_at
         FROM memory_relations
         WHERE source_id = ?1 OR target_id = ?1
         ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map(params![obs_id], map_relation)?;
    rows.collect()
}

fn map_relation(r: &rusqlite::Row<'_>) -> rusqlite::Result<Relation> {
    Ok(Relation {
        id: r.get(0)?,
        source_id: r.get(1)?,
        target_id: r.get(2)?,
        relation: r.get(3)?,
        judgment_status: r.get(4)?,
        reason: r.get(5)?,
        evidence: r.get(6)?,
        confidence: r.get(7)?,
        marked_by_actor: r.get(8)?,
        marked_by_kind: r.get(9)?,
        marked_by_model: r.get(10)?,
        session_id: r.get(11)?,
        created_at: r.get(12)?,
        updated_at: r.get(13)?,
    })
}

fn now_iso() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}
