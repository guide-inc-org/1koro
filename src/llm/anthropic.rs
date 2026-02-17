use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::{FunctionCall, LlmClient, LlmResponse, Message, ToolCall, ToolDef};
use crate::config::LlmConfig;

const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
const API_VERSION: &str = "2023-06-01";

pub struct AnthropicClient {
    client: Client,
    base_url: String,
    api_key: String,
    model: String,
    max_tokens: u32,
}

// --- Request types ---

#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<AnthropicToolDef>>,
}

#[derive(Serialize)]
struct AnthropicToolDef {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

#[derive(Serialize)]
struct AnthropicMessage {
    role: String,
    content: AnthropicContent,
}

#[derive(Serialize)]
#[serde(untagged)]
enum AnthropicContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

#[derive(Serialize)]
#[serde(tag = "type")]
enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
    },
}

// --- Response types ---

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<ResponseBlock>,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum ResponseBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
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

    fn convert_tools(tools: &[ToolDef]) -> Vec<AnthropicToolDef> {
        tools
            .iter()
            .map(|t| AnthropicToolDef {
                name: t.function.name.clone(),
                description: t.function.description.clone(),
                input_schema: t.function.parameters.clone(),
            })
            .collect()
    }

    fn convert_messages(messages: &[Message]) -> (Option<String>, Vec<AnthropicMessage>) {
        let mut system: Option<String> = None;
        let mut result: Vec<AnthropicMessage> = Vec::new();

        for msg in messages {
            match msg.role.as_str() {
                "system" => {
                    let text = msg.content.clone().unwrap_or_default();
                    match &mut system {
                        Some(s) => {
                            s.push_str("\n\n");
                            s.push_str(&text);
                        }
                        None => system = Some(text),
                    }
                }
                "user" => {
                    result.push(AnthropicMessage {
                        role: "user".to_string(),
                        content: AnthropicContent::Text(
                            msg.content.clone().unwrap_or_default(),
                        ),
                    });
                }
                "assistant" => {
                    let mut blocks = Vec::new();
                    if let Some(text) = &msg.content {
                        if !text.is_empty() {
                            blocks.push(ContentBlock::Text { text: text.clone() });
                        }
                    }
                    if let Some(tool_calls) = &msg.tool_calls {
                        for tc in tool_calls {
                            let input: serde_json::Value =
                                serde_json::from_str(&tc.function.arguments)
                                    .unwrap_or(serde_json::Value::Object(Default::default()));
                            blocks.push(ContentBlock::ToolUse {
                                id: tc.id.clone(),
                                name: tc.function.name.clone(),
                                input,
                            });
                        }
                    }
                    if blocks.is_empty() {
                        blocks.push(ContentBlock::Text {
                            text: String::new(),
                        });
                    }
                    result.push(AnthropicMessage {
                        role: "assistant".to_string(),
                        content: AnthropicContent::Blocks(blocks),
                    });
                }
                "tool" => {
                    let block = ContentBlock::ToolResult {
                        tool_use_id: msg.tool_call_id.clone().unwrap_or_default(),
                        content: msg.content.clone().unwrap_or_default(),
                    };
                    // Merge consecutive tool results into one user message
                    if let Some(last) = result.last_mut() {
                        if last.role == "user" {
                            if let AnthropicContent::Blocks(ref mut blocks) = last.content {
                                if blocks
                                    .iter()
                                    .all(|b| matches!(b, ContentBlock::ToolResult { .. }))
                                {
                                    blocks.push(block);
                                    continue;
                                }
                            }
                        }
                    }
                    result.push(AnthropicMessage {
                        role: "user".to_string(),
                        content: AnthropicContent::Blocks(vec![block]),
                    });
                }
                _ => {}
            }
        }

        (system, result)
    }
}

#[async_trait::async_trait]
impl LlmClient for AnthropicClient {
    async fn chat(
        &self,
        messages: Vec<Message>,
        tools: Option<&[ToolDef]>,
    ) -> Result<LlmResponse> {
        let url = format!("{}/v1/messages", self.base_url);
        let (system, api_messages) = Self::convert_messages(&messages);

        let request = AnthropicRequest {
            model: self.model.clone(),
            max_tokens: self.max_tokens,
            system,
            messages: api_messages,
            tools: tools.map(Self::convert_tools),
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
            .context("Failed to call Anthropic API")?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Anthropic API error ({}): {}", status, body);
        }

        let body: AnthropicResponse = response
            .json()
            .await
            .context("Failed to parse Anthropic response")?;

        let mut content = None;
        let mut tool_calls = Vec::new();

        for block in body.content {
            match block {
                ResponseBlock::Text { text } => content = Some(text),
                ResponseBlock::ToolUse { id, name, input } => {
                    tool_calls.push(ToolCall {
                        id,
                        type_: "function".to_string(),
                        function: FunctionCall {
                            name,
                            arguments: serde_json::to_string(&input)?,
                        },
                    });
                }
            }
        }

        Ok(LlmResponse { content, tool_calls })
    }
}
