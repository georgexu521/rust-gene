//! 工具执行辅助函数与常量

use crate::tools::ToolResult;

/// 只读工具列表（不消耗迭代预算，可并发执行）
pub(crate) const READ_ONLY_TOOLS: &[&str] = &[
    "grep",
    "glob",
    "file_read",
    "project_list",
    "memory_load",
    "skills_list",
    "skill_view",
    "web_search",
    "list_mcp_resources",
    "read_mcp_resource",
];

pub(crate) const DEFAULT_READ_ONLY_TOOL_CONCURRENCY: usize = 8;

/// 工具结果截断阈值（字节），超过此值会截断并写入磁盘
pub(crate) const TOOL_RESULT_TRUNCATE_THRESHOLD: usize = 32 * 1024; // 32 KiB

/// 工具结果磁盘缓存目录
pub(crate) fn tool_result_cache_dir() -> std::path::PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("priority-agent")
        .join("tool-results")
}

pub(crate) fn read_only_tool_concurrency() -> usize {
    std::env::var("PRIORITY_AGENT_READ_ONLY_TOOL_CONCURRENCY")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(DEFAULT_READ_ONLY_TOOL_CONCURRENCY)
}

pub(crate) fn safe_prefix_by_bytes(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes.min(s.len());
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

pub(crate) fn safe_suffix_by_bytes(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut start = s.len().saturating_sub(max_bytes);
    while start < s.len() && !s.is_char_boundary(start) {
        start += 1;
    }
    &s[start..]
}

const HIGH_SIGNAL_TOOL_RESULT_TERMS: &[&str] = &[
    "assess_memory_candidate",
    "memorywriteoutcome",
    "write_decision.status",
    "memorystatus::accepted",
    "record_memory_decision",
    "add_learning_async",
    "add_topic_learning_async",
    "add_auto_learning_async",
    "format!(\"saved:",
    "saved: {}",
    "memory_save",
    "/save",
    "acceptance",
    "failed",
    "panic",
    "error:",
];

fn high_signal_tool_result_snippets(content: &str, max_bytes: usize) -> String {
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() || max_bytes == 0 {
        return String::new();
    }

    let mut ranges: Vec<(usize, usize)> = Vec::new();
    for (idx, line) in lines.iter().enumerate() {
        if high_signal_term_match(line).is_some() {
            let start = idx.saturating_sub(2);
            let end = (idx + 3).min(lines.len());
            if let Some((_, last_end)) = ranges.last_mut() {
                if start <= *last_end {
                    *last_end = (*last_end).max(end);
                    continue;
                }
            }
            ranges.push((start, end));
        }
    }

    let mut out = String::new();
    for (start, end) in ranges {
        if !out.is_empty() {
            out.push_str("\n...\n");
        }
        for line in &lines[start..end] {
            out.push_str(&format_high_signal_context_line(line));
            out.push('\n');
            if out.len() >= max_bytes {
                return safe_prefix_by_bytes(&out, max_bytes).to_string();
            }
        }
    }
    out
}

fn high_signal_term_match(line: &str) -> Option<(&'static str, usize)> {
    let lower = line.to_lowercase();
    HIGH_SIGNAL_TOOL_RESULT_TERMS
        .iter()
        .find_map(|term| lower.find(term).map(|idx| (*term, idx)))
}

fn format_high_signal_context_line(line: &str) -> String {
    const CONTEXT_LINE_LIMIT: usize = 320;
    const SIGNAL_LINE_LIMIT: usize = 900;

    if line.len() <= CONTEXT_LINE_LIMIT {
        return line.to_string();
    }

    if let Some((term, idx)) = high_signal_term_match(line) {
        let start = idx.saturating_sub(SIGNAL_LINE_LIMIT / 3);
        let end = (idx + term.len() + SIGNAL_LINE_LIMIT * 2 / 3).min(line.len());
        let mut byte_start = start;
        while byte_start > 0 && !line.is_char_boundary(byte_start) {
            byte_start -= 1;
        }
        let mut byte_end = end;
        while byte_end < line.len() && !line.is_char_boundary(byte_end) {
            byte_end += 1;
        }
        let prefix = if byte_start > 0 { "..." } else { "" };
        let suffix = if byte_end < line.len() { "..." } else { "" };
        return format!("{prefix}{}{suffix}", &line[byte_start..byte_end]);
    }

    format!("{}...", safe_prefix_by_bytes(line, CONTEXT_LINE_LIMIT))
}

/// 截断工具结果，如果超过阈值则写入磁盘
pub(crate) async fn truncate_tool_result(
    result: &mut ToolResult,
    tool_name: &str,
    tool_call_id: &str,
) {
    if result.content.len() > TOOL_RESULT_TRUNCATE_THRESHOLD {
        let cache_dir = tool_result_cache_dir();
        let _ = tokio::fs::create_dir_all(&cache_dir).await;

        let filename = format!(
            "{}_{}_{}.txt",
            tool_name,
            tool_call_id,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );
        let file_path = cache_dir.join(&filename);

        if tokio::fs::write(&file_path, &result.content).await.is_ok() {
            let original = result.content.clone();
            let original_len = original.len();
            let target_half_bytes = 2048.min(original_len / 2);
            let first = safe_prefix_by_bytes(&original, target_half_bytes);
            let last = safe_suffix_by_bytes(&original, target_half_bytes);
            let relevant = high_signal_tool_result_snippets(&original, 4096);
            let relevant_section = if relevant.trim().is_empty() {
                String::new()
            } else {
                format!("\n\n--- Relevant snippets ---\n{}", relevant)
            };
            result.content = format!(
                "[Output truncated: {} bytes -> saved to {}]\n\n--- First {} bytes ---\n{}\n\n--- Last {} bytes ---\n{}{}",
                original_len,
                file_path.display(),
                first.len(),
                first,
                last.len(),
                last,
                relevant_section
            );
            merge_tool_result_data(
                result,
                "output_truncation",
                serde_json::json!({
                    "original_bytes": original_len,
                    "preview_bytes": result.content.len(),
                    "threshold_bytes": TOOL_RESULT_TRUNCATE_THRESHOLD,
                    "stored_path": file_path.display().to_string(),
                }),
            );
        }
    }
}

fn merge_tool_result_data(result: &mut ToolResult, key: &str, value: serde_json::Value) {
    match result.data.take() {
        Some(serde_json::Value::Object(mut object)) => {
            object.insert(key.to_string(), value);
            result.data = Some(serde_json::Value::Object(object));
        }
        Some(existing) => {
            result.data = Some(serde_json::json!({
                "value": existing,
                key: value,
            }));
        }
        None => {
            result.data = Some(serde_json::json!({
                key: value,
            }));
        }
    }
}

/// 检查工具是否为只读（可并发执行）
pub(crate) fn is_read_only(tool_name: &str) -> bool {
    READ_ONLY_TOOLS.contains(&tool_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_prefix_by_bytes_short_string() {
        let s = "hello";
        assert_eq!(safe_prefix_by_bytes(s, 10), "hello");
    }

    #[test]
    fn test_safe_prefix_by_bytes_exact_boundary() {
        let s = "hello";
        assert_eq!(safe_prefix_by_bytes(s, 5), "hello");
    }

    #[test]
    fn test_safe_prefix_by_bytes_utf8_boundary() {
        let s = "你好世界";
        // "你好" is 6 bytes, "你" is 3 bytes, "好" is 3 bytes
        let prefix = safe_prefix_by_bytes(s, 4);
        assert_eq!(prefix, "你");
    }

    #[test]
    fn test_safe_suffix_by_bytes_short_string() {
        let s = "hello";
        assert_eq!(safe_suffix_by_bytes(s, 10), "hello");
    }

    #[test]
    fn test_safe_suffix_by_bytes_utf8_boundary() {
        let s = "你好世界";
        // "你好世界" = 12 bytes (3 bytes per char). Last 6 bytes = "世界"
        let suffix = safe_suffix_by_bytes(s, 6);
        assert_eq!(suffix, "世界");
    }

    #[test]
    fn test_is_read_only_tools() {
        assert!(is_read_only("grep"));
        assert!(is_read_only("glob"));
        assert!(is_read_only("file_read"));
        assert!(is_read_only("read_mcp_resource"));
        assert!(!is_read_only("file_write"));
        assert!(!is_read_only("bash"));
    }

    #[tokio::test]
    async fn test_truncate_tool_result_small_output() {
        let mut result = ToolResult::success("short output");
        truncate_tool_result(&mut result, "grep", "call_small").await;
        assert_eq!(result.content, "short output");
    }

    #[tokio::test]
    async fn test_truncate_tool_result_large_output() {
        let mut result = ToolResult::success("A".repeat(40_000));
        truncate_tool_result(&mut result, "grep", "call_large").await;
        assert!(result.content.contains("Output truncated"));
        assert!(result.content.contains("--- First"));
        assert!(result.content.contains("--- Last"));
    }

    #[tokio::test]
    async fn test_truncate_tool_result_preserves_high_signal_middle_snippet() {
        let content = format!(
            "{}\nlet assessment = assess_memory_candidate(content, category, &existing, true);\n{}",
            "A".repeat(20_000),
            "B".repeat(20_000)
        );
        let mut result = ToolResult::success(content);
        truncate_tool_result(&mut result, "file_read", "call_signal").await;
        assert!(result.content.contains("--- Relevant snippets ---"));
        assert!(result.content.contains("assess_memory_candidate"));
    }

    #[tokio::test]
    async fn test_truncate_tool_result_utf8_boundaries() {
        let mut result = ToolResult::success("中".repeat(20_000));
        truncate_tool_result(&mut result, "grep", "call_utf8").await;
        assert!(result.content.contains("Output truncated"));
    }
}
