mod agent;
mod api;
mod channels;
mod config;
mod context;
mod llm;
mod memory;
mod session;
mod skills;
mod tools;

use std::sync::Arc;

use anyhow::Result;
use clap::{Parser, Subcommand};
use tokio::signal;
use tokio::sync::Mutex;

#[derive(Parser)]
#[command(name = "1koro", version, about = "Personal AI agent that never forgets")]
struct Cli {
    /// Path to config file
    #[arg(short, long, default_value = "~/.1koro/config.toml")]
    config: String,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the agent (default)
    Run,
    /// Initialize config directory (~/.1koro/)
    Init,
    /// Show current state from core memory
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
        Commands::Run => {
            run_agent(&cli.config).await?;
        }
        Commands::Status => {
            let cfg = config::load(&cli.config)?;
            let mem = memory::MemoryManager::new(&cfg.memory)?;
            let state = mem.read_core("state.md")?;
            println!("{state}");
        }
    }

    Ok(())
}

async fn run_agent(config_path: &str) -> Result<()> {
    let cfg = config::load(config_path)?;

    // Memory
    let mem = Arc::new(memory::MemoryManager::new(&cfg.memory)?);

    // LLM client
    let llm_client = llm::create_client(&cfg.llm)?;

    // Tool registry
    let tool_ctx = tools::ToolContext {
        memory: mem.clone(),
        base_dir: cfg.memory.base_dir.clone(),
    };
    let mut tool_registry = tools::ToolRegistry::new(tool_ctx);
    tool_registry.register(Box::new(tools::shell::ShellTool));
    tool_registry.register(Box::new(tools::memory::SearchLogsTool));
    tool_registry.register(Box::new(tools::memory::ReadCoreMemoryTool));
    tool_registry.register(Box::new(tools::memory::UpdateCoreMemoryTool));
    tool_registry.register(Box::new(tools::memory::AppendLogTool));
    tool_registry.register(Box::new(tools::memory::ReadDailyLogTool));
    tool_registry.register(Box::new(tools::memory::WriteSummaryTool));
    tool_registry.register(Box::new(tools::file::ReadFileTool));
    tool_registry.register(Box::new(tools::import::ImportDailyOpsTool));

    // Skills
    let skill_loader = skills::SkillLoader::new(&cfg.memory.base_dir);
    let skill_summaries = skill_loader.load_summaries()?;
    if !skill_summaries.is_empty() {
        tracing::info!("Loaded {} skills", skill_summaries.len());
    }

    // Sessions
    let session_store = session::SessionStore::new(cfg.memory.base_dir.clone())?;

    // Agent
    let agent = agent::Agent::new(
        llm_client,
        mem.clone(),
        session_store,
        tool_registry,
        skill_summaries,
    );

    // MCP server (separate port)
    channels::mcp::start(&cfg.mcp, mem.clone()).await?;

    // HTTP API server
    let state = api::AppState {
        agent: Arc::new(Mutex::new(agent)),
    };
    let app = api::router(state);

    let bind = &cfg.api.bind;
    let listener = tokio::net::TcpListener::bind(bind).await?;
    tracing::info!("1koro listening on {bind}");

    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            signal::ctrl_c().await.ok();
            tracing::info!("\nShutting down...");
        })
        .await?;

    Ok(())
}
