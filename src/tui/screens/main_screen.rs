//! 主屏幕
//!
//! 包含聊天区、输入区、状态栏的渲染

use crate::tui::{app::TuiApp, components::message};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

/// 渲染聊天区域
pub fn render_chat_area(f: &mut Frame, app: &TuiApp, area: Rect) {
    // 创建块
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.border))
        .title(format!(
            " Chat {} ",
            if app.is_querying { "(Thinking...)" } else { "" }
        ));

    let inner_area = block.inner(area);
    f.render_widget(block, area);

    // 如果有消息，渲染它们
    if app.messages.is_empty() {
        let empty_text = Paragraph::new("No messages yet. Start typing below!").style(
            Style::default()
                .fg(app.theme.text_dim)
                .add_modifier(Modifier::ITALIC),
        );
        f.render_widget(empty_text, inner_area);
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

    // 渲染每条消息
    let mut current_y = inner_area.y;
    let max_y = inner_area.y + inner_area.height;

    for (_idx, msg) in messages.iter().enumerate().skip(app.scroll_offset) {
        // 检查是否还有空间
        if current_y >= max_y {
            break;
        }

        let msg_height = estimate_message_height(msg, inner_area.width as usize, &app.theme);
        let msg_area = Rect {
            x: inner_area.x,
            y: current_y,
            width: inner_area.width,
            height: (msg_height as u16).min(max_y - current_y),
        };

        // 渲染消息
        let paragraph = message::render_message(msg, inner_area.width as usize, &app.theme);
        f.render_widget(paragraph, msg_area);

        current_y += msg_height as u16;
    }

    // 渲染滚动指示器
    if app.scroll_offset > 0 {
        let scroll_indicator = Paragraph::new("↑ more above").style(
            Style::default()
                .fg(app.theme.text_dim)
                .add_modifier(Modifier::ITALIC),
        );
        f.render_widget(
            scroll_indicator,
            Rect {
                x: inner_area.x,
                y: inner_area.y,
                width: inner_area.width,
                height: 1,
            },
        );
    }
}

/// 渲染输入区域
pub fn render_input_area(f: &mut Frame, app: &TuiApp, area: Rect) {
    let title = match app.mode {
        crate::tui::app::AppMode::VimNormal => " -- NORMAL -- ",
        _ => {
            if app.vim_mode {
                " -- INSERT -- "
            } else {
                " Input "
            }
        }
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.border))
        .title(title);

    let inner_area = block.inner(area);
    f.render_widget(block, area);

    // 渲染输入内容
    let input_text = app.input.value();

    // 如果正在查询，显示提示
    let (display_text, style) = if app.is_querying {
        (
            Text::from("Waiting for response..."),
            Style::default().fg(app.theme.text),
        )
    } else if app.mode == crate::tui::app::AppMode::VimNormal {
        let text =
            Text::from("Vim Normal Mode: j/k scroll, i insert, : command, Ctrl+V toggle off");
        (text, Style::default().fg(app.theme.text_dim))
    } else if input_text.is_empty() {
        let text = Text::from("Type your message here... (Shift+Enter for newline, Enter to send)");
        (text, Style::default().fg(app.theme.text_dim))
    } else {
        let lines: Vec<Line> = input_text
            .lines()
            .map(|line| Line::from(line.to_string()))
            .collect();
        (Text::from(lines), Style::default().fg(app.theme.text))
    };

    let paragraph = Paragraph::new(display_text).style(style);
    f.render_widget(paragraph, inner_area);

    // 设置光标位置（如果不是正在查询）
    if !app.is_querying {
        let (cursor_line, cursor_col) = app.input.cursor_line_column();
        let cursor_x = inner_area.x + cursor_col as u16;
        let cursor_y = inner_area.y + cursor_line as u16;
        f.set_cursor_position((
            cursor_x.min(inner_area.x + inner_area.width - 1),
            cursor_y.min(inner_area.y + inner_area.height - 1),
        ));
    }
}

/// 渲染状态栏
pub fn render_status_bar(f: &mut Frame, app: &TuiApp, area: Rect) {
    // 分割状态栏为左右两部分
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // 左侧：状态信息
    let spinner_frames = ['⠻', '⠹', '⠹', '⠸', '⠼', '⠴', '⠶', '⠷', '⠷', '⠿'];
    let spinner_idx = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        / 100) as usize
        % spinner_frames.len();

    let status_text = if app.is_querying {
        vec![
            Span::styled(
                format!("{} ", spinner_frames[spinner_idx]),
                Style::default().fg(app.theme.status_thinking),
            ),
            Span::styled("Thinking...", Style::default().fg(app.theme.text)),
        ]
    } else if let Some(ref error) = app.error_message {
        vec![
            Span::styled("✗ ", Style::default().fg(app.theme.error)),
            Span::styled(error.clone(), Style::default().fg(app.theme.error)),
        ]
    } else {
        let mut spans = vec![
            Span::styled("✓ ", Style::default().fg(app.theme.success)),
            Span::styled("Ready", Style::default().fg(app.theme.text)),
        ];
        if let Some(ref wt) = app.worktree_manager {
            if let Some(name) = wt.try_active_worktree_name() {
                spans.push(Span::styled(" | ", Style::default().fg(app.theme.border)));
                spans.push(Span::styled(
                    format!("[worktree: {}]", name),
                    Style::default().fg(app.theme.status_worktree),
                ));
            }
        }
        spans
    };

    let status = Paragraph::new(Line::from(status_text));
    f.render_widget(status, chunks[0]);

    // 右侧：统计和提示
    let mut right_text = Vec::new();
    if app.vim_mode {
        right_text.push(Span::styled(
            "[VIM]",
            Style::default()
                .fg(app.theme.status_vim)
                .add_modifier(Modifier::BOLD),
        ));
        right_text.push(Span::styled(" | ", Style::default().fg(app.theme.border)));
    }
    if app.is_querying {
        right_text.push(Span::styled(
            "Esc: cancel",
            Style::default().fg(app.theme.text_dim),
        ));
    } else {
        if app.paused {
            right_text.push(Span::styled(
                "[PAUSED]",
                Style::default().fg(app.theme.warning).add_modifier(Modifier::BOLD),
            ));
            right_text.push(Span::styled(" | ", Style::default().fg(app.theme.border)));
        }
        if app.focus_mode {
            right_text.push(Span::styled(
                "[FOCUS]",
                Style::default().fg(app.theme.info).add_modifier(Modifier::BOLD),
            ));
            right_text.push(Span::styled(" | ", Style::default().fg(app.theme.border)));
        }
        // 显示 Plan Mode 状态
        if let Some(plan_state_label) = app.plan_mode_status_label() {
            right_text.push(Span::styled(
                plan_state_label,
                Style::default().fg(app.theme.warning).add_modifier(Modifier::BOLD),
            ));
            right_text.push(Span::styled(" | ", Style::default().fg(app.theme.border)));
        }
        right_text.push(Span::styled(
            format!(
                "{} / {}",
                app.current_provider_label(),
                app.current_model_label()
            ),
            Style::default().fg(app.theme.info),
        ));
        right_text.push(Span::styled(" | ", Style::default().fg(app.theme.border)));
        right_text.push(Span::styled(
            format!("{} msgs", app.message_count()),
            Style::default().fg(app.theme.text_dim),
        ));
        right_text.push(Span::styled(" | ", Style::default().fg(app.theme.border)));
        right_text.push(Span::styled("/help", Style::default().fg(app.theme.info)));
        right_text.push(Span::styled(" | ", Style::default().fg(app.theme.border)));
        right_text.push(Span::styled(
            "Ctrl+C: quit",
            Style::default().fg(app.theme.text_dim),
        ));
    }
    let stats = Paragraph::new(Line::from(right_text)).alignment(Alignment::Right);
    f.render_widget(stats, chunks[1]);
}

/// 渲染消息搜索弹窗
pub fn render_message_search(f: &mut Frame, app: &TuiApp, area: Rect) {
    use ratatui::widgets::{Block, Borders, Clear, Paragraph};

    let search_height = (area.height / 2).max(10).min(20);
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
) -> usize {
    let base_height = 3; // header + blank line + trailing blank
    let effective_width = width.saturating_sub(4).max(1);

    let content_height = if msg.role == crate::state::MessageRole::Assistant {
        // Assistant messages: markdown rendering with code blocks, lists, etc.
        let mut lines = 0;
        let mut in_code_block = false;
        for line in msg.content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("```") {
                in_code_block = !in_code_block;
                lines += 1; // fence line
            } else if in_code_block {
                lines += 1;
            } else if trimmed.starts_with('#') {
                // Heading: rarely wraps
                lines += 1;
            } else if trimmed.is_empty() {
                lines += 1;
            } else {
                let dw = unicode_width::UnicodeWidthStr::width(line);
                lines += (dw + effective_width - 1) / effective_width;
            }
        }
        lines.max(1)
    } else {
        // User/System/Tool: simple text wrapping
        msg.content
            .lines()
            .map(|line| {
                let dw = unicode_width::UnicodeWidthStr::width(line);
                (dw + effective_width - 1) / effective_width
            })
            .sum::<usize>()
            .max(1)
    };

    base_height + content_height
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

/// 渲染权限审批弹窗
pub fn render_permission_approval(
    f: &mut Frame,
    req: &crate::engine::conversation_loop::ToolApprovalRequest,
    area: Rect,
) {
    let popup_area = centered_rect(70, 50, area);

    let block = Block::default()
        .title(" Tool Permission Request ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .style(Style::default().bg(Color::Black));

    let mut lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("Tool: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(req.tool_call.name.clone(), Style::default().fg(Color::Cyan)),
        ]),
        Line::from(""),
    ];

    if let Ok(args) = serde_json::to_string_pretty(&req.tool_call.arguments) {
        lines.push(Line::from(vec![Span::styled(
            "Arguments:",
            Style::default().add_modifier(Modifier::BOLD),
        )]));
        for line in args.lines() {
            lines.push(Line::from(Span::styled(
                format!("  {}", line),
                Style::default().fg(Color::DarkGray),
            )));
        }
        lines.push(Line::from(""));
    }

    lines.push(Line::from(vec![
        Span::styled("Prompt: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::styled(req.prompt.clone(), Style::default().fg(Color::White)),
    ]));

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(
            "y",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" = Allow  ", Style::default().fg(Color::Gray)),
        Span::styled(
            "n",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" = Deny", Style::default().fg(Color::Gray)),
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
            Span::styled(" = View Diff/Preview", Style::default().fg(Color::Gray)),
        ]));
    }

    let text = Text::from(lines);
    let paragraph = Paragraph::new(text).wrap(Wrap { trim: true }).block(block);

    f.render_widget(Clear, popup_area);
    f.render_widget(paragraph, popup_area);
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
