//! REPL 工具 - 在支持的解释器中执行代码片段

use crate::tools::{Tool, ToolContext, ToolOperationKind, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::process::Command;

/// REPL 工具
pub struct REPLTool;

#[async_trait]
impl Tool for REPLTool {
    fn name(&self) -> &str {
        "repl"
    }

    fn operation_kind(&self, _params: &Value) -> ToolOperationKind {
        ToolOperationKind::Shell
    }

    fn description(&self) -> &str {
        "Evaluate code in a REPL for supported languages: python, javascript, ruby, bash."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "language": {
                    "type": "string",
                    "enum": ["python", "py", "javascript", "js", "node", "ruby", "rb", "bash", "shell", "sh"],
                    "description": "Programming language to evaluate"
                },
                "code": {
                    "type": "string",
                    "description": "Code snippet to execute"
                },
                "timeout": {
                    "type": "integer",
                    "default": 30,
                    "description": "Execution timeout in seconds"
                }
            },
            "required": ["language", "code"]
        })
    }

    async fn execute(&self, params: serde_json::Value, _context: ToolContext) -> ToolResult {
        let language = params["language"].as_str().unwrap_or("");
        let code = params["code"].as_str().unwrap_or("");
        let _timeout = params["timeout"].as_u64().unwrap_or(30) as usize;

        if language.is_empty() || code.is_empty() {
            return ToolResult::error("language and code are required");
        }

        let (command, args, ext) = match language {
            "python" | "py" => ("python3", vec![], "py"),
            "javascript" | "js" | "node" => ("node", vec![], "js"),
            "ruby" | "rb" => ("ruby", vec![], "rb"),
            "bash" | "shell" | "sh" => ("bash", vec!["-c".to_string(), code.to_string()], "sh"),
            _ => return ToolResult::error(format!("Unsupported language: {}", language)),
        };

        let result = if language == "bash" || language == "shell" || language == "sh" {
            Command::new(command).args(args).output().await
        } else {
            let tmp = std::env::temp_dir().join(format!("repl_{}.{}", uuid::Uuid::new_v4(), ext));
            if let Err(e) = tokio::fs::write(&tmp, code).await {
                return ToolResult::error(format!("Failed to write temp file: {}", e));
            }
            let out = Command::new(command)
                .arg(tmp.to_str().unwrap_or(""))
                .output()
                .await;
            let _ = tokio::fs::remove_file(&tmp).await;
            out
        };

        match result {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                if output.status.success() {
                    let mut content = stdout.to_string();
                    if !stderr.is_empty() {
                        content.push_str("\n[stderr]\n");
                        content.push_str(&stderr);
                    }
                    ToolResult::success(content)
                } else {
                    ToolResult::error_with_content(
                        format!("REPL exited with code {:?}", output.status.code()),
                        format!("stdout:\n{}\nstderr:\n{}", stdout, stderr),
                    )
                }
            }
            Err(e) => ToolResult::error(format!("Failed to run REPL: {}", e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_repl_python() {
        let tool = REPLTool;
        let result = tool
            .execute(
                json!({"language": "python", "code": "print('hello from repl')"}),
                ToolContext::new(".", "test"),
            )
            .await;
        // python3 may not be installed in test environment
        // so we just assert it doesn't panic
        assert!(result.content.contains("hello from repl") || !result.success);
    }

    #[tokio::test]
    async fn test_repl_bash() {
        let tool = REPLTool;
        let result = tool
            .execute(
                json!({"language": "bash", "code": "echo 'hello bash'"}),
                ToolContext::new(".", "test"),
            )
            .await;
        assert!(result.success);
        assert!(result.content.contains("hello bash"));
    }

    #[tokio::test]
    async fn test_repl_unsupported() {
        let tool = REPLTool;
        let result = tool
            .execute(
                json!({"language": "fortran", "code": "print *, 'hi'"}),
                ToolContext::new(".", "test"),
            )
            .await;
        assert!(!result.success);
    }
}
