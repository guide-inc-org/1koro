pub mod anthropic;
pub mod openai_compatible;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::config::LlmConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Object-safe LLM client trait.
#[async_trait::async_trait]
pub trait LlmClient: Send + Sync {
    async fn chat(&self, messages: Vec<Message>) -> Result<String>;
}

/// Create an LLM client from config.
///
/// Supported providers:
/// - "openai"       → OpenAI (default: https://api.openai.com/v1)
/// - "minimax"      → MiniMax (default: https://api.minimaxi.chat/v1)
/// - "openrouter"   → OpenRouter (default: https://openrouter.ai/api/v1)
/// - "google"       → Google Gemini OpenAI-compat (default: https://generativelanguage.googleapis.com/v1beta/openai)
/// - "groq"         → Groq (default: https://api.groq.com/openai/v1)
/// - "together"     → Together AI (default: https://api.together.xyz/v1)
/// - "deepseek"     → DeepSeek (default: https://api.deepseek.com/v1)
/// - "anthropic"    → Anthropic Messages API (separate implementation)
///
/// Any provider except "anthropic" uses the OpenAI-compatible client.
/// You can override the base_url in config for custom/self-hosted endpoints.
pub fn create_client(config: &LlmConfig) -> Result<Box<dyn LlmClient>> {
    match config.provider.as_str() {
        "anthropic" => Ok(Box::new(anthropic::AnthropicClient::new(config)?)),
        provider => {
            let base_url = config
                .base_url
                .clone()
                .unwrap_or_else(|| default_base_url(provider).to_string());
            Ok(Box::new(openai_compatible::OpenAICompatibleClient::new(
                config, &base_url,
            )?))
        }
    }
}

fn default_base_url(provider: &str) -> &str {
    match provider {
        "openai" => "https://api.openai.com/v1",
        "minimax" => "https://api.minimaxi.chat/v1",
        "openrouter" => "https://openrouter.ai/api/v1",
        "google" => "https://generativelanguage.googleapis.com/v1beta/openai",
        "groq" => "https://api.groq.com/openai/v1",
        "together" => "https://api.together.xyz/v1",
        "deepseek" => "https://api.deepseek.com/v1",
        _ => "https://api.openai.com/v1",
    }
}
