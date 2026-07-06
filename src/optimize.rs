use crate::paths::home;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Default)]
pub struct RuntimeState {
    #[serde(default)]
    pub sessions: HashMap<String, SessionRecord>,
}

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct SessionRecord {
    pub start_ts: u64,
    #[serde(default)]
    pub turn_count: u32,
    #[serde(default)]
    pub recent_tool_calls: Vec<ToolCallRecord>,
}

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct ToolCallRecord {
    pub name: String,
    pub ts: u64,
}

const STALE_SESSION_SECS: u64 = 24 * 60 * 60;

pub fn runtime_state_path() -> PathBuf {
    crate::state::state_dir().join("runtime-state.json")
}

pub fn load_runtime() -> RuntimeState {
    fs::read_to_string(runtime_state_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_runtime(state: &RuntimeState) {
    let _ = fs::create_dir_all(crate::state::state_dir());
    if let Ok(json) = serde_json::to_string_pretty(state) {
        let _ = fs::write(runtime_state_path(), json + "\n");
    }
}

pub fn prune_stale_sessions(state: &mut RuntimeState, now: u64) {
    state
        .sessions
        .retain(|_, record| now.saturating_sub(record.start_ts) < STALE_SESSION_SECS);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::test_support::with_temp_home;

    #[test]
    fn load_defaults_to_empty_when_no_file() {
        with_temp_home(|| {
            assert!(load_runtime().sessions.is_empty());
        });
    }

    #[test]
    fn save_then_load_roundtrips() {
        with_temp_home(|| {
            let mut state = RuntimeState::default();
            state.sessions.insert(
                "sess-1".to_string(),
                SessionRecord { start_ts: 1000, turn_count: 3, recent_tool_calls: vec![] },
            );
            save_runtime(&state);
            let loaded = load_runtime();
            assert_eq!(loaded.sessions.len(), 1);
            assert_eq!(loaded.sessions["sess-1"].turn_count, 3);
        });
    }

    #[test]
    fn load_falls_back_to_default_on_corrupt_file() {
        with_temp_home(|| {
            fs::create_dir_all(crate::state::state_dir()).unwrap();
            fs::write(runtime_state_path(), "not json").unwrap();
            assert!(load_runtime().sessions.is_empty());
        });
    }

    #[test]
    fn prune_drops_sessions_older_than_24h_keeps_recent() {
        let mut state = RuntimeState::default();
        state.sessions.insert(
            "old".to_string(),
            SessionRecord { start_ts: 0, turn_count: 1, recent_tool_calls: vec![] },
        );
        state.sessions.insert(
            "recent".to_string(),
            SessionRecord { start_ts: 100_000, turn_count: 1, recent_tool_calls: vec![] },
        );
        let now = 100_100; // 100s after "recent", ~27.7h after "old"
        prune_stale_sessions(&mut state, now);
        assert!(!state.sessions.contains_key("old"));
        assert!(state.sessions.contains_key("recent"));
    }
}
