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

    /// Read daily log for a specific date (YYYY-MM-DD).
    pub fn read_daily_log(&self, date: &str) -> Result<Option<String>> {
        let path = self.base_dir.join(format!("logs/daily/{date}.md"));
        if path.exists() {
            Ok(Some(std::fs::read_to_string(path)?))
        } else {
            Ok(None)
        }
    }

    /// Read all daily logs in a date range (inclusive), returning (date, content) pairs.
    pub fn read_daily_logs_range(
        &self,
        start: &str,
        end: &str,
    ) -> Result<Vec<(String, String)>> {
        let logs_dir = self.base_dir.join("logs/daily");
        let mut results = Vec::new();

        if !logs_dir.exists() {
            return Ok(results);
        }

        let mut entries: Vec<_> = std::fs::read_dir(&logs_dir)?
            .filter_map(|e| e.ok())
            .collect();
        entries.sort_by_key(|e| e.file_name());

        for entry in entries {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "md") {
                let date = path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                if date.as_str() >= start && date.as_str() <= end {
                    let content = std::fs::read_to_string(&path)?;
                    results.push((date, content));
                }
            }
        }

        Ok(results)
    }

    /// Read heartbeat.md for periodic task instructions.
    pub fn read_heartbeat(&self) -> Result<Option<String>> {
        let path = self.base_dir.join("heartbeat.md");
        if path.exists() {
            Ok(Some(std::fs::read_to_string(path)?))
        } else {
            Ok(None)
        }
    }
}
