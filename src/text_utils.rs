//! Small shared text helpers with deliberately narrow semantics.

/// Return at most `max_chars` Unicode scalar values, appending an ellipsis when truncated.
pub(crate) fn truncate_preview(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }

    let mut preview: String = text.chars().take(max_chars.saturating_sub(1)).collect();
    preview.push('…');
    preview
}

#[cfg(test)]
mod tests {
    use super::truncate_preview;

    #[test]
    fn keeps_short_text_unchanged() {
        assert_eq!(truncate_preview("hello", 10), "hello");
    }

    #[test]
    fn truncates_on_char_boundary() {
        assert_eq!(truncate_preview("你好世界", 3), "你好…");
    }
}
