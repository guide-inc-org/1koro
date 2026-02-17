use std::time::Duration;

use anyhow::Result;
use serde_json::{Value, json};
use tokio::process::Command;

use super::{Tool, ToolContext, ToolResult};

pub struct ShellTool {
    timeout: Duration,
}

impl ShellTool {
    pub fn new(timeout_secs: u64) -> Self {
        Self {
            timeout: Duration::from_secs(timeout_secs),
        }
    }
}

#[async_trait::async_trait]
impl Tool for ShellTool {
    fn name(&self) -> &str {
        "shell"
    }
    fn description(&self) -> &str {
        "Execute a shell command (runs in memory directory)"
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
            .process_group(0)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| anyhow::anyhow!("Shell spawn error: {e}"))?;

        let pgid = child.id().unwrap_or(0) as i32;

        let output = match tokio::time::timeout(self.timeout, child.wait_with_output()).await {
            Ok(Ok(output)) => output,
            Ok(Err(e)) => {
                return Ok(ToolResult {
                    for_llm: format!("Shell error: {e}"),
                });
            }
            Err(_) => {
                if pgid > 0 {
                    let kill_ret = unsafe { libc::killpg(pgid, libc::SIGKILL) };
                    if kill_ret != 0 {
                        tracing::warn!(
                            "killpg({pgid}) failed: {}",
                            std::io::Error::last_os_error()
                        );
                    }
                    for _ in 0..3 {
                        let ret =
                            unsafe { libc::waitpid(-pgid, std::ptr::null_mut(), libc::WNOHANG) };
                        if ret != 0 {
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                }
                return Ok(ToolResult {
                    for_llm: format!("Shell timeout after {}s", self.timeout.as_secs()),
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
