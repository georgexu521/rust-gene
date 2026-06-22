//! File tool support module.
//!
//! Separates read, write, edit matching, path policy, and mutation history from the file tool entrypoint.

use super::*;

/// 文件读取工具
pub struct FileReadTool;

#[async_trait]
impl Tool for FileReadTool {
    fn name(&self) -> &str {
        "file_read"
    }

    fn description(&self) -> &str {
        "Read a workspace file or directory. Files return full content by default; \
         use limit and offset for line ranges. Directories list entries with '/' on \
         subdirs. If unchanged since last read, reuse the previous content. Use glob \
         for filenames and grep for content search."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path to the file or directory to read (relative or absolute; supports ~/Desktop for user-approved desktop inspection)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of lines to read (optional)",
                    "minimum": 1
                },
                "offset": {
                    "type": "integer",
                    "description": "Line number to start reading from (1-indexed, optional)",
                    "minimum": 1
                }
            },
            "required": ["path"]
        })
    }

    fn to_classifier_input(&self, params: &serde_json::Value) -> String {
        let path = params["path"].as_str().unwrap_or("");
        format!("file_read: {}", path)
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["read"]
    }

    fn search_hint(&self) -> Option<&'static str> {
        Some("view file contents directory entries")
    }

    fn strict_schema(&self) -> bool {
        true
    }

    fn operation_kind(&self, _params: &serde_json::Value) -> ToolOperationKind {
        ToolOperationKind::Read
    }

    fn is_read_only(&self, _params: &serde_json::Value) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _params: &serde_json::Value) -> bool {
        true
    }

    fn is_search_or_read_command(&self, _params: &serde_json::Value) -> ToolSearchOrReadSemantics {
        ToolSearchOrReadSemantics {
            is_read: true,
            ..Default::default()
        }
    }

    fn tool_use_summary(&self, params: &serde_json::Value) -> Option<String> {
        let path = params["path"].as_str()?.trim();
        if path.is_empty() {
            None
        } else {
            Some(path.to_string())
        }
    }

    async fn execute(&self, params: serde_json::Value, context: ToolContext) -> ToolResult {
        let path_str = params["path"].as_str().unwrap_or("");
        if path_str.is_empty() {
            return ToolResult::error("Path cannot be empty");
        }
        if is_unc_or_network_path(path_str) {
            return ToolResult::error(format!(
                "Refusing to read UNC/network path '{}'. Use a local path instead.",
                path_str
            ));
        }

        let limit = params["limit"].as_u64().map(|u| u as usize);
        let offset = params["offset"].as_u64().and_then(|o| {
            if o == 0 {
                None
            } else {
                Some((o as usize).saturating_sub(1))
            }
        });

        let path = match resolve_read_path(path_str, &context.working_dir) {
            Ok(path) => path,
            Err(msg) => return ToolResult::error(msg),
        };
        info!("Reading file: {:?}", path);
        let identity = file_path_identity(path_str, &path, &context.working_dir);

        // 检查文件是否存在
        if !path.exists() {
            return ToolResult::error(format!("File does not exist: {}", path_str));
        }

        if path.is_dir() {
            if let Some(ref tracker) = context.read_tracker {
                tracker.mark_read(&path);
            }
            return read_directory_result(&path, path_str, &identity, limit, offset).await;
        }

        // 检查是否是文件
        if !path.is_file() {
            return ToolResult::error(format!("Path is not a file: {}", path_str));
        }
        if let Err(msg) = check_file_size_limit(&path, "read") {
            return ToolResult::error(msg);
        }

        // 读取文件内容
        let snapshot = match read_text_file(&path, "read").await {
            Ok(snapshot) => snapshot,
            Err(e) => {
                error!("{}", e);
                return ToolResult::error(e);
            }
        };
        // Read-before-edit guard: record this successful read.
        if let Some(ref tracker) = context.read_tracker {
            tracker.mark_read(&path);
        }
        let content = snapshot.content.as_str();

        let targeted_read = limit.is_some() || offset.is_some();

        // 文件缓存优化：如果文件在本会话中已读过且未变更，返回短信提示。
        // 但 offset/limit 读取是新的局部证据，不能被上一次全文读取短路掉。
        // Skip cache short-circuit in eval/non-interactive mode so the model
        // always sees full file content when reading the same file multiple times.
        let eval_no_cache = std::env::var("PRIORITY_AGENT_EVAL_NO_FILE_CACHE")
            .unwrap_or_default()
            .trim()
            == "1";

        if let Some(ref cache) = context.file_cache {
            if !eval_no_cache
                && !targeted_read
                && cache.is_unchanged_since_last_read_for_session(&context.session_id, &path)
            {
                let lines_count = content.lines().count();
                let content_preview = file_read_content_preview(content.lines());
                let preview_note = content_preview
                    .as_deref()
                    .map(|preview| format!("\nContext preview: {preview}"))
                    .unwrap_or_default();
                return ToolResult::success_with_data(
                    format!(
                        "[File unchanged since last read: {}] ({} lines)\nIf you need the full content, it was provided in a previous message.{}",
                        path_str, lines_count, preview_note
                    ),
                    json!({
                        "path": path_str,
                        "resolved_path": identity.resolved_path,
                        "path_identity": path_identity_json(&identity),
                        "kind": "file",
                        "unchanged": true,
                        "total_lines": lines_count,
                        "displayed_lines": 0,
                        "line_start": serde_json::Value::Null,
                        "line_end": serde_json::Value::Null,
                        "truncated": false,
                        "targeted_read": false,
                        "read_coverage": "full",
                        "size_bytes": snapshot.byte_len,
                        "content_hash": content_hash_hex(content),
                        "content_preview": content_preview,
                        "text_format": text_format_json(&snapshot),
                        "display_format": "unchanged_notice",
                        "content_format": {
                            "visible_content": "unchanged_notice",
                            "raw_content_in_tool_result": false,
                            "display_prefix": serde_json::Value::Null,
                            "truncation_hint_in_content": false
                        }
                    }),
                );
            }
            cache.mark_read_for_session(&context.session_id, &path);
        }

        let mtime = std::fs::metadata(&path)
            .map(|m| m.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH))
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        let state_key = identity.state_key.clone();

        // 应用 limit 和 offset
        let lines: Vec<&str> = content.lines().collect();
        let start = offset.unwrap_or(0);
        if offset.is_some() && start >= lines.len() {
            return ToolResult::error(format!(
                "Offset {} is beyond end of file ({} lines total)",
                start + 1,
                lines.len()
            ));
        }
        let end = limit
            .map(|l| (start + l).min(lines.len()))
            .unwrap_or(lines.len());

        let selected_lines = if start > 0 || end < lines.len() {
            &lines[start..end]
        } else {
            &lines[..]
        };

        let result = selected_lines.join("\n");
        let truncated = end < lines.len() || start > 0;
        let line_start_display = if selected_lines.is_empty() {
            0
        } else {
            start + 1
        };
        let line_end_display = if selected_lines.is_empty() { 0 } else { end };
        let read_coverage = if targeted_read {
            (line_start_display > 0 && line_end_display >= line_start_display)
                .then_some((line_start_display, line_end_display))
        } else {
            None
        };
        mark_file_read_with_state_and_coverage(
            &context.session_id,
            &state_key,
            content,
            mtime,
            read_coverage,
        );

        // 添加行号信息
        let formatted = if lines.len() > 1 {
            selected_lines
                .iter()
                .enumerate()
                .map(|(i, line)| format!("{:4} | {}", start + i + 1, line))
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            result.clone()
        };

        let truncated_info = if truncated {
            format!(
                "\n\n[{} lines total, showing lines {}-{}]",
                lines.len(),
                start + 1,
                end
            )
        } else {
            String::new()
        };
        let content_hash = content_hash_hex(content);
        let selected_content_hash = content_hash_hex(&result);
        let content_preview = file_read_content_preview(selected_lines.iter().copied());
        if let Some(store) = context.session_store.as_ref() {
            record_file_read(
                store,
                &FileReadLedgerInput {
                    session_id: &context.session_id,
                    path: path_str,
                    resolved_path: &identity.resolved_path,
                    content_hash: &content_hash,
                    content_preview: content_preview.as_deref(),
                    size_bytes: snapshot.byte_len as u64,
                    total_lines: lines.len(),
                    displayed_lines: selected_lines.len(),
                    line_start: (line_start_display > 0).then_some(line_start_display),
                    line_end: (line_end_display > 0).then_some(line_end_display),
                    targeted_read,
                    truncated,
                    mtime: Some(mtime),
                },
            );
        }

        ToolResult::success_with_data(
            format!("{}{}", formatted, truncated_info),
            json!({
                "path": path_str,
                "resolved_path": identity.resolved_path,
                "path_identity": path_identity_json(&identity),
                "kind": "file",
                "total_lines": lines.len(),
                "displayed_lines": selected_lines.len(),
                "line_start": json_line_number(line_start_display),
                "line_end": json_line_number(line_end_display),
                "truncated": truncated,
                "targeted_read": targeted_read,
                "read_coverage": if targeted_read { "partial" } else { "full" },
                "size_bytes": snapshot.byte_len,
                "content_hash": content_hash,
                "selected_content_hash": selected_content_hash,
                "content_preview": content_preview,
                "text_format": text_format_json(&snapshot),
                "display_format": if lines.len() > 1 { "line_numbered_content" } else { "raw_content" },
                "content_format": {
                    "visible_content": if lines.len() > 1 { "line_numbered_display" } else { "raw_content" },
                    "raw_content_in_tool_result": lines.len() <= 1,
                    "display_prefix": if lines.len() > 1 { serde_json::Value::String("{line} | ".to_string()) } else { serde_json::Value::Null },
                    "truncation_hint_in_content": truncated
                }
            }),
        )
    }
}
