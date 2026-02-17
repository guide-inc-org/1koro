use anyhow::{Context, Result};
use chrono::Local;
use std::path::PathBuf;

use super::MemoryManager;

impl MemoryManager {
    /// Append a log entry to today's daily log.
    pub fn append_log(&self, entry: &str) -> Result<()> {
        let today = Local::now().format("%Y-%m-%d").to_string();
        let path = self.log_path(&today);

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut content = std::fs::read_to_string(&path).unwrap_or_default();
        if content.is_empty() {
            content = format!("# {today}\n\n");
        }
        content.push_str(&format!("- {}\n", entry));

        std::fs::write(&path, content)
            .with_context(|| format!("Failed to write log: {}", path.display()))
    }

    /// Search logs for a query string. Returns matching lines with dates.
    pub fn search_logs(&self, query: &str) -> Result<Vec<String>> {
        let logs_dir = self.base_dir.join("logs/daily");
        let mut results = Vec::new();

        if !logs_dir.exists() {
            return Ok(results);
        }

        let mut entries: Vec<_> = std::fs::read_dir(&logs_dir)?
            .filter_map(|e| e.ok())
            .collect();
        entries.sort_by_key(|e| e.file_name());

        let query_lower = query.to_lowercase();
        for entry in entries {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "md") {
                let content = std::fs::read_to_string(&path)?;
                for line in content.lines() {
                    if line.to_lowercase().contains(&query_lower) {
                        let date = path
                            .file_stem()
                            .unwrap_or_default()
                            .to_string_lossy();
                        results.push(format!("[{date}] {line}"));
                    }
                }
            }
        }

        Ok(results)
    }

    fn log_path(&self, date: &str) -> PathBuf {
        self.base_dir.join(format!("logs/daily/{date}.md"))
    }
}
