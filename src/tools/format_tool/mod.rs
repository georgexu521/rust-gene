//! 代码格式化工具
//!
//! 调用项目配置的格式化器来格式化代码。
//! 支持：Rust (rustfmt)、JS/TS (prettier)、Python (black)、Go (gofmt)

use crate::tools::file_tool::history::{
    checkpoint_metadata_json, create_file_checkpoint, record_file_change, FileChangeRequest,
};
use crate::tools::file_tool::{edit_diff_summary, edit_diff_summary_json};
use crate::tools::{Tool, ToolContext, ToolOperationKind, ToolPermissionLevel, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;

/// 代码格式化工具
pub struct FormatTool;

#[async_trait]
impl Tool for FormatTool {
    fn name(&self) -> &str {
        "format"
    }

    fn description(&self) -> &str {
        "Format code using language-specific formatters. \
         Supports: Rust (rustfmt), JS/TS (prettier), Python (black), Go (gofmt). \
         Actions: 'format' (format file), 'check' (check if formatted)."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["format", "check"],
                    "description": "Format action: 'format' to format file, 'check' to verify formatting"
                },
                "file_path": {
                    "type": "string",
                    "description": "Path to the file to format (required)"
                },
                "formatter": {
                    "type": "string",
                    "enum": ["auto", "rustfmt", "prettier", "black", "gofmt"],
                    "description": "Formatter to use (default: auto-detect from file extension)"
                }
            },
            "required": ["action", "file_path"],
            "additionalProperties": false
        })
    }

    fn requires_confirmation(&self, params: &serde_json::Value) -> bool {
        params["action"].as_str() == Some("format")
    }

    fn confirmation_prompt(&self, params: &serde_json::Value) -> Option<String> {
        if !self.requires_confirmation(params) {
            return None;
        }
        let file_path = params["file_path"].as_str().unwrap_or("the target file");
        Some(format!("Format and rewrite {file_path}?"))
    }

    fn operation_kind(&self, params: &serde_json::Value) -> ToolOperationKind {
        match params["action"].as_str() {
            Some("check") | Some("review") => ToolOperationKind::Read,
            Some("format") => ToolOperationKind::Edit,
            _ => ToolOperationKind::Edit,
        }
    }

    fn permission_level(&self) -> ToolPermissionLevel {
        ToolPermissionLevel::HighRisk
    }

    fn is_concurrency_safe(&self, params: &serde_json::Value) -> bool {
        params["action"].as_str() == Some("check")
    }

    fn strict_schema(&self) -> bool {
        true
    }

    fn tool_use_summary(&self, params: &serde_json::Value) -> Option<String> {
        let action = params["action"].as_str()?;
        let file_path = params["file_path"].as_str()?;
        Some(format!("{action} {file_path}"))
    }

    async fn execute(&self, params: serde_json::Value, context: ToolContext) -> ToolResult {
        let action = params["action"].as_str().unwrap_or("");
        let file_path = params["file_path"].as_str().unwrap_or("");

        if file_path.is_empty() {
            return ToolResult::error("file_path is required".to_string());
        }

        let path = match crate::tools::file_tool::resolve_path(file_path, &context.working_dir) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(e.to_string()),
        };

        if !path.exists() {
            return ToolResult::error(format!("File not found: {}", path.display()));
        }

        let formatter = params["formatter"].as_str().unwrap_or("auto");
        let detected_formatter = if formatter == "auto" {
            match detect_formatter(&path) {
                Some(f) => f,
                None => {
                    return ToolResult::error(format!(
                        "No formatter detected for file: {}. Supported: .rs, .ts/.tsx/.js/.jsx, .py, .go",
                        path.display()
                    ))
                }
            }
        } else {
            formatter.to_string()
        };

        match action {
            "format" => self.format_file(&path, &detected_formatter, &context).await,
            "check" => self.check_format(&path, &detected_formatter).await,
            _ => ToolResult::error(format!("Unknown format action: {}", action)),
        }
    }
}

impl FormatTool {
    /// 格式化文件
    async fn format_file(&self, path: &Path, formatter: &str, context: &ToolContext) -> ToolResult {
        let before_content = match tokio::fs::read_to_string(path).await {
            Ok(content) => content,
            Err(e) => {
                return ToolResult::error(format!(
                    "Failed to read {} before formatting: {}",
                    path.display(),
                    e
                ));
            }
        };
        let checkpoint = match create_file_checkpoint(context, "format", path).await {
            Some(checkpoint) => checkpoint,
            None => {
                return ToolResult::error(format!(
                    "Failed to create checkpoint before formatting {}",
                    path.display()
                ));
            }
        };
        let (command, args) = build_format_command(formatter, path);

        match Command::new(&command)
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
        {
            Ok(output) => {
                if output.status.success() {
                    let after_content = match tokio::fs::read_to_string(path).await {
                        Ok(content) => content,
                        Err(e) => {
                            let checkpoint_json = checkpoint_metadata_json(Some(&checkpoint));
                            let data = json!({
                                "file": path.to_string_lossy(),
                                "formatter": formatter,
                                "success": false,
                                "checkpoint": checkpoint_json,
                                "read_after_format_error": e.to_string(),
                            });
                            let mut result = ToolResult::error(format!(
                                "Formatted {} using {}, but failed to read the result for checkpoint history: {}",
                                path.display(),
                                formatter,
                                e
                            ));
                            result.data = Some(data);
                            return result;
                        }
                    };
                    if let Some(ref cache) = context.file_cache {
                        cache.invalidate_content(path);
                        cache.invalidate_metadata(path);
                    }
                    let diff_summary =
                        edit_diff_summary(&path.to_string_lossy(), &before_content, &after_content);
                    let file_change = record_file_change(
                        context,
                        FileChangeRequest {
                            checkpoint: Some(&checkpoint),
                            tool_name: "format",
                            path,
                            existed_before: true,
                            before_content: Some(&before_content),
                            after_content: &after_content,
                            diff: &diff_summary,
                            bytes_written: after_content.len() as u64,
                        },
                    )
                    .await;
                    ToolResult::success_with_data(
                        format!("Formatted {} using {}", path.display(), formatter),
                        json!({
                            "file": path.to_string_lossy(),
                            "formatter": formatter,
                            "success": true,
                            "changed": before_content != after_content,
                            "bytes_written": after_content.len(),
                            "checkpoint": checkpoint_metadata_json(Some(&checkpoint)),
                            "file_change": file_change.unwrap_or(serde_json::Value::Null),
                            "diff": edit_diff_summary_json(&diff_summary)
                        }),
                    )
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    self.format_error_with_checkpoint(
                        path,
                        formatter,
                        &before_content,
                        &checkpoint,
                        context,
                        format!("Formatter {} failed: {}", formatter, stderr.trim()),
                    )
                    .await
                }
            }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    self.format_error_with_checkpoint(
                        path,
                        formatter,
                        &before_content,
                        &checkpoint,
                        context,
                        format!(
                            "Formatter '{}' not found. Please install it first.",
                            formatter
                        ),
                    )
                    .await
                } else {
                    self.format_error_with_checkpoint(
                        path,
                        formatter,
                        &before_content,
                        &checkpoint,
                        context,
                        format!("Failed to run {}: {}", formatter, e),
                    )
                    .await
                }
            }
        }
    }

    async fn format_error_with_checkpoint(
        &self,
        path: &Path,
        formatter: &str,
        before_content: &str,
        checkpoint: &crate::engine::checkpoint::Checkpoint,
        context: &ToolContext,
        message: String,
    ) -> ToolResult {
        let after_content = tokio::fs::read_to_string(path).await.ok();
        let (changed, file_change, diff_json) = if let Some(after_content) = after_content.as_ref()
        {
            let diff_summary =
                edit_diff_summary(&path.to_string_lossy(), before_content, after_content);
            let file_change = if before_content != after_content {
                record_file_change(
                    context,
                    FileChangeRequest {
                        checkpoint: Some(checkpoint),
                        tool_name: "format",
                        path,
                        existed_before: true,
                        before_content: Some(before_content),
                        after_content,
                        diff: &diff_summary,
                        bytes_written: after_content.len() as u64,
                    },
                )
                .await
            } else {
                None
            };
            (
                before_content != after_content,
                file_change.unwrap_or(serde_json::Value::Null),
                edit_diff_summary_json(&diff_summary),
            )
        } else {
            (false, serde_json::Value::Null, serde_json::Value::Null)
        };

        if changed {
            if let Some(ref cache) = context.file_cache {
                cache.invalidate_content(path);
                cache.invalidate_metadata(path);
            }
        }

        let data = json!({
            "file": path.to_string_lossy(),
            "formatter": formatter,
            "success": false,
            "changed_after_failure": changed,
            "checkpoint": checkpoint_metadata_json(Some(checkpoint)),
            "file_change": file_change,
            "diff": diff_json,
        });
        let mut result = ToolResult::error(message);
        result.data = Some(data);
        result
    }

    /// 检查格式
    async fn check_format(&self, path: &Path, formatter: &str) -> ToolResult {
        let (command, args) = build_check_command(formatter, path);

        match Command::new(&command)
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
        {
            Ok(output) => {
                let is_formatted = output.status.success();
                if is_formatted {
                    ToolResult::success_with_data(
                        format!("{} is properly formatted", path.display()),
                        json!({
                            "file": path.to_string_lossy(),
                            "formatter": formatter,
                            "is_formatted": true
                        }),
                    )
                } else {
                    ToolResult::success_with_data(
                        format!("{} needs formatting", path.display()),
                        json!({
                            "file": path.to_string_lossy(),
                            "formatter": formatter,
                            "is_formatted": false
                        }),
                    )
                }
            }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    ToolResult::error(format!(
                        "Formatter '{}' not found. Please install it first.",
                        formatter
                    ))
                } else {
                    ToolResult::error(format!("Failed to run {}: {}", formatter, e))
                }
            }
        }
    }
}

/// 根据文件扩展名检测格式化器
fn detect_formatter(path: &Path) -> Option<String> {
    let ext = path.extension().and_then(|e| e.to_str())?;
    match ext {
        "rs" => Some("rustfmt".to_string()),
        "ts" | "tsx" | "js" | "jsx" | "json" | "css" | "md" => Some("prettier".to_string()),
        "py" => Some("black".to_string()),
        "go" => Some("gofmt".to_string()),
        _ => None,
    }
}

/// 构建格式化命令
fn build_format_command(formatter: &str, path: &Path) -> (String, Vec<String>) {
    let path_str = path.to_string_lossy().to_string();
    match formatter {
        "rustfmt" => ("rustfmt".to_string(), vec![path_str]),
        "prettier" => (
            "prettier".to_string(),
            vec!["--write".to_string(), path_str],
        ),
        "black" => ("black".to_string(), vec![path_str]),
        "gofmt" => {
            // gofmt -w 格式化并写入
            ("gofmt".to_string(), vec!["-w".to_string(), path_str])
        }
        _ => (formatter.to_string(), vec![path_str]),
    }
}

/// 构建检查命令
fn build_check_command(formatter: &str, path: &Path) -> (String, Vec<String>) {
    let path_str = path.to_string_lossy().to_string();
    match formatter {
        "rustfmt" => ("rustfmt".to_string(), vec!["--check".to_string(), path_str]),
        "prettier" => (
            "prettier".to_string(),
            vec!["--check".to_string(), path_str],
        ),
        "black" => ("black".to_string(), vec!["--check".to_string(), path_str]),
        "gofmt" => {
            // gofmt -l 列出需要格式化的文件
            ("gofmt".to_string(), vec!["-l".to_string(), path_str])
        }
        _ => (formatter.to_string(), vec!["--check".to_string(), path_str]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_detect_formatter() {
        assert_eq!(
            detect_formatter(Path::new("main.rs")),
            Some("rustfmt".to_string())
        );
        assert_eq!(
            detect_formatter(Path::new("index.ts")),
            Some("prettier".to_string())
        );
        assert_eq!(
            detect_formatter(Path::new("app.py")),
            Some("black".to_string())
        );
        assert_eq!(
            detect_formatter(Path::new("main.go")),
            Some("gofmt".to_string())
        );
        assert_eq!(detect_formatter(Path::new("unknown.xyz")), None);
    }

    #[test]
    fn test_format_tool_name() {
        let tool = FormatTool;
        assert_eq!(tool.name(), "format");
    }

    #[test]
    fn format_tool_contract_is_parameter_sensitive() {
        let tool = FormatTool;
        let check = json!({"action": "check", "file_path": "src/main.rs"});
        let format = json!({"action": "format", "file_path": "src/main.rs"});

        assert_eq!(tool.operation_kind(&check), ToolOperationKind::Read);
        assert!(!tool.requires_confirmation(&check));
        assert!(tool.is_concurrency_safe(&check));

        assert_eq!(tool.operation_kind(&format), ToolOperationKind::Edit);
        assert!(tool.requires_confirmation(&format));
        assert!(tool.confirmation_prompt(&format).is_some());
        assert!(!tool.is_concurrency_safe(&format));
        assert_eq!(tool.permission_level(), ToolPermissionLevel::HighRisk);
        assert!(tool.strict_schema());
    }

    #[tokio::test]
    async fn format_action_creates_checkpoint_and_file_change() {
        if Command::new("rustfmt")
            .arg("--version")
            .output()
            .await
            .is_err()
        {
            return;
        }

        let temp = tempfile::TempDir::new().unwrap();
        let path = temp.path().join("main.rs");
        let original = "fn main(){println!(\"hi\");}\n";
        tokio::fs::write(&path, original).await.unwrap();

        let session_id = format!("test-format-checkpoint-{}", uuid::Uuid::new_v4().simple());
        let manager = crate::engine::checkpoint::get_checkpoint_manager(&session_id).await;
        manager.lock().await.clear_all().await.unwrap();
        let context =
            ToolContext::new(temp.path(), &session_id).with_checkpoint_manager(manager.clone());
        let result = FormatTool
            .execute(
                json!({"action": "format", "file_path": "main.rs", "formatter": "rustfmt"}),
                context,
            )
            .await;

        assert!(result.success, "format failed: {:?}", result.error);
        let data = result.data.as_ref().expect("format metadata");
        assert!(data["checkpoint"]["id"].as_str().is_some());
        assert_eq!(data["file_change"]["tool_name"], "format");
        assert_eq!(data["changed"], true);

        let checkpoint_id = data["checkpoint"]["id"].as_str().unwrap().to_string();
        {
            let checkpoint_manager = manager.lock().await;
            assert!(checkpoint_manager
                .list_file_changes()
                .iter()
                .any(|change| change.tool_name == "format"));
            checkpoint_manager
                .restore_checkpoint(&checkpoint_id)
                .await
                .unwrap();
        }
        let restored = tokio::fs::read_to_string(&path).await.unwrap();
        assert_eq!(restored, original);
        manager.lock().await.clear_all().await.unwrap();
    }
}
