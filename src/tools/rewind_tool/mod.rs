//! Rewind tool
//!
//! Restore checkpoint-backed file changes from the shared file history.

use crate::engine::checkpoint::{FileChangeRecord, RestoreResult};
use crate::tools::Tool;
use crate::tools::ToolContext;
use crate::tools::ToolResult;
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct RewindTool;

#[async_trait]
impl Tool for RewindTool {
    fn name(&self) -> &str {
        "rewind"
    }

    fn description(&self) -> &str {
        "Rewind checkpoint-backed file changes. Supports latest_file_change, file_change_id, checkpoint_id, path, or legacy steps=1."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "target": {
                    "type": "string",
                    "enum": ["latest_file_change", "file_change_id", "checkpoint_id", "path"],
                    "description": "What to restore. Defaults to latest_file_change."
                },
                "id": {
                    "type": "string",
                    "description": "File change ID (fc_...) or checkpoint ID (cp_...), depending on target."
                },
                "path": {
                    "type": "string",
                    "description": "Restore the latest tracked file change for this path."
                },
                "steps": {
                    "type": "integer",
                    "description": "Legacy alias for restoring the Nth most recent file change; 1 restores the latest."
                }
            },
            "required": []
        })
    }

    async fn execute(&self, params: Value, context: ToolContext) -> ToolResult {
        let manager = match &context.checkpoint_manager {
            Some(manager) => manager.clone(),
            None => crate::engine::checkpoint::get_checkpoint_manager(&context.session_id).await,
        };
        let checkpoint_guard = manager.lock().await;
        let target = params
            .get("target")
            .and_then(|v| v.as_str())
            .unwrap_or("latest_file_change");

        let restore_result = match target {
            "latest_file_change" => {
                if let Some(steps) = params.get("steps").and_then(|v| v.as_u64()) {
                    if steps == 0 {
                        return ToolResult::error("steps must be greater than 0");
                    }
                    let file_changes = checkpoint_guard.list_file_changes();
                    let Some(change) = file_changes.iter().rev().nth(steps as usize - 1) else {
                        return ToolResult::error(format!(
                            "Only {} checkpoint-backed file change(s) are available",
                            file_changes.len()
                        ));
                    };
                    checkpoint_guard.restore_file_change(&change.id).await
                } else {
                    checkpoint_guard.restore_latest_file_change().await
                }
            }
            "file_change_id" => {
                let Some(id) = params.get("id").and_then(|v| v.as_str()) else {
                    return ToolResult::error("id is required for target=file_change_id");
                };
                checkpoint_guard.restore_file_change(id).await
            }
            "checkpoint_id" => {
                let Some(id) = params.get("id").and_then(|v| v.as_str()) else {
                    return ToolResult::error("id is required for target=checkpoint_id");
                };
                checkpoint_guard.restore_checkpoint(id).await
            }
            "path" => {
                let Some(path) = params.get("path").and_then(|v| v.as_str()) else {
                    return ToolResult::error("path is required for target=path");
                };
                let file_changes = checkpoint_guard.list_file_changes();
                let Some(change) = latest_file_change_for_path(file_changes, path) else {
                    return ToolResult::error(format!(
                        "No checkpoint-backed file change recorded for path: {}",
                        path
                    ));
                };
                checkpoint_guard.restore_file_change(&change.id).await
            }
            other => return ToolResult::error(format!("Unknown rewind target: {}", other)),
        };

        match restore_result {
            Ok(result) => rewind_success_result(result),
            Err(err) => ToolResult::error(format!("Failed to rewind file change: {}", err)),
        }
    }
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

fn rewind_success_result(result: RestoreResult) -> ToolResult {
    let failed_files = result
        .failed_files
        .iter()
        .map(|(path, error)| json!({ "path": path, "error": error }))
        .collect::<Vec<_>>();
    let mut lines = vec![format!(
        "Rewound file state using checkpoint: {}",
        result.checkpoint_id
    )];
    if !result.restored_files.is_empty() {
        lines.push(format!("Restored {} file(s).", result.restored_files.len()));
    }
    if !result.removed_files.is_empty() {
        lines.push(format!(
            "Removed {} file(s) that did not exist before the change.",
            result.removed_files.len()
        ));
    }
    if !failed_files.is_empty() {
        lines.push(format!("Failed to restore {} file(s).", failed_files.len()));
    }

    ToolResult::success_with_data(
        lines.join("\n"),
        json!({
            "checkpoint_id": result.checkpoint_id,
            "restored_files": result.restored_files,
            "removed_files": result.removed_files,
            "failed_files": failed_files,
            "success": failed_files.is_empty(),
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::checkpoint::{CheckpointManager, FileChangeInput};
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::sync::Mutex;

    #[tokio::test]
    async fn rewind_latest_file_change_restores_checkpoint() {
        let temp = TempDir::new().unwrap();
        let file = temp.path().join("sample.txt");
        std::fs::write(&file, "before").unwrap();

        let mut checkpoint_manager = CheckpointManager::new("rewind_tool_test").await;
        checkpoint_manager.clear_all().await.unwrap();
        let checkpoint = checkpoint_manager
            .create_checkpoint("file_write", None, None, std::slice::from_ref(&file))
            .await
            .unwrap();
        std::fs::write(&file, "after").unwrap();
        checkpoint_manager
            .record_file_change(FileChangeInput {
                checkpoint_id: checkpoint.id,
                tool_name: "file_write".to_string(),
                tool_call_id: None,
                path: file.to_string_lossy().to_string(),
                existed_before: true,
                before_hash: Some("before_hash".to_string()),
                after_hash: Some("after_hash".to_string()),
                diff: Some("-before\n+after\n".to_string()),
                bytes_written: 5,
            })
            .await
            .unwrap();

        let manager = Arc::new(Mutex::new(checkpoint_manager));
        let context = ToolContext::new(temp.path(), "rewind_tool_test")
            .with_checkpoint_manager(manager.clone());
        let result = RewindTool
            .execute(json!({ "target": "latest_file_change" }), context)
            .await;

        assert!(result.success, "{:?}", result.error);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "before");
        let data = result.data.unwrap();
        assert_eq!(data["restored_files"].as_array().unwrap().len(), 1);

        manager.lock().await.clear_all().await.unwrap();
    }
}
