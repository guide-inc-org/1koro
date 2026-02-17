use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

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

/// Per-session locking: concurrent requests for different sessions run in parallel,
/// same-session requests are serialized to prevent message loss.
pub struct SessionStore {
    base_dir: PathBuf,
    sessions: Mutex<HashMap<String, Arc<tokio::sync::Mutex<Session>>>>,
}

impl SessionStore {
    pub fn new(base_dir: PathBuf) -> Result<Self> {
        let sessions_dir = base_dir.join("sessions");
        std::fs::create_dir_all(&sessions_dir)?;

        let mut loaded: HashMap<String, Session> = HashMap::new();
        for entry in std::fs::read_dir(&sessions_dir)?.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json") {
                let content = std::fs::read_to_string(&path)?;
                if let Ok(session) = serde_json::from_str::<Session>(&content) {
                    let key = session.key.clone();
                    loaded
                        .entry(key)
                        .and_modify(|existing| {
                            if session.updated_at > existing.updated_at {
                                *existing = session.clone();
                            }
                        })
                        .or_insert(session);
                }
            }
        }

        let sessions: HashMap<String, Arc<tokio::sync::Mutex<Session>>> = loaded
            .into_iter()
            .map(|(k, v)| (k, Arc::new(tokio::sync::Mutex::new(v))))
            .collect();

        Ok(Self {
            base_dir,
            sessions: Mutex::new(sessions),
        })
    }

    /// Returns a per-session lock. Caller should hold this across the entire
    /// request lifecycle to prevent concurrent message loss.
    pub fn get_or_create(&self, key: &str) -> Arc<tokio::sync::Mutex<Session>> {
        let mut map = self.sessions.lock().expect("session map lock poisoned");
        map.entry(key.to_string())
            .or_insert_with(|| {
                Arc::new(tokio::sync::Mutex::new(Session {
                    key: key.to_string(),
                    messages: Vec::new(),
                    summary: None,
                    updated_at: Local::now(),
                }))
            })
            .clone()
    }

    /// Save session to disk. Does not acquire the session map lock.
    pub fn save_to_disk(&self, key: &str, session: &Session) -> Result<()> {
        let dir = self.base_dir.join("sessions");
        std::fs::create_dir_all(&dir)?;

        let filename = Self::session_filename(key);
        let path = dir.join(format!("{filename}.json"));
        let tmp = dir.join(format!("{filename}.{}.json.tmp", std::process::id()));

        let json = serde_json::to_string_pretty(session)?;
        std::fs::write(&tmp, &json)?;
        if let Err(e) = std::fs::rename(&tmp, &path) {
            let _ = std::fs::remove_file(&tmp);
            return Err(e.into());
        }
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
