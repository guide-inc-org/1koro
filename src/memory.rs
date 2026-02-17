use anyhow::{Context, Result};
use chrono::Local;
use std::path::PathBuf;

use crate::config::MemoryConfig;

pub struct MemoryManager {
    base_dir: PathBuf,
}

impl MemoryManager {
    pub fn new(config: &MemoryConfig) -> Result<Self> {
        Ok(Self { base_dir: config.base_dir.clone() })
    }

    // --- Core Memory ---

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

    // --- Daily Logs ---

    pub fn append_log(&self, entry: &str) -> Result<()> {
        let today = Local::now().format("%Y-%m-%d").to_string();
        let path = self.base_dir.join(format!("logs/daily/{today}.md"));
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut content = std::fs::read_to_string(&path).unwrap_or_default();
        if content.is_empty() {
            content = format!("# {today}\n\n");
        }
        content.push_str(&format!("- {entry}\n"));
        std::fs::write(&path, content)
            .with_context(|| format!("Failed to write log: {}", path.display()))
    }

    pub fn search_logs(&self, query: &str) -> Result<Vec<String>> {
        let logs_dir = self.base_dir.join("logs/daily");
        let mut results = Vec::new();
        if !logs_dir.exists() {
            return Ok(results);
        }
        let mut entries: Vec<_> = std::fs::read_dir(&logs_dir)?.filter_map(|e| e.ok()).collect();
        entries.sort_by_key(|e| e.file_name());
        let query_lower = query.to_lowercase();
        for entry in entries {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "md") {
                let content = std::fs::read_to_string(&path)?;
                for line in content.lines() {
                    if line.to_lowercase().contains(&query_lower) {
                        let date = path.file_stem().unwrap_or_default().to_string_lossy();
                        results.push(format!("[{date}] {line}"));
                    }
                }
            }
        }
        Ok(results)
    }

    pub fn read_daily_log(&self, date: &str) -> Result<Option<String>> {
        let path = self.base_dir.join(format!("logs/daily/{date}.md"));
        if path.exists() {
            Ok(Some(std::fs::read_to_string(path)?))
        } else {
            Ok(None)
        }
    }

    // --- Summaries ---

    pub fn read_weekly_summary(&self, week_id: &str) -> Result<Option<String>> {
        let path = self.base_dir.join(format!("logs/weekly/{week_id}.md"));
        if path.exists() { Ok(Some(std::fs::read_to_string(path)?)) } else { Ok(None) }
    }

    pub fn read_monthly_summary(&self, month_id: &str) -> Result<Option<String>> {
        let path = self.base_dir.join(format!("logs/monthly/{month_id}.md"));
        if path.exists() { Ok(Some(std::fs::read_to_string(path)?)) } else { Ok(None) }
    }

    pub fn write_weekly_summary(&self, week_id: &str, content: &str) -> Result<()> {
        let path = self.base_dir.join(format!("logs/weekly/{week_id}.md"));
        if let Some(p) = path.parent() { std::fs::create_dir_all(p)?; }
        Ok(std::fs::write(path, content)?)
    }

    pub fn write_monthly_summary(&self, month_id: &str, content: &str) -> Result<()> {
        let path = self.base_dir.join(format!("logs/monthly/{month_id}.md"));
        if let Some(p) = path.parent() { std::fs::create_dir_all(p)?; }
        Ok(std::fs::write(path, content)?)
    }
}
