pub mod cli;
pub mod discord;
pub mod mcp;
pub mod slack;

use std::sync::Arc;

use anyhow::Result;

use crate::bus::{MessageBus, OutboundMessage};

#[async_trait::async_trait]
pub trait Channel: Send + Sync {
    fn name(&self) -> &str;
    async fn start(&self) -> Result<()>;
    async fn send(&self, msg: &OutboundMessage) -> Result<()>;
}

pub struct ChannelManager {
    bus: Arc<MessageBus>,
    channels: Vec<Box<dyn Channel>>,
}

impl ChannelManager {
    pub fn new(bus: Arc<MessageBus>) -> Self {
        Self {
            bus,
            channels: Vec::new(),
        }
    }

    pub fn register(&mut self, channel: Box<dyn Channel>) {
        self.channels.push(channel);
    }

    pub async fn start_all(&self) -> Result<()> {
        for ch in &self.channels {
            ch.start().await?;
            tracing::info!("Channel started: {}", ch.name());
        }
        Ok(())
    }

    /// Dispatch outbound messages to the right channel based on session_key prefix.
    pub async fn dispatch_loop(&self) -> Result<()> {
        let mut rx = self.bus.outbound_subscriber();
        loop {
            match rx.recv().await {
                Ok(msg) => {
                    let channel_name = msg.session_key.split(':').next().unwrap_or("");
                    for ch in &self.channels {
                        if ch.name() == channel_name {
                            if let Err(e) = ch.send(&msg).await {
                                tracing::error!("Failed to send to {}: {e}", ch.name());
                            }
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("Outbound bus lagged by {n} messages");
                }
                Err(_) => break,
            }
        }
        Ok(())
    }
}
