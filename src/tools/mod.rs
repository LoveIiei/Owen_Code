pub mod file;
pub mod shell;
pub mod web_search;

/// Result of a tool execution shown in the UI
#[derive(Debug, Clone)]
pub struct ToolResult {
    pub tool: String,
    pub input: String,
    pub output: String,
    pub success: bool,
}

pub use file::FileTool;
pub use shell::ShellTool;
pub use web_search::WebSearch;
