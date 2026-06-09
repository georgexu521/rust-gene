use super::{centered_rect, TuiApp};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

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
    if items.is_empty() && !app.command_palette_query.is_empty() {
        lines.push(Line::from(Span::styled(
            format!("No command matched '{}'.", app.command_palette_query),
            Style::default().fg(app.theme.tokens.fg.faint),
        )));
        lines.push(Line::from(Span::styled(
            "Try: /quick /doctor /permissions /session /model /provider",
            Style::default().fg(app.theme.tokens.fg.faint),
        )));
    } else if items.is_empty() {
        lines.push(Line::from(Span::styled(
            "No commands registered.",
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
    let title = if app.shortcut_help_filter.is_empty() {
        " Shortcuts (? to close, / to filter) ".to_string()
    } else {
        format!(" Shortcuts (filter: {}) ", app.shortcut_help_filter)
    };
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.tokens.tone.info))
        .style(Style::default().bg(Color::Black));
    let kb = &app.keybindings;
    let all_lines = vec![
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
            "Vim / Navigation",
            Style::default()
                .fg(app.theme.tokens.fg.strong)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from("  ↑/↓          move cursor or scroll at edge"),
        Line::from("  pageup/down  half-page scroll"),
        Line::from(format!("  {}       toggle vim mode", kb.toggle_vim_mode)),
        Line::from("  vim: j/k scroll, g top, G bottom, / search, b sidebar"),
        Line::from("  vim: Enter switch session, P pin, D delete, R rename"),
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
        Line::from("  s/p/a/x       session/project/global allow/deny"),
        Line::from("  esc           cancel without saving a rule"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Diff Viewer",
            Style::default()
                .fg(app.theme.tokens.fg.strong)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from("  n/p           next/prev hunk"),
        Line::from("  Tab           next file"),
        Line::from("  Esc/q         close diff"),
        Line::from("  ↑/↓/PgUp/PgDn scroll/page"),
        Line::from(""),
        Line::from(Span::styled(
            "Press any key to close.",
            Style::default().fg(app.theme.tokens.fg.faint),
        )),
    ];

    // Filter lines if shortcut_help_filter is non-empty
    let filter = app.shortcut_help_filter.to_lowercase();
    let filtered: Vec<Line> = if filter.is_empty() {
        all_lines
    } else {
        all_lines
            .into_iter()
            .filter(|line| {
                // Always keep section headers and separators
                let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
                text.is_empty()
                    || text.to_lowercase().contains(&filter)
                    || line
                        .spans
                        .first()
                        .map(|s| s.style.add_modifier == Modifier::BOLD)
                        .unwrap_or(false)
            })
            .collect()
    };

    f.render_widget(Clear, popup_area);
    f.render_widget(
        Paragraph::new(Text::from(filtered))
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
