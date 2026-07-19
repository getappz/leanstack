mod forward;
pub mod heuristic;
pub mod providers;
pub mod shape_xlat;
pub mod think;

pub use providers::ProviderConfig;
use axum::{Router, extract::State, response::Response, routing::post};

pub fn router() -> Router {
    Router::new()
        .route("/proxy/v1/messages", post(v1_messages_handler))
        .with_state(AppState {
            config: ProviderConfig::default_free(),
        })
}

async fn v1_messages_handler(
    State(state): State<AppState>,
    axum::extract::Json(body): axum::extract::Json<serde_json::Value>,
) -> Response {
    forward::proxy_request(body, &state.config).await
}

#[derive(Clone)]
struct AppState {
    config: ProviderConfig,
}
