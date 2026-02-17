use anyhow::Result;
use serde_json::{json, Value};

use super::{Tool, ToolContext, ToolResult};

pub struct SearchLogsTool;

#[async_trait::async_trait]
impl Tool for SearchLogsTool {
    fn name(&self) -> &str {
        "search_logs"
    }
    fn description(&self) -> &str {
        "Search past conversation logs and daily notes for a keyword"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Search query" }
            },
            "required": ["query"]
        })
    }
    async fn execute(&self, args: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let query = args["query"].as_str().unwrap_or("");
        let results = ctx.memory.search_logs(query)?;
        let text = if results.is_empty() {
            "No results found.".to_string()
        } else {
            results.join("\n")
        };
        Ok(ToolResult {
            for_llm: text,
            for_user: None,
        })
    }
}

pub struct ReadCoreMemoryTool;

#[async_trait::async_trait]
impl Tool for ReadCoreMemoryTool {
    fn name(&self) -> &str {
        "read_core_memory"
    }
    fn description(&self) -> &str {
        "Read a core memory file (identity.md, user.md, or state.md)"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file": {
                    "type": "string",
                    "description": "Filename: identity.md, user.md, or state.md",
                    "enum": ["identity.md", "user.md", "state.md"]
                }
            },
            "required": ["file"]
        })
    }
    async fn execute(&self, args: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let file = args["file"].as_str().unwrap_or("state.md");
        let content = ctx.memory.read_core(file)?;
        Ok(ToolResult {
            for_llm: content,
            for_user: None,
        })
    }
}

pub struct UpdateCoreMemoryTool;

#[async_trait::async_trait]
impl Tool for UpdateCoreMemoryTool {
    fn name(&self) -> &str {
        "update_core_memory"
    }
    fn description(&self) -> &str {
        "Update a core memory file (user.md or state.md) with new content"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file": {
                    "type": "string",
                    "description": "Filename: user.md or state.md",
                    "enum": ["user.md", "state.md"]
                },
                "content": {
                    "type": "string",
                    "description": "New content for the file (markdown)"
                }
            },
            "required": ["file", "content"]
        })
    }
    async fn execute(&self, args: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let file = args["file"].as_str().unwrap_or("state.md");
        if file == "identity.md" {
            return Ok(ToolResult {
                for_llm: "Error: identity.md is read-only".to_string(),
                for_user: None,
            });
        }
        let content = args["content"].as_str().unwrap_or("");
        ctx.memory.write_core(file, content)?;
        Ok(ToolResult {
            for_llm: format!("Updated {file}"),
            for_user: None,
        })
    }
}

pub struct AppendLogTool;

#[async_trait::async_trait]
impl Tool for AppendLogTool {
    fn name(&self) -> &str {
        "append_note"
    }
    fn description(&self) -> &str {
        "Append a note to today's daily log"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "text": { "type": "string", "description": "Note to append" }
            },
            "required": ["text"]
        })
    }
    async fn execute(&self, args: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let text = args["text"].as_str().unwrap_or("");
        ctx.memory.append_log(text)?;
        Ok(ToolResult {
            for_llm: "Note appended to today's log.".to_string(),
            for_user: None,
        })
    }
}
