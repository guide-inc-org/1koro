use std::sync::Arc;

use anyhow::Result;
use chrono::Local;

use crate::context::ContextBuilder;
use crate::llm::{LlmClient, Message};
use crate::memory::MemoryManager;
use crate::session::{Session, SessionStore};
use crate::skills::SkillSummary;
use crate::tools::ToolRegistry;

const MAX_TOOL_ITERATIONS: usize = 10;
const SESSION_COMPRESS_THRESHOLD: usize = 20;
const MAX_SUMMARY_LENGTH: usize = 2000;

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
        &self,
        text: &str,
        channel: &str,
        user: &str,
    ) -> Result<AgentResponse> {
        let key = format!("{channel}:{user}");
        tracing::info!("[{key}] {user}: {text}");

        // Hold per-session lock for the entire request to prevent message loss
        let session_lock = self.sessions.get_or_create(&key);
        let mut session = session_lock.lock().await;

        if session.messages.len() >= SESSION_COMPRESS_THRESHOLD {
            self.compress_session(&mut session).await?;
            self.sessions.save_to_disk(&key, &session)?;
        }

        // Persist user input immediately so it's never lost, even if tool_loop fails
        session.messages.push(Message::user(text));
        session.updated_at = Local::now();
        self.sessions.save_to_disk(&key, &session)?;

        if let Err(e) = self.memory.append_log(&format!("[{key}] {user}: {text}")) {
            tracing::warn!("Failed to append log: {e}");
        }

        let messages = ContextBuilder::build_messages(&self.memory, &session, &self.skills)?;
        let (response_text, new_messages) = self.tool_loop(messages).await?;

        session.messages.extend(new_messages);
        session.updated_at = Local::now();
        self.sessions.save_to_disk(&key, &session)?;

        drop(session); // Release session lock before logging

        if let Some(r) = &response_text
            && let Err(e) = self.memory.append_log(&format!("[{key}] 1koro: {r}"))
        {
            tracing::warn!("Failed to append log: {e}");
        }

        Ok(AgentResponse {
            text: response_text,
            actions: vec![],
        })
    }

    async fn compress_session(&self, session: &mut Session) -> Result<()> {
        let mid = session.messages.len() / 2;
        let mut input = String::new();
        for msg in &session.messages[..mid] {
            input.push_str(&format!(
                "{}: {}\n",
                msg.role,
                msg.content.as_deref().unwrap_or("[tool call]")
            ));
        }

        let msgs = vec![
            Message::system(
                "Summarize this conversation concisely. Key facts, decisions, context. Under 300 words.",
            ),
            Message::user(input),
        ];

        if let Ok(resp) = self.llm.chat(msgs, None).await {
            let new_summary = resp.content.unwrap_or_default();
            session.summary = Some(match &session.summary {
                Some(prev) => {
                    let combined = format!("{prev}\n\n{new_summary}");
                    if combined.len() > MAX_SUMMARY_LENGTH {
                        new_summary
                    } else {
                        combined
                    }
                }
                None => new_summary,
            });
            session.messages = session.messages[mid..].to_vec();
            tracing::info!("Compressed session {}", session.key);
        }
        Ok(())
    }

    async fn tool_loop(
        &self,
        mut messages: Vec<Message>,
    ) -> Result<(Option<String>, Vec<Message>)> {
        let defs = self.tools.tool_defs();
        let tools = if defs.is_empty() {
            None
        } else {
            Some(defs.as_slice())
        };
        let mut new = Vec::new();

        for _ in 0..MAX_TOOL_ITERATIONS {
            let resp = self.llm.chat(messages.clone(), tools).await?;
            if resp.tool_calls.is_empty() {
                if let Some(ref c) = resp.content {
                    new.push(Message::assistant(c));
                }
                return Ok((resp.content, new));
            }
            let asst =
                Message::assistant_with_tool_calls(resp.content.clone(), resp.tool_calls.clone());
            messages.push(asst.clone());
            new.push(asst);

            for tc in &resp.tool_calls {
                let result = match self
                    .tools
                    .execute(&tc.function.name, &tc.function.arguments)
                    .await
                {
                    Ok(r) => r.for_llm,
                    Err(e) => format!("Tool error: {e}"),
                };
                let msg = Message::tool_result(&tc.id, &result);
                messages.push(msg.clone());
                new.push(msg);
            }
        }
        Ok((Some("Tool use limit reached.".into()), new))
    }
}
