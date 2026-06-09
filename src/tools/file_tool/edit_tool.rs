use super::*;
use crate::tools::{Tool, ToolContext, ToolOperationKind, ToolResult};
use async_trait::async_trait;
use serde_json::json;

/// 智能引号归一化（Claude Code 模式）
/// 处理文件中的智能引号 vs 模型输出的直引号差异
fn normalize_quotes(input: &str) -> String {
    input
        .replace(['\u{2018}', '\u{2019}', '\u{201A}', '\u{201B}'], "'") // single quotes
        .replace(['\u{201C}', '\u{201D}'], "\"") // double quotes
}

/// 反转义处理（Claude Code 使用 &lt;fnr&gt; 等转义）
fn desanitize(input: &str) -> String {
    input
        .replace("<fnr>", "")
        .replace("<n>", "\n")
        .replace("<TAB>", "\t")
        .replace("<NEWLINE>", "\n")
}

pub struct FileEditTool;

fn exact_replace_preflight_error(
    identity: &FilePathIdentity,
    content: &str,
    old_string: &str,
    new_string: &str,
    expected: Option<usize>,
    normalize_whitespace: bool,
) -> Option<ToolResult> {
    let base_data = |failure: &str| {
        json!({
            "failure": failure,
            "path_identity": path_identity_json(identity),
            "operation": "exact_replace",
            "recovery": {
                "recommended_action": "adjust_anchor",
                "next_actions": ["file_read", "use_line_start_line_end", "retry_file_edit"],
            }
        })
    };

    if old_string.trim().is_empty() {
        let message = "old_string cannot be empty or whitespace-only unless insert_after, insert_before, or line_start/line_end is used. For a known target line, use line_start and line_end instead.";
        return Some(file_edit_error_with_data(
            message,
            json!({
                "failure": "empty_old_string",
                "path_identity": path_identity_json(identity),
                "operation": "exact_replace",
                "recovery": {
                    "recommended_action": "use_line_range",
                    "next_actions": ["file_read", "use_line_start_line_end", "retry_file_edit"],
                }
            }),
        ));
    }

    if old_string == new_string {
        return Some(file_edit_error_with_data(
            "Refusing file_edit no-op: old_string and new_string are identical.",
            json!({
                "failure": "no_op_edit",
                "path_identity": path_identity_json(identity),
                "operation": "exact_replace",
                "old_hash": content_hash_hex(old_string),
                "new_hash": content_hash_hex(new_string),
                "recovery": {
                    "recommended_action": "change_replacement_or_skip",
                    "next_actions": ["skip_edit", "provide_different_new_string"],
                }
            }),
        ));
    }

    if contains_file_read_line_prefix(old_string) {
        return Some(file_edit_error_with_data(
            file_read_line_prefix_guidance("old_string"),
            json!({
                "failure": "file_read_line_prefix_in_old_string",
                "path_identity": path_identity_json(identity),
                "operation": "exact_replace",
                "recovery": {
                    "recommended_action": "remove_display_line_prefix",
                    "next_actions": ["copy_text_after_pipe", "use_line_start_line_end", "retry_file_edit"],
                }
            }),
        ));
    }

    let occurrences = if normalize_whitespace {
        find_occurrences_normalized(content, old_string)
    } else {
        find_occurrences(content, old_string)
    };
    let expected_count = expected.unwrap_or(1);
    let max_replacements = max_file_edit_replacements();

    if expected_count > max_replacements {
        return Some(file_edit_error_with_data(
            format!(
                "Refusing file_edit with {} replacement(s): exceeds safety limit {}. Use narrower anchors or set PRIORITY_AGENT_MAX_FILE_EDIT_REPLACEMENTS explicitly for deliberate bulk edits.",
                expected_count, max_replacements
            ),
            json!({
                "failure": "replacement_limit_exceeded",
                "path_identity": path_identity_json(identity),
                "operation": "exact_replace",
                "expected_replacements": expected_count,
                "max_replacements": max_replacements,
                "recovery": {
                    "recommended_action": "narrow_anchor",
                    "next_actions": ["use_more_specific_old_string", "use_line_start_line_end"],
                }
            }),
        ));
    }

    if occurrences.is_empty() {
        let fuzzy = fuzzy_find_occurrences(content, old_string);
        let mut data = base_data("old_string_not_found");
        let candidate_outcome = generate_edit_candidates(content, old_string, &occurrences);
        if let EditCandidateOutcome::AutoApplied { replacements, .. } = &candidate_outcome {
            if *replacements == expected_count {
                return None;
            }
        }
        data["match_diagnostics"] = json!({
            "expected_occurrences": expected_count,
            "exact_occurrences": 0,
            "fuzzy_occurrences": fuzzy.len(),
            "fuzzy_lines": occurrence_line_numbers(content, &fuzzy),
            "context": if fuzzy.is_empty() {
                serde_json::Value::Null
            } else {
                json!(build_match_context(content, &fuzzy, 2))
            },
        });
        match candidate_outcome {
            EditCandidateOutcome::AutoApplied {
                replacements,
                strategy,
                ..
            } => {
                data["match_diagnostics"]["recovery"] = json!({
                    "status": "auto_candidate_available_but_rejected",
                    "strategy": strategy,
                    "replacements": replacements,
                    "expected_replacements": expected_count,
                });
            }
            EditCandidateOutcome::Candidates { candidates, count } => {
                data["match_diagnostics"]["candidates"] = json!({
                    "count": count,
                    "items": candidates.iter().map(EditCandidate::to_json).collect::<Vec<_>>(),
                });
            }
            EditCandidateOutcome::Mismatch { detail } => {
                data["match_diagnostics"]["candidate_detail"] = json!(detail);
            }
        }
        data["recovery"]["recommended_action"] = if fuzzy.is_empty() {
            json!("re_read_once_then_line_range_edit")
        } else {
            json!("copy_exact_fuzzy_match")
        };
        let message = if fuzzy.is_empty() {
            "Could not find old_string in the file. It must match exactly, including whitespace, indentation, and line endings.\n\n\
             Common issues:\n\
             - Line number prefixes from file_read output (e.g. '12 | ')\n\
             - Smart quotes vs straight quotes\n\
             - Escaped characters (literal \\n vs actual newline)\n\
             - Trailing whitespace differences\n\n\
             Tip: try file_read to verify the exact text, or use line_start/line_end for precise replacement."
                .to_string()
        } else {
            format!(
                "old_string not found exactly, but fuzzy matches found:\n{}\n\nPlease adjust old_string to match one of these occurrences precisely.",
                build_match_context(content, &fuzzy, 2)
            )
        };
        return Some(file_edit_error_with_data(message, data));
    }

    if occurrences.len() != expected_count {
        let ctx = build_match_context(content, &occurrences, 2);
        return Some(file_edit_error_with_data(
            format!(
                "Expected {} occurrence(s) of old_string, but found {}.\n{}\n\nPlease provide a more specific old_string or set expected_replacements to {}.",
                expected_count,
                occurrences.len(),
                ctx,
                occurrences.len()
            ),
            json!({
                "failure": "old_string_occurrence_mismatch",
                "path_identity": path_identity_json(identity),
                "operation": "exact_replace",
                "match_diagnostics": {
                    "expected_occurrences": expected_count,
                    "actual_occurrences": occurrences.len(),
                    "lines": occurrence_line_numbers(content, &occurrences),
                    "context": ctx,
                },
                "recovery": {
                    "recommended_action": "narrow_anchor",
                    "next_actions": ["use_more_specific_old_string", "use_line_start_line_end", "set_expected_replacements_if_intentional"],
                    "safe_expected_replacements": occurrences.len(),
                }
            }),
        ));
    }

    None
}

#[async_trait]
impl Tool for FileEditTool {
    fn name(&self) -> &str {
        "file_edit"
    }

    fn description(&self) -> &str {
        "Apply a SEARCH/REPLACE edit to an existing file. \
         You MUST call file_read on this path first — the tool refuses otherwise, \
         since SEARCH must match on-disk bytes exactly. \
         \
         `old_string` is whitespace-sensitive plain text (no regex) and must be \
         UNIQUE in the file; otherwise the edit is refused to avoid surprise rewrites. \
         Do NOT include file_read display prefixes like `12 |`; those are not file content. \
         \
         If you're unsure about exact whitespace, use line_start + line_end instead: \
         set both (1-indexed, inclusive) and provide new_string for a reliable range replace. \
         For coordinated changes across multiple files, use file_patch instead."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path to the file to edit"
                },
                "old_string": {
                    "type": "string",
                    "description": "The text to find and replace"
                },
                "new_string": {
                    "type": "string",
                    "description": "The text to replace old_string with"
                },
                "expected_replacements": {
                    "type": "integer",
                    "description": "How many times old_string or an insert anchor should appear. Defaults to 1. Use values greater than 1 only for deliberate mass edits.",
                    "minimum": 1
                },
                "insert_after": {
                    "type": "string",
                    "description": "If provided, new_string will be inserted after this anchor (old_string is ignored when this is set). The anchor must appear expected_replacements times, default 1."
                },
                "insert_before": {
                    "type": "string",
                    "description": "If provided, new_string will be inserted before this anchor (old_string is ignored when this is set). The anchor must appear expected_replacements times, default 1."
                },
                "line_start": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "If provided, replaces lines line_start..=line_end with new_string (old_string is ignored). 1-indexed."
                },
                "line_end": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "End line number for line-range replacement (inclusive). Must be paired with line_start."
                },
                "normalize_whitespace": {
                    "type": "boolean",
                    "default": false,
                    "description": "If true, ignores leading/trailing whitespace differences when matching old_string."
                },
                "allow_stale_read": {
                    "type": "boolean",
                    "default": false,
                    "description": "Allow editing even when the file changed since this session last read it. Use only for intentional overwrites."
                }
            },
            "required": ["path", "new_string"]
        })
    }

    fn to_classifier_input(&self, params: &serde_json::Value) -> String {
        let path = params["path"].as_str().unwrap_or("");
        let has_old = params["old_string"].as_str().is_some();
        let has_lines = params["line_start"].as_u64().is_some();
        let mode = if has_old {
            "exact"
        } else if has_lines {
            "line_range"
        } else {
            "insert"
        };
        format!("file_edit: {} ({})", path, mode)
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["edit"]
    }

    fn search_hint(&self) -> Option<&'static str> {
        Some("replace insert file text")
    }

    fn strict_schema(&self) -> bool {
        true
    }

    fn operation_kind(&self, _params: &serde_json::Value) -> ToolOperationKind {
        ToolOperationKind::Edit
    }

    fn is_read_only(&self, _params: &serde_json::Value) -> bool {
        false
    }

    fn is_concurrency_safe(&self, _params: &serde_json::Value) -> bool {
        false
    }

    fn tool_use_summary(&self, params: &serde_json::Value) -> Option<String> {
        let path = params["path"].as_str()?.trim();
        if path.is_empty() {
            return None;
        }
        let mode = if params["old_string"].as_str().is_some() {
            "replace"
        } else if params["line_start"].as_u64().is_some() {
            "line_range"
        } else if params["insert_after"].as_str().is_some() {
            "insert_after"
        } else if params["insert_before"].as_str().is_some() {
            "insert_before"
        } else {
            "edit"
        };
        Some(format!("{path} ({mode})"))
    }

    async fn execute(&self, params: serde_json::Value, context: ToolContext) -> ToolResult {
        // 检查只读模式
        if context.permissions.read_only {
            return ToolResult::error("Cannot edit files in read-only mode");
        }

        let path_str = params["path"].as_str().unwrap_or("");
        let old_string = params["old_string"].as_str().unwrap_or("");
        let new_string = params["new_string"].as_str().unwrap_or("");
        let insert_after = params["insert_after"].as_str();
        let insert_before = params["insert_before"].as_str();
        let expected_replacements = params["expected_replacements"].as_u64().map(|n| n as usize);
        let line_start = params["line_start"].as_u64().map(|n| n as usize);
        let line_end = params["line_end"].as_u64().map(|n| n as usize);
        let normalize_ws = params["normalize_whitespace"].as_bool().unwrap_or(false);
        let allow_stale_read = params["allow_stale_read"].as_bool().unwrap_or(false);

        if path_str.is_empty() {
            return ToolResult::error("Path cannot be empty");
        }
        if is_unc_or_network_path(path_str) {
            return ToolResult::error(format!(
                "Refusing to edit UNC/network path '{}'. Use a local path instead.",
                path_str
            ));
        }

        let path = match resolve_path(path_str, &context.working_dir) {
            Ok(path) => path,
            Err(msg) => return ToolResult::error(msg),
        };
        info!("Editing file: {:?}", path);
        let identity = file_path_identity(path_str, &path, &context.working_dir);
        if let Some(result) =
            high_risk_file_target_result(&path, &identity, &context.working_dir, "file_edit")
        {
            return result;
        }
        let state_key = identity.state_key.clone();
        let file_guard = acquire_file_mutation_lock(&state_key).await;

        // 读取文件内容
        if let Err(msg) = check_file_size_limit(&path, "edit") {
            return ToolResult::error(msg);
        }
        let snapshot = match read_text_file(&path, "edit").await {
            Ok(snapshot) => snapshot,
            Err(e) => {
                return ToolResult::error(e);
            }
        };
        let content = snapshot.content.clone();

        // ── Edit safety checks ────────────────────────────────────────
        // Claude-like write discipline: existing files must be read in this
        // session before mutation so stale/partial context cannot silently win.
        if !allow_edit_without_read() {
            let status =
                read_before_edit_status(&context.session_id, &state_key, line_start, line_end);
            if status != ReadBeforeEditStatus::Allowed {
                return ToolResult::error(file_read_state_guidance(path_str, status));
            }
            // ReadTracker — simpler path-level guard, cleared on context fold.
            if let Some(ref tracker) = context.read_tracker {
                if let Err(msg) = tracker.check_edit(&path, old_string) {
                    return ToolResult::error(msg);
                }
            }
        }

        // 2. 文件修改检测：检查文件是否在读取后被外部修改
        let current_mtime = std::fs::metadata(&path)
            .map(|m| m.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH))
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        if is_file_modified_since_read(&context.session_id, &state_key, &content, current_mtime) {
            if !allow_stale_read {
                let message = format!(
                    "Refusing file_edit for '{}': file changed since this session last read it. Re-read the file and retry, or set allow_stale_read=true if this overwrite is intentional.",
                    path_str
                );
                return file_edit_error_with_data(
                    message,
                    stale_conflict_json(
                        &identity,
                        &context.session_id,
                        &content,
                        current_mtime,
                        "pre_write_stale_check",
                    ),
                );
            }
            warn!(
                "File '{}' was modified since it was read; continuing because allow_stale_read=true",
                path_str
            );
        }

        // 2. 对 old_string 和 new_string 应用 desanitize 和 quote normalization（仅在 PRIORITY_AGENT_SMART_EDIT=1 时）
        let (old_string, new_string) = if std::env::var("PRIORITY_AGENT_SMART_EDIT")
            .as_ref()
            .map(|v| v.as_str())
            == Ok("1")
        {
            (
                normalize_text_line_endings(&desanitize(&normalize_quotes(old_string))),
                normalize_text_line_endings(&desanitize(&normalize_quotes(new_string))),
            )
        } else {
            (
                normalize_text_line_endings(old_string),
                normalize_text_line_endings(new_string),
            )
        };

        // 确定操作模式
        let using_exact_replace = line_start.is_none()
            && line_end.is_none()
            && insert_after.is_none()
            && insert_before.is_none();
        if using_exact_replace {
            if let Some(result) = exact_replace_preflight_error(
                &identity,
                &content,
                &old_string,
                &new_string,
                expected_replacements,
                normalize_ws,
            ) {
                return result;
            }
        }

        let result = if let (Some(start), Some(end)) = (line_start, line_end) {
            Self::do_replace_lines(content, start, end, &new_string)
        } else if let Some(after) = insert_after {
            Self::do_insert(
                content,
                after,
                &new_string,
                InsertMode::After,
                expected_replacements,
            )
        } else if let Some(before) = insert_before {
            Self::do_insert(
                content,
                before,
                &new_string,
                InsertMode::Before,
                expected_replacements,
            )
        } else {
            if old_string.trim().is_empty() {
                return ToolResult::error(
                    "old_string cannot be empty or whitespace-only unless insert_after, insert_before, or line_start/line_end is used. For a known target line, use line_start and line_end instead."
                        .to_string(),
                );
            }
            Self::do_replace(
                content,
                &old_string,
                &new_string,
                expected_replacements,
                normalize_ws,
            )
        };

        match result {
            Ok((new_content, replacements)) => {
                if let Some(result) = priority_agent_settings_validation_error(
                    &identity,
                    &new_content,
                    "schema_guard",
                ) {
                    return result;
                }
                if using_exact_replace
                    && replacements > 1
                    && is_code_like_path(&path)
                    && !allow_bulk_code_edit()
                {
                    return ToolResult::error(format!(
                        "Refusing multi-occurrence file_edit on code file '{}' ({} replacement(s)). Use a unique old_string, line_start/line_end, or set PRIORITY_AGENT_ALLOW_BULK_CODE_EDIT=1 for an intentional bulk code edit.",
                        path_str, replacements
                    ));
                }
                let checkpoint = match create_file_checkpoint(&context, "file_edit", &path).await {
                    Some(checkpoint) => checkpoint,
                    None => {
                        return checkpoint_creation_failed_result("file_edit", path_str, &identity)
                    }
                };
                let diagnostics_before =
                    collect_file_edit_diagnostics(&context, &path, &snapshot.content).await;
                let before_write_snapshot = match read_text_file(&path, "verify before edit").await
                {
                    Ok(snapshot) => snapshot,
                    Err(e) => return ToolResult::error(e),
                };
                let before_write_mtime = std::fs::metadata(&path)
                    .map(|m| m.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH))
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                if before_write_mtime != current_mtime
                    || before_write_snapshot.content.as_str() != snapshot.content.as_str()
                {
                    let message = format!(
                        "Refusing file_edit for '{}': file changed while this edit was being prepared. Re-read the file and retry.",
                        path_str
                    );
                    return file_edit_error_with_data(
                        message,
                        stale_conflict_json(
                            &identity,
                            &context.session_id,
                            &before_write_snapshot.content,
                            before_write_mtime,
                            "pre_write_race_check",
                        ),
                    );
                }
                let diff_summary =
                    edit_diff_summary(&identity.display_path, &snapshot.content, &new_content);
                match write_text_file(
                    &path,
                    &new_content,
                    snapshot.encoding,
                    snapshot.has_bom,
                    snapshot.line_ending,
                    MAX_EDITABLE_FILE_SIZE_BYTES,
                )
                .await
                {
                    Ok(bytes_written) => {
                        // 使文件缓存失效
                        if let Some(ref cache) = context.file_cache {
                            cache.invalidate_content(&path);
                            cache.invalidate_metadata(&path);
                        }
                        let new_mtime = std::fs::metadata(&path)
                            .map(|m| m.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH))
                            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                        mark_file_read_with_state(
                            &context.session_id,
                            &state_key,
                            &new_content,
                            new_mtime,
                        );
                        info!("Successfully edited file: {:?}", path);
                        drop(file_guard);
                        let file_change = record_file_change(
                            &context,
                            FileChangeRequest {
                                checkpoint: Some(&checkpoint),
                                tool_name: "file_edit",
                                path: &path,
                                existed_before: true,
                                before_content: Some(snapshot.content.as_str()),
                                after_content: &new_content,
                                diff: &diff_summary,
                                bytes_written: bytes_written as u64,
                            },
                        )
                        .await;
                        let file_change_id = file_change
                            .as_ref()
                            .and_then(|v| v.get("id").and_then(|id| id.as_str()));
                        let diagnostics =
                            collect_file_edit_diagnostics(&context, &path, &new_content).await;
                        let diagnostics_delta =
                            file_edit_diagnostics_delta(&diagnostics_before, &diagnostics);
                        let diagnostics_line = file_edit_diagnostics_content_line(&diagnostics);
                        let checkpoint_json = checkpoint_metadata_json(Some(&checkpoint));
                        let file_change_json =
                            file_change.clone().unwrap_or(serde_json::Value::Null);
                        let text_format = text_write_format_json(
                            snapshot.encoding,
                            snapshot.has_bom,
                            snapshot.line_ending,
                        );
                        let edit_preview = edit_preview_json(
                            &identity,
                            true,
                            Some(snapshot.content.as_str()),
                            &new_content,
                            &diff_summary,
                            text_format.clone(),
                            checkpoint_json.clone(),
                            file_change_json.clone(),
                            Some(replacements),
                            bytes_written as u64,
                            "edit_complete",
                        );
                        let data = json!({
                            "path": path_str,
                            "resolved_path": identity.resolved_path,
                            "path_identity": path_identity_json(&identity),
                            "replacements": replacements,
                            "bytes_written": bytes_written,
                            "text_format": text_format,
                            "checkpoint": checkpoint_json,
                            "file_change": file_change_json,
                            "diff": edit_diff_summary_json(&diff_summary),
                            "edit_preview": edit_preview,
                            "diagnostics_before": diagnostics_before,
                            "diagnostics": diagnostics.clone(),
                            "diagnostics_after": diagnostics,
                            "diagnostics_delta": diagnostics_delta,
                            "mutation_result": mutation_result::from_file_edit_json(
                                path_str,
                                &identity.resolved_path,
                                &identity.display_path,
                                replacements,
                                bytes_written as u64,
                                diff_summary.additions,
                                diff_summary.deletions,
                                diff_summary.changed_line_start as u64,
                                diff_summary.changed_line_end as u64,
                                &diff_summary.unified_diff,
                                diff_summary.preview_truncated,
                                text_format.get("encoding").and_then(|v| v.as_str()).unwrap_or("utf-8"),
                                text_format.get("bom").and_then(|v| v.as_bool()).unwrap_or(false),
                                text_format.get("line_ending").and_then(|v| v.as_str()).unwrap_or("LF"),
                                Some(checkpoint.id.as_str()),
                                checkpoint.sequence,
                                Some(context.session_id.as_str()),
                                file_change_id,
                                &Some(diagnostics),
                                Some(diagnostics_delta.clone()),
                            ),
                        });
                        let mut content = format!(
                            "File edited successfully: {} ({} replacement(s))",
                            path_str, replacements
                        );
                        if let Some(line) = diagnostics_line {
                            content.push('\n');
                            content.push_str(&line);
                        }
                        ToolResult::success_with_data(content, data)
                    }
                    Err(e) => ToolResult::error(e),
                }
            }
            Err(e) => ToolResult::error(e),
        }
    }

    fn requires_confirmation(&self, _params: &serde_json::Value) -> bool {
        true // 编辑文件总是需要确认
    }

    fn confirmation_prompt(&self, params: &serde_json::Value) -> Option<String> {
        let path = params["path"].as_str().unwrap_or("unknown file");
        Some(format!("This will edit the file: {}\nContinue?", path))
    }
}

pub enum InsertMode {
    After,
    Before,
}

impl FileEditTool {
    /// 预览编辑结果（不写入磁盘）
    pub fn preview_edit(params: &serde_json::Value, original: &str) -> Result<String, String> {
        let old_string = params["old_string"].as_str().unwrap_or("");
        let new_string = params["new_string"].as_str().unwrap_or("");
        let insert_after = params["insert_after"].as_str();
        let insert_before = params["insert_before"].as_str();
        let expected_replacements = params["expected_replacements"].as_u64().map(|n| n as usize);
        let line_start = params["line_start"].as_u64().map(|n| n as usize);
        let line_end = params["line_end"].as_u64().map(|n| n as usize);
        let normalize_ws = params["normalize_whitespace"].as_bool().unwrap_or(false);

        if let (Some(start), Some(end)) = (line_start, line_end) {
            Self::do_replace_lines(original.to_string(), start, end, new_string).map(|(s, _)| s)
        } else if let Some(after) = insert_after {
            Self::do_insert(
                original.to_string(),
                after,
                new_string,
                InsertMode::After,
                expected_replacements,
            )
            .map(|(s, _)| s)
        } else if let Some(before) = insert_before {
            Self::do_insert(
                original.to_string(),
                before,
                new_string,
                InsertMode::Before,
                expected_replacements,
            )
            .map(|(s, _)| s)
        } else {
            if old_string.trim().is_empty() {
                return Err(
                    "old_string cannot be empty or whitespace-only unless insert_after, insert_before, or line_start/line_end is used. For a known target line, use line_start and line_end instead."
                        .to_string(),
                );
            }
            Self::do_replace(
                original.to_string(),
                old_string,
                new_string,
                expected_replacements,
                normalize_ws,
            )
            .map(|(s, _)| s)
        }
    }

    pub fn do_replace(
        content: String,
        old_string: &str,
        new_string: &str,
        expected: Option<usize>,
        normalize_whitespace: bool,
    ) -> Result<(String, usize), String> {
        let mut occurrences = if normalize_whitespace {
            find_occurrences_normalized(&content, old_string)
        } else {
            find_occurrences(&content, old_string)
        };

        if old_string.trim().is_empty() {
            return Err(
                "old_string cannot be empty or whitespace-only unless insert_after, insert_before, or line_start/line_end is used. For a known target line, use line_start and line_end instead."
                    .to_string(),
            );
        }
        if old_string == new_string {
            return Err(
                "Refusing file_edit no-op: old_string and new_string are identical.".to_string(),
            );
        }

        if occurrences.is_empty() {
            if contains_file_read_line_prefix(old_string) {
                return Err(file_read_line_prefix_guidance("old_string"));
            }
            match generate_edit_candidates(&content, old_string, &occurrences) {
                EditCandidateOutcome::AutoApplied {
                    replacements,
                    strategy,
                    occurrence,
                } if replacements == expected.unwrap_or(1) => {
                    occurrences = vec![occurrence];
                    tracing::debug!(
                        "file_edit using deterministic recovery candidate strategy={}",
                        strategy
                    );
                }
                EditCandidateOutcome::Candidates { candidates, .. } => {
                    let details = candidates
                        .iter()
                        .map(|candidate| {
                            format!(
                                "- {} ({}) bytes {}..{}: {}",
                                candidate.strategy,
                                candidate.confidence,
                                candidate.occurrence.0,
                                candidate.occurrence.1,
                                candidate.guidance
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    return Err(format!(
                        "Could not find old_string exactly, but deterministic edit candidates were found:\n{}\n\nUse a more specific old_string or line_start/line_end if the candidate is intended.",
                        details
                    ));
                }
                EditCandidateOutcome::Mismatch { detail } => {
                    return Err(format!(
                        "Could not find old_string in the file. {}\n\n\
                         Common issues:\n\
                         - Line number prefixes from file_read output (e.g. '12 | ')\n\
                         - Smart quotes vs straight quotes\n\
                         - Escaped characters (literal \\n vs actual newline)\n\
                         - Trailing whitespace differences\n\n\
                         Tip: file_read the target file to verify exact content, or use line_start/line_end.",
                        detail
                    ));
                }
                EditCandidateOutcome::AutoApplied { .. } => {}
            }
        }

        if occurrences.is_empty() {
            // 尝试模糊匹配
            let fuzzy = fuzzy_find_occurrences(&content, old_string);
            if fuzzy.is_empty() {
                return Err(
                    "Could not find old_string in the file. It must match exactly, including whitespace, indentation, and line endings.\n\n\
                     Common issues: line number prefixes, smart quotes, escape sequences, trailing whitespace.\n\
                     Tip: file_read to verify, or use line_start/line_end for precise replacement."
                        .to_string(),
                );
            }
            // 如果模糊匹配有结果，但不符合预期，也返回详细信息
            let ctx = build_match_context(&content, &fuzzy, 2);
            return Err(format!(
                "old_string not found exactly, but fuzzy matches found:\n{}\n\nPlease adjust old_string to match one of these occurrences precisely.",
                ctx
            ));
        }

        let count = occurrences.len();
        let expected_count = expected.unwrap_or(1);
        let max_replacements = max_file_edit_replacements();

        if expected_count > max_replacements {
            return Err(format!(
                "Refusing file_edit with {} replacement(s): exceeds safety limit {}. Use narrower anchors or set PRIORITY_AGENT_MAX_FILE_EDIT_REPLACEMENTS explicitly for deliberate bulk edits.",
                expected_count, max_replacements
            ));
        }

        if count != expected_count {
            let ctx = build_match_context(&content, &occurrences, 2);
            return Err(format!(
                "Expected {} occurrence(s) of old_string, but found {}.\n{}\n\nPlease provide a more specific old_string or set expected_replacements to {}.",
                expected_count, count, ctx, count
            ));
        }

        // 从后往前替换，避免位置偏移问题
        let mut new_content = content;
        for (start, end) in occurrences.into_iter().rev() {
            new_content.replace_range(start..end, new_string);
        }
        Ok((new_content, count))
    }

    pub fn do_insert(
        content: String,
        anchor: &str,
        new_string: &str,
        mode: InsertMode,
        expected: Option<usize>,
    ) -> Result<(String, usize), String> {
        if contains_file_read_line_prefix(anchor) {
            let field = match mode {
                InsertMode::After => "insert_after",
                InsertMode::Before => "insert_before",
            };
            return Err(file_read_line_prefix_guidance(field));
        }
        let occurrences = find_occurrences(&content, anchor);
        if occurrences.is_empty() {
            return Err(format!(
                "Could not find anchor '{}' in file for insertion.",
                anchor
            ));
        }
        let count = occurrences.len();
        let expected_count = expected.unwrap_or(1);
        if count != expected_count {
            let field = match mode {
                InsertMode::After => "insert_after",
                InsertMode::Before => "insert_before",
            };
            let ctx = build_match_context(&content, &occurrences, 2);
            return Err(format!(
                "Expected {} occurrence(s) of {} anchor, but found {}.\n{}\n\nPlease provide a more specific anchor or set expected_replacements to {} if this bulk insert is intentional.",
                expected_count, field, count, ctx, count
            ));
        }
        let mut new_content = content;
        for (start, end) in occurrences.into_iter().rev() {
            match mode {
                InsertMode::After => {
                    new_content.insert_str(end, new_string);
                }
                InsertMode::Before => {
                    new_content.insert_str(start, new_string);
                }
            }
        }
        Ok((new_content, count))
    }

    /// 按行号范围替换内容（1-indexed，包含两端）
    pub fn do_replace_lines(
        content: String,
        line_start: usize,
        line_end: usize,
        new_string: &str,
    ) -> Result<(String, usize), String> {
        if line_start == 0 || line_end == 0 {
            return Err("line_start and line_end must be >= 1".to_string());
        }
        if line_start > line_end {
            return Err(format!(
                "line_start ({}) cannot be greater than line_end ({})",
                line_start, line_end
            ));
        }
        let lines: Vec<&str> = content.lines().collect();
        if line_start > lines.len() {
            return Err(format!(
                "line_start ({}) is beyond end of file ({} lines total)",
                line_start,
                lines.len()
            ));
        }
        let start_idx = line_start - 1;
        let end_idx = (line_end - 1).min(lines.len() - 1);
        let mut new_lines = lines[..start_idx].to_vec();
        new_lines.push(new_string);
        new_lines.extend_from_slice(&lines[end_idx + 1..]);
        let mut new_content = new_lines.join("\n");
        // 如果原始内容以换行符结尾，保留末尾换行符
        if content.ends_with('\n') {
            new_content.push('\n');
        }
        let replaced_count = end_idx - start_idx + 1;
        Ok((new_content, replaced_count))
    }
}
