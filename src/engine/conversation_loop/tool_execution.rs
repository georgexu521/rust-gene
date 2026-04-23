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
            result.content = format!(
                "[Output truncated: {} bytes -> saved to {}]\n\n--- First {} bytes ---\n{}\n\n--- Last {} bytes ---\n{}",
                original_len,
                file_path.display(),
                first.len(),
                first,
                last.len(),
                last
            );
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
    async fn test_truncate_tool_result_utf8_boundaries() {
        let mut result = ToolResult::success("中".repeat(20_000));
        truncate_tool_result(&mut result, "grep", "call_utf8").await;
        assert!(result.content.contains("Output truncated"));
    }
}
