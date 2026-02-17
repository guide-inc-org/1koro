use std::time::Duration;

use anyhow::Result;
use serde_json::{Value, json};
use tokio::process::Command;

use super::{Tool, ToolContext, ToolResult};

const SHELL_TIMEOUT: Duration = Duration::from_secs(30);

pub struct ShellTool;

#[async_trait::async_trait]
impl Tool for ShellTool {
    fn name(&self) -> &str {
        "shell"
    }
    fn description(&self) -> &str {
        "Execute a shell command (30s timeout, runs in memory directory)"
    }
    fn parameters(&self) -> Value {
        json!({ "type": "object", "properties": { "command": { "type": "string", "description": "Shell command to execute" } }, "required": ["command"] })
    }
    async fn execute(&self, args: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let cmd = args["command"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'command'"))?;

        let child = Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .current_dir(&ctx.base_dir)
            .process_group(0) // new process group so we can kill all descendants
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| anyhow::anyhow!("Shell spawn error: {e}"))?;

        let pgid = child.id().unwrap_or(0) as i32;

        let output = match tokio::time::timeout(SHELL_TIMEOUT, child.wait_with_output()).await {
            Ok(Ok(output)) => output,
            Ok(Err(e)) => {
                return Ok(ToolResult {
                    for_llm: format!("Shell error: {e}"),
                });
            }
            Err(_) => {
                // Kill the entire process group (sh + all children), then reap
                if pgid > 0 {
                    unsafe {
                        libc::killpg(pgid, libc::SIGKILL);
                    }
                    // waitpid to reap zombie; WNOHANG avoids blocking if already reaped
                    unsafe {
                        libc::waitpid(-pgid, std::ptr::null_mut(), libc::WNOHANG);
                    }
                }
                return Ok(ToolResult {
                    for_llm: format!("Shell timeout after {}s", SHELL_TIMEOUT.as_secs()),
                });
            }
        };

        let text = if output.status.success() {
            String::from_utf8_lossy(&output.stdout).to_string()
        } else {
            format!(
                "Error (exit {}): {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            )
        };
        Ok(ToolResult { for_llm: text })
    }
}
