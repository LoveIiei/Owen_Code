use crate::{
    agent::{self, AgentEvent, AgentStep},
    ai::{AiBackend, Message, NimBackend, OllamaBackend, Role},
    config::{Backend, Config},
    events::{AppEvent, EventHandler},
    session::{Session, SessionMeta, SessionStore},
    tools::{FileTool, ShellTool, ToolResult},
    ui,
};
use anyhow::Result;
use chrono::Local;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use tokio::sync::mpsc;

#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    Normal,
    Insert,
    ModelSelect,
    SessionSelect,
    Help,
}

#[derive(Debug, Clone)]
pub enum EntryKind {
    User,
    Assistant,
    Tool { success: bool },
}

#[derive(Debug, Clone)]
pub struct ChatEntry {
    pub role: Role,
    pub kind: EntryKind,
    pub content: String,
    pub timestamp: chrono::DateTime<chrono::Local>,
}

impl ChatEntry {
    pub fn new(role: Role, content: String) -> Self {
        let kind = match role {
            Role::User => EntryKind::User,
            Role::Assistant => EntryKind::Assistant,
            Role::System => EntryKind::Assistant,
        };
        Self {
            role,
            kind,
            content,
            timestamp: Local::now(),
        }
    }

    pub fn tool(label: String, output: String, success: bool) -> Self {
        Self {
            role: Role::Assistant,
            kind: EntryKind::Tool { success },
            content: format!("{}\n{}", label, output),
            timestamp: Local::now(),
        }
    }
}

// ── Multi-line input buffer ───────────────────────────────────────────────────

/// A simple multi-line text buffer with row/col cursor tracking.
#[derive(Debug, Clone, Default)]
pub struct InputBuffer {
    pub lines: Vec<String>,
    pub row: usize,
    pub col: usize, // byte index into lines[row]
}

impl InputBuffer {
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
            row: 0,
            col: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.lines.iter().all(|l| l.is_empty())
    }

    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    pub fn clear(&mut self) {
        self.lines = vec![String::new()];
        self.row = 0;
        self.col = 0;
    }

    pub fn insert_char(&mut self, c: char) {
        self.lines[self.row].insert(self.col, c);
        self.col += c.len_utf8();
    }

    pub fn insert_newline(&mut self) {
        let rest = self.lines[self.row].split_off(self.col);
        self.lines.insert(self.row + 1, rest);
        self.row += 1;
        self.col = 0;
    }

    pub fn backspace(&mut self) {
        if self.col > 0 {
            let char_idx = self.lines[self.row][..self.col].chars().count();
            let mut chars: Vec<char> = self.lines[self.row].chars().collect();
            if char_idx > 0 {
                let removed = chars.remove(char_idx - 1);
                self.col -= removed.len_utf8();
                self.lines[self.row] = chars.into_iter().collect();
            }
        } else if self.row > 0 {
            let current = self.lines.remove(self.row);
            self.row -= 1;
            self.col = self.lines[self.row].len();
            self.lines[self.row].push_str(&current);
        }
    }

    pub fn delete_forward(&mut self) {
        let line_len = self.lines[self.row].len();
        if self.col < line_len {
            let char_idx = self.lines[self.row][..self.col].chars().count();
            let mut chars: Vec<char> = self.lines[self.row].chars().collect();
            if char_idx < chars.len() {
                chars.remove(char_idx);
                self.lines[self.row] = chars.into_iter().collect();
            }
        } else if self.row + 1 < self.lines.len() {
            let next = self.lines.remove(self.row + 1);
            self.lines[self.row].push_str(&next);
        }
    }

    pub fn move_left(&mut self) {
        if self.col > 0 {
            let prev_len = self.lines[self.row][..self.col]
                .chars()
                .last()
                .map(|c| c.len_utf8())
                .unwrap_or(1);
            self.col -= prev_len;
        } else if self.row > 0 {
            self.row -= 1;
            self.col = self.lines[self.row].len();
        }
    }

    pub fn move_right(&mut self) {
        let line_len = self.lines[self.row].len();
        if self.col < line_len {
            let next_len = self.lines[self.row][self.col..]
                .chars()
                .next()
                .map(|c| c.len_utf8())
                .unwrap_or(1);
            self.col += next_len;
        } else if self.row + 1 < self.lines.len() {
            self.row += 1;
            self.col = 0;
        }
    }

    pub fn move_up(&mut self) {
        if self.row > 0 {
            self.row -= 1;
            self.col = self.col.min(self.lines[self.row].len());
        }
    }

    pub fn move_down(&mut self) {
        if self.row + 1 < self.lines.len() {
            self.row += 1;
            self.col = self.col.min(self.lines[self.row].len());
        }
    }

    pub fn move_home(&mut self) {
        self.col = 0;
    }
    pub fn move_end(&mut self) {
        self.col = self.lines[self.row].len();
    }
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }
}

// ── App ───────────────────────────────────────────────────────────────────────

pub struct App {
    pub config: Config,
    pub backend: Box<dyn AiBackend>,

    pub mode: AppMode,
    pub messages: Vec<Message>,
    pub chat_log: Vec<ChatEntry>,
    pub tool_results: Vec<ToolResult>,

    // Agent state
    pub agent_steps: Vec<AgentStep>, // tool call cards shown in chat
    pub agent_iteration: u32,        // current agent loop iteration
    pub streaming_label: String,     // e.g. "Thinking…" vs "Running tool…"

    pub input: InputBuffer,
    pub input_history: Vec<String>,
    pub input_history_idx: Option<usize>,

    pub scroll: u16,
    pub streaming: bool,
    pub streaming_buffer: String,

    pub status: String,
    pub available_models: Vec<String>,
    pub selected_model_idx: usize,

    pub session: Session,
    pub session_list: Vec<SessionMeta>,
    pub session_list_idx: usize,

    pub working_dir: String,
    pub file_tree: Vec<String>,

    pub event_tx: mpsc::UnboundedSender<AppEvent>,
    pub should_quit: bool,
}

impl App {
    pub async fn new() -> Result<Self> {
        let config = Config::load()?;

        let backend: Box<dyn AiBackend> = match config.default_backend {
            Backend::Ollama => Box::new(OllamaBackend::new(
                config.ollama.base_url.clone(),
                config.ollama.default_model.clone(),
            )),
            Backend::Nim => Box::new(NimBackend::new(
                config.nim.base_url.clone(),
                config.nim.api_key.clone(),
                config.nim.default_model.clone(),
            )?),
        };

        let working_dir = std::env::current_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let (tx, _) = mpsc::unbounded_channel();

        let (messages, chat_log, session) = if let Some(last) = SessionStore::load_last() {
            let msgs = last.to_messages();
            let log = last.to_chat_log();
            (msgs, log, last)
        } else {
            let sys = vec![Message::system(SYSTEM_PROMPT.to_string())];
            let s = Session::new(
                format!("Session {}", Local::now().format("%Y-%m-%d %H:%M")),
                working_dir.clone(),
                backend.name().to_string(),
                backend.model().to_string(),
                &sys,
                &[],
            );
            (sys, vec![], s)
        };

        let resumed = !chat_log.is_empty();

        let mut app = Self {
            config,
            backend,
            mode: AppMode::Normal,
            messages,
            chat_log,
            tool_results: Vec::new(),
            agent_steps: Vec::new(),
            agent_iteration: 0,
            streaming_label: String::new(),
            input: InputBuffer::new(),
            input_history: Vec::new(),
            input_history_idx: None,
            scroll: u16::MAX,
            streaming: false,
            streaming_buffer: String::new(),
            status: "Ready".to_string(),
            available_models: Vec::new(),
            selected_model_idx: 0,
            session,
            session_list: Vec::new(),
            session_list_idx: 0,
            working_dir: working_dir.clone(),
            file_tree: Vec::new(),
            event_tx: tx,
            should_quit: false,
        };

        app.refresh_file_tree().await;

        if resumed {
            app.chat_log.push(ChatEntry::new(
                Role::Assistant,
                format!(
                    "↩ Resumed session **{}**\nBackend: {} | Model: {}\n\nUse /sessions to browse sessions, /new to start fresh.",
                    app.session.name,
                    app.backend.name(),
                    app.backend.model(),
                ),
            ));
        } else {
            app.chat_log.push(ChatEntry::new(
                Role::Assistant,
                format!(
                    "Welcome to **OwenCode** 🤖\n\nBackend: {} | Model: {}\nWorking dir: {}\n\nPress **[i]** to start typing. Use **Ctrl+Enter** to send, **Enter** for new line.\nType /help for all commands.",
                    app.backend.name(),
                    app.backend.model(),
                    working_dir,
                ),
            ));
        }

        Ok(app)
    }

    pub async fn run(&mut self) -> Result<()> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let mut events = EventHandler::new();
        self.event_tx = events.tx.clone();

        loop {
            terminal.draw(|f| ui::draw(f, self))?;

            match events.next().await? {
                AppEvent::Key(key) => self.handle_key(key).await,
                AppEvent::Agent(ev) => self.handle_agent_event(ev).await,
                AppEvent::Resize(_, _) | AppEvent::Tick | AppEvent::Mouse(_) => {}
            }

            if self.should_quit {
                break;
            }
        }

        self.autosave();

        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        Ok(())
    }

    // ── Persistence ───────────────────────────────────────────────────────────

    fn autosave(&mut self) {
        self.session.messages = self
            .messages
            .iter()
            .map(|m| crate::session::SerializedMessage {
                role: match m.role {
                    Role::User => "user".to_string(),
                    Role::Assistant => "assistant".to_string(),
                    Role::System => "system".to_string(),
                },
                content: m.content.clone(),
            })
            .collect();

        self.session.chat_log = self
            .chat_log
            .iter()
            .map(|e| crate::session::SerializedChatEntry {
                role: match e.role {
                    Role::User => "user".to_string(),
                    Role::Assistant => "assistant".to_string(),
                    Role::System => "system".to_string(),
                },
                content: e.content.clone(),
                // Ensure the .to_rfc3339() conversion is applied here
                timestamp: e.timestamp.to_rfc3339(),
            })
            .collect();

        self.session.working_dir = self.working_dir.clone();
        self.session.model = self.backend.model().to_string();

        let _ = SessionStore::autosave(&mut self.session);
    }

    // ── Key routing ───────────────────────────────────────────────────────────

    async fn handle_key(&mut self, key: crossterm::event::KeyEvent) {
        match self.mode {
            AppMode::Normal => self.handle_normal_key(key).await,
            AppMode::Insert => self.handle_insert_key(key).await,
            AppMode::ModelSelect => self.handle_model_select_key(key).await,
            AppMode::SessionSelect => self.handle_session_select_key(key).await,
            AppMode::Help => {
                self.mode = AppMode::Normal;
            }
        }
    }

    async fn handle_normal_key(&mut self, key: crossterm::event::KeyEvent) {
        match (key.code, key.modifiers) {
            (KeyCode::Char('q'), _) => self.should_quit = true,
            (KeyCode::Char('c'), m) if m.contains(KeyModifiers::CONTROL) => self.should_quit = true,
            (KeyCode::Char('i'), _) | (KeyCode::Char('a'), _) => self.mode = AppMode::Insert,
            (KeyCode::Char('m'), _) => self.open_model_select().await,
            (KeyCode::Char('s'), m) if m.contains(KeyModifiers::CONTROL) => {
                self.save_session_interactive()
            }
            (KeyCode::Char('?'), _) => self.mode = AppMode::Help,
            (KeyCode::Down, _) | (KeyCode::Char('j'), _) => {
                self.scroll = self.scroll.saturating_add(1)
            }
            (KeyCode::Up, _) | (KeyCode::Char('k'), _) => {
                self.scroll = self.scroll.saturating_sub(1)
            }
            (KeyCode::Char('g'), _) => self.scroll = 0,
            (KeyCode::Char('G'), _) => self.scroll = u16::MAX,
            _ => {}
        }
    }

    async fn handle_insert_key(&mut self, key: crossterm::event::KeyEvent) {
        match (key.code, key.modifiers) {
            // Submit: Ctrl+Enter or Alt+Enter
            (KeyCode::Enter, m)
                if m.contains(KeyModifiers::CONTROL) || m.contains(KeyModifiers::ALT) =>
            {
                if !self.streaming {
                    let text = self.input.text();
                    let trimmed = text.trim().to_string();
                    if !trimmed.is_empty() {
                        self.input_history.push(trimmed.clone());
                        self.input_history_idx = None;
                        self.input.clear();
                        self.handle_input(trimmed).await;
                    }
                }
            }
            // Newline
            (KeyCode::Enter, _) => self.input.insert_newline(),

            // Escape to normal
            (KeyCode::Esc, _) => self.mode = AppMode::Normal,

            // History navigation: Alt+Up / Alt+Down
            (KeyCode::Up, m) if m.contains(KeyModifiers::ALT) => self.history_prev(),
            (KeyCode::Down, m) if m.contains(KeyModifiers::ALT) => self.history_next(),

            // Cursor movement
            (KeyCode::Up, _) => self.input.move_up(),
            (KeyCode::Down, _) => self.input.move_down(),
            (KeyCode::Left, _) => self.input.move_left(),
            (KeyCode::Right, _) => self.input.move_right(),
            (KeyCode::Home, _) => self.input.move_home(),
            (KeyCode::End, _) => self.input.move_end(),

            // Editing
            (KeyCode::Char(c), m) if !m.contains(KeyModifiers::CONTROL) => {
                self.input.insert_char(c);
            }
            (KeyCode::Backspace, _) => self.input.backspace(),
            (KeyCode::Delete, _) => self.input.delete_forward(),

            // Ctrl+C still quits
            (KeyCode::Char('c'), m) if m.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            _ => {}
        }
    }

    async fn handle_model_select_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.mode = AppMode::Normal,
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected_model_idx + 1 < self.available_models.len() {
                    self.selected_model_idx += 1;
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected_model_idx = self.selected_model_idx.saturating_sub(1);
            }
            KeyCode::Enter => {
                if let Some(model) = self.available_models.get(self.selected_model_idx).cloned() {
                    self.backend.set_model(model.clone()).await;
                    self.session.model = model.clone();
                    self.status = format!("Switched to model: {}", model);
                }
                self.mode = AppMode::Normal;
            }
            _ => {}
        }
    }

    async fn handle_session_select_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.mode = AppMode::Normal,
            KeyCode::Down | KeyCode::Char('j') => {
                if self.session_list_idx + 1 < self.session_list.len() {
                    self.session_list_idx += 1;
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.session_list_idx = self.session_list_idx.saturating_sub(1);
            }
            KeyCode::Enter => {
                if let Some(meta) = self.session_list.get(self.session_list_idx).cloned() {
                    self.load_session(&meta.id).await;
                }
                self.mode = AppMode::Normal;
            }
            KeyCode::Char('d') => {
                if let Some(meta) = self.session_list.get(self.session_list_idx).cloned() {
                    let _ = SessionStore::delete(&meta.id);
                    self.session_list.remove(self.session_list_idx);
                    self.session_list_idx = self.session_list_idx.saturating_sub(1);
                    if self.session_list.is_empty() {
                        self.mode = AppMode::Normal;
                    }
                    self.status = "Session deleted".to_string();
                }
            }
            _ => {}
        }
    }

    // ── History ───────────────────────────────────────────────────────────────

    fn history_prev(&mut self) {
        if self.input_history.is_empty() {
            return;
        }
        let idx = match self.input_history_idx {
            None => self.input_history.len() - 1,
            Some(0) => 0,
            Some(i) => i - 1,
        };
        self.input_history_idx = Some(idx);
        self.set_input_from_history(idx);
    }

    fn history_next(&mut self) {
        match self.input_history_idx {
            None => {}
            Some(i) if i + 1 >= self.input_history.len() => {
                self.input_history_idx = None;
                self.input.clear();
            }
            Some(i) => {
                let idx = i + 1;
                self.input_history_idx = Some(idx);
                self.set_input_from_history(idx);
            }
        }
    }

    fn set_input_from_history(&mut self, idx: usize) {
        let text = self.input_history[idx].clone();
        self.input.clear();
        for ch in text.chars() {
            if ch == '\n' {
                self.input.insert_newline();
            } else {
                self.input.insert_char(ch);
            }
        }
    }

    // ── Input dispatch ────────────────────────────────────────────────────────

    async fn handle_input(&mut self, input: String) {
        if input.starts_with('/') {
            self.handle_command(input).await;
            return;
        }
        self.chat_log
            .push(ChatEntry::new(Role::User, input.clone()));
        self.messages.push(Message::user(input));
        self.scroll = u16::MAX;
        self.start_agent().await;
    }

    async fn start_agent(&mut self) {
        self.streaming = true;
        self.streaming_buffer.clear();
        self.streaming_label = "Planning…".to_string();
        self.agent_steps.clear();
        self.agent_iteration = 0;
        self.status = self.streaming_label.clone();

        let messages = self.messages.clone();
        let config = self.config.clone();
        let working_dir = self.working_dir.clone();
        let file_tree = self.file_tree.clone();
        let event_tx = self.event_tx.clone();

        let (agent_tx, mut agent_rx) = tokio::sync::mpsc::unbounded_channel::<AgentEvent>();
        tokio::spawn(async move {
            while let Some(ev) = agent_rx.recv().await {
                let done = matches!(ev, AgentEvent::Done | AgentEvent::Error(_));
                let _ = event_tx.send(AppEvent::Agent(ev));
                if done {
                    break;
                }
            }
        });

        agent::run_agent(messages, config, working_dir, file_tree, agent_tx);
    }

    async fn handle_agent_event(&mut self, event: AgentEvent) {
        match event {
            // ── Planner lifecycle ─────────────────────────────────────────────
            AgentEvent::PlannerStarted => {
                self.streaming_label = format!("Planning… [{}]", self.config.planner.model);
                self.status = self.streaming_label.clone();
            }

            AgentEvent::PlannerDone { calls } => {
                if calls.is_empty() {
                    self.streaming_label = format!("Thinking… [{}]", self.backend.model());
                } else {
                    let names: Vec<&str> = calls.iter().map(|c| c.tool.as_str()).collect();
                    self.streaming_label =
                        format!("Plan: {} tool(s) → {}", calls.len(), names.join(", "));
                }
                self.status = self.streaming_label.clone();
            }

            // ── Reasoner started ──────────────────────────────────────────────
            AgentEvent::ReasonerStarted => {
                self.streaming_label = format!("Thinking… [{}]", self.backend.model());
                self.status = self.streaming_label.clone();
            }

            // ── Streaming token ───────────────────────────────────────────────
            AgentEvent::Token(token) => {
                self.streaming_buffer.push_str(&token);
            }

            // ── AI turn complete — may contain reactive tool calls ─────────────
            AgentEvent::TurnComplete(full_text) => {
                let prose = agent::strip_tool_calls(&full_text);
                if !prose.is_empty() {
                    self.chat_log.push(ChatEntry::new(Role::Assistant, prose));
                    self.scroll = u16::MAX;
                }
                self.streaming_buffer.clear();
                self.streaming_label = "Running tools…".to_string();
                self.status = self.streaming_label.clone();
            }

            // ── Tool starting ─────────────────────────────────────────────────
            AgentEvent::ToolStart {
                iteration,
                name,
                summary,
            } => {
                self.agent_iteration = iteration;
                self.streaming_label = format!("Tool: {}", summary);
                self.status = self.streaming_label.clone();
                self.agent_steps.push(AgentStep {
                    iteration,
                    tool: name,
                    summary,
                    output: String::new(),
                    success: false,
                });
                self.scroll = u16::MAX;
            }

            // ── Tool done ─────────────────────────────────────────────────────
            AgentEvent::ToolDone {
                iteration,
                name,
                output,
                success,
            } => {
                if let Some(step) = self
                    .agent_steps
                    .iter_mut()
                    .rev()
                    .find(|s| s.tool == name && s.iteration == iteration)
                {
                    step.output = output.clone();
                    step.success = success;
                }
                let icon = if success { "✓" } else { "✗" };
                let preview = if output.len() > 800 {
                    format!("{}\n…(truncated)", &output[..800])
                } else {
                    output.clone()
                };
                self.chat_log.push(ChatEntry::tool(
                    format!("{} {}", icon, name),
                    preview,
                    success,
                ));
                self.scroll = u16::MAX;
                self.streaming_label = format!("Thinking… [{}]", self.backend.model());
                self.status = self.streaming_label.clone();
            }

            // ── Agent finished ────────────────────────────────────────────────
            AgentEvent::Done => {
                let leftover = std::mem::take(&mut self.streaming_buffer);
                if !leftover.trim().is_empty() {
                    let prose = agent::strip_tool_calls(&leftover);
                    if !prose.is_empty() {
                        self.chat_log.push(ChatEntry::new(Role::Assistant, prose));
                    }
                }
                self.sync_messages_from_log();
                self.streaming = false;
                self.streaming_label.clear();
                self.status = "Ready".to_string();
                self.scroll = u16::MAX;
                self.autosave();
            }

            // ── Error ─────────────────────────────────────────────────────────
            AgentEvent::Error(e) => {
                self.chat_log.push(ChatEntry::new(
                    Role::Assistant,
                    format!("⚠️  Agent error: {}", e),
                ));
                self.streaming = false;
                self.streaming_label.clear();
                self.status = format!("Error: {}", e);
            }
        }
    }

    /// Rebuild `self.messages` from the chat log so history stays consistent
    /// after the agent injects tool results.
    fn sync_messages_from_log(&mut self) {
        // Keep the system prompt
        let system = self
            .messages
            .first()
            .cloned()
            .unwrap_or_else(|| Message::system(SYSTEM_PROMPT.to_string()));

        let mut msgs = vec![system];
        for entry in &self.chat_log {
            match entry.role {
                Role::User => msgs.push(Message::user(entry.content.clone())),
                Role::Assistant => msgs.push(Message::assistant(entry.content.clone())),
                Role::System => {} // skip, already handled
            }
        }
        self.messages = msgs;
    }

    // ── Commands ──────────────────────────────────────────────────────────────

    async fn handle_command(&mut self, input: String) {
        let parts: Vec<&str> = input.splitn(2, ' ').collect();
        let cmd = parts[0];
        let args = parts.get(1).copied().unwrap_or("").trim();

        match cmd {
            "/quit" | "/exit" | "/q" => self.should_quit = true,
            "/help" | "/?" => self.mode = AppMode::Help,
            "/model" | "/m" => self.open_model_select().await,

            "/save" | "/s" => {
                let name = if args.is_empty() {
                    format!("Session {}", Local::now().format("%Y-%m-%d %H:%M"))
                } else {
                    args.to_string()
                };
                self.save_session(name);
            }
            "/sessions" => self.open_session_select().await,
            "/new" => {
                self.autosave();
                self.new_session();
            }
            "/load" => {
                if args.is_empty() {
                    self.open_session_select().await;
                } else {
                    self.load_session(args).await;
                }
            }

            "/clear" => {
                self.chat_log.clear();
                self.messages.truncate(1);
                self.status = "Chat cleared".to_string();
            }

            "/run" | "/exec" | "/shell" => {
                if args.is_empty() {
                    self.push_assistant("Usage: /run <command>".into());
                    return;
                }
                self.status = format!("Running: {}", args);
                self.chat_log
                    .push(ChatEntry::new(Role::User, format!("$ {}", args)));
                match ShellTool::execute(args, Some(&self.working_dir)).await {
                    Ok(result) => {
                        let icon = if result.success { "✓" } else { "✗" };
                        self.push_assistant(format!(
                            "{} `{}`\n```\n{}\n```",
                            icon, args, result.output
                        ));
                        self.tool_results.push(result);
                    }
                    Err(e) => self.push_assistant(format!("⚠️  Shell error: {}", e)),
                }
                self.status = "Ready".to_string();
                self.refresh_file_tree().await;
            }
            "/read" | "/cat" => {
                if args.is_empty() {
                    self.push_assistant("Usage: /read <file>".into());
                    return;
                }
                match FileTool::read(args).await {
                    Ok(result) => {
                        let preview = if result.output.len() > 2000 {
                            format!("{}\n…(truncated)", &result.output[..2000])
                        } else {
                            result.output.clone()
                        };
                        self.push_assistant(format!("📄 {}\n```\n{}\n```", args, preview));
                        self.messages.push(Message::user(format!(
                            "File contents of `{}`:\n```\n{}\n```",
                            args, result.output
                        )));
                    }
                    Err(e) => self.push_assistant(format!("⚠️  Read error: {}", e)),
                }
            }
            "/ls" | "/dir" => {
                let path = if args.is_empty() {
                    self.working_dir.clone()
                } else {
                    args.to_string()
                };
                match FileTool::list_directory(&path).await {
                    Ok(result) => {
                        self.push_assistant(format!("📁 {}\n```\n{}\n```", path, result.output))
                    }
                    Err(e) => self.push_assistant(format!("⚠️  List error: {}", e)),
                }
            }
            "/cd" => {
                if args.is_empty() {
                    self.push_assistant(format!("Current directory: {}", self.working_dir));
                    return;
                }
                let new_dir = if args.starts_with('/') {
                    args.to_string()
                } else {
                    format!("{}/{}", self.working_dir, args)
                };
                match std::fs::canonicalize(&new_dir) {
                    Ok(path) => {
                        self.working_dir = path.to_string_lossy().to_string();
                        self.status = format!("cd: {}", self.working_dir);
                        self.refresh_file_tree().await;
                    }
                    Err(e) => self.push_assistant(format!("⚠️  cd error: {}", e)),
                }
            }
            _ => {
                self.push_assistant(format!(
                    "Unknown command: `{}`. Type /help for available commands.",
                    cmd
                ));
            }
        }
        self.scroll = u16::MAX;
    }

    // ── Session helpers ───────────────────────────────────────────────────────

    fn save_session(&mut self, name: String) {
        self.session.name = name.clone();
        self.autosave();
        self.push_assistant(format!("💾 Session saved as **{}**", name));
    }

    fn save_session_interactive(&mut self) {
        let name = format!("Session {}", Local::now().format("%Y-%m-%d %H:%M"));
        self.save_session(name);
    }

    fn new_session(&mut self) {
        let sys = vec![Message::system(SYSTEM_PROMPT.to_string())];
        self.session = Session::new(
            format!("Session {}", Local::now().format("%Y-%m-%d %H:%M")),
            self.working_dir.clone(),
            self.backend.name().to_string(),
            self.backend.model().to_string(),
            &sys,
            &[],
        );
        self.messages = sys;
        self.chat_log.clear();
        self.push_assistant("🆕 New session started. Previous session was auto-saved.".into());
    }

    async fn load_session(&mut self, id: &str) {
        match SessionStore::load(id) {
            Ok(session) => {
                let name = session.name.clone();
                self.messages = session.to_messages();
                self.chat_log = session.to_chat_log();
                self.working_dir = session.working_dir.clone();
                self.session = session;
                self.scroll = u16::MAX;
                self.status = format!("Loaded: {}", name);
                self.refresh_file_tree().await;
                self.push_assistant(format!("↩ Loaded session **{}**", name));
            }
            Err(e) => self.status = format!("Failed to load session: {}", e),
        }
    }

    async fn open_session_select(&mut self) {
        match SessionStore::list() {
            Ok(sessions) if sessions.is_empty() => {
                self.push_assistant(
                    "No saved sessions. Use /save to save the current session.".into(),
                );
            }
            Ok(sessions) => {
                self.session_list = sessions;
                self.session_list_idx = 0;
                self.mode = AppMode::SessionSelect;
                self.status = "Select a session".to_string();
            }
            Err(e) => self.status = format!("Failed to list sessions: {}", e),
        }
    }

    // ── Misc helpers ──────────────────────────────────────────────────────────

    async fn open_model_select(&mut self) {
        self.status = "Fetching models…".to_string();
        match self.backend.list_models().await {
            Ok(models) => {
                self.available_models = models;
                let current = self.backend.model().to_string();
                self.selected_model_idx = self
                    .available_models
                    .iter()
                    .position(|m| m == &current)
                    .unwrap_or(0);
                self.mode = AppMode::ModelSelect;
                self.status = "Select a model".to_string();
            }
            Err(e) => self.status = format!("Failed to list models: {}", e),
        }
    }

    async fn refresh_file_tree(&mut self) {
        let dir = self.working_dir.clone();
        if let Ok(result) = FileTool::list_directory(&dir).await {
            self.file_tree = result.output.lines().map(|s| s.to_string()).collect();
        }
    }

    fn push_assistant(&mut self, content: String) {
        self.chat_log.push(ChatEntry::new(Role::Assistant, content));
        self.scroll = u16::MAX;
    }
}

const SYSTEM_PROMPT: &str = r#"You are OwenCode, an autonomous AI coding assistant running in a terminal UI.
You help users write, review, debug, and understand code.

A separate planning step has already gathered relevant context for you (file contents, shell output, search results) and injected it above. Use that information to give a thorough, accurate answer.

If you need additional information that was NOT gathered in the planning step, you can request more tools reactively using XML tool calls anywhere in your response:

<tool_call name="read_file"><path>src/foo.rs</path></tool_call>
<tool_call name="run_shell"><command>cargo test 2>&1 | tail -20</command></tool_call>
<tool_call name="write_file"><path>src/foo.rs</path><content>// full content</content></tool_call>
<tool_call name="list_dir"><path>src/</path></tool_call>
<tool_call name="web_search"><query>tokio spawn blocking docs</query></tool_call>

Guidelines:
- Only use reactive tool calls if something critical is missing. The planner should have handled most needs.
- When editing files, show the complete new file content inside write_file.
- Keep responses concise. Use markdown code blocks for code.
- After reactive tool results arrive you will be called again automatically.
"#;
