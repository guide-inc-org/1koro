use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::{LlmClient, Message};
use crate::config::LlmConfig;

/// Generic client for any OpenAI-compatible chat completions API.
///
/// Works with: OpenAI, MiniMax, OpenRouter, Google Gemini, Groq,
/// Together AI, DeepSeek, vLLM, Ollama, LiteLLM, and any other
/// provider that implements the `/chat/completions` endpoint.
pub struct OpenAICompatibleClient {
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
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: MessageContent,
}

#[derive(Deserialize)]
struct MessageContent {
    content: String,
}

impl OpenAICompatibleClient {
    pub fn new(config: &LlmConfig, base_url: &str) -> Result<Self> {
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
impl LlmClient for OpenAICompatibleClient {
    async fn chat(&self, messages: Vec<Message>) -> Result<String> {
        let url = format!("{}/chat/completions", self.base_url);

        let request = ChatRequest {
            model: self.model.clone(),
            messages,
            max_tokens: self.max_tokens,
        };

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .with_context(|| format!("Failed to call {url}"))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("LLM API error ({}): {}", status, body);
        }

        let body: ChatResponse = response
            .json()
            .await
            .context("Failed to parse LLM response")?;

        body.choices
            .first()
            .map(|c| c.message.content.clone())
            .ok_or_else(|| anyhow::anyhow!("Empty response from LLM"))
    }
}
