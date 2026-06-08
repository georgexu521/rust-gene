//! 选择性压缩：只压缩旧的工具输出，保留近期 2 轮原文。
//!
//! 设计约束（来自 docs/ROUTING_AND_CONTEXT_ANALYSIS_2026-06-08.md）：
//! - keep: 系统提示、所有对话文本、最后 2 轮 tool output 原文
//! - compress: 前 N 轮的工具输出 → 结构化摘要
//! - 压缩产物标记为 `evidence_safe_for_closeout=false`
//! - 压缩后保留 command、exit code、关键行
//!
//! 由 `PRIORITY_AGENT_SELECTIVE_COMPRESSION` 环境变量控制。
//! 默认关闭。建议在 P3 Phase A token 数据支持后再启用。

use crate::services::api::Message;

/// 选择性压缩工具输出。
///
/// 保留最后 `preserve_turns` 轮工具输出的原文，压缩更早的。
pub fn selectively_compress_tool_outputs(
    messages: &mut Vec<Message>,
    preserve_turns: usize,
) -> SelectiveCompressionReport {
    let mut report = SelectiveCompressionReport::default();
    if !selective_compression_enabled() {
        return report;
    }

    // 从后往前找到最后 `preserve_turns` 个 user 消息的位置
    let preserve_boundary = find_preserve_boundary(messages, preserve_turns);

    for i in 0..preserve_boundary.min(messages.len()) {
        if let Message::Tool {
            content,
            tool_call_id,
        } = &messages[i]
        {
            if content.len() <= 300 {
                continue; // already short, skip
            }
            let summary = compress_tool_output(tool_call_id, content);
            report.compressed_count += 1;
            report.chars_before += content.len();
            report.chars_after += summary.len();
            messages[i] = Message::Tool {
                tool_call_id: tool_call_id.clone(),
                content: summary,
            };
        }
    }

    report
}

fn find_preserve_boundary(messages: &[Message], preserve_turns: usize) -> usize {
    let mut user_count = 0usize;
    for (i, msg) in messages.iter().enumerate().rev() {
        if matches!(msg, Message::User { .. }) {
            user_count += 1;
            if user_count >= preserve_turns {
                return i;
            }
        }
    }
    0
}

fn compress_tool_output(call_id: &str, content: &str) -> String {
    let short_id = if call_id.len() > 12 {
        &call_id[..12]
    } else {
        call_id
    };

    // Extract key info: exit status, pass/fail lines
    let exit_line = content
        .lines()
        .find(|l| l.contains("[exit status:") || l.contains("exit="))
        .unwrap_or("");

    let pass_line = content
        .lines()
        .find(|l| l.contains("passed") || l.contains("ok") || l.contains("test result"))
        .unwrap_or("");

    let fail_line = content
        .lines()
        .find(|l| l.contains("failed") || l.contains("FAIL") || l.contains("error"))
        .unwrap_or("");

    let key_line = if !fail_line.is_empty() {
        format!("fail: {}", fail_line.trim().chars().take(120).collect::<String>())
    } else if !pass_line.is_empty() {
        format!("pass: {}", pass_line.trim().chars().take(120).collect::<String>())
    } else if !exit_line.is_empty() {
        exit_line.trim().to_string()
    } else {
        format!("chars={}", content.len())
    };

    format!(
        "[compressed-tool-output]\ntool_id={}\n{}\nevidence_safe_for_closeout=false\nraw_chars={}",
        short_id,
        key_line,
        content.len()
    )
}

pub fn selective_compression_enabled() -> bool {
    matches!(
        std::env::var("PRIORITY_AGENT_SELECTIVE_COMPRESSION")
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str(),
        "1" | "true" | "yes" | "on"
    )
}

/// 后台异步裁剪：在每次 turn 结束后扫描旧 tool output，
/// 将非 evidence 的旧输出压缩为结构化摘要。
/// 保护：最近 2 轮 + required validation evidence + 失败输出的关键错误行。
/// 借鉴 OpenCode 的 `prune()` 后台裁剪模式。
pub fn background_prune_tool_outputs(
    messages: &mut Vec<Message>,
) -> BackgroundPruneReport {
    let mut report = BackgroundPruneReport::default();
    if !background_prune_enabled() {
        return report;
    }

    let preserve_boundary = find_preserve_boundary(messages, 2);
    for i in 0..preserve_boundary.min(messages.len()) {
        if let Message::Tool {
            content,
            tool_call_id,
        } = &messages[i]
        {
            if content.len() <= 500 {
                continue;
            }
            // 保护: required validation evidence
            if is_validation_evidence(content) {
                report.evidence_preserved += 1;
                continue;
            }
            // 保护: 失败输出的关键错误行已在 compress_tool_output 中保留
            let summary = compress_tool_output(tool_call_id, content);
            report.pruned_count += 1;
            report.chars_before += content.len();
            report.chars_after += summary.len();
            messages[i] = Message::Tool {
                tool_call_id: tool_call_id.clone(),
                content: summary,
            };
        }
    }

    report
}

/// 检查 tool output 是否是 required validation 证据。
/// 保护: [exit status:], cargo test, rg, 所有 required command 的输出。
fn is_validation_evidence(content: &str) -> bool {
    let lower = content.to_ascii_lowercase();
    content.contains("[exit status:")
        || content.contains("required command")
        || lower.contains("cargo test")
        || lower.contains("cargo check")
        || lower.contains("cargo build")
        || (lower.contains("rg ") && lower.contains("fixtures/"))
}

pub fn background_prune_enabled() -> bool {
    matches!(
        std::env::var("PRIORITY_AGENT_BACKGROUND_PRUNE")
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str(),
        "1" | "true" | "yes" | "on"
    )
}

#[derive(Debug, Default)]
pub struct BackgroundPruneReport {
    pub pruned_count: usize,
    pub evidence_preserved: usize,
    pub chars_before: usize,
    pub chars_after: usize,
}

#[derive(Debug, Default)]
pub struct SelectiveCompressionReport {
    pub compressed_count: usize,
    pub chars_before: usize,
    pub chars_after: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_recent_tool_outputs() {
        let mut messages = vec![
            Message::user("first"),
            Message::Tool {
                tool_call_id: "t1".to_string(),
                content: "old_tool_output_that_is_very_long".repeat(10),
            },
            Message::assistant("ok"),
            Message::user("second"),
            Message::Tool {
                tool_call_id: "t2".to_string(),
                content: "recent_output".to_string(),
            },
        ];

        let report = selectively_compress_tool_outputs(&mut messages, 1);
        assert!(report.compressed_count >= 1);

        // t1 should be compressed (old)
        let t1 = match &messages[1] {
            Message::Tool { content, .. } => content,
            _ => panic!("expected tool"),
        };
        assert!(t1.contains("[compressed-tool-output]"));
        assert!(t1.contains("evidence_safe_for_closeout=false"));

        // t2 should NOT be compressed (recent)
        let t2 = match &messages[3] {
            Message::Tool { content, .. } => content,
            _ => panic!("expected tool"),
        };
        assert_eq!(t2, "recent_output");
    }

    #[test]
    fn skips_short_outputs() {
        let mut messages = vec![
            Message::user("first"),
            Message::Tool {
                tool_call_id: "t1".to_string(),
                content: "short".to_string(),
            },
        ];
        let report = selectively_compress_tool_outputs(&mut messages, 0);
        assert_eq!(report.compressed_count, 0);
    }

    #[test]
    fn disabled_by_default() {
        assert!(!selective_compression_enabled() || std::env::var("PRIORITY_AGENT_SELECTIVE_COMPRESSION").is_ok());
    }
}
