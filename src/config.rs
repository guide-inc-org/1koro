use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub agent: AgentConfig,
    pub llm: LlmConfig,
    #[serde(default)]
    pub api: ApiConfig,
    #[serde(default)]
    pub mcp: McpConfig,
    #[serde(default)]
    pub memory: MemoryConfig,
}

#[derive(Debug, Deserialize)]
pub struct AgentConfig {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct LlmConfig {
    pub provider: String,
    pub base_url: Option<String>,
    pub model: String,
    pub api_key: String,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
}

fn default_max_tokens() -> u32 {
    8192
}

#[derive(Debug, Deserialize)]
pub struct ApiConfig {
    #[serde(default = "default_api_bind")]
    pub bind: String,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            bind: default_api_bind(),
        }
    }
}

fn default_api_bind() -> String {
    "127.0.0.1:3000".to_string()
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
    "127.0.0.1:3001".to_string()
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
provider = "openrouter"
model = "google/gemini-2.5-flash"
api_key = "YOUR_API_KEY"
max_tokens = 8192

[api]
bind = "127.0.0.1:3000"

[mcp]
enabled = false
bind = "127.0.0.1:3001"

[memory]
core_memory_max_tokens = 2000
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
