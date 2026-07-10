//! Append-only JSONL ledger of everything flared observed and did.

use std::path::PathBuf;

pub struct EventLog {
    path: PathBuf,
}

impl EventLog {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { path: dir.into().join("events.jsonl") }
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// Append one event line: `{"ts": <unix>, "kind": ..., "detail": ...}`.
    pub fn append(&self, kind: &str, detail: serde_json::Value) -> eyre::Result<()> {
        use std::io::Write;
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let line = serde_json::json!({ "ts": ts, "kind": kind, "detail": detail });
        let mut file =
            std::fs::OpenOptions::new().create(true).append(true).open(&self.path)?;
        writeln!(file, "{line}")?;
        Ok(())
    }

    /// Last `n` events, oldest first. Missing file -> empty.
    pub fn tail(&self, n: usize) -> eyre::Result<Vec<serde_json::Value>> {
        let text = match std::fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(err) => return Err(err.into()),
        };
        let events: Vec<serde_json::Value> = text
            .lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();
        let skip = events.len().saturating_sub(n);
        Ok(events.into_iter().skip(skip).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn append_then_tail_returns_last_n_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let log = EventLog::new(dir.path());
        for i in 0..3 {
            log.append("sweep", serde_json::json!({ "i": i })).unwrap();
        }
        let tail = log.tail(2).unwrap();
        assert_eq!(tail.len(), 2);
        assert_eq!(tail[0]["detail"]["i"], 1);
        assert_eq!(tail[1]["detail"]["i"], 2);
        assert_eq!(tail[1]["kind"], "sweep");
        assert!(tail[1]["ts"].is_u64());
    }

    #[test]
    fn tail_on_missing_file_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let log = EventLog::new(dir.path());
        assert_eq!(log.tail(10).unwrap(), Vec::<serde_json::Value>::new());
    }
}
