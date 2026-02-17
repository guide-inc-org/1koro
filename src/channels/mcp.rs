use anyhow::Result;

use super::Channel;
use crate::bus::OutboundMessage;
use crate::config::McpConfig;

pub struct McpChannel {
    _config: McpConfig,
}

impl McpChannel {
    pub fn new(config: McpConfig) -> Self {
        Self { _config: config }
    }
}

#[async_trait::async_trait]
impl Channel for McpChannel {
    fn name(&self) -> &str {
        "mcp"
    }

    async fn start(&self) -> Result<()> {
        // TODO: Implement MCP server with rmcp
        // Bind to Headscale VPN IP only
        tracing::info!("MCP server: not yet implemented");
        Ok(())
    }

    async fn send(&self, _msg: &OutboundMessage) -> Result<()> {
        // MCP is request/response, not push-based
        Ok(())
    }
}
