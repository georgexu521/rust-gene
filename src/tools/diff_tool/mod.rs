//! Diff 工具 - 生成和查看代码差异

use crate::engine::checkpoint::{FileChangeRecord, FileChangeRoundSummary};
use crate::tools::{Tool, ToolContext, ToolOperationKind, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::process::Command;

/// Diff 工具
pub struct DiffTool;

#[async_trait]
impl Tool for DiffTool {
    fn name(&self) -> &str {
        "diff"
    }

    fn operation_kind(&self, _params: &Value) -> ToolOperationKind {
        ToolOperationKind::Read
    }

    fn description(&self) -> &str {
        "Generate or view diffs. Actions: 'generate' (diff two strings), 'file' (git diff for a file), 'compare' (diff two files), 'history' (recent checkpoint-backed file changes and rounds), 'file_change' (stored diff for a file change), 'tool_round' (combined stored diff for a tool round)."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["generate", "file", "compare", "history", "file_change", "tool_round"],
                    "description": "The diff action to perform"
                },
                "id": {
                    "type": "string",
                    "description": "File change ID for 'file_change' action or tool round ID for 'tool_round' action"
                },
                "path": {
                    "type": "string",
                    "description": "File path for 'file' action, or latest path match for 'file_change' action"
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

    async fn execute(&self, params: serde_json::Value, context: ToolContext) -> ToolResult {
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
            "history" => checkpoint_history_result(&context).await,
            "file_change" => checkpoint_file_change_diff_result(&context, &params).await,
            "tool_round" => checkpoint_tool_round_diff_result(&context, &params).await,
            _ => ToolResult::error(format!("Unknown action: {}", action)),
        }
    }
}

async fn checkpoint_history_result(context: &ToolContext) -> ToolResult {
    let manager = match &context.checkpoint_manager {
        Some(manager) => manager.clone(),
        None => crate::engine::checkpoint::get_checkpoint_manager(&context.session_id).await,
    };
    let checkpoint_guard = manager.lock().await;
    let file_changes = checkpoint_guard.list_file_changes();
    let file_change_rounds = checkpoint_guard.list_file_change_rounds();
    if file_changes.is_empty() {
        return ToolResult::success_with_data(
            "No checkpoint-backed file changes recorded for this session.",
            json!({ "file_changes": [], "file_change_rounds": [] }),
        );
    }

    let mut lines = vec!["Recent checkpoint-backed file changes:".to_string()];
    if !file_change_rounds.is_empty() {
        lines.push("Recent tool-round summaries:".to_string());
        for (idx, summary) in file_change_rounds.iter().rev().take(10).enumerate() {
            let round = summary
                .tool_round_id
                .as_deref()
                .unwrap_or("<single change>");
            lines.push(format!(
                "{}. {} | {} change(s), {} file(s), {} bytes",
                idx + 1,
                round,
                summary.change_count,
                summary.paths.len(),
                summary.total_bytes_written
            ));
        }
        lines.push("Recent file changes:".to_string());
    }
    for (idx, change) in file_changes.iter().rev().take(20).enumerate() {
        let round = change
            .tool_round_id
            .as_deref()
            .map(|round| format!(" | round {}", round))
            .unwrap_or_default();
        lines.push(format!(
            "{}. {} [{}] {} bytes | {}{}",
            idx + 1,
            change.id,
            change.tool_name,
            change.bytes_written,
            change.path,
            round
        ));
    }
    ToolResult::success_with_data(
        lines.join("\n"),
        json!({
            "file_changes": file_changes.iter().rev().take(20).collect::<Vec<_>>(),
            "file_change_rounds": file_change_rounds.iter().rev().take(10).collect::<Vec<_>>(),
        }),
    )
}

async fn checkpoint_file_change_diff_result(
    context: &ToolContext,
    params: &serde_json::Value,
) -> ToolResult {
    let manager = match &context.checkpoint_manager {
        Some(manager) => manager.clone(),
        None => crate::engine::checkpoint::get_checkpoint_manager(&context.session_id).await,
    };
    let checkpoint_guard = manager.lock().await;
    let file_changes = checkpoint_guard.list_file_changes();
    let change = if let Some(id) = params.get("id").and_then(|v| v.as_str()) {
        file_changes.iter().find(|change| change.id == id)
    } else if let Some(path) = params.get("path").and_then(|v| v.as_str()) {
        latest_file_change_for_path(file_changes, path)
    } else {
        file_changes.last()
    };

    let Some(change) = change else {
        return ToolResult::error("No matching checkpoint-backed file change found");
    };
    let diff = change
        .diff
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            format!(
                "No stored diff for file change {}.\nPath: {}\nTool: {}\nCheckpoint: {}",
                change.id, change.path, change.tool_name, change.checkpoint_id
            )
        });
    ToolResult::success_with_data(
        diff,
        json!({
            "file_change": change,
            "checkpoint_id": change.checkpoint_id,
        }),
    )
}

async fn checkpoint_tool_round_diff_result(
    context: &ToolContext,
    params: &serde_json::Value,
) -> ToolResult {
    let manager = match &context.checkpoint_manager {
        Some(manager) => manager.clone(),
        None => crate::engine::checkpoint::get_checkpoint_manager(&context.session_id).await,
    };
    let checkpoint_guard = manager.lock().await;
    let summary = if let Some(id) = params.get("id").and_then(|v| v.as_str()) {
        checkpoint_guard.file_change_round(id)
    } else {
        checkpoint_guard.latest_file_change_round()
    };

    let Some(summary) = summary else {
        return ToolResult::error("No matching checkpoint-backed tool round found");
    };
    let diff = summary
        .combined_diff
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| tool_round_no_diff_message(&summary));

    ToolResult::success_with_data(
        diff,
        json!({
            "file_change_round": summary,
        }),
    )
}

fn tool_round_no_diff_message(summary: &FileChangeRoundSummary) -> String {
    format!(
        "No stored diff for tool round {}.\nChanges: {}\nFiles: {}",
        summary
            .tool_round_id
            .as_deref()
            .unwrap_or("<single change>"),
        summary.change_count,
        summary.paths.join(", ")
    )
}

fn latest_file_change_for_path<'a>(
    file_changes: &'a [FileChangeRecord],
    path: &str,
) -> Option<&'a FileChangeRecord> {
    file_changes
        .iter()
        .rev()
        .find(|change| change.path == path || change.path.ends_with(path))
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

    #[tokio::test]
    async fn test_diff_file_change_uses_checkpoint_history() {
        let tool = DiffTool;
        let temp = tempfile::TempDir::new().unwrap();
        let file = temp.path().join("sample.txt");
        std::fs::write(&file, "before").unwrap();

        let mut checkpoint_manager =
            crate::engine::checkpoint::CheckpointManager::new("diff_tool_test").await;
        checkpoint_manager.clear_all().await.unwrap();
        let checkpoint = checkpoint_manager
            .create_checkpoint("file_write", None, None, std::slice::from_ref(&file))
            .await
            .unwrap();
        std::fs::write(&file, "after").unwrap();
        let change = checkpoint_manager
            .record_file_change(crate::engine::checkpoint::FileChangeInput {
                checkpoint_id: checkpoint.id,
                tool_name: "file_write".to_string(),
                tool_call_id: None,
                message_id: None,
                part_id: None,
                tool_round_id: None,
                path: file.to_string_lossy().to_string(),
                existed_before: true,
                before_hash: Some("before_hash".to_string()),
                after_hash: Some("after_hash".to_string()),
                diff: Some("--- sample.txt\n+++ sample.txt\n-before\n+after\n".to_string()),
                bytes_written: 5,
            })
            .await
            .unwrap();

        let manager = std::sync::Arc::new(tokio::sync::Mutex::new(checkpoint_manager));
        let context = ToolContext::new(temp.path(), "diff_tool_test")
            .with_checkpoint_manager(manager.clone());
        let result = tool
            .execute(json!({ "action": "file_change", "id": change.id }), context)
            .await;

        assert!(result.success, "{:?}", result.error);
        assert!(result.content.contains("+after"));

        manager.lock().await.clear_all().await.unwrap();
    }
}
