use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

fn run_git_in(repo_root: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(repo_root)
        .output()
        .map_err(|e| format!("git not available: {e}"))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(stderr);
    }
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok(stdout)
}

fn run_git_in_ok(repo_root: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .args(args)
        .current_dir(repo_root)
        .output()
        .ok()
        .is_some_and(|o| o.status.success())
}

pub fn resolve_target_branch(
    conn: &rusqlite::Connection,
    item: &agentflare_backend::item::Item,
    repo_root: &Path,
) -> String {
    if let Some(ref parent_id) = item.parent_id
        && let Ok(parent) = agentflare_backend::item::get(conn, parent_id)
        && let Ok(meta) = serde_json::from_str::<serde_json::Value>(&parent.metadata)
        && let Some(branch) = meta.get("branch").and_then(|v| v.as_str())
    {
        return branch.to_string();
    }
    resolve_default_branch(repo_root)
}

fn resolve_default_branch(repo_root: &Path) -> String {
    if let Ok(out) = run_git_in(
        repo_root,
        &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
    ) && let Some(stripped) = out.strip_prefix("origin/")
    {
        return stripped.to_string();
    }
    if run_git_in_ok(repo_root, &["rev-parse", "--verify", "main"]) {
        return "main".to_string();
    }
    "master".to_string()
}

pub fn already_isolated_for(branch: &str, repo_root: &Path) -> bool {
    let git_dir = match run_git_in(repo_root, &["rev-parse", "--git-dir"]) {
        Ok(d) => d,
        Err(_) => return false,
    };
    let common_dir = match run_git_in(repo_root, &["rev-parse", "--git-common-dir"]) {
        Ok(d) => d,
        Err(_) => return false,
    };
    if git_dir == common_dir {
        return false;
    }
    if run_git_in_ok(
        repo_root,
        &["rev-parse", "--show-superproject-working-tree"],
    ) {
        return false;
    }
    match run_git_in(repo_root, &["branch", "--show-current"]) {
        Ok(b) => b == branch,
        Err(_) => false,
    }
}

pub fn ensure_worktrees_ignored(repo_root: &Path) {
    let gitignore = repo_root.join(".gitignore");
    if let Ok(existing) = std::fs::read_to_string(&gitignore)
        && existing
            .lines()
            .any(|l| l.trim() == ".worktrees/" || l.trim() == ".worktrees")
    {
        return;
    }
    let mut content = String::new();
    if let Ok(existing) = std::fs::read_to_string(&gitignore) {
        content = existing;
    }
    if !content.ends_with('\n') && !content.is_empty() {
        content.push('\n');
    }
    content.push_str(".worktrees/\n");
    if std::fs::write(&gitignore, content).is_err() {
        eprintln!("worktree: failed to write .gitignore");
        return;
    }
    if !run_git_in_ok(repo_root, &["add", ".gitignore"]) {
        eprintln!("worktree: failed to git add .gitignore");
        return;
    }
    if !run_git_in_ok(repo_root, &["commit", "-m", "chore: ignore .worktrees/"]) {
        eprintln!("worktree: failed to commit .gitignore");
    }
}

pub fn create_for_item(
    conn: &rusqlite::Connection,
    item: &agentflare_backend::item::Item,
    repo_root: &Path,
) -> Option<PathBuf> {
    let branch = format!("task/{}", item.sequence_id);
    if already_isolated_for(&branch, repo_root) {
        return Some(std::env::current_dir().unwrap_or_else(|_| repo_root.to_path_buf()));
    }
    let target = resolve_target_branch(conn, item, repo_root);
    ensure_worktrees_ignored(repo_root);
    let worktree_path = repo_root
        .join(".worktrees")
        .join("task")
        .join(item.sequence_id.to_string());
    if let Some(parent) = worktree_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match run_git_in(
        repo_root,
        &[
            "worktree",
            "add",
            &worktree_path.to_string_lossy(),
            "-b",
            &branch,
            &target,
        ],
    ) {
        Ok(_) => Some(worktree_path),
        Err(e) => {
            eprintln!("worktree: creation skipped for item {}: {}", item.id, e);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    struct Repo {
        _dir: TempDir,
        path: PathBuf,
    }

    fn init_repo() -> Repo {
        let dir = TempDir::new().unwrap();
        let path = dir.path().to_path_buf();
        run_git_in(&path, &["init"]).unwrap();
        run_git_in(&path, &["config", "user.email", "test@test.com"]).unwrap();
        run_git_in(&path, &["config", "user.name", "Test"]).unwrap();
        run_git_in(&path, &["commit", "--allow-empty", "-m", "initial"]).unwrap();
        Repo { _dir: dir, path }
    }

    #[test]
    fn resolve_default_branch_resolves_from_origin_head() {
        let repo = init_repo();
        assert_eq!(resolve_default_branch(&repo.path), "master");
    }

    #[test]
    fn resolve_default_branch_falls_back_when_no_remote() {
        let repo = init_repo();
        assert_eq!(resolve_default_branch(&repo.path), "master");
    }

    #[test]
    fn ensure_worktrees_ignored_is_noop_when_already_ignored() {
        let repo = init_repo();
        std::fs::write(repo.path.join(".gitignore"), ".worktrees/\n").unwrap();
        run_git_in_ok(&repo.path, &["add", ".gitignore"]);
        run_git_in_ok(&repo.path, &["commit", "-m", "add gitignore"]);
        let before = std::fs::read_to_string(repo.path.join(".gitignore")).unwrap();
        ensure_worktrees_ignored(&repo.path);
        let after = std::fs::read_to_string(repo.path.join(".gitignore")).unwrap();
        assert_eq!(before, after);
    }

    #[test]
    fn ensure_worktrees_ignored_adds_and_commits_when_missing() {
        let repo = init_repo();
        ensure_worktrees_ignored(&repo.path);
        let content = std::fs::read_to_string(repo.path.join(".gitignore")).unwrap();
        assert!(content.contains(".worktrees/"));
    }

    #[test]
    fn already_isolated_for_false_in_regular_repo() {
        let repo = init_repo();
        assert!(!already_isolated_for("task/1", &repo.path));
    }

    #[test]
    fn create_for_item_creates_worktree_and_branch() {
        let repo = init_repo();
        let worktree_path = repo.path.join(".worktrees").join("task").join("1");
        let item = agentflare_backend::item::Item {
            id: "test-id".into(),
            project_id: "proj".into(),
            state_id: "state".into(),
            name: "test".into(),
            description: String::new(),
            priority: "none".into(),
            parent_id: None,
            assignee_agent: None,
            sequence_id: 1,
            sort_order: 0.0,
            started_at: None,
            completed_at: None,
            archived_at: None,
            external_source: None,
            external_id: None,
            metadata: "{}".into(),
            created_at: 0,
            updated_at: 0,
            deleted_at: None,
        };
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        let result = create_for_item(&conn, &item, &repo.path);
        assert!(result.is_some());
        assert!(worktree_path.exists());
    }

    #[test]
    fn create_for_item_soft_fails_on_bad_git() {
        let tmp = TempDir::new().unwrap();
        let bad_root = tmp.path().join("not-a-repo");
        std::fs::create_dir_all(&bad_root).unwrap();
        let item = agentflare_backend::item::Item {
            id: "test-id".into(),
            project_id: "proj".into(),
            state_id: "state".into(),
            name: "test".into(),
            description: String::new(),
            priority: "none".into(),
            parent_id: None,
            assignee_agent: None,
            sequence_id: 1,
            sort_order: 0.0,
            started_at: None,
            completed_at: None,
            archived_at: None,
            external_source: None,
            external_id: None,
            metadata: "{}".into(),
            created_at: 0,
            updated_at: 0,
            deleted_at: None,
        };
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        let result = create_for_item(&conn, &item, &bad_root);
        assert!(result.is_none());
    }
}
