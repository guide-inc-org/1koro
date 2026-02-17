use anyhow::Result;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;

use super::Channel;
use crate::bus::{InboundMessage, OutboundMessage};

/// CLI channel for local testing via stdin/stdout.
pub struct CliChannel {
    bus_tx: mpsc::Sender<InboundMessage>,
}

impl CliChannel {
    pub fn new(bus_tx: mpsc::Sender<InboundMessage>) -> Self {
        Self { bus_tx }
    }
}

#[async_trait::async_trait]
impl Channel for CliChannel {
    fn name(&self) -> &str {
        "cli"
    }

    async fn start(&self) -> Result<()> {
        let tx = self.bus_tx.clone();
        tokio::spawn(async move {
            let stdin = BufReader::new(tokio::io::stdin());
            let mut lines = stdin.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if line.trim().is_empty() {
                    continue;
                }
                let _ = tx
                    .send(InboundMessage {
                        session_key: "cli:default".to_string(),
                        channel_name: "cli".to_string(),
                        user_id: "user".to_string(),
                        user_name: "User".to_string(),
                        text: line,
                    })
                    .await;
            }
        });
        Ok(())
    }

    async fn send(&self, msg: &OutboundMessage) -> Result<()> {
        println!("\n🥊 {}\n", msg.text);
        Ok(())
    }
}
