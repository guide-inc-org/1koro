use std::path::PathBuf;

use anyhow::Result;
use tokio::sync::mpsc;

use crate::bus::InboundMessage;
use crate::config::CronConfig;

pub struct Scheduler {
    _config: CronConfig,
    _bus_tx: mpsc::Sender<InboundMessage>,
    _base_dir: PathBuf,
}

impl Scheduler {
    pub fn new(
        config: CronConfig,
        bus_tx: mpsc::Sender<InboundMessage>,
        base_dir: PathBuf,
    ) -> Self {
        Self {
            _config: config,
            _bus_tx: bus_tx,
            _base_dir: base_dir,
        }
    }

    pub async fn start(&self) -> Result<()> {
        // TODO: Implement with tokio-cron-scheduler
        // - Read heartbeat.md for periodic tasks
        // - Execute cron jobs by sending InboundMessage to bus
        // - Daily/weekly summary generation
        tracing::info!("Scheduler: not yet implemented");
        Ok(())
    }
}
