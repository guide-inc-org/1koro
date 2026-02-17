use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

use crate::llm::Message;

#[derive(Debug, Serialize, Deserialize)]
pub struct Session {
    pub key: String,
    pub messages: Vec<Message>,
    #[serde(default)]
    pub summary: Option<String>,
    pub updated_at: DateTime<Local>,
}

pub struct SessionStore {
    base_dir: PathBuf,
    sessions: HashMap<String, Session>,
}

impl SessionStore {
    pub fn new(base_dir: PathBuf) -> Result<Self> {
        let sessions_dir = base_dir.join("sessions");
        std::fs::create_dir_all(&sessions_dir)?;

        let mut sessions = HashMap::new();
        for entry in std::fs::read_dir(&sessions_dir)?.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json") {
                let content = std::fs::read_to_string(&path)?;
                if let Ok(session) = serde_json::from_str::<Session>(&content) {
                    sessions.insert(session.key.clone(), session);
                }
            }
        }

        Ok(Self {
            base_dir,
            sessions,
        })
    }

    pub fn get_or_create(&mut self, key: &str) -> &mut Session {
        if !self.sessions.contains_key(key) {
            self.sessions.insert(
                key.to_string(),
                Session {
                    key: key.to_string(),
                    messages: Vec::new(),
                    summary: None,
                    updated_at: Local::now(),
                },
            );
        }
        self.sessions.get_mut(key).unwrap()
    }

    pub fn save(&self, key: &str) -> Result<()> {
        if let Some(session) = self.sessions.get(key) {
            let dir = self.base_dir.join("sessions");
            std::fs::create_dir_all(&dir)?;

            let safe_key = key.replace(':', "_").replace('/', "_");
            let path = dir.join(format!("{safe_key}.json"));
            let tmp = dir.join(format!("{safe_key}.json.tmp"));

            let json = serde_json::to_string_pretty(session)?;
            std::fs::write(&tmp, &json)?;
            std::fs::rename(&tmp, &path)?;
        }
        Ok(())
    }
}
