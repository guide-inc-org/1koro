use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::Client;
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
    pub fn assistant_with_tool_calls(content: Option<String>, tool_calls: Vec<ToolCall>) -> Self {
        Self {
            role: "assistant".into(),
            content,
            tool_calls: Some(tool_calls),
            tool_call_id: None,
        }
    }
    pub fn tool_result(id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: "tool".into(),
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: Some(id.into()),
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

// --- Trait ---

#[async_trait::async_trait]
pub trait LlmClient: Send + Sync {
    async fn chat(&self, messages: Vec<Message>, tools: Option<&[ToolDef]>) -> Result<LlmResponse>;
}

// --- OpenRouter / OpenAI-compatible client ---

pub struct OpenRouterClient {
    client: Client,
    base_url: String,
    api_key: String,
    model: String,
    max_tokens: u32,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<ToolDef>>,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ChoiceMessage,
}

#[derive(Deserialize)]
struct ChoiceMessage {
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<ToolCall>>,
}

const MAX_RETRIES: u32 = 2;

#[async_trait::async_trait]
impl LlmClient for OpenRouterClient {
    async fn chat(&self, messages: Vec<Message>, tools: Option<&[ToolDef]>) -> Result<LlmResponse> {
        let url = format!("{}/chat/completions", self.base_url);
        let request = ChatRequest {
            model: self.model.clone(),
            messages,
            max_tokens: self.max_tokens,
            tools: tools.map(|t| t.to_vec()),
        };

        let mut last_err = None;
        for attempt in 0..=MAX_RETRIES {
            if attempt > 0 {
                let delay = Duration::from_secs(1 << (attempt - 1)); // 1s, 2s
                tracing::warn!("LLM retry {attempt}/{MAX_RETRIES} after {delay:?}");
                tokio::time::sleep(delay).await;
            }

            let response = match self
                .client
                .post(&url)
                .bearer_auth(&self.api_key)
                .json(&request)
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    last_err = Some(anyhow::anyhow!("Failed to call LLM API: {e}"));
                    continue;
                }
            };

            let status = response.status();
            if status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error() {
                let body = response.text().await.unwrap_or_default();
                last_err = Some(anyhow::anyhow!("LLM API error ({}): {}", status, body));
                continue;
            }
            if !status.is_success() {
                let body = response.text().await.unwrap_or_default();
                anyhow::bail!("LLM API error ({}): {}", status, body);
            }

            let body: ChatResponse = response
                .json()
                .await
                .context("Failed to parse LLM response")?;
            let choice = body
                .choices
                .into_iter()
                .next()
                .context("No choices in LLM response")?;

            return Ok(LlmResponse {
                content: choice.message.content,
                tool_calls: choice.message.tool_calls.unwrap_or_default(),
            });
        }

        Err(last_err.unwrap_or_else(|| anyhow::anyhow!("LLM request failed after retries")))
    }
}

// --- Factory ---

pub fn create_client(config: &LlmConfig) -> Result<Arc<dyn LlmClient>> {
    let base_url = config
        .base_url
        .clone()
        .unwrap_or_else(|| "https://openrouter.ai/api/v1".to_string());

    Ok(Arc::new(OpenRouterClient {
        client: Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(120))
            .build()?,
        base_url: base_url.trim_end_matches('/').to_string(),
        api_key: config.api_key.clone(),
        model: config.model.clone(),
        max_tokens: config.max_tokens,
    }))
}
