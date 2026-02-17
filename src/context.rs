use anyhow::Result;
use chrono::{Datelike, Local};

use crate::llm::Message;
use crate::memory::MemoryManager;
use crate::session::Session;
use crate::skills::SkillSummary;

pub struct ContextBuilder;

impl ContextBuilder {
    pub fn build_messages(
        memory: &MemoryManager,
        session: &Session,
        user_text: &str,
        skills: &[SkillSummary],
    ) -> Result<Vec<Message>> {
        let mut messages = vec![Message::system(Self::build_system_prompt(memory, skills)?)];
        if let Some(summary) = &session.summary {
            messages.push(Message::system(format!("Previous conversation summary:\n{summary}")));
        }
        messages.extend(session.messages.iter().cloned());
        messages.push(Message::user(user_text));
        Ok(messages)
    }

    fn build_system_prompt(memory: &MemoryManager, skills: &[SkillSummary]) -> Result<String> {
        let mut p = String::new();

        // Core memory
        p.push_str(&memory.read_core("identity.md").unwrap_or_default());
        p.push_str("\n\n---\n\n");
        p.push_str(&memory.read_core("user.md").unwrap_or_default());
        p.push_str("\n\n---\n\n");
        p.push_str(&memory.read_core("state.md").unwrap_or_default());

        // Monthly summary
        let now = Local::now();
        if let Ok(Some(m)) = memory.read_monthly_summary(&now.format("%Y-%m").to_string()) {
            p.push_str("\n\n---\n\n# This Month\n\n");
            p.push_str(&m);
        }

        // Weekly summary
        let week_id = format!("{}-W{:02}", now.year(), now.iso_week().week());
        if let Ok(Some(w)) = memory.read_weekly_summary(&week_id) {
            p.push_str("\n\n---\n\n# This Week\n\n");
            p.push_str(&w);
        }

        p.push_str("\n\n---\n\nYou have access to tools. Use them to search memory, execute commands, or read files.\n");

        if !skills.is_empty() {
            p.push_str("\n# Available Skills\n\n");
            for s in skills {
                p.push_str(&format!("- **{}**: {} (use `read_file` to load: {})\n", s.name, s.description, s.path.display()));
            }
        }

        Ok(p)
    }
}
