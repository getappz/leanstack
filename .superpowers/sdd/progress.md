Plan: docs/superpowers/plans/2026-07-08-gateway-search-execute.md
Branch: worktree-gateway-search-execute

(Previous content here was stale leftover from an unrelated, already-merged
feature (ponytail L1 integration, commit ef8d5e7) — reset before starting
this plan's execution.)

Task 1: complete (68cd5dd..df08c42, review approved — one Important finding
resolved by controller as a false positive: brief's "Interfaces" line used
gateway_registry::db:: as a fully-qualified-path label, not a public-API
contract; db stays a private mod, consumed intra-crate by later tasks, matching
the plan's own later lib.rs revisions)

Task 2: complete (df08c42..c3b3442, review approved, no Critical/Important.
Minor (plan-inherited, for final-review triage): search.rs FTS-operator test
only asserts no-error not no-false-match; schema_json parse failure silently
maps to Value::Null instead of surfacing an error.)

Task 3: complete (c3b3442..518c907, review approved, no Critical/Important.
IDE dead_code/"not in module tree" flags confirmed stale false alarms — mod
is wired in, pub-reachable items aren't flagged by lib-crate dead_code lint.
Minor (plan-inherited, for final-review triage): no test at exact distance=3
threshold boundary or distance=0 via suggest(); no Display/to_string test for
GatewayError variants.)

Task 4: complete (518c907..7b13955, review approved, no Critical/Important.
Minor (plan-inherited): HttpApi's optional fields (auth_ref/tools) not tested
in the omitted/default case, only McpStdio's defaults are covered.)

Task 5: complete (7b13955..0702b74, review approved, no Critical/Important/
Minor issues. No-McpStdio-variant constraint correctly respected.)

Task 6: complete (0702b74..b7a40c1, two commits: 49909e5 feat + b7a40c1 fix,
both reviewed and approved). Highest-risk task in the plan — real rmcp client
transport, verified by reviewers actually running the child-process-spawning
integration tests, not just trusting reports. Deviations from original brief
(all compiler-driven, vetted as correct):
  - discover() integration tests relocated to tests/mcp_stdio_discover.rs
    (CARGO_BIN_EXE_gateway-fixture-server is only set for integration-test/
    bench targets, not src/ unit tests) — same assertions, just relocated.
  - fixture_server.rs uses #[tokio::main(flavor = "current_thread")] (crate's
    tokio dep only enables "rt", not "rt-multi-thread") — matches src/main.rs's
    own Commands::Mcp runtime pattern.
  - Cargo.toml: rmcp (client+transport-child-process+server+transport-io+
    macros) and schemars moved from [dev-dependencies] to [dependencies] —
    the [[bin]] gateway-fixture-server target needs them and Cargo never
    exposes dev-dependencies to bin targets; previously only "worked" via
    accidental workspace-wide feature unification with the root crate's own
    rmcp dependency. tempfile correctly stays in [dev-dependencies] (only
    used in #[cfg(test)] code).
Task 7: complete (McpStdioBackend::call() implemented over the real rmcp
client — CallToolRequestParams::new + .call_tool(), is_error/structured_content
handling, per the brief's Step 3 unchanged). Deviation (as directed, same
CARGO_BIN_EXE_gateway-fixture-server reason as Task 6): the two new call()
tests went into a new tests/mcp_stdio_call.rs integration-test file instead of
inline in src/mcp_stdio.rs's unit-test module. Both tests pass against the
real spawned fixture-server process; full crate suite (24 tests across lib +
2 integration files) green; `cargo build -p gateway-registry` standalone
still succeeds.
