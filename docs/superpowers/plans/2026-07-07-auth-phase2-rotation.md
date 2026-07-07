# Auth Vault Phase 2 — Rotation, Cooldown & Health Scoring Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add smart multi-profile rotation, cooldown tracking, health scoring, aliases, and project associations to the auth profile vault.

**Architecture:** New `src/auth_db.rs` handles SQLite persistence (reusing rusqlite, matching rollup.rs `open_or_rebuild()` + `migrate()` pattern). `src/auth.rs` extended with rotation algorithms, health scoring, cooldown management. `src/main.rs` refactors `AgentsAction::Auth` to top-level `Auth` command.

**Tech Stack:** Rust, rusqlite (already in Cargo.toml), serde_json, sha2 (already in deps)

## Global Constraints

- No new dependencies — reuse rusqlite (already from PR #26)
- Generate `Cargo.lock` on first build
- 10-12 new tests, all hermetic via `with_temp_home`
- JSON output on all commands via `--json` flag
- Reuse `crate::paths::home()` for path resolution
- Reuse `crate::paths::test_support::with_temp_home` for test isolation

---

### Task 1: Create auth_db.rs — SQLite schema and migrations

**Files:**
- Create: `src/auth_db.rs`

**Interfaces:**
- Consumes: `rusqlite::Connection`, `crate::paths::home`
- Produces: `pub fn open_or_rebuild() -> Connection`, `pub fn migrate(conn: &Connection)`, `pub fn record_error(conn: &Connection, agent: &str, profile: &str, error_msg: &str)`, `pub fn set_cooldown(conn: &Connection, agent: &str, profile: &str, minutes: u32, reason: &str)`, `pub fn list_cooldowns(conn: &Connection, agent: Option<&str>) -> Vec<CooldownRow>`, `pub fn clear_cooldown(conn: &Connection, agent: &str, profile: &str)`, `pub fn get_health(conn: &Connection, agent: &str, profile: &str) -> ProfileHealth`, `pub fn list_health(conn: &Connection, agent: &str) -> Vec<ProfileHealth>`, `pub fn set_alias(conn: &Connection, agent: &str, alias: &str, profile: &str)`, `pub fn resolve_alias(conn: &Connection, agent: &str, name: &str) -> Option<String>`, `pub fn set_project(conn: &Connection, path: &str, agent: &str, profile: &str)`, `pub fn get_project(conn: &Connection, path: &str, agent: &str) -> Option<String>`, `pub fn unset_project(conn: &Connection, path: &str, agent: &str)`, `pub fn set_rotation_last(conn: &Connection, agent: &str, profile: &str, algorithm: &str)`, `pub fn get_rotation_last(conn: &Connection, agent: &str) -> Option<(String, String)>`
- Produces structs: `pub struct CooldownRow { pub agent: String, pub profile: String, pub until: String, pub reason: Option<String> }`, `pub struct ProfileHealth { pub agent: String, pub profile: String, pub status: String, pub error_count_1h: i32, pub penalty: f64, pub last_used_at: Option<String> }`

- [ ] **Step 1: Write failing test for open_or_rebuild**

```rust
// src/auth_db.rs
#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::test_support::with_temp_home;

    #[test]
    fn open_or_rebuild_creates_tables() {
        with_temp_home(|| {
            let conn = open_or_rebuild();
            let mut stmt = conn.prepare(
                "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name"
            ).unwrap();
            let tables: Vec<String> = stmt.query_map([], |row| row.get(0))
                .unwrap()
                .filter_map(|r| r.ok())
                .collect();
            assert!(tables.contains(&"profile_health".to_string()));
            assert!(tables.contains(&"cooldowns".to_string()));
            assert!(tables.contains(&"aliases".to_string()));
            assert!(tables.contains(&"projects".to_string()));
        });
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cd C:\Users\shiva\workspace\leanstack && cargo test auth_db::tests::open_or_rebuild_creates_tables
```

Expected: FAIL — module not found / function not defined

- [ ] **Step 3: Write auth_db.rs with schema, migrations, open_or_rebuild**

```rust
use crate::paths::home;
use rusqlite::{params, Connection, Result as SqlResult};
use std::path::PathBuf;

const SCHEMA_VERSION: i32 = 1;

pub struct CooldownRow {
    pub agent: String,
    pub profile: String,
    pub until: String,
    pub reason: Option<String>,
}

pub struct ProfileHealth {
    pub agent: String,
    pub profile: String,
    pub status: String,
    pub error_count_1h: i32,
    pub penalty: f64,
    pub last_used_at: Option<String>,
}

fn db_path() -> PathBuf {
    home().join(".local").join("share").join("agentflare").join("auth.db")
}

pub fn migrate(conn: &Connection) -> SqlResult<()> {
    let version: i32 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap_or(0);
    if version >= SCHEMA_VERSION {
        return Ok(());
    }
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS profile_health (
            agent       TEXT NOT NULL,
            profile     TEXT NOT NULL,
            status      TEXT NOT NULL DEFAULT 'healthy',
            error_count_1h INTEGER NOT NULL DEFAULT 0,
            penalty     REAL NOT NULL DEFAULT 0.0,
            last_error_time TEXT,
            last_used_at TEXT,
            updated_at  TEXT NOT NULL,
            PRIMARY KEY (agent, profile)
        );
        CREATE TABLE IF NOT EXISTS cooldowns (
            agent   TEXT NOT NULL,
            profile TEXT NOT NULL,
            until   TEXT NOT NULL,
            reason  TEXT,
            PRIMARY KEY (agent, profile)
        );
        CREATE TABLE IF NOT EXISTS aliases (
            agent   TEXT NOT NULL,
            alias   TEXT NOT NULL,
            profile TEXT NOT NULL,
            PRIMARY KEY (agent, alias)
        );
        CREATE TABLE IF NOT EXISTS projects (
            path    TEXT NOT NULL,
            agent   TEXT NOT NULL,
            profile TEXT NOT NULL,
            PRIMARY KEY (path, agent)
        );",
    )?;
    conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    Ok(())
}

pub fn open_or_rebuild() -> Connection {
    let path = db_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let conn = Connection::open(&path).expect("open auth.db");
    migrate(&conn).expect("migrate auth.db");
    conn
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo test auth_db::tests::open_or_rebuild_creates_tables
```

Expected: PASS

- [ ] **Step 5: Write tests + impl for record_error, cooldown CRUD, health CRUD, aliases, projects**

Write tests then impl in order. Each function = one test + impl cycle.

```rust
// Test: record_error increments error count
#[test]
fn record_error_increments_count() {
    with_temp_home(|| {
        let conn = open_or_rebuild();
        record_error(&conn, "claude-code", "alice", "rate limit exceeded");
        let h = get_health(&conn, "claude-code", "alice");
        assert_eq!(h.error_count_1h, 1);
        assert!(h.penalty > 0.0);
    });
}

// Test: record_error resets count after 1 hour gap  
#[test]
fn record_error_resets_after_hour() {
    with_temp_home(|| {
        let conn = open_or_rebuild();
        record_error(&conn, "claude-code", "alice", "timeout");
        // Simulate old timestamp
        conn.execute(
            "UPDATE profile_health SET last_error_time = datetime('now', '-2 hours') WHERE agent = ?1 AND profile = ?2",
            params!["claude-code", "alice"],
        ).unwrap();
        record_error(&conn, "claude-code", "alice", "timeout");
        let h = get_health(&conn, "claude-code", "alice");
        assert_eq!(h.error_count_1h, 1); // reset to 1
    });
}

// Test: set_cooldown creates entry
#[test]
fn set_cooldown_and_list() {
    with_temp_home(|| {
        let conn = open_or_rebuild();
        set_cooldown(&conn, "claude-code", "alice", 30, "manual");
        let list = list_cooldowns(&conn, Some("claude-code"));
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].profile, "alice");
    });
}

// Test: clear_cooldown removes entry
#[test]
fn clear_cooldown_removes() {
    with_temp_home(|| {
        let conn = open_or_rebuild();
        set_cooldown(&conn, "claude-code", "alice", 30, "test");
        clear_cooldown(&conn, "claude-code", "alice");
        let list = list_cooldowns(&conn, Some("claude-code"));
        assert!(list.is_empty());
    });
}

// Test: set_alias and resolve_alias
#[test]
fn alias_set_and_resolve() {
    with_temp_home(|| {
        let conn = open_or_rebuild();
        set_alias(&conn, "claude-code", "work", "work@company.com");
        assert_eq!(
            resolve_alias(&conn, "claude-code", "work"),
            Some("work@company.com".to_string())
        );
        assert_eq!(resolve_alias(&conn, "claude-code", "unknown"), None);
    });
}

// Test: project set/get/unset with cascading
#[test]
fn project_association_cascading() {
    with_temp_home(|| {
        let conn = open_or_rebuild();
        set_project(&conn, "/home/user/projects", "claude-code", "work");
        assert_eq!(
            get_project(&conn, "/home/user/projects/sub", "claude-code"),
            Some("work".to_string())
        );
        unset_project(&conn, "/home/user/projects", "claude-code");
        assert_eq!(
            get_project(&conn, "/home/user/projects/sub", "claude-code"),
            None
        );
    });
}

// Test: rotation state set/get
#[test]
fn rotation_last_tracks() {
    with_temp_home(|| {
        let conn = open_or_rebuild();
        set_rotation_last(&conn, "claude-code", "alice", "smart");
        let last = get_rotation_last(&conn, "claude-code");
        assert_eq!(last, Some(("alice".to_string(), "smart".to_string())));
    });
}
```

Impls:

```rust
fn now_iso() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

pub fn record_error(conn: &Connection, agent: &str, profile: &str, error_msg: &str) {
    let penalty = penalty_for_error(error_msg);
    let now = now_iso();
    let existing = conn.query_row(
        "SELECT error_count_1h, last_error_time FROM profile_health WHERE agent = ?1 AND profile = ?2",
        params![agent, profile],
        |row| Ok((row.get::<_, i32>(0)?, row.get::<_, Option<String>>(1)?)),
    );
    let new_count = match existing {
        Ok((count, Some(last_time))) => {
            // Reset if last error was > 1h ago
            if is_older_than_1h(&last_time, &now) { 1 } else { count + 1 }
        }
        _ => 1,
    };
    let decayed = decay_penalty(conn, agent, profile);
    let new_penalty = decayed + penalty;
    let status = if new_count >= 5 { "critical" } else if new_count >= 1 { "warning" } else { "healthy" };
    conn.execute(
        "INSERT INTO profile_health (agent, profile, status, error_count_1h, penalty, last_error_time, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(agent, profile) DO UPDATE SET
         status = excluded.status, error_count_1h = excluded.error_count_1h,
         penalty = excluded.penalty, last_error_time = excluded.last_error_time,
         updated_at = excluded.updated_at",
        params![agent, profile, status, new_count, new_penalty, now, now],
    ).ok();
}

fn is_older_than_1h(timestamp: &str, now: &str) -> bool {
    // Simple: parse ISO timestamps, compare. If parse fails, assume old.
    if let (Ok(ts), Ok(n)) = (
        chrono::NaiveDateTime::parse_from_str(timestamp, "%Y-%m-%dT%H:%M:%S"),
        chrono::NaiveDateTime::parse_from_str(now, "%Y-%m-%dT%H:%M:%S"),
    ) {
        (n - ts).num_hours() >= 1
    } else {
        true
    }
}

fn penalty_for_error(msg: &str) -> f64 {
    let m = msg.to_lowercase();
    if m.contains("429") || m.contains("rate limit") || m.contains("too many requests") { 10.0 }
    else if m.contains("401") || m.contains("403") || m.contains("unauthorized") { 100.0 }
    else if m.contains("timeout") || m.contains("deadline exceeded") { 5.0 }
    else if m.contains("500") || m.contains("502") || m.contains("503") || m.contains("504") { 5.0 }
    else { 3.0 }
}

fn decay_penalty(conn: &Connection, agent: &str, profile: &str) -> f64 {
    let row = conn.query_row(
        "SELECT penalty, last_error_time FROM profile_health WHERE agent = ?1 AND profile = ?2",
        params![agent, profile],
        |row| Ok((row.get::<_, f64>(0)?, row.get::<_, Option<String>>(1)?)),
    );
    match row {
        Ok((penalty, Some(last_time))) => {
            let now = now_iso();
            if let (Ok(ts), Ok(n)) = (
                chrono::NaiveDateTime::parse_from_str(&last_time, "%Y-%m-%dT%H:%M:%S"),
                chrono::NaiveDateTime::parse_from_str(&now, "%Y-%m-%dT%H:%M:%S"),
            ) {
                let minutes = (n - ts).num_minutes().max(0) as f64;
                let decay_intervals = (minutes / 5.0).floor();
                penalty * 0.8_f64.powf(decay_intervals)
            } else {
                penalty
            }
        }
        _ => 0.0,
    }
}

pub fn set_cooldown(conn: &Connection, agent: &str, profile: &str, minutes: u32, reason: &str) {
    let until = chrono::Utc::now() + chrono::Duration::minutes(minutes as i64);
    conn.execute(
        "INSERT INTO cooldowns (agent, profile, until, reason) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(agent, profile) DO UPDATE SET until = excluded.until, reason = excluded.reason",
        params![agent, profile, until.format("%Y-%m-%dT%H:%M:%S").to_string(), reason],
    ).ok();
}

pub fn list_cooldowns(conn: &Connection, agent: Option<&str>) -> Vec<CooldownRow> {
    let mut stmt = if let Some(a) = agent {
        conn.prepare("SELECT agent, profile, until, reason FROM cooldowns WHERE agent = ?1 AND until > datetime('now')").unwrap()
    } else {
        conn.prepare("SELECT agent, profile, until, reason FROM cooldowns WHERE until > datetime('now')").unwrap()
    };
    let rows = if agent.is_some() {
        stmt.query_map(params![agent.unwrap()], |row| {
            Ok(CooldownRow {
                agent: row.get(0)?, profile: row.get(1)?, until: row.get(2)?, reason: row.get(3)?,
            })
        }).unwrap()
    } else {
        stmt.query_map([], |row| {
            Ok(CooldownRow {
                agent: row.get(0)?, profile: row.get(1)?, until: row.get(2)?, reason: row.get(3)?,
            })
        }).unwrap()
    };
    rows.filter_map(|r| r.ok()).collect()
}

pub fn clear_cooldown(conn: &Connection, agent: &str, profile: &str) {
    conn.execute("DELETE FROM cooldowns WHERE agent = ?1 AND profile = ?2", params![agent, profile]).ok();
}

pub fn get_health(conn: &Connection, agent: &str, profile: &str) -> ProfileHealth {
    conn.query_row(
        "SELECT agent, profile, status, error_count_1h, penalty, last_used_at FROM profile_health WHERE agent = ?1 AND profile = ?2",
        params![agent, profile],
        |row| Ok(ProfileHealth {
            agent: row.get(0)?, profile: row.get(1)?, status: row.get(2)?,
            error_count_1h: row.get(3)?, penalty: row.get(4)?, last_used_at: row.get(5)?,
        }),
    ).unwrap_or_else(|_| ProfileHealth {
        agent: agent.to_string(), profile: profile.to_string(),
        status: "healthy".to_string(), error_count_1h: 0, penalty: 0.0, last_used_at: None,
    })
}

pub fn list_health(conn: &Connection, agent: &str) -> Vec<ProfileHealth> {
    let mut stmt = conn.prepare(
        "SELECT agent, profile, status, error_count_1h, penalty, last_used_at FROM profile_health WHERE agent = ?1"
    ).unwrap();
    stmt.query_map(params![agent], |row| {
        Ok(ProfileHealth {
            agent: row.get(0)?, profile: row.get(1)?, status: row.get(2)?,
            error_count_1h: row.get(3)?, penalty: row.get(4)?, last_used_at: row.get(5)?,
        })
    }).unwrap().filter_map(|r| r.ok()).collect()
}

pub fn set_alias(conn: &Connection, agent: &str, alias: &str, profile: &str) {
    conn.execute(
        "INSERT INTO aliases (agent, alias, profile) VALUES (?1, ?2, ?3) ON CONFLICT(agent, alias) DO UPDATE SET profile = excluded.profile",
        params![agent, alias, profile],
    ).ok();
}

pub fn resolve_alias(conn: &Connection, agent: &str, name: &str) -> Option<String> {
    conn.query_row(
        "SELECT profile FROM aliases WHERE agent = ?1 AND alias = ?2",
        params![agent, name],
        |row| row.get(0),
    ).ok()
}

pub fn set_project(conn: &Connection, path: &str, agent: &str, profile: &str) {
    conn.execute(
        "INSERT INTO projects (path, agent, profile) VALUES (?1, ?2, ?3) ON CONFLICT(path, agent) DO UPDATE SET profile = excluded.profile",
        params![path, agent, profile],
    ).ok();
}

pub fn get_project(conn: &Connection, path: &str, agent: &str) -> Option<String> {
    // Cascading: find longest matching parent path
    let mut stmt = conn.prepare(
        "SELECT path, profile FROM projects WHERE agent = ?1 ORDER BY length(path) DESC"
    ).unwrap();
    let candidates: Vec<(String, String)> = stmt.query_map(params![agent], |row| {
        Ok((row.get::<_, String>(0)?, row.get(1)?))
    }).unwrap().filter_map(|r| r.ok()).collect();
    for (p, profile) in candidates {
        if path.starts_with(&p) {
            return Some(profile);
        }
    }
    None
}

pub fn unset_project(conn: &Connection, path: &str, agent: &str) {
    conn.execute("DELETE FROM projects WHERE path = ?1 AND agent = ?2", params![path, agent]).ok();
}

pub fn set_rotation_last(conn: &Connection, agent: &str, profile: &str, algorithm: &str) {
    conn.execute(
        "INSERT OR REPLACE INTO rotation_state (agent, last_profile, algorithm) VALUES (?1, ?2, ?3)",
        params![agent, profile, algorithm],
    ).ok();
}

pub fn get_rotation_last(conn: &Connection, agent: &str) -> Option<(String, String)> {
    conn.query_row(
        "SELECT last_profile, algorithm FROM rotation_state WHERE agent = ?1",
        params![agent],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).ok()
}
```

Note: also add `rotation_state` table to schema:
```sql
CREATE TABLE IF NOT EXISTS rotation_state (
    agent TEXT PRIMARY KEY,
    last_profile TEXT,
    algorithm TEXT NOT NULL DEFAULT 'smart'
);
```

- [ ] **Step 6: Run all auth_db tests**

```bash
cargo test auth_db
```

Expected: all PASS (7 tests)

- [ ] **Step 7: Commit**

```bash
git add src/auth_db.rs Cargo.lock
git commit -m "feat: add auth_db SQLite layer for health, cooldown, rotation state"
```

---

### Task 2: Extend auth.rs — rotation, cooldown, health CLI

**Files:**
- Modify: `src/auth.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: `crate::auth_db::*` from Task 1, existing auth functions (backup, activate, etc.)
- Produces: `pub fn rotate(agent: &str, algorithm: &str, json: bool)`, `pub fn next(agent: &str, algorithm: &str, json: bool)`, `pub fn pick(agent: &str)`, `pub fn cooldown_set(target: &str, minutes: Option<u32>, json: bool)`, `pub fn cooldown_list(agent: Option<&str>, json: bool)`, `pub fn cooldown_clear(target: &str, json: bool)`, `pub fn set_alias_cmd(agent: &str, profile: &str, alias: &str, json: bool)`, `pub fn project_set(agent: &str, profile: &str, json: bool)`, `pub fn project_unset(agent: &str, json: bool)`

- [ ] **Step 1: Write rotate function**

```rust
use crate::auth_db::{self, CooldownRow, ProfileHealth};

pub fn rotate(agent: &str, algorithm: &str, json: bool) {
    let conn = auth_db::open_or_rebuild();
    let cooldowns = auth_db::list_cooldowns(&conn, Some(agent));
    let health = auth_db::list_health(&conn, agent);
    let vault_profiles = list_profiles(agent);
    
    let active = vault_profiles.iter().filter(|p| {
        !cooldowns.iter().any(|c| c.profile == **p)
    }).cloned().collect::<Vec<_>>();
    
    if active.is_empty() {
        if json {
            println!("{}", serde_json::json!({"error": "no non-cooldown profiles available"}));
        } else {
            eprintln!("error: all profiles are in cooldown");
        }
        return;
    }
    
    let chosen = match algorithm {
        "round-robin" => round_robin(&conn, agent, &active),
        "random" => random_pick(&active),
        _ => smart_pick(&health, &active, agent),
    };
    
    activate(agent, &chosen, json);
    auth_db::set_rotation_last(&conn, agent, &chosen, algorithm);
}

fn smart_pick(health: &[ProfileHealth], profiles: &[String], agent: &str) -> String {
    let mut scored: Vec<(String, f64)> = profiles.iter().map(|p| {
        let h = health.iter().find(|h| h.profile == *p);
        let base = match h.map(|h| h.status.as_str()) {
            Some("healthy") => 100.0,
            Some("warning") => 50.0,
            Some("critical") => 0.0,
            _ => 100.0,
        };
        let penalty = h.map(|h| h.penalty).unwrap_or(0.0);
        let recency = if h.and_then(|h| h.last_used_at.as_ref()).is_some() { 0.0 } else { 10.0 };
        let jitter = (rand::random::<f64>() * 10.0) - 5.0; // ±5
        (p.clone(), base - penalty + recency + jitter)
    }).collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    scored[0].0.clone()
}

fn round_robin(conn: &Connection, agent: &str, profiles: &[String]) -> String {
    if let Some((last, _)) = auth_db::get_rotation_last(conn, agent) {
        if let Some(pos) = profiles.iter().position(|p| *p == last) {
            let next = (pos + 1) % profiles.len();
            return profiles[next].clone();
        }
    }
    profiles[0].clone()
}

fn random_pick(profiles: &[String]) -> String {
    let idx = rand::random::<usize>() % profiles.len();
    profiles[idx].clone()
}
```

- [ ] **Step 2: Write cooldown CLI functions**

```rust
pub fn cooldown_set(target: &str, minutes: Option<u32>, json: bool) {
    let (agent, profile) = match parse_target(target) {
        Some(p) => p,
        None => { eprintln!("error: expected <agent>/<profile>"); return; }
    };
    let mins = minutes.unwrap_or(60);
    let conn = auth_db::open_or_rebuild();
    auth_db::set_cooldown(&conn, &agent, &profile, mins, "manual");
    if json {
        println!("{}", serde_json::json!({"agent": agent, "profile": profile, "cooldown_minutes": mins}));
    } else {
        println!("cooldown set: {agent}/{profile} for {mins} minutes");
    }
}

pub fn cooldown_list(agent: Option<&str>, json: bool) {
    let conn = auth_db::open_or_rebuild();
    let list = auth_db::list_cooldowns(&conn, agent);
    if json {
        println!("{}", serde_json::to_string(&list.iter().map(|c| serde_json::json!({
            "agent": c.agent, "profile": c.profile, "until": c.until, "reason": c.reason,
        })).collect::<Vec<_>>()).unwrap());
    } else if list.is_empty() {
        println!("no active cooldowns");
    } else {
        for c in &list {
            println!("  {}/{}  until {}  {}", c.agent, c.profile, c.until, c.reason.as_deref().unwrap_or(""));
        }
    }
}

pub fn cooldown_clear(target: &str, json: bool) {
    let (agent, profile) = match parse_target(target) {
        Some(p) => p,
        None => { eprintln!("error: expected <agent>/<profile>"); return; }
    };
    let conn = auth_db::open_or_rebuild();
    auth_db::clear_cooldown(&conn, &agent, &profile);
    if json {
        println!("{}", serde_json::json!({"cleared": true, "agent": agent, "profile": profile}));
    } else {
        println!("cooldown cleared: {agent}/{profile}");
    }
}

fn parse_target(target: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = target.splitn(2, '/').collect();
    if parts.len() == 2 {
        Some((parts[0].to_string(), parts[1].to_string()))
    } else {
        None
    }
}
```

- [ ] **Step 3: Write next, pick, alias, project functions**

```rust
pub fn next(agent: &str, algorithm: &str, json: bool) {
    let conn = auth_db::open_or_rebuild();
    let cooldowns = auth_db::list_cooldowns(&conn, Some(agent));
    let health = auth_db::list_health(&conn, agent);
    let profiles = list_profiles(agent);
    let active: Vec<String> = profiles.iter()
        .filter(|p| !cooldowns.iter().any(|c| c.profile == **p))
        .cloned().collect();
    let chosen = if active.is_empty() {
        "(none — all cooldown'd)".to_string()
    } else {
        match algorithm {
            "round-robin" => round_robin(&conn, agent, &active),
            "random" => random_pick(&active),
            _ => smart_pick(&health, &active, agent),
        }
    };
    if json {
        println!("{}", serde_json::json!({"agent": agent, "next": chosen, "algorithm": algorithm}));
    } else {
        println!("next rotation for {agent} [{algorithm}]: {chosen}");
    }
}

pub fn pick(agent: &str) {
    let profiles = list_profiles(agent);
    if profiles.is_empty() {
        println!("no profiles for {agent}");
        return;
    }
    for (i, p) in profiles.iter().enumerate() {
        println!("  [{}] {p}", i + 1);
    }
    print!("choose profile: ");
    use std::io::Write;
    std::io::stdout().flush().ok();
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).ok();
    if let Ok(idx) = input.trim().parse::<usize>() {
        if idx > 0 && idx <= profiles.len() {
            activate(agent, &profiles[idx - 1], false);
            return;
        }
    }
    eprintln!("invalid selection");
}

pub fn set_alias_cmd(agent: &str, profile: &str, alias: &str, json: bool) {
    let conn = auth_db::open_or_rebuild();
    auth_db::set_alias(&conn, agent, alias, profile);
    if json {
        println!("{}", serde_json::json!({"agent": agent, "alias": alias, "profile": profile}));
    } else {
        println!("alias set: {agent}/{alias} -> {profile}");
    }
}

pub fn project_set(agent: &str, profile: &str, json: bool) {
    let cwd = std::env::current_dir().unwrap_or_default();
    let path = cwd.to_string_lossy().to_string();
    let conn = auth_db::open_or_rebuild();
    auth_db::set_project(&conn, &path, agent, profile);
    if json {
        println!("{}", serde_json::json!({"path": path, "agent": agent, "profile": profile}));
    } else {
        println!("project set: {path} -> {agent}/{profile}");
    }
}

pub fn project_unset(agent: &str, json: bool) {
    let cwd = std::env::current_dir().unwrap_or_default();
    let path = cwd.to_string_lossy().to_string();
    let conn = auth_db::open_or_rebuild();
    auth_db::unset_project(&conn, &path, agent);
    if json {
        println!("{}", serde_json::json!({"path": path, "agent": agent, "unset": true}));
    } else {
        println!("project unset: {path}/{agent}");
    }
}
```

- [ ] **Step 4: Update activate to resolve aliases and projects**

```rust
// At the top of existing activate function, add:
let profile = resolve_name(agent, profile);

// Before catalog lookup, add:
fn resolve_name(agent: &str, name: &str) -> String {
    let conn = auth_db::open_or_rebuild();
    // Check alias first
    if let Some(real) = auth_db::resolve_alias(&conn, agent, name) {
        return real;
    }
    // Check project association
    let cwd = std::env::current_dir().unwrap_or_default();
    let path = cwd.to_string_lossy().to_string();
    if let Some(project_profile) = auth_db::get_project(&conn, &path, agent) {
        return project_profile;
    }
    name.to_string()
}
```

- [ ] **Step 5: Write auth tests (rotation, cooldown, alias)**

```rust
#[test]
fn cooldown_set_and_rotate_skips() {
    with_temp_home(|| {
        // Setup: backup two profiles
        setup_vault_profile("claude-code", "alice", r#"{"token":"a"}"#);
        setup_vault_profile("claude-code", "bob", r#"{"token":"b"}"#);
        
        // Set cooldown on alice
        let conn = auth_db::open_or_rebuild();
        auth_db::set_cooldown(&conn, "claude-code", "alice", 60, "test");
        
        // Activate using simple pick (not full rotate — test that cooldown'd is skipped)
        let profiles = list_profiles("claude-code");
        let cooldowns = auth_db::list_cooldowns(&conn, Some("claude-code"));
        let active: Vec<_> = profiles.iter().filter(|p| !cooldowns.iter().any(|c| c.profile == **p)).collect();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0], "bob");
    });
}

#[test]
fn alias_resolves_in_activate() {
    with_temp_home(|| {
        setup_vault_profile("claude-code", "work@company.com", r#"{"token":"x"}"#);
        let conn = auth_db::open_or_rebuild();
        auth_db::set_alias(&conn, "claude-code", "w", "work@company.com");
        
        let resolved = auth_db::resolve_alias(&conn, "claude-code", "w");
        assert_eq!(resolved, Some("work@company.com".to_string()));
    });
}

fn setup_vault_profile(agent: &str, profile: &str, content: &str) {
    let dir = profile_dir(agent, profile);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("auth.json"), content).unwrap();
}
```

- [ ] **Step 6: Run all auth tests**

```bash
cargo test auth
```

Expected: all PASS (includes Phase 1 + Phase 2 tests)

- [ ] **Step 7: Commit**

```bash
git add src/auth.rs src/main.rs
git commit -m "feat: add rotation, cooldown, health scoring, aliases, project associations"
```

---

### Task 3: Update main.rs — refactor Auth to top-level command

**Files:**
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: `crate::auth` functions from Task 2
- Produces: Top-level `Auth` variant in Commands

- [ ] **Step 1: Move AuthAction from AgentsAction to top-level Commands**

Remove `Auth { action: AuthAction }` from `AgentsAction`. Add to `Commands`:

```rust
    /// Auth profile vault — backup, switch, rotate, and manage agent OAuth tokens.
    Auth {
        #[command(subcommand)]
        action: AuthAction,
    },
```

- [ ] **Step 2: Add new AuthAction variants (rotate, cooldown, alias, project)**

Add to `AuthAction` enum (keeping existing: Backup, Activate, Status, Catalog, Ls, Clear, Delete, Rename):

```rust
    /// Smart profile rotation (skips cooldown'd profiles).
    Rotate {
        agent: String,
        /// Rotation algorithm (smart, round-robin, random).
        #[arg(long, default_value = "smart")]
        algorithm: String,
        #[arg(long)]
        json: bool,
    },
    /// Preview what rotation would pick.
    Next {
        agent: String,
        #[arg(long, default_value = "smart")]
        algorithm: String,
        #[arg(long)]
        json: bool,
    },
    /// Interactive profile selector.
    Pick {
        agent: String,
    },
    /// Manage cooldowns.
    Cooldown {
        #[command(subcommand)]
        action: CooldownAction,
    },
    /// Create short alias for a profile.
    Alias {
        agent: String,
        profile: String,
        alias: String,
        #[arg(long)]
        json: bool,
    },
    /// Manage project-profile associations.
    Project {
        #[command(subcommand)]
        action: ProjectAction,
    },
}

#[derive(Subcommand)]
enum CooldownAction {
    /// Block a profile from rotation for N minutes.
    Set {
        /// <agent>/<profile>
        target: String,
        #[arg(long)]
        minutes: Option<u32>,
        #[arg(long)]
        json: bool,
    },
    /// List active cooldowns.
    List {
        agent: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Clear a cooldown.
    Clear {
        /// <agent>/<profile>
        target: String,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum ProjectAction {
    /// Link current directory to a profile.
    Set {
        agent: String,
        profile: String,
        #[arg(long)]
        json: bool,
    },
    /// Remove project association for current directory.
    Unset {
        agent: String,
        #[arg(long)]
        json: bool,
    },
}
```

- [ ] **Step 3: Wire match arms in main()**

Move auth match from `AgentsAction::Auth { action }` to top-level:

```rust
        Commands::Auth { action } => match action {
            AuthAction::Backup { agent, profile, json } => auth::backup(&agent, &profile, json),
            AuthAction::Activate { agent, profile, json } => auth::activate(&agent, &profile, json),
            AuthAction::Status { agent, json } => auth::status(agent.as_deref(), json),
            AuthAction::Catalog { json } => auth::list_agents(json),
            AuthAction::Ls { agent, json } => auth::ls(&agent, json),
            AuthAction::Clear { agent, json } => auth::clear(&agent, json),
            AuthAction::Delete { agent, profile, json } => auth::delete(&agent, &profile, json),
            AuthAction::Rename { agent, old, new, json } => auth::rename(&agent, &old, &new, json),
            AuthAction::Rotate { agent, algorithm, json } => auth::rotate(&agent, &algorithm, json),
            AuthAction::Next { agent, algorithm, json } => auth::next(&agent, &algorithm, json),
            AuthAction::Pick { agent } => auth::pick(&agent),
            AuthAction::Cooldown { action } => match action {
                CooldownAction::Set { target, minutes, json } => auth::cooldown_set(&target, minutes, json),
                CooldownAction::List { agent, json } => auth::cooldown_list(agent.as_deref(), json),
                CooldownAction::Clear { target, json } => auth::cooldown_clear(&target, json),
            },
            AuthAction::Alias { agent, profile, alias, json } => auth::set_alias_cmd(&agent, &profile, &alias, json),
            AuthAction::Project { action } => match action {
                ProjectAction::Set { agent, profile, json } => auth::project_set(&agent, &profile, json),
                ProjectAction::Unset { agent, json } => auth::project_unset(&agent, json),
            },
        },
```

Remove `AgentsAction::Auth` from the `Agents` match arm.

- [ ] **Step 4: Add rand dependency for jitter + random rotation**

```toml
# Cargo.toml
rand = "0.8"
```

- [ ] **Step 5: Build and test**

```bash
cargo build && cargo test auth
```

- [ ] **Step 6: Commit**

```bash
git add src/main.rs Cargo.toml Cargo.lock
git commit -m "refactor: move Auth to top-level command, add rotation/cooldown/alias/project"
```

---

### Task 4: Integration test — full rotate flow

**Files:**
- Modify: `src/auth.rs` (add test)

- [ ] **Step 1: Write integration test**

```rust
#[test]
fn full_rotate_flow() {
    with_temp_home(|| {
        let conn = auth_db::open_or_rebuild();
        setup_vault_profile("claude-code", "alice", r#"{"token":"a"}"#);
        setup_vault_profile("claude-code", "bob", r#"{"token":"b"}"#);
        setup_vault_profile("claude-code", "carol", r#"{"token":"c"}"#);
        
        // Cooldown alice
        auth_db::set_cooldown(&conn, "claude-code", "alice", 60, "manual");
        
        // Record error on bob
        auth_db::record_error(&conn, "claude-code", "bob", "502 Bad Gateway");
        
        // Smart rotate should pick carol (alice cooldown'd, bob has penalty)
        let profiles = list_profiles("claude-code");
        let health: Vec<_> = profiles.iter().map(|p| auth_db::get_health(&conn, "claude-code", p)).collect();
        let active: Vec<_> = profiles.iter().filter(|p| {
            !auth_db::list_cooldowns(&conn, Some("claude-code")).iter().any(|c| c.profile == **p)
        }).cloned().collect();
        let picked = smart_pick(&health, &active, "claude-code");
        assert_eq!(picked, "carol");
    });
}
```

- [ ] **Step 2: Run test**

```bash
cargo test auth::tests::full_rotate_flow
```

Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add src/auth.rs
git commit -m "test: full rotation integration test with cooldown + health"
```

- [ ] **Final: Run full test suite**

```bash
cargo test
```
