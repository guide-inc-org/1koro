use std::sync::Arc;

use anyhow::Result;
use chrono::Local;

use crate::bus::{InboundMessage, MessageBus, OutboundMessage};
use crate::context::ContextBuilder;
use crate::llm::{LlmClient, Message};
use crate::memory::MemoryManager;
use crate::session::SessionStore;
use crate::skills::SkillSummary;
use crate::tools::ToolRegistry;

const MAX_TOOL_ITERATIONS: usize = 10;

pub struct Agent {
    bus: Arc<MessageBus>,
    llm: Box<dyn LlmClient>,
    memory: Arc<MemoryManager>,
    sessions: SessionStore,
    tools: ToolRegistry,
    skills: Vec<SkillSummary>,
}

impl Agent {
    pub fn new(
        bus: Arc<MessageBus>,
        llm: Box<dyn LlmClient>,
        memory: Arc<MemoryManager>,
        sessions: SessionStore,
        tools: ToolRegistry,
        skills: Vec<SkillSummary>,
    ) -> Self {
        Self {
            bus,
            llm,
            memory,
            sessions,
            tools,
            skills,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        loop {
            let msg = match self.bus.recv_inbound().await {
                Some(msg) => msg,
                None => break,
            };

            if let Err(e) = self.handle_message(msg).await {
                tracing::error!("Error handling message: {e}");
            }
        }

        Ok(())
    }

    async fn handle_message(&mut self, msg: InboundMessage) -> Result<()> {
        tracing::info!(
            "[{}] {}: {}",
            msg.session_key,
            msg.user_name,
            msg.text
        );

        // Build context
        let session = self.sessions.get_or_create(&msg.session_key);
        let messages =
            ContextBuilder::build_messages(&self.memory, session, &msg.text, &self.skills)?;

        // Run tool loop
        let (response_text, new_messages) = self.run_tool_loop(messages).await?;

        // Update session with user message + agent messages
        let session = self.sessions.get_or_create(&msg.session_key);
        session.messages.push(Message::user(&msg.text));
        session.messages.extend(new_messages);
        session.updated_at = Local::now();

        // Persist session
        self.sessions.save(&msg.session_key)?;

        // Log conversation
        let _ = self.memory.append_log(&format!(
            "[{}] {}: {}",
            msg.session_key, msg.user_name, msg.text
        ));
        if let Some(ref text) = response_text {
            let _ = self
                .memory
                .append_log(&format!("[{}] 1koro: {}", msg.session_key, text));
        }

        // Send response
        if let Some(text) = response_text {
            self.bus.send_outbound(OutboundMessage {
                session_key: msg.session_key,
                text,
            });
        }

        Ok(())
    }

    /// Core tool loop: call LLM, execute tool calls, repeat until final text response.
    async fn run_tool_loop(
        &self,
        mut messages: Vec<Message>,
    ) -> Result<(Option<String>, Vec<Message>)> {
        let tool_defs = self.tools.tool_defs();
        let tools_arg = if tool_defs.is_empty() {
            None
        } else {
            Some(tool_defs.as_slice())
        };
        let mut new_messages = Vec::new();

        for _ in 0..MAX_TOOL_ITERATIONS {
            let response = self.llm.chat(messages.clone(), tools_arg).await?;

            if response.tool_calls.is_empty() {
                // Final response — no more tool calls
                if let Some(ref content) = response.content {
                    new_messages.push(Message::assistant(content));
                }
                return Ok((response.content, new_messages));
            }

            // Assistant message with tool calls
            let assistant_msg = Message::assistant_with_tool_calls(
                response.content.clone(),
                response.tool_calls.clone(),
            );
            messages.push(assistant_msg.clone());
            new_messages.push(assistant_msg);

            // Execute each tool
            for tc in &response.tool_calls {
                tracing::debug!("Tool call: {}({})", tc.function.name, tc.function.arguments);

                let result = self
                    .tools
                    .execute(&tc.function.name, &tc.function.arguments)
                    .await;

                let result_text = match result {
                    Ok(r) => r.for_llm,
                    Err(e) => format!("Tool error: {e}"),
                };

                let tool_msg = Message::tool_result(&tc.id, &result_text);
                messages.push(tool_msg.clone());
                new_messages.push(tool_msg);
            }
        }

        Ok((
            Some("I've reached my tool use limit for this turn.".to_string()),
            new_messages,
        ))
    }
}
