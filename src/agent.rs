use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use chrono::{Datelike, Local};

use crate::llm::{LlmClient, Message};
use crate::memory::MemoryManager;
use crate::session::{Session, SessionStore};
use crate::tools::ToolRegistry;

const MAX_TOOL_ITERATIONS: usize = 10;
const SESSION_COMPRESS_THRESHOLD: usize = 20;
const MAX_SUMMARY_LENGTH: usize = 2000;

// --- Skills (merged from skills.rs) ---

pub struct SkillSummary {
    pub name: String,
    pub description: String,
    pub path: PathBuf,
}

pub fn load_skills(base_dir: &Path) -> Result<Vec<SkillSummary>> {
    let dir = base_dir.join("skills");
    let mut skills = Vec::new();
    if !dir.exists() {
        return Ok(skills);
    }
    for entry in std::fs::read_dir(&dir)?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let skill_file = path.join("SKILL.md");
            if skill_file.exists() {
                let content = std::fs::read_to_string(&skill_file)?;
                let name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let description = content
                    .lines()
                    .find(|l| !l.starts_with('#') && !l.trim().is_empty())
                    .unwrap_or("")
                    .to_string();
                skills.push(SkillSummary {
                    name,
                    description,
                    path: skill_file,
                });
            }
        }
    }
    skills.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(skills)
}

// --- Context building (merged from context.rs) ---

fn build_system_prompt(memory: &MemoryManager, skills: &[SkillSummary]) -> Result<String> {
    let mut p = String::new();
    p.push_str(&memory.read_core("identity.md").unwrap_or_default());
    p.push_str("\n\n---\n\n");
    p.push_str(&memory.read_core("user.md").unwrap_or_default());
    p.push_str("\n\n---\n\n");
    p.push_str(&memory.read_core("state.md").unwrap_or_default());

    let now = Local::now();
    if let Ok(Some(m)) = memory.read_monthly_summary(&now.format("%Y-%m").to_string()) {
        p.push_str("\n\n---\n\n# This Month\n\n");
        p.push_str(&m);
    }
    let week_id = format!("{}-W{:02}", now.year(), now.iso_week().week());
    if let Ok(Some(w)) = memory.read_weekly_summary(&week_id) {
        p.push_str("\n\n---\n\n# This Week\n\n");
        p.push_str(&w);
    }

    p.push_str("\n\n---\n\nYou have access to tools. Use them to search memory, execute commands, or read files.\n");
    if !skills.is_empty() {
        p.push_str("\n# Available Skills\n\n");
        for s in skills {
            p.push_str(&format!(
                "- **{}**: {} (use `read_file` to load: {})\n",
                s.name,
                s.description,
                s.path.display()
            ));
        }
    }
    Ok(p)
}

fn build_messages(
    memory: &MemoryManager,
    session: &Session,
    skills: &[SkillSummary],
) -> Result<Vec<Message>> {
    let mut messages = vec![Message::system(build_system_prompt(memory, skills)?)];
    if let Some(summary) = &session.summary {
        messages.push(Message::system(format!(
            "Previous conversation summary:\n{summary}"
        )));
    }
    messages.extend(session.messages.iter().cloned());
    Ok(messages)
}

// --- Agent ---

pub struct AgentResponse {
    pub text: Option<String>,
    pub actions: Vec<serde_json::Value>,
}

pub struct Agent {
    llm: Arc<LlmClient>,
    memory: Arc<MemoryManager>,
    sessions: SessionStore,
    tools: ToolRegistry,
    skills: Vec<SkillSummary>,
}

impl Agent {
    pub fn new(
        llm: Arc<LlmClient>,
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

        let session_lock = self.sessions.get_or_create(&key);
        let mut session = session_lock.lock().await;

        if session.messages.len() >= SESSION_COMPRESS_THRESHOLD {
            self.compress_session(&mut session).await?;
            self.sessions.save_to_disk(&key, &session)?;
        }

        session.messages.push(Message::user(text));
        session.updated_at = Local::now();
        self.sessions.save_to_disk(&key, &session)?;

        if let Err(e) = self.memory.append_log(&format!("[{key}] {user}: {text}")) {
            tracing::warn!("Failed to append log: {e}");
        }

        let messages = build_messages(&self.memory, &session, &self.skills)?;
        let (response_text, new_messages) = self.tool_loop(messages).await?;

        session.messages.extend(new_messages);
        session.updated_at = Local::now();
        self.sessions.save_to_disk(&key, &session)?;
        drop(session);

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
