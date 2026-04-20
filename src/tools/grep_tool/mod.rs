//! Grep 工具 - 内容搜索
//!
//! 在文件中搜索文本内容

use crate::tools::file_tool::resolve_path;
use crate::tools::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use tracing::info;

/// Grep 文本搜索工具
pub struct GrepTool;

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }

    fn description(&self) -> &str {
        "Search for text patterns in files. \
         Returns matching lines with file names and line numbers. \
         Supports regular expressions."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "The regex pattern to search for"
                },
                "path": {
                    "type": "string",
                    "description": "The file or directory to search in (default: current directory)"
                },
                "include": {
                    "type": "string",
                    "description": "Glob pattern for files to include (e.g., '*.rs')"
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

        let include_pattern = params["include"].as_str();

        info!("Grep search: '{}' in {:?}", pattern, search_path);

        // 编译正则表达式
        let regex = match regex::Regex::new(pattern) {
            Ok(r) => r,
            Err(e) => {
                return ToolResult::error(format!("Invalid regex pattern: {}", e));
            }
        };

        // 收集要搜索的文件
        let files_to_search = collect_files(&search_path, include_pattern).await;

        // 执行搜索
        let mut matches = Vec::new();
        let mut match_count = 0;
        const MAX_MATCHES: usize = 100;

        for file_path in files_to_search {
            if match_count >= MAX_MATCHES {
                break;
            }

            let content = match tokio::fs::read_to_string(&file_path).await {
                Ok(c) => c,
                Err(_) => continue, // 跳过无法读取的文件（如二进制文件）
            };

            let relative_path = file_path
                .strip_prefix(&context.working_dir)
                .unwrap_or(&file_path)
                .to_string_lossy();

            for (line_num, line) in content.lines().enumerate() {
                if match_count >= MAX_MATCHES {
                    break;
                }

                if regex.is_match(line) {
                    match_count += 1;
                    matches.push(json!({
                        "file": relative_path.to_string(),
                        "line": line_num + 1,
                        "content": line.to_string(),
                        "match": regex.find(line).map(|m| m.as_str().to_string())
                    }));
                }
            }
        }

        // 格式化输出
        if matches.is_empty() {
            return ToolResult::success("No matches found.");
        }

        let mut output_lines = Vec::new();
        let mut current_file: Option<String> = None;

        for m in &matches {
            let file = m["file"].as_str().unwrap_or("");
            let line = m["line"].as_u64().unwrap_or(0);
            let content = m["content"].as_str().unwrap_or("");

            // 当文件改变时打印文件名
            if current_file.as_ref() != Some(&file.to_string()) {
                output_lines.push(format!("\n{}", file));
                current_file = Some(file.to_string());
            }

            // 高亮匹配部分
            let highlighted = regex.replace_all(content, |caps: &regex::Captures| {
                format!("**{}**", &caps[0])
            });

            output_lines.push(format!("{:4}: {}", line, highlighted));
        }

        let mut output = output_lines.join("\n");
        if match_count >= MAX_MATCHES {
            output.push_str("\n\n[Results truncated, showing first 100 matches]");
        }

        ToolResult::success_with_data(
            output,
            json!({
                "pattern": pattern,
                "total_matches": match_count,
                "truncated": match_count >= MAX_MATCHES,
                "matches": matches
            }),
        )
    }
}

/// 收集要搜索的文件
async fn collect_files(
    path: &std::path::Path,
    include_pattern: Option<&str>,
) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();

    if path.is_file() {
        files.push(path.to_path_buf());
        return files;
    }

    let mut entries = match tokio::fs::read_dir(path).await {
        Ok(e) => e,
        Err(_) => return files,
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let entry_path = entry.path();

        if entry_path.is_dir() {
            // 递归搜索子目录，但跳过常见忽略目录
            let dir_name = entry_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();

            if should_skip_dir(&dir_name) {
                continue;
            }

            let sub_files = Box::pin(collect_files(&entry_path, include_pattern)).await;
            files.extend(sub_files);
        } else if entry_path.is_file() {
            // 检查文件是否符合 include 模式
            if let Some(pattern) = include_pattern {
                let file_name = entry_path
                    .file_name()
                    .map(|n| n.to_string_lossy())
                    .unwrap_or_default();

                if !glob::Pattern::new(pattern)
                    .map(|p| p.matches(&file_name))
                    .unwrap_or(false)
                {
                    continue;
                }
            }

            files.push(entry_path);
        }
    }

    files
}

/// 检查是否应该跳过此目录
fn should_skip_dir(name: &str) -> bool {
    let skip_dirs = [
        ".git",
        ".svn",
        ".hg", // 版本控制
        "node_modules",
        "vendor", // 依赖
        "target",
        "build",
        "dist", // 构建输出
        ".idea",
        ".vscode", // IDE
        "__pycache__",
        ".pytest_cache", // Python
    ];

    skip_dirs.contains(&name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_grep_tool() {
        let tool = GrepTool;
        let params = json!({
            "pattern": "fn main",
            "path": "src"
        });
        let context = ToolContext::new(".", "test-session");

        let result = tool.execute(params, context).await;

        assert!(result.success);
        // 应该能找到 main 函数
        assert!(result.content.contains("main") || result.content.contains("No matches"));
    }

    #[tokio::test]
    async fn test_grep_with_include() {
        let tool = GrepTool;
        let params = json!({
            "pattern": "fn ",
            "path": "src",
            "include": "*.rs"
        });
        let context = ToolContext::new(".", "test-session");

        let result = tool.execute(params, context).await;

        assert!(result.success);
    }
}
