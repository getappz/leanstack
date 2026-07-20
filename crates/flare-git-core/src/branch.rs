//! Branch resolution and the protected-branch predicate — the latter is
//! shared by both agentflare's own PreToolUse branch guard and the new
//! git-shim's command classifier, so "is this branch protected" has exactly
//! one definition.

use crate::shell::{run_in_opt, run_in_ok};
use std::path::{Path, PathBuf};

/// Current branch name (`HEAD` in detached-HEAD state). `None` outside a git
/// repo or if git isn't on `PATH`.
#[must_use]
pub fn current_branch(repo_root: &Path) -> Option<String> {
    run_in_opt(repo_root, &["rev-parse", "--abbrev-ref", "HEAD"])
}

/// Best-effort resolution of "the" default branch: prefer the remote's own
/// record of it (`origin/HEAD`'s symbolic ref, which survives a repo default
/// named anything other than main/master), then whichever of main/master
/// actually exists as a local branch, then whatever is actually checked out
/// here — so a repo naming its default branch e.g. `trunk`/`develop` still
/// resolves instead of falling through to a hardcoded guess that may not
/// even exist.
#[must_use]
pub fn resolve_default_branch(repo_root: &Path) -> String {
    if let Some(origin_head) = run_in_opt(
        repo_root,
        &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
    ) && let Some(stripped) = origin_head.strip_prefix("origin/")
    {
        return stripped.to_string();
    }
    if run_in_ok(repo_root, &["rev-parse", "--verify", "main"]) {
        return "main".to_string();
    }
    if run_in_ok(repo_root, &["rev-parse", "--verify", "master"]) {
        return "master".to_string();
    }
    run_in_opt(repo_root, &["symbolic-ref", "--short", "HEAD"]).unwrap_or_else(|| "master".to_string())
}

/// `git rev-parse --show-toplevel` from `start` — handles worktrees/submodules
/// correctly, works regardless of subdirectory. `None` outside a git repo.
#[must_use]
pub fn repo_toplevel(start: &Path) -> Option<PathBuf> {
    run_in_opt(start, &["rev-parse", "--show-toplevel"]).map(PathBuf::from)
}

/// `true` if `branch` is protected: the repo's resolved default branch when
/// known, otherwise a bare guess against the two conventional names
/// ("main"/"master") when resolution failed entirely (no git, no remote, no
/// main/master branch found).
#[must_use]
pub fn is_protected_branch(branch: &str, default: Option<&str>) -> bool {
    match default {
        Some(default) => branch == default,
        None => branch == "main" || branch == "master",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::test_support::init_repo_with_branch;

    #[test]
    fn current_branch_reports_the_checked_out_branch() {
        let repo = init_repo_with_branch("feature/x");
        assert_eq!(current_branch(&repo.path).as_deref(), Some("feature/x"));
    }

    #[test]
    fn resolve_default_branch_resolves_from_origin_head() {
        let repo = init_repo_with_branch("master");
        assert_eq!(resolve_default_branch(&repo.path), "master");
    }

    #[test]
    fn resolve_default_branch_falls_back_to_actual_head_for_nonstandard_names() {
        // No origin, no "main", no "master" — must not guess "master" when
        // the repo's real default branch is named something else entirely.
        let repo = init_repo_with_branch("trunk");
        assert_eq!(resolve_default_branch(&repo.path), "trunk");
    }

    #[test]
    fn repo_toplevel_finds_root_from_a_subdirectory() {
        let repo = init_repo_with_branch("master");
        let sub = repo.path.join("sub");
        std::fs::create_dir(&sub).unwrap();
        assert_eq!(
            repo_toplevel(&sub).map(|p| p.canonicalize().unwrap()),
            repo.path.canonicalize().ok(),
        );
    }

    #[test]
    fn is_protected_branch_prefers_default_over_hardcoded_names() {
        // A repo whose default branch is deliberately named neither
        // main nor master must still be caught via the resolved default.
        assert!(is_protected_branch("develop", Some("develop")));
        assert!(!is_protected_branch("feature/y", Some("develop")));
    }

    #[test]
    fn is_protected_branch_falls_back_to_main_or_master_when_default_unresolved() {
        assert!(is_protected_branch("master", None));
        assert!(is_protected_branch("main", None));
        assert!(!is_protected_branch("feature/x", None));
    }
}
