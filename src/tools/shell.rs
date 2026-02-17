use std::time::Duration;

use anyhow::Result;
use serde_json::Value;
use tokio::process::Command;

use super::{ToolContext, ToolResult, ok, require_str};

pub async fn execute(args: &Value, ctx: &ToolContext, timeout: Duration) -> Result<ToolResult> {
    let cmd = require_str(args, "command")?;

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

    let output = match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(Ok(output)) => output,
        Ok(Err(e)) => return ok(format!("Shell error: {e}")),
        Err(_) => {
            if pgid > 0 {
                let ret = unsafe { libc::killpg(pgid, libc::SIGKILL) };
                if ret != 0 {
                    tracing::warn!("killpg({pgid}) failed: {}", std::io::Error::last_os_error());
                }
                for _ in 0..3 {
                    let r = unsafe { libc::waitpid(-pgid, std::ptr::null_mut(), libc::WNOHANG) };
                    if r != 0 {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            }
            return ok(format!("Shell timeout after {}s", timeout.as_secs()));
        }
    };

    if output.status.success() {
        ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        ok(format!(
            "Error (exit {}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}
