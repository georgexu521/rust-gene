use super::*;

/// 文件写入工具
pub struct FileWriteTool;

#[async_trait]
impl Tool for FileWriteTool {
    fn name(&self) -> &str {
        "file_write"
    }

    fn description(&self) -> &str {
        "Create a new file, or overwrite an existing file only after reading it \
         in this session. Parent directories are created as needed. For targeted \
         changes to existing files, use file_edit instead — this tool replaces \
         the entire file. Existing-file writes are rejected until file_read has \
         provided current file content."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path to the file to write"
                },
                "content": {
                    "type": "string",
                    "description": "The content to write to the file"
                }
            },
            "required": ["path", "content"]
        })
    }

    fn to_classifier_input(&self, params: &serde_json::Value) -> String {
        let path = params["path"].as_str().unwrap_or("");
        let content_len = params["content"].as_str().map(|s| s.len()).unwrap_or(0);
        format!("file_write: {} ({} bytes)", path, content_len)
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["write"]
    }

    fn search_hint(&self) -> Option<&'static str> {
        Some("create overwrite files")
    }

    fn strict_schema(&self) -> bool {
        true
    }

    fn operation_kind(&self, _params: &serde_json::Value) -> ToolOperationKind {
        ToolOperationKind::Write
    }

    fn is_read_only(&self, _params: &serde_json::Value) -> bool {
        false
    }

    fn is_concurrency_safe(&self, _params: &serde_json::Value) -> bool {
        false
    }

    fn is_destructive(&self, params: &serde_json::Value) -> bool {
        let path = params["path"].as_str().unwrap_or("");
        let input_path = Path::new(path);
        input_path.is_absolute() && input_path.exists()
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
        // 检查只读模式
        if context.permissions.read_only {
            return ToolResult::error("Cannot write files in read-only mode");
        }

        let path_str = params["path"].as_str().unwrap_or("");
        let content = params["content"].as_str().unwrap_or("");

        if path_str.is_empty() {
            return ToolResult::error("Path cannot be empty");
        }
        if is_unc_or_network_path(path_str) {
            return ToolResult::error(format!(
                "Refusing to write UNC/network path '{}'. Use a local path instead.",
                path_str
            ));
        }
        if content.len() as u64 > MAX_EDITABLE_FILE_SIZE_BYTES {
            return ToolResult::error(format!(
                "Refusing to write content larger than {} bytes",
                MAX_EDITABLE_FILE_SIZE_BYTES
            ));
        }

        let path = match resolve_path(path_str, &context.working_dir) {
            Ok(path) => path,
            Err(msg) => return ToolResult::error(msg),
        };
        info!("Writing file: {:?}", path);
        let identity = file_path_identity(path_str, &path, &context.working_dir);
        if let Some(result) =
            high_risk_file_target_result(&path, &identity, &context.working_dir, "file_write")
        {
            return result;
        }
        let file_guard = acquire_file_mutation_lock(&identity.state_key).await;

        // ReadTracker: block overwriting unreferenced files.
        if let Some(ref tracker) = context.read_tracker {
            if !allow_edit_without_read() {
                if let Err(msg) = tracker.check_edit(&path, content) {
                    return ToolResult::error(msg);
                }
            }
        }

        let existed_before = path.exists();
        let existing_snapshot = if existed_before {
            if let Err(msg) = check_file_size_limit(&path, "write") {
                return ToolResult::error(msg);
            }
            match read_text_file(&path, "write").await {
                Ok(snapshot) => Some(snapshot),
                Err(e) => return ToolResult::error(e),
            }
        } else {
            None
        };
        if existed_before && !allow_edit_without_read() {
            if let Some(error) = check_read_before_write(&context.session_id, &identity.state_key) {
                return error;
            }
        }
        let (content_has_bom, content_body) = split_leading_text_bom(content);
        let encoding = existing_snapshot
            .as_ref()
            .map(|snapshot| snapshot.encoding)
            .unwrap_or(TextFileEncoding::Utf8);
        let has_bom = existing_snapshot
            .as_ref()
            .map(|snapshot| snapshot.has_bom)
            .unwrap_or(false)
            || content_has_bom;
        let line_ending = existing_snapshot
            .as_ref()
            .map(|snapshot| snapshot.line_ending)
            .unwrap_or_else(|| detect_line_ending(content_body));

        // 检查父目录是否存在，不存在则创建
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                debug!("Creating parent directories: {:?}", parent);
                if let Err(e) = tokio::fs::create_dir_all(parent).await {
                    error!("Failed to create parent directories: {}", e);
                    return ToolResult::error(format!("Failed to create directories: {}", e));
                }
            }
        }

        let before_content = existing_snapshot
            .as_ref()
            .map(|snapshot| snapshot.content.as_str());
        let diff_summary = edit_diff_summary(
            &identity.display_path,
            before_content.unwrap_or(""),
            content_body,
        );
        if let Some(result) =
            priority_agent_settings_validation_error(&identity, content_body, "schema_guard")
        {
            return result;
        }
        let checkpoint = match create_file_checkpoint(&context, "file_write", &path).await {
            Some(checkpoint) => checkpoint,
            None => {
                return checkpoint_creation_failed_result("file_write", path_str, &identity);
            }
        };

        // 写入文件
        match write_text_file(
            &path,
            content_body,
            encoding,
            has_bom,
            line_ending,
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
                info!("Successfully wrote {} bytes to {:?}", content.len(), path);
                let action = if existed_before {
                    "overwritten"
                } else {
                    "written"
                };
                drop(file_guard);
                let file_change = record_file_change(
                    &context,
                    FileChangeRequest {
                        checkpoint: Some(&checkpoint),
                        tool_name: "file_write",
                        path: &path,
                        existed_before,
                        before_content,
                        after_content: content_body,
                        diff: &diff_summary,
                        bytes_written: bytes_written as u64,
                    },
                )
                .await;
                let checkpoint_json = checkpoint_metadata_json(Some(&checkpoint));
                let file_change_json = file_change.unwrap_or(serde_json::Value::Null);
                let text_format = text_write_format_json(encoding, has_bom, line_ending);
                let edit_preview = edit_preview_json(
                    &identity,
                    existed_before,
                    before_content,
                    content_body,
                    &diff_summary,
                    text_format.clone(),
                    checkpoint_json.clone(),
                    file_change_json.clone(),
                    None,
                    bytes_written as u64,
                    "write_complete",
                );
                ToolResult::success_with_data(
                    format!("File {} successfully: {}", action, path_str),
                    json!({
                        "path": path_str,
                        "resolved_path": identity.resolved_path,
                        "path_identity": path_identity_json(&identity),
                        "bytes_written": bytes_written,
                        "existed_before": existed_before,
                        "checkpoint": checkpoint_json,
                        "file_change": file_change_json,
                        "diff": edit_diff_summary_json(&diff_summary),
                        "text_format": text_format,
                        "edit_preview": edit_preview,
                        "guidance": if existed_before {
                            "file_write replaced the entire file; use file_edit for targeted existing-file changes"
                        } else {
                            "file_write created a new file"
                        }
                    }),
                )
            }
            Err(e) => ToolResult::error(e),
        }
    }

    fn requires_confirmation(&self, params: &serde_json::Value) -> bool {
        let path = params["path"].as_str().unwrap_or("");
        if path.is_empty() {
            return false;
        }
        let input_path = Path::new(path);
        // 对相对路径一律要求确认，避免 cwd 与 tool working_dir 不一致时误判。
        if !input_path.is_absolute() {
            return true;
        }
        // 绝对路径仅在目标已存在时要求确认。
        input_path.exists()
    }

    fn confirmation_prompt(&self, params: &serde_json::Value) -> Option<String> {
        params["path"]
            .as_str()
            .map(|p| format!("This will overwrite the existing file: {}\nContinue?", p))
    }
}
