//! Bridges the GitHub operations to the `flare_git` MCP tool. Kept thin: the
//! `mcp_server` tool method validates fields and calls into here; this module
//! classifies `GitHubError` so the server can map it to the right `ErrorData`.

use crate::github::GitHubError;
use crate::github::models::{CheckRun, Comment, PullRequest, Review, ReviewComment};

/// `true` = a client mistake / auth problem (invalid_params); `false` =
/// internal/transport failure (internal_error).
pub fn is_client_error(err: &GitHubError) -> bool {
    matches!(
        err,
        GitHubError::NoAuth(_) | GitHubError::Forbidden(_) | GitHubError::NotFound
    )
}

/// Assembles the `pr_status` payload: one JSON blob covering everything a
/// review-and-fix loop otherwise pulls with separate `pr_get` / `run_list` /
/// review-comment / issue-comment round trips.
///
/// Known gap: the REST API doesn't expose review-thread resolution (only
/// GraphQL's `isResolved` does), so `review_comments` includes resolved
/// threads too — callers should treat old/superseded comments with
/// judgment, not as unconditionally outstanding.
pub fn pr_status_json(
    pr: &PullRequest,
    checks: &[CheckRun],
    reviews: &[Review],
    review_comments: &[ReviewComment],
    comments: &[Comment],
) -> String {
    let value = serde_json::json!({
        "number": pr.number,
        "title": pr.title,
        "state": pr.state,
        "draft": pr.draft,
        "mergeable": pr.mergeable,
        "mergeable_state": pr.mergeable_state,
        "additions": pr.additions,
        "deletions": pr.deletions,
        "changed_files": pr.changed_files,
        "html_url": pr.html_url,
        "head_ref": pr.head.as_ref().map(|h| &h.git_ref),
        "base_ref": pr.base.as_ref().map(|h| &h.git_ref),
        "checks": checks.iter().map(|c| serde_json::json!({
            "name": c.name, "status": c.status, "conclusion": c.conclusion,
        })).collect::<Vec<_>>(),
        "reviews": reviews.iter().map(|r| serde_json::json!({
            "author": r.user.login, "state": r.state, "body": r.body, "submitted_at": r.submitted_at,
        })).collect::<Vec<_>>(),
        "review_comments": review_comments.iter().map(|c| serde_json::json!({
            "author": c.user.login, "path": c.path, "line": c.line, "body": c.body,
        })).collect::<Vec<_>>(),
        "comments": comments.iter().map(|c| serde_json::json!({
            "author": c.user.login, "body": c.body, "created_at": c.created_at,
        })).collect::<Vec<_>>(),
    });
    serde_json::to_string(&value).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_and_notfound_are_client_errors_transport_is_not() {
        assert!(is_client_error(&GitHubError::NoAuth("x".into())));
        assert!(is_client_error(&GitHubError::NotFound));
        assert!(!is_client_error(&GitHubError::RateLimited("x".into())));
        assert!(!is_client_error(&GitHubError::Transport("x".into())));
        assert!(!is_client_error(&GitHubError::Parse("x".into())));
    }

    fn sample_pr() -> PullRequest {
        serde_json::from_value(serde_json::json!({
            "number": 42, "html_url": "https://gh/o/r/pull/42", "state": "open",
            "title": "fix: thing", "draft": false, "mergeable": true,
            "mergeable_state": "clean", "additions": 5, "deletions": 1, "changed_files": 2
        }))
        .unwrap()
    }

    #[test]
    fn pr_status_json_bundles_every_section() {
        let pr = sample_pr();
        let checks: Vec<CheckRun> = serde_json::from_value(serde_json::json!([
            {"name": "ci", "status": "completed", "conclusion": "success"}
        ]))
        .unwrap();
        let reviews: Vec<Review> = serde_json::from_value(serde_json::json!([
            {"user": {"login": "coderabbitai[bot]"}, "state": "CHANGES_REQUESTED", "body": "", "submitted_at": null}
        ]))
        .unwrap();
        let review_comments: Vec<ReviewComment> = serde_json::from_value(serde_json::json!([
            {"user": {"login": "bob"}, "path": "src/x.rs", "line": 10, "body": "nit"}
        ]))
        .unwrap();
        let comments: Vec<Comment> = serde_json::from_value(serde_json::json!([
            {"user": {"login": "coderabbitai[bot]"}, "body": "walkthrough", "created_at": null}
        ]))
        .unwrap();

        let out = pr_status_json(&pr, &checks, &reviews, &review_comments, &comments);
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["number"], 42);
        assert_eq!(v["mergeable_state"], "clean");
        assert_eq!(v["checks"][0]["conclusion"], "success");
        assert_eq!(v["reviews"][0]["author"], "coderabbitai[bot]");
        assert_eq!(v["review_comments"][0]["line"], 10);
        assert_eq!(v["comments"][0]["author"], "coderabbitai[bot]");
    }

    #[test]
    fn pr_status_json_handles_empty_sections() {
        let pr = sample_pr();
        let out = pr_status_json(&pr, &[], &[], &[], &[]);
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert!(v["checks"].as_array().unwrap().is_empty());
        assert!(v["reviews"].as_array().unwrap().is_empty());
    }
}
