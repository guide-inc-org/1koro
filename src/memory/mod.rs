pub mod core;
pub mod logs;
pub mod summary;

use anyhow::{Context, Result};
use std::path::PathBuf;

use crate::config::MemoryConfig;

pub struct MemoryManager {
    base_dir: PathBuf,
}

impl MemoryManager {
    pub fn new(config: &MemoryConfig) -> Result<Self> {
        Ok(Self {
            base_dir: config.base_dir.clone(),
        })
    }

    pub fn read_core(&self, filename: &str) -> Result<String> {
        let path = self.base_dir.join("core").join(filename);
        std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read core memory: {}", path.display()))
    }

    pub fn write_core(&self, filename: &str, content: &str) -> Result<()> {
        let path = self.base_dir.join("core").join(filename);
        std::fs::write(&path, content)
            .with_context(|| format!("Failed to write core memory: {}", path.display()))
    }

    pub fn base_dir(&self) -> &PathBuf {
        &self.base_dir
    }
}
