use anyhow::Result;
use serde_json::{json, Value};

use super::{Tool, ToolContext, ToolResult};

/// Import logs from kondo-daily-ops repository into 1koro's daily logs.
pub struct ImportDailyOpsTool;

#[async_trait::async_trait]
impl Tool for ImportDailyOpsTool {
    fn name(&self) -> &str {
        "import_dailyops"
    }
    fn description(&self) -> &str {
        "Import a log file from ~/kondo-daily-ops/logs/ into 1koro's daily logs. \
         Provide the date (YYYY-MM-DD) or a relative path."
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "date": {
                    "type": "string",
                    "description": "Date in YYYY-MM-DD format to import"
                }
            },
            "required": ["date"]
        })
    }
    async fn execute(&self, args: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let date = args["date"].as_str().unwrap_or("");
        if date.is_empty() {
            return Ok(ToolResult {
                for_llm: "Error: date is required".to_string(),
                for_user: None,
            });
        }

        // Parse date to build path: ~/kondo-daily-ops/logs/YYYY/MM/DD.md
        let parts: Vec<&str> = date.split('-').collect();
        if parts.len() != 3 {
            return Ok(ToolResult {
                for_llm: "Error: date must be YYYY-MM-DD format".to_string(),
                for_user: None,
            });
        }

        let year = parts[0];
        let month = parts[1];
        let day = parts[2];

        let home = dirs::home_dir().unwrap_or_default();
        let source_file = home
            .join("kondo-daily-ops/logs")
            .join(year)
            .join(month)
            .join(format!("{day}.md"));
        let source_dir = home
            .join("kondo-daily-ops/logs")
            .join(year)
            .join(month)
            .join(day);

        // Try file first, then directory with README.md
        let content = if source_file.exists() {
            std::fs::read_to_string(&source_file)?
        } else if source_dir.join("README.md").exists() {
            std::fs::read_to_string(source_dir.join("README.md"))?
        } else {
            return Ok(ToolResult {
                for_llm: format!(
                    "No log found for {date} at {} or {}/README.md",
                    source_file.display(),
                    source_dir.display()
                ),
                for_user: None,
            });
        };

        // Write to 1koro's daily log
        let target = ctx.base_dir.join(format!("logs/daily/{date}.md"));
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // If target already exists, append rather than overwrite
        if target.exists() {
            let existing = std::fs::read_to_string(&target)?;
            let merged = format!(
                "{existing}\n\n---\n## Imported from kondo-daily-ops\n\n{content}"
            );
            std::fs::write(&target, merged)?;
        } else {
            std::fs::write(&target, &content)?;
        }

        Ok(ToolResult {
            for_llm: format!("Imported log for {date} ({} bytes)", content.len()),
            for_user: None,
        })
    }
}
