use super::ToolResult;
use anyhow::Result;
use std::path::Path;

pub struct FileTool;

impl FileTool {
    pub async fn read(path: &str) -> Result<ToolResult> {
        let content = tokio::fs::read_to_string(path).await;
        match content {
            Ok(text) => Ok(ToolResult {
                tool: "read_file".to_string(),
                input: path.to_string(),
                output: text,
                success: true,
            }),
            Err(e) => Ok(ToolResult {
                tool: "read_file".to_string(),
                input: path.to_string(),
                output: format!("Error reading file: {}", e),
                success: false,
            }),
        }
    }

    pub async fn write(path: &str, content: &str) -> Result<ToolResult> {
        if let Some(parent) = Path::new(path).parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        match tokio::fs::write(path, content).await {
            Ok(_) => Ok(ToolResult {
                tool: "write_file".to_string(),
                input: format!("{}  ({} bytes)", path, content.len()),
                output: format!("Successfully wrote {} bytes to {}", content.len(), path),
                success: true,
            }),
            Err(e) => Ok(ToolResult {
                tool: "write_file".to_string(),
                input: path.to_string(),
                output: format!("Error writing file: {}", e),
                success: false,
            }),
        }
    }

    pub async fn list_directory(path: &str) -> Result<ToolResult> {
        let mut entries = tokio::fs::read_dir(path).await?;
        let mut names = Vec::new();

        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name().to_string_lossy().to_string();
            let is_dir = entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false);
            if is_dir {
                names.push(format!("{}/", name));
            } else {
                names.push(name);
            }
        }

        names.sort();
        Ok(ToolResult {
            tool: "list_dir".to_string(),
            input: path.to_string(),
            output: names.join("\n"),
            success: true,
        })
    }
}
