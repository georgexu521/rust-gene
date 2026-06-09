//! 主屏幕
//!
//! 包含聊天区、输入区、状态栏的渲染

use crate::{
    state::{MessageItem, MessageRole},
    tui::{
        app::{SidebarPanel, TuiApp},
        components::message,
        tool_view::{ToolRunStatus, ToolRunView},
    },
};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

mod approvals;
pub use approvals::*;
mod status_bar;
pub use status_bar::*;
mod popups;
pub use popups::*;

/// 渲染聊天区域（Claude Code 风格：无边框，留白分隔）
pub fn render_chat_area(f: &mut Frame, app: &TuiApp, area: Rect) {
    // 直接使用 area，不添加外边框
    let inner_area = area;

    // Session intro line (Reasonix style)
    let mut top_offset = 0u16;
    if !app.messages.is_empty() {
        let session_id = app.session_manager.current_session_id().unwrap_or("?");
        let short_id = if session_id.len() > 8 {
            &session_id[..8]
        } else {
            session_id
        };
        let intro = Paragraph::new(Line::from(vec![
            Span::styled(
                format!("◈ {} · ", short_id),
                Style::default().fg(app.theme.tokens.fg.faint),
            ),
            Span::styled(
                format!("{} · ", app.current_model_label()),
                Style::default().fg(app.theme.tokens.fg.faint),
            ),
            Span::styled(
                app.current_agent_mode_label(),
                Style::default().fg(app.theme.tokens.fg.faint),
            ),
        ]));
        f.render_widget(
            intro,
            Rect {
                x: inner_area.x + 2,
                y: inner_area.y,
                width: inner_area.width.saturating_sub(4),
                height: 1,
            },
        );
        top_offset = 1;
    }

    // 如果有消息，渲染它们
    if app.messages.is_empty() {
        let welcome = Paragraph::new(Text::from(vec![
            Line::from(""),
            Line::from(vec![Span::styled(
                "Welcome to Priority Agent",
                Style::default()
                    .fg(app.theme.tokens.fg.strong)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(vec![Span::styled(
                "Type a message and press Enter to chat.",
                Style::default().fg(app.theme.tokens.fg.faint),
            )]),
            Line::from(vec![Span::styled(
                format!("Model: {}", app.current_model_label()),
                Style::default().fg(app.theme.tokens.fg.faint),
            )]),
        ]))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
        f.render_widget(welcome, inner_area);
        return;
    }

    // 计算可见消息（focus_mode 下仅显示 user/assistant）
    let messages: Vec<_> = if app.focus_mode {
        app.visible_messages()
            .iter()
            .filter(|m| {
                matches!(
                    m.role,
                    crate::state::MessageRole::User | crate::state::MessageRole::Assistant
                )
            })
            .collect()
    } else {
        app.visible_messages().iter().collect()
    };

    let items = transcript_items(&messages, app);
    let content_top = inner_area.y + top_offset;
    let content_height = inner_area.height.saturating_sub(top_offset);
    let max_y = content_top + content_height;
    let bottom_anchored = app.scroll_offset >= messages.len();
    let window = transcript_window(
        &items,
        app.scroll_offset,
        bottom_anchored,
        content_height,
        inner_area.width as usize,
        app,
    );

    // Compute total scroll rows and remaining
    let total_rows: usize = item_heights(&items, inner_area.width as usize, app)
        .iter()
        .sum();
    let viewport_rows = content_height as usize;
    let max_scroll = total_rows.saturating_sub(viewport_rows);
    let scroll_top = window.start; // approximate

    let mut current_y = content_top + u16::from(window.more_above);

    // Scroll indicator (Reasonix style)
    let show_indicator = !window.bottom_anchored;
    if show_indicator && max_scroll > 0 {
        let above = scroll_top;
        let remaining = max_scroll.saturating_sub(scroll_top);
        let mut indicator_parts = vec![Span::styled(
            format!("{} above", above),
            Style::default()
                .fg(app.theme.tokens.fg.faint)
                .add_modifier(Modifier::ITALIC),
        )];
        if remaining > 0 {
            indicator_parts.push(Span::styled(
                format!(" · {} remaining", remaining),
                Style::default().fg(app.theme.tokens.fg.faint),
            ));
        }
        indicator_parts.push(Span::styled(
            " · PgUp/PgDn scroll",
            Style::default().fg(app.theme.tokens.fg.faint),
        ));

        let indicator = Paragraph::new(Line::from(indicator_parts))
            .style(Style::default().bg(app.theme.tokens.surface.bg_elev));
        f.render_widget(
            indicator,
            Rect {
                x: inner_area.x,
                y: content_top,
                width: inner_area.width,
                height: 1,
            },
        );
        current_y = content_top + 1;
    }

    for item in items.iter().skip(window.start) {
        if current_y >= max_y {
            break;
        }

        let msg_height = estimate_transcript_item_height(item, inner_area.width as usize, app);
        let msg_height = (msg_height as u16).min(max_y - current_y);
        let msg_area = Rect {
            x: inner_area.x,
            y: current_y,
            width: inner_area.width,
            height: msg_height,
        };

        match item {
            TranscriptItem::Message { message_index, msg } => {
                let collapsed = app.collapsed_indices.contains(message_index);
                // Streaming state for last assistant message
                let stream_meta = if app.is_querying
                    && msg.role == MessageRole::Assistant
                    && *message_index == messages.len() - 1
                {
                    let tokens = app.stream_usage_snapshot.map(|u| u.completion_tokens);
                    Some(message::StreamMeta {
                        is_streaming: true,
                        tick: app.tick_count,
                        token_count: tokens,
                        model_label: Some(app.current_model_label()),
                        started_at: app.stream_started_at,
                    })
                } else {
                    None
                };
                let paragraph = if collapsed {
                    message::render_message_compact(msg, &app.theme)
                } else {
                    message::render_message_with_stream(
                        msg,
                        inner_area.width as usize,
                        &app.theme,
                        stream_meta.as_ref(),
                    )
                };
                f.render_widget(paragraph, msg_area);
            }
            TranscriptItem::ToolRuns(runs) => {
                let paragraph = render_tool_runs_message(runs, app);
                f.render_widget(paragraph, msg_area);
            }
        };

        current_y += msg_height;
    }
}

/// 渲染输入区域（Reasonix 风格：› prompt + placeholder，无边框）
pub fn render_input_area(f: &mut Frame, app: &TuiApp, area: Rect) {
    let inner_area = Rect {
        x: area.x + 2,
        y: area.y,
        width: area.width.saturating_sub(4),
        height: area.height,
    };

    let input_text = app.input.value();

    let prompt_color = match app.agent_mode {
        crate::engine::agent_mode::AgentMode::Auto
        | crate::engine::agent_mode::AgentMode::Build => app.theme.tokens.tone.brand,
        crate::engine::agent_mode::AgentMode::Plan
        | crate::engine::agent_mode::AgentMode::Explore => app.theme.tokens.tone.accent,
        crate::engine::agent_mode::AgentMode::Review => app.theme.tokens.tone.info,
    };

    let (display_text, style) = if app.is_querying {
        let mut lines = vec![Line::from(vec![
            Span::styled("› ", Style::default().fg(app.theme.tokens.fg.faint)),
            Span::styled(
                "Thinking...",
                Style::default().fg(app.theme.tokens.tone.warn),
            ),
        ])];
        if let Some(ref err) = app.error_message {
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(err.clone(), Style::default().fg(app.theme.tokens.tone.err)),
            ]));
        }
        (Text::from(lines), Style::default())
    } else if app.mode == crate::tui::app::AppMode::VimNormal {
        let text = Text::from(vec![Line::from(vec![
            Span::styled("› ", Style::default().fg(app.theme.tokens.fg.faint)),
            Span::styled(
                "Vim Normal: j/k scroll, i insert, : command, Ctrl+V toggle",
                Style::default().fg(app.theme.tokens.fg.faint),
            ),
        ])]);
        (text, Style::default())
    } else if input_text.is_empty() {
        let text = Text::from(vec![Line::from(vec![
            Span::styled("› ", Style::default().fg(prompt_color)),
            Span::styled(
                "Message Priority Agent...",
                Style::default().fg(app.theme.tokens.fg.faint),
            ),
        ])]);
        (text, Style::default())
    } else {
        let mut lines: Vec<Line> = input_text
            .lines()
            .enumerate()
            .map(|(i, line)| {
                if i == 0 {
                    Line::from(vec![
                        Span::styled("› ", Style::default().fg(prompt_color)),
                        Span::styled(
                            line.to_string(),
                            Style::default().fg(app.theme.tokens.fg.body),
                        ),
                    ])
                } else {
                    Line::from(vec![
                        Span::styled("  ", Style::default()),
                        Span::styled(
                            line.to_string(),
                            Style::default().fg(app.theme.tokens.fg.body),
                        ),
                    ])
                }
            })
            .collect();
        if lines.is_empty() {
            lines.push(Line::from(vec![Span::styled(
                "› ",
                Style::default().fg(prompt_color),
            )]));
        }
        (Text::from(lines), Style::default())
    };

    let paragraph = Paragraph::new(display_text).style(style);
    f.render_widget(paragraph, inner_area);

    // 设置光标位置
    if !app.is_querying {
        let (cursor_line, cursor_col) = app.input.cursor_line_column();
        let cursor_x = inner_area.x + cursor_col as u16 + 2; // +2 for "> " prefix on first line
        let cursor_y = inner_area.y + cursor_line as u16;
        // Only add prefix offset on first line
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

/// 渲染实时活动行（Reasonix 风格：显示当前运行的 tool 或 thinking 状态）
pub fn render_live_activity_row(f: &mut Frame, app: &TuiApp, area: Rect) {
    if !app.is_querying {
        return;
    }
    let runtime = app.runtime_status_snapshot_now();
    let tool_label = runtime.current_tool_label.as_deref().unwrap_or("Thinking");

    let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let ch = frames[app.tick_count % frames.len()];

    let mut spans = vec![
        Span::styled(
            format!("{} ", ch),
            Style::default().fg(app.theme.tokens.tone.brand),
        ),
        Span::styled(
            tool_label,
            Style::default()
                .fg(app.theme.tokens.tone.brand)
                .add_modifier(Modifier::BOLD),
        ),
    ];

    // Elapsed time for active tool
    if let Some(tool) = app
        .runtime_state_snapshot
        .tool_uses
        .iter()
        .rev()
        .find(|t| t.active)
    {
        let elapsed = tool.elapsed_ms.unwrap_or_default() / 1000;
        spans.push(Span::styled(
            format!(" · {}s", elapsed),
            Style::default().fg(app.theme.tokens.fg.faint),
        ));
    }

    // Token rate
    if let Some(usage) = app.stream_usage_snapshot {
        let tps = if usage.completion_tokens > 0 {
            Some(format!("{} tok", usage.completion_tokens))
        } else {
            None
        };
        if let Some(tps_str) = tps {
            spans.push(Span::styled(
                format!(" · {}", tps_str),
                Style::default().fg(app.theme.tokens.fg.faint),
            ));
        }
    }

    // Active tool count
    if runtime.active_tool_count > 1 {
        spans.push(Span::styled(
            format!(" · {} tools", runtime.active_tool_count),
            Style::default().fg(app.theme.tokens.fg.faint),
        ));
    }

    // Memory recall indicator
    if app.memory_use {
        spans.push(Span::styled(
            " · mem",
            Style::default().fg(app.theme.tokens.tone.info),
        ));
    }

    let line = Line::from(spans);
    f.render_widget(
        Paragraph::new(line).style(Style::default().bg(app.theme.tokens.surface.bg_elev)),
        area,
    );
}

/// 渲染 Toast 通知（Reasonix 风格：底部自动消失）
pub fn render_toasts(f: &mut Frame, app: &TuiApp, area: Rect) {
    if app.toasts.is_empty() {
        return;
    }
    let mut lines: Vec<Line> = Vec::new();
    for toast in &app.toasts {
        let remaining = toast.expires_at_tick.saturating_sub(app.tick_count);
        let fade = if remaining < 20 {
            // last 5s fading
            Style::default().fg(app.theme.tokens.fg.faint)
        } else {
            Style::default().fg(toast.color)
        };
        lines.push(Line::from(vec![
            Span::styled(toast.glyph, fade.add_modifier(Modifier::BOLD)),
            Span::styled(" ", Style::default()),
            Span::styled(&toast.message, fade),
        ]));
    }
    let n = lines.len() as u16;
    f.render_widget(
        Paragraph::new(Text::from(lines)),
        Rect {
            x: area.x + 2,
            y: area.y + area.height.saturating_sub(n),
            width: area.width.saturating_sub(4),
            height: n,
        },
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TranscriptWindow {
    start: usize,
    message_height: usize,
    more_above: bool,
    bottom_anchored: bool,
}

#[derive(Debug, Clone, Copy)]
enum TranscriptItem<'a> {
    Message {
        message_index: usize,
        msg: &'a MessageItem,
    },
    ToolRuns(&'a [ToolRunView]),
}

fn transcript_items<'a>(messages: &[&'a MessageItem], app: &'a TuiApp) -> Vec<TranscriptItem<'a>> {
    let tool_group_count = if app.focus_mode {
        0
    } else {
        messages
            .iter()
            .filter(|msg| {
                msg.role == MessageRole::User && app.tool_runs_for_message(&msg.id).is_some()
            })
            .count()
    };
    let mut items = Vec::with_capacity(messages.len() + tool_group_count);

    for (idx, msg) in messages.iter().enumerate() {
        items.push(TranscriptItem::Message {
            message_index: idx,
            msg,
        });
        if !app.focus_mode && msg.role == MessageRole::User {
            if let Some(runs) = app.tool_runs_for_message(&msg.id) {
                items.push(TranscriptItem::ToolRuns(runs));
            }
        }
    }

    items
}

fn transcript_window(
    items: &[TranscriptItem<'_>],
    scroll_offset: usize,
    bottom_anchored: bool,
    viewport_height: u16,
    width: usize,
    app: &TuiApp,
) -> TranscriptWindow {
    if items.is_empty() || viewport_height == 0 {
        return TranscriptWindow {
            start: 0,
            message_height: 0,
            more_above: false,
            bottom_anchored,
        };
    }

    if !bottom_anchored {
        let start = scroll_offset.min(items.len().saturating_sub(1));
        let more_above = start > 0;
        let max_height = (viewport_height as usize).saturating_sub(usize::from(more_above));
        return TranscriptWindow {
            start,
            message_height: visible_items_height(items, start, width, app, max_height),
            more_above,
            bottom_anchored: false,
        };
    }

    let heights = item_heights(items, width, app);
    let viewport = viewport_height as usize;
    let active_start = active_turn_start(items).unwrap_or(items.len().saturating_sub(1));
    let active_height = sum_heights(&heights, active_start, items.len());

    let mut start = active_start;
    let mut used_height = active_height;

    if active_height > viewport {
        // Keep the active turn anchored at the user's prompt. When the assistant
        // answer becomes taller than the viewport, reverse-filling from the last
        // item hides the prompt/tool context and makes the screen look like it
        // jumped to the top of the answer. Claude/Codex-style CLIs preserve the
        // turn start and let the lower part of a long answer scroll out instead.
        start =
            previous_turn_start_that_fits(&heights, active_start, viewport).unwrap_or(active_start);
        let more_above = start > 0;
        let max_height = viewport.saturating_sub(usize::from(more_above));
        used_height = visible_items_height(items, start, width, app, max_height);
    } else {
        while start > 0 {
            let candidate_start = start - 1;
            let candidate_height = heights[candidate_start];
            let more_above = candidate_start > 0;
            if used_height + candidate_height + usize::from(more_above) > viewport {
                break;
            }
            start = candidate_start;
            used_height += candidate_height;
        }
    }

    let more_above = start > 0;
    let max_height = viewport.saturating_sub(usize::from(more_above));
    TranscriptWindow {
        start,
        message_height: used_height.min(max_height),
        more_above,
        bottom_anchored: true,
    }
}

fn active_turn_start(items: &[TranscriptItem<'_>]) -> Option<usize> {
    items.iter().rposition(
        |item| matches!(item, TranscriptItem::Message { msg, .. } if msg.role == MessageRole::User),
    )
}

fn previous_turn_start_that_fits(
    heights: &[usize],
    active_start: usize,
    viewport: usize,
) -> Option<usize> {
    if active_start == 0 || viewport < 8 {
        return None;
    }

    let context_budget = (viewport / 3).clamp(4, 8);
    let previous_start = active_start.saturating_sub(2);
    if previous_start == active_start {
        return None;
    }

    let previous_height = sum_heights(heights, previous_start, active_start);
    if previous_height <= context_budget {
        Some(previous_start)
    } else {
        None
    }
}

fn item_heights(items: &[TranscriptItem<'_>], width: usize, app: &TuiApp) -> Vec<usize> {
    items
        .iter()
        .map(|item| estimate_transcript_item_height(item, width, app))
        .collect()
}

fn sum_heights(heights: &[usize], start: usize, end: usize) -> usize {
    heights
        .get(start..end.min(heights.len()))
        .unwrap_or_default()
        .iter()
        .sum()
}

fn visible_items_height(
    items: &[TranscriptItem<'_>],
    start: usize,
    width: usize,
    app: &TuiApp,
    max_height: usize,
) -> usize {
    item_heights(items, width, app)
        .into_iter()
        .skip(start)
        .sum::<usize>()
        .min(max_height)
}

fn estimate_transcript_item_height(item: &TranscriptItem<'_>, width: usize, app: &TuiApp) -> usize {
    match item {
        TranscriptItem::Message { message_index, msg } => {
            let collapsed = app.collapsed_indices.contains(message_index);
            estimate_message_height(msg, width, &app.theme, collapsed)
        }
        TranscriptItem::ToolRuns(runs) => estimate_tool_runs_height(runs, app),
    }
}

fn estimate_tool_runs_height(runs: &[ToolRunView], app: &TuiApp) -> usize {
    let lines = runs
        .iter()
        .map(|run| run.render_lines(app.is_tool_run_expanded(run)).len())
        .sum::<usize>();
    lines.max(1) + 1
}

fn render_tool_runs_message<'a>(runs: &'a [ToolRunView], app: &'a TuiApp) -> Paragraph<'a> {
    let mut lines = Vec::new();
    for (run_idx, run) in runs.iter().enumerate() {
        let expanded = app.is_tool_run_expanded(run);
        let accent = match run.status {
            ToolRunStatus::Queued | ToolRunStatus::Running => app.theme.tokens.tone.warn,
            ToolRunStatus::WaitingPermission => app.theme.tokens.tone.warn,
            ToolRunStatus::Backgrounded | ToolRunStatus::Completed => app.theme.tokens.fg.faint,
            ToolRunStatus::Cancelled => app.theme.tokens.tone.warn,
            ToolRunStatus::TimedOut | ToolRunStatus::Failed => app.theme.tokens.tone.err,
        };
        for (line_idx, line) in run.render_lines(expanded).into_iter().enumerate() {
            let prefix = if line_idx == 0 {
                match run.status {
                    ToolRunStatus::Queued | ToolRunStatus::Running => "● ",
                    ToolRunStatus::WaitingPermission => "? ",
                    ToolRunStatus::Backgrounded => "↪ ",
                    ToolRunStatus::Completed => "✓ ",
                    ToolRunStatus::Cancelled => "× ",
                    ToolRunStatus::TimedOut | ToolRunStatus::Failed => "✗ ",
                }
            } else {
                "  "
            };
            let style = if line_idx == 0 {
                Style::default().fg(accent)
            } else if expanded
                && (matches!(line.trim_start().chars().next(), Some('{' | '}' | '"'))
                    || line.contains("result:"))
            {
                Style::default().fg(app.theme.tokens.fg.body)
            } else {
                Style::default().fg(app.theme.tokens.fg.faint)
            };
            lines.push(Line::from(vec![
                Span::styled(prefix, Style::default().fg(accent)),
                Span::styled(line, style),
            ]));
        }
        if run_idx + 1 < runs.len() {
            lines.push(Line::from(""));
        }
    }

    Paragraph::new(Text::from(lines)).wrap(Wrap { trim: true })
}

pub fn render_tool_viewer(f: &mut Frame, app: &TuiApp, area: Rect) {
    let popup_area = centered_rect(90, 85, area);
    let block = Block::default()
        .title(format!(" Tool Output: {} ", app.tool_viewer_title))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.tokens.tone.brand))
        .style(Style::default().bg(app.theme.tokens.surface.bg_elev));

    let mut lines = app
        .tool_viewer_content
        .lines()
        .map(|raw| {
            let style = tool_viewer_line_style(raw, app);
            Line::from(Span::styled(raw.to_string(), style))
        })
        .collect::<Vec<_>>();

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "No tool output.",
            Style::default().fg(app.theme.tokens.fg.faint),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("esc/q", Style::default().fg(app.theme.tokens.tone.info)),
        Span::styled(" close  ", Style::default().fg(app.theme.tokens.fg.faint)),
        Span::styled("↑/↓", Style::default().fg(app.theme.tokens.tone.info)),
        Span::styled(" scroll  ", Style::default().fg(app.theme.tokens.fg.faint)),
        Span::styled("PgUp/PgDn", Style::default().fg(app.theme.tokens.tone.info)),
        Span::styled(" page", Style::default().fg(app.theme.tokens.fg.faint)),
    ]));

    let total_lines = lines.len().saturating_sub(1) as u16;
    let scroll = app.tool_viewer_scroll_offset.min(total_lines);
    let paragraph = Paragraph::new(Text::from(lines))
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0))
        .block(block);

    f.render_widget(Clear, popup_area);
    f.render_widget(paragraph, popup_area);
}

fn tool_viewer_line_style(raw: &str, app: &TuiApp) -> Style {
    let trimmed = raw.trim_start();
    if raw.starts_with("Tool:")
        || raw.starts_with("Status:")
        || raw.starts_with("Elapsed:")
        || raw.ends_with(':')
    {
        Style::default()
            .fg(app.theme.tokens.fg.strong)
            .add_modifier(Modifier::BOLD)
    } else if trimmed.starts_with("ERROR")
        || trimmed.starts_with("Error")
        || trimmed.starts_with("error")
        || trimmed.contains("panicked")
        || trimmed.contains("failed")
    {
        Style::default().fg(app.theme.tokens.tone.err)
    } else if trimmed.starts_with('+') && !trimmed.starts_with("+++") {
        Style::default().fg(app.theme.tokens.tone.ok)
    } else if trimmed.starts_with('-') && !trimmed.starts_with("---") {
        Style::default().fg(app.theme.tokens.tone.err)
    } else if trimmed.starts_with("@@") || trimmed.starts_with("diff --git") {
        Style::default()
            .fg(app.theme.tokens.fg.faint)
            .add_modifier(Modifier::BOLD)
    } else if matches!(trimmed.chars().next(), Some('{' | '}' | '[' | ']' | '"')) {
        Style::default().fg(app.theme.tokens.tone.info)
    } else if raw.starts_with("- ") {
        Style::default().fg(app.theme.tokens.fg.faint)
    } else {
        Style::default().fg(app.theme.tokens.fg.body)
    }
}

/// 渲染会话侧边栏（带搜索、pin、元数据、面板切换）
pub fn render_sidebar(f: &mut Frame, app: &TuiApp, area: Rect) {
    match app.sidebar_panel {
        SidebarPanel::Sessions => render_sessions_panel(f, app, area),
        SidebarPanel::Context => render_context_panel(f, app, area),
    }
}

fn render_sessions_panel(f: &mut Frame, app: &TuiApp, area: Rect) {
    use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.tokens.tone.brand))
        .title(format!(
            " Sessions {} ",
            if app.sidebar_filter.is_empty() {
                String::new()
            } else {
                format!("(filter: {})", app.sidebar_filter)
            }
        ))
        .style(Style::default().bg(app.theme.tokens.surface.bg));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // 搜索栏
    let filter_text = if app.sidebar_filter.is_empty() {
        "/ to filter"
    } else {
        &app.sidebar_filter
    };

    let search_chunks = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            ratatui::layout::Constraint::Length(1),
            ratatui::layout::Constraint::Min(1),
        ])
        .split(inner);

    let search_para = Paragraph::new(Line::from(Span::styled(
        format!(" {filter_text} "),
        Style::default()
            .fg(if app.sidebar_filter.is_empty() {
                app.theme.tokens.fg.faint
            } else {
                app.theme.tokens.fg.body
            })
            .bg(app.theme.tokens.surface.bg_elev),
    )));
    f.render_widget(search_para, search_chunks[0]);

    // 获取会话列表
    let all_sessions = app.session_manager.list_sessions(50).unwrap_or_default();

    // 分离已固定和未固定的会话
    let current_id = app.session_manager.current_session_id().unwrap_or("");
    let mut pinned: Vec<&crate::session_store::SessionRecord> = Vec::new();
    let mut unpinned: Vec<&crate::session_store::SessionRecord> = Vec::new();

    for s in &all_sessions {
        if app.pinned_sessions.contains(&s.id) {
            pinned.push(s);
        } else {
            unpinned.push(s);
        }
    }

    // 应用搜索筛选
    if !app.sidebar_filter.is_empty() {
        let filter = app.sidebar_filter.to_lowercase();
        pinned.retain(|s| s.title.to_lowercase().contains(&filter));
        unpinned.retain(|s| s.title.to_lowercase().contains(&filter));
    }

    let mut items: Vec<ListItem> = Vec::new();

    // 已固定分组
    if !pinned.is_empty() {
        items.push(ListItem::new(Line::from(Span::styled(
            "─ Pinned ─".to_string(),
            Style::default()
                .fg(app.theme.tokens.tone.accent)
                .add_modifier(Modifier::BOLD),
        ))));
        for (i, session) in pinned.iter().enumerate() {
            items.push(build_session_item(
                app,
                session,
                current_id,
                i,
                true,
                pinned.len(),
            ));
        }
    }

    // 未固定分组
    if !pinned.is_empty() && !unpinned.is_empty() {
        items.push(ListItem::new(Line::from("")));
    }

    let pinned_count = pinned.len();
    for (i, session) in unpinned.iter().enumerate() {
        items.push(build_session_item(
            app,
            session,
            current_id,
            pinned_count + i,
            false,
            unpinned.len(),
        ));
    }

    if items.is_empty() {
        items.push(ListItem::new(Line::from(Span::styled(
            "No sessions found",
            Style::default().fg(app.theme.tokens.fg.faint),
        ))));
    }

    let list = List::new(items);
    f.render_widget(list, search_chunks[1]);
}

fn render_context_panel(f: &mut Frame, app: &TuiApp, area: Rect) {
    use ratatui::widgets::{Block, Borders, Paragraph};

    let panel_label = app.sidebar_panel.label();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.tokens.tone.info))
        .title(format!(" {} (Ctrl+Tab) ", panel_label))
        .style(Style::default().bg(app.theme.tokens.surface.bg));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let msgs = app.messages.len();

    let text =
        format!("Active session\nMessages: {msgs}\n\nCtrl+Tab: Sessions\nb: toggle sidebar",);

    f.render_widget(
        Paragraph::new(text)
            .style(Style::default().fg(app.theme.tokens.fg.body))
            .wrap(ratatui::widgets::Wrap { trim: true }),
        inner,
    );
}

fn build_session_item<'a>(
    app: &'a TuiApp,
    session: &crate::session_store::SessionRecord,
    current_id: &str,
    index: usize,
    is_pinned: bool,
    _total: usize,
) -> ratatui::widgets::ListItem<'a> {
    let is_current = session.id == current_id;
    let is_selected = index == app.sidebar_selected;

    let base = if is_current {
        Style::default()
            .fg(app.theme.tokens.tone.ok)
            .add_modifier(Modifier::BOLD)
    } else if is_selected {
        Style::default()
            .fg(app.theme.tokens.fg.strong)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(app.theme.tokens.fg.body)
    };

    let prefix = if is_current {
        "●"
    } else if is_pinned {
        "◆"
    } else {
        "○"
    };

    let title = if session.title.is_empty() {
        format!("Session {}", &session.id[..8.min(session.id.len())])
    } else {
        session.title.clone()
    };

    // 截断标题以适应侧边栏
    let max_title = 18usize;
    let display_title = if title.len() > max_title {
        format!("{}…", &title[..max_title])
    } else {
        title
    };

    // 元数据：模型 + 消息数
    let model_short: String = session
        .model
        .split('-')
        .next()
        .unwrap_or(&session.model)
        .chars()
        .take(6)
        .collect();
    let msg_count = app.session_manager.message_count(&session.id).unwrap_or(0);

    let meta = format!(" {:>6}  {:>3} msgs", model_short, msg_count);

    let line = Line::from(vec![
        Span::styled(format!("{} ", prefix), base),
        Span::styled(display_title, base),
        Span::styled(meta, Style::default().fg(app.theme.tokens.fg.faint)),
    ]);

    ratatui::widgets::ListItem::new(line)
}

/// 渲染消息搜索弹窗
pub fn render_message_search(f: &mut Frame, app: &TuiApp, area: Rect) {
    use ratatui::widgets::{Block, Borders, Clear, Paragraph};

    let search_height = (area.height / 2).clamp(10, 20);
    let popup_area = centered_rect(80, search_height, area);

    f.render_widget(Clear, popup_area);

    let search = &app.message_search_state;
    let title = search.status_text();

    let block = Block::default()
        .title(format!(" {} ", title))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.tokens.tone.brand))
        .style(Style::default().bg(app.theme.tokens.surface.bg_elev));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    if search.results.is_empty() {
        let hint = if search.query.is_empty() {
            "Type to search messages..."
        } else {
            "No matches found"
        };
        let text = Paragraph::new(hint)
            .style(Style::default().fg(app.theme.tokens.fg.faint))
            .alignment(Alignment::Center);
        f.render_widget(text, inner);
    } else {
        let results_list = search.render_results();
        let mut state = search.list_state.clone();
        f.render_stateful_widget(results_list, inner, &mut state);
    }

    // 底部提示
    let hint_area = Rect {
        x: popup_area.x,
        y: popup_area.y + popup_area.height - 1,
        width: popup_area.width,
        height: 1,
    };
    let hint = Paragraph::new("Esc: close | Enter: jump | ↑/↓: navigate | n: toggle case")
        .style(Style::default().fg(app.theme.tokens.fg.faint))
        .alignment(Alignment::Center);
    f.render_widget(hint, hint_area);
}

/// 估算消息高度
fn estimate_message_height(
    msg: &crate::state::MessageItem,
    width: usize,
    _theme: &crate::tui::theme::Theme,
    collapsed: bool,
) -> usize {
    if collapsed {
        return 2; // header + "..." indicator
    }
    let base_height = 1; // minimal spacing between messages
    let effective_width = width.saturating_sub(4).max(1);

    let mut lines = 0;
    let mut in_code_block = false;
    let mut last_was_text = false;

    for raw_line in msg.content.lines() {
        let trimmed = raw_line.trim();

        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            lines += 1; // fence line
            last_was_text = false;
        } else if in_code_block {
            lines += 1;
            last_was_text = false;
        } else if trimmed.is_empty() {
            if last_was_text {
                lines += 1; // paragraph separator (parse_markdown adds this)
            }
            last_was_text = false;
        } else if trimmed.starts_with('#') {
            lines += 1;
            last_was_text = true;
        } else if trimmed == "---" || trimmed == "***" || trimmed == "___" {
            lines += 2; // horizontal rule + blank line
            last_was_text = false;
        } else {
            let dw = unicode_width::UnicodeWidthStr::width(raw_line);
            lines += dw.div_ceil(effective_width).max(1);
            last_was_text = true;
        }
    }

    // No trailing blank line (compact Claude Code style)

    base_height + lines.max(1)
}

#[cfg(test)]
mod tests;

/// 计算居中矩形
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
