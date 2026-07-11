use rusqlite::{params, Connection};

use super::observations::Observation;

pub fn search(
    conn: &Connection,
    query: &str,
    project: Option<&str>,
    limit: usize,
) -> rusqlite::Result<Vec<Observation>> {
    if query.trim().is_empty() {
        return search_fallback(conn, project, limit);
    }
    let limit = limit.min(50) as i64;
    let like_pat = format!("%{}%", query.replace('%', "\\%").replace('_', "\\_"));
    let project_filter = project_filter_clause(project.is_some());

    let sql = format!(
        "SELECT o.id, o.session_id, o.type, o.title, o.content, o.tool_name,
                o.project, o.scope, o.topic_key, o.normalized_hash,
                o.revision_count, o.duplicate_count, o.last_seen_at,
                o.review_after, o.pinned, o.created_at, o.updated_at, o.deleted_at
         FROM observations_fts f
         JOIN observations o ON o.id = f.rowid
         WHERE observations_fts MATCH ?1
           AND o.deleted_at IS NULL
           {project_filter}
         ORDER BY rank
         LIMIT ?2",
    );

    let mut stmt = conn.prepare(&sql)?;
    let fts_query = build_fts_query(query);
    let rows = if let Some(p) = project {
        stmt.query_map(params![fts_query, p, limit], map_search_row)?
    } else {
        stmt.query_map(params![fts_query, limit], map_search_row)?
    };
    let results: Vec<Observation> = rows.collect::<Result<_, _>>()?;
    if !results.is_empty() {
        return Ok(results);
    }

    let like_sql = format!(
        "SELECT o.id, o.session_id, o.type, o.title, o.content, o.tool_name,
                o.project, o.scope, o.topic_key, o.normalized_hash,
                o.revision_count, o.duplicate_count, o.last_seen_at,
                o.review_after, o.pinned, o.created_at, o.updated_at, o.deleted_at
         FROM observations o
         WHERE o.deleted_at IS NULL
           AND (o.title LIKE ?1 ESCAPE '\\' OR o.content LIKE ?1 ESCAPE '\\')
           {project_filter}
         ORDER BY o.created_at DESC
         LIMIT ?2",
    );
    let mut like_stmt = conn.prepare(&like_sql)?;
    let like_rows = if let Some(p) = project {
        like_stmt.query_map(params![like_pat, p, limit], map_search_row)?
    } else {
        like_stmt.query_map(params![like_pat, limit], map_search_row)?
    };
    like_rows.collect()
}

fn search_fallback(conn: &Connection, project: Option<&str>, limit: usize) -> rusqlite::Result<Vec<Observation>> {
    super::observations::list_recent(conn, project, limit)
}

fn build_fts_query(raw: &str) -> String {
    let tokens: Vec<String> = raw
        .split_whitespace()
        .filter(|t| !t.is_empty())
        .map(|t| {
            if t.chars().any(|c| c == '"' || c == '*' || c == '(' || c == ')' || c == '+' || c == '-' || c == '~') {
                let cleaned: String = t.chars().filter(|c| !matches!(c, '"' | '*' | '(' | ')' | '+' | '-' | '~' | '^')).collect();
                if cleaned.is_empty() {
                    String::new()
                } else {
                    format!("\"{}\"", cleaned)
                }
            } else {
                format!("\"{}\"", t)
            }
        })
        .filter(|t| !t.is_empty())
        .collect();
    if tokens.is_empty() {
        return raw.to_string();
    }
    tokens.join(" ")
}

fn project_filter_clause(has_project: bool) -> &'static str {
    if has_project {
        "AND o.project = ?2"
    } else {
        ""
    }
}

fn map_search_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Observation> {
    Ok(Observation {
        id: r.get(0)?,
        session_id: r.get(1)?,
        r#type: r.get(2)?,
        title: r.get(3)?,
        content: r.get(4)?,
        tool_name: r.get(5)?,
        project: r.get(6)?,
        scope: r.get(7)?,
        topic_key: r.get(8)?,
        normalized_hash: r.get(9)?,
        revision_count: r.get(10)?,
        duplicate_count: r.get(11)?,
        last_seen_at: r.get(12)?,
        review_after: r.get(13)?,
        pinned: r.get(14)?,
        created_at: r.get(15)?,
        updated_at: r.get(16)?,
        deleted_at: r.get(17)?,
    })
}
