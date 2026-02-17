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
        Self { llm, memory, sessions, tools, skills }
    }

    pub async fn handle_message(&self, text: &str, channel: &str, user: &str) -> Result<AgentResponse> {
        let key = format!("{channel}:{user}");
        tracing::info!("[{key}] {user}: {text}");

        self.maybe_compress(&key).await?;

        let session = self.sessions.get_or_create(&key);
        let messages = ContextBuilder::build_messages(&self.memory, &session, text, &self.skills)?;
        let (response_text, new_messages) = self.tool_loop(messages).await?;

        let mut session = self.sessions.get_or_create(&key);
        session.messages.push(Message::user(text));
        session.messages.extend(new_messages);
        session.updated_at = Local::now();
        self.sessions.update_and_save(&key, session)?;

        if let Err(e) = self.memory.append_log(&format!("[{key}] {user}: {text}")) {
            tracing::warn!("Failed to append log: {e}");
        }
        if let Some(ref r) = response_text {
            if let Err(e) = self.memory.append_log(&format!("[{key}] 1koro: {r}")) {
                tracing::warn!("Failed to append log: {e}");
            }
        }

        Ok(AgentResponse { text: response_text, actions: vec![] })
    }

    async fn maybe_compress(&self, key: &str) -> Result<()> {
        let session = self.sessions.get_or_create(key);
        if session.messages.len() < SESSION_COMPRESS_THRESHOLD { return Ok(()); }

        let mid = session.messages.len() / 2;
        let mut input = String::new();
        for msg in &session.messages[..mid] {
            input.push_str(&format!("{}: {}\n", msg.role, msg.content.as_deref().unwrap_or("[tool call]")));
        }

        let msgs = vec![
            Message::system("Summarize this conversation concisely. Key facts, decisions, context. Under 300 words."),
            Message::user(input),
        ];

        if let Ok(resp) = self.llm.chat(msgs, None).await {
            let mut session = self.sessions.get_or_create(key);
            let new_summary = resp.content.unwrap_or_default();
            session.summary = Some(match &session.summary {
                Some(prev) => {
                    let combined = format!("{prev}\n\n{new_summary}");
                    if combined.len() > MAX_SUMMARY_LENGTH { new_summary } else { combined }
                }
                None => new_summary,
            });
            session.messages = session.messages[mid..].to_vec();
            self.sessions.update_and_save(key, session)?;
            tracing::info!("Compressed session {key}");
        }
        Ok(())
    }

    async fn tool_loop(&self, mut messages: Vec<Message>) -> Result<(Option<String>, Vec<Message>)> {
        let defs = self.tools.tool_defs();
        let tools = if defs.is_empty() { None } else { Some(defs.as_slice()) };
        let mut new = Vec::new();

        for _ in 0..MAX_TOOL_ITERATIONS {
            let resp = self.llm.chat(messages.clone(), tools).await?;
            if resp.tool_calls.is_empty() {
                if let Some(ref c) = resp.content { new.push(Message::assistant(c)); }
                return Ok((resp.content, new));
            }
            let asst = Message::assistant_with_tool_calls(resp.content.clone(), resp.tool_calls.clone());
            messages.push(asst.clone());
            new.push(asst);

            for tc in &resp.tool_calls {
                let result = match self.tools.execute(&tc.function.name, &tc.function.arguments).await {
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
