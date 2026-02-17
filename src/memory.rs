use anyhow::{Context, Result, bail};
use chrono::Local;
use std::path::PathBuf;

use crate::config::MemoryConfig;

const CORE_FILES: &[&str] = &["identity.md", "user.md", "state.md"];
const WRITABLE_CORE_FILES: &[&str] = &["user.md", "state.md"];

pub struct MemoryManager {
    base_dir: PathBuf,
}

impl MemoryManager {
    pub fn new(config: &MemoryConfig) -> Result<Self> {
        Ok(Self {
            base_dir: config.base_dir.clone(),
        })
    }

    // --- Validation ---

    fn validate_core_read(filename: &str) -> Result<()> {
        if CORE_FILES.contains(&filename) {
            Ok(())
        } else {
            bail!("Invalid core memory file: {filename}")
        }
    }

    fn validate_core_write(filename: &str) -> Result<()> {
        if WRITABLE_CORE_FILES.contains(&filename) {
            Ok(())
        } else {
            bail!("Cannot write to core memory file: {filename}")
        }
    }

    fn validate_date(date: &str) -> Result<()> {
        let b = date.as_bytes();
        if b.len() != 10
            || b[4] != b'-'
            || b[7] != b'-'
            || !b[..4].iter().all(u8::is_ascii_digit)
            || !b[5..7].iter().all(u8::is_ascii_digit)
            || !b[8..10].iter().all(u8::is_ascii_digit)
        {
            bail!("Invalid date format (expected YYYY-MM-DD): {date}");
        }
        let month: u32 = date[5..7].parse().unwrap_or(0);
        let day: u32 = date[8..10].parse().unwrap_or(0);
        if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
            bail!("Invalid date value: {date}");
        }
        Ok(())
    }

    fn validate_week_id(id: &str) -> Result<()> {
        let b = id.as_bytes();
        if b.len() != 8
            || b[4] != b'-'
            || b[5] != b'W'
            || !b[..4].iter().all(u8::is_ascii_digit)
            || !b[6..8].iter().all(u8::is_ascii_digit)
        {
            bail!("Invalid week id (expected YYYY-Wnn): {id}");
        }
        let week: u32 = id[6..8].parse().unwrap_or(0);
        if !(1..=53).contains(&week) {
            bail!("Invalid week number: {id}");
        }
        Ok(())
    }

    fn validate_month_id(id: &str) -> Result<()> {
        let b = id.as_bytes();
        if b.len() != 7
            || b[4] != b'-'
            || !b[..4].iter().all(u8::is_ascii_digit)
            || !b[5..7].iter().all(u8::is_ascii_digit)
        {
            bail!("Invalid month id (expected YYYY-MM): {id}");
        }
        let month: u32 = id[5..7].parse().unwrap_or(0);
        if !(1..=12).contains(&month) {
            bail!("Invalid month number: {id}");
        }
        Ok(())
    }

    // --- Core Memory ---

    pub fn read_core(&self, filename: &str) -> Result<String> {
        Self::validate_core_read(filename)?;
        let path = self.base_dir.join("core").join(filename);
        std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read core memory: {}", path.display()))
    }

    pub fn write_core(&self, filename: &str, content: &str) -> Result<()> {
        Self::validate_core_write(filename)?;
        let path = self.base_dir.join("core").join(filename);
        std::fs::write(&path, content)
            .with_context(|| format!("Failed to write core memory: {}", path.display()))
    }

    // --- Daily Logs ---

    /// Append a log entry using O_APPEND for atomic writes.
    /// Concurrent appends are safe — both writes will be preserved.
    pub fn append_log(&self, entry: &str) -> Result<()> {
        use std::io::Write;

        let today = Local::now().format("%Y-%m-%d").to_string();
        let path = self.base_dir.join(format!("logs/daily/{today}.md"));
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .with_context(|| format!("Failed to open log: {}", path.display()))?;

        if file.metadata()?.len() == 0 {
            writeln!(file, "# {today}\n")?;
        }
        writeln!(file, "- {entry}")
            .with_context(|| format!("Failed to append to log: {}", path.display()))
    }

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
                        let date = path.file_stem().unwrap_or_default().to_string_lossy();
                        results.push(format!("[{date}] {line}"));
                    }
                }
            }
        }
        Ok(results)
    }

    pub fn read_daily_log(&self, date: &str) -> Result<Option<String>> {
        Self::validate_date(date)?;
        let path = self.base_dir.join(format!("logs/daily/{date}.md"));
        if path.exists() {
            Ok(Some(std::fs::read_to_string(path)?))
        } else {
            Ok(None)
        }
    }

    // --- Summaries ---

    pub fn read_weekly_summary(&self, week_id: &str) -> Result<Option<String>> {
        Self::validate_week_id(week_id)?;
        let path = self.base_dir.join(format!("logs/weekly/{week_id}.md"));
        if path.exists() {
            Ok(Some(std::fs::read_to_string(path)?))
        } else {
            Ok(None)
        }
    }

    pub fn read_monthly_summary(&self, month_id: &str) -> Result<Option<String>> {
        Self::validate_month_id(month_id)?;
        let path = self.base_dir.join(format!("logs/monthly/{month_id}.md"));
        if path.exists() {
            Ok(Some(std::fs::read_to_string(path)?))
        } else {
            Ok(None)
        }
    }

    pub fn write_weekly_summary(&self, week_id: &str, content: &str) -> Result<()> {
        Self::validate_week_id(week_id)?;
        let path = self.base_dir.join(format!("logs/weekly/{week_id}.md"));
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p)?;
        }
        Ok(std::fs::write(path, content)?)
    }

    pub fn write_monthly_summary(&self, month_id: &str, content: &str) -> Result<()> {
        Self::validate_month_id(month_id)?;
        let path = self.base_dir.join(format!("logs/monthly/{month_id}.md"));
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p)?;
        }
        Ok(std::fs::write(path, content)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_core_read() {
        assert!(MemoryManager::validate_core_read("identity.md").is_ok());
        assert!(MemoryManager::validate_core_read("user.md").is_ok());
        assert!(MemoryManager::validate_core_read("state.md").is_ok());
        assert!(MemoryManager::validate_core_read("../etc/passwd").is_err());
        assert!(MemoryManager::validate_core_read("../../secret").is_err());
        assert!(MemoryManager::validate_core_read("").is_err());
    }

    #[test]
    fn test_validate_core_write() {
        assert!(MemoryManager::validate_core_write("user.md").is_ok());
        assert!(MemoryManager::validate_core_write("state.md").is_ok());
        assert!(MemoryManager::validate_core_write("identity.md").is_err());
        assert!(MemoryManager::validate_core_write("../hack.md").is_err());
    }

    #[test]
    fn test_validate_date() {
        assert!(MemoryManager::validate_date("2026-02-17").is_ok());
        assert!(MemoryManager::validate_date("2000-01-01").is_ok());
        assert!(MemoryManager::validate_date("../../../etc").is_err());
        assert!(MemoryManager::validate_date("2026-2-17").is_err());
        assert!(MemoryManager::validate_date("").is_err());
        assert!(MemoryManager::validate_date("2026-02-17/../../x").is_err());
        // Range checks
        assert!(MemoryManager::validate_date("2026-00-01").is_err());
        assert!(MemoryManager::validate_date("2026-13-01").is_err());
        assert!(MemoryManager::validate_date("2026-01-00").is_err());
        assert!(MemoryManager::validate_date("2026-01-32").is_err());
        assert!(MemoryManager::validate_date("2026-99-99").is_err());
    }

    #[test]
    fn test_validate_week_id() {
        assert!(MemoryManager::validate_week_id("2026-W08").is_ok());
        assert!(MemoryManager::validate_week_id("2026-W52").is_ok());
        assert!(MemoryManager::validate_week_id("2026-W53").is_ok());
        assert!(MemoryManager::validate_week_id("../W08").is_err());
        assert!(MemoryManager::validate_week_id("2026-08").is_err());
        assert!(MemoryManager::validate_week_id("").is_err());
        // Range checks
        assert!(MemoryManager::validate_week_id("2026-W00").is_err());
        assert!(MemoryManager::validate_week_id("2026-W54").is_err());
    }

    #[test]
    fn test_validate_month_id() {
        assert!(MemoryManager::validate_month_id("2026-02").is_ok());
        assert!(MemoryManager::validate_month_id("2000-12").is_ok());
        assert!(MemoryManager::validate_month_id("../../xx").is_err());
        assert!(MemoryManager::validate_month_id("2026-2").is_err());
        assert!(MemoryManager::validate_month_id("").is_err());
        // Range checks
        assert!(MemoryManager::validate_month_id("2026-00").is_err());
        assert!(MemoryManager::validate_month_id("2026-13").is_err());
    }
}
