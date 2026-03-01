pub mod nim;
pub mod ollama;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

impl Message {
    pub fn user(content: impl Into<String>) -> Self {
        Self { role: Role::User, content: content.into() }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self { role: Role::Assistant, content: content.into() }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self { role: Role::System, content: content.into() }
    }
}

/// A chunk streamed back from the AI backend
#[derive(Debug, Clone)]
pub enum StreamChunk {
    Token(String),
    Done,
    Error(String),
}

#[async_trait]
pub trait AiBackend: Send + Sync {
    fn name(&self) -> &str;
    fn model(&self) -> &str;

    async fn list_models(&self) -> Result<Vec<String>>;

    async fn stream_chat(
        &self,
        messages: Vec<Message>,
        tx: mpsc::UnboundedSender<StreamChunk>,
    ) -> Result<()>;

    async fn set_model(&mut self, model: String);
}

pub use nim::NimBackend;
pub use ollama::OllamaBackend;
