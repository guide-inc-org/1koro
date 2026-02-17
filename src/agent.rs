use anyhow::Result;
use tokio::signal;

use crate::config::Config;
use crate::memory::MemoryManager;

pub struct Agent {
    config: Config,
    memory: MemoryManager,
}

impl Agent {
    pub async fn new(config: Config) -> Result<Self> {
        let memory = MemoryManager::new(&config.memory)?;
        Ok(Self { config, memory })
    }

    pub async fn run(&self) -> Result<()> {
        tracing::info!("Starting 1koro agent: {}", self.config.agent.name);

        // Load core memory
        let identity = self.memory.read_core("identity.md")?;
        let user = self.memory.read_core("user.md")?;
        let state = self.memory.read_core("state.md")?;
        tracing::info!("Core memory loaded (3 files)");
        tracing::debug!(
            "Identity: {}bytes, User: {}bytes, State: {}bytes",
            identity.len(),
            user.len(),
            state.len()
        );

        // TODO: Start channel listeners (Slack, Discord)
        // TODO: Start MCP server
        // TODO: Start cron scheduler

        tracing::info!("1koro is running. Press Ctrl+C to stop.");

        signal::ctrl_c().await?;
        tracing::info!("Shutting down...");

        Ok(())
    }
}
