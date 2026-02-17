mod agent;
mod api;
mod config;
mod llm;
mod mcp;
mod memory;
mod session;
mod tools;

use std::sync::Arc;

use anyhow::Result;
use clap::{Parser, Subcommand};
use tokio::signal;

use tools::ToolKind;

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
        reg.add(ToolKind::Shell(std::time::Duration::from_secs(
            cfg.tools.shell_timeout,
        )));
        tracing::warn!(
            "Shell tool enabled ({}s timeout) — arbitrary command execution is possible",
            cfg.tools.shell_timeout
        );
    }
    reg.add(ToolKind::SearchLogs);
    reg.add(ToolKind::ReadCoreMemory);
    reg.add(ToolKind::UpdateCoreMemory);
    reg.add(ToolKind::AppendLog);
    reg.add(ToolKind::ReadDailyLog);
    reg.add(ToolKind::WriteSummary);
    reg.add(ToolKind::ReadFile);

    let skills = agent::load_skills(&cfg.memory.base_dir)?;
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
        let mcp_ctx = tools::ToolContext {
            memory: mem.clone(),
            base_dir: cfg.memory.base_dir.clone(),
        };
        let mut mcp_reg = tools::ToolRegistry::new(mcp_ctx);
        mcp_reg.add(ToolKind::SearchLogs);
        mcp_reg.add(ToolKind::ReadCoreMemory);
        mcp_reg.add(ToolKind::UpdateCoreMemory);
        mcp_reg.add(ToolKind::ReadDailyLog);
        mcp::start(
            &cfg.mcp.bind,
            Arc::new(mcp_reg),
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
    use std::net::IpAddr;

    let host = if let Some(inner) = bind.strip_prefix('[')
        && let Some(bracket_end) = inner.find(']')
    {
        &inner[..bracket_end]
    } else if let Some(colon) = bind.rfind(':') {
        &bind[..colon]
    } else {
        bind
    };

    if host == "localhost" {
        return true;
    }
    host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_localhost_loopback() {
        assert!(is_localhost("127.0.0.1:3000"));
        assert!(is_localhost("127.0.1.1:8080"));
        assert!(is_localhost("127.255.255.255:3000"));
        assert!(is_localhost("localhost:3000"));
        assert!(is_localhost("[::1]:3000"));
    }

    #[test]
    fn test_is_localhost_rejects_non_local() {
        assert!(!is_localhost("0.0.0.0:3000"));
        assert!(!is_localhost("192.168.1.1:3000"));
        assert!(!is_localhost("example.com:3000"));
        assert!(!is_localhost("localhost.evil.com:3000"));
        assert!(!is_localhost("127.evil.com:3000"));
        assert!(!is_localhost("127.0.0.evil:3000"));
        assert!(!is_localhost("127.0.0.1.1:3000"));
    }

    #[test]
    fn test_is_localhost_malformed_no_panic() {
        assert!(!is_localhost("]"));
        assert!(!is_localhost("[]"));
        assert!(!is_localhost("["));
        assert!(!is_localhost(""));
        assert!(!is_localhost(":"));
        assert!(!is_localhost("[]:3000"));
    }
}
