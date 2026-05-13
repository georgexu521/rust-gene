use super::EditDiffSummary;
use crate::engine::checkpoint::{Checkpoint, FileChangeInput, FileChangeRecord};
use crate::tools::ToolContext;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use tracing::warn;

pub(super) struct FileChangeRequest<'a> {
    pub(super) checkpoint: Option<&'a Checkpoint>,
    pub(super) tool_name: &'a str,
    pub(super) path: &'a Path,
    pub(super) existed_before: bool,
    pub(super) before_content: Option<&'a str>,
    pub(super) after_content: &'a str,
    pub(super) diff: &'a EditDiffSummary,
    pub(super) bytes_written: u64,
}

pub(super) async fn create_file_checkpoint(
    context: &ToolContext,
    tool_name: &str,
    path: &Path,
) -> Option<Checkpoint> {
    create_files_checkpoint(context, tool_name, &[path.to_path_buf()]).await
}

pub(super) async fn create_files_checkpoint(
    context: &ToolContext,
    tool_name: &str,
    paths: &[PathBuf],
) -> Option<Checkpoint> {
    let manager = match &context.checkpoint_manager {
        Some(manager) => manager.clone(),
        None => crate::engine::checkpoint::get_checkpoint_manager(&context.session_id).await,
    };

    let mut checkpoint_manager = manager.lock().await;
    match checkpoint_manager
        .create_checkpoint(
            tool_name,
            None,
            context.metadata.get("tool_call_id").cloned(),
            paths,
        )
        .await
    {
        Ok(checkpoint) => Some(checkpoint),
        Err(err) => {
            warn!("Failed to create checkpoint for {}: {}", tool_name, err);
            None
        }
    }
}

pub(super) async fn record_file_change(
    context: &ToolContext,
    request: FileChangeRequest<'_>,
) -> Option<Value> {
    let checkpoint = request.checkpoint?;
    let manager = match &context.checkpoint_manager {
        Some(manager) => manager.clone(),
        None => crate::engine::checkpoint::get_checkpoint_manager(&context.session_id).await,
    };

    let input = FileChangeInput {
        checkpoint_id: checkpoint.id.clone(),
        tool_name: request.tool_name.to_string(),
        tool_call_id: context.metadata.get("tool_call_id").cloned(),
        path: request.path.to_string_lossy().to_string(),
        existed_before: request.existed_before,
        before_hash: request.before_content.map(stable_text_hash),
        after_hash: Some(stable_text_hash(request.after_content)),
        diff: Some(request.diff.unified_diff.clone()),
        bytes_written: request.bytes_written,
    };

    let mut checkpoint_manager = manager.lock().await;
    match checkpoint_manager.record_file_change(input).await {
        Ok(record) => Some(file_change_record_json(&record)),
        Err(err) => {
            warn!(
                "Failed to record file change for {}: {}",
                request.path.display(),
                err
            );
            None
        }
    }
}

pub(super) fn checkpoint_metadata_json(checkpoint: Option<&Checkpoint>) -> Value {
    checkpoint
        .map(|checkpoint| {
            json!({
                "id": checkpoint.id.clone(),
                "sequence": checkpoint.sequence,
                "tool_name": checkpoint.tool_name.clone(),
                "tool_call_id": checkpoint.tool_call_id.clone(),
                "timestamp": checkpoint.timestamp.to_rfc3339(),
            })
        })
        .unwrap_or(Value::Null)
}

fn file_change_record_json(record: &FileChangeRecord) -> Value {
    json!({
        "id": record.id.clone(),
        "checkpoint_id": record.checkpoint_id.clone(),
        "checkpoint_sequence": record.checkpoint_sequence,
        "session_id": record.session_id.clone(),
        "tool_name": record.tool_name.clone(),
        "tool_call_id": record.tool_call_id.clone(),
        "timestamp": record.timestamp.to_rfc3339(),
        "path": record.path.clone(),
        "existed_before": record.existed_before,
        "before_hash": record.before_hash.clone(),
        "after_hash": record.after_hash.clone(),
        "diff": record.diff.clone(),
        "bytes_written": record.bytes_written,
    })
}

fn stable_text_hash(content: &str) -> String {
    format!("{:x}", md5::compute(content))
}
