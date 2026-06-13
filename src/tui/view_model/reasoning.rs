#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssistantReasoningView {
    pub visible_answer: String,
    pub hidden_reasoning: String,
    pub hidden_reasoning_lines: usize,
    pub has_unclosed_reasoning: bool,
}

impl AssistantReasoningView {
    pub fn has_hidden_reasoning(&self) -> bool {
        self.hidden_reasoning_lines > 0 || self.has_unclosed_reasoning
    }

    pub fn reasoning_label(&self) -> String {
        if self.has_unclosed_reasoning {
            "Thinking...".to_string()
        } else {
            format!(
                "Thinking hidden · {} lines",
                self.hidden_reasoning_lines.max(1)
            )
        }
    }
}

pub const EXPANDED_REASONING_MAX_LINES: usize = 12;

pub fn expanded_reasoning_lines(reasoning: &AssistantReasoningView) -> Vec<&str> {
    reasoning
        .hidden_reasoning
        .lines()
        .filter(|line| !line.trim().is_empty())
        .take(EXPANDED_REASONING_MAX_LINES)
        .collect()
}

pub fn expanded_reasoning_height(reasoning: &AssistantReasoningView) -> usize {
    let shown = expanded_reasoning_lines(reasoning).len();
    if shown == 0 {
        0
    } else {
        shown + usize::from(reasoning.hidden_reasoning_lines > EXPANDED_REASONING_MAX_LINES)
    }
}

pub fn assistant_reasoning_view(content: &str) -> AssistantReasoningView {
    let mut visible_answer = String::new();
    let mut hidden_reasoning = String::new();
    let mut hidden_reasoning_lines = 0usize;
    let mut has_unclosed_reasoning = false;
    let mut rest = content;

    while let Some(start) = rest.find("<think>") {
        visible_answer.push_str(&rest[..start]);
        let after_start = &rest[start + "<think>".len()..];
        if let Some(end) = after_start.find("</think>") {
            append_reasoning_block(&mut hidden_reasoning, &after_start[..end]);
            hidden_reasoning_lines += count_visible_lines(&after_start[..end]);
            rest = &after_start[end + "</think>".len()..];
        } else {
            append_reasoning_block(&mut hidden_reasoning, after_start);
            hidden_reasoning_lines += count_visible_lines(after_start);
            has_unclosed_reasoning = true;
            rest = "";
            break;
        }
    }
    visible_answer.push_str(rest);

    AssistantReasoningView {
        visible_answer: trim_excess_blank_lines(&visible_answer),
        hidden_reasoning: trim_excess_blank_lines(&hidden_reasoning),
        hidden_reasoning_lines,
        has_unclosed_reasoning,
    }
}

fn append_reasoning_block(buffer: &mut String, block: &str) {
    let trimmed = trim_excess_blank_lines(block);
    if trimmed.is_empty() {
        return;
    }
    if !buffer.is_empty() {
        buffer.push_str("\n\n");
    }
    buffer.push_str(&trimmed);
}

fn count_visible_lines(value: &str) -> usize {
    value.lines().filter(|line| !line.trim().is_empty()).count()
}

fn trim_excess_blank_lines(value: &str) -> String {
    let mut lines = value.lines().collect::<Vec<_>>();
    while lines.first().is_some_and(|line| line.trim().is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|line| line.trim().is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hides_closed_think_blocks_and_keeps_answer() {
        let view = assistant_reasoning_view("<think>one\ntwo</think>\n\nAnswer");

        assert_eq!(view.visible_answer, "Answer");
        assert_eq!(view.hidden_reasoning, "one\ntwo");
        assert_eq!(view.hidden_reasoning_lines, 2);
        assert!(view.has_hidden_reasoning());
        assert_eq!(view.reasoning_label(), "Thinking hidden · 2 lines");
    }

    #[test]
    fn treats_unclosed_think_block_as_active_reasoning() {
        let view = assistant_reasoning_view("<think>still thinking\nmore");

        assert_eq!(view.visible_answer, "");
        assert_eq!(view.hidden_reasoning, "still thinking\nmore");
        assert_eq!(view.hidden_reasoning_lines, 2);
        assert!(view.has_unclosed_reasoning);
        assert_eq!(view.reasoning_label(), "Thinking...");
    }

    #[test]
    fn preserves_content_without_reasoning_tags() {
        let view = assistant_reasoning_view("Plain answer");

        assert_eq!(view.visible_answer, "Plain answer");
        assert_eq!(view.hidden_reasoning, "");
        assert_eq!(view.hidden_reasoning_lines, 0);
        assert!(!view.has_hidden_reasoning());
    }

    #[test]
    fn expanded_reasoning_lines_are_bounded() {
        let body = (0..20)
            .map(|idx| format!("line {idx}"))
            .collect::<Vec<_>>()
            .join("\n");
        let view = assistant_reasoning_view(&format!("<think>{body}</think>Answer"));

        assert_eq!(
            expanded_reasoning_lines(&view).len(),
            EXPANDED_REASONING_MAX_LINES
        );
        assert_eq!(
            expanded_reasoning_height(&view),
            EXPANDED_REASONING_MAX_LINES + 1
        );
    }
}
