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
    pub name: String,
}

#[derive(Deserialize)]
pub struct MessageRequest {
    pub text: String,
    #[serde(default = "default_channel")]
    pub channel: String,
    #[serde(default = "default_user")]
    pub user: String,
}

fn default_channel() -> String { "cli".into() }
fn default_user() -> String { "masaki".into() }

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

async fn handle_message(State(state): State<AppState>, Json(req): Json<MessageRequest>) -> impl IntoResponse {
    let mut agent = state.agent.lock().await;
    match agent.handle_message(&req.text, &req.channel, &req.user).await {
        Ok(resp) => (StatusCode::OK, Json(MessageResponse {
            text: resp.text.unwrap_or_else(|| "(no response)".into()),
            actions: resp.actions,
        })),
        Err(e) => {
            tracing::error!("Agent error: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(MessageResponse {
                text: format!("Error: {e}"),
                actions: vec![],
            }))
        }
    }
}

async fn handle_health(State(state): State<AppState>) -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "name": state.name,
        "version": env!("CARGO_PKG_VERSION"),
    }))
}
