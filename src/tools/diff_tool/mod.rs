//! Diff 工具 - 生成和查看代码差异

use crate::tools::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use std::process::Command;

/// Diff 工具
pub struct DiffTool;

#[async_trait]
impl Tool for DiffTool {
    fn name(&self) -> &str {
        "diff"
    }

    fn description(&self) -> &str {
        "Generate or view diffs. Actions: 'generate' (diff two strings), 'file' (git diff for a file), 'compare' (diff two files)."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["generate", "file", "compare"],
                    "description": "The diff action to perform"
                },
                "path": {
                    "type": "string",
                    "description": "File path for 'file' action"
                },
                "old_path": {
                    "type": "string",
                    "description": "Old file path for 'compare' action"
                },
                "new_path": {
                    "type": "string",
                    "description": "New file path for 'compare' action"
                },
                "old_content": {
                    "type": "string",
                    "description": "Old content for 'generate' action"
                },
                "new_content": {
                    "type": "string",
                    "description": "New content for 'generate' action"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, params: serde_json::Value, _context: ToolContext) -> ToolResult {
        let action = params["action"].as_str().unwrap_or("");

        match action {
            "generate" => {
                let old_content = params["old_content"].as_str().unwrap_or("");
                let new_content = params["new_content"].as_str().unwrap_or("");

                // Write to temp files and run diff -u
                let old_file =
                    std::env::temp_dir().join(format!("diff_old_{}", uuid::Uuid::new_v4()));
                let new_file =
                    std::env::temp_dir().join(format!("diff_new_{}", uuid::Uuid::new_v4()));

                if let Err(e) = std::fs::write(&old_file, old_content) {
                    return ToolResult::error(format!("Failed to write temp file: {}", e));
                }
                if let Err(e) = std::fs::write(&new_file, new_content) {
                    return ToolResult::error(format!("Failed to write temp file: {}", e));
                }

                let output = Command::new("diff")
                    .args([
                        "-u",
                        old_file.to_str().unwrap_or(""),
                        new_file.to_str().unwrap_or(""),
                    ])
                    .output();

                // Clean up temp files
                let _ = std::fs::remove_file(&old_file).ok();
                let _ = std::fs::remove_file(&new_file).ok();

                match output {
                    Ok(out) => {
                        let diff = String::from_utf8_lossy(&out.stdout);
                        if diff.is_empty() {
                            ToolResult::success("No differences found.".to_string())
                        } else {
                            ToolResult::success(diff.to_string())
                        }
                    }
                    Err(e) => ToolResult::error(format!("diff command failed: {}", e)),
                }
            }
            "file" => {
                let path = params["path"].as_str().unwrap_or("");
                if path.is_empty() {
                    return ToolResult::error("path is required for 'file' action");
                }

                let output = Command::new("git")
                    .args(["diff", "HEAD", "--", path])
                    .current_dir(".")
                    .output();

                match output {
                    Ok(out) if out.status.success() => {
                        let diff = String::from_utf8_lossy(&out.stdout);
                        if diff.is_empty() {
                            ToolResult::success(format!("No changes in {}", path))
                        } else {
                            ToolResult::success(diff.to_string())
                        }
                    }
                    Ok(out) => ToolResult::error(format!(
                        "git diff failed: {}",
                        String::from_utf8_lossy(&out.stderr)
                    )),
                    Err(e) => ToolResult::error(format!("Failed to run git diff: {}", e)),
                }
            }
            "compare" => {
                let old_path = params["old_path"].as_str().unwrap_or("");
                let new_path = params["new_path"].as_str().unwrap_or("");

                if old_path.is_empty() || new_path.is_empty() {
                    return ToolResult::error(
                        "old_path and new_path are required for 'compare' action",
                    );
                }

                let output = Command::new("diff")
                    .args(["-u", old_path, new_path])
                    .output();

                match output {
                    Ok(out) => {
                        let diff = String::from_utf8_lossy(&out.stdout);
                        if diff.is_empty() {
                            ToolResult::success("No differences found.".to_string())
                        } else {
                            ToolResult::success(diff.to_string())
                        }
                    }
                    Err(e) => ToolResult::error(format!("diff command failed: {}", e)),
                }
            }
            _ => ToolResult::error(format!("Unknown action: {}", action)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_diff_generate() {
        let tool = DiffTool;
        let result = tool
            .execute(
                json!({
                    "action": "generate",
                    "old_content": "line1\nline2\n",
                    "new_content": "line1\nline2 modified\nline3\n"
                }),
                ToolContext::new(".", "test"),
            )
            .await;
        assert!(result.success);
        assert!(
            result.content.contains("line2 modified") || result.content.contains("No differences")
        );
    }

    #[tokio::test]
    async fn test_diff_file() {
        let tool = DiffTool;
        let result = tool
            .execute(
                json!({ "action": "file", "path": "Cargo.toml" }),
                ToolContext::new(".", "test"),
            )
            .await;
        assert!(result.success);
    }
}
