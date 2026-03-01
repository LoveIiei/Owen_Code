use crate::ai::{Message, Role};
use crate::app::ChatEntry;
use anyhow::Result;
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub name: String,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
    pub working_dir: String,
    pub backend: String,
    pub model: String,
    pub messages: Vec<SerializedMessage>,
    pub chat_log: Vec<SerializedChatEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedChatEntry {
    pub role: String,
    pub content: String,
    pub timestamp: DateTime<Local>,
}

impl Session {
    pub fn new(
        name: String,
        working_dir: String,
        backend: String,
        model: String,
        messages: &[Message],
        chat_log: &[ChatEntry],
    ) -> Self {
        let now = Local::now();
        let id = now.format("%Y%m%d_%H%M%S").to_string();

        Self {
            id,
            name,
            created_at: now,
            updated_at: now,
            working_dir,
            backend,
            model,
            messages: messages
                .iter()
                .map(|m| SerializedMessage {
                    role: role_to_str(&m.role).to_string(),
                    content: m.content.clone(),
                })
                .collect(),
            chat_log: chat_log
                .iter()
                .map(|e| SerializedChatEntry {
                    role: role_to_str(&e.role).to_string(),
                    content: e.content.clone(),
                    timestamp: e.timestamp,
                })
                .collect(),
        }
    }

    pub fn to_messages(&self) -> Vec<Message> {
        self.messages
            .iter()
            .map(|m| Message {
                role: str_to_role(&m.role),
                content: m.content.clone(),
            })
            .collect()
    }

    pub fn to_chat_log(&self) -> Vec<ChatEntry> {
        self.chat_log
            .iter()
            .map(|e| ChatEntry {
                role: str_to_role(&e.role),
                content: e.content.clone(),
                timestamp: e.timestamp,
            })
            .collect()
    }
}

fn role_to_str(role: &Role) -> &'static str {
    match role {
        Role::User => "user",
        Role::Assistant => "assistant",
        Role::System => "system",
    }
}

fn str_to_role(s: &str) -> Role {
    match s {
        "user" => Role::User,
        "assistant" => Role::Assistant,
        _ => Role::System,
    }
}

pub struct SessionStore;

impl SessionStore {
    pub fn sessions_dir() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("aicode")
            .join("sessions")
    }

    pub fn save(session: &Session) -> Result<PathBuf> {
        let dir = Self::sessions_dir();
        std::fs::create_dir_all(&dir)?;

        let path = dir.join(format!("{}.json", session.id));
        let json = serde_json::to_string_pretty(session)?;
        std::fs::write(&path, json)?;
        Ok(path)
    }

    pub fn list() -> Result<Vec<SessionMeta>> {
        let dir = Self::sessions_dir();
        if !dir.exists() {
            return Ok(vec![]);
        }

        let mut sessions = Vec::new();
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            if let Ok(contents) = std::fs::read_to_string(&path) {
                if let Ok(session) = serde_json::from_str::<Session>(&contents) {
                    sessions.push(SessionMeta {
                        id: session.id,
                        name: session.name,
                        updated_at: session.updated_at,
                        model: session.model,
                        message_count: session.chat_log.len(),
                    });
                }
            }
        }

        // Sort by most recent first
        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(sessions)
    }

    pub fn load(id: &str) -> Result<Session> {
        let path = Self::sessions_dir().join(format!("{}.json", id));
        let contents = std::fs::read_to_string(&path)?;
        let session = serde_json::from_str(&contents)?;
        Ok(session)
    }

    pub fn delete(id: &str) -> Result<()> {
        let path = Self::sessions_dir().join(format!("{}.json", id));
        std::fs::remove_file(path)?;
        Ok(())
    }

    /// Auto-save with a fixed "last" ID for seamless resume on next launch
    pub fn autosave(session: &mut Session) -> Result<()> {
        let dir = Self::sessions_dir();
        std::fs::create_dir_all(&dir)?;

        session.updated_at = Local::now();

        // Save with real ID
        let json = serde_json::to_string_pretty(&session)?;
        std::fs::write(dir.join(format!("{}.json", session.id)), &json)?;

        // Also write a "last" symlink/copy for auto-resume
        std::fs::write(dir.join("_last.json"), json)?;
        Ok(())
    }

    pub fn load_last() -> Option<Session> {
        let path = Self::sessions_dir().join("_last.json");
        std::fs::read_to_string(path)
            .ok()
            .and_then(|c| serde_json::from_str(&c).ok())
    }
}

#[derive(Debug, Clone)]
pub struct SessionMeta {
    pub id: String,
    pub name: String,
    pub updated_at: DateTime<Local>,
    pub model: String,
    pub message_count: usize,
}
