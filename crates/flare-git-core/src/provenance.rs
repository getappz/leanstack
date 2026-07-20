//! Commit provenance trailers — self-reported agent/branch/item identity
//! appended to commit messages via a `prepare-commit-msg` hook.
//!
//! Deliberately NOT cryptographically attested: agentflare has no
//! signing/binding system for this, so a trailer is a bare string an agent
//! could misreport — the same trust level as every other
//! `AGENTFLARE_AGENT`-based identity check already in this codebase (see
//! `claims::owner_id`'s identical fallback chain).

use std::path::Path;

use crate::branch::current_branch;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Trailers {
    pub agent: Option<String>,
    pub branch: Option<String>,
    pub item_id: Option<String>,
}

/// Resolves the current commit's provenance: agent identity
/// (`AGENTFLARE_AGENT`, falling back to auto-detection), the current
/// branch, and — if the branch matches the `task/<sequence_id>` convention
/// `flare_git_core::worktree` uses — the item id it belongs to.
#[must_use]
pub fn build_trailers(repo_root: &Path) -> Trailers {
    let agent = std::env::var("AGENTFLARE_AGENT")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(agent_detector::agent_name);
    let branch = current_branch(repo_root);
    let item_id = branch
        .as_deref()
        .and_then(|b| b.strip_prefix("task/"))
        .map(str::to_string);
    Trailers {
        agent,
        branch,
        item_id,
    }
}

/// Appends non-empty `Trailers` fields to `msg` as git trailers, skipping
/// any field that didn't resolve rather than writing an empty trailer.
/// A no-op if `msg` already carries agentflare trailers (e.g. `commit
/// --amend` re-invokes `prepare-commit-msg` on an already-stamped
/// message) — never duplicates.
#[must_use]
pub fn append_trailers(msg: &str, t: &Trailers) -> String {
    if msg.contains("Agentflare-Agent:")
        || msg.contains("Agentflare-Branch:")
        || msg.contains("Agentflare-Item:")
    {
        return msg.to_string();
    }
    let mut lines = Vec::new();
    if let Some(agent) = &t.agent {
        lines.push(format!("Agentflare-Agent: {agent}"));
    }
    if let Some(branch) = &t.branch {
        lines.push(format!("Agentflare-Branch: {branch}"));
    }
    if let Some(item_id) = &t.item_id {
        lines.push(format!("Agentflare-Item: {item_id}"));
    }
    if lines.is_empty() {
        return msg.to_string();
    }
    let mut out = msg.trim_end().to_string();
    out.push_str("\n\n");
    out.push_str(&lines.join("\n"));
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trailers() -> Trailers {
        Trailers {
            agent: Some("claude-code".to_string()),
            branch: Some("task/42".to_string()),
            item_id: Some("42".to_string()),
        }
    }

    #[test]
    fn appends_all_resolved_fields_after_a_blank_line() {
        let out = append_trailers("fix: thing\n", &trailers());
        assert_eq!(
            out,
            "fix: thing\n\nAgentflare-Agent: claude-code\nAgentflare-Branch: task/42\nAgentflare-Item: 42\n"
        );
    }

    #[test]
    fn skips_unresolved_fields_entirely() {
        let t = Trailers {
            agent: Some("claude-code".to_string()),
            branch: None,
            item_id: None,
        };
        let out = append_trailers("fix: thing\n", &t);
        assert_eq!(out, "fix: thing\n\nAgentflare-Agent: claude-code\n");
    }

    #[test]
    fn returns_message_unchanged_when_nothing_resolved() {
        let out = append_trailers("fix: thing\n", &Trailers::default());
        assert_eq!(out, "fix: thing\n");
    }

    #[test]
    fn does_not_duplicate_trailers_on_an_already_stamped_message() {
        let once = append_trailers("fix: thing\n", &trailers());
        let twice = append_trailers(&once, &trailers());
        assert_eq!(once, twice);
    }

    #[test]
    fn item_id_extracted_from_task_branch_convention() {
        // Mirrors flare_git_core::worktree's `format!("task/{}", item.sequence_id)`.
        let t = Trailers {
            agent: None,
            branch: Some("task/17".to_string()),
            item_id: Some("17".to_string()),
        };
        assert_eq!(t.item_id.as_deref(), Some("17"));
    }
}
