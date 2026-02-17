mod agent;
mod api;
mod config;
mod llm;
mod mcp;
mod memory;
mod session;
mod tools;

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use clap::{Parser, Subcommand};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
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
    /// Chat with 1koro via the running API server
    Chat {
        /// Message to send (omit for interactive mode)
        message: Option<String>,
        /// API server URL
        #[arg(long, default_value = "http://127.0.0.1:3000")]
        url: String,
        /// Auth token (or IKORO_AUTH_TOKEN env)
        #[arg(long, env = "IKORO_AUTH_TOKEN")]
        token: Option<String>,
        /// Channel name
        #[arg(long, default_value = "cli")]
        channel: String,
    },
    /// Start MCP server on stdio (for Claude Code integration)
    Mcp,
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
        Commands::Chat {
            message,
            url,
            token,
            channel,
        } => chat(&url, token.as_deref(), &channel, message.as_deref()).await?,
        Commands::Mcp => mcp_stdio(&cli.config).await?,
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

async fn mcp_stdio(config_path: &str) -> Result<()> {
    let cfg = config::load(config_path)?;
    let mem = Arc::new(memory::MemoryManager::new(&cfg.memory)?);
    let ctx = tools::ToolContext {
        memory: mem.clone(),
        base_dir: cfg.memory.base_dir.clone(),
    };
    let mut reg = tools::ToolRegistry::new(ctx);
    reg.add(ToolKind::SearchLogs);
    reg.add(ToolKind::ReadCoreMemory);
    reg.add(ToolKind::UpdateCoreMemory);
    reg.add(ToolKind::AppendLog);
    reg.add(ToolKind::ReadDailyLog);
    reg.add(ToolKind::WriteSummary);
    reg.add(ToolKind::ReadFile);
    if cfg.tools.shell_enabled {
        reg.add(ToolKind::Shell(Duration::from_secs(cfg.tools.shell_timeout)));
    }
    let reg = Arc::new(reg);

    let stdin = tokio::io::BufReader::new(tokio::io::stdin());
    let mut stdout = tokio::io::stdout();
    let mut lines = stdin.lines();

    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let req: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                let err = mcp::rpc_err(Value::Null, -32700, &format!("Parse error: {e}"));
                let mut buf = serde_json::to_vec(&err)?;
                buf.push(b'\n');
                stdout.write_all(&buf).await?;
                stdout.flush().await?;
                continue;
            }
        };
        // Notifications (no "id") — don't send a response
        if req.get("id").is_none() {
            continue;
        }
        let resp = mcp::handle_request(&reg, &cfg.agent.name, &req).await;
        let mut buf = serde_json::to_vec(&resp)?;
        buf.push(b'\n');
        stdout.write_all(&buf).await?;
        stdout.flush().await?;
    }
    Ok(())
}

async fn chat(url: &str, token: Option<&str>, channel: &str, message: Option<&str>) -> Result<()> {
    let client = reqwest::Client::new();
    if let Some(msg) = message {
        println!("{}", send_message(&client, url, token, channel, msg).await?);
        return Ok(());
    }
    // Interactive REPL
    let stdin = std::io::stdin();
    loop {
        eprint!("1koro> ");
        let mut line = String::new();
        if stdin.read_line(&mut line)? == 0 {
            break;
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if matches!(line, "exit" | "quit") {
            break;
        }
        match send_message(&client, url, token, channel, line).await {
            Ok(text) => println!("\n{text}\n"),
            Err(e) => eprintln!("Error: {e}"),
        }
    }
    Ok(())
}

async fn send_message(
    client: &reqwest::Client,
    url: &str,
    token: Option<&str>,
    channel: &str,
    text: &str,
) -> Result<String> {
    let mut req = client
        .post(format!("{url}/message"))
        .json(&serde_json::json!({"text": text, "channel": channel}));
    if let Some(t) = token {
        req = req.header("Authorization", format!("Bearer {t}"));
    }
    let resp = req.send().await?;
    if !resp.status().is_success() {
        anyhow::bail!("API error: {} {}", resp.status(), resp.text().await?);
    }
    let body: Value = resp.json().await?;
    Ok(body["text"]
        .as_str()
        .unwrap_or("(no response)")
        .to_string())
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
