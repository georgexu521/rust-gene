use crate::{state::MessageItem, tui::view_model::reasoning::assistant_reasoning_view};
use ratatui::{
    text::Text,
    widgets::{Paragraph, Wrap},
};

use super::MessageRenderOptions;
use super::{reasoning::render_reasoning_summary, text::append_markdown_lines, StreamMeta};

pub(super) fn render_assistant_message<'a>(
    message: &'a MessageItem,
    theme: &'a crate::tui::theme::Theme,
    stream: Option<&StreamMeta>,
    options: MessageRenderOptions,
) -> Paragraph<'a> {
    let is_streaming = stream.map(|s| s.is_streaming).unwrap_or(false);
    let tick = stream.map(|s| s.tick).unwrap_or(0);
    let is_error = !is_streaming && assistant_message_is_error(&message.content);

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

    let reasoning = assistant_reasoning_view(&message.content);
    if let Some(line) = render_reasoning_summary(&reasoning, theme) {
        lines.push(line);
    }
    if options.reasoning_expanded && reasoning.has_hidden_reasoning() {
        super::reasoning::append_reasoning_body(&mut lines, &reasoning, theme);
    }

    if is_error {
        let error_body = assistant_error_body(&reasoning.visible_answer);
        append_markdown_lines(&mut lines, &error_body, theme, "  ");
    } else {
        append_markdown_lines(&mut lines, &reasoning.visible_answer, theme, "  ");
    }
    Paragraph::new(Text::from(lines)).wrap(Wrap { trim: true })
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
