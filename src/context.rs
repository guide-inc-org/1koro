use anyhow::Result;

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
        let mut messages = Vec::new();

        // System prompt: core memory + skills
        let system = Self::build_system_prompt(memory, skills)?;
        messages.push(Message::system(system));

        // Session summary (compressed older history)
        if let Some(summary) = &session.summary {
            messages.push(Message::system(format!(
                "Previous conversation summary:\n{summary}"
            )));
        }

        // Recent session messages
        messages.extend(session.messages.iter().cloned());

        // New user message
        messages.push(Message::user(user_text));

        Ok(messages)
    }

    fn build_system_prompt(
        memory: &MemoryManager,
        skills: &[SkillSummary],
    ) -> Result<String> {
        let mut prompt = String::new();

        // Core memory
        let identity = memory.read_core("identity.md").unwrap_or_default();
        let user = memory.read_core("user.md").unwrap_or_default();
        let state = memory.read_core("state.md").unwrap_or_default();

        prompt.push_str(&identity);
        prompt.push_str("\n\n---\n\n");
        prompt.push_str(&user);
        prompt.push_str("\n\n---\n\n");
        prompt.push_str(&state);

        // Tools usage hint
        prompt.push_str("\n\n---\n\n");
        prompt.push_str("You have access to tools. Use them when needed to answer questions, ");
        prompt.push_str("search memory, execute commands, or read files.\n");

        // Skill summaries (lazy loading pattern from PicoClaw)
        if !skills.is_empty() {
            prompt.push_str("\n# Available Skills\n\n");
            for skill in skills {
                prompt.push_str(&format!(
                    "- **{}**: {} (use `read_file` to load: {})\n",
                    skill.name,
                    skill.description,
                    skill.path.display()
                ));
            }
        }

        Ok(prompt)
    }
}
