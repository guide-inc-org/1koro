pub mod minimax;

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".to_string(),
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: content.into(),
        }
    }
}

/// Object-safe LLM client trait for dynamic dispatch.
#[async_trait::async_trait]
pub trait LlmClient: Send + Sync {
    async fn chat(&self, messages: Vec<Message>) -> Result<String>;
}

use crate::config::LlmConfig;

pub fn create_client(config: &LlmConfig) -> Result<Box<dyn LlmClient>> {
    match config.provider.as_str() {
        "minimax" => Ok(Box::new(minimax::MiniMaxClient::new(config)?)),
        other => anyhow::bail!("Unknown LLM provider: {other}"),
    }
}
