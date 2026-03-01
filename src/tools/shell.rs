use super::ToolResult;
use anyhow::Result;
use std::process::Stdio;
use tokio::process::Command;

pub struct ShellTool;

impl ShellTool {
    pub async fn execute(cmd: &str, working_dir: Option<&str>) -> Result<ToolResult> {
        let mut command = if cfg!(target_os = "windows") {
            let mut c = Command::new("cmd");
            c.args(["/C", cmd]);
            c
        } else {
            let mut c = Command::new("sh");
            c.args(["-c", cmd]);
            c
        };

        command.stdout(Stdio::piped()).stderr(Stdio::piped());

        if let Some(dir) = working_dir {
            command.current_dir(dir);
        }

        let output = command.output().await?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        let combined = if stderr.is_empty() {
            stdout.clone()
        } else if stdout.is_empty() {
            stderr.clone()
        } else {
            format!("{}\n--- stderr ---\n{}", stdout, stderr)
        };

        Ok(ToolResult {
            tool: "shell".to_string(),
            input: cmd.to_string(),
            output: if combined.is_empty() {
                "(no output)".to_string()
            } else {
                combined
            },
            success: output.status.success(),
        })
    }
}
