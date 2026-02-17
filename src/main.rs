mod agent;
mod config;

mod channels;
mod llm;
mod memory;
mod scheduler;
mod tools;

use anyhow::Result;
use clap::{Parser, Subcommand};

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
    /// Start the agent
    Run,
    /// Initialize config directory
    Init,
    /// Show current status
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
            let config = config::load(&cli.config)?;
            let agent = agent::Agent::new(config).await?;
            agent.run().await?;
        }
        Commands::Status => {
            let config = config::load(&cli.config)?;
            let mem = memory::MemoryManager::new(&config.memory)?;
            let state = mem.read_core("state.md")?;
            println!("{state}");
        }
    }

    Ok(())
}
