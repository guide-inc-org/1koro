use anyhow::Result;
use tokio::process::Command;

pub async fn run_command(cmd: &str) -> Result<String> {
    let output = Command::new("sh").arg("-c").arg(cmd).output().await?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if output.status.success() {
        Ok(stdout.to_string())
    } else {
        anyhow::bail!("Command failed ({}): {}", output.status, stderr)
    }
}
