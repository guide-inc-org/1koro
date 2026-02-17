use anyhow::Result;
use tokio::sync::mpsc;

use super::Channel;
use crate::bus::{InboundMessage, OutboundMessage};
use crate::config::SlackConfig;

pub struct SlackChannel {
    _config: SlackConfig,
    _bus_tx: mpsc::Sender<InboundMessage>,
}

impl SlackChannel {
    pub fn new(config: SlackConfig, bus_tx: mpsc::Sender<InboundMessage>) -> Self {
        Self {
            _config: config,
            _bus_tx: bus_tx,
        }
    }
}

#[async_trait::async_trait]
impl Channel for SlackChannel {
    fn name(&self) -> &str {
        "slack"
    }

    async fn start(&self) -> Result<()> {
        // TODO: Implement Slack Socket Mode with slack-morphism
        tracing::info!("Slack channel: not yet implemented");
        Ok(())
    }

    async fn send(&self, msg: &OutboundMessage) -> Result<()> {
        // TODO: Send message to Slack channel/thread
        tracing::info!("Slack would send to {}: {}", msg.session_key, msg.text);
        Ok(())
    }
}
