//! Markdown 渲染组件
//!
//! 使用 pulldown-cmark 将 Markdown 转换为 ratatui::Text

use pulldown_cmark::{Event, Tag, TagEnd};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
};

/// 将 Markdown 文本转换为 ratatui::Text
pub fn parse_markdown(text: &str) -> Text<'_> {
    let parser = pulldown_cmark::Parser::new(text);
    let mut lines: Vec<Line> = Vec::new();
    let mut current_line: Vec<Span> = Vec::new();
    let mut current_style = Style::default().fg(Color::White);
    let mut in_code_block = false;
    let mut code_language = String::new();
    let mut list_stack: Vec<u64> = Vec::new();
    let mut pending_text = String::new();

    for event in parser {
        match event {
            Event::Start(tag) => {
                flush_pending(&mut current_line, &mut pending_text, current_style);
                match tag {
                    Tag::Strong => {
                        current_style = current_style.add_modifier(Modifier::BOLD);
                    }
                    Tag::Emphasis => {
                        current_style = current_style.add_modifier(Modifier::ITALIC);
                    }
                    Tag::Link { .. } => {
                        current_style = current_style
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::UNDERLINED);
                    }
                    Tag::Heading { level, .. } => {
                        current_style =
                            current_style.fg(Color::Yellow).add_modifier(Modifier::BOLD);
                        // 添加前导空格表示层级
                        let indent = "  ".repeat(level as usize);
                        pending_text.push_str(&indent);
                    }
                    Tag::List(start) => {
                        list_stack.push(start.unwrap_or(1));
                    }
                    Tag::Item => {
                        let indent = "  ".repeat(list_stack.len().saturating_sub(1));
                        if let Some(idx) = list_stack.last_mut() {
                            pending_text.push_str(&format!("{}{}. ", indent, *idx));
                            *idx += 1;
                        } else {
                            pending_text.push_str(&format!("{}• ", indent));
                        }
                    }
                    Tag::CodeBlock(lang) => {
                        in_code_block = true;
                        code_language = match lang {
                            pulldown_cmark::CodeBlockKind::Fenced(lang_str) => lang_str.to_string(),
                            pulldown_cmark::CodeBlockKind::Indented => String::new(),
                        };
                        if !current_line.is_empty() || !pending_text.is_empty() {
                            flush_pending(&mut current_line, &mut pending_text, current_style);
                            lines.push(Line::from(current_line));
                            current_line = Vec::new();
                        }
                        if !code_language.is_empty() {
                            lines.push(Line::from(Span::styled(
                                format!("``` {}", code_language),
                                Style::default().fg(Color::DarkGray),
                            )));
                        } else {
                            lines.push(Line::from(Span::styled(
                                "```",
                                Style::default().fg(Color::DarkGray),
                            )));
                        }
                    }
                    _ => {}
                }
            }
            Event::End(tag_end) => {
                flush_pending(&mut current_line, &mut pending_text, current_style);
                match tag_end {
                    TagEnd::Strong => {
                        current_style = current_style.remove_modifier(Modifier::BOLD);
                    }
                    TagEnd::Emphasis => {
                        current_style = current_style.remove_modifier(Modifier::ITALIC);
                    }
                    TagEnd::Link => {
                        current_style = Style::default().fg(Color::White);
                    }
                    TagEnd::Heading(_) => {
                        current_style = Style::default().fg(Color::White);
                        lines.push(Line::from(current_line));
                        current_line = Vec::new();
                    }
                    TagEnd::Paragraph => {
                        if !current_line.is_empty() {
                            lines.push(Line::from(current_line));
                            current_line = Vec::new();
                        }
                        // 段落之间添加空行
                        lines.push(Line::from(""));
                    }
                    TagEnd::List(_) => {
                        list_stack.pop();
                        if !current_line.is_empty() {
                            lines.push(Line::from(current_line));
                            current_line = Vec::new();
                        }
                    }
                    TagEnd::Item => {
                        lines.push(Line::from(current_line));
                        current_line = Vec::new();
                    }
                    TagEnd::CodeBlock => {
                        in_code_block = false;
                        lines.push(Line::from(Span::styled(
                            "```",
                            Style::default().fg(Color::DarkGray),
                        )));
                        lines.push(Line::from(""));
                        code_language.clear();
                    }
                    _ => {}
                }
            }
            Event::Text(text) => {
                if in_code_block {
                    flush_pending(&mut current_line, &mut pending_text, current_style);
                    for line in text.split('\n') {
                        lines.push(Line::from(vec![
                            Span::styled("  ", Style::default()),
                            Span::styled(
                                line.to_string(),
                                Style::default()
                                    .fg(Color::Green)
                                    .add_modifier(Modifier::DIM),
                            ),
                        ]));
                    }
                } else {
                    pending_text.push_str(&text);
                }
            }
            Event::Code(code) => {
                pending_text.push_str(&code);
            }
            Event::Html(html) => {
                pending_text.push_str(&html);
            }
            Event::SoftBreak | Event::HardBreak => {
                pending_text.push('\n');
            }
            Event::Rule => {
                flush_pending(&mut current_line, &mut pending_text, current_style);
                if !current_line.is_empty() {
                    lines.push(Line::from(current_line));
                    current_line = Vec::new();
                }
                lines.push(Line::from(Span::styled(
                    "────────────────────────────────────────",
                    Style::default().fg(Color::DarkGray),
                )));
                lines.push(Line::from(""));
            }
            _ => {}
        }
    }

    flush_pending(&mut current_line, &mut pending_text, current_style);
    if !current_line.is_empty() {
        lines.push(Line::from(current_line));
    }

    Text::from(lines)
}

fn flush_pending(line: &mut Vec<Span>, pending: &mut String, style: Style) {
    if !pending.is_empty() {
        line.push(Span::styled(pending.clone(), style));
        pending.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_markdown_basic() {
        let text = "Hello **world** and *italic* text.";
        let result = parse_markdown(text);
        assert!(!result.lines.is_empty());
    }

    #[test]
    fn test_parse_markdown_code_block() {
        let text = "```rust\nfn main() {}\n```";
        let result = parse_markdown(text);
        let lines: Vec<_> = result.lines.iter().map(|l| l.to_string()).collect();
        assert!(lines.iter().any(|l| l.contains("fn main")));
    }

    #[test]
    fn test_parse_markdown_list() {
        let text = "- item 1\n- item 2";
        let result = parse_markdown(text);
        assert!(!result.lines.is_empty());
    }
}
