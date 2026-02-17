use std::sync::Arc;

use anyhow::Result;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::config::McpConfig;
use crate::memory::MemoryManager;

// --- JSON-RPC types ---

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
    fn success(id: serde_json::Value, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    fn error(id: serde_json::Value, code: i64, message: String) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError { code, message }),
        }
    }
}

#[derive(Clone)]
struct McpState {
    memory: Arc<MemoryManager>,
}

/// Start the MCP server on its own port.
pub async fn start(config: &McpConfig, memory: Arc<MemoryManager>) -> Result<()> {
    if !config.enabled {
        tracing::info!("MCP server disabled");
        return Ok(());
    }

    let state = McpState { memory };

    let app = Router::new()
        .route("/mcp", post(handle_rpc))
        .with_state(state);

    let bind = config.bind.clone();
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!("MCP server listening on {bind}");

    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!("MCP server error: {e}");
        }
    });

    Ok(())
}

async fn handle_rpc(
    State(state): State<McpState>,
    Json(req): Json<JsonRpcRequest>,
) -> impl IntoResponse {
    if req.jsonrpc != "2.0" {
        return (
            StatusCode::OK,
            Json(JsonRpcResponse::error(
                req.id.unwrap_or(serde_json::Value::Null),
                -32600,
                "Invalid JSON-RPC version".to_string(),
            )),
        );
    }

    let id = req.id.unwrap_or(serde_json::Value::Null);

    let result = match req.method.as_str() {
        "initialize" => handle_initialize(),
        "tools/list" => handle_tools_list(),
        "tools/call" => handle_tools_call(&state, &req.params).await,
        _ => Err((-32601, format!("Method not found: {}", req.method))),
    };

    let response = match result {
        Ok(val) => JsonRpcResponse::success(id, val),
        Err((code, msg)) => JsonRpcResponse::error(id, code as i64, msg),
    };

    (StatusCode::OK, Json(response))
}

fn handle_initialize() -> Result<serde_json::Value, (i32, String)> {
    Ok(serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {
            "tools": {}
        },
        "serverInfo": {
            "name": "1koro",
            "version": env!("CARGO_PKG_VERSION")
        }
    }))
}

fn handle_tools_list() -> Result<serde_json::Value, (i32, String)> {
    Ok(serde_json::json!({
        "tools": [
            {
                "name": "read_core_memory",
                "description": "Read a core memory file (identity.md, user.md, or state.md)",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "file": {
                            "type": "string",
                            "enum": ["identity.md", "user.md", "state.md"]
                        }
                    },
                    "required": ["file"]
                }
            },
            {
                "name": "update_core_memory",
                "description": "Update user.md or state.md",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "file": { "type": "string", "enum": ["user.md", "state.md"] },
                        "content": { "type": "string" }
                    },
                    "required": ["file", "content"]
                }
            },
            {
                "name": "search_logs",
                "description": "Search past logs for a keyword",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string" }
                    },
                    "required": ["query"]
                }
            },
            {
                "name": "read_daily_log",
                "description": "Read a daily log by date (YYYY-MM-DD)",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "date": { "type": "string" }
                    },
                    "required": ["date"]
                }
            }
        ]
    }))
}

async fn handle_tools_call(
    state: &McpState,
    params: &serde_json::Value,
) -> Result<serde_json::Value, (i32, String)> {
    let tool_name = params["name"]
        .as_str()
        .ok_or((-32602, "Missing tool name".to_string()))?;
    let args = &params["arguments"];

    let result = match tool_name {
        "read_core_memory" => {
            let file = args["file"].as_str().unwrap_or("state.md");
            state
                .memory
                .read_core(file)
                .map_err(|e| (-32000, e.to_string()))?
        }
        "update_core_memory" => {
            let file = args["file"].as_str().unwrap_or("state.md");
            if file == "identity.md" {
                return Err((-32000, "identity.md is read-only".to_string()));
            }
            let content = args["content"].as_str().unwrap_or("");
            state
                .memory
                .write_core(file, content)
                .map_err(|e| (-32000, e.to_string()))?;
            format!("Updated {file}")
        }
        "search_logs" => {
            let query = args["query"].as_str().unwrap_or("");
            let results = state
                .memory
                .search_logs(query)
                .map_err(|e| (-32000, e.to_string()))?;
            if results.is_empty() {
                "No results found.".to_string()
            } else {
                results.join("\n")
            }
        }
        "read_daily_log" => {
            let date = args["date"].as_str().unwrap_or("");
            state
                .memory
                .read_daily_log(date)
                .map_err(|e| (-32000, e.to_string()))?
                .unwrap_or_else(|| format!("No log found for {date}"))
        }
        _ => return Err((-32602, format!("Unknown tool: {tool_name}"))),
    };

    Ok(serde_json::json!({
        "content": [{ "type": "text", "text": result }]
    }))
}
