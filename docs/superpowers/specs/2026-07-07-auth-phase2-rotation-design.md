# Phase 2: Auth Rotation, Cooldown & Health Scoring

**Status:** design  
**Issue:** [#23](https://github.com/getappz/agentflare/issues/23)  
**Depends on:** Phase 1 (vault + basic switching — in master)

## Summary

Add smart multi-profile rotation, cooldown tracking, and health scoring to the auth profile vault. Uses SQLite (rusqlite, already in deps) for state persistence, matching the `src/rollup.rs` pattern.

## Architecture

```
agentflare auth rotate <agent>          # smart rotation (default)
agentflare auth rotate <agent> --algorithm round-robin|random
agentflare auth next <agent>            # preview rotation result
agentflare auth pick <agent>            # interactive fzf-style selector
agentflare auth cooldown set <agent>/<profile> [--minutes N]
agentflare auth cooldown list [agent]
agentflare auth cooldown clear <agent>/<profile>
agentflare auth alias <agent> <profile> <alias>
agentflare auth project set <agent> <profile>
agentflare auth project unset <agent>
```

Commands use slug targets: `<agent>/<profile>` for cooldown/alias where both are needed.

Top-level refactor: Phase 1 `agents auth *` commands move to `agentflare auth *` for consistency.

## Files

| File | Purpose |
|------|---------|
| `src/auth_db.rs` | SQLite schema, migrations, CRUD for health/cooldowns/aliases/projects |
| `src/auth.rs` | Extended CLI dispatch + rotation logic + health scoring |
| `src/main.rs` | Refactor AuthAction to top-level `Auth` command |

## Database

Reuses `rusqlite` from `src/rollup.rs`. DB path: `~/.local/share/agentflare/auth.db`. Migrations follow `rollup.rs` pattern: `SCHEMA_VERSION` tracking, `migrate()` function, `open_or_rebuild()`.

### Schema v2 (extends v1 — vault tables already present)

```sql
CREATE TABLE profile_health (
    agent       TEXT NOT NULL,
    profile     TEXT NOT NULL,
    status      TEXT NOT NULL DEFAULT 'healthy',
    error_count_1h INTEGER NOT NULL DEFAULT 0,
    penalty     REAL NOT NULL DEFAULT 0.0,
    last_used_at TEXT,
    updated_at  TEXT NOT NULL,
    PRIMARY KEY (agent, profile)
);

CREATE TABLE cooldowns (
    agent   TEXT NOT NULL,
    profile TEXT NOT NULL,
    until   TEXT NOT NULL,
    reason  TEXT,
    PRIMARY KEY (agent, profile)
);

CREATE TABLE aliases (
    agent   TEXT NOT NULL,
    alias   TEXT NOT NULL,
    profile TEXT NOT NULL,
    PRIMARY KEY (agent, alias)
);

CREATE TABLE projects (
    path    TEXT NOT NULL,
    agent   TEXT NOT NULL,
    profile TEXT NOT NULL,
    PRIMARY KEY (path, agent)
);
```

## Health Scoring

Passive — health inferred from error history, not token introspection.

### Status Tiers

| Status | Condition |
|--------|-----------|
| `healthy` | 0 errors in last hour, no cooldown |
| `warning` | 1+ errors in last hour, or penalty > 5.0 |
| `critical` | 5+ errors in last hour, or explicitly cooldown'd |

### Penalty System

Error types and their penalty weights:

| Error type | Penalty | Detection |
|-----------|---------|-----------|
| rate_limit | 10.0 | Output contains "429" / "rate limit" / "too many requests" |
| auth_error | 100.0 | Output contains "401" / "403" / "unauthorized" |
| timeout | 5.0 | Timeout or "deadline exceeded" |
| server_error | 5.0 | Output contains "500" / "502" / "503" / "504" |
| unknown | 3.0 | Any other error |

Exponential decay: penalty × 0.8 every 5 minutes since `last_error` time.

## Rotation Algorithms

### Smart (default)

Multi-factor scoring per profile:
1. Health base: healthy=100, warning=50, critical=0
2. Minus penalty (if any)
3. Plus recency bonus: +10 if never used in last 30 min
4. Plus random jitter: ±5
5. Highest score wins

Skips: cooldown'd profiles, profiles with auth_error penalty >= 100.

### Round-robin

Sequential through profiles in alphabetical order. Skips cooldown'd profiles. Tracks position per agent in `rotation_state.last_profile`.

### Random

Uniform random among all non-cooldown profiles.

## Cooldown

- `cooldown set <agent>/<profile> --minutes 60` — blocks profile for N minutes
- Default: 60 minutes
- Setting cooldown auto-calculates: if no `--minutes`, uses default
- `cooldown list` — shows all active cooldowns with remaining time
- `cooldown clear` — removes cooldown entry
- Rotation algorithms automatically skip cooldown'd profiles
- Activating a cooldown'd profile warns + confirms (--force bypasses)

## Aliases

- `auth alias claude-code work work@company.com` — maps alias `work` to full profile name
- Resolved transparently in `activate`, `rotate`, etc.
- `auth ls` shows aliases inline: `work -> work@company.com`

## Project Associations

- `auth project set claude-code work@company.com` — binds profile to CWD
- Cascading: parent dir associations apply to subdirectories
- `auth activate claude-code` auto-resolves to project-associated profile
- Stored as absolute path → profile mapping

## CLI Structure (in main.rs)

```rust
Auth {
    #[command(subcommand)]
    action: AuthAction,
}
```

`AuthAction` moves from `AgentsAction` to become top-level, gaining new variants: Rotate, Next, Pick, Cooldown { Set/List/Clear }, Alias, Project { Set/Unset }.

## JSON Output

All commands support `--json`. Rotation output:
```json
{"agent":"claude-code","profile":"bob@gmail.com","algorithm":"smart","reason":"best health score (100)","skipped":["alice@gmail.com (cooldown)"]}
```

## Testing

- All DB operations tested with temp SQLite in `with_temp_home`
- Rotation: test that cooldown'd profiles are skipped
- Penalty: test decay calculation, error categorization
- Health: test status transitions based on error counts
- Project associations: test cascading resolution
- 10-12 new tests

## Out of Scope (Phase 3)

- `auth run` wrapper with automatic failover
- Daemon detection + reload
- Profile isolation (isolated/shallow profiles)
- `auth exec` / `auth login`
