//! `init`-time guarantee that a GitHub credential exists for a github repo.

#[derive(Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum CredState {
    Present(&'static str),
    Missing,
}

#[allow(dead_code)]
pub fn classify(env_present: bool, gh_present: bool) -> CredState {
    if env_present {
        CredState::Present("GITHUB_TOKEN")
    } else if gh_present {
        CredState::Present("gh")
    } else {
        CredState::Missing
    }
}

#[allow(dead_code)]
fn env_present() -> bool {
    ["GITHUB_TOKEN", "GH_TOKEN"].iter().any(|v| std::env::var(v).map(|s| !s.trim().is_empty()).unwrap_or(false))
}

#[allow(dead_code)]
fn gh_present() -> bool {
    std::process::Command::new("gh").args(["auth", "token"]).output()
        .map(|o| o.status.success() && !o.stdout.is_empty()).unwrap_or(false)
}

#[allow(dead_code)]
pub fn is_github_repo(repo_root: &std::path::Path) -> bool {
    crate::git::run_in_opt(repo_root, &["remote", "get-url", "origin"])
        .map(|u| u.contains("github")).unwrap_or(false)
}

#[allow(dead_code)]
fn store_token(token: &str) -> Result<(), String> {
    let conn = crate::db::open().map_err(|e| e.to_string())?;
    crate::gateway_secrets::set_secret(&conn, "github_token", token).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn classify_prefers_env_then_gh_then_missing() {
        assert_eq!(classify(true, true), CredState::Present("GITHUB_TOKEN"));
        assert_eq!(classify(false, true), CredState::Present("gh"));
        assert_eq!(classify(false, false), CredState::Missing);
    }
}
