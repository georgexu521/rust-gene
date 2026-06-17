//! 流式输出清理器：跨 chunk 移除记忆上下文标签区间。
//!
//! LLM 有时会在流式输出中复述注入的记忆上下文块（`<relevant-memory>`,
//! `<memory-context>` 等）。这个状态机确保这些区间中的内容永远不会
//! 泄漏到用户 UI，即使标签跨越多个流式 chunk。
//!
//! 借鉴自 Hermes 的 `StreamingContextScrubber`
//! (`agent/memory_manager.py:62-225`)。

/// 需要从流式输出中移除的标签对列表。
const SCRUBBED_TAG_PAIRS: &[(&str, &str)] = &[
    ("<relevant-memory>", "</relevant-memory>"),
    ("<memory-context>", "</memory-context>"),
    (
        "<relevant-memory-instructions>",
        "</relevant-memory-instructions>",
    ),
    (
        "<memory-context-instructions>",
        "</memory-context-instructions>",
    ),
    ("<memory-instructions>", "</memory-instructions>"),
];

/// 跨 chunk 的状态机，移除记忆上下文标签区间。
///
/// 用法：
/// ```ignore
/// let mut scrubber = ContextScrubber::new();
/// for chunk in stream {
///     let visible = scrubber.feed(&chunk);
///     if !visible.is_empty() {
///         emit(visible);
///     }
/// }
/// let trailing = scrubber.flush();
/// if !trailing.is_empty() {
///     emit(trailing);
/// }
/// ```
pub struct ContextScrubber {
    in_span: bool,
    buf: String,
}

impl ContextScrubber {
    pub fn new() -> Self {
        Self {
            in_span: false,
            buf: String::new(),
        }
    }

    #[cfg(test)]
    pub fn reset(&mut self) {
        self.in_span = false;
        self.buf.clear();
    }

    /// 喂入一个 chunk，返回可见部分（已移除记忆上下文标签区间）。
    pub fn feed(&mut self, text: &str) -> String {
        if text.is_empty() {
            return String::new();
        }
        let mut input = self.buf.clone();
        input.push_str(text);
        self.buf.clear();
        let mut out = String::new();

        while !input.is_empty() {
            if self.in_span {
                // 在区间内 — 寻找最近的关闭标签
                if let Some(idx) = find_any_close_tag(&input) {
                    input = input[idx..].to_string();
                    self.in_span = false;
                } else {
                    // 挂起可能的部分关闭标签尾
                    self.buf = hold_partial_close_suffix(&input);
                    return out;
                }
            } else {
                // 在区间外 — 寻找块边界的开标签
                if let Some((tag, idx)) = find_boundary_open_tag(&input) {
                    out.push_str(&input[..idx]);
                    input = input[idx + tag.len()..].to_string();
                    self.in_span = true;
                } else {
                    let held = hold_partial_suffix(&input);
                    if held > 0 {
                        out.push_str(&input[..input.len() - held]);
                        self.buf = input[input.len() - held..].to_string();
                    } else {
                        out.push_str(&input);
                    }
                    return out;
                }
            }
        }
        out
    }

    /// 流结束时清空挂起缓冲区。
    /// 如果仍在未闭合的区间内，丢弃内容（泄漏部分记忆上下文比截断回复更糟）。
    pub fn flush(&mut self) -> String {
        if self.in_span {
            self.in_span = false;
            self.buf.clear();
            return String::new();
        }
        std::mem::take(&mut self.buf)
    }
}

fn find_boundary_open_tag(input: &str) -> Option<(&'static str, usize)> {
    for (open, _) in SCRUBBED_TAG_PAIRS {
        if let Some(idx) = input.find(open) {
            // 必须在新行开头（或整段文本开头）
            let prefix = &input[..idx];
            if prefix.is_empty() || prefix.ends_with('\n') {
                return Some((open, idx));
            }
        }
    }
    None
}

fn find_any_close_tag(input: &str) -> Option<usize> {
    let lower = input.to_lowercase();
    for (_, close) in SCRUBBED_TAG_PAIRS {
        if let Some(idx) = lower.find(close) {
            return Some(idx + close.len());
        }
    }
    None
}

fn hold_partial_suffix(input: &str) -> usize {
    let lower = input.to_lowercase();
    for (open, _) in SCRUBBED_TAG_PAIRS {
        for i in (1..open.len()).rev() {
            if lower.ends_with(&open[..i]) {
                return i;
            }
        }
    }
    0
}

fn hold_partial_close_suffix(input: &str) -> String {
    let lower = input.to_lowercase();
    for (_, close) in SCRUBBED_TAG_PAIRS {
        for i in (1..close.len()).rev() {
            if lower.ends_with(&close[..i]) {
                return input[input.len() - i..].to_string();
            }
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passes_through_plain_text() {
        let mut scrubber = ContextScrubber::new();
        let result = scrubber.feed("hello world");
        assert_eq!(result, "hello world");
        assert_eq!(scrubber.flush(), "");
    }

    #[test]
    fn strips_memory_context_span() {
        let mut scrubber = ContextScrubber::new();
        let input = "before\n<memory-context>\nsome context\n</memory-context>\nafter";
        let result = scrubber.feed(input);
        assert_eq!(result, "before\n\nafter");
        assert_eq!(scrubber.flush(), "");
    }

    #[test]
    fn strips_relevant_memory_span() {
        let mut scrubber = ContextScrubber::new();
        let input = "text\n<relevant-memory>\ncached data\n</relevant-memory>\nmore";
        let result = scrubber.feed(input);
        assert_eq!(result, "text\n\nmore");
        assert_eq!(scrubber.flush(), "");
    }

    #[test]
    fn handles_split_across_chunks() {
        let mut scrubber = ContextScrubber::new();
        let r1 = scrubber.feed("start\n<memory-cont");
        assert_eq!(r1, "start\n");
        let r2 = scrubber.feed("ext>\nsecret\n</memory-context>\nend");
        assert_eq!(r2, "\nend");
        assert_eq!(scrubber.flush(), "");
    }

    #[test]
    fn handles_close_tag_split_across_chunks() {
        let mut scrubber = ContextScrubber::new();
        let r1 = scrubber.feed("<relevant-memory>\nsecret\n</relevant-m");
        assert_eq!(r1, "");
        let r2 = scrubber.feed("emory>\nvisible");
        assert_eq!(r2, "\nvisible");
        assert_eq!(scrubber.flush(), "");
    }

    #[test]
    fn ignores_tag_not_on_newline() {
        let mut scrubber = ContextScrubber::new();
        let input = "inline <memory-context> text </memory-context> here";
        let result = scrubber.feed(input);
        assert_eq!(result, input);
        // flush should still clear the buffer since we're not in a span
        assert_eq!(scrubber.flush(), "");
    }

    #[test]
    fn flush_discards_unterminated_span() {
        let mut scrubber = ContextScrubber::new();
        let r1 = scrubber.feed("<memory-context>\nleaked");
        assert_eq!(r1, "");
        let r2 = scrubber.flush();
        assert_eq!(r2, ""); // unterminated — discarded
    }

    #[test]
    fn empty_input_noop() {
        let mut scrubber = ContextScrubber::new();
        assert_eq!(scrubber.feed(""), "");
        assert_eq!(scrubber.feed(""), "");
        assert_eq!(scrubber.flush(), "");
    }

    #[test]
    fn reset_clears_state() {
        let mut scrubber = ContextScrubber::new();
        scrubber.feed("<memory-context>\ndata");
        assert!(scrubber.in_span);
        scrubber.reset();
        assert!(!scrubber.in_span);
        assert!(scrubber.buf.is_empty());
    }
}
