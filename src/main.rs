mod agent;
mod api;
mod config;
mod context;
mod llm;
mod mcp;
mod memory;
mod session;
mod skills;
mod tools;

use std::sync::Arc;

use anyhow::Result;
use clap::{Parser, Subcommand};
use tokio::signal;

#[derive(Parser)]
#[command(
    name = "1koro",
    version,
    about = "Personal AI agent that never forgets"
)]
struct Cli {
    #[arg(short, long, default_value = "~/.1koro/config.toml")]
    config: String,
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Run,
    Init,
    Status,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();
    match cli.command.unwrap_or(Commands::Run) {
        Commands::Init => {
            config::init_config_dir().await?;
            tracing::info!("Initialized ~/.1koro/");
        }
        Commands::Run => run(&cli.config).await?,
        Commands::Status => {
            let cfg = config::load(&cli.config)?;
            println!(
                "{}",
                memory::MemoryManager::new(&cfg.memory)?.read_core("state.md")?
            );
        }
    }
    Ok(())
}

async fn run(config_path: &str) -> Result<()> {
    let cfg = config::load(config_path)?;
    let mem = Arc::new(memory::MemoryManager::new(&cfg.memory)?);
    let llm = llm::create_client(&cfg.llm)?;

    let tool_ctx = tools::ToolContext {
        memory: mem.clone(),
        base_dir: cfg.memory.base_dir.clone(),
    };
    let mut reg = tools::ToolRegistry::new(tool_ctx);
    if cfg.tools.shell_enabled {
        reg.register(Box::new(tools::shell::ShellTool));
        tracing::warn!("Shell tool enabled — arbitrary command execution is possible");
    }
    reg.register(Box::new(tools::memory::SearchLogsTool));
    reg.register(Box::new(tools::memory::ReadCoreMemoryTool));
    reg.register(Box::new(tools::memory::UpdateCoreMemoryTool));
    reg.register(Box::new(tools::memory::AppendLogTool));
    reg.register(Box::new(tools::memory::ReadDailyLogTool));
    reg.register(Box::new(tools::memory::WriteSummaryTool));
    reg.register(Box::new(tools::file::ReadFileTool));

    let skills = skills::SkillLoader::new(&cfg.memory.base_dir).load_summaries()?;
    if !skills.is_empty() {
        tracing::info!("Loaded {} skills", skills.len());
    }

    let sessions = session::SessionStore::new(cfg.memory.base_dir.clone())?;
    let agent = agent::Agent::new(llm, mem.clone(), sessions, reg, skills);

    if cfg.mcp.enabled {
        if cfg.mcp.api_key.is_none() && !is_localhost(&cfg.mcp.bind) {
            anyhow::bail!(
                "MCP authentication required for non-localhost binding '{}'. Set [mcp] api_key.",
                cfg.mcp.bind
            );
        }
        mcp::start(
            &cfg.mcp.bind,
            mem.clone(),
            &cfg.agent.name,
            cfg.mcp.api_key.clone(),
        )
        .await?;
        if cfg.mcp.api_key.is_none() {
            tracing::warn!("MCP authentication disabled (localhost-only)");
        }
    }

    if cfg.api.api_key.is_none() {
        if is_localhost(&cfg.api.bind) {
            tracing::warn!("API authentication disabled (localhost-only)");
        } else {
            anyhow::bail!(
                "API authentication required for non-localhost binding '{}'. Set [api] api_key.",
                cfg.api.bind
            );
        }
    }

    let state = api::AppState {
        agent: Arc::new(agent),
        name: cfg.agent.name.clone(),
        api_key: cfg.api.api_key.clone(),
    };
    let listener = tokio::net::TcpListener::bind(&cfg.api.bind).await?;
    tracing::info!("{} listening on {}", cfg.agent.name, cfg.api.bind);

    axum::serve(listener, api::router(state))
        .with_graceful_shutdown(async {
            signal::ctrl_c().await.ok();
        })
        .await?;
    Ok(())
}

fn is_localhost(bind: &str) -> bool {
    // Extract the host part (before the last ':port')
    let host = if let Some(bracket_end) = bind.find(']') {
        // IPv6: [::1]:3000
        &bind[..=bracket_end]
    } else if let Some(colon) = bind.rfind(':') {
        &bind[..colon]
    } else {
        bind
    };
    host == "localhost" || host == "127.0.0.1" || host == "[::1]" || is_loopback_ip(host)
}

/// Check if host is a 127.x.y.z loopback IP (not a hostname like 127.evil.com).
fn is_loopback_ip(host: &str) -> bool {
    if let Some(rest) = host.strip_prefix("127.") {
        // Must be digits and dots only (e.g. "0.0.1", "0.1.1")
        !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit() || b == b'.')
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_localhost_loopback() {
        assert!(is_localhost("127.0.0.1:3000"));
        assert!(is_localhost("127.0.1.1:8080"));
        assert!(is_localhost("localhost:3000"));
        assert!(is_localhost("[::1]:3000"));
    }

    #[test]
    fn test_is_localhost_rejects_non_local() {
        assert!(!is_localhost("0.0.0.0:3000"));
        assert!(!is_localhost("192.168.1.1:3000"));
        assert!(!is_localhost("example.com:3000"));
        // "localhost" prefix in a different hostname must not match
        assert!(!is_localhost("localhost.evil.com:3000"));
        // "127." prefix in a hostname must not match
        assert!(!is_localhost("127.evil.com:3000"));
        assert!(!is_localhost("127.0.0.evil:3000"));
    }

    #[test]
    fn test_is_loopback_ip() {
        assert!(is_loopback_ip("127.0.0.1"));
        assert!(is_loopback_ip("127.0.1.1"));
        assert!(is_loopback_ip("127.255.255.255"));
        assert!(!is_loopback_ip("127.evil.com"));
        assert!(!is_loopback_ip("127.0.0.evil"));
        assert!(!is_loopback_ip("127."));
        assert!(!is_loopback_ip("128.0.0.1"));
        assert!(!is_loopback_ip("localhost"));
    }
}
