use super::history::{
    checkpoint_metadata_json, create_files_checkpoint, record_file_change, FileChangeRequest,
};
use super::text_codec::{
    detect_line_ending, read_text_file, split_leading_text_bom, text_write_format_json,
    write_text_file, TextFileEncoding, TextFileSnapshot,
};
use super::{
    acquire_file_mutation_lock, check_file_size_limit, edit_diff_summary, edit_diff_summary_json,
    edit_preview_json, file_path_identity, file_read_state_guidance, high_risk_file_target_result,
    is_file_modified_since_read, is_unc_or_network_path, mark_file_read_with_state,
    path_identity_json, read_before_edit_status, resolve_path, FileEditTool, FilePathIdentity,
    ReadBeforeEditStatus, MAX_EDITABLE_FILE_SIZE_BYTES,
};
use crate::engine::checkpoint::RestoreResult;
use crate::tools::{Tool, ToolContext, ToolOperationKind, ToolPermissionLevel, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::PathBuf;
use tokio::sync::OwnedMutexGuard;

const MAX_FILE_PATCH_OPERATIONS: usize = 20;

pub struct FilePatchTool;

#[derive(Clone, Debug)]
struct PatchSpec {
    path_arg: String,
    path: PathBuf,
    identity: FilePathIdentity,
    params: Value,
}

#[derive(Clone, Debug)]
struct PreparedPatch {
    path_arg: String,
    path: PathBuf,
    identity: FilePathIdentity,
    existed_before: bool,
    before_content: Option<String>,
    after_content: String,
    encoding: TextFileEncoding,
    has_bom: bool,
    line_ending: super::text_codec::LineEndingStyle,
    diff: super::EditDiffSummary,
    replacements: usize,
}

#[async_trait]
impl Tool for FilePatchTool {
    fn name(&self) -> &str {
        "file_patch"
    }

    fn description(&self) -> &str {
        "Apply N SEARCH/REPLACE edits across ONE OR MORE files in one call. \
         Every target file must have been file_read'd first — the tool refuses \
         the whole batch otherwise. Edits validate across the full batch before \
         writing; validation failures leave all files untouched. \
         Same per-edit rules as file_edit: search is exact text (whitespace \
         sensitive, no regex) and must be unique in its target file. \
         Use this for renames spanning multiple files, cross-file refactors, \
         or any batch where you'd otherwise loop file_edit."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "operations": {
                    "type": "array",
                    "description": "Patch operations to apply atomically. Each item requires path plus one of: old_string/new_string, line_start/line_end/new_string, insert_after/new_string, insert_before/new_string, or content for write mode.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "path": { "type": "string" },
                            "mode": {
                                "type": "string",
                                "enum": ["replace", "line_range", "insert_after", "insert_before", "write"],
                                "description": "Optional. Inferred from fields when omitted."
                            },
                            "old_string": { "type": "string" },
                            "new_string": { "type": "string" },
                            "insert_after": { "type": "string" },
                            "insert_before": { "type": "string" },
                            "line_start": { "type": "integer", "minimum": 1 },
                            "line_end": { "type": "integer", "minimum": 1 },
                            "content": { "type": "string" },
                            "expected_replacements": {
                                "type": "integer",
                                "minimum": 1,
                                "description": "Defaults to 1 for replace and insert operations."
                            },
                            "normalize_whitespace": { "type": "boolean", "default": false }
                        },
                        "required": ["path"]
                    }
                }
            },
            "required": ["operations"]
        })
    }

    fn to_classifier_input(&self, params: &Value) -> String {
        let count = params["operations"]
            .as_array()
            .map(|operations| operations.len())
            .unwrap_or(0);
        format!("file_patch: {count} operation(s)")
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["patch"]
    }

    fn search_hint(&self) -> Option<&'static str> {
        Some("coordinated multi file patch")
    }

    fn strict_schema(&self) -> bool {
        true
    }

    fn operation_kind(&self, _params: &Value) -> ToolOperationKind {
        ToolOperationKind::Patch
    }

    fn is_read_only(&self, _params: &Value) -> bool {
        false
    }

    fn is_concurrency_safe(&self, _params: &Value) -> bool {
        false
    }

    fn tool_use_summary(&self, params: &Value) -> Option<String> {
        let operations = params["operations"].as_array()?;
        Some(format!("{} operation(s)", operations.len()))
    }

    fn input_paths(&self, params: &Value) -> Vec<String> {
        params["operations"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|operation| operation.get("path").and_then(Value::as_str))
            .filter(|path| !path.trim().is_empty())
            .map(str::to_string)
            .collect()
    }

    async fn execute(&self, params: Value, context: ToolContext) -> ToolResult {
        if context.permissions.read_only {
            return ToolResult::error("Cannot patch files in read-only mode");
        }

        let operations = match params["operations"].as_array() {
            Some(operations) if !operations.is_empty() => operations,
            _ => return ToolResult::error("operations must be a non-empty array"),
        };
        if operations.len() > MAX_FILE_PATCH_OPERATIONS {
            return ToolResult::error(format!(
                "Refusing file_patch with {} operation(s): max is {}",
                operations.len(),
                MAX_FILE_PATCH_OPERATIONS
            ));
        }

        let specs = match prepare_specs(operations, &context) {
            Ok(specs) => specs,
            Err(result) => return result,
        };

        let _guards = acquire_patch_locks(&specs).await;
        let prepared = match preflight_operations(&specs, &context).await {
            Ok(prepared) => prepared,
            Err(err) => return ToolResult::error(err),
        };

        let checkpoint_paths = prepared
            .iter()
            .map(|patch| patch.path.clone())
            .collect::<Vec<_>>();
        let checkpoint = match create_files_checkpoint(&context, "file_patch", &checkpoint_paths)
            .await
        {
            Some(checkpoint) => checkpoint,
            None => {
                return ToolResult::error(
                    "file_patch could not create a checkpoint; refusing atomic patch without rollback",
                );
            }
        };

        let mut written_paths = Vec::new();
        let mut bytes_written = Vec::with_capacity(prepared.len());
        for patch in &prepared {
            if let Some(parent) = patch.path.parent() {
                if !parent.exists() {
                    if let Err(err) = tokio::fs::create_dir_all(parent).await {
                        return ToolResult::error(format!(
                            "file_patch failed to create parent directory {}: {}",
                            parent.display(),
                            err
                        ));
                    }
                }
            }
            let written = match write_text_file(
                &patch.path,
                &patch.after_content,
                patch.encoding,
                patch.has_bom,
                patch.line_ending,
                MAX_EDITABLE_FILE_SIZE_BYTES,
            )
            .await
            {
                Ok(written) => written,
                Err(err) => {
                    let rollback = restore_patch_checkpoint(&context, &checkpoint.id).await;
                    return patch_write_failure_result(
                        patch,
                        &checkpoint,
                        &written_paths,
                        err,
                        rollback,
                    );
                }
            };
            written_paths.push(patch.path.to_string_lossy().to_string());
            bytes_written.push(written as u64);
            if let Some(ref cache) = context.file_cache {
                cache.invalidate_content(&patch.path);
                cache.invalidate_metadata(&patch.path);
            }
            let new_mtime = std::fs::metadata(&patch.path)
                .map(|m| m.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH))
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            mark_file_read_with_state(
                &context.session_id,
                &patch.identity.state_key,
                &patch.after_content,
                new_mtime,
            );
        }

        let mut file_changes = Vec::new();
        for (patch, bytes_written) in prepared.iter().zip(bytes_written.iter().copied()) {
            if let Some(change) = record_file_change(
                &context,
                FileChangeRequest {
                    checkpoint: Some(&checkpoint),
                    tool_name: "file_patch",
                    path: &patch.path,
                    existed_before: patch.existed_before,
                    before_content: patch.before_content.as_deref(),
                    after_content: &patch.after_content,
                    diff: &patch.diff,
                    bytes_written,
                },
            )
            .await
            {
                file_changes.push(change);
            }
        }

        let checkpoint_json = checkpoint_metadata_json(Some(&checkpoint));
        let files = prepared
            .iter()
            .zip(bytes_written.iter().copied())
            .enumerate()
            .map(|(index, (patch, bytes_written))| {
                let text_format =
                    text_write_format_json(patch.encoding, patch.has_bom, patch.line_ending);
                let file_change = file_changes
                    .get(index)
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                let edit_preview = edit_preview_json(
                    &patch.identity,
                    patch.existed_before,
                    patch.before_content.as_deref(),
                    &patch.after_content,
                    &patch.diff,
                    text_format.clone(),
                    checkpoint_json.clone(),
                    file_change,
                    Some(patch.replacements),
                    bytes_written,
                    "patch_complete",
                );
                json!({
                    "path": patch.path_arg,
                    "resolved_path": patch.identity.resolved_path,
                    "path_identity": path_identity_json(&patch.identity),
                    "existed_before": patch.existed_before,
                    "replacements": patch.replacements,
                    "bytes_written": bytes_written,
                    "text_format": text_format,
                    "diff": edit_diff_summary_json(&patch.diff),
                    "edit_preview": edit_preview,
                })
            })
            .collect::<Vec<_>>();
        let combined_diff = prepared
            .iter()
            .map(|patch| patch.diff.unified_diff.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");

        ToolResult::success_with_data(
            format!(
                "Applied file_patch successfully: {} operation(s), {} file(s)",
                prepared.len(),
                prepared.len()
            ),
            json!({
                "operation_count": prepared.len(),
                "files": files,
                "checkpoint": checkpoint_json,
                "file_changes": file_changes,
                "diff": {
                    "unified_diff": combined_diff,
                    "file_count": prepared.len(),
                }
            }),
        )
    }

    fn requires_confirmation(&self, _params: &Value) -> bool {
        true
    }

    fn confirmation_prompt(&self, _params: &Value) -> Option<String> {
        Some("This will patch one or more files. Continue?".to_string())
    }

    fn permission_level(&self) -> ToolPermissionLevel {
        ToolPermissionLevel::MediumRisk
    }
}

async fn restore_patch_checkpoint(context: &ToolContext, checkpoint_id: &str) -> Value {
    let manager = match &context.checkpoint_manager {
        Some(manager) => manager.clone(),
        None => crate::engine::checkpoint::get_checkpoint_manager(&context.session_id).await,
    };
    let result = manager.lock().await.restore_checkpoint(checkpoint_id).await;
    rollback_metadata_json(checkpoint_id, result)
}

fn rollback_metadata_json(checkpoint_id: &str, result: Result<RestoreResult, String>) -> Value {
    match result {
        Ok(result) => {
            let failed_files = result
                .failed_files
                .into_iter()
                .map(|(path, error)| json!({ "path": path, "error": error }))
                .collect::<Vec<_>>();
            let success = failed_files.is_empty();
            json!({
                "attempted": true,
                "success": success,
                "checkpoint_id": result.checkpoint_id,
                "restored_files": result.restored_files,
                "removed_files": result.removed_files,
                "failed_files": failed_files,
            })
        }
        Err(error) => json!({
            "attempted": true,
            "success": false,
            "checkpoint_id": checkpoint_id,
            "error": error,
        }),
    }
}

fn patch_write_failure_result(
    patch: &PreparedPatch,
    checkpoint: &crate::engine::checkpoint::Checkpoint,
    written_paths: &[String],
    write_error: String,
    rollback: Value,
) -> ToolResult {
    let rollback_error = rollback.get("error").cloned().unwrap_or(Value::Null);
    let data = json!({
        "partial_failure": true,
        "failed_path": patch.path_arg,
        "failed_resolved_path": patch.identity.resolved_path,
        "checkpoint": checkpoint_metadata_json(Some(checkpoint)),
        "written_paths": written_paths,
        "rollback_attempted": rollback
            .get("attempted")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        "rollback_success": rollback
            .get("success")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        "rollback_error": rollback_error,
        "rollback": rollback,
    });
    let mut result = ToolResult::error(format!(
        "file_patch failed while writing {}: {}",
        patch.path.display(),
        write_error
    ));
    result.content = data.to_string();
    result.data = Some(data);
    result
}

#[allow(clippy::result_large_err)]
fn prepare_specs(
    operations: &[Value],
    context: &ToolContext,
) -> Result<Vec<PatchSpec>, ToolResult> {
    let mut seen = HashSet::new();
    let mut specs = Vec::with_capacity(operations.len());
    for operation in operations {
        let path_arg = operation["path"]
            .as_str()
            .ok_or_else(|| ToolResult::error("file_patch operation missing path"))?;
        if path_arg.trim().is_empty() {
            return Err(ToolResult::error(
                "file_patch operation path cannot be empty",
            ));
        }
        if is_unc_or_network_path(path_arg) {
            return Err(ToolResult::error(format!(
                "Refusing to patch UNC/network path '{}'. Use a local path instead.",
                path_arg
            )));
        }
        let path = match resolve_path(path_arg, &context.working_dir) {
            Ok(path) => path,
            Err(msg) => return Err(ToolResult::error(msg)),
        };
        let identity = file_path_identity(path_arg, &path, &context.working_dir);
        if let Some(result) =
            high_risk_file_target_result(&path, &identity, &context.working_dir, "file_patch")
        {
            return Err(result);
        }
        if !seen.insert(identity.state_key.clone()) {
            return Err(ToolResult::error(format!(
                "file_patch has multiple operations for the same file: {}",
                path_arg
            )));
        }
        specs.push(PatchSpec {
            path_arg: path_arg.to_string(),
            path,
            identity,
            params: operation.clone(),
        });
    }
    Ok(specs)
}

async fn acquire_patch_locks(specs: &[PatchSpec]) -> Vec<OwnedMutexGuard<()>> {
    let mut keys = specs
        .iter()
        .map(|spec| spec.identity.state_key.clone())
        .collect::<Vec<_>>();
    keys.sort();
    let mut guards = Vec::with_capacity(keys.len());
    for key in keys {
        guards.push(acquire_file_mutation_lock(&key).await);
    }
    guards
}

async fn preflight_operations(
    specs: &[PatchSpec],
    context: &ToolContext,
) -> Result<Vec<PreparedPatch>, String> {
    let mut prepared = Vec::with_capacity(specs.len());
    for spec in specs {
        prepared.push(preflight_operation(spec, context).await?);
    }
    Ok(prepared)
}

async fn preflight_operation(
    spec: &PatchSpec,
    context: &ToolContext,
) -> Result<PreparedPatch, String> {
    let params = &spec.params;
    let mode = infer_mode(params)?;
    let existed_before = spec.path.exists();
    let before_snapshot = if existed_before {
        if !spec.path.is_file() {
            return Err(format!(
                "file_patch target is not a file: {}",
                spec.path_arg
            ));
        }
        check_file_size_limit(&spec.path, "patch")?;
        Some(read_text_file(&spec.path, "patch").await?)
    } else {
        None
    };

    let (before_content, after_content, encoding, has_bom, line_ending, replacements) =
        match (mode, before_snapshot.as_ref()) {
            (PatchMode::Write, snapshot) => {
                let content = params["content"]
                    .as_str()
                    .ok_or_else(|| "file_patch write operation requires content".to_string())?;
                if let Some(snapshot) = snapshot {
                    ensure_existing_file_read_is_fresh(spec, context, PatchMode::Write, snapshot)?;
                }
                let (content_has_bom, content_body) = split_leading_text_bom(content);
                let encoding = snapshot
                    .map(|snapshot| snapshot.encoding)
                    .unwrap_or(TextFileEncoding::Utf8);
                let has_bom =
                    snapshot.map(|snapshot| snapshot.has_bom).unwrap_or(false) || content_has_bom;
                let line_ending = snapshot
                    .map(|snapshot| snapshot.line_ending)
                    .unwrap_or_else(|| detect_line_ending(content_body));
                (
                    snapshot.map(|snapshot| snapshot.content.clone()),
                    content_body.to_string(),
                    encoding,
                    has_bom,
                    line_ending,
                    1,
                )
            }
            (_, None) => {
                return Err(format!(
                    "file_patch {} operation requires an existing file: {}",
                    mode.as_str(),
                    spec.path_arg
                ));
            }
            (mode, Some(snapshot)) => {
                ensure_existing_file_read_is_fresh(spec, context, mode, snapshot)?;
                let (new_content, replacements) =
                    apply_existing_file_operation(mode, params, snapshot.content.clone())?;
                (
                    Some(snapshot.content.clone()),
                    new_content,
                    snapshot.encoding,
                    snapshot.has_bom,
                    snapshot.line_ending,
                    replacements,
                )
            }
        };

    if after_content.len() as u64 > MAX_EDITABLE_FILE_SIZE_BYTES {
        return Err(format!(
            "Refusing file_patch content larger than {} bytes for {}",
            MAX_EDITABLE_FILE_SIZE_BYTES, spec.path_arg
        ));
    }

    let diff = edit_diff_summary(
        &spec.identity.display_path,
        before_content.as_deref().unwrap_or(""),
        &after_content,
    );

    Ok(PreparedPatch {
        path_arg: spec.path_arg.clone(),
        path: spec.path.clone(),
        identity: spec.identity.clone(),
        existed_before,
        before_content,
        after_content,
        encoding,
        has_bom,
        line_ending,
        diff,
        replacements,
    })
}

fn ensure_existing_file_read_is_fresh(
    spec: &PatchSpec,
    context: &ToolContext,
    mode: PatchMode,
    snapshot: &TextFileSnapshot,
) -> Result<(), String> {
    ensure_read_state(spec, context, mode)?;
    let current_mtime = std::fs::metadata(&spec.path)
        .map(|m| m.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH))
        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
    if is_file_modified_since_read(
        &context.session_id,
        &spec.identity.state_key,
        &snapshot.content,
        current_mtime,
    ) {
        return Err(format!(
            "Refusing file_patch for '{}': file changed since this session last read it. Re-read the file and retry.",
            spec.path_arg
        ));
    }
    Ok(())
}

fn ensure_read_state(
    spec: &PatchSpec,
    context: &ToolContext,
    mode: PatchMode,
) -> Result<(), String> {
    let (line_start, line_end) = if mode == PatchMode::LineRange {
        (
            spec.params["line_start"]
                .as_u64()
                .map(|value| value as usize),
            spec.params["line_end"].as_u64().map(|value| value as usize),
        )
    } else {
        (None, None)
    };
    let status = read_before_edit_status(
        &context.session_id,
        &spec.identity.state_key,
        line_start,
        line_end,
    );
    if status == ReadBeforeEditStatus::Allowed {
        Ok(())
    } else {
        Err(file_read_state_guidance(&spec.path_arg, status))
    }
}

fn apply_existing_file_operation(
    mode: PatchMode,
    params: &Value,
    content: String,
) -> Result<(String, usize), String> {
    let new_string = params["new_string"].as_str().unwrap_or("");
    let expected = params["expected_replacements"]
        .as_u64()
        .map(|value| value as usize)
        .or(Some(1));
    match mode {
        PatchMode::Replace => FileEditTool::do_replace(
            content,
            params["old_string"].as_str().unwrap_or(""),
            new_string,
            expected,
            params["normalize_whitespace"].as_bool().unwrap_or(false),
        ),
        PatchMode::LineRange => {
            let line_start = params["line_start"]
                .as_u64()
                .ok_or_else(|| "line_range operation requires line_start".to_string())?
                as usize;
            let line_end = params["line_end"]
                .as_u64()
                .ok_or_else(|| "line_range operation requires line_end".to_string())?
                as usize;
            FileEditTool::do_replace_lines(content, line_start, line_end, new_string)
        }
        PatchMode::InsertAfter => {
            let anchor = params["insert_after"]
                .as_str()
                .ok_or_else(|| "insert_after operation requires insert_after".to_string())?;
            let (updated, count) = FileEditTool::do_insert(
                content,
                anchor,
                new_string,
                super::InsertMode::After,
                expected,
            )?;
            Ok((updated, count))
        }
        PatchMode::InsertBefore => {
            let anchor = params["insert_before"]
                .as_str()
                .ok_or_else(|| "insert_before operation requires insert_before".to_string())?;
            let (updated, count) = FileEditTool::do_insert(
                content,
                anchor,
                new_string,
                super::InsertMode::Before,
                expected,
            )?;
            Ok((updated, count))
        }
        PatchMode::Write => unreachable!("write mode is handled before existing-file operations"),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PatchMode {
    Replace,
    LineRange,
    InsertAfter,
    InsertBefore,
    Write,
}

impl PatchMode {
    fn as_str(self) -> &'static str {
        match self {
            PatchMode::Replace => "replace",
            PatchMode::LineRange => "line_range",
            PatchMode::InsertAfter => "insert_after",
            PatchMode::InsertBefore => "insert_before",
            PatchMode::Write => "write",
        }
    }
}

fn infer_mode(params: &Value) -> Result<PatchMode, String> {
    if let Some(mode) = params["mode"].as_str() {
        return match mode {
            "replace" => Ok(PatchMode::Replace),
            "line_range" => Ok(PatchMode::LineRange),
            "insert_after" => Ok(PatchMode::InsertAfter),
            "insert_before" => Ok(PatchMode::InsertBefore),
            "write" => Ok(PatchMode::Write),
            other => Err(format!("Unsupported file_patch mode: {}", other)),
        };
    }
    if params.get("content").and_then(Value::as_str).is_some() {
        return Ok(PatchMode::Write);
    }
    if params.get("line_start").and_then(Value::as_u64).is_some()
        || params.get("line_end").and_then(Value::as_u64).is_some()
    {
        return Ok(PatchMode::LineRange);
    }
    if params.get("insert_after").and_then(Value::as_str).is_some() {
        return Ok(PatchMode::InsertAfter);
    }
    if params
        .get("insert_before")
        .and_then(Value::as_str)
        .is_some()
    {
        return Ok(PatchMode::InsertBefore);
    }
    Ok(PatchMode::Replace)
}
