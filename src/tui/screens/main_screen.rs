//! 主屏幕
//!
//! 包含聊天区、输入区、状态栏的渲染

use crate::{
    state::{MessageItem, MessageRole},
    tui::{
        app::TuiApp,
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

/// 渲染聊天区域（Claude Code 风格：无边框，留白分隔）
pub fn render_chat_area(f: &mut Frame, app: &TuiApp, area: Rect) {
    // 直接使用 area，不添加外边框
    let inner_area = area;

    // 如果有消息，渲染它们
    if app.messages.is_empty() {
        let welcome = Paragraph::new(Text::from(vec![
            Line::from(""),
            Line::from(vec![Span::styled(
                "Welcome to Priority Agent",
                Style::default()
                    .fg(app.theme.text_highlight)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(vec![Span::styled(
                "Type a message and press Enter to chat.",
                Style::default().fg(app.theme.text_dim),
            )]),
            Line::from(vec![Span::styled(
                format!("Model: {}", app.current_model_label()),
                Style::default().fg(app.theme.text_dim),
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
    let max_y = inner_area.y + inner_area.height;
    let bottom_anchored = app.scroll_offset >= messages.len();
    let window = transcript_window(
        &items,
        app.scroll_offset,
        bottom_anchored,
        inner_area.height,
        inner_area.width as usize,
        app,
    );

    let mut current_y = if window.bottom_anchored {
        max_y.saturating_sub(window.message_height as u16)
    } else {
        inner_area.y + u16::from(window.more_above)
    };

    if window.more_above {
        let indicator_y = if window.bottom_anchored {
            current_y.saturating_sub(1).max(inner_area.y)
        } else {
            inner_area.y
        };
        let indicator = Paragraph::new("↑ more above").style(
            Style::default()
                .fg(app.theme.text_dim)
                .add_modifier(Modifier::ITALIC),
        );
        f.render_widget(
            indicator,
            Rect {
                x: inner_area.x,
                y: indicator_y,
                width: inner_area.width,
                height: 1,
            },
        );
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
                let paragraph = if collapsed {
                    message::render_message_compact(msg, &app.theme)
                } else {
                    message::render_message(msg, inner_area.width as usize, &app.theme)
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

/// 渲染输入区域（Claude Code 风格：底线分隔 + > 前缀）
pub fn render_input_area(f: &mut Frame, app: &TuiApp, area: Rect) {
    let border_style = Style::default().fg(app.theme.border);

    // 输入区上下分隔线，形成 Claude Code 式的轻量输入槽
    let top_sep = Paragraph::new("").block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(border_style),
    );
    let sep_area = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: 1,
    };
    f.render_widget(top_sep, sep_area);

    let bottom_sep = Paragraph::new("").block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(border_style),
    );
    let bottom_area = Rect {
        x: area.x,
        y: area.y + area.height.saturating_sub(1),
        width: area.width,
        height: 1,
    };
    f.render_widget(bottom_sep, bottom_area);

    let inner_area = Rect {
        x: area.x + 2,
        y: area.y + 1,
        width: area.width.saturating_sub(4),
        height: area.height.saturating_sub(2),
    };

    let input_text = app.input.value();

    let (display_text, style) = if app.is_querying {
        let mut lines = vec![Line::from(vec![
            Span::styled("› ", Style::default().fg(app.theme.text_dim)),
            Span::styled(
                "Thinking...",
                Style::default().fg(app.theme.status_thinking),
            ),
        ])];
        if let Some(ref err) = app.error_message {
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(err.clone(), Style::default().fg(app.theme.error)),
            ]));
        }
        (Text::from(lines), Style::default())
    } else if app.mode == crate::tui::app::AppMode::VimNormal {
        let text = Text::from(vec![Line::from(vec![
            Span::styled("› ", Style::default().fg(app.theme.text_dim)),
            Span::styled(
                "Vim Normal: j/k scroll, i insert, : command, Ctrl+V toggle",
                Style::default().fg(app.theme.text_dim),
            ),
        ])]);
        (text, Style::default())
    } else if input_text.is_empty() {
        let text = Text::from(vec![Line::from(vec![
            Span::styled("› ", Style::default().fg(app.theme.text_dim)),
            Span::styled(
                "Message Priority Agent...",
                Style::default().fg(app.theme.text_dim),
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
                        Span::styled("› ", Style::default().fg(app.theme.text_dim)),
                        Span::styled(line.to_string(), Style::default().fg(app.theme.text)),
                    ])
                } else {
                    Line::from(vec![
                        Span::styled("  ", Style::default()),
                        Span::styled(line.to_string(), Style::default().fg(app.theme.text)),
                    ])
                }
            })
            .collect();
        if lines.is_empty() {
            lines.push(Line::from(vec![Span::styled(
                "› ",
                Style::default().fg(app.theme.text_dim),
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

/// 渲染状态栏（Claude Code 风格：单行，简洁，· 分隔）
pub fn render_status_bar(f: &mut Frame, app: &TuiApp, area: Rect) {
    let mut parts = Vec::new();

    // 左侧：状态
    if app.is_querying {
        let frames = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
        let ch = frames[app.tick_count % frames.len()];
        let label = app
            .current_tool_status_label()
            .unwrap_or_else(|| "Thinking".to_string());
        parts.push(Span::styled(
            format!("{} {}...", ch, label),
            Style::default().fg(app.theme.status_thinking),
        ));
        let active_tools = app.active_tool_count();
        if active_tools > 0 {
            parts.push(Span::styled(
                format!("{} tools", active_tools),
                Style::default().fg(app.theme.text_dim),
            ));
        }
        if let Some(usage) = app.stream_usage_label() {
            parts.push(Span::styled(usage, Style::default().fg(app.theme.text_dim)));
        }
        parts.push(Span::styled(
            "esc to interrupt",
            Style::default().fg(app.theme.text_dim),
        ));
    } else if let Some(ref error) = app.error_message {
        parts.push(Span::styled(
            format!("✗ {}", error),
            Style::default().fg(app.theme.error),
        ));
    } else {
        parts.push(Span::styled(
            "Ready",
            Style::default().fg(app.theme.text_dim),
        ));
    }

    // 中间：模式徽章
    if app.vim_mode {
        parts.push(Span::styled(
            "vim",
            Style::default().fg(app.theme.status_vim),
        ));
    }
    if app.paused {
        parts.push(Span::styled(
            "paused",
            Style::default().fg(app.theme.warning),
        ));
    }
    if app.focus_mode {
        parts.push(Span::styled("focus", Style::default().fg(app.theme.info)));
    }
    parts.push(Span::styled(
        app.workspace_status_label(),
        Style::default().fg(app.theme.text_dim),
    ));
    if let Some(label) = app.plan_mode_status_label() {
        parts.push(Span::styled(label, Style::default().fg(app.theme.warning)));
    }
    let pasted_blocks = app.pasted_block_count();
    if pasted_blocks > 0 {
        parts.push(Span::styled(
            format!("{} pasted", pasted_blocks),
            Style::default().fg(app.theme.info),
        ));
    }

    parts.push(Span::styled(
        app.current_permission_label(),
        Style::default().fg(app.theme.warning),
    ));
    parts.push(Span::styled(
        format!(
            "{} / {}",
            app.current_provider_label(),
            app.current_model_label()
        ),
        Style::default().fg(app.theme.text_dim),
    ));
    parts.push(Span::styled(
        format!("{} msgs", app.message_count()),
        Style::default().fg(app.theme.text_dim),
    ));
    if !app.is_querying {
        if let Some(usage) = app.stream_usage_label() {
            parts.push(Span::styled(
                format!("last {}", usage),
                Style::default().fg(app.theme.text_dim),
            ));
        }
    }
    parts.push(Span::styled(
        "? shortcuts",
        Style::default().fg(app.theme.text_dim),
    ));

    // 用 " · " 连接所有部分
    let mut spans = Vec::new();
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" · ", Style::default().fg(app.theme.border)));
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
        start = items.len().saturating_sub(1);
        used_height = 0;
        for idx in (0..items.len()).rev() {
            let candidate_height = heights[idx];
            let more_above = idx > 0;
            let next_height = used_height + candidate_height + usize::from(more_above);
            if used_height > 0 && next_height > viewport {
                break;
            }
            start = idx;
            used_height += candidate_height;
            if used_height + usize::from(more_above) >= viewport {
                break;
            }
        }
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
            ToolRunStatus::Queued | ToolRunStatus::Running => app.theme.status_thinking,
            ToolRunStatus::WaitingPermission => app.theme.warning,
            ToolRunStatus::Completed => app.theme.text_dim,
            ToolRunStatus::Failed => app.theme.error,
        };
        for (line_idx, line) in run.render_lines(expanded).into_iter().enumerate() {
            let prefix = if line_idx == 0 {
                match run.status {
                    ToolRunStatus::Queued | ToolRunStatus::Running => "● ",
                    ToolRunStatus::WaitingPermission => "? ",
                    ToolRunStatus::Completed => "✓ ",
                    ToolRunStatus::Failed => "✗ ",
                }
            } else {
                "  "
            };
            let style = if line_idx == 0 {
                Style::default().fg(accent)
            } else if expanded && matches!(line.trim_start().chars().next(), Some('{' | '}' | '"'))
            {
                Style::default().fg(app.theme.text)
            } else if expanded && line.contains("result:") {
                Style::default().fg(app.theme.text)
            } else {
                Style::default().fg(app.theme.text_dim)
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
        .border_style(Style::default().fg(app.theme.border_active))
        .style(Style::default().bg(app.theme.bg_popup));

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
            Style::default().fg(app.theme.text_dim),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("esc/q", Style::default().fg(app.theme.info)),
        Span::styled(" close  ", Style::default().fg(app.theme.text_dim)),
        Span::styled("↑/↓", Style::default().fg(app.theme.info)),
        Span::styled(" scroll  ", Style::default().fg(app.theme.text_dim)),
        Span::styled("PgUp/PgDn", Style::default().fg(app.theme.info)),
        Span::styled(" page", Style::default().fg(app.theme.text_dim)),
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
            .fg(app.theme.text_highlight)
            .add_modifier(Modifier::BOLD)
    } else if trimmed.starts_with("ERROR")
        || trimmed.starts_with("Error")
        || trimmed.starts_with("error")
        || trimmed.contains("panicked")
        || trimmed.contains("failed")
    {
        Style::default().fg(app.theme.error)
    } else if trimmed.starts_with('+') && !trimmed.starts_with("+++") {
        Style::default().fg(app.theme.diff_add)
    } else if trimmed.starts_with('-') && !trimmed.starts_with("---") {
        Style::default().fg(app.theme.diff_remove)
    } else if trimmed.starts_with("@@") || trimmed.starts_with("diff --git") {
        Style::default()
            .fg(app.theme.diff_line_number)
            .add_modifier(Modifier::BOLD)
    } else if matches!(trimmed.chars().next(), Some('{' | '}' | '[' | ']' | '"')) {
        Style::default().fg(app.theme.info)
    } else if raw.starts_with("- ") {
        Style::default().fg(app.theme.text_dim)
    } else {
        Style::default().fg(app.theme.text)
    }
}

/// 渲染会话侧边栏
pub fn render_sidebar(f: &mut Frame, app: &TuiApp, area: Rect) {
    use ratatui::widgets::{Block, Borders, List, ListItem};

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.border_active))
        .title(" Sessions ")
        .style(Style::default().bg(app.theme.bg));

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
                    .fg(app.theme.assistant_message)
                    .add_modifier(Modifier::BOLD)
            } else if is_selected {
                Style::default()
                    .fg(app.theme.text_highlight)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(app.theme.text)
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
        .border_style(Style::default().fg(app.theme.border_active))
        .style(Style::default().bg(app.theme.bg_popup));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    if search.results.is_empty() {
        let hint = if search.query.is_empty() {
            "Type to search messages..."
        } else {
            "No matches found"
        };
        let text = Paragraph::new(hint)
            .style(Style::default().fg(app.theme.text_dim))
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
        .style(Style::default().fg(app.theme.text_dim))
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
        .border_style(Style::default().fg(app.theme.info))
        .style(Style::default().bg(Color::Black));

    let mut lines = vec![
        Line::from(vec![
            Span::styled("Search: ", Style::default().fg(app.theme.text_dim)),
            Span::styled(
                if app.command_palette_query.is_empty() {
                    "type to filter commands"
                } else {
                    app.command_palette_query.as_str()
                },
                if app.command_palette_query.is_empty() {
                    Style::default().fg(app.theme.text_dim)
                } else {
                    Style::default().fg(app.theme.text)
                },
            ),
        ]),
        Line::from(""),
    ];

    let items = app.command_palette_items();
    if items.is_empty() {
        lines.push(Line::from(Span::styled(
            "No commands matched.",
            Style::default().fg(app.theme.text_dim),
        )));
    } else {
        let mut last_category = "";
        for (idx, cmd) in items.iter().enumerate() {
            if cmd.category != last_category {
                if idx > 0 {
                    lines.push(Line::from(""));
                }
                lines.push(Line::from(Span::styled(
                    cmd.category,
                    Style::default()
                        .fg(app.theme.text_highlight)
                        .add_modifier(Modifier::BOLD),
                )));
                last_category = cmd.category;
            }

            let selected = idx == app.command_palette_selected;
            let marker = if selected { "› " } else { "  " };
            let style = if selected {
                Style::default()
                    .fg(app.theme.text)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(app.theme.text_dim)
            };
            let alias = if cmd.aliases.is_empty() {
                String::new()
            } else {
                format!(" ({})", cmd.aliases.join(", "))
            };
            lines.push(Line::from(vec![
                Span::styled(marker, Style::default().fg(app.theme.info)),
                Span::styled(format!("{:<18}", cmd.name), style),
                Span::styled(cmd.description, Style::default().fg(app.theme.text_dim)),
                Span::styled(alias, Style::default().fg(app.theme.text_dim)),
            ]));
        }
    }

    if let Some(selected) = items.get(app.command_palette_selected) {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("Usage: ", Style::default().fg(app.theme.text_dim)),
            Span::styled(
                selected.usage,
                Style::default()
                    .fg(app.theme.text)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Info:  ", Style::default().fg(app.theme.text_dim)),
            Span::styled(
                selected.description,
                Style::default().fg(app.theme.text_dim),
            ),
        ]));
        if !selected.aliases.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("Alias: ", Style::default().fg(app.theme.text_dim)),
                Span::styled(
                    selected.aliases.join(", "),
                    Style::default().fg(app.theme.text_dim),
                ),
            ]));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("enter", Style::default().fg(app.theme.info)),
        Span::styled(
            " execute or insert  ",
            Style::default().fg(app.theme.text_dim),
        ),
        Span::styled("esc", Style::default().fg(app.theme.info)),
        Span::styled(" close  ", Style::default().fg(app.theme.text_dim)),
        Span::styled("ctrl+p", Style::default().fg(app.theme.info)),
        Span::styled(" toggle", Style::default().fg(app.theme.text_dim)),
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
        .border_style(Style::default().fg(app.theme.info))
        .style(Style::default().bg(Color::Black));
    let kb = &app.keybindings;
    let lines = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "Core",
            Style::default()
                .fg(app.theme.text_highlight)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(format!("  {}        send message", kb.chat_submit)),
        Line::from(format!("  {}   insert newline", kb.chat_newline)),
        Line::from("  ctrl+p       command palette"),
        Line::from("  ctrl+m       model picker"),
        Line::from("  ctrl+l       provider picker"),
        Line::from("  ctrl+o       expand/collapse tool details"),
        Line::from("  ctrl+t       open full tool output"),
        Line::from(format!("  {}       quit", kb.global_quit)),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Navigation",
            Style::default()
                .fg(app.theme.text_highlight)
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
                .fg(app.theme.text_highlight)
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
            Style::default().fg(app.theme.text_dim),
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
        .border_style(Style::default().fg(app.theme.info))
        .style(Style::default().bg(Color::Black));

    let mut lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("Provider ", Style::default().fg(app.theme.text_dim)),
            Span::styled(
                app.current_provider_label(),
                Style::default()
                    .fg(app.theme.text_highlight)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Base URL ", Style::default().fg(app.theme.text_dim)),
            Span::styled(
                app.current_provider_base_url(),
                Style::default().fg(app.theme.text_dim),
            ),
        ]),
        Line::from(vec![
            Span::styled("Search  ", Style::default().fg(app.theme.text_dim)),
            Span::styled(
                if app.model_select_query.is_empty() {
                    "type to filter models"
                } else {
                    app.model_select_query.as_str()
                },
                if app.model_select_query.is_empty() {
                    Style::default().fg(app.theme.text_dim)
                } else {
                    Style::default().fg(app.theme.text)
                },
            ),
        ]),
        Line::from(""),
    ];

    let choices = app.model_choices();
    for (idx, choice) in choices.iter().enumerate() {
        let selected = idx == app.model_select_selected;
        let marker = if selected { "› " } else { "  " };
        let model_style = if choice.active {
            Style::default()
                .fg(app.theme.text_highlight)
                .add_modifier(Modifier::BOLD)
        } else if selected {
            Style::default()
                .fg(app.theme.text)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(app.theme.text)
        };
        lines.push(Line::from(vec![
            Span::styled(marker, Style::default().fg(app.theme.info)),
            Span::styled(format!("{:<24}", choice.model), model_style),
            Span::styled(choice.note.clone(), Style::default().fg(app.theme.text_dim)),
        ]));
    }

    if let Some(notice) = &app.model_notice {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            notice.clone(),
            Style::default().fg(app.theme.info),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("enter", Style::default().fg(app.theme.info)),
        Span::styled(
            " switch for next request  ",
            Style::default().fg(app.theme.text_dim),
        ),
        Span::styled("esc", Style::default().fg(app.theme.info)),
        Span::styled(" close  ", Style::default().fg(app.theme.text_dim)),
        Span::styled("backspace", Style::default().fg(app.theme.info)),
        Span::styled(" edit search  ", Style::default().fg(app.theme.text_dim)),
        Span::styled("/settings", Style::default().fg(app.theme.info)),
        Span::styled(
            " provider/API keys",
            Style::default().fg(app.theme.text_dim),
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
        .border_style(Style::default().fg(app.theme.info))
        .style(Style::default().bg(Color::Black));

    let mut lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("Current ", Style::default().fg(app.theme.text_dim)),
            Span::styled(
                format!(
                    "{} / {}",
                    app.current_provider_label(),
                    app.current_model_label()
                ),
                Style::default()
                    .fg(app.theme.text_highlight)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Search  ", Style::default().fg(app.theme.text_dim)),
            Span::styled(
                if app.provider_select_query.is_empty() {
                    "type to filter providers"
                } else {
                    app.provider_select_query.as_str()
                },
                if app.provider_select_query.is_empty() {
                    Style::default().fg(app.theme.text_dim)
                } else {
                    Style::default().fg(app.theme.text)
                },
            ),
        ]),
        Line::from(""),
    ];

    for (idx, choice) in app.provider_choices().iter().enumerate() {
        let selected = idx == app.provider_select_selected;
        let marker = if selected { "› " } else { "  " };
        let style = if choice.active {
            Style::default()
                .fg(app.theme.text_highlight)
                .add_modifier(Modifier::BOLD)
        } else if choice.configured {
            Style::default().fg(app.theme.text)
        } else {
            Style::default().fg(app.theme.text_dim)
        };
        lines.push(Line::from(vec![
            Span::styled(marker, Style::default().fg(app.theme.info)),
            Span::styled(format!("{:<10}", choice.name), style),
            Span::styled(
                format!("{:<12}", choice.provider_type),
                Style::default().fg(app.theme.text_dim),
            ),
            Span::styled(format!("{:<20}", choice.model), style),
            Span::styled(choice.note.clone(), Style::default().fg(app.theme.text_dim)),
        ]));
        if selected && !choice.base_url.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("  └ ", Style::default().fg(app.theme.text_dim)),
                Span::styled(
                    choice.base_url.clone(),
                    Style::default().fg(app.theme.text_dim),
                ),
            ]));
        }
    }

    if let Some(notice) = &app.provider_notice {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            notice.clone(),
            Style::default().fg(app.theme.info),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("enter", Style::default().fg(app.theme.info)),
        Span::styled(
            " switch configured provider  ",
            Style::default().fg(app.theme.text_dim),
        ),
        Span::styled("esc", Style::default().fg(app.theme.info)),
        Span::styled(" close  ", Style::default().fg(app.theme.text_dim)),
        Span::styled("backspace", Style::default().fg(app.theme.info)),
        Span::styled(" edit search  ", Style::default().fg(app.theme.text_dim)),
        Span::styled("/settings", Style::default().fg(app.theme.info)),
        Span::styled(
            " edit keys/base URL",
            Style::default().fg(app.theme.text_dim),
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

/// 渲染权限审批弹窗
pub fn render_permission_approval(
    f: &mut Frame,
    req: &crate::engine::conversation_loop::ToolApprovalRequest,
    area: Rect,
) {
    let popup_area = centered_rect(72, 58, area);
    let risk = permission_risk_label(&req.tool_call.name, &req.tool_call.arguments);
    let risk_color = match risk {
        "high" => Color::Red,
        "medium" => Color::Yellow,
        _ => Color::Green,
    };

    let block = Block::default()
        .title(" Tool Approval ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(risk_color))
        .style(Style::default().bg(Color::Black));

    let mut lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("Request ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                req.tool_call.name.clone(),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("  risk ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                risk,
                Style::default().fg(risk_color).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Scope   ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                permission_scope_label(&req.tool_call.name, &req.tool_call.arguments),
                Style::default().fg(Color::Gray),
            ),
        ]),
        Line::from(""),
    ];

    if let Some(summary) = permission_preview(&req.tool_call.name, &req.tool_call.arguments) {
        lines.push(Line::from(vec![
            Span::styled("Preview ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled("  ", Style::default()),
            Span::styled(summary.0, Style::default().fg(Color::White)),
        ]));
        for line in summary.1.lines().take(6) {
            lines.push(Line::from(Span::styled(
                format!("  {}", line),
                Style::default().fg(Color::Gray),
            )));
        }
        lines.push(Line::from(""));
    }

    lines.push(Line::from(Span::styled(
        "Reason",
        Style::default().add_modifier(Modifier::BOLD),
    )));
    for line in req.prompt.lines().take(4) {
        lines.push(Line::from(Span::styled(
            format!("  {}", line),
            Style::default().fg(Color::White),
        )));
    }

    if let Ok(args) = serde_json::to_string_pretty(&req.tool_call.arguments) {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Arguments",
            Style::default().add_modifier(Modifier::BOLD),
        )));
        for line in args.lines().take(8) {
            lines.push(Line::from(Span::styled(
                format!("  {}", line),
                Style::default().fg(Color::DarkGray),
            )));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(
            "y",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" allow once  ", Style::default().fg(Color::Gray)),
        Span::styled(
            "n",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" deny  ", Style::default().fg(Color::Gray)),
        Span::styled(
            "esc",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" cancel", Style::default().fg(Color::Gray)),
    ]));
    lines.push(Line::from(vec![
        Span::styled(
            "s",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" allow session  ", Style::default().fg(Color::Gray)),
        Span::styled(
            "p",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" allow project  ", Style::default().fg(Color::Gray)),
        Span::styled(
            "a",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" allow always", Style::default().fg(Color::Gray)),
    ]));
    lines.push(Line::from(vec![
        Span::styled(
            "x",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" deny always", Style::default().fg(Color::Gray)),
    ]));

    let has_diff_preview = matches!(
        req.tool_call.name.as_str(),
        "file_write" | "file_edit" | "bash"
    );
    if has_diff_preview {
        lines.push(Line::from(vec![
            Span::styled(
                "d",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" preview diff/output", Style::default().fg(Color::Gray)),
        ]));
    }

    let text = Text::from(lines);
    let paragraph = Paragraph::new(text).wrap(Wrap { trim: true }).block(block);

    f.render_widget(Clear, popup_area);
    f.render_widget(paragraph, popup_area);
}

fn permission_scope_label(tool_name: &str, args: &serde_json::Value) -> String {
    match tool_name {
        "bash" => args
            .get("command")
            .and_then(serde_json::Value::as_str)
            .map(|cmd| format!("shell: {}", compact_permission_line(cmd, 76)))
            .unwrap_or_else(|| "shell command".to_string()),
        "file_write" | "file_edit" | "file_read" => args
            .get("path")
            .and_then(serde_json::Value::as_str)
            .map(|path| format!("file: {}", path))
            .unwrap_or_else(|| "file operation".to_string()),
        "mcp_tool" => {
            let server = args
                .get("server_name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("server");
            let tool = args
                .get("tool_name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("tool");
            format!("mcp: {} / {}", server, tool)
        }
        _ => "tool call".to_string(),
    }
}

fn permission_risk_label(tool_name: &str, args: &serde_json::Value) -> &'static str {
    let name = tool_name.to_ascii_lowercase();
    if name.contains("bash") {
        let command = args
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if command.contains("rm ")
            || command.contains("sudo")
            || command.contains("chmod")
            || command.contains("curl")
            || command.contains("delete")
        {
            return "high";
        }
        return "medium";
    }
    if name.contains("write") || name.contains("edit") || name.contains("delete") {
        return "medium";
    }
    if name.contains("web") || name.contains("mcp") || name.contains("github") {
        return "medium";
    }
    "low"
}

fn permission_preview(tool_name: &str, args: &serde_json::Value) -> Option<(&'static str, String)> {
    let name = tool_name.to_ascii_lowercase();
    if name.contains("bash") {
        return args
            .get("command")
            .and_then(|v| v.as_str())
            .map(|cmd| ("Command", format!("$ {}", cmd)));
    }
    if name.contains("file") || name.contains("format") {
        let path = args
            .get("path")
            .or_else(|| args.get("file_path"))
            .and_then(|v| v.as_str())
            .unwrap_or("(unknown path)");
        let action = if name.contains("write") {
            "Write"
        } else if name.contains("edit") {
            "Edit"
        } else {
            "File"
        };
        return Some((action, path.to_string()));
    }
    if name.contains("web") {
        return args
            .get("url")
            .or_else(|| args.get("query"))
            .and_then(|v| v.as_str())
            .map(|target| ("Network", target.to_string()));
    }
    None
}

fn compact_permission_line(text: &str, max_chars: usize) -> String {
    let line = text.lines().next().unwrap_or("").trim();
    if line.chars().count() <= max_chars {
        line.to_string()
    } else {
        format!(
            "{}…",
            line.chars()
                .take(max_chars.saturating_sub(1))
                .collect::<String>()
        )
    }
}

/// 渲染计划审批弹窗
pub fn render_plan_approval(f: &mut Frame, plan: &crate::engine::plan_mode::Plan, area: Rect) {
    let popup_area = centered_rect(70, 70, area);

    let block = Block::default()
        .title(format!(" Plan Approval: {} ", plan.title))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .style(Style::default().bg(Color::Black));

    let mut lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("Goal: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(plan.goal.clone(), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled(
                "Complexity: ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                plan.estimated_complexity.clone(),
                Style::default().fg(Color::Cyan),
            ),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            format!("Steps ({}):", plan.steps.len()),
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from(Span::styled(
            "────────────────────────────────────────",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    for (i, step) in plan.steps.iter().enumerate() {
        let status_icon = match step.status {
            crate::engine::plan_mode::StepStatus::Pending => "[ ]",
            crate::engine::plan_mode::StepStatus::InProgress => "[~]",
            crate::engine::plan_mode::StepStatus::Completed => "[x]",
            crate::engine::plan_mode::StepStatus::Skipped => "[s]",
            crate::engine::plan_mode::StepStatus::Failed(_) => "[!]",
        };
        let tool_info = step
            .tool
            .as_deref()
            .map(|t| format!(" (via {})", t))
            .unwrap_or_default();
        lines.push(Line::from(vec![
            Span::styled(
                format!("  {} {}. ", status_icon, i + 1),
                Style::default().fg(Color::Gray),
            ),
            Span::styled(step.description.clone(), Style::default().fg(Color::White)),
            Span::styled(tool_info, Style::default().fg(Color::DarkGray)),
        ]));
    }

    lines.push(Line::from(Span::styled(
        "────────────────────────────────────────",
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(
            "y",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" = Approve  ", Style::default().fg(Color::Gray)),
        Span::styled(
            "n",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" = Reject  ", Style::default().fg(Color::Gray)),
        Span::styled(
            "m",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" = Modify", Style::default().fg(Color::Gray)),
    ]));

    let text = Text::from(lines);
    let paragraph = Paragraph::new(text).wrap(Wrap { trim: true }).block(block);

    f.render_widget(Clear, popup_area);
    f.render_widget(paragraph, popup_area);
}

/// 渲染问答用户弹窗
pub fn render_ask_user(f: &mut Frame, question: &str, options: &[String], area: Rect) {
    let popup_area = centered_rect(70, 50, area);

    let block = Block::default()
        .title(" Question from Agent ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .style(Style::default().bg(Color::Black));

    let mut lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("Q: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(question.to_string(), Style::default().fg(Color::White)),
        ]),
        Line::from(""),
    ];

    if !options.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            "Options:",
            Style::default().add_modifier(Modifier::BOLD),
        )]));
        for (i, opt) in options.iter().enumerate() {
            lines.push(Line::from(vec![
                Span::styled(format!("  {}. ", i + 1), Style::default().fg(Color::Cyan)),
                Span::styled(opt.clone(), Style::default().fg(Color::White)),
            ]));
        }
        lines.push(Line::from(""));
    }

    lines.push(Line::from(vec![
        Span::styled(
            "Enter",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" = Submit answer  ", Style::default().fg(Color::Gray)),
        Span::styled(
            "Esc",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" = Cancel", Style::default().fg(Color::Gray)),
    ]));

    let text = Text::from(lines);
    let paragraph = Paragraph::new(text).wrap(Wrap { trim: true }).block(block);

    f.render_widget(Clear, popup_area);
    f.render_widget(paragraph, popup_area);
}

/// 渲染 Onboarding 引导弹窗
pub fn render_onboarding(
    f: &mut Frame,
    state: &crate::onboarding::OnboardingState,
    area: Rect,
    theme: &crate::tui::theme::Theme,
) {
    let popup_area = centered_rect(80, 75, area);
    let step = state.step;

    let block = Block::default()
        .title(format!(
            " Onboarding ({}/{}) — {} ",
            step.index() + 1,
            crate::onboarding::OnboardingStep::total_steps(),
            step.title()
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .style(Style::default().bg(theme.bg));

    let mut lines = vec![Line::from("")];

    // 步骤内容
    for line in step.content().lines() {
        if line.trim().is_empty() {
            lines.push(Line::from(""));
        } else if line.starts_with("- ") {
            lines.push(Line::from(vec![
                Span::styled("  • ", Style::default().fg(Color::Cyan)),
                Span::styled(
                    line.strip_prefix("- ").unwrap_or(line).to_string(),
                    Style::default().fg(theme.text),
                ),
            ]));
        } else if line.ends_with(':') && !line.contains(" ") {
            lines.push(Line::from(vec![Span::styled(
                line.to_string(),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )]));
        } else if line.starts_with("Welcome") || line.starts_with("You're all set") {
            lines.push(Line::from(vec![Span::styled(
                line.to_string(),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )]));
        } else {
            lines.push(Line::from(Span::styled(
                line.to_string(),
                Style::default().fg(theme.text),
            )));
        }
    }

    // 底部导航提示
    lines.push(Line::from(""));
    lines.push(Line::from(""));

    let nav_spans = if step.index() == 0 {
        vec![
            Span::styled(
                "Enter/→",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" = Next  ", Style::default().fg(Color::Gray)),
            Span::styled(
                "Esc",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" = Skip", Style::default().fg(Color::Gray)),
        ]
    } else if step == crate::onboarding::OnboardingStep::Done {
        vec![
            Span::styled(
                "←",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" = Back  ", Style::default().fg(Color::Gray)),
            Span::styled(
                "Enter",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" = Finish", Style::default().fg(Color::Gray)),
        ]
    } else {
        vec![
            Span::styled(
                "←",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" = Back  ", Style::default().fg(Color::Gray)),
            Span::styled(
                "Enter/→",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" = Next  ", Style::default().fg(Color::Gray)),
            Span::styled(
                "Esc",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" = Skip", Style::default().fg(Color::Gray)),
        ]
    };

    lines.push(Line::from(nav_spans));

    let text = Text::from(lines);
    let paragraph = Paragraph::new(text).wrap(Wrap { trim: true }).block(block);

    f.render_widget(Clear, popup_area);
    f.render_widget(paragraph, popup_area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::HashMap, time::SystemTime};

    fn msg(role: MessageRole, content: &str) -> MessageItem {
        MessageItem {
            id: format!("{:?}-{}", role, content.len()),
            role,
            content: content.to_string(),
            timestamp: SystemTime::UNIX_EPOCH,
            metadata: HashMap::new(),
        }
    }

    #[test]
    fn transcript_window_prefers_active_turn_when_bottom_anchored() {
        let app = TuiApp::new();
        let items = [
            msg(MessageRole::User, "old question"),
            msg(MessageRole::Assistant, "old answer"),
            msg(MessageRole::User, "current question"),
            msg(MessageRole::Assistant, "current answer"),
        ];
        let refs: Vec<_> = items.iter().collect();
        let transcript = transcript_items(&refs, &app);

        let window = transcript_window(&transcript, refs.len(), true, 6, 80, &app);

        assert_eq!(window.start, 2);
        assert!(window.more_above);
        assert!(window.bottom_anchored);
    }

    #[test]
    fn transcript_window_includes_recent_context_when_it_fits() {
        let app = TuiApp::new();
        let items = [
            msg(MessageRole::User, "old question"),
            msg(MessageRole::Assistant, "old answer"),
            msg(MessageRole::User, "current question"),
            msg(MessageRole::Assistant, "current answer"),
        ];
        let refs: Vec<_> = items.iter().collect();
        let transcript = transcript_items(&refs, &app);

        let window = transcript_window(&transcript, refs.len(), true, 7, 80, &app);

        assert_eq!(window.start, 1);
        assert!(window.more_above);
        assert_eq!(window.message_height, 6);
    }

    #[test]
    fn transcript_window_preserves_manual_scroll_offset() {
        let app = TuiApp::new();
        let items = [
            msg(MessageRole::User, "one"),
            msg(MessageRole::Assistant, "two"),
            msg(MessageRole::User, "three"),
        ];
        let refs: Vec<_> = items.iter().collect();
        let transcript = transcript_items(&refs, &app);

        let window = transcript_window(&transcript, 1, false, 6, 80, &app);

        assert_eq!(window.start, 1);
        assert!(window.more_above);
        assert!(!window.bottom_anchored);
    }

    #[test]
    fn transcript_items_insert_tool_runs_after_active_user() {
        let mut app = TuiApp::new();
        let items = [
            msg(MessageRole::User, "old question"),
            msg(MessageRole::Assistant, "old answer"),
            msg(MessageRole::User, "current question"),
            msg(MessageRole::Assistant, "current answer"),
        ];
        let mut run = ToolRunView::new("tool_1".to_string(), "bash".to_string());
        run.arguments = Some(serde_json::json!({
            "command": "ls -la ~/Desktop"
        }));
        app.tool_runs_by_message_id
            .insert(items[2].id.clone(), vec![run]);
        let refs: Vec<_> = items.iter().collect();

        let transcript = transcript_items(&refs, &app);

        assert_eq!(transcript.len(), 5);
        assert!(matches!(transcript[3], TranscriptItem::ToolRuns(_)));
        let window = transcript_window(&transcript, refs.len(), true, 8, 80, &app);
        assert_eq!(window.start, 2);
    }

    #[test]
    fn transcript_items_keep_tool_runs_for_previous_turns() {
        let mut app = TuiApp::new();
        let items = [
            msg(MessageRole::User, "first question"),
            msg(MessageRole::Assistant, "first answer"),
            msg(MessageRole::User, "second question"),
            msg(MessageRole::Assistant, "second answer"),
        ];
        app.tool_runs_by_message_id.insert(
            items[0].id.clone(),
            vec![ToolRunView::new("tool_1".to_string(), "bash".to_string())],
        );
        app.tool_runs_by_message_id.insert(
            items[2].id.clone(),
            vec![ToolRunView::new("tool_2".to_string(), "grep".to_string())],
        );
        let refs: Vec<_> = items.iter().collect();

        let transcript = transcript_items(&refs, &app);

        assert_eq!(transcript.len(), 6);
        assert!(matches!(transcript[1], TranscriptItem::ToolRuns(_)));
        assert!(matches!(transcript[4], TranscriptItem::ToolRuns(_)));
    }

    #[test]
    fn estimate_tool_runs_height_uses_single_expanded_tool() {
        let mut app = TuiApp::new();
        let mut first = ToolRunView::new("tool_1".to_string(), "bash".to_string());
        first.arguments = Some(serde_json::json!({ "command": "ls" }));
        first.mark_complete("Result: OK\na.txt\nb.txt\n".to_string());
        let second = ToolRunView::new("tool_2".to_string(), "grep".to_string());
        let collapsed_runs = vec![first.clone(), second.clone()];
        let collapsed = estimate_tool_runs_height(&collapsed_runs, &app);

        app.expanded_tool_run_id = Some("tool_1".to_string());
        let expanded = estimate_tool_runs_height(&collapsed_runs, &app);

        assert!(expanded > collapsed);
    }

    #[test]
    fn permission_risk_marks_dangerous_shell_as_high() {
        let args = serde_json::json!({ "command": "sudo rm -rf target" });
        assert_eq!(permission_risk_label("bash", &args), "high");
    }

    #[test]
    fn permission_preview_extracts_bash_command() {
        let args = serde_json::json!({ "command": "ls -la" });
        assert_eq!(
            permission_preview("bash", &args),
            Some(("Command", "$ ls -la".to_string()))
        );
    }
}

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
