//! Local git primitives, worktree management, and PATH-shim policy for
//! agentflare -- single source of truth for anything that shells out to
//! `git` locally, consumed by the CLI, MCP server, and the `git` PATH shim.
//! Remote GitHub REST API operations (PRs/issues/CI/releases) are a
//! separate, unrelated concern and live in `src/github/*` -- not here.

pub mod branch;
pub mod shell;
pub mod worktree;
