mod heuristic;
mod providers;
mod shape_xlat;
mod think;

pub use providers::{ProviderConfig, ProviderKind};
use axum::Router;

pub fn router() -> Router {
    Router::new().route("/proxy/v1/messages", axum::routing::post(v1_messages_handler))
}

async fn v1_messages_handler(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::extract::Json(body): axum::extract::Json<serde_json::Value>,
) -> axum::response::Response {
    // 1. Translate Anthropic request → OpenAI
    // 2. Select provider
    // 3. Forward
    // 4. Translate response back → Anthropic
    todo!()
}

#[derive(Clone)]
struct AppState {
    config: ProviderConfig,
}
