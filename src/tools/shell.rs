use anyhow::Result;
use serde_json::{json, Value};
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
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The shell command to execute"
                }
            },
            "required": ["command"]
        })
    }
    async fn execute(&self, args: Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let cmd = args["command"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'command' argument"))?;

        let output = Command::new("sh").arg("-c").arg(cmd).output().await?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let result = if output.status.success() {
            stdout.to_string()
        } else {
            format!("Error (exit {}): {}", output.status, stderr)
        };

        Ok(ToolResult {
            for_llm: result,
            for_user: None,
        })
    }
}
