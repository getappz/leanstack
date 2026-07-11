use serde::Deserialize;
use serde_json::json;

use super::store;
use super::{observations, relations, search, sessions, summaries};

fn open_db() -> Result<rusqlite::Connection, String> {
    store::open().map_err(|e| format!("cannot open brain.db: {e}"))
}

#[derive(Debug, Deserialize)]
pub struct RememberInput {
    pub title: String,
    pub content: String,
    pub r#type: String,
    pub session_id: Option<String>,
    pub project: Option<String>,
    pub topic_key: Option<String>,
    pub scope: Option<String>,
}

pub fn handle_remember(input: RememberInput) -> Result<String, String> {
    if input.title.trim().is_empty() || input.content.trim().is_empty() {
        return Err("title and content are required".into());
    }
    let conn = open_db()?;
    let outcome = observations::save(
        &conn,
        input.session_id.as_deref(),
        &input.r#type,
        &input.title,
        &input.content,
        None,
        input.project.as_deref(),
        input.scope.as_deref(),
        input.topic_key.as_deref(),
    )
    .map_err(|e| format!("save failed: {e}"))?;
    let (status, id) = match outcome {
        observations::SaveOutcome::Created(id) => ("created", id),
        observations::SaveOutcome::Updated(id) => ("updated", id),
        observations::SaveOutcome::Duplicate(id) => ("duplicate", id),
    };
    Ok(json!({"status": status, "id": id}).to_string())
}

#[derive(Debug, Deserialize)]
pub struct RecallInput {
    pub query: Option<String>,
    pub id: Option<i64>,
    pub r#type: Option<String>,
    pub project: Option<String>,
    pub limit: Option<usize>,
}

pub fn handle_recall(input: RecallInput) -> Result<String, String> {
    let conn = open_db()?;
    if let Some(id) = input.id {
        let obs = observations::get(&conn, id).map_err(|e| format!("lookup failed: {e}"))?;
        return Ok(json!(obs).to_string());
    }
    let limit = input.limit.unwrap_or(10).min(50);
    let results = if let Some(ref q) = input.query.filter(|q| !q.trim().is_empty()) {
        search::search(&conn, q, input.project.as_deref(), limit)
            .map_err(|e| format!("search failed: {e}"))?
    } else {
        observations::list_recent(&conn, input.project.as_deref(), limit)
            .map_err(|e| format!("list failed: {e}"))?
    };
    let filtered = if let Some(ref t) = input.r#type {
        results.into_iter().filter(|o| o.r#type == *t).collect()
    } else {
        results
    };
    Ok(json!(filtered).to_string())
}

#[derive(Debug, Deserialize)]
pub struct ContextInput {
    pub session_id: Option<String>,
    pub project: Option<String>,
}

pub fn handle_context(input: ContextInput) -> Result<String, String> {
    let conn = open_db()?;
    let session = match input.session_id.as_deref() {
        Some(id) if !id.is_empty() => sessions::get(&conn, id).map_err(|e| format!("session lookup: {e}"))?,
        _ => None,
    };
    let recent_sessions = sessions::list_recent(&conn, input.project.as_deref(), 5)
        .map_err(|e| format!("recent sessions: {e}"))?;
    let recent_obs = observations::list_recent(&conn, input.project.as_deref(), 10)
        .map_err(|e| format!("recent observations: {e}"))?;
    let recent_summaries = summaries::list_recent(&conn, input.project.as_deref(), 5)
        .map_err(|e| format!("recent summaries: {e}"))?;
    Ok(json!({
        "session": session,
        "recent_sessions": recent_sessions,
        "recent_observations": recent_obs,
        "recent_summaries": recent_summaries,
    })
    .to_string())
}

#[derive(Debug, Deserialize)]
pub struct HandoffInput {
    pub session_id: String,
    pub summary: String,
    pub findings: Option<Vec<serde_json::Value>>,
    pub decisions: Option<Vec<serde_json::Value>>,
    pub files_touched: Option<Vec<serde_json::Value>>,
    pub evidence: Option<Vec<serde_json::Value>>,
}

pub fn handle_handoff(input: HandoffInput) -> Result<String, String> {
    if input.session_id.trim().is_empty() || input.summary.trim().is_empty() {
        return Err("session_id and summary are required".into());
    }
    let conn = open_db()?;
    let session = sessions::get(&conn, &input.session_id)
        .map_err(|e| format!("session lookup: {e}"))?
        .ok_or_else(|| format!("session not found: {}", input.session_id))?;

    let project = session.project.clone().unwrap_or_default();
    let findings = input.findings.as_ref().map(|v| serde_json::to_string(&v).unwrap_or_default());
    let decisions = input.decisions.as_ref().map(|v| serde_json::to_string(&v).unwrap_or_default());
    let files = input.files_touched.as_ref().map(|v| serde_json::to_string(&v).unwrap_or_default());
    let evidence = input.evidence.as_ref().map(|v| serde_json::to_string(&v).unwrap_or_default());

    sessions::update_enriched(
        &conn,
        &input.session_id,
        None,
        findings.as_deref(),
        decisions.as_deref(),
        files.as_deref(),
        evidence.as_deref(),
        None,
        None,
    )
    .map_err(|e| format!("update enriched: {e}"))?;

    let snapshot = json!({
        "session_id": input.session_id,
        "project": session.project,
        "summary": input.summary,
        "findings_count": input.findings.as_ref().map(|v| v.len()).unwrap_or(0),
        "decisions_count": input.decisions.as_ref().map(|v| v.len()).unwrap_or(0),
        "files_touched_count": input.files_touched.as_ref().map(|v| v.len()).unwrap_or(0),
    })
    .to_string();
    sessions::update_enriched(&conn, &input.session_id, None, None, None, None, None, None, Some(&snapshot))
        .map_err(|e| format!("update snapshot: {e}"))?;

    sessions::close(&conn, &input.session_id, &input.summary)
        .map_err(|e| format!("close session: {e}"))?;

    summaries::append(&conn, &project, Some(&input.session_id), &input.summary)
        .map_err(|e| format!("append summary: {e}"))?;

    Ok(json!({
        "status": "closed",
        "session_id": input.session_id,
        "project": project,
        "snapshot": snapshot,
    })
    .to_string())
}

#[derive(Debug, Deserialize)]
pub struct RelateInput {
    pub source_id: i64,
    pub target_id: i64,
    pub relation: String,
    pub reason: Option<String>,
    pub confidence: Option<f64>,
}

pub fn handle_relate(input: RelateInput) -> Result<String, String> {
    if input.source_id == input.target_id {
        return Err("source_id and target_id must differ".into());
    }
    let conn = open_db()?;
    let id = relations::create(
        &conn,
        input.source_id,
        input.target_id,
        &input.relation,
        None,
        input.reason.as_deref(),
        None,
        input.confidence,
    )
    .map_err(|e| format!("create relation: {e}"))?;
    Ok(json!({"status": "created", "id": id}).to_string())
}

#[derive(Debug, Deserialize)]
pub struct CurateInput {
    pub action: String,
    pub id: i64,
    pub title: Option<String>,
    pub content: Option<String>,
    pub r#type: Option<String>,
    pub pinned: Option<bool>,
}

pub fn handle_curate(input: CurateInput) -> Result<String, String> {
    let conn = open_db()?;
    match input.action.as_str() {
        "update" => {
            let ok = observations::update(&conn, input.id, input.title.as_deref(), input.content.as_deref(), input.r#type.as_deref(), input.pinned)
                .map_err(|e| format!("update: {e}"))?;
            Ok(json!({"status": if ok { "updated" } else { "not_found" }}).to_string())
        }
        "delete" => {
            let ok = observations::soft_delete(&conn, input.id)
                .map_err(|e| format!("delete: {e}"))?;
            Ok(json!({"status": if ok { "deleted" } else { "not_found" }}).to_string())
        }
        "pin" => {
            let ok = observations::pin(&conn, input.id, true)
                .map_err(|e| format!("pin: {e}"))?;
            Ok(json!({"status": if ok { "pinned" } else { "not_found" }}).to_string())
        }
        "unpin" => {
            let ok = observations::pin(&conn, input.id, false)
                .map_err(|e| format!("unpin: {e}"))?;
            Ok(json!({"status": if ok { "unpinned" } else { "not_found" }}).to_string())
        }
        other => Err(format!("unknown action '{other}'; use update, delete, pin, or unpin")),
    }
}
