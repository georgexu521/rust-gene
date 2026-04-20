//! Glob 工具 - 文件搜索
//!
//! 使用 glob 模式搜索文件

use crate::tools::file_tool::{is_allowed_absolute_path, resolve_path};
use crate::tools::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use tracing::{debug, info};

/// Glob 文件搜索工具
pub struct GlobTool;

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        "glob"
    }

    fn description(&self) -> &str {
        "Search for files using glob patterns. \
         Returns a list of file paths matching the pattern. \
         Examples: '*.rs' for all Rust files, '**/*.md' for all markdown files recursively."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "The glob pattern to search for (e.g., '*.rs', 'src/**/*.ts')"
                },
                "path": {
                    "type": "string",
                    "description": "The directory to search in (default: current working directory)"
                }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(&self, params: serde_json::Value, context: ToolContext) -> ToolResult {
        let pattern = params["pattern"].as_str().unwrap_or("");
        if pattern.is_empty() {
            return ToolResult::error("Pattern cannot be empty");
        }

        let search_path = match params["path"].as_str() {
            Some(path_str) => match resolve_path(path_str, &context.working_dir) {
                Ok(path) => path,
                Err(msg) => return ToolResult::error(msg),
            },
            None => context.working_dir.clone(),
        };

        info!("Glob search: '{}' in {:?}", pattern, search_path);

        // 构建完整路径模式，并校验边界
        let full_pattern = if std::path::Path::new(pattern).is_absolute() {
            let abs = crate::tools::file_tool::normalize_path(std::path::Path::new(pattern));
            if !is_allowed_absolute_path(&abs, &context.working_dir) {
                return ToolResult::error(format!(
                    "Access denied: absolute pattern '{}' is outside allowed roots",
                    pattern
                ));
            }
            abs.to_string_lossy().to_string()
        } else {
            search_path.join(pattern).to_string_lossy().to_string()
        };

        // 使用 glob crate 进行搜索
        let mut matches = Vec::new();
        match glob::glob(&full_pattern) {
            Ok(paths) => {
                for entry in paths {
                    match entry {
                        Ok(path) => {
                            // 返回相对路径（如果可能）
                            let display_path = if let Ok(relative) = path.strip_prefix(&search_path)
                            {
                                relative.to_string_lossy().to_string()
                            } else {
                                path.to_string_lossy().to_string()
                            };

                            // 区分文件和目录
                            let is_file = path.is_file();
                            let is_dir = path.is_dir();

                            matches.push(json!({
                                "path": display_path,
                                "is_file": is_file,
                                "is_dir": is_dir,
                                "absolute": path.to_string_lossy().to_string()
                            }));
                        }
                        Err(e) => {
                            debug!("Glob error: {}", e);
                        }
                    }
                }
            }
            Err(e) => {
                return ToolResult::error(format!("Invalid glob pattern: {}", e));
            }
        };

        // 限制结果数量
        const MAX_RESULTS: usize = 100;
        let total = matches.len();
        let truncated = if total > MAX_RESULTS {
            matches.truncate(MAX_RESULTS);
            true
        } else {
            false
        };

        // 格式化输出
        let file_list: Vec<String> = matches
            .iter()
            .map(|m| {
                let path = m["path"].as_str().unwrap_or("");
                let is_dir = m["is_dir"].as_bool().unwrap_or(false);
                if is_dir {
                    format!("{}/", path)
                } else {
                    path.to_string()
                }
            })
            .collect();

        let mut content = file_list.join("\n");

        if truncated {
            content.push_str(&format!(
                "\n\n[Found {} files, showing first {}]",
                total, MAX_RESULTS
            ));
        } else {
            content.push_str(&format!("\n\n[Found {} files]", total));
        }

        ToolResult::success_with_data(
            content,
            json!({
                "pattern": pattern,
                "total_matches": total,
                "truncated": truncated,
                "matches": matches
            }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_glob_tool() {
        let tool = GlobTool;
        let params = json!({
            "pattern": "src/**/*.rs"
        });
        let context = ToolContext::new(".", "test-session");

        let result = tool.execute(params, context).await;

        assert!(result.success);
        // 应该能找到 main.rs
        assert!(result.content.contains("main.rs"));
    }
}
