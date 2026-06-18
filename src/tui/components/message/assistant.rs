use crate::{
    state::MessageItem,
    tui::{
        components::collapsible::flatten_line_breaks,
        sync_store::{TuiMessagePart, TuiPartKind},
        view_model::reasoning::assistant_reasoning_view,
        view_model::tool_rows::{tool_row_lines, tool_rows_for_runs},
    },
};
use ratatui::{
    style::{Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Paragraph, Wrap},
};

use super::MessageRenderOptions;
use super::{text::append_markdown_lines, StreamMeta};

pub(super) fn render_assistant_message<'a>(
    message: &'a MessageItem,
    parts: Option<&[TuiMessagePart]>,
    theme: &'a crate::tui::theme::Theme,
    stream: Option<&StreamMeta>,
    options: MessageRenderOptions,
    _width: usize,
) -> Paragraph<'a> {
    let is_streaming = stream.map(|s| s.is_streaming).unwrap_or(false);
    let tick = stream.map(|s| s.tick).unwrap_or(0);

    let (reasoning_view, visible_answer, is_error) = if let Some(parts) = parts {
        let text = parts
            .iter()
            .filter(|p| p.kind == TuiPartKind::Text)
            .map(|p| p.text.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");
        let reasoning = parts
            .iter()
            .find(|p| p.kind == TuiPartKind::Thinking)
            .map(|p| p.text.as_str())
            .unwrap_or("");
        let is_error = !is_streaming && assistant_message_is_error(&text);
        let visible_answer = if is_error {
            assistant_error_body(&text)
        } else {
            text
        };
        (
            AssistantReasoningViewForParts {
                hidden_reasoning: reasoning.to_string(),
                has_hidden_reasoning: !reasoning.trim().is_empty(),
                has_unclosed_reasoning: parts
                    .iter()
                    .find(|p| p.kind == TuiPartKind::Thinking)
                    .map(|p| p.streaming)
                    .unwrap_or(false),
            },
            visible_answer,
            is_error,
        )
    } else {
        let view = assistant_reasoning_view(&message.content);
        let is_error = !is_streaming && assistant_message_is_error(&view.visible_answer);
        let visible_answer = if is_error {
            assistant_error_body(&view.visible_answer)
        } else {
            view.visible_answer.clone()
        };
        (
            AssistantReasoningViewForParts {
                hidden_reasoning: view.hidden_reasoning.clone(),
                has_hidden_reasoning: view.has_hidden_reasoning(),
                has_unclosed_reasoning: view.has_unclosed_reasoning,
            },
            visible_answer,
            is_error,
        )
    };

    let (glyph, label, header_color) = if is_streaming {
        let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        (
            frames[tick % frames.len()],
            "Writing",
            theme.tokens.tone.brand,
        )
    } else if is_error {
        (
            theme.tokens.card.error.glyph,
            "Error",
            theme.tokens.card.error.color,
        )
    } else {
        ("‹", "Reply", theme.tokens.tone.ok)
    };

    let meta = if is_streaming {
        let tok = stream.and_then(|s| s.token_count).unwrap_or(0);
        let tps = stream
            .and_then(|s| s.started_at)
            .map(|start| {
                let elapsed = start.elapsed().as_secs_f64().max(0.5);
                let rate = tok as f64 / elapsed;
                format!("{} tok · {:.0} t/s", tok, rate)
            })
            .unwrap_or_else(|| format!("{} tok", tok));
        Some(tps)
    } else {
        completed_assistant_meta(message)
    };

    let meta = if let Some(model) = stream.and_then(|s| s.model_label.as_deref()) {
        match meta {
            Some(m) => Some(format!("{} · {}", m, model)),
            None => Some(model.to_string()),
        }
    } else {
        meta
    };

    let mut lines = vec![super::card_header(
        glyph,
        label,
        header_color,
        meta,
        theme.tokens.fg.faint,
    )];

    if let Some(line) = render_reasoning_summary_for_parts(&reasoning_view, theme) {
        lines.push(line);
    }
    if options.reasoning_expanded && reasoning_view.has_hidden_reasoning {
        append_reasoning_body_for_parts(&mut lines, &reasoning_view, theme);
    }

    if let Some(parts) = parts {
        append_part_lines(&mut lines, parts, theme);
    } else {
        let mut answer_lines = Vec::new();
        append_markdown_lines(&mut answer_lines, &visible_answer, theme, "  ");
        let answer_lines = flatten_line_breaks(answer_lines);
        lines.extend(answer_lines);
    }

    Paragraph::new(Text::from(lines)).wrap(Wrap { trim: true })
}

fn append_part_lines(
    lines: &mut Vec<Line<'static>>,
    parts: &[TuiMessagePart],
    theme: &crate::tui::theme::Theme,
) {
    for part in parts {
        match part.kind {
            TuiPartKind::Text => {
                if part.text.trim().is_empty() {
                    continue;
                }
                let mut answer_lines = Vec::new();
                append_markdown_lines(&mut answer_lines, &part.text, theme, "  ");
                lines.extend(flatten_line_breaks(answer_lines));
            }
            TuiPartKind::Tool => {
                let Some(run) = part.tool_run.as_ref() else {
                    continue;
                };
                let view = tool_rows_for_runs(std::slice::from_ref(run), 120);
                let Some(row) = view.rows.first() else {
                    continue;
                };
                for (idx, line) in tool_row_lines(row, false, run).into_iter().enumerate() {
                    let prefix = if idx == 0 { row.icon } else { " " };
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("  {prefix} "),
                            Style::default().fg(theme.tokens.tone.ok),
                        ),
                        Span::styled(line, Style::default().fg(theme.tokens.fg.faint).italic()),
                    ]));
                }
            }
            TuiPartKind::Thinking => {}
        }
    }
}

struct AssistantReasoningViewForParts {
    hidden_reasoning: String,
    has_hidden_reasoning: bool,
    has_unclosed_reasoning: bool,
}

impl AssistantReasoningViewForParts {
    fn reasoning_label(&self) -> String {
        if self.has_unclosed_reasoning {
            "Thinking...".to_string()
        } else {
            format!(
                "Thinking hidden · {} lines",
                self.hidden_reasoning
                    .lines()
                    .filter(|l| !l.trim().is_empty())
                    .count()
                    .max(1)
            )
        }
    }
}

fn render_reasoning_summary_for_parts(
    reasoning: &AssistantReasoningViewForParts,
    theme: &crate::tui::theme::Theme,
) -> Option<ratatui::text::Line<'static>> {
    reasoning.has_hidden_reasoning.then(|| {
        ratatui::text::Line::from(vec![
            ratatui::text::Span::styled("  ", ratatui::style::Style::default()),
            ratatui::text::Span::styled(
                reasoning.reasoning_label(),
                ratatui::style::Style::default()
                    .fg(theme.tokens.fg.faint)
                    .add_modifier(ratatui::style::Modifier::ITALIC),
            ),
        ])
    })
}

fn append_reasoning_body_for_parts(
    lines: &mut Vec<ratatui::text::Line<'static>>,
    reasoning: &AssistantReasoningViewForParts,
    theme: &crate::tui::theme::Theme,
) {
    use crate::tui::view_model::reasoning::EXPANDED_REASONING_MAX_LINES;
    let shown: Vec<&str> = reasoning
        .hidden_reasoning
        .lines()
        .filter(|line| !line.trim().is_empty())
        .take(EXPANDED_REASONING_MAX_LINES)
        .collect();
    for line in &shown {
        lines.push(ratatui::text::Line::from(vec![
            ratatui::text::Span::styled("    ", ratatui::style::Style::default()),
            ratatui::text::Span::styled(
                line.to_string(),
                ratatui::style::Style::default().fg(theme.tokens.fg.sub),
            ),
        ]));
    }

    let total = reasoning
        .hidden_reasoning
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count();
    if total > shown.len() {
        lines.push(ratatui::text::Line::from(vec![
            ratatui::text::Span::styled("    ", ratatui::style::Style::default()),
            ratatui::text::Span::styled(
                format!("... {} more reasoning lines", total - shown.len()),
                ratatui::style::Style::default()
                    .fg(theme.tokens.fg.faint)
                    .add_modifier(ratatui::style::Modifier::ITALIC),
            ),
        ]));
    }
}

fn assistant_message_is_error(content: &str) -> bool {
    let trimmed = content.trim_start();
    trimmed.starts_with("[Error:")
        || trimmed.starts_with("[Error]")
        || trimmed.starts_with("Error:")
        || trimmed.starts_with("Failed to get response")
}

fn assistant_error_body(content: &str) -> String {
    let trimmed = content.trim();
    if let Some(inner) = trimmed
        .strip_prefix("[Error:")
        .and_then(|value| value.strip_suffix(']'))
    {
        let body = inner.trim();
        return if body.is_empty() {
            "Provider request failed.".to_string()
        } else {
            body.to_string()
        };
    }
    if let Some(inner) = trimmed.strip_prefix("[Error]").map(str::trim) {
        return if inner.is_empty() {
            "Provider request failed.".to_string()
        } else {
            inner.to_string()
        };
    }
    if let Some(inner) = trimmed.strip_prefix("Error:").map(str::trim) {
        return if inner.is_empty() {
            "Provider request failed.".to_string()
        } else {
            inner.to_string()
        };
    }
    trimmed.to_string()
}

fn completed_assistant_meta(message: &MessageItem) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(tokens) = message.metadata.get("completion_tokens") {
        parts.push(format!("{tokens} tok"));
    } else if let Some(tokens) = message.metadata.get("total_tokens") {
        parts.push(format!("{tokens} total tok"));
    }
    if let Some(elapsed) = message
        .metadata
        .get("elapsed_ms")
        .and_then(|value| value.parse::<u64>().ok())
    {
        parts.push(format_elapsed_ms(elapsed));
    }
    if let Some(validation) = message.metadata.get("validation_status") {
        parts.push(format!("validation {validation}"));
    }
    if let Some(failed_tools) = message.metadata.get("failed_tool_count") {
        parts.push(format!("{failed_tools} failed tools"));
    } else if let Some(tool_count) = message.metadata.get("tool_count") {
        parts.push(format!("{tool_count} tools"));
    }
    if let Some(reasoning_tokens) = message.metadata.get("reasoning_tokens") {
        parts.push(format!("{reasoning_tokens} reasoning"));
    }
    if let Some(cached_tokens) = message.metadata.get("cached_tokens") {
        parts.push(format!("cache {cached_tokens}"));
    }
    if let Some(cache_write_tokens) = message.metadata.get("cache_write_tokens") {
        parts.push(format!("cache write {cache_write_tokens}"));
    }
    if let Some(provider_phase) = message.metadata.get("provider_phase") {
        if provider_phase != "provider done" {
            parts.push(provider_phase.clone());
        }
    }
    if let Some(model) = message.metadata.get("model_label") {
        parts.push(model.clone());
    }

    (!parts.is_empty()).then(|| parts.join(" · "))
}

fn format_elapsed_ms(elapsed_ms: u64) -> String {
    if elapsed_ms < 1_000 {
        format!("{elapsed_ms}ms")
    } else {
        format!("{:.1}s", elapsed_ms as f64 / 1_000.0)
    }
}
