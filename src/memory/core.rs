use anyhow::Result;

use super::MemoryManager;

impl MemoryManager {
    /// Build the core memory block for LLM system prompt injection.
    pub fn build_core_context(&self) -> Result<String> {
        let identity = self.read_core("identity.md").unwrap_or_default();
        let user = self.read_core("user.md").unwrap_or_default();
        let state = self.read_core("state.md").unwrap_or_default();

        Ok(format!(
            "{identity}\n\n---\n\n{user}\n\n---\n\n{state}"
        ))
    }

    /// Update state.md.
    pub fn update_state(&self, content: &str) -> Result<()> {
        self.write_core("state.md", content)
    }
}
