# Proxy Server Spec: Embedded Anthropic→OpenAI Translation + Multi-Backend Failover

## Overview

Embed a Rust HTTP proxy within the agentflare binary that translates
Anthropic Messages API (used by Claude Code) to OpenAI Chat Completions
API, supporting free/cheap inference backends with automatic failover.

## Architecture

```
Claude Code  ──POST /v1/messages──►  agentflare proxy  ──POST /v1/chat/completions──►  OpenRouter
  (Anthropic)                           localhost:3000          │                        (OpenAI-compat)
                                       │                        ├──► NVIDIA NIM (fallback 1)
                                       │                        └──► LM Studio (fallback 2)
                                       │
                                    ──env vars──►  ~/.agentflare/proxy.toml
                                       │
                                    ──config──►  ~/.claude/settings.json (auto-wired)
```

### Components

| Component | File | Status |
|---|---|---|
| Config struct + TOML parsing | `src/config.rs` | DONE |
| Anthropic→OpenAI translate | `src/translate.rs` | DONE |
| Backend abstraction + failover | `src/backend.rs` | TODO |
| HTTP server (TcpListener) | `src/server.rs` | TODO |
| SSE stream translation | `src/stream.rs` | TODO |
| CLI subcommand | `src/cli/proxy.rs` | TODO |
| Wire into workspace | `Cargo.toml` + `cli/mod.rs` | TODO |

## Translation Layer (done)

### Request: Anthropic Messages → OpenAI Chat Completions

| Anthropic field | OpenAI field | Notes |
|---|---|---|
| `model` | `model` | remapped via `model_map` |
| `max_tokens` | `max_tokens` | |
| `stream` | `stream` | same |
| `temperature/top_p/top_k` | `temperature/top_p` | top_k filtered (OpenAI has none) |
| `stop_sequences` | `stop` | |
| `system` (string or array) | `messages[0].role=system` | flattened to one system msg |
| `messages[].role=user` | `messages[].role=user` | |
| `messages[].role=assistant` | `messages[].role=assistant` | |
| `content[] {type:text}` | `content[] {type:text}` | |
| `content[] {type:image}` | `content[] {type:image_url}` | base64 inline |
| `content[] {type:tool_use}` | `tool_calls[]` | function-call format |
| `content[] {type:tool_result}` | separate `role:tool` message | by tool_call_id |
| `thinking {type:enabled}` | `reasoning_effort:medium` | |
| `thinking {type:enabled,budget_tokens:N}` | `max_completion_tokens:N` | |

### Response: OpenAI Chat Completions → Anthropic Messages

- Non-streaming: `openai_to_anthropic()` converts choice → message
- Streaming (SSE): `translate_stream_event()` maps each SSE event:
  - `data: {"choices":[{"delta":{"content":"..."}}]}` → `content_block_delta`
  - `data: {"choices":[{"delta":{"tool_calls":[...]}}]}` → `content_block_start` (tool_use)
  - `data: {"choices":[{"delta":{"reasoning_content":"..."}}]}` → `thinking_delta`
  - `data: {"choices":[{"finish_reason":"stop"}]}` → `content_block_stop` + `message_delta`
  - `data: [DONE]` → `message_stop`

### Decisions
- No `tool_choice` forwarding (always `auto`) — matches anthropic-proxy-rs
- No `metadata/service_tier` forwarding
- Thinking: map to `reasoning_effort` + `max_completion_tokens`
- API key: read from incoming `x-api-key` header (passthrough) or from config

## Backend Abstraction + Failover (todo)

### Backend Trait

```rust
trait Backend {
    fn name(&self) -> &str;
    fn base_url(&self) -> &str;
    fn api_key(&self) -> Option<&str>;
}
enum BackendKind {
    OpenRouter { api_key: String, model: String },
    NvidiaNim { api_key: Option<String>, model: String },
    LmStudio { base_url: String, model: String },
    Custom { base_url: String, api_key: Option<String>, kind: Option<String> },
}
```

### Failover Chain

```rust
struct FailoverChain {
    backends: Vec<(BackendKind, BackendState)>,
    current_index: AtomicUsize,
}
```

1. Try `backends[current_index]` with `ureq::post`
2. On HTTP 429/5xx/network error → increment index, retry
3. On HTTP 4xx (non-429) → return error (don't failover auth errors)
4. After all backends exhausted → return last error
5. On success → reset index to 0

### Config File: `~/.agentflare/proxy.toml`

```toml
bind = "127.0.0.1"
port = 3000

[[providers]]
name = "openrouter"
base_url = "https://openrouter.ai/api"
api_key = "sk-or-..."
model = "openai/gpt-4o"

[[providers]]
name = "nvidia-nim"
kind = "nvidia_nim"
base_url = "https://integrate.api.nvidia.com"
api_key = "nvapi-..."
model = "meta/llama-3.1-70b-instruct"

[[providers]]
name = "local"
kind = "openai_compat"
base_url = "http://localhost:1234/v1"
model = "local-model"

[model_map]
"claude-sonnet-4-20250514" = "openai/gpt-4o"
"claude-haiku-3-5" = "openai/gpt-4o-mini"
```

## HTTP Server (todo)

- `std::net::TcpListener` on `bind:port` (no HTTP framework deps)
- Minimal HTTP parser (read method, path, headers, Content-Length body)
- Route table:
  - `POST /v1/messages` → translate → forward → translate back → stream back
  - `POST /v1/messages/count_tokens` → local estimate → JSON response
  - `GET /healthz` → 200 OK
  - `GET /v1/models` → list mapped models from config
- SSE streaming: `Transfer-Encoding: chunked`, `Content-Type: text/event-stream`
- One thread per connection (std::thread)
- Graceful shutdown via SIGINT/SIGTERM

## CLI Subcommand (todo)

```
agentflare proxy start   # start proxy daemon
agentflare proxy stop    # stop via PID file
agentflare proxy status  # check if running
agentflare proxy config  # edit proxy.toml via $EDITOR
```

- `start` reads `~/.agentflare/proxy.toml`, binds, prints "Proxy running on :3000"
- `start --daemon` forks to background, writes PID file
- Auto-wire: `agentflare proxy start` also writes `ANTHROPIC_BASE_URL` to
  `~/.claude/settings.json` (idempotent, `agentflare proxy stop` restores)

## File Layout

```
crates/proxy-server/
├── Cargo.toml
└── src/
    ├── lib.rs       # re-exports
    ├── config.rs    # ProxyConfig, ProviderConfig, NamedProvider [DONE]
    ├── translate.rs # anthropic_to_openai, openai_to_anthropic, stream events [DONE]
    ├── backend.rs   # Backend trait, BackendKind variants, ureq calls
    ├── failover.rs  # FailoverChain: ordered retry across backends
    ├── server.rs    # TcpListener, request routing, SSE streaming
    └── stream.rs    # SSE event translation loop

src/cli/proxy.rs     # CLI subcommand
```

## Dependencies

No new external deps needed beyond what's already in the workspace:
- `serde` / `serde_json` — JSON
- `ureq` — HTTP client to upstream
- `thiserror` — error types
- `std::net::TcpListener` — HTTP server (stdlib)
- `std::thread` — connection handling

## Open Questions for Claude

1. Should the server use `tokio` async or `std::thread` per connection?
   Current choice: `std::thread` (simpler, no new deps). Trade-off: less
   scalable under high concurrency. Acceptable for local-only proxy.

2. API key handling: passthrough from incoming `x-api-key` header per
   request, or use config `api_key` per provider? Current: both — use
   header if present, fall back to config key.

3. Model mapping: should `ANTHROPIC_MODEL` from the client be mapped
   automatically, or require explicit entries in `[model_map]`? Current:
   explicit map entries; unknown models passed through verbatim.

4. Daemon mode: write PID file at `~/.agentflare/proxy.pid` and fork?
   On Windows, no fork — use a background job object instead?

5. Should `agentflare proxy start` auto-write Claude Code settings
   (`~/.claude/settings.json`), or just print the env vars? Current:
   auto-write with `agentflare proxy stop` restoring original.
