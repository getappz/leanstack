mod forward;
pub mod heuristic;
pub mod providers;
pub mod shape_xlat;
pub mod think;

use axum::{extract::State, response::Response, routing::post, Router};
pub use providers::ProviderConfig;

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
