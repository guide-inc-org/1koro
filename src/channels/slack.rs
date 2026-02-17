use anyhow::Result;

use crate::config::SlackConfig;

pub struct SlackChannel {
    _config: SlackConfig,
}

impl SlackChannel {
    pub fn new(config: SlackConfig) -> Result<Self> {
        Ok(Self { _config: config })
    }

    pub async fn start(&self) -> Result<()> {
        tracing::info!("Slack channel: not yet implemented");
        Ok(())
    }
}
