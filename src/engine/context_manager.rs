//! 上下文管理器（旧路径，已废弃）
//!
//! 当前主对话循环使用 `PreflightCompressionController`、`ContextBudgetController`
//! 和 `ContextCompressor` 的组合。此模块无生产调用，保留用于参考。

use crate::engine::context_compressor::{estimate_tokens, ContextCompressor};
use crate::services::api::Message;
use tracing::{debug, info};

#[deprecated(
    note = "使用 PreflightCompressionController + ContextBudgetController + ContextCompressor 替代。此模块无生产调用。"
)]
pub struct ContextManager {
    tool_result_budget: usize,
    snip_threshold: f64,
    compress_threshold: f64,
    max_context_tokens: u64,
    compressor: ContextCompressor,
    memory_context: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ContextAction {
    None,
    ToolBudgetApplied { truncated: usize },
    Snipped { removed: usize },
    Compressed { before: usize, after: usize },
}

#[allow(deprecated)]
impl ContextManager {
    pub fn new(max_context_tokens: u64) -> Self {
        Self {
            tool_result_budget: 2000,
            snip_threshold: 0.6,
            compress_threshold: 0.8,
            max_context_tokens,
            compressor: ContextCompressor::new(max_context_tokens),
            memory_context: None,
        }
    }

    pub fn with_tool_budget(mut self, budget: usize) -> Self {
        self.tool_result_budget = budget;
        self
    }

    pub fn with_thresholds(mut self, snip: f64, compress: f64) -> Self {
        self.snip_threshold = snip;
        self.compress_threshold = compress;
        self
    }

    pub fn set_memory(&mut self, memory: Option<String>) {
        self.memory_context = memory;
    }

    pub fn inject_memory(&self, user_message: &str) -> String {
        if let Some(ref memory) = self.memory_context {
            format!("[Memory]\n{}\n\n---\n\n{}", memory, user_message)
        } else {
            user_message.to_string()
        }
    }

    pub fn manage(&mut self, messages: &mut Vec<Message>) -> Vec<ContextAction> {
        let mut actions = Vec::new();
        let truncated = self.apply_tool_budget(messages);
        if truncated > 0 {
            actions.push(ContextAction::ToolBudgetApplied { truncated });
        }
        let usage = self.token_usage_ratio(messages);
        if usage > self.snip_threshold {
            let removed = self.snip_old_tool_results(messages);
            if removed > 0 {
                actions.push(ContextAction::Snipped { removed });
                debug!(
                    "Snipped {} old tool results (usage: {:.1}%)",
                    removed,
                    usage * 100.0
                );
            }
        }
        let usage2 = self.token_usage_ratio(messages);
        if usage2 > self.compress_threshold {
            let before = messages.len();
            *messages = self.compressor.compress(messages);
            let after = messages.len();
            actions.push(ContextAction::Compressed { before, after });
            info!("Compressed: {} -> {}", before, after);
        }
        if actions.is_empty() {
            actions.push(ContextAction::None);
        }
        actions
    }

    fn apply_tool_budget(&self, messages: &mut [Message]) -> usize {
        let mut count = 0;
        let budget = self.tool_result_budget;
        for msg in messages.iter_mut() {
            if let Message::Tool { content, .. } = msg {
                if content.len() > budget {
                    let orig = content.len();
                    let head: String = content.chars().take(budget * 70 / 100).collect();
                    let tail: String = content
                        .chars()
                        .rev()
                        .take(budget * 25 / 100)
                        .collect::<String>()
                        .chars()
                        .rev()
                        .collect();
                    *content = format!(
                        "{}\n\n[... {} bytes omitted ...]\n\n{}",
                        head,
                        orig - budget,
                        tail
                    );
                    count += 1;
                }
            }
        }
        count
    }

    fn snip_old_tool_results(&self, messages: &mut Vec<Message>) -> usize {
        let positions: Vec<usize> = messages
            .iter()
            .enumerate()
            .filter_map(|(i, m)| {
                if matches!(m, Message::Tool { .. }) {
                    Some(i)
                } else {
                    None
                }
            })
            .collect();
        if positions.len() <= 2 {
            return 0;
        }
        let keep = 2;
        let to_remove: Vec<usize> = positions[..positions.len().saturating_sub(keep)].to_vec();
        let mut ids = std::collections::HashSet::new();
        for &pos in &to_remove {
            if let Message::Tool { tool_call_id, .. } = &messages[pos] {
                ids.insert(tool_call_id.clone());
            }
        }
        let mut removed = 0;
        for &pos in to_remove.iter().rev() {
            messages.remove(pos);
            removed += 1;
        }
        for msg in messages.iter_mut() {
            if let Message::Assistant { tool_calls, .. } = msg {
                if let Some(calls) = tool_calls {
                    calls.retain(|tc| !ids.contains(&tc.id));
                    if calls.is_empty() {
                        *tool_calls = None;
                    }
                }
            }
        }
        removed
    }

    fn token_usage_ratio(&self, messages: &[Message]) -> f64 {
        let tokens: u64 = messages
            .iter()
            .map(|m| {
                let c = match m {
                    Message::System { content } => content,
                    Message::User { content } => content,
                    Message::Assistant { content, .. } => content,
                    Message::Tool { content, .. } => content,
                };
                estimate_tokens(c) + 4
            })
            .sum();
        tokens as f64 / self.max_context_tokens as f64
    }

    pub fn needs_compression(&self, messages: &[Message]) -> bool {
        self.token_usage_ratio(messages) > self.compress_threshold
    }
}

#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use super::*;
    use crate::services::api::ToolCall;

    #[test]
    fn test_memory_injection() {
        let mut mgr = ContextManager::new(128_000);
        mgr.set_memory(Some("user likes concise code".into()));
        let r = mgr.inject_memory("write a function");
        assert!(r.contains("Memory"));
        assert!(r.contains("concise"));
    }

    #[test]
    fn test_no_memory() {
        let mgr = ContextManager::new(128_000);
        assert_eq!(mgr.inject_memory("hello"), "hello");
    }

    #[test]
    fn test_tool_budget() {
        let mut mgr = ContextManager::new(128_000).with_tool_budget(100);
        let big = "x".repeat(500);
        let mut msgs = vec![
            Message::user("test"),
            Message::assistant_with_tools(
                "ok",
                vec![ToolCall {
                    id: "c1".into(),
                    name: "b".into(),
                    arguments: serde_json::json!({}),
                }],
            ),
            Message::tool("c1", &big),
        ];
        mgr.manage(&mut msgs);
        if let Message::Tool { content, .. } = &msgs[2] {
            assert!(content.len() < 500);
            assert!(content.contains("omitted"));
        }
    }

    #[test]
    fn test_manage_unicode_heavy_messages_does_not_panic() {
        let mut mgr = ContextManager::new(300).with_thresholds(0.2, 0.3);
        let unicode_chunk = "你好🚀".repeat(120);
        let mut msgs = vec![
            Message::system("sys"),
            Message::user(format!("需求: {}", unicode_chunk)),
            Message::assistant_with_tools(
                "准备执行",
                vec![ToolCall {
                    id: "c2".into(),
                    name: "grep".into(),
                    arguments: serde_json::json!({"pattern":"x"}),
                }],
            ),
            Message::tool("c2", unicode_chunk.clone()),
            Message::assistant("继续"),
        ];

        let actions = mgr.manage(&mut msgs);
        assert!(!actions.is_empty());
        assert!(!msgs.is_empty());
    }
}
