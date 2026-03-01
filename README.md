# aicode — AI Coding Assistant TUI

A terminal UI coding assistant written in Rust, inspired by Claude Code. Supports **Ollama** (local) and **Nvidia NIM** (cloud) as AI backends.

## Features

- 🖥️ **Rich TUI** — built with ratatui: chat history, file tree sidebar, status bar
- 🤖 **Dual backends** — Ollama (local) and Nvidia NIM (OpenAI-compatible)
- 📡 **Streaming** — token-by-token streaming output with live display
- 🔧 **Tool use** — shell execution, file read/write, directory listing
- 📁 **File tree** — sidebar shows current directory contents with icons
- ⌨️ **Vim-like** — Normal/Insert modes, j/k navigation
- 💬 **Slash commands** — `/run`, `/read`, `/ls`, `/cd`, `/model`, `/clear`
- 🎨 **Syntax highlighting** — code blocks rendered with language labels
- 🔁 **Model switching** — pop-up model picker for available models
- 💾 **Persistent config** — `~/.config/aicode/config.toml`

## Installation

### Prerequisites

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# For Ollama backend (optional)
curl -fsSL https://ollama.ai/install.sh | sh
ollama pull llama3.2
```

### Build & Run

```bash
git clone <repo>
cd aicode
cargo build --release
./target/release/aicode
```

Or install globally:

```bash
cargo install --path .
aicode
```

## Configuration

Config is auto-generated at `~/.config/aicode/config.toml` on first run:

```toml
default_backend = "ollama"  # or "nim"

[ollama]
base_url = "http://localhost:11434"
default_model = "llama3.2"

[nim]
base_url = "https://integrate.api.nvidia.com/v1"
api_key = "nvapi-xxxxxxxxxxxx"  # Get from build.nvidia.com
default_model = "meta/llama-3.1-70b-instruct"

[ui]
show_file_tree = true
syntax_highlight = true
mouse_enabled = true
```

### Nvidia NIM Setup

1. Sign up at [build.nvidia.com](https://build.nvidia.com)
2. Generate an API key
3. Set `api_key` in config or `NVIDIA_API_KEY` env var
4. Set `default_backend = "nim"` in config

## Keybindings

| Key | Action |
|-----|--------|
| `i` or `a` | Enter insert mode |
| `Esc` | Return to normal mode |
| `↑↓` / `j` `k` | Scroll chat |
| `g` / `G` | Scroll to top / bottom |
| `m` | Open model selector |
| `?` | Help overlay |
| `q` / `Ctrl+C` | Quit |

## Slash Commands

| Command | Description |
|---------|-------------|
| `/run <cmd>` | Execute a shell command |
| `/read <file>` | Read a file into AI context |
| `/ls [path]` | List directory contents |
| `/cd <path>` | Change working directory |
| `/model` | Switch AI model (popup) |
| `/clear` | Clear chat history |
| `/help` | Show help |
| `/quit` | Exit |

## Architecture

```
src/
├── main.rs          # Entry point + panic handler
├── app.rs           # App state, event loop, command handling
├── config.rs        # Config file management
├── events.rs        # Event system (keyboard, AI stream, tick)
├── ai/
│   ├── mod.rs       # AiBackend trait + shared types
│   ├── ollama.rs    # Ollama streaming backend
│   └── nim.rs       # Nvidia NIM (OpenAI-compat) backend
├── tools/
│   ├── mod.rs       # Tool trait + ToolResult
│   ├── shell.rs     # Shell command execution
│   └── file.rs      # File read/write/list
└── ui/
    ├── mod.rs        # Main draw() + layout
    ├── chat.rs       # Chat history + input box
    ├── file_tree.rs  # Sidebar file tree
    ├── status_bar.rs # Bottom status bar
    ├── model_select.rs # Model picker popup
    └── help.rs       # Help overlay
```

## Roadmap

- [ ] Multi-line input editor
- [ ] Diff view for file edits
- [ ] Clipboard support
- [ ] Command history (↑↓ in input)
- [ ] Session save/restore
- [ ] Config hot-reload
- [ ] Custom system prompt per session
- [ ] MCP (Model Context Protocol) tool support
