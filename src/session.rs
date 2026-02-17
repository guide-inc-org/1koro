use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Mutex;

use anyhow::Result;
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

use crate::llm::Message;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub key: String,
    pub messages: Vec<Message>,
    #[serde(default)]
    pub summary: Option<String>,
    pub updated_at: DateTime<Local>,
}

pub struct SessionStore {
    base_dir: PathBuf,
    sessions: Mutex<HashMap<String, Session>>,
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
                    let key = session.key.clone();
                    sessions.entry(key)
                        .and_modify(|existing: &mut Session| {
                            if session.updated_at > existing.updated_at {
                                *existing = session.clone();
                            }
                        })
                        .or_insert(session);
                }
            }
        }

        Ok(Self { base_dir, sessions: Mutex::new(sessions) })
    }

    pub fn get_or_create(&self, key: &str) -> Session {
        let mut sessions = self.sessions.lock().expect("session lock poisoned");
        sessions.entry(key.to_string())
            .or_insert_with(|| Session {
                key: key.to_string(),
                messages: Vec::new(),
                summary: None,
                updated_at: Local::now(),
            })
            .clone()
    }

    pub fn update_and_save(&self, key: &str, session: Session) -> Result<()> {
        let mut sessions = self.sessions.lock().expect("session lock poisoned");
        sessions.insert(key.to_string(), session);
        let session = sessions.get(key).expect("just inserted");
        self.save_to_disk(key, session)
    }

    fn save_to_disk(&self, key: &str, session: &Session) -> Result<()> {
        let dir = self.base_dir.join("sessions");
        std::fs::create_dir_all(&dir)?;

        let filename = Self::session_filename(key);
        let path = dir.join(format!("{filename}.json"));
        let tmp = dir.join(format!("{filename}.json.tmp"));

        let json = serde_json::to_string_pretty(session)?;
        std::fs::write(&tmp, &json)?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }

    fn session_filename(key: &str) -> String {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_filename_uniqueness() {
        let a = SessionStore::session_filename("slack:user1");
        let b = SessionStore::session_filename("slack_user1");
        assert_ne!(a, b, "different keys must produce different filenames");
    }

    #[test]
    fn test_session_filename_deterministic() {
        let a = SessionStore::session_filename("test:key");
        let b = SessionStore::session_filename("test:key");
        assert_eq!(a, b);
    }
}
