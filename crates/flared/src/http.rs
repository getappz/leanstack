//! Localhost HTTP surface. Never binds beyond 127.0.0.1.

use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Path as AxumPath, Query, State};
use axum::http::StatusCode;
use axum::response::Html;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Deserialize;

use crate::config::Config;
use crate::daemon::{kill_process_tree, sweep_once, unix_now, SharedSnapshot, SweepOutcome};
use crate::events::EventLog;
use crate::leases::LeaseStore;
use crate::model::{Finding, Identity, Lease, ProcInfo};
use crate::scanner::identity_of;

type HttpError = (StatusCode, String);

fn internal(err: impl std::fmt::Display) -> HttpError {
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}

#[derive(Clone)]
pub struct AppState {
    pub cfg: Arc<Config>,
    pub snapshot: SharedSnapshot,
    pub store: Arc<LeaseStore>,
    pub log: Arc<EventLog>,
}

impl AppState {
    pub fn new(cfg: Arc<Config>, state_dir: PathBuf) -> Self {
        Self {
            cfg,
            snapshot: SharedSnapshot::default(),
            store: Arc::new(LeaseStore::new(&state_dir)),
            log: Arc::new(EventLog::new(&state_dir)),
        }
    }
}

#[derive(Deserialize)]
pub struct LeaseRequest {
    pub pid: u32,
    #[serde(default = "default_class")]
    pub class: String,
    pub ttl_seconds: u64,
    #[serde(default)]
    pub allow_kill: bool,
}

fn default_class() -> String {
    "agent".into()
}

#[derive(Deserialize)]
pub struct EventsQuery {
    #[serde(default = "default_events_n")]
    pub n: usize,
}

fn default_events_n() -> usize {
    50
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(dashboard))
        .route("/status", get(status))
        .route("/processes", get(processes))
        .route("/orphans", get(orphans))
        .route("/events", get(events))
        .route("/leases", get(leases_list))
        .route("/lease", post(lease_create))
        .route("/lease/{id}/heartbeat", post(lease_heartbeat))
        .route("/lease/{id}", delete(lease_delete))
        .route("/sweep", post(sweep_now))
        .route("/clean", post(clean_dry_run))
        .route("/clean/execute", post(clean_execute))
        .with_state(state)
}

async fn dashboard() -> Html<&'static str> {
    Html(DASHBOARD)
}

async fn status(State(s): State<AppState>) -> Json<serde_json::Value> {
    let last = s.snapshot.lock().expect("snapshot lock").last.clone();
    let lease_count = s.store.load().map(|l| l.len()).unwrap_or(0);
    Json(serde_json::json!({
        "last": last,
        "lease_count": lease_count,
        "port": s.cfg.port,
    }))
}

async fn processes(State(s): State<AppState>) -> Json<Vec<ProcInfo>> {
    Json(s.snapshot.lock().expect("snapshot lock").processes.clone())
}

async fn orphans(State(s): State<AppState>) -> Json<Vec<Finding>> {
    let snap = s.snapshot.lock().expect("snapshot lock");
    Json(snap.last.as_ref().map(|l| l.orphans.clone()).unwrap_or_default())
}

async fn events(
    State(s): State<AppState>,
    Query(q): Query<EventsQuery>,
) -> Result<Json<Vec<serde_json::Value>>, HttpError> {
    s.log.tail(q.n).map(Json).map_err(internal)
}

async fn leases_list(State(s): State<AppState>) -> Result<Json<Vec<Lease>>, HttpError> {
    s.store.load().map(Json).map_err(internal)
}

async fn lease_create(
    State(s): State<AppState>,
    Json(req): Json<LeaseRequest>,
) -> Result<Json<Lease>, HttpError> {
    let pid = req.pid;
    let identity = tokio::task::spawn_blocking(move || identity_of(pid))
        .await
        .map_err(internal)?
        .ok_or((StatusCode::NOT_FOUND, format!("pid {pid} is not running")))?;
    let lease = s
        .store
        .create(
            pid,
            &req.class,
            req.ttl_seconds,
            Identity { exe_name: identity.0, start_time: identity.1 },
            req.allow_kill,
            unix_now(),
        )
        .map_err(internal)?;
    let _ = s.log.append(
        "lease.create",
        serde_json::json!({ "id": lease.id, "pid": lease.pid, "class": lease.class }),
    );
    Ok(Json(lease))
}

async fn lease_heartbeat(
    State(s): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<Lease>, HttpError> {
    match s.store.heartbeat(&id, unix_now()).map_err(internal)? {
        Some(lease) => Ok(Json(lease)),
        None => Err((StatusCode::NOT_FOUND, format!("unknown lease '{id}'"))),
    }
}

async fn lease_delete(
    State(s): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<serde_json::Value>, HttpError> {
    let removed = s.store.remove(&id).map_err(internal)?;
    let _ = s.log.append("lease.delete", serde_json::json!({ "id": id, "removed": removed }));
    Ok(Json(serde_json::json!({ "removed": removed })))
}

async fn run_sweep(s: AppState, execute: bool) -> Result<Json<SweepOutcome>, HttpError> {
    tokio::task::spawn_blocking(move || {
        sweep_once(&s.cfg, &s.store, &s.log, execute, true, &mut kill_process_tree)
    })
    .await
    .map_err(internal)?
    .map(Json)
    .map_err(internal)
}

async fn sweep_now(State(s): State<AppState>) -> Result<Json<SweepOutcome>, HttpError> {
    run_sweep(s, true).await
}

async fn clean_dry_run(State(s): State<AppState>) -> Result<Json<SweepOutcome>, HttpError> {
    run_sweep(s, false).await
}

async fn clean_execute(State(s): State<AppState>) -> Result<Json<SweepOutcome>, HttpError> {
    run_sweep(s, true).await
}

const DASHBOARD: &str = r#"<!doctype html>
<html><head><meta charset="utf-8"><title>flared</title>
<style>
body{font-family:ui-monospace,monospace;margin:2rem;background:#111;color:#ddd}
h1{color:#f80}table{border-collapse:collapse;margin-top:1rem}
td,th{padding:.25rem .75rem;border-bottom:1px solid #333;text-align:left}
.green{color:#6c6}.yellow{color:#cc6}.red{color:#c66}
</style></head><body>
<h1>flared</h1>
<div id="pressure">loading…</div>
<h2>workloads</h2><table id="buckets"></table>
<h2>orphan findings</h2><table id="orphans"></table>
<script>
async function tick(){
  const s = await (await fetch('/status')).json();
  const last = s.last;
  if(!last){document.getElementById('pressure').textContent='no sweep yet';return}
  const p = last.pressure;
  document.getElementById('pressure').innerHTML =
    `pressure: <span class="${p.level}">${p.level}</span>` +
    ` · cpu ${p.cpu_pct.toFixed(0)}%` +
    ` · mem free ${(p.avail_mem_bytes/2**30).toFixed(1)} GiB` +
    ` · leases ${s.lease_count}`;
  document.getElementById('buckets').innerHTML =
    Object.entries(last.bucket_counts).sort((a,b)=>b[1]-a[1])
      .map(([k,v])=>`<tr><td>${k}</td><td>${v}</td></tr>`).join('');
  const orphans = await (await fetch('/orphans')).json();
  document.getElementById('orphans').innerHTML =
    orphans.length ? orphans.map(o=>`<tr><td>${o.pid}</td><td>${o.name}</td><td>${o.reason}</td></tr>`).join('')
                   : '<tr><td>none</td></tr>';
}
tick(); setInterval(tick, 15000);
</script></body></html>"#;

/// Serve until aborted. Binds 127.0.0.1 only.
pub async fn serve(state: AppState) -> eyre::Result<()> {
    let port = state.cfg.port;
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
    tracing::info!(%port, "flared http listening on 127.0.0.1");
    axum::serve(listener, router(state)).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn state() -> (tempfile::TempDir, AppState) {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(Arc::new(Config::default()), dir.path().to_path_buf());
        (dir, state)
    }

    async fn body_json(response: axum::response::Response) -> serde_json::Value {
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn status_serves_snapshot_json() {
        let (_dir, state) = state();
        let app = router(state);
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/status")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let json = body_json(response).await;
        assert!(json.get("last").is_some());
        assert!(json.get("lease_count").is_some());
    }

    #[tokio::test]
    async fn dashboard_serves_html() {
        let (_dir, state) = state();
        let app = router(state);
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn lease_lifecycle_over_http() {
        let (_dir, state) = state();
        let app = router(state.clone());

        // Register a lease for OUR OWN live process — identity is resolvable.
        let me = std::process::id();
        let response = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/lease")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::json!({ "pid": me, "ttl_seconds": 300 }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let lease = body_json(response).await;
        let id = lease["id"].as_str().unwrap().to_string();
        assert_eq!(lease["pid"].as_u64().unwrap() as u32, me);
        assert!(!lease["identity"]["exe_name"].as_str().unwrap().is_empty());

        // Heartbeat.
        let response = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri(format!("/lease/{id}/heartbeat"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Delete.
        let response = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .method("DELETE")
                    .uri(format!("/lease/{id}"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(state.store.load().unwrap().is_empty());
    }

    #[tokio::test]
    async fn lease_for_dead_pid_is_rejected() {
        let (_dir, state) = state();
        let app = router(state);
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/lease")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::json!({ "pid": u32::MAX - 7, "ttl_seconds": 300 })
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
