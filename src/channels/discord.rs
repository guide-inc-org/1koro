use anyhow::Result;

use crate::config::DiscordConfig;

pub struct DiscordChannel {
    _config: DiscordConfig,
}

impl DiscordChannel {
    pub fn new(config: DiscordConfig) -> Result<Self> {
        Ok(Self { _config: config })
    }

    pub async fn start(&self) -> Result<()> {
        tracing::info!("Discord channel: not yet implemented");
        Ok(())
    }
}
