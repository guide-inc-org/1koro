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
        Self { bind: default_api_bind() }
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
        Self { enabled: false, bind: default_mcp_bind() }
    }
}

fn default_mcp_bind() -> String {
    "127.0.0.1:3001".to_string()
}

#[derive(Debug, Deserialize)]
pub struct MemoryConfig {
    #[serde(default = "default_base_dir")]
    pub base_dir: PathBuf,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self { base_dir: default_base_dir() }
    }
}

fn default_base_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".1koro")
}

pub fn load(path: &str) -> Result<Config> {
    let path = expand_tilde(path);
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read config: {}", path.display()))?;
    toml::from_str(&content).with_context(|| "Failed to parse config.toml")
}

pub async fn init_config_dir() -> Result<()> {
    let base = default_base_dir();
    for d in ["core", "logs/daily", "logs/weekly", "logs/monthly", "sessions", "skills"] {
        tokio::fs::create_dir_all(base.join(d)).await?;
    }
    let write_if_missing = |path: PathBuf, content: &'static str| async move {
        if !path.exists() { tokio::fs::write(&path, content).await?; }
        Ok::<_, anyhow::Error>(())
    };
    write_if_missing(base.join("core/identity.md"), "# Identity\n\nI am 1koro, a personal AI agent. I remember everything.\n").await?;
    write_if_missing(base.join("core/user.md"), "# User\n\n(Not yet configured)\n").await?;
    write_if_missing(base.join("core/state.md"), "# State\n\n(No state yet)\n").await?;
    write_if_missing(base.join("config.toml"), "[agent]\nname = \"1koro\"\n\n[llm]\nprovider = \"openrouter\"\nmodel = \"google/gemini-2.5-flash\"\napi_key = \"YOUR_API_KEY\"\nmax_tokens = 8192\n\n[api]\nbind = \"127.0.0.1:3000\"\n\n[mcp]\nenabled = false\nbind = \"127.0.0.1:3001\"\n").await?;
    Ok(())
}

fn expand_tilde(path: &str) -> PathBuf {
    if path.starts_with("~/") {
        if let Some(home) = dirs::home_dir() { return home.join(&path[2..]); }
    }
    PathBuf::from(path)
}
