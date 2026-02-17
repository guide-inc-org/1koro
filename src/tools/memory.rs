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

pub struct ReadDailyLogTool;

#[async_trait::async_trait]
impl Tool for ReadDailyLogTool {
    fn name(&self) -> &str {
        "read_daily_log"
    }
    fn description(&self) -> &str {
        "Read a daily log by date. Use this to review what happened on a specific day."
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "date": {
                    "type": "string",
                    "description": "Date in YYYY-MM-DD format"
                }
            },
            "required": ["date"]
        })
    }
    async fn execute(&self, args: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let date = args["date"].as_str().unwrap_or("");
        match ctx.memory.read_daily_log(date)? {
            Some(content) => Ok(ToolResult {
                for_llm: content,
                for_user: None,
            }),
            None => Ok(ToolResult {
                for_llm: format!("No log found for {date}"),
                for_user: None,
            }),
        }
    }
}

pub struct WriteSummaryTool;

#[async_trait::async_trait]
impl Tool for WriteSummaryTool {
    fn name(&self) -> &str {
        "write_summary"
    }
    fn description(&self) -> &str {
        "Write a weekly or monthly summary. Use period='weekly' with id like '2026-W08', \
         or period='monthly' with id like '2026-02'."
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "period": {
                    "type": "string",
                    "description": "Summary period: 'weekly' or 'monthly'",
                    "enum": ["weekly", "monthly"]
                },
                "id": {
                    "type": "string",
                    "description": "Period identifier (e.g. '2026-W08' for weekly, '2026-02' for monthly)"
                },
                "content": {
                    "type": "string",
                    "description": "Summary content in markdown"
                }
            },
            "required": ["period", "id", "content"]
        })
    }
    async fn execute(&self, args: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let period = args["period"].as_str().unwrap_or("weekly");
        let id = args["id"].as_str().unwrap_or("");
        let content = args["content"].as_str().unwrap_or("");

        if id.is_empty() || content.is_empty() {
            return Ok(ToolResult {
                for_llm: "Error: id and content are required".to_string(),
                for_user: None,
            });
        }

        match period {
            "weekly" => ctx.memory.write_weekly_summary(id, content)?,
            "monthly" => ctx.memory.write_monthly_summary(id, content)?,
            _ => {
                return Ok(ToolResult {
                    for_llm: format!("Error: unknown period '{period}', use 'weekly' or 'monthly'"),
                    for_user: None,
                });
            }
        }

        Ok(ToolResult {
            for_llm: format!("Written {period} summary: {id}"),
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
