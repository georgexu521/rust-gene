//! 代码格式化工具
//!
//! 调用项目配置的格式化器来格式化代码。
//! 支持：Rust (rustfmt)、JS/TS (prettier)、Python (black)、Go (gofmt)

use crate::tools::{Tool, ToolContext, ToolResult};
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
            "required": ["action", "file_path"]
        })
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
            "format" => self.format_file(&path, &detected_formatter).await,
            "check" => self.check_format(&path, &detected_formatter).await,
            _ => ToolResult::error(format!("Unknown format action: {}", action)),
        }
    }
}

impl FormatTool {
    /// 格式化文件
    async fn format_file(&self, path: &Path, formatter: &str) -> ToolResult {
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
                    ToolResult::success_with_data(
                        format!("Formatted {} using {}", path.display(), formatter),
                        json!({
                            "file": path.to_string_lossy(),
                            "formatter": formatter,
                            "success": true
                        }),
                    )
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    ToolResult::error(format!("Formatter {} failed: {}", formatter, stderr.trim()))
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
}
