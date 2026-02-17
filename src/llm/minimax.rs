use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::{LlmClient, Message};
use crate::config::LlmConfig;

pub struct MiniMaxClient {
    client: Client,
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

impl MiniMaxClient {
    pub fn new(config: &LlmConfig) -> Result<Self> {
        Ok(Self {
            client: Client::new(),
            api_key: config.api_key.clone(),
            model: config.model.clone(),
            max_tokens: config.max_tokens,
        })
    }
}

#[async_trait::async_trait]
impl LlmClient for MiniMaxClient {
    async fn chat(&self, messages: Vec<Message>) -> Result<String> {
        let request = ChatRequest {
            model: self.model.clone(),
            messages,
            max_tokens: self.max_tokens,
        };

        let response = self
            .client
            .post("https://api.minimaxi.chat/v1/text/chatcompletion_v2")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await
            .context("Failed to call MiniMax API")?;

        let body: ChatResponse = response
            .json()
            .await
            .context("Failed to parse MiniMax response")?;

        body.choices
            .first()
            .map(|c| c.message.content.clone())
            .ok_or_else(|| anyhow::anyhow!("Empty response from MiniMax"))
    }
}
