use crate::tui::{
    app::{AppMode, TuiApp},
    components::attachment_token::AttachmentToken,
    view_model::activity::{active_turn_status, format_elapsed, ActivePhase},
};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::Paragraph,
    Frame,
};

/// Render the prompt composer. During active turns, the composer stays visible
/// and a single active-status row is shown above the muted prompt line.
pub fn render_input_area(f: &mut Frame, app: &TuiApp, area: Rect) {
    let inner_area = Rect {
        x: area.x + 2,
        y: area.y,
        width: area.width.saturating_sub(4),
        height: area.height,
    };

    let input_text = app.composer.text.value();
    let tokens = app.composer_attachment_tokens();

    let prompt_color = match app.agent_mode {
        crate::engine::agent_mode::AgentMode::Auto
        | crate::engine::agent_mode::AgentMode::Build => app.theme.tokens.tone.brand,
        crate::engine::agent_mode::AgentMode::Plan
        | crate::engine::agent_mode::AgentMode::Explore => app.theme.tokens.tone.accent,
        crate::engine::agent_mode::AgentMode::Review => app.theme.tokens.tone.info,
    };

    let input_line = if app.mode == AppMode::VimNormal {
        Text::from(vec![Line::from(vec![
            Span::styled("› ", Style::default().fg(app.theme.tokens.fg.faint)),
            Span::styled(
                "Vim Normal: j/k scroll, i insert, : command, Ctrl+V toggle",
                Style::default().fg(app.theme.tokens.fg.faint),
            ),
        ])])
    } else if input_text.is_empty() && tokens.is_empty() {
        Text::from(vec![Line::from(vec![
            Span::styled("› ", Style::default().fg(prompt_color)),
            Span::styled(
                "Message Priority Agent...",
                Style::default().fg(app.theme.tokens.fg.faint),
            ),
        ])])
    } else {
        Text::from(input_lines_with_pills(
            input_text,
            prompt_color,
            app,
            &tokens,
        ))
    };

    let (display_text, style, prefix_line_count) = if app.is_querying {
        let mut lines = vec![context_strip_line(app, usize::from(inner_area.width))];
        if let Some(line) = attachment_line(app, usize::from(inner_area.width)) {
            lines.push(line);
        }
        lines.push(activity_line(app));
        let prefix_line_count = lines.len();
        lines.extend(input_line.lines.into_iter().map(|mut line| {
            for span in &mut line.spans {
                span.style = span.style.fg(app.theme.tokens.fg.faint);
            }
            line
        }));
        if let Some(ref err) = app.error_message {
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(err.clone(), Style::default().fg(app.theme.tokens.tone.err)),
            ]));
        }
        (Text::from(lines), Style::default(), prefix_line_count)
    } else {
        let mut lines = vec![context_strip_line(app, usize::from(inner_area.width))];
        if let Some(line) = attachment_line(app, usize::from(inner_area.width)) {
            lines.push(line);
        }
        let prefix_line_count = lines.len();
        lines.extend(input_line.lines);
        (Text::from(lines), Style::default(), prefix_line_count)
    };

    f.render_widget(Paragraph::new(display_text).style(style), inner_area);

    if !app.is_querying {
        let (cursor_line, cursor_col) = app.composer.text.cursor_line_column();
        let pill_offset = if tokens.is_empty() {
            0
        } else {
            tokens
                .iter()
                .map(|t| display_width(&t.pill_label()) + 1)
                .sum()
        };
        let cursor_x = inner_area.x + pill_offset as u16 + cursor_col as u16 + 2;
        let cursor_y = inner_area.y + prefix_line_count as u16 + cursor_line as u16;
        let actual_cursor_x = if cursor_line == 0 {
            cursor_x
        } else {
            inner_area.x + cursor_col as u16
        };
        f.set_cursor_position((
            actual_cursor_x.min(inner_area.x + inner_area.width - 1),
            cursor_y.min(inner_area.y + inner_area.height - 1),
        ));
    }
}

fn context_strip_line(app: &TuiApp, width: usize) -> Line<'static> {
    let provider_model = format!(
        "{} / {}",
        app.current_provider_label(),
        app.current_model_label()
    );
    let mut suffixes = Vec::new();

    if app.memory_use {
        suffixes.push((
            format!(" · {}", memory_label(app.memory_recall_mode.as_str())),
            Style::default().fg(app.theme.tokens.tone.info),
        ));
    }

    if !app.history.is_empty() {
        suffixes.push((
            format!(" · hist:{}", app.history.len()),
            Style::default().fg(app.theme.tokens.fg.faint),
        ));
    }

    if app.prompt_stash.is_some() {
        suffixes.push((
            " · stash".to_string(),
            Style::default().fg(app.theme.tokens.tone.accent),
        ));
    }

    let attachment_summaries = app.composer_attachment_summaries();
    let attachment_count = attachment_summaries.len();
    if attachment_count > 0 {
        suffixes.push((
            format!(" · files:{}", attachment_count),
            Style::default().fg(app.theme.tokens.tone.info),
        ));
    }

    let paste_summaries = app.pasted_block_summaries();
    let paste_count = paste_summaries.len();
    if paste_count > 0 {
        let paste_label = if width >= 96 {
            if let Some(first) = paste_summaries.first() {
                if paste_count > 1 {
                    format!(" · paste:{} {} (+{})", paste_count, first, paste_count - 1)
                } else {
                    format!(" · paste:{} {}", paste_count, first)
                }
            } else {
                format!(" · paste:{}", paste_count)
            }
        } else {
            format!(" · paste:{}", paste_count)
        };
        suffixes.push((
            paste_label,
            Style::default().fg(app.theme.tokens.tone.accent),
        ));
    }

    let mode_prefix = (app.agent_mode != crate::engine::agent_mode::AgentMode::Auto)
        .then(|| app.current_agent_mode_label());
    let prefix_width = mode_prefix
        .map(|label| display_width(label) + display_width(" · "))
        .unwrap_or(0);
    let suffix_width = suffixes
        .iter()
        .map(|(label, _)| display_width(label))
        .sum::<usize>();
    let provider_budget = width
        .saturating_sub(prefix_width)
        .saturating_sub(suffix_width)
        .max(1);
    let mut spans = Vec::new();
    if let Some(mode_label) = mode_prefix {
        spans.push(Span::styled(
            mode_label,
            Style::default().fg(app.theme.tokens.fg.faint),
        ));
        spans.push(Span::styled(
            " · ",
            Style::default().fg(app.theme.tokens.fg.faint),
        ));
    }
    spans.push(Span::styled(
        truncate_display_width(&provider_model, provider_budget),
        Style::default().fg(app.theme.tokens.fg.faint),
    ));

    for (label, style) in suffixes {
        spans.push(Span::styled(label, style));
    }

    Line::from(fit_spans_to_width(spans, width))
}

fn attachment_line(app: &TuiApp, width: usize) -> Option<Line<'static>> {
    let summaries = app.composer_attachment_summaries();
    if summaries.is_empty() {
        return None;
    }

    let mut summary = summaries
        .iter()
        .take(2)
        .cloned()
        .collect::<Vec<_>>()
        .join("  ");
    let remaining = summaries.len().saturating_sub(2);
    if remaining > 0 {
        summary.push_str(&format!("  +{remaining}"));
    }

    let prefix = "  files ";
    let full_hint = "  · /attach preview  · backspace removes last";
    let short_hint = "  · /attach preview";
    let hint = if width >= display_width(prefix) + display_width(full_hint) + 24 {
        full_hint
    } else {
        short_hint
    };
    let summary_budget = width
        .saturating_sub(display_width(prefix))
        .saturating_sub(display_width(hint))
        .max(1);
    let summary = truncate_display_width(&summary, summary_budget);

    let mut spans = vec![
        Span::styled(prefix, Style::default().fg(app.theme.tokens.tone.info)),
        Span::styled(summary, Style::default().fg(app.theme.tokens.fg.meta)),
    ];

    spans.push(Span::styled(
        hint,
        Style::default().fg(app.theme.tokens.fg.faint),
    ));

    Some(Line::from(spans))
}

fn truncate_display_width(value: &str, max_width: usize) -> String {
    if display_width(value) <= max_width {
        return value.to_string();
    }
    if max_width == 0 {
        return String::new();
    }
    if max_width == 1 {
        return "…".to_string();
    }

    let content_width = max_width - 1;
    let mut width = 0usize;
    let mut out = String::new();
    for ch in value.chars() {
        let ch_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if width + ch_width > content_width {
            break;
        }
        width += ch_width;
        out.push(ch);
    }
    out.push('…');
    out
}

fn display_width(value: &str) -> usize {
    unicode_width::UnicodeWidthStr::width(value)
}

fn fit_spans_to_width(mut spans: Vec<Span<'static>>, width: usize) -> Vec<Span<'static>> {
    if width == 0 {
        return Vec::new();
    }

    let mut used = 0usize;
    let mut fitted = Vec::new();
    for mut span in spans.drain(..) {
        let span_width = display_width(span.content.as_ref());
        if used + span_width <= width {
            used += span_width;
            fitted.push(span);
            continue;
        }

        let budget = width.saturating_sub(used);
        if budget > 0 {
            span.content = truncate_display_width(span.content.as_ref(), budget).into();
            fitted.push(span);
        }
        break;
    }
    fitted
}

fn memory_label(mode: &str) -> &'static str {
    match mode {
        "off" => "mem:off",
        "strict" => "mem:strict",
        "balanced" => "mem:bal",
        "preference-only" => "mem:pref",
        _ => "mem",
    }
}

fn input_lines_with_pills(
    input_text: &str,
    prompt_color: ratatui::style::Color,
    app: &TuiApp,
    tokens: &[AttachmentToken],
) -> Vec<Line<'static>> {
    let selection = app.composer.text.selection_range();
    let mut char_offset = 0usize;
    let mut lines: Vec<Line> = input_text
        .split('\n')
        .enumerate()
        .map(|(i, line)| {
            let line_len = line.chars().count();
            let spans = line_spans(line, i == 0, prompt_color, app, char_offset, selection);
            char_offset += line_len + 1; // include '\n'
            Line::from(spans)
        })
        .collect();
    if lines.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            "› ",
            Style::default().fg(prompt_color),
        )]));
    }
    if !tokens.is_empty() && !lines.is_empty() {
        let first = lines.first_mut().expect("non-empty");
        let mut pill_spans: Vec<Span<'static>> = tokens
            .iter()
            .map(|t| {
                Span::styled(
                    format!("{} ", t.pill_label()),
                    Style::default().fg(app.theme.tokens.tone.info),
                )
            })
            .collect();
        pill_spans.append(&mut first.spans);
        *first = Line::from(pill_spans);
    }
    lines
}

fn line_spans(
    line: &str,
    is_first: bool,
    prompt_color: ratatui::style::Color,
    app: &TuiApp,
    line_start_char: usize,
    selection: Option<(usize, usize)>,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    spans.push(Span::styled(
        if is_first { "› " } else { "  " },
        Style::default().fg(prompt_color),
    ));

    let body_style = Style::default().fg(app.theme.tokens.fg.body);
    let select_style = Style::default()
        .fg(Color::Black)
        .bg(app.theme.tokens.tone.brand)
        .add_modifier(Modifier::BOLD);

    let Some((sel_start, sel_end)) = selection else {
        spans.push(Span::styled(line.to_string(), body_style));
        return spans;
    };

    let line_end_char = line_start_char + line.chars().count();
    if sel_end <= line_start_char || sel_start >= line_end_char {
        spans.push(Span::styled(line.to_string(), body_style));
        return spans;
    }

    let local_start = sel_start.saturating_sub(line_start_char);
    let local_end = sel_end.min(line_end_char) - line_start_char;

    let chars: Vec<char> = line.chars().collect();
    if local_start > 0 {
        spans.push(Span::styled(
            chars[..local_start].iter().collect::<String>(),
            body_style,
        ));
    }
    spans.push(Span::styled(
        chars[local_start..local_end].iter().collect::<String>(),
        select_style,
    ));
    if local_end < chars.len() {
        spans.push(Span::styled(
            chars[local_end..].iter().collect::<String>(),
            body_style,
        ));
    }

    spans
}

fn activity_line(app: &TuiApp) -> Line<'static> {
    let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let ch = frames[app.tick_count % frames.len()];
    let status = active_turn_status(app);
    let phase = status
        .as_ref()
        .map(|status| status.phase)
        .unwrap_or(ActivePhase::Thinking);
    let label = status
        .as_ref()
        .map(|status| status.label.clone())
        .unwrap_or_else(|| "Thinking".to_string());
    let color = match phase {
        ActivePhase::ProviderSlow | ActivePhase::ProviderTimedOut => app.theme.tokens.tone.err,
        ActivePhase::ProviderRetrying | ActivePhase::ProviderWaiting | ActivePhase::Thinking => {
            app.theme.tokens.tone.warn
        }
        ActivePhase::Writing => app.theme.tokens.tone.info,
        ActivePhase::ToolRunning => app.theme.tokens.tone.brand,
        ActivePhase::PermissionWaiting => app.theme.tokens.tone.warn,
    };

    let mut spans = vec![
        Span::styled(format!("{} ", ch), Style::default().fg(color)),
        Span::styled(label, Style::default().fg(color)),
    ];

    if let Some(status) = status {
        if let Some(detail) = status.detail {
            spans.push(Span::styled(
                format!(" · {}", detail),
                Style::default().fg(app.theme.tokens.fg.faint),
            ));
        }
        if let Some(elapsed_ms) = status.elapsed_ms {
            spans.push(Span::styled(
                format!(" · {}", format_elapsed(elapsed_ms)),
                Style::default().fg(app.theme.tokens.fg.faint),
            ));
        }
        if status.interrupt_hint {
            spans.push(Span::styled(
                " · esc to interrupt",
                Style::default().fg(app.theme.tokens.fg.faint),
            ));
        }
    }

    Line::from(spans)
}
