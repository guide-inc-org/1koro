pub mod discord;
pub mod mcp;
pub mod slack;

#[derive(Debug)]
pub struct IncomingMessage {
    pub channel: ChannelType,
    pub user_id: String,
    pub user_name: String,
    pub text: String,
}

#[derive(Debug)]
pub struct OutgoingMessage {
    pub text: String,
}

#[derive(Debug, Clone)]
pub enum ChannelType {
    Slack,
    Discord,
    Mcp,
}
