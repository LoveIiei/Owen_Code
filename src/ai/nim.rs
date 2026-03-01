use super::{AiBackend, Message, Role, StreamChunk};
use anyhow::Result;
use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::{header, Client};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

pub struct NimBackend {
    client: Client,
    base_url: String,
    model: String,
}

impl NimBackend {
    pub fn new(base_url: String, api_key: String, model: String) -> Result<Self> {
        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            header::HeaderValue::from_str(&format!("Bearer {}", api_key))?,
        );
        headers.insert(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("application/json"),
        );

        let client = Client::builder().default_headers(headers).build()?;

        Ok(Self {
            client,
            base_url,
            model,
        })
    }
}

// ---------- OpenAI-compatible request/response types ----------

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
    max_tokens: u32,
    temperature: f32,
}

#[derive(Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct StreamChunkData {
    choices: Vec<StreamChoice>,
}

#[derive(Deserialize)]
struct StreamChoice {
    delta: DeltaContent,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct DeltaContent {
    content: Option<String>,
}

#[derive(Deserialize)]
struct ModelList {
    data: Vec<ModelData>,
}

#[derive(Deserialize)]
struct ModelData {
    id: String,
}

fn role_str(role: &Role) -> &'static str {
    match role {
        Role::User => "user",
        Role::Assistant => "assistant",
        Role::System => "system",
    }
}

#[async_trait]
impl AiBackend for NimBackend {
    fn name(&self) -> &str {
        "Nvidia NIM"
    }

    fn model(&self) -> &str {
        &self.model
    }

    async fn list_models(&self) -> Result<Vec<String>> {
        let url = format!("{}/models", self.base_url);
        let resp: ModelList = self.client.get(&url).send().await?.json().await?;
        Ok(resp.data.into_iter().map(|m| m.id).collect())
    }

    async fn stream_chat(
        &self,
        messages: Vec<Message>,
        tx: mpsc::UnboundedSender<StreamChunk>,
    ) -> Result<()> {
        let url = format!("{}/chat/completions", self.base_url);

        let body = ChatRequest {
            model: self.model.clone(),
            stream: true,
            max_tokens: 16384,
            temperature: 0.7,
            messages: messages
                .iter()
                .map(|m| ChatMessage {
                    role: role_str(&m.role).to_string(),
                    content: m.content.clone(),
                })
                .collect(),
        };

        let response = self.client.post(&url).json(&body).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let err = response.text().await.unwrap_or_default();
            let _ = tx.send(StreamChunk::Error(format!("NIM error {}: {}", status, err)));
            return Ok(());
        }

        let mut stream = response.bytes_stream();
        let mut buffer = String::new();

        while let Some(chunk) = stream.next().await {
            let bytes = match chunk {
                Ok(b) => b,
                Err(e) => {
                    let _ = tx.send(StreamChunk::Error(e.to_string()));
                    break;
                }
            };

            buffer.push_str(&String::from_utf8_lossy(&bytes));

            // Process complete SSE lines
            while let Some(pos) = buffer.find('\n') {
                let line = buffer[..pos].trim().to_string();
                buffer = buffer[pos + 1..].to_string();

                if line.is_empty() || line.starts_with(':') {
                    continue;
                }

                let data = if let Some(d) = line.strip_prefix("data: ") {
                    d
                } else {
                    continue;
                };

                if data == "[DONE]" {
                    let _ = tx.send(StreamChunk::Done);
                    return Ok(());
                }

                match serde_json::from_str::<StreamChunkData>(data) {
                    Ok(chunk) => {
                        for choice in chunk.choices {
                            if let Some(content) = choice.delta.content {
                                if !content.is_empty() {
                                    let _ = tx.send(StreamChunk::Token(content));
                                }
                            }
                            if choice.finish_reason.is_some() {
                                let _ = tx.send(StreamChunk::Done);
                                return Ok(());
                            }
                        }
                    }
                    Err(_) => {}
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
