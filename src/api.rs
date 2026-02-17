use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::agent::Agent;

#[derive(Clone)]
pub struct AppState {
    pub agent: Arc<Agent>,
    pub name: String,
    pub api_key: Option<String>,
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
        .layer(middleware::from_fn_with_state(state.clone(), auth_layer))
        .with_state(state)
}

async fn auth_layer(State(state): State<AppState>, req: Request, next: Next) -> impl IntoResponse {
    if let Some(ref expected) = state.api_key {
        if req.uri().path() != "/health" {
            let auth_ok = req.headers()
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.strip_prefix("Bearer "))
                .is_some_and(|t| t == expected);
            if !auth_ok {
                return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "Unauthorized"}))).into_response();
            }
        }
    }
    next.run(req).await.into_response()
}

async fn handle_message(State(state): State<AppState>, Json(req): Json<MessageRequest>) -> impl IntoResponse {
    match state.agent.handle_message(&req.text, &req.channel, &req.user).await {
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
