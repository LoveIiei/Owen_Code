use super::{AiBackend, Message, Role, StreamChunk};
use anyhow::Result;
use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

pub struct OllamaBackend {
    client: Client,
    base_url: String,
    model: String,
}

impl OllamaBackend {
    pub fn new(base_url: String, model: String) -> Self {
        Self {
            client: Client::new(),
            base_url,
            model,
        }
    }
}

// ---------- Ollama request/response types ----------

#[derive(Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    stream: bool,
}

#[derive(Serialize, Deserialize)]
struct OllamaMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct OllamaChatChunk {
    message: Option<OllamaMessage>,
    done: bool,
}

#[derive(Deserialize)]
struct OllamaModelList {
    models: Vec<OllamaModel>,
}

#[derive(Deserialize)]
struct OllamaModel {
    name: String,
}

fn role_str(role: &Role) -> &'static str {
    match role {
        Role::User => "user",
        Role::Assistant => "assistant",
        Role::System => "system",
    }
}

#[async_trait]
impl AiBackend for OllamaBackend {
    fn name(&self) -> &str {
        "Ollama"
    }

    fn model(&self) -> &str {
        &self.model
    }

    async fn list_models(&self) -> Result<Vec<String>> {
        let url = format!("{}/api/tags", self.base_url);
        let resp: OllamaModelList = self.client.get(&url).send().await?.json().await?;
        Ok(resp.models.into_iter().map(|m| m.name).collect())
    }

    async fn stream_chat(
        &self,
        messages: Vec<Message>,
        tx: mpsc::UnboundedSender<StreamChunk>,
    ) -> Result<()> {
        let url = format!("{}/api/chat", self.base_url);

        let body = OllamaChatRequest {
            model: self.model.clone(),
            stream: true,
            messages: messages
                .iter()
                .map(|m| OllamaMessage {
                    role: role_str(&m.role).to_string(),
                    content: m.content.clone(),
                })
                .collect(),
        };

        let response = self.client.post(&url).json(&body).send().await?;

        if !response.status().is_success() {
            let err = response.text().await.unwrap_or_default();
            let _ = tx.send(StreamChunk::Error(format!("Ollama error: {}", err)));
            return Ok(());
        }

        let mut stream = response.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let bytes = match chunk {
                Ok(b) => b,
                Err(e) => {
                    let _ = tx.send(StreamChunk::Error(e.to_string()));
                    break;
                }
            };

            // Ollama sends one JSON object per line
            for line in bytes.split(|&b| b == b'\n') {
                if line.is_empty() {
                    continue;
                }
                match serde_json::from_slice::<OllamaChatChunk>(line) {
                    Ok(chunk) => {
                        if let Some(msg) = chunk.message {
                            if !msg.content.is_empty() {
                                let _ = tx.send(StreamChunk::Token(msg.content));
                            }
                        }
                        if chunk.done {
                            let _ = tx.send(StreamChunk::Done);
                            return Ok(());
                        }
                    }
                    Err(_) => {} // skip malformed lines
                }
            }
        }

        let _ = tx.send(StreamChunk::Done);
        Ok(())
    }

    async fn set_model(&mut self, model: String) {
        self.model = model;
    }
}
