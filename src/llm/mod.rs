pub mod openai_compatible;

use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::config::LlmConfig;

// --- Message ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".into(),
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".into(),
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn assistant_with_tool_calls(
        content: Option<String>,
        tool_calls: Vec<ToolCall>,
    ) -> Self {
        Self {
            role: "assistant".into(),
            content,
            tool_calls: Some(tool_calls),
            tool_call_id: None,
        }
    }

    pub fn tool_result(
        tool_call_id: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            role: "tool".into(),
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: Some(tool_call_id.into()),
        }
    }
}

// --- Tool definitions ---

#[derive(Debug, Clone, Serialize)]
pub struct ToolDef {
    #[serde(rename = "type")]
    pub type_: String,
    pub function: FunctionDef,
}

#[derive(Debug, Clone, Serialize)]
pub struct FunctionDef {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

// --- Tool calls ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub function: FunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

// --- LLM Response ---

pub struct LlmResponse {
    pub content: Option<String>,
    pub tool_calls: Vec<ToolCall>,
}

// --- LLM Client trait (keep for future Bedrock support) ---

#[async_trait::async_trait]
pub trait LlmClient: Send + Sync {
    async fn chat(
        &self,
        messages: Vec<Message>,
        tools: Option<&[ToolDef]>,
    ) -> Result<LlmResponse>;
}

// --- Factory ---

pub fn create_client(config: &LlmConfig) -> Result<Arc<dyn LlmClient>> {
    let base_url = config
        .base_url
        .clone()
        .unwrap_or_else(|| default_base_url(&config.provider).to_string());
    Ok(Arc::new(openai_compatible::OpenAICompatibleClient::new(
        config, &base_url,
    )?))
}

fn default_base_url(provider: &str) -> &str {
    match provider {
        "openai" => "https://api.openai.com/v1",
        "openrouter" => "https://openrouter.ai/api/v1",
        "google" => "https://generativelanguage.googleapis.com/v1beta/openai",
        "groq" => "https://api.groq.com/openai/v1",
        "together" => "https://api.together.xyz/v1",
        "deepseek" => "https://api.deepseek.com/v1",
        _ => "https://openrouter.ai/api/v1",
    }
}
