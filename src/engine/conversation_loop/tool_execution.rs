//! 工具执行辅助函数与常量

use crate::tools::{ToolRegistry, ToolResult};
use serde_json::Value;

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

/// Maximum tool-call iterations per turn. Mirrors Reasonix's
/// DEFAULT_MAX_ITER_PER_TURN = 50. Env override: PRIORITY_AGENT_MAX_ITER.
#[allow(dead_code)]
pub(crate) const DEFAULT_MAX_ITERATIONS: usize = 50;

/// Whether tool dispatch should be forced serial (mirrors Reasonix's
/// REASONIX_TOOL_DISPATCH=serial). Default is parallel for read-only tools.
pub(crate) fn force_serial_tool_dispatch() -> bool {
    std::env::var("PRIORITY_AGENT_TOOL_DISPATCH")
        .map(|v| v.trim().eq_ignore_ascii_case("serial"))
        .unwrap_or(false)
}

/// 工具结果磁盘缓存目录
#[allow(dead_code)]
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

/// Default max result tokens before shrinking (mirrors Reasonix's
/// DEFAULT_MAX_RESULT_TOKENS = 4096). CJK text costs ~2× tokens vs ASCII.
#[allow(dead_code)] // API-ready: callers will integrate in follow-up
pub(crate) const DEFAULT_MAX_RESULT_TOKENS: usize = 4096;

/// Truncate a tool result string to fit within `max_tokens` tokens.
/// Uses a conservative estimate: 1 token ≈ 3 chars for CJK safety.
/// Preserves head + tail with a truncation marker.
#[allow(dead_code)] // API-ready: callers will integrate in follow-up
pub(crate) fn shrink_tool_result_by_tokens(content: &str, max_tokens: usize) -> String {
    if max_tokens == 0 {
        return content.to_string();
    }
    // Conservative: 1 token ≈ 3 chars (covers CJK which is ~2 chars/token).
    let max_chars = max_tokens.saturating_mul(3);
    if content.len() <= max_chars {
        return content.to_string();
    }
    let head_chars = max_chars * 3 / 4;
    let tail_chars = max_chars.saturating_sub(head_chars).saturating_sub(100);
    if tail_chars < 200 {
        let head = safe_prefix_by_bytes(content, max_chars);
        return format!("{head}\n\n... [truncated] ...");
    }
    let head = safe_prefix_by_bytes(content, head_chars);
    let tail = safe_suffix_by_bytes(content, tail_chars);
    let skipped = content.len().saturating_sub(head.len() + tail.len());
    format!("{head}\n\n... [truncated ~{skipped} chars, ~{max_tokens} token budget] ...\n\n{tail}")
}

#[allow(dead_code)]
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

#[allow(dead_code)]
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

#[allow(dead_code)]
fn high_signal_term_match(line: &str) -> Option<(&'static str, usize)> {
    let lower = line.to_lowercase();
    HIGH_SIGNAL_TOOL_RESULT_TERMS
        .iter()
        .find_map(|term| lower.find(term).map(|idx| (*term, idx)))
}

#[allow(dead_code)]
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

/// 截断工具结果，如果超过阈值则写入 ToolOutputStore
pub(crate) async fn truncate_tool_result(
    result: &mut ToolResult,
    tool_name: &str,
    tool_call_id: &str,
    session_id: Option<&str>,
) {
    let policy = crate::tool_output_store::ToolOutputPolicy::from_env();
    let threshold = policy.effective_threshold();
    if result.content.len() > threshold {
        let store = crate::tool_output_store::ToolOutputStore::new();
        let _ = tokio::fs::create_dir_all(store.base_dir()).await;

        let sid = session_id.unwrap_or("unknown");
        match store
            .truncate_or_store_with_policy(
                sid,
                tool_call_id,
                tool_name,
                &result.content,
                "text/plain",
                &policy,
            )
            .await
        {
            Ok(Some(meta)) => {
                let original_len = result.content.len();
                let preview = crate::tool_output_store::build_result_preview_with_policy(
                    &result.content,
                    &meta,
                    &policy,
                );
                result.content = preview;
                merge_tool_result_data(
                    result,
                    "output_truncation",
                    serde_json::json!({
                        "original_bytes": original_len,
                        "preview_bytes": result.content.len(),
                        "threshold_bytes": threshold,
                        "max_lines": policy.max_lines,
                        "preview_direction": format!("{:?}", policy.preview_direction),
                        "retention_days": policy.retention_days,
                        "output_uri": meta.uri(),
                        "tool_output_id": meta.id,
                    }),
                );
            }
            Ok(None) => {} // output was within threshold, already cleared by check above
            Err(e) => {
                tracing::warn!("failed to store tool output: {e}");
                // Fall back to simple truncation without URI
                let original = result.content.clone();
                let original_len = original.len();
                let half = 2048.min(original_len / 2);
                let first = safe_prefix_by_bytes(&original, half);
                let last = safe_suffix_by_bytes(&original, half);
                result.content = format!(
                    "[Output truncated: {original_len} bytes]\n--- First {half} bytes ---\n{first}\n\n--- Last {half} bytes ---\n{last}"
                );
            }
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

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn tool_call_is_read_only(
    registry: &ToolRegistry,
    tool_name: &str,
    params: &Value,
) -> bool {
    registry
        .get(tool_name)
        .map(|tool| tool.is_read_only(params))
        .unwrap_or_else(|| is_read_only(tool_name))
}

pub(crate) fn tool_call_is_concurrency_safe(
    registry: &ToolRegistry,
    tool_name: &str,
    params: &Value,
) -> bool {
    registry
        .get(tool_name)
        .map(|tool| tool.is_concurrency_safe(params))
        .unwrap_or_else(|| is_read_only(tool_name))
}

pub(crate) fn tool_call_is_storm_exempt(registry: &ToolRegistry, tool_name: &str) -> bool {
    registry
        .get(tool_name)
        .map(|tool| tool.requires_user_interaction())
        .unwrap_or(false)
        || matches!(
            tool_name,
            "project_list"
                | "memory_load"
                | "skills_list"
                | "skill_view"
                | "list_mcp_resources"
                | "read_mcp_resource"
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::ToolRegistryProfile;
    use serde_json::json;

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

    #[test]
    fn test_tool_call_concurrency_uses_tool_contract() {
        let registry = ToolRegistry::with_profile(ToolRegistryProfile::Core);

        assert!(tool_call_is_concurrency_safe(
            &registry,
            "bash",
            &json!({ "command": "ls -la" })
        ));
        assert!(tool_call_is_read_only(
            &registry,
            "bash",
            &json!({ "command": "ls -la" })
        ));
        assert!(tool_call_is_concurrency_safe(
            &registry,
            "grep",
            &json!({ "pattern": "ToolOperationKind" })
        ));
        assert!(!tool_call_is_concurrency_safe(
            &registry,
            "bash",
            &json!({ "command": "cargo test -q" })
        ));
        assert!(!tool_call_is_concurrency_safe(
            &registry,
            "file_write",
            &json!({ "path": "tmp.txt", "content": "hello" })
        ));
    }

    #[tokio::test]
    async fn test_truncate_tool_result_small_output() {
        let mut result = ToolResult::success("short output");
        truncate_tool_result(&mut result, "grep", "call_small", None).await;
        assert_eq!(result.content, "short output");
    }

    #[tokio::test]
    async fn test_truncate_tool_result_large_output() {
        let mut result = ToolResult::success("A".repeat(40_000));
        truncate_tool_result(&mut result, "grep", "call_large", None).await;
        assert!(result.content.contains("Output truncated"));
        assert!(!result.content.contains("tool-results"));
        assert!(result.content.contains("--- First"));
        assert!(result.content.contains("--- Last"));
    }

    #[tokio::test]
    async fn test_truncate_tool_result_preserves_head_and_tail() {
        let content = format!(
            "{}\nHIGH_SIGNAL_MIDDLE\n{}",
            "A".repeat(20_000),
            "B".repeat(20_000)
        );
        let mut result = ToolResult::success(content);
        truncate_tool_result(&mut result, "file_read", "call_signal", None).await;
        assert!(result.content.contains("Output truncated"));
        assert!(result.content.contains("tool-output://"));
        assert!(result.content.contains("First"));
        assert!(result.content.contains("Last"));
    }

    #[tokio::test]
    async fn test_truncate_tool_result_utf8_boundaries() {
        let mut result = ToolResult::success("中".repeat(20_000));
        truncate_tool_result(&mut result, "grep", "call_utf8", None).await;
        assert!(result.content.contains("Output truncated"));
    }
}
