use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::{LlmClient, Message};
use crate::config::LlmConfig;

const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
const API_VERSION: &str = "2023-06-01";

/// Client for Anthropic's Messages API.
///
/// Anthropic uses a different request/response format from OpenAI,
/// so it needs its own implementation.
pub struct AnthropicClient {
    client: Client,
    base_url: String,
    api_key: String,
    model: String,
    max_tokens: u32,
}

#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    messages: Vec<AnthropicMessage>,
}

#[derive(Serialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<ContentBlock>,
}

#[derive(Deserialize)]
struct ContentBlock {
    text: Option<String>,
}

impl AnthropicClient {
    pub fn new(config: &LlmConfig) -> Result<Self> {
        let base_url = config
            .base_url
            .clone()
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());

        Ok(Self {
            client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: config.api_key.clone(),
            model: config.model.clone(),
            max_tokens: config.max_tokens,
        })
    }
}

#[async_trait::async_trait]
impl LlmClient for AnthropicClient {
    async fn chat(&self, messages: Vec<Message>) -> Result<String> {
        let url = format!("{}/v1/messages", self.base_url);

        // Extract system message (Anthropic puts it as a top-level field)
        let mut system = None;
        let mut api_messages = Vec::new();

        for msg in messages {
            if msg.role == "system" {
                system = Some(msg.content);
            } else {
                api_messages.push(AnthropicMessage {
                    role: msg.role,
                    content: msg.content,
                });
            }
        }

        let request = AnthropicRequest {
            model: self.model.clone(),
            max_tokens: self.max_tokens,
            system,
            messages: api_messages,
        };

        let response = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", API_VERSION)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .with_context(|| "Failed to call Anthropic API")?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Anthropic API error ({}): {}", status, body);
        }

        let body: AnthropicResponse = response
            .json()
            .await
            .context("Failed to parse Anthropic response")?;

        body.content
            .iter()
            .filter_map(|b| b.text.as_ref())
            .next()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Empty response from Anthropic"))
    }
}
