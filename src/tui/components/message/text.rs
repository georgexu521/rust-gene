use crate::tui::components::markdown::parse_markdown;
use ratatui::{
    style::Style,
    text::{Line, Span},
};

pub(super) fn append_markdown_lines(
    lines: &mut Vec<Line<'static>>,
    content: &str,
    theme: &crate::tui::theme::Theme,
    indent: &'static str,
) {
    let markdown_text = parse_markdown(content, theme);
    for line in markdown_text.lines {
        let mut spans = vec![Span::styled(indent, Style::default())];
        spans.extend(line.spans.into_iter().map(|span| {
            let content = span.content.to_string();
            Span::styled(content, span.style)
        }));
        lines.push(Line::from(spans));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::theme::Theme;

    #[test]
    fn append_markdown_lines_owns_rendered_spans() {
        let theme = Theme::graphite();
        let mut lines = Vec::new();

        append_markdown_lines(&mut lines, "**done**", &theme, "  ");

        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].spans[0].content.as_ref(), "  ");
        assert!(lines[0].spans.iter().any(|span| span.content == "done"));
    }
}
