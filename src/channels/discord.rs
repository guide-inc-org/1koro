use anyhow::Result;
use tokio::sync::mpsc;

use super::Channel;
use crate::bus::{InboundMessage, OutboundMessage};
use crate::config::DiscordConfig;

pub struct DiscordChannel {
    _config: DiscordConfig,
    _bus_tx: mpsc::Sender<InboundMessage>,
}

impl DiscordChannel {
    pub fn new(config: DiscordConfig, bus_tx: mpsc::Sender<InboundMessage>) -> Self {
        Self {
            _config: config,
            _bus_tx: bus_tx,
        }
    }
}

#[async_trait::async_trait]
impl Channel for DiscordChannel {
    fn name(&self) -> &str {
        "discord"
    }

    async fn start(&self) -> Result<()> {
        // TODO: Implement Discord bot with serenity
        tracing::info!("Discord channel: not yet implemented");
        Ok(())
    }

    async fn send(&self, msg: &OutboundMessage) -> Result<()> {
        tracing::info!("Discord would send to {}: {}", msg.session_key, msg.text);
        Ok(())
    }
}
