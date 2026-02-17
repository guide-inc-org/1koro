use std::sync::Arc;

use anyhow::Result;
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::tools::ToolRegistry;

#[derive(Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<serde_json::Value>,
    method: String,
    #[serde(default)]
    params: serde_json::Value,
}

#[derive(Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Serialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

impl JsonRpcResponse {
    fn ok(id: serde_json::Value, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: Some(result),
            error: None,
        }
    }
    fn err(id: serde_json::Value, code: i64, msg: String) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(JsonRpcError { code, message: msg }),
        }
    }
}

#[derive(Clone)]
struct McpState {
    registry: Arc<ToolRegistry>,
    name: String,
    api_key: Option<String>,
}

pub async fn start(
    bind: &str,
    registry: Arc<ToolRegistry>,
    name: &str,
    api_key: Option<String>,
) -> Result<()> {
    let state = McpState {
        registry,
        name: name.to_string(),
        api_key,
    };
    let app = Router::new()
        .route("/mcp", post(handle_rpc))
        .layer(middleware::from_fn_with_state(state.clone(), auth_layer))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind(bind).await?;
    tracing::info!("MCP server listening on {bind}");
    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!("MCP server error: {e}");
        }
    });
    Ok(())
}

async fn auth_layer(State(state): State<McpState>, req: Request, next: Next) -> impl IntoResponse {
    if let Some(ref expected) = state.api_key {
        let auth_ok = req
            .headers()
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .is_some_and(|t| t == expected);
        if !auth_ok {
            let resp = JsonRpcResponse::err(serde_json::Value::Null, -32000, "Unauthorized".into());
            return (StatusCode::UNAUTHORIZED, Json(resp)).into_response();
        }
    }
    next.run(req).await.into_response()
}

async fn handle_rpc(
    State(state): State<McpState>,
    Json(req): Json<JsonRpcRequest>,
) -> impl IntoResponse {
    let id = req.id.unwrap_or(serde_json::Value::Null);
    if req.jsonrpc != "2.0" {
        return (
            StatusCode::OK,
            Json(JsonRpcResponse::err(
                id,
                -32600,
                "Invalid JSON-RPC version".into(),
            )),
        );
    }
    let result = match req.method.as_str() {
        "initialize" => Ok(serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": state.name, "version": env!("CARGO_PKG_VERSION") }
        })),
        "tools/list" => Ok(serde_json::json!({ "tools": tools_list(&state.registry) })),
        "tools/call" => tools_call(&state.registry, &req.params).await,
        _ => Err((-32601, format!("Method not found: {}", req.method))),
    };
    let resp = match result {
        Ok(v) => JsonRpcResponse::ok(id, v),
        Err((code, msg)) => JsonRpcResponse::err(id, code as i64, msg),
    };
    (StatusCode::OK, Json(resp))
}

/// Generate MCP tool list from ToolRegistry (single source of truth).
fn tools_list(registry: &ToolRegistry) -> serde_json::Value {
    let tools: Vec<serde_json::Value> = registry
        .tool_defs()
        .iter()
        .map(|td| {
            serde_json::json!({
                "name": td.function.name,
                "description": td.function.description,
                "inputSchema": td.function.parameters,
            })
        })
        .collect();
    serde_json::json!(tools)
}

/// Execute tool via ToolRegistry (same validation as agent tools).
async fn tools_call(
    registry: &ToolRegistry,
    params: &serde_json::Value,
) -> Result<serde_json::Value, (i32, String)> {
    let name = params["name"]
        .as_str()
        .ok_or((-32602, "Missing tool name".into()))?;
    let args = &params["arguments"];
    let args_json =
        serde_json::to_string(args).map_err(|e| (-32602, format!("Invalid arguments: {e}")))?;
    let result = registry
        .execute(name, &args_json)
        .await
        .map_err(|e| (-32000, e.to_string()))?;
    Ok(serde_json::json!({ "content": [{ "type": "text", "text": result.for_llm }] }))
}
