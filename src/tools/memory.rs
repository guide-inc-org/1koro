use anyhow::Result;
use serde_json::{Value, json};

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
        json!({ "type": "object", "properties": { "query": { "type": "string" } }, "required": ["query"] })
    }
    async fn execute(&self, args: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let results = ctx
            .memory
            .search_logs(args["query"].as_str().unwrap_or(""))?;
        Ok(ToolResult {
            for_llm: if results.is_empty() {
                "No results found.".into()
            } else {
                results.join("\n")
            },
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
        json!({ "type": "object", "properties": { "file": { "type": "string", "enum": ["identity.md", "user.md", "state.md"] } }, "required": ["file"] })
    }
    async fn execute(&self, args: Value, ctx: &ToolContext) -> Result<ToolResult> {
        Ok(ToolResult {
            for_llm: ctx
                .memory
                .read_core(args["file"].as_str().unwrap_or("state.md"))?,
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
        "Update user.md or state.md with new content"
    }
    fn parameters(&self) -> Value {
        json!({ "type": "object", "properties": { "file": { "type": "string", "enum": ["user.md", "state.md"] }, "content": { "type": "string" } }, "required": ["file", "content"] })
    }
    async fn execute(&self, args: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let file = args["file"].as_str().unwrap_or("state.md");
        if file == "identity.md" {
            return Ok(ToolResult {
                for_llm: "Error: identity.md is read-only".into(),
            });
        }
        ctx.memory
            .write_core(file, args["content"].as_str().unwrap_or(""))?;
        Ok(ToolResult {
            for_llm: format!("Updated {file}"),
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
        "Read a daily log by date (YYYY-MM-DD)"
    }
    fn parameters(&self) -> Value {
        json!({ "type": "object", "properties": { "date": { "type": "string" } }, "required": ["date"] })
    }
    async fn execute(&self, args: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let date = args["date"].as_str().unwrap_or("");
        Ok(ToolResult {
            for_llm: ctx
                .memory
                .read_daily_log(date)?
                .unwrap_or_else(|| format!("No log for {date}")),
        })
    }
}

pub struct WriteSummaryTool;

#[async_trait::async_trait]
impl Tool for WriteSummaryTool {
    fn name(&self) -> &str {
        "write_summary"
    }
    fn description(&self) -> &str {
        "Write a weekly or monthly summary. period='weekly' id='2026-W08', or period='monthly' id='2026-02'"
    }
    fn parameters(&self) -> Value {
        json!({ "type": "object", "properties": {
            "period": { "type": "string", "enum": ["weekly", "monthly"] },
            "id": { "type": "string", "description": "e.g. '2026-W08' or '2026-02'" },
            "content": { "type": "string" }
        }, "required": ["period", "id", "content"] })
    }
    async fn execute(&self, args: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let period = args["period"].as_str().unwrap_or("weekly");
        let id = args["id"].as_str().unwrap_or("");
        let content = args["content"].as_str().unwrap_or("");
        if id.is_empty() || content.is_empty() {
            return Ok(ToolResult {
                for_llm: "Error: id and content required".into(),
            });
        }
        match period {
            "weekly" => ctx.memory.write_weekly_summary(id, content)?,
            "monthly" => ctx.memory.write_monthly_summary(id, content)?,
            _ => {
                return Ok(ToolResult {
                    for_llm: format!("Unknown period: {period}"),
                });
            }
        }
        Ok(ToolResult {
            for_llm: format!("Written {period} summary: {id}"),
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
        json!({ "type": "object", "properties": { "text": { "type": "string" } }, "required": ["text"] })
    }
    async fn execute(&self, args: Value, ctx: &ToolContext) -> Result<ToolResult> {
        ctx.memory.append_log(args["text"].as_str().unwrap_or(""))?;
        Ok(ToolResult {
            for_llm: "Note appended.".into(),
        })
    }
}
