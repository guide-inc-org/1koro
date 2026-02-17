use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::agent::Agent;

#[derive(Clone)]
pub struct AppState {
    pub agent: Arc<Mutex<Agent>>,
}

#[derive(Deserialize)]
pub struct MessageRequest {
    pub text: String,
    #[serde(default = "default_channel")]
    pub channel: String,
    #[serde(default = "default_user")]
    pub user: String,
}

fn default_channel() -> String {
    "cli".to_string()
}

fn default_user() -> String {
    "masaki".to_string()
}

#[derive(Serialize)]
pub struct MessageResponse {
    pub text: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<serde_json::Value>,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/message", post(handle_message))
        .route("/health", get(handle_health))
        .with_state(state)
}

async fn handle_message(
    State(state): State<AppState>,
    Json(req): Json<MessageRequest>,
) -> impl IntoResponse {
    tracing::info!("[{}:{}] {}", req.channel, req.user, req.text);

    let mut agent = state.agent.lock().await;
    match agent
        .handle_message(&req.text, &req.channel, &req.user)
        .await
    {
        Ok(resp) => (
            StatusCode::OK,
            Json(MessageResponse {
                text: resp.text.unwrap_or_else(|| "(no response)".to_string()),
                actions: resp.actions,
            }),
        ),
        Err(e) => {
            tracing::error!("Agent error: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(MessageResponse {
                    text: format!("Error: {e}"),
                    actions: vec![],
                }),
            )
        }
    }
}

async fn handle_health() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}
