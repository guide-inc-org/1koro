mod agent;
mod bus;
mod channels;
mod config;
mod context;
mod llm;
mod memory;
mod scheduler;
mod session;
mod skills;
mod tools;

use std::sync::Arc;

use anyhow::Result;
use clap::{Parser, Subcommand};
use tokio::signal;

#[derive(Parser)]
#[command(name = "1koro", version, about = "Personal AI agent that never forgets 🥊")]
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

    // Message bus
    let bus = Arc::new(bus::MessageBus::new(100));

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
    tool_registry.register(Box::new(tools::file::ReadFileTool));

    // Skills
    let skill_loader = skills::SkillLoader::new(&cfg.memory.base_dir);
    let skill_summaries = skill_loader.load_summaries()?;
    if !skill_summaries.is_empty() {
        tracing::info!("Loaded {} skills", skill_summaries.len());
    }

    // Sessions
    let session_store = session::SessionStore::new(cfg.memory.base_dir.clone())?;

    // Agent
    let mut agent_instance = agent::Agent::new(
        bus.clone(),
        llm_client,
        mem.clone(),
        session_store,
        tool_registry,
        skill_summaries,
    );

    // Channel manager
    let mut channel_mgr = channels::ChannelManager::new(bus.clone());

    // CLI channel (always available)
    channel_mgr.register(Box::new(channels::cli::CliChannel::new(
        bus.inbound_sender(),
    )));

    // Configured channels
    if let Some(ref slack_cfg) = cfg.channels.slack {
        if slack_cfg.enabled {
            channel_mgr.register(Box::new(channels::slack::SlackChannel::new(
                slack_cfg.clone(),
                bus.inbound_sender(),
            )));
        }
    }
    if let Some(ref discord_cfg) = cfg.channels.discord {
        if discord_cfg.enabled {
            channel_mgr.register(Box::new(channels::discord::DiscordChannel::new(
                discord_cfg.clone(),
                bus.inbound_sender(),
            )));
        }
    }

    // Start channels
    channel_mgr.start_all().await?;

    tracing::info!("🥊 1koro is running. Type a message to chat.");

    // Run agent + dispatch concurrently
    let agent_handle = tokio::spawn(async move { agent_instance.run().await });

    let dispatch_handle = tokio::spawn(async move { channel_mgr.dispatch_loop().await });

    tokio::select! {
        r = agent_handle => {
            if let Err(e) = r { tracing::error!("Agent task error: {e}"); }
        }
        r = dispatch_handle => {
            if let Err(e) = r { tracing::error!("Dispatch task error: {e}"); }
        }
        _ = signal::ctrl_c() => {
            tracing::info!("\nShutting down...");
        }
    }

    Ok(())
}
