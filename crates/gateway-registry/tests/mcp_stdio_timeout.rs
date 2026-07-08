//! Integration test proving `McpStdioBackend` bounds downstream I/O with a
//! timeout instead of hanging forever (final-review Finding 1: no timeout
//! anywhere in this crate meant one hung backend could wedge the whole
//! `Registry`, since `ensure_gateway_registry` holds a single lock across
//! every backend's calls).
//!
//! Uses the `gateway-fixture-server` fixture's "hang" mode
//! (`GATEWAY_FIXTURE_HANG=1`) to simulate an unresponsive downstream server:
//! the fixture never completes the MCP `initialize` handshake, so both
//! `discover()` and `call()` block on `ensure_connected()` until our
//! short test-only timeout fires — proving the mechanism without waiting out
//! the crate's real 30s `DEFAULT_TIMEOUT`.

use gateway_registry::{GatewayError, McpStdioBackend};
use std::collections::HashMap;
use std::time::Duration;

fn hung_backend(timeout: Duration) -> McpStdioBackend {
    let fixture = env!("CARGO_BIN_EXE_gateway-fixture-server").to_string();
    let mut env = HashMap::new();
    env.insert("GATEWAY_FIXTURE_HANG".to_string(), "1".to_string());
    McpStdioBackend::with_timeout(fixture, vec![], env, timeout)
}

#[tokio::test]
async fn discover_against_a_hung_backend_times_out_instead_of_hanging_forever() {
    let backend = hung_backend(Duration::from_secs(1));
    let err = backend.discover().await.unwrap_err();
    assert!(matches!(err, GatewayError::Timeout(_)), "expected Timeout, got {err:?}");
}

#[tokio::test]
async fn call_against_a_hung_backend_times_out_instead_of_hanging_forever() {
    let backend = hung_backend(Duration::from_secs(1));
    let err = backend.call("echo", serde_json::json!({"text": "hi"})).await.unwrap_err();
    assert!(matches!(err, GatewayError::Timeout(_)), "expected Timeout, got {err:?}");
}
