use anyhow::Result;

use crate::config::McpConfig;

pub struct McpChannel {
    _config: McpConfig,
}

impl McpChannel {
    pub fn new(config: McpConfig) -> Result<Self> {
        Ok(Self { _config: config })
    }

    pub async fn start(&self) -> Result<()> {
        tracing::info!("MCP server: not yet implemented");
        Ok(())
    }
}
