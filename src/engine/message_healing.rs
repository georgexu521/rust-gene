//! Message healing pipeline — heals messages before each API call to prevent
//! 400 errors from oversized tool results, dangling tool_calls, or stale
//! reasoning content.
//!
//! Mirrors Reasonix's heal-on-every-send pattern.

use crate::services::api::{Message, ToolCall};

/// Result of running the healing pipeline over a set of messages.
#[derive(Debug, Clone, Default)]
pub struct HealReport {
    pub dangling_dropped: usize,
    pub oversized_shrunk: usize,
    pub chars_saved: usize,
}

/// Maximum chars for a single tool result before truncation.
const DEFAULT_MAX_TOOL_RESULT_CHARS: usize = 32_000;

/// Heal messages before sending to the LLM. Runs 2 passes:
///
/// 1. **Oversized tool results** — truncate tool results exceeding char limit.
/// 2. **Dangling tool_calls** — drop assistant messages whose tool_calls
///    reference tool results that don't exist (provider rejects mismatched pairs).
pub fn heal_active_log_before_send(
    messages: &[Message],
    max_result_chars: Option<usize>,
) -> (Vec<Message>, HealReport) {
    let max_chars = max_result_chars.unwrap_or(DEFAULT_MAX_TOOL_RESULT_CHARS);
    let mut report = HealReport::default();

    let messages = shrink_oversized_tool_results(messages, max_chars, &mut report);
    let messages = drop_dangling_tool_calls(&messages, &mut report);

    (messages, report)
}

/// Drop assistant messages whose tool_calls entries have no matching tool result.
fn drop_dangling_tool_calls(messages: &[Message], report: &mut HealReport) -> Vec<Message> {
    let tool_ids: std::collections::HashSet<&str> = messages
        .iter()
        .filter_map(|m| match m {
            Message::Tool { tool_call_id, .. } => Some(tool_call_id.as_str()),
            _ => None,
        })
        .collect();

    messages
        .iter()
        .filter_map(|msg| match msg {
            Message::Assistant {
                content,
                tool_calls,
            } => {
                let Some(calls) = tool_calls else {
                    return Some(msg.clone());
                };
                let valid: Vec<ToolCall> = calls
                    .iter()
                    .filter(|tc| {
                        let keep = tool_ids.contains(tc.id.as_str());
                        if !keep {
                            report.dangling_dropped += 1;
                        }
                        keep
                    })
                    .cloned()
                    .collect();
                if valid.is_empty() {
                    report.dangling_dropped += calls.len();
                    // Drop assistant message entirely — all tool calls are orphans.
                    None
                } else if valid.len() == calls.len() {
                    Some(msg.clone())
                } else {
                    Some(Message::Assistant {
                        content: content.clone(),
                        tool_calls: Some(valid),
                    })
                }
            }
            _ => Some(msg.clone()),
        })
        .collect()
}

/// Truncate tool result content exceeding the char limit.
fn shrink_oversized_tool_results(
    messages: &[Message],
    max_chars: usize,
    report: &mut HealReport,
) -> Vec<Message> {
    messages
        .iter()
        .map(|msg| match msg {
            Message::Tool {
                tool_call_id,
                content,
            } => {
                if content.len() <= max_chars {
                    return msg.clone();
                }
                report.oversized_shrunk += 1;
                report.chars_saved += content.len().saturating_sub(max_chars);
                Message::Tool {
                    tool_call_id: tool_call_id.clone(),
                    content: truncate_tool_result(content, max_chars),
                }
            }
            _ => msg.clone(),
        })
        .collect()
}

/// Truncate a tool result keeping head + tail, with a marker in the middle.
fn truncate_tool_result(content: &str, max_chars: usize) -> String {
    if content.len() <= max_chars {
        return content.to_string();
    }
    let head_bytes = max_chars * 3 / 4;
    let tail_bytes = max_chars.saturating_sub(head_bytes).saturating_sub(100);
    if tail_bytes < 200 {
        let head = safe_prefix(content, max_chars);
        return head.to_string();
    }
    let head = safe_prefix(content, head_bytes);
    let tail = safe_suffix(content, tail_bytes);
    let skipped = content.len().saturating_sub(head.len() + tail.len());
    format!("{head}\n\n... [truncated {skipped} chars] ...\n\n{tail}")
}

fn safe_prefix(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes.min(s.len());
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

fn safe_suffix(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut start = s.len().saturating_sub(max_bytes);
    while start < s.len() && !s.is_char_boundary(start) {
        start += 1;
    }
    &s[start..]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oversized_tool_result_is_truncated() {
        let big_content = "x".repeat(50_000);
        let messages = vec![Message::tool("call_1", &big_content)];
        let (healed, report) = heal_active_log_before_send(&messages, Some(10_000));
        assert_eq!(report.oversized_shrunk, 1);
        assert!(report.chars_saved > 0);
        let content = match &healed[0] {
            Message::Tool { content, .. } => content,
            _ => panic!("expected tool message"),
        };
        assert!(content.len() <= 10_500);
        assert!(content.contains("truncated"));
    }

    #[test]
    fn small_tool_result_is_untouched() {
        let messages = vec![Message::tool("call_1", "short result")];
        let (healed, report) = heal_active_log_before_send(&messages, Some(10_000));
        assert_eq!(report.oversized_shrunk, 0);
        let content = match &healed[0] {
            Message::Tool { content, .. } => content,
            _ => panic!("expected tool"),
        };
        assert_eq!(content, "short result");
    }

    #[test]
    fn assistant_without_tool_calls_is_untouched() {
        let messages = vec![Message::assistant("hello")];
        let (healed, _) = heal_active_log_before_send(&messages, None);
        assert_eq!(healed.len(), 1);
        match &healed[0] {
            Message::Assistant { content, .. } => assert_eq!(content, "hello"),
            _ => panic!("expected assistant"),
        }
    }
}
