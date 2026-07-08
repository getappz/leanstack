//! Ties the SQLite manifest, BM25 search, config-driven backends, and
//! debounced refresh together — the one type `src/mcp_server.rs` talks to.

use crate::backend::{Backend, HttpApiBackend};
use crate::config::{GatewayConfig, ServerConfig};
use crate::db::{self, ServerTools};
use crate::error::{suggest, GatewayError};
use crate::mcp_stdio::McpStdioBackend;
use crate::search::{search, MatchMode, ToolHit};
use rusqlite::Connection;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, Instant};

const REFRESH_DEBOUNCE: Duration = Duration::from_secs(60);

pub struct Registry {
    conn: Connection,
    backends: HashMap<String, Backend>,
    last_refresh: Option<Instant>,
}

impl Registry {
    pub async fn open_default(
        db_path: &Path,
        config: &GatewayConfig,
        secrets: &HashMap<String, String>,
    ) -> Result<Self, GatewayError> {
        let conn = db::open_db(db_path)?;
        let backends = build_backends(config, secrets);
        let mut reg = Self { conn, backends, last_refresh: None };
        reg.ensure_fresh().await?;
        Ok(reg)
    }

    /// In-memory registry: no on-disk manifest, no debounce skip needed since
    /// each caller builds a fresh `Registry`. Not test-gated — this is a
    /// legitimate constructor for embedded/ephemeral use (and it's what lets
    /// `tests/registry.rs`, a separate integration-test crate, exercise
    /// `Registry` end-to-end without a `#[cfg(test)]` visibility problem: a
    /// `#[cfg(test)]` item is only visible within this crate's own test
    /// build, not from an external `tests/*.rs` binary). Mirrors
    /// `db::open_in_memory`, which is likewise a plain public function.
    pub async fn open_in_memory(
        config: &GatewayConfig,
        secrets: &HashMap<String, String>,
    ) -> Result<Self, GatewayError> {
        let conn = db::open_in_memory()?;
        let backends = build_backends(config, secrets);
        let mut reg = Self { conn, backends, last_refresh: None };
        reg.ensure_fresh().await?;
        Ok(reg)
    }

    pub async fn ensure_fresh(&mut self) -> Result<(), GatewayError> {
        if let Some(t) = self.last_refresh {
            if t.elapsed() < REFRESH_DEBOUNCE {
                return Ok(());
            }
        }
        let mut entries = Vec::new();
        for (name, backend) in &self.backends {
            let tools = backend.discover().await?;
            entries.push(ServerTools { server: name.clone(), tools });
        }
        db::rebuild(&mut self.conn, &entries)?;
        self.last_refresh = Some(Instant::now());
        Ok(())
    }

    pub fn search(&self, query: &str, limit: usize, mode: MatchMode) -> Result<Vec<ToolHit>, GatewayError> {
        Ok(search(&self.conn, query, limit, mode)?)
    }

    pub async fn execute(&self, server: &str, tool: &str, args: Value) -> Result<Value, GatewayError> {
        let backend = match self.backends.get(server) {
            Some(b) => b,
            None => {
                let candidates: Vec<String> = self.backends.keys().cloned().collect();
                let msg = match suggest(server, &candidates) {
                    Some(s) => format!("server '{server}' not found — did you mean '{s}'?"),
                    None => format!("server '{server}' not found"),
                };
                return Err(GatewayError::ServerNotFound(msg));
            }
        };
        let known_tools = db::tool_names(&self.conn, server)?;
        if !known_tools.is_empty() && !known_tools.contains(&tool.to_string()) {
            let msg = match suggest(tool, &known_tools) {
                Some(s) => format!("tool '{tool}' not found on server '{server}' — did you mean '{s}'?"),
                None => format!("tool '{tool}' not found on server '{server}'"),
            };
            return Err(GatewayError::ToolNotFound(msg));
        }
        backend.call(tool, args).await
    }
}

fn build_backends(config: &GatewayConfig, secrets: &HashMap<String, String>) -> HashMap<String, Backend> {
    let mut out = HashMap::new();
    for (name, server_config) in &config.servers {
        let backend = match server_config {
            ServerConfig::McpStdio { command, args, auth_ref, auth_env } => {
                let mut env = HashMap::new();
                if let (Some(auth_ref), Some(auth_env)) = (auth_ref, auth_env) {
                    if let Some(secret) = secrets.get(auth_ref) {
                        env.insert(auth_env.clone(), secret.clone());
                    }
                }
                Backend::McpStdio(McpStdioBackend::new(command.clone(), args.clone(), env))
            }
            ServerConfig::HttpApi { base_url, tools, .. } => {
                Backend::HttpApi(HttpApiBackend { base_url: base_url.clone(), tools: tools.clone() })
            }
        };
        out.insert(name.clone(), backend);
    }
    out
}

// NOTE: the four Registry integration tests (search/execute against the real
// `gateway-fixture-server` binary) originally planned as a `#[cfg(test)] mod
// tests` here had to move to `tests/registry.rs` instead, for the same
// reason `mcp_stdio.rs`'s and the discover/call backend tests moved (Tasks 6
// and 7): `env!("CARGO_BIN_EXE_gateway-fixture-server")` is only populated by
// Cargo for integration-test/bench targets, not for the lib's own unit-test
// binary. See `tests/registry.rs`.
