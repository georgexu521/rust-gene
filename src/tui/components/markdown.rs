//! Markdown 渲染组件
//!
//! 使用 pulldown-cmark 将 Markdown 转换为 ratatui::Text
//! 支持代码块语法高亮（syntect）

use pulldown_cmark::{Event, Tag, TagEnd};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
};
use std::sync::LazyLock;

// ── syntect 全局缓存 ──
static SYNTAX_SET: LazyLock<syntect::parsing::SyntaxSet> =
    LazyLock::new(syntect::parsing::SyntaxSet::load_defaults_newlines);
static THEME_SET: LazyLock<syntect::highlighting::ThemeSet> =
    LazyLock::new(syntect::highlighting::ThemeSet::load_defaults);

/// 将 syntect 样式转换为 ratatui 样式
fn syntect_to_ratatui(style: syntect::highlighting::Style) -> Style {
    use syntect::highlighting::FontStyle;
    let fg = Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
    let mut ratatui_style = Style::default().fg(fg);
    if style.font_style.contains(FontStyle::BOLD) {
        ratatui_style = ratatui_style.add_modifier(Modifier::BOLD);
    }
    if style.font_style.contains(FontStyle::ITALIC) {
        ratatui_style = ratatui_style.add_modifier(Modifier::ITALIC);
    }
    if style.font_style.contains(FontStyle::UNDERLINE) {
        ratatui_style = ratatui_style.add_modifier(Modifier::UNDERLINED);
    }
    ratatui_style
}

/// 高亮代码块内容
fn highlight_code_block(code: &str, language: &str) -> Vec<Line<'static>> {
    let ss = &*SYNTAX_SET;
    let ts = &*THEME_SET;

    let syntax = ss
        .find_syntax_by_token(language)
        .or_else(|| ss.find_syntax_by_extension(language))
        .unwrap_or_else(|| ss.find_syntax_plain_text());

    let theme = &ts.themes["base16-ocean.dark"];
    let mut highlighter = syntect::easy::HighlightLines::new(syntax, theme);

    let mut lines = Vec::new();
    for line in syntect::util::LinesWithEndings::from(code) {
        let highlighted = highlighter.highlight_line(line, ss).unwrap_or_default();
        let spans: Vec<Span<'static>> = highlighted
            .into_iter()
            .map(|(style, text)| Span::styled(text.to_string(), syntect_to_ratatui(style)))
            .collect();

        let mut full_spans = vec![Span::styled("  ", Style::default())];
        full_spans.extend(spans);
        lines.push(Line::from(full_spans));
    }
    lines
}

/// 将 Markdown 文本转换为 ratatui::Text
pub fn parse_markdown(text: &str) -> Text<'_> {
    let parser = pulldown_cmark::Parser::new(text);
    let mut lines: Vec<Line> = Vec::new();
    let mut current_line: Vec<Span> = Vec::new();
    let mut current_style = Style::default().fg(Color::White);
    let mut in_code_block = false;
    let mut code_language = String::new();
    let mut code_block_buffer = String::new();
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
                            pulldown_cmark::CodeBlockKind::Fenced(lang_str) => {
                                lang_str.to_string()
                            }
                            pulldown_cmark::CodeBlockKind::Indented => String::new(),
                        };
                        code_block_buffer.clear();
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
                        // 使用 syntect 高亮代码块
                        let highlighted = highlight_code_block(&code_block_buffer, &code_language);
                        lines.extend(highlighted);
                        lines.push(Line::from(Span::styled(
                            "```",
                            Style::default().fg(Color::DarkGray),
                        )));
                        lines.push(Line::from(""));
                        code_language.clear();
                        code_block_buffer.clear();
                    }
                    _ => {}
                }
            }
            Event::Text(text) => {
                if in_code_block {
                    code_block_buffer.push_str(&text);
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

    #[test]
    fn test_highlight_code_block_rust() {
        let code = "fn main() {\n    println!(\"hello\");\n}\n";
        let lines = highlight_code_block(code, "rust");
        assert!(!lines.is_empty());
        // Should have 3 lines of code (plus maybe extra)
        assert!(lines.len() >= 3);
    }

    #[test]
    fn test_highlight_code_block_unknown_lang() {
        let code = "some code here\n";
        let lines = highlight_code_block(code, "unknown_lang_xyz");
        assert!(!lines.is_empty());
        // Should fallback to plain text
        assert!(lines[0].to_string().contains("some code here"));
    }
}
