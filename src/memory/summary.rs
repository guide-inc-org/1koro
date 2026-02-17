use anyhow::Result;

use super::MemoryManager;

impl MemoryManager {
    pub fn read_weekly_summary(&self, week_id: &str) -> Result<Option<String>> {
        let path = self.base_dir.join(format!("logs/weekly/{week_id}.md"));
        if path.exists() {
            Ok(Some(std::fs::read_to_string(path)?))
        } else {
            Ok(None)
        }
    }

    pub fn read_monthly_summary(&self, month_id: &str) -> Result<Option<String>> {
        let path = self.base_dir.join(format!("logs/monthly/{month_id}.md"));
        if path.exists() {
            Ok(Some(std::fs::read_to_string(path)?))
        } else {
            Ok(None)
        }
    }

    pub fn write_weekly_summary(&self, week_id: &str, content: &str) -> Result<()> {
        let path = self.base_dir.join(format!("logs/weekly/{week_id}.md"));
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, content)?;
        Ok(())
    }

    pub fn write_monthly_summary(&self, month_id: &str, content: &str) -> Result<()> {
        let path = self.base_dir.join(format!("logs/monthly/{month_id}.md"));
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, content)?;
        Ok(())
    }
}
