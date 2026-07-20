//! Append-only audit log for the git-shim's classified events. Every
//! `classify::Event` handed to `log_event` is appended as one JSONL line —
//! callers that want to suppress `SilentExempt` noise decide that
//! themselves before calling; this module always logs whatever it's given.

use std::io::Write as _;
use std::path::{Path, PathBuf};

use crate::classify::Event;

/// Default audit log location: `~/.agentflare/audit/git.jsonl`.
#[must_use]
pub fn default_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".agentflare").join("audit").join("git.jsonl"))
}

/// Appends one JSONL line for `event`, creating the parent directory and
/// file if needed.
pub fn log_event(audit_path: &Path, event: &Event) -> std::io::Result<()> {
    if let Some(parent) = audit_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(audit_path)?;
    let line = serde_json::to_string(event).map_err(std::io::Error::other)?;
    writeln!(f, "{line}")
}

/// Reads back every event in the log, oldest first. A missing file reads
/// as empty (nothing has been logged yet — not an error). A malformed line
/// fails closed: returns an error rather than silently skipping it, since
/// a corrupt audit entry is a bug worth surfacing, not data to quietly drop.
pub fn read_events(audit_path: &Path) -> std::io::Result<Vec<Event>> {
    let content = match std::fs::read_to_string(audit_path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).map_err(std::io::Error::other))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classify::Disposition;
    use tempfile::TempDir;

    fn sample_event(subcommand: &str) -> Event {
        Event {
            subcommand: subcommand.to_string(),
            args: vec!["origin".to_string()],
            disposition: Disposition::Passthrough,
        }
    }

    #[test]
    fn reading_a_missing_log_is_empty_not_an_error() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("does-not-exist").join("git.jsonl");
        assert_eq!(read_events(&path).unwrap(), Vec::new());
    }

    #[test]
    fn append_then_read_back_round_trips_in_order() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("audit").join("git.jsonl");
        log_event(&path, &sample_event("fetch")).unwrap();
        log_event(&path, &sample_event("push")).unwrap();
        let events = read_events(&path).unwrap();
        assert_eq!(events, vec![sample_event("fetch"), sample_event("push")]);
    }

    #[test]
    fn deny_events_round_trip_with_their_reason() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("git.jsonl");
        let event = Event {
            subcommand: "checkout".to_string(),
            args: vec!["master".to_string()],
            disposition: Disposition::Deny {
                reason: "protected branch".to_string(),
            },
        };
        log_event(&path, &event).unwrap();
        assert_eq!(read_events(&path).unwrap(), vec![event]);
    }

    #[test]
    fn a_malformed_line_fails_closed_instead_of_being_silently_dropped() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("git.jsonl");
        log_event(&path, &sample_event("fetch")).unwrap();
        std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(b"not valid json\n")
            .unwrap();
        assert!(
            read_events(&path).is_err(),
            "a corrupt line must surface as an error, not be silently skipped"
        );
    }
}
