use std::sync::Arc;

use anyhow::Result;
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router};
use serde_json::{Value, json};

use crate::tools::ToolRegistry;

#[derive(Clone)]
struct McpState {
    registry: Arc<ToolRegistry>,
    name: String,
    api_key: Option<String>,
}

fn rpc_ok(id: Value, result: Value) -> Value {
    json!({"jsonrpc":"2.0","id":id,"result":result})
}

fn rpc_err(id: Value, code: i64, msg: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":msg}})
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
            return (
                StatusCode::UNAUTHORIZED,
                Json(rpc_err(Value::Null, -32000, "Unauthorized")),
            )
                .into_response();
        }
    }
    next.run(req).await.into_response()
}

async fn handle_rpc(State(state): State<McpState>, Json(req): Json<Value>) -> impl IntoResponse {
    let id = req.get("id").cloned().unwrap_or(Value::Null);

    if req["jsonrpc"].as_str() != Some("2.0") {
        return (
            StatusCode::OK,
            Json(rpc_err(id, -32600, "Invalid JSON-RPC version")),
        );
    }

    let method = req["method"].as_str().unwrap_or("");
    let params = &req["params"];

    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": state.name, "version": env!("CARGO_PKG_VERSION") }
        })),
        "tools/list" => Ok(json!({ "tools": state.registry.tool_defs_mcp() })),
        "tools/call" => tools_call(&state.registry, params).await,
        _ => Err((-32601_i64, format!("Method not found: {method}"))),
    };

    let resp = match result {
        Ok(v) => rpc_ok(id, v),
        Err((code, msg)) => rpc_err(id, code, &msg),
    };
    (StatusCode::OK, Json(resp))
}

async fn tools_call(registry: &ToolRegistry, params: &Value) -> Result<Value, (i64, String)> {
    let name = params["name"]
        .as_str()
        .ok_or((-32602_i64, "Missing tool name".into()))?;
    let args = &params["arguments"];
    let args_json =
        serde_json::to_string(args).map_err(|e| (-32602_i64, format!("Invalid arguments: {e}")))?;
    let result = registry
        .execute(name, &args_json)
        .await
        .map_err(|e| (-32000_i64, e.to_string()))?;
    Ok(json!({ "content": [{ "type": "text", "text": result.for_llm }] }))
}
