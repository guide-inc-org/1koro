use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub agent: AgentConfig,
    pub llm: LlmConfig,
    #[serde(default)]
    pub channels: ChannelsConfig,
    #[serde(default)]
    pub mcp: McpConfig,
    #[serde(default)]
    pub memory: MemoryConfig,
    #[serde(default)]
    pub cron: CronConfig,
}

#[derive(Debug, Deserialize)]
pub struct AgentConfig {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct LlmConfig {
    pub provider: String,
    /// Base URL for the API. Optional — each provider has a sensible default.
    pub base_url: Option<String>,
    pub model: String,
    pub api_key: String,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
}

fn default_max_tokens() -> u32 {
    8192
}

#[derive(Debug, Default, Deserialize)]
pub struct ChannelsConfig {
    pub slack: Option<SlackConfig>,
    pub discord: Option<DiscordConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SlackConfig {
    pub enabled: bool,
    pub bot_token: String,
    pub app_token: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DiscordConfig {
    pub enabled: bool,
    pub token: String,
    pub guild_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct McpConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_mcp_bind")]
    pub bind: String,
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            bind: default_mcp_bind(),
        }
    }
}

fn default_mcp_bind() -> String {
    "127.0.0.1:3000".to_string()
}

#[derive(Debug, Deserialize)]
pub struct MemoryConfig {
    #[serde(default = "default_base_dir")]
    pub base_dir: PathBuf,
    #[serde(default = "default_core_memory_max_tokens")]
    pub core_memory_max_tokens: u32,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            base_dir: default_base_dir(),
            core_memory_max_tokens: default_core_memory_max_tokens(),
        }
    }
}

fn default_base_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".1koro")
}

fn default_core_memory_max_tokens() -> u32 {
    2000
}

#[derive(Debug, Deserialize)]
pub struct CronConfig {
    #[serde(default = "default_daily_cron")]
    pub daily_summary: String,
    #[serde(default = "default_weekly_cron")]
    pub weekly_summary: String,
}

impl Default for CronConfig {
    fn default() -> Self {
        Self {
            daily_summary: default_daily_cron(),
            weekly_summary: default_weekly_cron(),
        }
    }
}

fn default_daily_cron() -> String {
    "0 3 * * *".to_string()
}

fn default_weekly_cron() -> String {
    "0 4 * * 1".to_string()
}

pub fn load(path: &str) -> Result<Config> {
    let path = expand_tilde(path);
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read config: {}", path.display()))?;
    let config: Config =
        toml::from_str(&content).with_context(|| "Failed to parse config.toml")?;
    Ok(config)
}

pub async fn init_config_dir() -> Result<()> {
    let base = default_base_dir();
    let dirs = [
        "core",
        "logs/daily",
        "logs/weekly",
        "logs/monthly",
        "sessions",
        "skills",
    ];
    for d in &dirs {
        tokio::fs::create_dir_all(base.join(d)).await?;
    }

    let identity = base.join("core/identity.md");
    if !identity.exists() {
        tokio::fs::write(
            &identity,
            "# Identity\n\nI am 1koro, a personal AI agent. I remember everything.\n",
        )
        .await?;
    }

    let user = base.join("core/user.md");
    if !user.exists() {
        tokio::fs::write(&user, "# User\n\n(Not yet configured)\n").await?;
    }

    let state = base.join("core/state.md");
    if !state.exists() {
        tokio::fs::write(&state, "# State\n\n(No state yet)\n").await?;
    }

    let config_path = base.join("config.toml");
    if !config_path.exists() {
        tokio::fs::write(
            &config_path,
            r#"[agent]
name = "1koro"

[llm]
provider = "minimax"
# base_url = "https://api.minimaxi.chat/v1"  # optional, uses provider default
model = "MiniMax-M1"
api_key = "YOUR_API_KEY"
max_tokens = 8192

# Other provider examples:
# provider = "openai"
# model = "gpt-4o"
#
# provider = "openrouter"
# model = "anthropic/claude-sonnet-4"
#
# provider = "anthropic"
# model = "claude-sonnet-4-5-20250929"
#
# provider = "google"
# model = "gemini-2.5-pro"

# [channels.slack]
# enabled = true
# bot_token = "xoxb-YOUR_BOT_TOKEN"
# app_token = "xapp-YOUR_APP_TOKEN"

# [channels.discord]
# enabled = true
# token = "YOUR_BOT_TOKEN"

[mcp]
enabled = false
bind = "127.0.0.1:3000"

[memory]
core_memory_max_tokens = 2000

[cron]
daily_summary = "0 3 * * *"
weekly_summary = "0 4 * * 1"
"#,
        )
        .await?;
    }

    Ok(())
}

fn expand_tilde(path: &str) -> PathBuf {
    if path.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(&path[2..]);
        }
    }
    PathBuf::from(path)
}
