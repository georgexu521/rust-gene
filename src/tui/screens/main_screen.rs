//! 主屏幕
//!
//! 包含聊天区、输入区、状态栏的渲染

use crate::{
    state::{MessageItem, MessageRole},
    tui::{
        app::{StatusBarDensity, TuiApp},
        components::message,
        tool_view::{ToolRunStatus, ToolRunView},
    },
};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

mod approvals;
pub use approvals::*;

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

/// 渲染状态栏（Reasonix 风格：mode glyph · session · cost · cache · ctx）
pub fn render_status_bar(f: &mut Frame, app: &TuiApp, area: Rect) {
    let mut parts = Vec::new();
    let runtime = app.runtime_status_snapshot_now();

    // Mode glyph (Reasonix style)
    let mode_glyph = match app.agent_mode {
        crate::engine::agent_mode::AgentMode::Auto => "●",
        crate::engine::agent_mode::AgentMode::Build => "●",
        crate::engine::agent_mode::AgentMode::Plan => "⊞",
        crate::engine::agent_mode::AgentMode::Explore => "⊙",
        crate::engine::agent_mode::AgentMode::Review => "◐",
    };
    let mode_color = match app.agent_mode {
        crate::engine::agent_mode::AgentMode::Auto
        | crate::engine::agent_mode::AgentMode::Build => app.theme.tokens.tone.ok,
        crate::engine::agent_mode::AgentMode::Plan
        | crate::engine::agent_mode::AgentMode::Explore => app.theme.tokens.tone.accent,
        crate::engine::agent_mode::AgentMode::Review => app.theme.tokens.tone.warn,
    };

    // 左侧：mode glyph + 状态
    if runtime.is_querying {
        let label = runtime.current_tool_label.as_deref().unwrap_or("Thinking");
        parts.push(Span::styled(
            format!("◌ {}", label),
            Style::default().fg(app.theme.tokens.tone.warn),
        ));
        if let Some(usage) = app.stream_usage_label() {
            parts.push(Span::styled(
                usage,
                Style::default().fg(app.theme.tokens.fg.faint),
            ));
        }
        parts.push(Span::styled(
            "esc to interrupt",
            Style::default().fg(app.theme.tokens.fg.faint),
        ));
    } else if let Some(ref error) = runtime.last_error {
        parts.push(Span::styled(
            format!("✗ {}", error),
            Style::default().fg(app.theme.tokens.tone.err),
        ));
    } else {
        parts.push(Span::styled(
            format!("{} {}", mode_glyph, app.current_agent_mode_label()),
            Style::default().fg(mode_color),
        ));
    }

    // Session info
    if let Some(session_id) = app.session_manager.current_session_id() {
        let short_id = if session_id.len() > 8 {
            &session_id[..8]
        } else {
            session_id
        };
        parts.push(Span::styled(
            short_id.to_string(),
            Style::default().fg(app.theme.tokens.fg.faint),
        ));
    }

    // Permission
    parts.push(Span::styled(
        app.current_permission_label(),
        Style::default().fg(app.theme.tokens.tone.warn),
    ));

    // Provider / Model
    parts.push(Span::styled(
        format!(
            "{} / {}",
            app.current_provider_label(),
            app.current_model_label()
        ),
        Style::default().fg(app.theme.tokens.fg.faint),
    ));

    // Context usage bar (Reasonix style: 8 cells)
    if let Some(usage) = app.stream_usage_snapshot {
        let cap: u32 = 128_000; // rough context cap, configurable
        let used = usage.prompt_tokens;
        let ratio = (used as f64 / cap as f64).min(1.0);
        let pct = (ratio * 100.0) as u32;
        let bar_color = if ratio >= 0.8 {
            app.theme.tokens.tone.err
        } else if ratio >= 0.5 {
            app.theme.tokens.tone.warn
        } else {
            app.theme.tokens.tone.ok
        };
        let filled = (ratio * 8.0).round() as usize;
        let empty = 8 - filled;
        let bar = "█".repeat(filled) + &"░".repeat(empty);
        parts.push(Span::styled(
            format!("ctx {bar} {pct}%"),
            Style::default().fg(bar_color),
        ));

        // Cache hit %
        if usage.cached_tokens.unwrap_or(0) > 0 && usage.prompt_tokens > 0 {
            let hit_pct = (usage.cached_tokens.unwrap_or(0) as f64 / usage.prompt_tokens as f64
                * 100.0) as u32;
            parts.push(Span::styled(
                format!("cache {}%", hit_pct),
                Style::default().fg(app.theme.tokens.tone.accent),
            ));
        }
    }

    // Turn cost (from last usage)
    if !app.is_querying {
        if let Some(usage) = app.stream_usage_label() {
            parts.push(Span::styled(
                format!("last {}", usage),
                Style::default().fg(app.theme.tokens.fg.faint),
            ));
        }
    }

    if runtime.mcp_server_count > 0 {
        parts.push(Span::styled(
            format!(
                "mcp:{}/{}",
                runtime.mcp_available_count, runtime.mcp_server_count
            ),
            Style::default().fg(app.theme.tokens.fg.faint),
        ));
    }

    if app.vim_mode {
        parts.push(Span::styled(
            "vim",
            Style::default().fg(app.theme.tokens.tone.violet),
        ));
    }

    // Memory mode
    if app.memory_use {
        let recall_label = match app.memory_recall_mode.as_str() {
            "off" => "mem:off",
            "strict" => "mem:strict",
            "balanced" => "mem:bal",
            "preference-only" => "mem:pref",
            _ => "mem",
        };
        parts.push(Span::styled(
            recall_label,
            Style::default().fg(app.theme.tokens.tone.info),
        ));
    }

    // Debug extras
    if app.status_bar_density == StatusBarDensity::Debug {
        parts.push(Span::styled(
            format!("scroll:{}", app.scroll_offset),
            Style::default().fg(app.theme.tokens.fg.faint),
        ));
        parts.push(Span::styled(
            format!(
                "tools:{}/{}",
                runtime.active_tool_count, runtime.total_tools
            ),
            Style::default().fg(app.theme.tokens.fg.faint),
        ));
        parts.push(Span::styled(
            format!("msgs:{}", runtime.messages),
            Style::default().fg(app.theme.tokens.fg.faint),
        ));
    }
    parts.push(Span::styled(
        format!("v{}", env!("CARGO_PKG_VERSION")),
        Style::default().fg(app.theme.tokens.fg.faint),
    ));
    parts.push(Span::styled(
        "? shortcuts",
        Style::default().fg(app.theme.tokens.fg.faint),
    ));

    // 用 " · " 连接所有部分
    let mut spans = Vec::new();
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(
                " · ",
                Style::default().fg(app.theme.tokens.fg.faint),
            ));
        }
        spans.push(part.clone());
    }

    f.render_widget(
        Paragraph::new(Line::from(spans)).alignment(Alignment::Left),
        area,
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

/// 渲染会话侧边栏
pub fn render_sidebar(f: &mut Frame, app: &TuiApp, area: Rect) {
    use ratatui::widgets::{Block, Borders, List, ListItem};

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.tokens.tone.brand))
        .title(" Sessions ")
        .style(Style::default().bg(app.theme.tokens.surface.bg));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let sessions = app.session_manager.list_sessions(20).unwrap_or_default();

    let current_id = app.session_manager.current_session_id().unwrap_or("");

    let items: Vec<ListItem> = sessions
        .iter()
        .enumerate()
        .map(|(i, session)| {
            let is_current = session.id == current_id;
            let is_selected = i == app.sidebar_selected;

            let title = if session.title.is_empty() {
                format!("Session {}", &session.id[..8.min(session.id.len())])
            } else {
                session.title.clone()
            };

            let style = if is_current {
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

            let prefix = if is_current { "● " } else { "○ " };
            ListItem::new(Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(title, style),
            ]))
        })
        .collect();

    let list = List::new(items);
    f.render_widget(list, inner);
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

/// 渲染帮助弹窗
pub fn render_help_popup(f: &mut Frame, area: Rect) {
    let popup_block = Block::default()
        .title(" Help ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue))
        .style(Style::default().bg(Color::Black));

    let help_text = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "Navigation:",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from("  ↑/↓     - Scroll messages"),
        Line::from("  Ctrl+C  - Exit application"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Input:",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from("  Enter       - Send message"),
        Line::from("  Backspace   - Delete character"),
        Line::from("  ←/→         - Move cursor"),
        Line::from("  Home/End    - Start/End of line"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Press any key to close...",
            Style::default().fg(Color::Gray),
        )]),
    ];

    let popup_area = centered_rect(60, 60, area);
    let help_paragraph = Paragraph::new(Text::from(help_text)).wrap(Wrap { trim: true });

    f.render_widget(Clear, popup_area);
    f.render_widget(help_paragraph.block(popup_block), popup_area);
}

pub fn render_command_palette(f: &mut Frame, app: &TuiApp, area: Rect) {
    let popup_area = centered_rect(74, 64, area);
    let block = Block::default()
        .title(" Command Palette ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.tokens.tone.info))
        .style(Style::default().bg(Color::Black));

    let mut lines = vec![
        Line::from(vec![
            Span::styled("Search: ", Style::default().fg(app.theme.tokens.fg.faint)),
            Span::styled(
                if app.command_palette_query.is_empty() {
                    "type to filter commands"
                } else {
                    app.command_palette_query.as_str()
                },
                if app.command_palette_query.is_empty() {
                    Style::default().fg(app.theme.tokens.fg.faint)
                } else {
                    Style::default().fg(app.theme.tokens.fg.body)
                },
            ),
        ]),
        Line::from(""),
    ];

    let items = app.command_palette_items();
    if items.is_empty() {
        let empty_message = if app.command_palette_query.is_empty() {
            "No commands registered.".to_string()
        } else {
            format!("No command matched '{}'.", app.command_palette_query)
        };
        lines.push(Line::from(Span::styled(
            empty_message,
            Style::default().fg(app.theme.tokens.fg.faint),
        )));
        lines.push(Line::from(Span::styled(
            "Try a command name, category, alias, or description.",
            Style::default().fg(app.theme.tokens.fg.faint),
        )));
    } else {
        let mut last_category = "";
        for (idx, cmd) in items.iter().enumerate() {
            let display_category = if app.command_palette_query.is_empty() {
                if app.is_contextual_palette_command(cmd.name) {
                    "Context"
                } else if crate::tui::commands::is_suggested_command(cmd.name) {
                    "Suggested"
                } else {
                    cmd.category
                }
            } else {
                cmd.category
            };
            if display_category != last_category {
                if idx > 0 {
                    lines.push(Line::from(""));
                }
                lines.push(Line::from(Span::styled(
                    display_category,
                    Style::default()
                        .fg(app.theme.tokens.fg.strong)
                        .add_modifier(Modifier::BOLD),
                )));
                last_category = display_category;
            }

            let selected = idx == app.command_palette_selected;
            let marker = if selected { "› " } else { "  " };
            let style = if selected {
                Style::default()
                    .fg(app.theme.tokens.fg.body)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(app.theme.tokens.fg.faint)
            };
            let alias = if cmd.aliases.is_empty() {
                String::new()
            } else {
                format!(" ({})", cmd.aliases.join(", "))
            };
            let maturity = if cmd.maturity == crate::tui::commands::CommandMaturity::Production {
                String::new()
            } else {
                format!(" {}", cmd.maturity.badge())
            };
            lines.push(Line::from(vec![
                Span::styled(marker, Style::default().fg(app.theme.tokens.tone.info)),
                Span::styled(format!("{:<18}", cmd.name), style),
                Span::styled(
                    cmd.description,
                    Style::default().fg(app.theme.tokens.fg.faint),
                ),
                Span::styled(alias, Style::default().fg(app.theme.tokens.fg.faint)),
                Span::styled(maturity, Style::default().fg(app.theme.tokens.tone.warn)),
            ]));
        }
    }

    if let Some(selected) = items.get(app.command_palette_selected) {
        let action = match crate::tui::commands::command_accept_behavior(selected) {
            crate::tui::commands::CommandAcceptBehavior::Execute => "execute now",
            crate::tui::commands::CommandAcceptBehavior::Insert => {
                "insert command; add arguments, then press Enter"
            }
        };
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("Action:", Style::default().fg(app.theme.tokens.fg.faint)),
            Span::styled(
                format!(" {}", action),
                Style::default().fg(app.theme.tokens.tone.info),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Usage: ", Style::default().fg(app.theme.tokens.fg.faint)),
            Span::styled(
                selected.usage,
                Style::default()
                    .fg(app.theme.tokens.fg.body)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Info:  ", Style::default().fg(app.theme.tokens.fg.faint)),
            Span::styled(
                selected.description,
                Style::default().fg(app.theme.tokens.fg.faint),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Maturity: ", Style::default().fg(app.theme.tokens.fg.faint)),
            Span::styled(
                selected.maturity.label(),
                Style::default().fg(match selected.maturity {
                    crate::tui::commands::CommandMaturity::Production => app.theme.tokens.tone.ok,
                    crate::tui::commands::CommandMaturity::Usable => app.theme.tokens.tone.warn,
                    crate::tui::commands::CommandMaturity::Placeholder => app.theme.tokens.tone.err,
                }),
            ),
        ]));
        if !selected.aliases.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("Alias: ", Style::default().fg(app.theme.tokens.fg.faint)),
                Span::styled(
                    selected.aliases.join(", "),
                    Style::default().fg(app.theme.tokens.fg.faint),
                ),
            ]));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("enter", Style::default().fg(app.theme.tokens.tone.info)),
        Span::styled(
            " execute or insert  ",
            Style::default().fg(app.theme.tokens.fg.faint),
        ),
        Span::styled("esc", Style::default().fg(app.theme.tokens.tone.info)),
        Span::styled(" close  ", Style::default().fg(app.theme.tokens.fg.faint)),
        Span::styled("ctrl+p", Style::default().fg(app.theme.tokens.tone.info)),
        Span::styled(" toggle", Style::default().fg(app.theme.tokens.fg.faint)),
    ]));

    f.render_widget(Clear, popup_area);
    f.render_widget(
        Paragraph::new(Text::from(lines))
            .wrap(Wrap { trim: true })
            .block(block),
        popup_area,
    );
}

pub fn render_shortcut_help(f: &mut Frame, app: &TuiApp, area: Rect) {
    let popup_area = centered_rect(68, 58, area);
    let block = Block::default()
        .title(" Shortcuts ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.tokens.tone.info))
        .style(Style::default().bg(Color::Black));
    let kb = &app.keybindings;
    let lines = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "Core",
            Style::default()
                .fg(app.theme.tokens.fg.strong)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(format!("  {}        send message", kb.chat_submit)),
        Line::from(format!("  {}   insert newline", kb.chat_newline)),
        Line::from("  ctrl+p       command palette"),
        Line::from("  ctrl+m       model picker"),
        Line::from("  ctrl+l       provider picker"),
        Line::from("  ctrl+o       expand/collapse tool details"),
        Line::from("  ctrl+t       open full tool output"),
        Line::from("  ctrl+shift+s cycle status bar density"),
        Line::from(format!("  {}       quit", kb.global_quit)),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Navigation",
            Style::default()
                .fg(app.theme.tokens.fg.strong)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from("  ↑/↓          move cursor or scroll at edge"),
        Line::from("  pageup/down  half-page scroll"),
        Line::from(format!("  {}       toggle vim mode", kb.toggle_vim_mode)),
        Line::from("  vim: j/k scroll, g top, G bottom, / search, b sidebar"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Approvals",
            Style::default()
                .fg(app.theme.tokens.fg.strong)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(format!(
            "  {}            allow / approve",
            kb.permission_approve
        )),
        Line::from(format!(
            "  {}            deny / reject",
            kb.permission_reject
        )),
        Line::from(format!(
            "  {}            view diff or preview",
            kb.permission_view_diff
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Press any key to close.",
            Style::default().fg(app.theme.tokens.fg.faint),
        )),
    ];

    f.render_widget(Clear, popup_area);
    f.render_widget(
        Paragraph::new(Text::from(lines))
            .wrap(Wrap { trim: true })
            .block(block),
        popup_area,
    );
}

pub fn render_model_select(f: &mut Frame, app: &TuiApp, area: Rect) {
    let popup_area = centered_rect(66, 48, area);
    let block = Block::default()
        .title(" Model ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.tokens.tone.info))
        .style(Style::default().bg(Color::Black));

    let mut lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("Provider ", Style::default().fg(app.theme.tokens.fg.faint)),
            Span::styled(
                app.current_provider_label(),
                Style::default()
                    .fg(app.theme.tokens.fg.strong)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Base URL ", Style::default().fg(app.theme.tokens.fg.faint)),
            Span::styled(
                app.current_provider_base_url(),
                Style::default().fg(app.theme.tokens.fg.faint),
            ),
        ]),
        Line::from(vec![
            Span::styled("Search  ", Style::default().fg(app.theme.tokens.fg.faint)),
            Span::styled(
                if app.model_select_query.is_empty() {
                    "type to filter models"
                } else {
                    app.model_select_query.as_str()
                },
                if app.model_select_query.is_empty() {
                    Style::default().fg(app.theme.tokens.fg.faint)
                } else {
                    Style::default().fg(app.theme.tokens.fg.body)
                },
            ),
        ]),
        Line::from(""),
    ];

    let choices = app.model_choices();
    if choices.is_empty() {
        let empty_message = if app.model_select_query.is_empty() {
            "No models available for the active provider.".to_string()
        } else {
            format!("No models matched '{}'.", app.model_select_query)
        };
        lines.push(Line::from(Span::styled(
            empty_message,
            Style::default().fg(app.theme.tokens.fg.faint),
        )));
        lines.push(Line::from(Span::styled(
            "Backspace edits search; /settings changes provider and API configuration.",
            Style::default().fg(app.theme.tokens.fg.faint),
        )));
    } else {
        for (idx, choice) in choices.iter().enumerate() {
            let selected = idx == app.model_select_selected;
            let marker = if selected { "› " } else { "  " };
            let model_style = if choice.active {
                Style::default()
                    .fg(app.theme.tokens.fg.strong)
                    .add_modifier(Modifier::BOLD)
            } else if selected {
                Style::default()
                    .fg(app.theme.tokens.fg.body)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(app.theme.tokens.fg.body)
            };
            lines.push(Line::from(vec![
                Span::styled(marker, Style::default().fg(app.theme.tokens.tone.info)),
                Span::styled(format!("{:<24}", choice.model), model_style),
                Span::styled(
                    choice.note.clone(),
                    Style::default().fg(app.theme.tokens.fg.faint),
                ),
            ]));
        }
    }

    if let Some(notice) = &app.model_notice {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            notice.clone(),
            Style::default().fg(app.theme.tokens.tone.info),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("enter", Style::default().fg(app.theme.tokens.tone.info)),
        Span::styled(
            " switch for next request  ",
            Style::default().fg(app.theme.tokens.fg.faint),
        ),
        Span::styled("esc", Style::default().fg(app.theme.tokens.tone.info)),
        Span::styled(" close  ", Style::default().fg(app.theme.tokens.fg.faint)),
        Span::styled("backspace", Style::default().fg(app.theme.tokens.tone.info)),
        Span::styled(
            " edit search  ",
            Style::default().fg(app.theme.tokens.fg.faint),
        ),
        Span::styled("/settings", Style::default().fg(app.theme.tokens.tone.info)),
        Span::styled(
            " provider/API keys",
            Style::default().fg(app.theme.tokens.fg.faint),
        ),
    ]));

    f.render_widget(Clear, popup_area);
    f.render_widget(
        Paragraph::new(Text::from(lines))
            .wrap(Wrap { trim: true })
            .block(block),
        popup_area,
    );
}

pub fn render_provider_select(f: &mut Frame, app: &TuiApp, area: Rect) {
    let popup_area = centered_rect(74, 56, area);
    let block = Block::default()
        .title(" Provider ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.tokens.tone.info))
        .style(Style::default().bg(Color::Black));

    let mut lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("Current ", Style::default().fg(app.theme.tokens.fg.faint)),
            Span::styled(
                format!(
                    "{} / {}",
                    app.current_provider_label(),
                    app.current_model_label()
                ),
                Style::default()
                    .fg(app.theme.tokens.fg.strong)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Search  ", Style::default().fg(app.theme.tokens.fg.faint)),
            Span::styled(
                if app.provider_select_query.is_empty() {
                    "type to filter providers"
                } else {
                    app.provider_select_query.as_str()
                },
                if app.provider_select_query.is_empty() {
                    Style::default().fg(app.theme.tokens.fg.faint)
                } else {
                    Style::default().fg(app.theme.tokens.fg.body)
                },
            ),
        ]),
        Line::from(""),
    ];

    let choices = app.provider_choices();
    if choices.is_empty() {
        let empty_message = if app.provider_select_query.is_empty() {
            "No providers are available.".to_string()
        } else {
            format!("No providers matched '{}'.", app.provider_select_query)
        };
        lines.push(Line::from(Span::styled(
            empty_message,
            Style::default().fg(app.theme.tokens.fg.faint),
        )));
        lines.push(Line::from(Span::styled(
            "Backspace edits search; /settings opens API key and base URL settings.",
            Style::default().fg(app.theme.tokens.fg.faint),
        )));
    } else {
        for (idx, choice) in choices.iter().enumerate() {
            let selected = idx == app.provider_select_selected;
            let marker = if selected { "› " } else { "  " };
            let style = if choice.active {
                Style::default()
                    .fg(app.theme.tokens.fg.strong)
                    .add_modifier(Modifier::BOLD)
            } else if choice.configured {
                Style::default().fg(app.theme.tokens.fg.body)
            } else {
                Style::default().fg(app.theme.tokens.fg.faint)
            };
            lines.push(Line::from(vec![
                Span::styled(marker, Style::default().fg(app.theme.tokens.tone.info)),
                Span::styled(format!("{:<10}", choice.name), style),
                Span::styled(
                    format!("{:<12}", choice.provider_type),
                    Style::default().fg(app.theme.tokens.fg.faint),
                ),
                Span::styled(format!("{:<20}", choice.model), style),
                Span::styled(
                    choice.note.clone(),
                    Style::default().fg(app.theme.tokens.fg.faint),
                ),
            ]));
            if selected && !choice.base_url.is_empty() {
                lines.push(Line::from(vec![
                    Span::styled("  └ ", Style::default().fg(app.theme.tokens.fg.faint)),
                    Span::styled(
                        choice.base_url.clone(),
                        Style::default().fg(app.theme.tokens.fg.faint),
                    ),
                ]));
            } else if selected && !choice.configured {
                lines.push(Line::from(vec![
                    Span::styled(
                        "  └ setup: ",
                        Style::default().fg(app.theme.tokens.tone.warn),
                    ),
                    Span::styled(
                        choice.note.clone(),
                        Style::default().fg(app.theme.tokens.fg.faint),
                    ),
                    Span::styled(
                        " or open /settings",
                        Style::default().fg(app.theme.tokens.fg.faint),
                    ),
                ]));
            }
        }
    }

    if let Some(notice) = &app.provider_notice {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            notice.clone(),
            Style::default().fg(app.theme.tokens.tone.info),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("enter", Style::default().fg(app.theme.tokens.tone.info)),
        Span::styled(
            " switch configured provider  ",
            Style::default().fg(app.theme.tokens.fg.faint),
        ),
        Span::styled("esc", Style::default().fg(app.theme.tokens.tone.info)),
        Span::styled(" close  ", Style::default().fg(app.theme.tokens.fg.faint)),
        Span::styled("backspace", Style::default().fg(app.theme.tokens.tone.info)),
        Span::styled(
            " edit search  ",
            Style::default().fg(app.theme.tokens.fg.faint),
        ),
        Span::styled("/settings", Style::default().fg(app.theme.tokens.tone.info)),
        Span::styled(
            " edit keys/base URL",
            Style::default().fg(app.theme.tokens.fg.faint),
        ),
    ]));

    f.render_widget(Clear, popup_area);
    f.render_widget(
        Paragraph::new(Text::from(lines))
            .wrap(Wrap { trim: true })
            .block(block),
        popup_area,
    );
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
