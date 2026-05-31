//! Grep 工具 - 内容搜索
//!
//! 在文件中搜索文本内容

use crate::tools::file_tool::resolve_path;
use crate::tools::{Tool, ToolContext, ToolOperationKind, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use std::hash::{Hash, Hasher};
use tracing::info;

/// Grep 文本搜索工具
pub struct GrepTool;

fn content_hash_hex(content: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    content.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn grep_no_match_recovery(
    pattern: &str,
    search_path: &std::path::Path,
    include_pattern: Option<&str>,
    files_searched: usize,
) -> serde_json::Value {
    let mut suggestions = vec![
        "Broaden the search path or search from the project root.".to_string(),
        "Try a simpler substring or escape regex metacharacters if a literal match was intended."
            .to_string(),
        "Use project_list search to find likely files, then grep within that narrower path."
            .to_string(),
    ];
    if include_pattern.is_some() {
        suggestions.push("Remove or relax the include glob and retry.".to_string());
    }
    if pattern.chars().any(|ch| ".+*?[](){}|^\\".contains(ch)) {
        suggestions
            .push("If this was intended as literal text, retry with regex escaping.".to_string());
    }
    json!({
        "reason": "no_matches",
        "pattern": pattern,
        "path": search_path.display().to_string(),
        "include": include_pattern,
        "files_searched": files_searched,
        "suggestions": suggestions,
    })
}

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }

    fn description(&self) -> &str {
        "Recursively grep file CONTENTS for a substring or regex — \
         'where is X called', 'what files contain Y'. Returns one match \
         per line as path:line: text. Skips dependency/VCS/build dirs \
         and binary files. For file NAMES use glob; for structured code \
         queries use symbol_query or find_in_code."
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

    fn aliases(&self) -> &'static [&'static str] {
        &["search"]
    }

    fn search_hint(&self) -> Option<&'static str> {
        Some("regex content search")
    }

    fn strict_schema(&self) -> bool {
        true
    }

    fn operation_kind(&self, _params: &serde_json::Value) -> ToolOperationKind {
        ToolOperationKind::Search
    }

    fn is_read_only(&self, _params: &serde_json::Value) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _params: &serde_json::Value) -> bool {
        true
    }

    fn max_result_size_chars(&self) -> Option<usize> {
        Some(40_000)
    }

    fn tool_use_summary(&self, params: &serde_json::Value) -> Option<String> {
        let pattern = params["pattern"].as_str()?.trim();
        if pattern.is_empty() {
            return None;
        }
        let path = params["path"].as_str().unwrap_or(".");
        Some(format!("{pattern} in {path}"))
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

        // 编译正则表达式（带 ReDoS 防护）
        const MAX_PATTERN_LEN: usize = 1000;
        if pattern.len() > MAX_PATTERN_LEN {
            return ToolResult::error(format!(
                "Regex pattern too long ({} chars, max {})",
                pattern.len(),
                MAX_PATTERN_LEN
            ));
        }
        let regex = match regex::RegexBuilder::new(pattern)
            .size_limit(1 << 20)
            .dfa_size_limit(1 << 20)
            .build()
        {
            Ok(r) => r,
            Err(e) => {
                return ToolResult::error(format!("Invalid regex pattern: {}", e));
            }
        };

        // 收集要搜索的文件
        let files_to_search = collect_files(&search_path, include_pattern).await;
        let files_searched = files_to_search.len();

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
                    let regex_match = regex.find(line);
                    match_count += 1;
                    matches.push(json!({
                        "file": relative_path.to_string(),
                        "resolved_file": file_path.to_string_lossy().to_string(),
                        "line": line_num + 1,
                        "line_start": line_num + 1,
                        "line_end": line_num + 1,
                        "content": line.to_string(),
                        "raw_line": line.to_string(),
                        "line_hash": content_hash_hex(line),
                        "match": regex_match.map(|m| m.as_str().to_string()),
                        "match_start_byte": regex_match.map(|m| m.start()),
                        "match_end_byte": regex_match.map(|m| m.end()),
                        "content_format": {
                            "visible_content": "raw_source_line",
                            "raw_content_in_tool_result": true,
                            "display_prefix": "{line}: ",
                            "truncation_hint_in_content": false
                        }
                    }));
                }
            }
        }

        // 格式化输出
        if matches.is_empty() {
            let recovery =
                grep_no_match_recovery(pattern, &search_path, include_pattern, files_searched);
            let suggestions = recovery["suggestions"]
                .as_array()
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item.as_str())
                        .map(|item| format!("- {}", item))
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_default();
            return ToolResult::success_with_data(
                format!(
                    "No matches found for '{}'.\n\nSearch recovery suggestions:\n{}",
                    pattern, suggestions
                ),
                json!({
                    "pattern": pattern,
                    "path": search_path.display().to_string(),
                    "include": include_pattern,
                    "kind": "search",
                    "total_matches": 0,
                    "truncated": false,
                    "files_searched": files_searched,
                    "display_format": "no_match_recovery",
                    "content_format": {
                        "visible_content": "recovery_text",
                        "raw_content_in_tool_result": false,
                        "display_prefix": serde_json::Value::Null,
                        "truncation_hint_in_content": false
                    },
                    "matches": [],
                    "search_recovery": recovery,
                }),
            );
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

            // Keep visible lines as raw source. Match metadata is available in
            // structured data; inline markdown highlighting can pollute patch
            // anchors when the model copies grep output into file_edit.
            output_lines.push(format!("{:4}: {}", line, content));
        }

        let mut output = output_lines.join("\n");
        if match_count >= MAX_MATCHES {
            output.push_str("\n\n[Results truncated, showing first 100 matches]");
        }

        ToolResult::success_with_data(
            output,
            json!({
                "pattern": pattern,
                "path": search_path.display().to_string(),
                "include": include_pattern,
                "kind": "search",
                "total_matches": match_count,
                "truncated": match_count >= MAX_MATCHES,
                "files_searched": files_searched,
                "display_format": "file_headers_with_line_numbers",
                "content_format": {
                    "visible_content": "grep_display",
                    "raw_match_lines_in_data": true,
                    "display_prefix": "{line}: ",
                    "truncation_hint_in_content": match_count >= MAX_MATCHES
                },
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

    #[tokio::test]
    async fn grep_output_keeps_source_lines_raw_without_markdown_highlight() {
        let tool = GrepTool;
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("script.sh");
        tokio::fs::write(
            &file_path,
            "summary_task() {\n  echo \"summary mode is not implemented yet\"\n}\n",
        )
        .await
        .unwrap();
        let params = json!({
            "pattern": "summary_task",
            "path": dir.path(),
            "include": "*.sh"
        });
        let context = ToolContext::new(".", "test-session");

        let result = tool.execute(params, context).await;

        assert!(result.success);
        assert!(result.content.contains("summary_task() {"));
        assert!(!result.content.contains("**summary_task**"));
        assert_eq!(
            result.data.as_ref().unwrap()["matches"][0]["match"],
            "summary_task"
        );
        let data = result.data.as_ref().unwrap();
        assert_eq!(data["kind"], "search");
        assert_eq!(data["display_format"], "file_headers_with_line_numbers");
        assert_eq!(data["content_format"]["raw_match_lines_in_data"], true);
        assert_eq!(data["matches"][0]["line_start"], 1);
        assert_eq!(data["matches"][0]["line_end"], 1);
        assert_eq!(data["matches"][0]["match_start_byte"], 0);
        assert_eq!(data["matches"][0]["match_end_byte"], "summary_task".len());
        assert_eq!(data["matches"][0]["raw_line"], "summary_task() {");
        assert!(data["matches"][0]["line_hash"].as_str().unwrap_or("").len() >= 8);
    }

    #[tokio::test]
    async fn test_grep_no_match_includes_recovery_metadata() {
        let tool = GrepTool;
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("sample.rs");
        tokio::fs::write(&file_path, "fn present_symbol() {}\n")
            .await
            .unwrap();
        let params = json!({
            "pattern": "absent_symbol",
            "path": dir.path(),
            "include": "*.rs"
        });
        let context = ToolContext::new(".", "test-session");

        let result = tool.execute(params, context).await;

        assert!(result.success);
        assert!(result.content.contains("Search recovery suggestions"));
        let recovery = result
            .data
            .as_ref()
            .and_then(|data| data.get("search_recovery"))
            .expect("search recovery metadata");
        assert_eq!(recovery["reason"], "no_matches");
        assert!(recovery["files_searched"].as_u64().unwrap_or(0) > 0);
        assert!(recovery["suggestions"].as_array().unwrap().len() >= 3);
    }
}
