use anyhow::Result;
use serde_json::{Value, json};
use tokio::process::Command;

use super::{Tool, ToolContext, ToolResult};

pub struct ShellTool;

#[async_trait::async_trait]
impl Tool for ShellTool {
    fn name(&self) -> &str {
        "shell"
    }
    fn description(&self) -> &str {
        "Execute a shell command and return the output"
    }
    fn parameters(&self) -> Value {
        json!({ "type": "object", "properties": { "command": { "type": "string", "description": "Shell command to execute" } }, "required": ["command"] })
    }
    async fn execute(&self, args: Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let cmd = args["command"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'command'"))?;
        let output = Command::new("sh").arg("-c").arg(cmd).output().await?;
        let result = if output.status.success() {
            String::from_utf8_lossy(&output.stdout).to_string()
        } else {
            format!(
                "Error (exit {}): {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            )
        };
        Ok(ToolResult { for_llm: result })
    }
}
