//! Git 工具 - 执行常见的 git 操作

use crate::tools::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use tokio::process::Command;

/// Git 工具
pub struct GitTool;

#[async_trait]
impl Tool for GitTool {
    fn name(&self) -> &str {
        "git"
    }

    fn description(&self) -> &str {
        "Run common git operations. Actions: 'status', 'diff', 'log', 'show'."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["status", "diff", "log", "show"],
                    "description": "The git action to perform"
                },
                "path": {
                    "type": "string",
                    "description": "Optional file path filter"
                },
                "commit": {
                    "type": "string",
                    "description": "Commit hash for 'show' action"
                },
                "range": {
                    "type": "string",
                    "description": "Commit range for 'diff' action (e.g. HEAD~1..HEAD)"
                },
                "cached": {
                    "type": "boolean",
                    "default": false,
                    "description": "Show staged changes (for 'diff' action)"
                },
                "stat": {
                    "type": "boolean",
                    "default": false,
                    "description": "Whether to show diffstat"
                },
                "n": {
                    "type": "integer",
                    "default": 10,
                    "description": "Number of commits for 'log' action"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, params: serde_json::Value, _context: ToolContext) -> ToolResult {
        let action = params["action"].as_str().unwrap_or("");
        let path = params["path"].as_str().unwrap_or("");
        let stat = params["stat"].as_bool().unwrap_or(false);

        let result = match action {
            "status" => {
                let mut cmd = Command::new("git");
                cmd.arg("status").arg("--short");
                if !path.is_empty() {
                    cmd.arg("--").arg(path);
                }
                cmd.output().await
            }
            "diff" => {
                let cached = params["cached"].as_bool().unwrap_or(false);
                let mut cmd = Command::new("git");
                if stat {
                    cmd.arg("diff").arg("--stat");
                } else if cached {
                    cmd.arg("diff").arg("--cached");
                } else {
                    cmd.arg("diff");
                }
                if let Some(range) = params["range"].as_str() {
                    if !is_valid_git_range(range) {
                        return ToolResult::error(format!("Invalid git diff range: '{}'", range));
                    }
                    cmd.arg(range);
                }
                if !path.is_empty() {
                    cmd.arg("--").arg(path);
                }
                cmd.output().await
            }
            "log" => {
                let n = params["n"].as_u64().unwrap_or(10) as usize;
                let mut cmd = Command::new("git");
                cmd.args(["log", "--oneline", "-n", &n.to_string()]);
                if !path.is_empty() {
                    cmd.arg("--").arg(path);
                }
                cmd.output().await
            }
            "show" => {
                let commit = params["commit"].as_str().unwrap_or("HEAD");
                let mut cmd = Command::new("git");
                cmd.arg("show").arg("--stat").arg(commit);
                if !path.is_empty() {
                    cmd.arg("--").arg(path);
                }
                cmd.output().await
            }
            _ => {
                return ToolResult::error(format!("Unknown git action: {}", action));
            }
        };

        match result {
            Ok(out) if out.status.success() => {
                let text = String::from_utf8_lossy(&out.stdout);
                ToolResult::success(text.to_string())
            }
            Ok(out) => ToolResult::error(format!(
                "git {} failed: {}",
                action,
                String::from_utf8_lossy(&out.stderr)
            )),
            Err(e) => ToolResult::error(format!("Failed to run git: {}", e)),
        }
    }
}

/// 检查 git diff range 参数是否合法
fn is_valid_git_range(range: &str) -> bool {
    if range.is_empty() {
        return false;
    }
    // 禁止以 - 开头，防止被解析为 git flag
    if range.starts_with('-') {
        return false;
    }
    // 禁止包含 shell 元字符或控制字符
    let forbidden = [';', '|', '&', '$', '`', '\n', '\r', '\t', '<', '>'];
    if range.chars().any(|c| forbidden.contains(&c)) {
        return false;
    }
    // 只允许常见的 git ref 字符
    range.chars().all(|c| {
        c.is_alphanumeric()
            || c == '.'
            || c == '~'
            || c == '^'
            || c == '/'
            || c == '-'
            || c == '_'
            || c == ':'
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_git_status() {
        let tool = GitTool;
        let result = tool
            .execute(json!({ "action": "status" }), ToolContext::new(".", "test"))
            .await;
        assert!(result.success);
    }

    #[tokio::test]
    async fn test_git_log() {
        let tool = GitTool;
        let result = tool
            .execute(
                json!({ "action": "log", "n": 3 }),
                ToolContext::new(".", "test"),
            )
            .await;
        assert!(result.success);
        // Should contain commit hashes (7 chars) and messages
        assert!(!result.content.is_empty());
    }

    #[test]
    fn test_is_valid_git_range() {
        assert!(is_valid_git_range("HEAD~1..HEAD"));
        assert!(is_valid_git_range("main...feature"));
        assert!(is_valid_git_range("abc1234"));
        assert!(!is_valid_git_range("--help"));
        assert!(!is_valid_git_range("HEAD;rm -rf /"));
        assert!(!is_valid_git_range("`whoami`"));
    }
}
