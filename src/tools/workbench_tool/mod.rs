//! Workbench 工具 - IDE 集成操作
//!
//! 支持在 VS Code / Cursor 中打开文件、显示文件、运行终端命令等

use crate::tools::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::json;

/// Workbench 工具
pub struct WorkbenchTool;

#[async_trait]
impl Tool for WorkbenchTool {
    fn name(&self) -> &str {
        "workbench"
    }

    fn description(&self) -> &str {
        "Interact with the host IDE (VS Code / Cursor). Actions: 'open_file', 'reveal', 'terminal', 'get_open_files'."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["open_file", "reveal", "terminal", "get_open_files"],
                    "description": "The IDE action to perform"
                },
                "path": {
                    "type": "string",
                    "description": "File path for 'open_file' or 'reveal'"
                },
                "line": {
                    "type": "integer",
                    "description": "Line number (1-based) for 'open_file'"
                },
                "column": {
                    "type": "integer",
                    "description": "Column number (1-based) for 'open_file'"
                },
                "command": {
                    "type": "string",
                    "description": "Command string for 'terminal' action"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, params: serde_json::Value, context: ToolContext) -> ToolResult {
        let action = params["action"].as_str().unwrap_or("");

        let client = match crate::ide::vscode::detect_vscode() {
            Some(c) => c,
            None => {
                return ToolResult::error(
                    "VS Code or Cursor CLI not found in PATH. Please install VS Code or Cursor and ensure 'code' or 'cursor' is available."
                );
            }
        };

        match action {
            "open_file" => {
                let path = params["path"].as_str().unwrap_or("");
                if path.is_empty() {
                    return ToolResult::error("Missing 'path' parameter for open_file");
                }
                let resolved =
                    match crate::tools::file_tool::resolve_path(path, &context.working_dir) {
                        Ok(p) => p,
                        Err(e) => return ToolResult::error(e),
                    };
                let line = params["line"].as_u64().map(|n| n as u32);
                let column = params["column"].as_u64().map(|n| n as u32);

                match client.open_file(&resolved, line, column).await {
                    Ok(msg) => ToolResult::success(msg),
                    Err(e) => ToolResult::error(format!("Failed to open file: {}", e)),
                }
            }
            "reveal" => {
                let path = params["path"].as_str().unwrap_or("");
                if path.is_empty() {
                    return ToolResult::error("Missing 'path' parameter for reveal");
                }
                let resolved =
                    match crate::tools::file_tool::resolve_path(path, &context.working_dir) {
                        Ok(p) => p,
                        Err(e) => return ToolResult::error(e),
                    };

                match client.reveal(&resolved).await {
                    Ok(msg) => ToolResult::success(msg),
                    Err(e) => ToolResult::error(format!("Failed to reveal file: {}", e)),
                }
            }
            "terminal" => {
                let command = params["command"].as_str().unwrap_or("");
                if command.is_empty() {
                    return ToolResult::error("Missing 'command' parameter for terminal");
                }

                match client.run_in_terminal(command).await {
                    Ok(msg) => ToolResult::success(msg),
                    Err(e) => ToolResult::error(format!("Failed to run terminal command: {}", e)),
                }
            }
            "get_open_files" => match client.get_open_files().await {
                Ok(files) => ToolResult::success(files.join("\n")),
                Err(e) => ToolResult::error(format!("Failed to get open files: {}", e)),
            },
            _ => ToolResult::error(format!("Unknown workbench action: {}", action)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workbench_tool_params() {
        let tool = WorkbenchTool;
        let params = tool.parameters();
        assert!(params.get("properties").is_some());
    }
}
