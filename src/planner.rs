//! The Planner: a small, fast model that reads the user's message and
//! available tools, then returns a JSON array of tool calls to execute
//! before the main Reasoner model is invoked.
//!
//! The planner is prompted to output *only* JSON — no prose, no markdown
//! fences. We strip any accidental wrapper and parse the array directly.
//!
//! If the planner decides no tools are needed, it returns `[]`.

use crate::{
    ai::{AiBackend, NimBackend, OllamaBackend, StreamChunk},
    config::{Backend, Config},
};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::mpsc;

// ── Output types ──────────────────────────────────────────────────────────────

/// A single planned tool call as returned by the planner model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedCall {
    /// Tool name — must match the executor's known tools.
    pub tool: String,
    /// Tool arguments as key→value pairs.
    pub args: HashMap<String, String>,
    /// One-line human-readable reason shown in the UI.
    pub reason: String,
}

// ── Tool catalogue (sent to the planner as context) ───────────────────────────

pub const TOOL_CATALOGUE: &str = r#"Available tools (use exact tool names):

read_file    – Read a file's contents.
               args: { "path": "<relative or absolute path>" }

write_file   – Write (create or overwrite) a file.
               args: { "path": "<path>", "content": "<full file text>" }

run_shell    – Run a shell command and return stdout+stderr.
               args: { "command": "<shell command>" }

list_dir     – List files and directories at a path.
               args: { "path": "<directory path>" }

web_search   – Search the web via DuckDuckGo instant answers.
               args: { "query": "<search query>" }
"#;

// ── Planner prompt ────────────────────────────────────────────────────────────

fn build_planner_prompt(
    user_message: &str,
    file_tree: &[String],
    conversation_summary: &str,
) -> String {
    let tree_str = if file_tree.is_empty() {
        "(no file tree available)".to_string()
    } else {
        file_tree.join("\n")
    };

    format!(
        r#"You are a planning agent. Your job is to decide which tools (if any) need to run BEFORE a coding assistant answers the user's message.

{TOOL_CATALOGUE}

Current working directory file tree:
{tree_str}

{conversation_summary}

User message:
{user_message}

Instructions:
- Output ONLY a JSON array of tool calls. No prose, no markdown, no explanation.
- If no tools are needed (the message is conversational, or you already have enough context), output exactly: []
- Do not schedule write_file unless the user explicitly asks to create or modify a file.
- Do not schedule run_shell unless the user explicitly asks to run something, build, or test.
- Prefer read_file and list_dir for exploration tasks.
- Limit to {MAX_PLANNED} tool calls maximum.
- Each element must have exactly: "tool", "args", "reason".

Examples:

User: "why is my main.rs failing to compile?"
Output: [{{"tool":"run_shell","args":{{"command":"cargo check 2>&1 | head -40"}},"reason":"Get compiler errors"}},{{"tool":"read_file","args":{{"path":"src/main.rs"}},"reason":"Read the failing file"}}]

User: "what files are in the src directory?"
Output: [{{"tool":"list_dir","args":{{"path":"src"}},"reason":"List src directory contents"}}]

User: "hello, how are you?"
Output: []

User: "what version of tokio should I use?"
Output: [{{"tool":"web_search","args":{{"query":"tokio crate latest version 2024"}},"reason":"Find current tokio version"}}]

Now output the JSON array for the user message above:"#,
        MAX_PLANNED = 5,
    )
}

fn build_conversation_summary(messages: &[crate::ai::Message]) -> String {
    // Give the planner a brief window of recent context (last 4 exchanges)
    // so it doesn't re-fetch files that were already read.
    let recent: Vec<_> = messages
        .iter()
        .rev()
        .take(8)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    if recent.is_empty() {
        return String::new();
    }

    let mut summary = String::from(
        "Recent conversation context (for reference — don't re-fetch what's already here):\n",
    );
    for msg in &recent {
        let role = match msg.role {
            crate::ai::Role::User => "User",
            crate::ai::Role::Assistant => "Assistant",
            crate::ai::Role::System => continue,
        };
        // Truncate long messages so the planner prompt stays compact
        let content = if msg.content.len() > 300 {
            format!("{}… (truncated)", &msg.content[..300])
        } else {
            msg.content.clone()
        };
        summary.push_str(&format!("[{role}]: {content}\n"));
    }
    summary
}

// ── Non-streaming call to the planner ─────────────────────────────────────────

/// Call the planner model and collect its full response (no streaming to UI).
async fn call_planner_model(prompt: &str, config: &Config) -> Result<String> {
    let messages = vec![crate::ai::Message::user(prompt.to_string())];

    let (tx, mut rx) = mpsc::unbounded_channel::<StreamChunk>();

    // Build a planner-specific backend
    match config.planner.backend {
        Backend::Ollama => {
            let backend =
                OllamaBackend::new(config.ollama.base_url.clone(), config.planner.model.clone());
            tokio::spawn(async move {
                let _ = backend.stream_chat(messages, tx).await;
            });
        }
        Backend::Nim => {
            let backend = NimBackend::new(
                config.nim.base_url.clone(),
                config.nim.api_key.clone(),
                config.planner.model.clone(),
            )?;
            tokio::spawn(async move {
                let _ = backend.stream_chat(messages, tx).await;
            });
        }
    }

    let mut full = String::new();
    while let Some(chunk) = rx.recv().await {
        match chunk {
            StreamChunk::Token(t) => full.push_str(&t),
            StreamChunk::Done => break,
            StreamChunk::Error(e) => return Err(anyhow::anyhow!("Planner error: {}", e)),
        }
    }

    Ok(full)
}

// ── JSON extraction + parse ───────────────────────────────────────────────────

/// Extract and parse the JSON array from the model's raw response.
/// Handles accidental markdown fences and leading/trailing prose.
fn extract_json_array(raw: &str) -> Result<Vec<PlannedCall>> {
    // Strip markdown code fences if present
    let stripped = raw
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    // Find the outermost [ … ] array
    let start = stripped
        .find('[')
        .ok_or_else(|| anyhow::anyhow!("No JSON array found in planner response"))?;
    let end = stripped
        .rfind(']')
        .ok_or_else(|| anyhow::anyhow!("Unclosed JSON array in planner response"))?;

    let json_slice = &stripped[start..=end];
    let calls: Vec<PlannedCall> = serde_json::from_str(json_slice)
        .map_err(|e| anyhow::anyhow!("Failed to parse planner JSON: {}\nRaw: {}", e, json_slice))?;

    Ok(calls)
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Run the planner and return a list of tool calls to execute.
/// Returns an empty vec if the planner decides no tools are needed.
/// On error (model unavailable, bad JSON, etc.) returns empty vec so the
/// agent falls through gracefully to the reasoner.
pub async fn plan(
    user_message: &str,
    messages: &[crate::ai::Message],
    file_tree: &[String],
    config: &Config,
) -> Vec<PlannedCall> {
    let summary = build_conversation_summary(messages);
    let prompt = build_planner_prompt(user_message, file_tree, &summary);

    let raw = match call_planner_model(&prompt, config).await {
        Ok(r) => r,
        Err(e) => {
            // Planner unavailable — log and continue without it
            eprintln!("[planner] model call failed: {e}");
            return vec![];
        }
    };

    match extract_json_array(&raw) {
        Ok(calls) => {
            // Enforce the per-batch limit from config
            calls
                .into_iter()
                .take(config.planner.max_tools_per_batch)
                .collect()
        }
        Err(e) => {
            eprintln!("[planner] JSON parse error: {e}\nRaw response: {raw}");
            vec![]
        }
    }
}
