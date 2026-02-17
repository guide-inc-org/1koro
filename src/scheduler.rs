use anyhow::Result;

use crate::config::CronConfig;

pub struct Scheduler {
    _config: CronConfig,
}

impl Scheduler {
    pub fn new(config: CronConfig) -> Result<Self> {
        Ok(Self { _config: config })
    }

    pub async fn start(&self) -> Result<()> {
        tracing::info!("Cron scheduler: not yet implemented");
        Ok(())
    }
}
