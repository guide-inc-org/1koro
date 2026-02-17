use std::sync::Arc;

use anyhow::Result;
use chrono::Local;

use crate::context::ContextBuilder;
use crate::llm::{LlmClient, Message};
use crate::memory::MemoryManager;
use crate::session::SessionStore;
use crate::skills::SkillSummary;
use crate::tools::ToolRegistry;

const MAX_TOOL_ITERATIONS: usize = 10;
const SESSION_COMPRESS_THRESHOLD: usize = 20;

pub struct AgentResponse {
    pub text: Option<String>,
    pub actions: Vec<serde_json::Value>,
}

pub struct Agent {
    llm: Arc<dyn LlmClient>,
    memory: Arc<MemoryManager>,
    sessions: SessionStore,
    tools: ToolRegistry,
    skills: Vec<SkillSummary>,
}

impl Agent {
    pub fn new(
        llm: Arc<dyn LlmClient>,
        memory: Arc<MemoryManager>,
        sessions: SessionStore,
        tools: ToolRegistry,
        skills: Vec<SkillSummary>,
    ) -> Self {
        Self {
            llm,
            memory,
            sessions,
            tools,
            skills,
        }
    }

    pub async fn handle_message(
        &mut self,
        text: &str,
        channel: &str,
        user: &str,
    ) -> Result<AgentResponse> {
        let session_key = format!("{channel}:{user}");

        tracing::info!("[{session_key}] {user}: {text}");

        // Compress session if needed
        self.maybe_compress_session(&session_key).await?;

        // Build context
        let session = self.sessions.get_or_create(&session_key);
        let messages =
            ContextBuilder::build_messages(&self.memory, session, text, &self.skills)?;

        // Run tool loop
        let (response_text, new_messages) = self.run_tool_loop(messages).await?;

        // Update session
        let session = self.sessions.get_or_create(&session_key);
        session.messages.push(Message::user(text));
        session.messages.extend(new_messages);
        session.updated_at = Local::now();
        self.sessions.save(&session_key)?;

        // Log conversation
        let _ = self
            .memory
            .append_log(&format!("[{session_key}] {user}: {text}"));
        if let Some(ref resp) = response_text {
            let _ = self
                .memory
                .append_log(&format!("[{session_key}] 1koro: {resp}"));
        }

        Ok(AgentResponse {
            text: response_text,
            actions: vec![],
        })
    }

    async fn maybe_compress_session(&mut self, session_key: &str) -> Result<()> {
        let session = self.sessions.get_or_create(session_key);
        if session.messages.len() < SESSION_COMPRESS_THRESHOLD {
            return Ok(());
        }

        tracing::info!(
            "Compressing session {session_key} ({} messages)",
            session.messages.len()
        );

        let mid = session.messages.len() / 2;
        let old_messages = &session.messages[..mid];

        let mut summary_input = String::new();
        for msg in old_messages {
            let role = &msg.role;
            let content = msg.content.as_deref().unwrap_or("[tool call]");
            summary_input.push_str(&format!("{role}: {content}\n"));
        }

        let messages = vec![
            Message::system(
                "Summarize the following conversation concisely. \
                 Capture key facts, decisions, and context. Keep it under 300 words.",
            ),
            Message::user(summary_input),
        ];

        match self.llm.chat(messages, None).await {
            Ok(response) => {
                let session = self.sessions.get_or_create(session_key);
                let new_summary = response.content.unwrap_or_default();

                session.summary = Some(match &session.summary {
                    Some(existing) => format!("{existing}\n\n{new_summary}"),
                    None => new_summary,
                });

                session.messages = session.messages[mid..].to_vec();
                self.sessions.save(session_key)?;
                tracing::info!("Session {session_key} compressed");
            }
            Err(e) => {
                tracing::error!("Session compression failed: {e}");
            }
        }

        Ok(())
    }

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
                if let Some(ref content) = response.content {
                    new_messages.push(Message::assistant(content));
                }
                return Ok((response.content, new_messages));
            }

            let assistant_msg = Message::assistant_with_tool_calls(
                response.content.clone(),
                response.tool_calls.clone(),
            );
            messages.push(assistant_msg.clone());
            new_messages.push(assistant_msg);

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
