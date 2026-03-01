//! Agentic pipeline:
//!
//!  1. PLANNER  — small/fast model returns JSON [{tool,args,reason}]
//!  2. EXECUTOR — runs planned tools in parallel, collects results
//!  3. REASONER — main model gets message + results, streams response,
//!                may emit XML tool calls for reactive follow-up (hybrid)
//!  4. Repeat EXECUTOR → REASONER for reactive calls (≤ MAX_REACTIVE rounds)

use crate::{
    ai::{AiBackend, Message, NimBackend, OllamaBackend, StreamChunk},
    config::{Backend, Config},
    planner::{self, PlannedCall},
    tools::{FileTool, ShellTool, WebSearch},
};
use anyhow::Result;
use std::collections::HashMap;
use tokio::sync::mpsc;

// ── Public types ──────────────────────────────────────────────────────────────

/// A tool call — used for both planned (JSON) and reactive (XML) calls.
#[derive(Debug, Clone)]
pub struct ToolCall {
    pub name: String,
    pub args: HashMap<String, String>,
}

impl From<PlannedCall> for ToolCall {
    fn from(p: PlannedCall) -> Self {
        Self { name: p.tool, args: p.args }
    }
}

/// A completed agent step shown as a card in the chat history.
#[derive(Debug, Clone)]
pub struct AgentStep {
    pub iteration: u32,
    pub tool: String,
    pub summary: String,
    pub output: String,
    pub success: bool,
}

/// Events the agent sends back to the UI/app.
#[derive(Debug, Clone)]
pub enum AgentEvent {
    /// Planner is running (no streaming — just a status update).
    PlannerStarted,
    /// Planner returned a plan (may be empty).
    PlannerDone { calls: Vec<PlannedCall> },
    /// A tool is about to be executed.
    ToolStart { iteration: u32, name: String, summary: String },
    /// A tool finished executing.
    ToolDone { iteration: u32, name: String, output: String, success: bool },
    /// Reasoner started streaming.
    ReasonerStarted,
    /// Streamed token from the reasoner.
    Token(String),
    /// Reasoner finished a turn (full text for parsing).
    TurnComplete(String),
    /// No more work to do — pipeline finished.
    Done,
    /// Unrecoverable error.
    Error(String),
}

// ── Tool execution ────────────────────────────────────────────────────────────

pub async fn execute_tool(call: &ToolCall, working_dir: &str) -> (String, bool) {
    let resolve = |path: &str| -> String {
        if path.starts_with('/') { path.to_string() }
        else { format!("{}/{}", working_dir, path) }
    };

    match call.name.as_str() {
        "read_file" => {
            let path = call.args.get("path").map(|s| s.as_str()).unwrap_or("");
            if path.is_empty() { return ("Error: missing path arg".into(), false); }
            match FileTool::read(&resolve(path)).await {
                Ok(r) => (r.output, r.success),
                Err(e) => (format!("Error: {e}"), false),
            }
        }
        "write_file" => {
            let path = call.args.get("path").map(|s| s.as_str()).unwrap_or("");
            let content = call.args.get("content").map(|s| s.as_str()).unwrap_or("");
            if path.is_empty() { return ("Error: missing path arg".into(), false); }
            match FileTool::write(&resolve(path), content).await {
                Ok(r) => (r.output, r.success),
                Err(e) => (format!("Error: {e}"), false),
            }
        }
        "run_shell" | "shell" | "bash" => {
            let cmd = call.args.get("command").or_else(|| call.args.get("cmd"))
                .map(|s| s.as_str()).unwrap_or("");
            if cmd.is_empty() { return ("Error: missing command arg".into(), false); }
            match ShellTool::execute(cmd, Some(working_dir)).await {
                Ok(r) => (r.output, r.success),
                Err(e) => (format!("Error: {e}"), false),
            }
        }
        "list_dir" | "list_directory" => {
            let path = call.args.get("path").map(|s| s.as_str()).unwrap_or(".");
            let full = if path.starts_with('/') { path.to_string() }
                       else { format!("{}/{}", working_dir, path) };
            match FileTool::list_directory(&full).await {
                Ok(r) => (r.output, r.success),
                Err(e) => (format!("Error: {e}"), false),
            }
        }
        "web_search" | "search" => {
            let query = call.args.get("query").or_else(|| call.args.get("q"))
                .map(|s| s.as_str()).unwrap_or("");
            if query.is_empty() { return ("Error: missing query arg".into(), false); }
            match WebSearch::search(query).await {
                Ok(r) => (r, true),
                Err(e) => (format!("Search error: {e}"), false),
            }
        }
        unknown => (format!("Unknown tool: {unknown}"), false),
    }
}

/// Execute a batch of tool calls in parallel, preserving order in results.
async fn execute_batch(
    calls: &[ToolCall],
    working_dir: &str,
    iteration: u32,
    event_tx: &mpsc::UnboundedSender<AgentEvent>,
) -> Vec<(String, String, bool)> {
    // Fire ToolStart events for all calls
    for call in calls {
        let _ = event_tx.send(AgentEvent::ToolStart {
            iteration,
            name: call.name.clone(),
            summary: tool_summary(call),
        });
    }

    // Run in parallel
    let futures: Vec<_> = calls
        .iter()
        .map(|call| {
            let call = call.clone();
            let wd = working_dir.to_string();
            async move { execute_tool(&call, &wd).await }
        })
        .collect();

    let results = futures::future::join_all(futures).await;

    // Fire ToolDone events and collect (name, output, success) triples
    let mut out = Vec::new();
    for (call, (output, success)) in calls.iter().zip(results) {
        let _ = event_tx.send(AgentEvent::ToolDone {
            iteration,
            name: call.name.clone(),
            output: output.clone(),
            success,
        });
        out.push((call.name.clone(), output, success));
    }
    out
}

pub fn tool_summary(call: &ToolCall) -> String {
    match call.name.as_str() {
        "read_file" => format!("read {}", call.args.get("path").map(|s| s.as_str()).unwrap_or("?")),
        "write_file" => format!("write {}", call.args.get("path").map(|s| s.as_str()).unwrap_or("?")),
        "run_shell" | "shell" | "bash" => {
            let cmd = call.args.get("command").or_else(|| call.args.get("cmd"))
                .map(|s| s.as_str()).unwrap_or("?");
            format!("$ {}", if cmd.len() > 60 { &cmd[..60] } else { cmd })
        }
        "list_dir" | "list_directory" => {
            format!("ls {}", call.args.get("path").map(|s| s.as_str()).unwrap_or("."))
        }
        "web_search" | "search" => {
            format!("search: {}", call.args.get("query").map(|s| s.as_str()).unwrap_or("?"))
        }
        other => other.to_string(),
    }
}

// ── XML reactive tool call parsing (for reasoner follow-up) ──────────────────

pub fn parse_tool_calls(text: &str) -> Vec<ToolCall> {
    let mut calls = Vec::new();
    let mut search = text;

    while let Some(start) = search.find("<tool_call") {
        let rest = &search[start..];
        let Some(tag_end) = rest.find('>') else { break };
        let tag = &rest[..tag_end + 1];
        let name = extract_attr(tag, "name").unwrap_or_default();
        if name.is_empty() { search = &search[start + 1..]; continue; }

        let close = "</tool_call>";
        let Some(end) = rest.find(close) else { break };
        let inner = &rest[tag_end + 1..end];
        let args = parse_xml_args(inner);
        calls.push(ToolCall { name, args });

        let advance = start + end + close.len();
        if advance >= search.len() { break; }
        search = &search[advance..];
    }
    calls
}

pub fn strip_tool_calls(text: &str) -> String {
    let mut result = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("<tool_call") {
        result.push_str(&rest[..start]);
        let close = "</tool_call>";
        if let Some(end) = rest.find(close) {
            rest = &rest[end + close.len()..];
        } else { break; }
    }
    result.push_str(rest);

    // Collapse consecutive blank lines
    let lines: Vec<&str> = result.lines().collect();
    let mut out = Vec::new();
    let mut prev_blank = false;
    for line in &lines {
        let blank = line.trim().is_empty();
        if blank && prev_blank { continue; }
        out.push(*line);
        prev_blank = blank;
    }
    out.join("\n").trim().to_string()
}

fn extract_attr(tag: &str, attr: &str) -> Option<String> {
    let needle = format!("{}=\"", attr);
    let start = tag.find(&needle)? + needle.len();
    let end = tag[start..].find('"')? + start;
    Some(tag[start..end].to_string())
}

fn parse_xml_args(inner: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut rest = inner.trim();
    while let Some(open_start) = rest.find('<') {
        let open_rest = &rest[open_start + 1..];
        let Some(open_end) = open_rest.find('>') else { break };
        let tag_name = &open_rest[..open_end];
        if tag_name.starts_with('/') { rest = &rest[open_start + 1 + open_end + 1..]; continue; }
        let after_open = &open_rest[open_end + 1..];
        let close_tag = format!("</{}>", tag_name);
        if let Some(close_pos) = after_open.find(&close_tag) {
            map.insert(tag_name.to_string(), after_open[..close_pos].to_string());
            let consumed = open_start + 1 + open_end + 1 + close_pos + close_tag.len();
            if consumed >= rest.len() { break; }
            rest = &rest[consumed..];
        } else { break; }
    }
    map
}

// ── Reasoner backend factory ──────────────────────────────────────────────────

fn build_reasoner(config: &Config) -> Result<Box<dyn AiBackend>> {
    match config.default_backend {
        Backend::Ollama => Ok(Box::new(OllamaBackend::new(
            config.ollama.base_url.clone(),
            config.ollama.default_model.clone(),
        ))),
        Backend::Nim => NimBackend::new(
            config.nim.base_url.clone(),
            config.nim.api_key.clone(),
            config.nim.default_model.clone(),
        ).map(|b| Box::new(b) as Box<dyn AiBackend>),
    }
}

// ── Format tool results for the reasoner's context ────────────────────────────

fn format_tool_results(results: &[(String, String, bool)]) -> String {
    results
        .iter()
        .map(|(name, output, success)| {
            let status = if *success { "ok" } else { "error" };
            format!("=== {name} [{status}] ===\n{output}\n")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// ── Main pipeline ─────────────────────────────────────────────────────────────

const MAX_REACTIVE_ROUNDS: u32 = 6;

pub fn run_agent(
    messages: Vec<Message>,
    config: Config,
    working_dir: String,
    file_tree: Vec<String>,
    event_tx: mpsc::UnboundedSender<AgentEvent>,
) {
    tokio::spawn(async move {
        // Extract the user's latest message for the planner
        let user_message = messages
            .iter()
            .rev()
            .find(|m| matches!(m.role, crate::ai::Role::User))
            .map(|m| m.content.as_str())
            .unwrap_or("");

        // ── PHASE 1: PLANNER ──────────────────────────────────────────────────
        let _ = event_tx.send(AgentEvent::PlannerStarted);

        let planned = planner::plan(user_message, &messages, &file_tree, &config).await;
        let _ = event_tx.send(AgentEvent::PlannerDone { calls: planned.clone() });

        // ── PHASE 2: EXECUTE PLANNED TOOLS (parallel) ─────────────────────────
        let mut all_tool_results: Vec<(String, String, bool)> = Vec::new();

        if !planned.is_empty() {
            let planned_calls: Vec<ToolCall> = planned.into_iter().map(Into::into).collect();
            let batch_results =
                execute_batch(&planned_calls, &working_dir, 0, &event_tx).await;
            all_tool_results.extend(batch_results);
        }

        // ── PHASE 3: BUILD REASONER CONTEXT ───────────────────────────────────
        // Inject tool results as context before the reasoner runs.
        let mut reasoner_messages = messages.clone();
        if !all_tool_results.is_empty() {
            let results_text = format_tool_results(&all_tool_results);
            reasoner_messages.push(Message::user(format!(
                "I gathered this information for you before answering:\n\n{results_text}\n\
                 Use it to give a thorough, accurate response."
            )));
        }

        // ── PHASE 4: REASONER + REACTIVE LOOP ────────────────────────────────
        let _ = event_tx.send(AgentEvent::ReasonerStarted);

        for reactive_round in 0..MAX_REACTIVE_ROUNDS {
            let backend = match build_reasoner(&config) {
                Ok(b) => b,
                Err(e) => { let _ = event_tx.send(AgentEvent::Error(e.to_string())); return; }
            };

            let (stream_tx, mut stream_rx) = mpsc::unbounded_channel::<StreamChunk>();
            let msgs_clone = reasoner_messages.clone();

            tokio::spawn(async move {
                if let Err(e) = backend.stream_chat(msgs_clone, stream_tx.clone()).await {
                    let _ = stream_tx.send(StreamChunk::Error(e.to_string()));
                }
            });

            let mut full_response = String::new();
            while let Some(chunk) = stream_rx.recv().await {
                match chunk {
                    StreamChunk::Token(t) => {
                        full_response.push_str(&t);
                        let _ = event_tx.send(AgentEvent::Token(t));
                    }
                    StreamChunk::Done => break,
                    StreamChunk::Error(e) => {
                        let _ = event_tx.send(AgentEvent::Error(e));
                        return;
                    }
                }
            }

            // Emit turn for the app to display
            let _ = event_tx.send(AgentEvent::TurnComplete(full_response.clone()));

            // Check for reactive tool calls in the reasoner's response
            let reactive_calls = parse_tool_calls(&full_response);
            if reactive_calls.is_empty() {
                // No more tools — we're done
                let _ = event_tx.send(AgentEvent::Done);
                return;
            }

            // Reasoner asked for more tools — commit its turn and execute
            reasoner_messages.push(Message::assistant(full_response));

            let iteration = reactive_round + 1;
            let batch_results =
                execute_batch(&reactive_calls, &working_dir, iteration, &event_tx).await;

            let results_text = format_tool_results(&batch_results);
            reasoner_messages.push(Message::user(format!(
                "Tool results:\n{results_text}\n\nContinue your response."
            )));
        }

        let _ = event_tx.send(AgentEvent::Error(format!(
            "Agent hit the reactive round limit ({MAX_REACTIVE_ROUNDS}). Stopping."
        )));
    });
}
