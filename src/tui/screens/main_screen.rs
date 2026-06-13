//! 主屏幕
//!
//! 包含聊天区、输入区、状态栏的渲染

use crate::{
    state::MessageRole,
    tui::{
        app::{SidebarPanel, TuiApp},
        components::message,
        tool_view::ToolRunView,
        view_model::timeline::{
            estimate_timeline_item_height, resolve_scroll_offset, timeline_item_heights,
            timeline_items, TimelineItem,
        },
        view_model::tool_rows::{tool_row_lines, tool_rows_for_runs_with_spine, ToolRowSeverity},
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
mod composer;
pub use composer::*;
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
        let mut intro_spans = vec![
            Span::styled(
                format!("◈ {} · ", short_id),
                Style::default().fg(app.theme.tokens.fg.faint),
            ),
            Span::styled(
                format!(
                    "{} / {}",
                    app.current_provider_label(),
                    app.current_model_label()
                ),
                Style::default().fg(app.theme.tokens.fg.faint),
            ),
        ];
        if app.agent_mode != crate::engine::agent_mode::AgentMode::Auto {
            intro_spans.push(Span::styled(
                format!(" · {}", app.current_agent_mode_label()),
                Style::default().fg(app.theme.tokens.fg.faint),
            ));
        }
        let intro = Paragraph::new(Line::from(intro_spans));
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

    // 如果没有消息，直接返回（不渲染欢迎界面）
    if app.messages.is_empty() {
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

    let items = timeline_items(&messages, app);
    let content_top = inner_area.y + top_offset;
    let content_height = inner_area.height.saturating_sub(top_offset);
    let max_y = content_top + content_height;
    let bottom_anchored = app.pinned_to_bottom || app.scroll_offset >= items.len();
    let scroll_offset = if bottom_anchored {
        items.len()
    } else {
        resolve_scroll_offset(&items, app.scroll_offset, app.scroll_anchor_id.as_deref())
    };
    let window = transcript_window(
        &items,
        scroll_offset,
        bottom_anchored,
        content_height,
        inner_area.width as usize,
        app,
    );

    // Compute total scroll rows and remaining
    let total_rows: usize = timeline_item_heights(&items, inner_area.width as usize, app)
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

        let msg_height = estimate_timeline_item_height(item, inner_area.width as usize, app);
        let msg_height = (msg_height as u16).min(max_y - current_y);
        let msg_area = Rect {
            x: inner_area.x,
            y: current_y,
            width: inner_area.width,
            height: msg_height,
        };

        match item {
            TimelineItem::Message {
                message_index, msg, ..
            } => {
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
                    message::render_message_with_options(
                        msg,
                        inner_area.width as usize,
                        &app.theme,
                        stream_meta.as_ref(),
                        message::MessageRenderOptions {
                            reasoning_expanded: app.expanded_reasoning_message_id.as_deref()
                                == Some(msg.id.as_str()),
                        },
                    )
                };
                f.render_widget(paragraph, msg_area);
            }
            TimelineItem::ToolRuns { runs, .. } => {
                let paragraph = render_tool_runs_message(runs, app);
                f.render_widget(paragraph, msg_area);
            }
        };

        current_y += msg_height;
    }
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

fn transcript_window(
    items: &[TimelineItem<'_>],
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

    let heights = timeline_item_heights(items, width, app);
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

fn active_turn_start(items: &[TimelineItem<'_>]) -> Option<usize> {
    items.iter().rposition(TimelineItem::is_user_message)
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

fn sum_heights(heights: &[usize], start: usize, end: usize) -> usize {
    heights
        .get(start..end.min(heights.len()))
        .unwrap_or_default()
        .iter()
        .sum()
}

fn visible_items_height(
    items: &[TimelineItem<'_>],
    start: usize,
    width: usize,
    app: &TuiApp,
    max_height: usize,
) -> usize {
    timeline_item_heights(items, width, app)
        .into_iter()
        .skip(start)
        .sum::<usize>()
        .min(max_height)
}

fn render_tool_runs_message<'a>(runs: &'a [ToolRunView], app: &'a TuiApp) -> Paragraph<'a> {
    let mut lines = Vec::new();
    let view = tool_rows_for_runs_with_spine(runs, &app.facade_snapshot.tool_turns, 120);
    if view.hidden_routine_count > 0 {
        lines.push(Line::from(vec![
            Span::styled("… ", Style::default().fg(app.theme.tokens.fg.faint)),
            Span::styled(
                format!(
                    "{} routine read/search tool(s) hidden",
                    view.hidden_routine_count
                ),
                Style::default().fg(app.theme.tokens.fg.faint),
            ),
        ]));
    }

    let visible_rows = view
        .rows
        .iter()
        .zip(runs.iter())
        .filter(|(row, _)| row.visible)
        .collect::<Vec<_>>();

    for (run_idx, (row, run)) in visible_rows.iter().enumerate() {
        let expanded = app.is_tool_run_expanded(run);
        let accent = tool_row_color(row.severity, app);
        for (line_idx, line) in tool_row_lines(row, expanded, run).into_iter().enumerate() {
            let prefix = if line_idx == 0 { row.icon } else { " " };
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
                Span::styled(format!("{prefix} "), Style::default().fg(accent)),
                Span::styled(line, style),
            ]));
        }
        if run_idx + 1 < visible_rows.len() {
            lines.push(Line::from(""));
        }
    }

    Paragraph::new(Text::from(lines)).wrap(Wrap { trim: true })
}

fn tool_row_color(severity: ToolRowSeverity, app: &TuiApp) -> ratatui::style::Color {
    match severity {
        ToolRowSeverity::Muted => app.theme.tokens.fg.faint,
        ToolRowSeverity::Success => app.theme.tokens.tone.ok,
        ToolRowSeverity::Info => app.theme.tokens.tone.info,
        ToolRowSeverity::Warning => app.theme.tokens.tone.warn,
        ToolRowSeverity::Error => app.theme.tokens.tone.err,
    }
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

    let title = if let Some(ref id) = app.renaming_session_id {
        format!(" Rename: {} ", &id[..8.min(id.len())])
    } else {
        format!(
            " Sessions {} ",
            if app.sidebar_filter.is_empty() {
                String::new()
            } else {
                format!("(filter: {})", app.sidebar_filter)
            }
        )
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.tokens.tone.brand))
        .title(title)
        .style(Style::default().bg(app.theme.tokens.surface.bg));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Rename input bar
    if app.renaming_session_id.is_some() {
        let rename_para = Paragraph::new(Line::from(vec![
            Span::styled(" › ", Style::default().fg(app.theme.tokens.tone.brand)),
            Span::styled(
                &app.rename_buffer,
                Style::default().fg(app.theme.tokens.fg.body),
            ),
        ]))
        .style(Style::default().bg(app.theme.tokens.surface.bg_elev));
        f.render_widget(rename_para, inner);
        // Set cursor at end of rename buffer
        let cursor_x = inner.x + 3 + app.rename_buffer.chars().count() as u16;
        f.set_cursor_position((cursor_x.min(inner.x + inner.width - 1), inner.y));
        return;
    }

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

    let visible_sessions = app.visible_sidebar_sessions(50);
    let current_id = app.session_manager.current_session_id().unwrap_or("");
    let pinned_count = visible_sessions
        .iter()
        .filter(|session| app.pinned_sessions.contains(&session.id))
        .count();

    let mut items: Vec<ListItem> = Vec::new();

    // 已固定分组
    if pinned_count > 0 {
        items.push(ListItem::new(Line::from(Span::styled(
            "─ Pinned ─".to_string(),
            Style::default()
                .fg(app.theme.tokens.tone.accent)
                .add_modifier(Modifier::BOLD),
        ))));
        for (i, session) in visible_sessions.iter().take(pinned_count).enumerate() {
            items.push(build_session_item(
                app,
                session,
                current_id,
                i,
                true,
                visible_sessions.len(),
                search_chunks[1].width,
            ));
        }
    }

    // 未固定分组
    if pinned_count > 0 && pinned_count < visible_sessions.len() {
        items.push(ListItem::new(Line::from("")));
    }

    for (i, session) in visible_sessions.iter().skip(pinned_count).enumerate() {
        items.push(build_session_item(
            app,
            session,
            current_id,
            pinned_count + i,
            false,
            visible_sessions.len(),
            search_chunks[1].width,
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

    let session_id = app.session_manager.current_session_id().unwrap_or("?");
    let short_session_id = &session_id[..session_id.len().min(8)];
    let timeline_items = app.timeline_item_count();
    let all_tool_runs = app
        .tool_runs_by_message_id
        .values()
        .map(Vec::len)
        .sum::<usize>()
        + app.tool_runs_snapshot.len();
    let failed_tool_runs = app
        .tool_runs_by_message_id
        .values()
        .flatten()
        .chain(app.tool_runs_snapshot.iter())
        .filter(|run| {
            matches!(
                run.status,
                crate::tui::tool_view::ToolRunStatus::Failed
                    | crate::tui::tool_view::ToolRunStatus::TimedOut
                    | crate::tui::tool_view::ToolRunStatus::Cancelled
            )
        })
        .count();
    let usage = app
        .stream_usage_snapshot
        .map(|usage| {
            format!(
                "{} tok{}",
                usage.total_tokens(),
                usage
                    .reasoning_tokens
                    .map(|tokens| format!(" · {} reasoning", tokens))
                    .unwrap_or_default()
            )
        })
        .unwrap_or_else(|| "no usage yet".to_string());
    let active = crate::tui::view_model::activity::active_turn_status(app)
        .map(|status| {
            let mut label = status.label;
            if let Some(detail) = status.detail {
                label.push_str(" · ");
                label.push_str(&detail);
            }
            label
        })
        .unwrap_or_else(|| "idle".to_string());
    let memory = if app.memory_use {
        format!("{} recall", app.memory_recall_mode)
    } else {
        "off".to_string()
    };

    let lines = vec![
        context_heading("Session", app),
        context_row(
            "id",
            format!(
                "{short_session_id} · {} msgs · {timeline_items} items",
                app.messages.len()
            ),
            app,
        ),
        context_row("mode", app.current_agent_mode_label(), app),
        Line::from(""),
        context_heading("Runtime", app),
        context_row("turn", active, app),
        context_row("usage", usage, app),
        context_row("permission", app.current_permission_label(), app),
        Line::from(""),
        context_heading("Composer", app),
        context_row(
            "state",
            format!(
                "hist:{} · stash:{} · files:{} · paste:{}",
                app.history.len(),
                if app.prompt_stash.is_some() {
                    "yes"
                } else {
                    "no"
                },
                app.composer_attachment_count(),
                app.pasted_block_count()
            ),
            app,
        ),
        context_row(
            "file",
            composer_attachment_sidebar_label(app).unwrap_or_else(|| "none".to_string()),
            app,
        ),
        context_row("memory", memory, app),
        Line::from(""),
        context_heading("Tools", app),
        context_row(
            "runs",
            format!("{all_tool_runs} total · {failed_tool_runs} failed"),
            app,
        ),
        context_row(
            "expanded",
            app.expanded_tool_run_id.as_deref().unwrap_or("none"),
            app,
        ),
        Line::from(""),
        context_row("keys", "Ctrl+Tab panel · b sidebar", app),
    ];

    f.render_widget(
        Paragraph::new(Text::from(lines))
            .style(Style::default().fg(app.theme.tokens.fg.body))
            .wrap(ratatui::widgets::Wrap { trim: true }),
        inner,
    );
}

fn composer_attachment_sidebar_label(app: &TuiApp) -> Option<String> {
    let summaries = app.composer_attachment_summaries();
    let first = summaries.first()?;
    if summaries.len() > 1 {
        Some(format!("{} (+{})", first, summaries.len() - 1))
    } else {
        Some(first.clone())
    }
}

fn context_heading(label: &'static str, app: &TuiApp) -> Line<'static> {
    Line::from(Span::styled(
        label,
        Style::default()
            .fg(app.theme.tokens.fg.strong)
            .add_modifier(Modifier::BOLD),
    ))
}

fn context_row(label: &'static str, value: impl Into<String>, app: &TuiApp) -> Line<'static> {
    Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(label, Style::default().fg(app.theme.tokens.fg.faint)),
        Span::styled("  ", Style::default()),
        Span::styled(value.into(), Style::default().fg(app.theme.tokens.fg.body)),
    ])
}

fn build_session_item<'a>(
    app: &'a TuiApp,
    session: &crate::session_store::SessionRecord,
    current_id: &str,
    index: usize,
    is_pinned: bool,
    _total: usize,
    sidebar_width: u16,
) -> ratatui::widgets::ListItem<'a> {
    let is_current = session.id == current_id;
    let is_selected = index == app.sidebar_selected;

    let row_bg = is_selected.then_some(app.theme.tokens.surface.bg_elev);
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
    let base = if let Some(bg) = row_bg {
        base.bg(bg)
    } else {
        base
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

    let sidebar_width = usize::from(sidebar_width);
    let title_budget = sidebar_width.saturating_sub(6).max(8);
    let display_title = truncate_chars_to_width(&title, title_budget);

    let model_short = compact_model_label(&session.model);
    let msg_count = app.session_manager.message_count(&session.id).unwrap_or(0);
    let short_id = &session.id[..8.min(session.id.len())];
    let delete_hint = if app.confirm_delete_session_id.as_deref() == Some(session.id.as_str()) {
        " · D again deletes"
    } else {
        ""
    };

    let selection = if is_selected { "› " } else { "  " };
    let meta = format!("{short_id} · {model_short} · {msg_count} msgs{delete_hint}");
    let meta_style = Style::default()
        .fg(if is_selected {
            app.theme.tokens.fg.sub
        } else {
            app.theme.tokens.fg.faint
        })
        .bg(row_bg.unwrap_or(app.theme.tokens.surface.bg));

    let title_line = Line::from(vec![
        Span::styled(selection, base),
        Span::styled(format!("{prefix} "), base),
        Span::styled(display_title, base),
    ]);
    let meta_line = Line::from(vec![
        Span::styled("    ", meta_style),
        Span::styled(meta, meta_style),
    ]);

    let mut lines = vec![title_line, meta_line];
    if is_selected {
        if let Some(preview) = selected_session_preview(app, &session.id, sidebar_width) {
            lines.push(Line::from(vec![
                Span::styled("    ", meta_style),
                Span::styled(preview, Style::default().fg(app.theme.tokens.fg.faint)),
            ]));
        }
    }

    ratatui::widgets::ListItem::new(lines)
}

pub(super) fn truncate_chars_with_ellipsis(value: &str, max_chars: usize) -> String {
    let mut out = value.chars().take(max_chars).collect::<String>();
    if value.chars().count() > max_chars {
        out.push('…');
    }
    out
}

fn truncate_chars_to_width(value: &str, max_chars: usize) -> String {
    if unicode_width::UnicodeWidthStr::width(value) <= max_chars {
        return value.to_string();
    }
    if max_chars == 0 {
        String::new()
    } else if max_chars == 1 {
        "…".to_string()
    } else {
        let content_width = max_chars - 1;
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
}

fn selected_session_preview(
    app: &TuiApp,
    session_id: &str,
    sidebar_width: usize,
) -> Option<String> {
    let preview_budget = sidebar_width.saturating_sub(4).max(8);
    app.session_manager
        .recent_preview_lines(session_id, 1)
        .ok()
        .and_then(|lines| lines.into_iter().next())
        .map(|line| truncate_chars_to_width(line.trim(), preview_budget))
}

fn compact_model_label(model: &str) -> String {
    let lower = model.to_ascii_lowercase();
    if lower.contains("deepseek") {
        "deepseek-v4".to_string()
    } else if lower.contains("minimax") {
        "minimax".to_string()
    } else if lower.contains("claude") {
        if lower.contains("haiku") {
            "claude-haiku".to_string()
        } else if lower.contains("sonnet") {
            "claude-sonnet".to_string()
        } else {
            "claude".to_string()
        }
    } else if lower.contains("gpt-5") {
        "gpt-5".to_string()
    } else if lower.contains("gpt-4") {
        "gpt-4".to_string()
    } else {
        truncate_chars_with_ellipsis(model, 12)
    }
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
