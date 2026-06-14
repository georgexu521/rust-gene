use super::{centered_rect, TuiApp};
use crate::tui::app::AppMode;
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
        // Did you mean suggestions
        let suggestions = did_you_mean_commands(&app.command_palette_query, app);
        if !suggestions.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Did you mean...",
                Style::default()
                    .fg(app.theme.tokens.fg.strong)
                    .add_modifier(Modifier::BOLD),
            )));
            for sugg in suggestions.iter().take(3) {
                lines.push(Line::from(vec![
                    Span::styled("  › ", Style::default().fg(app.theme.tokens.tone.info)),
                    Span::styled(
                        sugg.clone(),
                        Style::default().fg(app.theme.tokens.tone.brand),
                    ),
                ]));
            }
        }
    } else if items.is_empty() {
        lines.push(Line::from(Span::styled(
            "No commands registered.",
            Style::default().fg(app.theme.tokens.fg.faint),
        )));
    } else {
        let mut last_category = "";
        for (idx, cmd) in items.iter().enumerate() {
            let display_category = if app.command_palette_query.is_empty() {
                if app.recent_palette_commands.iter().any(|r| r == cmd.name) {
                    "Recently Used"
                } else if app.is_contextual_palette_command(cmd.name) {
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

            // Highlight matching characters in command name
            let name_spans = if app.command_palette_query.is_empty() {
                vec![Span::styled(format!("{:<18}", cmd.name), style)]
            } else {
                highlight_matches(
                    cmd.name,
                    &app.command_palette_query,
                    style,
                    app.theme.tokens.tone.accent,
                )
            };

            let mut spans = vec![Span::styled(
                marker,
                Style::default().fg(app.theme.tokens.tone.info),
            )];
            spans.extend(name_spans);
            spans.push(Span::styled(
                cmd.description,
                Style::default().fg(app.theme.tokens.fg.faint),
            ));
            spans.push(Span::styled(
                alias,
                Style::default().fg(app.theme.tokens.fg.faint),
            ));
            spans.push(Span::styled(
                maturity,
                Style::default().fg(app.theme.tokens.tone.warn),
            ));
            lines.push(Line::from(spans));
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
    let mode_label = match app.mode {
        AppMode::VimNormal => "Vim",
        AppMode::PermissionApproval => "Approval",
        AppMode::DiffViewer => "Diff",
        AppMode::CommandPalette => "Palette",
        AppMode::FilePicker => "Attach",
        _ => "",
    };
    let title = if app.shortcut_help_filter.is_empty() {
        if mode_label.is_empty() {
            " Shortcuts (? to close, / to filter) ".to_string()
        } else {
            format!(" {mode_label} Shortcuts (? to close, / to filter) ")
        }
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
        Line::from("  ctrl+r       prompt history/stash picker"),
        Line::from("  ctrl+m       model picker (alt+m if ctrl+m sends Enter)"),
        Line::from("  ctrl+l       provider picker (alt+l if ctrl+l sends Enter)"),
        Line::from("  ctrl+o       expand reasoning or tool details"),
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
        Line::from("  vim: j/k scroll, g top, G bottom, / search"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Sidebar",
            Style::default()
                .fg(app.theme.tokens.fg.strong)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from("  b            show/hide sidebar"),
        Line::from("  ctrl+tab     switch Sessions/Context panel"),
        Line::from("  j/k or ↑/↓   move selected session"),
        Line::from("  /            filter sessions by title/id/model"),
        Line::from("  enter        switch session"),
        Line::from("  P / D / R    pin, delete, rename session"),
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

    // Context-aware filtering by AppMode
    let mut current_section = String::new();
    let mode_sections: &[&str] = match app.mode {
        AppMode::VimNormal => &["Vim / Navigation"],
        AppMode::PermissionApproval => &["Approvals"],
        AppMode::DiffViewer => &["Diff Viewer"],
        AppMode::CommandPalette => &["Core"],
        AppMode::FilePicker => &["Core"],
        _ => &[
            "Core",
            "Vim / Navigation",
            "Sidebar",
            "Approvals",
            "Diff Viewer",
        ],
    };
    let mut context_filtered: Vec<Line> = Vec::new();
    for line in all_lines {
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        if text.is_empty() {
            if !current_section.is_empty() {
                context_filtered.push(line);
            }
            continue;
        }
        let is_header = line
            .spans
            .first()
            .map(|s| s.style.add_modifier == Modifier::BOLD)
            .unwrap_or(false);
        if is_header {
            current_section = text.clone();
        }
        if mode_sections.contains(&current_section.as_str()) || text == "Press any key to close." {
            context_filtered.push(line);
        }
    }

    // Filter lines if shortcut_help_filter is non-empty
    let filter = app.shortcut_help_filter.to_lowercase();
    let filtered: Vec<Line> = if filter.is_empty() {
        context_filtered
    } else {
        context_filtered
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

pub fn render_prompt_picker(f: &mut Frame, app: &TuiApp, area: Rect) {
    let popup_area = centered_rect(68, 42, area);
    let block = Block::default()
        .title(" Prompt History (enter use, esc close) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.tokens.tone.info))
        .style(Style::default().bg(Color::Black));

    let items = app.prompt_picker_items();
    let mut lines = Vec::new();
    if items.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            "No prompt history or stash yet.",
            Style::default().fg(app.theme.tokens.fg.faint),
        )]));
    } else {
        for (idx, (kind, label, _content)) in items.iter().enumerate() {
            let selected = idx == app.prompt_picker_selected;
            let marker = if selected { "›" } else { " " };
            let style = if selected {
                Style::default()
                    .fg(app.theme.tokens.tone.brand)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(app.theme.tokens.fg.meta)
            };
            lines.push(Line::from(vec![
                Span::styled(format!("{marker} "), style),
                Span::styled(format!("{kind:<7} "), style),
                Span::styled(label.clone(), Style::default().fg(app.theme.tokens.fg.body)),
            ]));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("↑/↓", Style::default().fg(app.theme.tokens.tone.info)),
        Span::styled(" select  ", Style::default().fg(app.theme.tokens.fg.faint)),
        Span::styled("enter", Style::default().fg(app.theme.tokens.tone.info)),
        Span::styled(
            " copy into composer  ",
            Style::default().fg(app.theme.tokens.fg.faint),
        ),
        Span::styled("esc", Style::default().fg(app.theme.tokens.tone.info)),
        Span::styled(" close", Style::default().fg(app.theme.tokens.fg.faint)),
    ]));

    f.render_widget(Clear, popup_area);
    f.render_widget(
        Paragraph::new(Text::from(lines))
            .wrap(Wrap { trim: true })
            .block(block),
        popup_area,
    );
}

/// Highlight matching characters in a command name.
fn highlight_matches(
    name: &str,
    query: &str,
    base_style: Style,
    accent: ratatui::style::Color,
) -> Vec<Span<'static>> {
    let query_lower = query.to_ascii_lowercase();
    let name_lower = name.to_ascii_lowercase();
    let mut matched = vec![false; name.chars().count()];

    let mut q_idx = 0usize;
    for (i, nc) in name_lower.chars().enumerate() {
        if let Some(qc) = query_lower.chars().nth(q_idx) {
            if nc == qc {
                matched[i] = true;
                q_idx += 1;
                if q_idx >= query_lower.chars().count() {
                    break;
                }
            }
        }
    }

    let mut spans = Vec::new();
    let mut current = String::new();
    let mut in_match = false;

    for (i, ch) in name.chars().enumerate() {
        let is_match = *matched.get(i).unwrap_or(&false);
        if is_match != in_match && !current.is_empty() {
            spans.push(Span::styled(
                current.clone(),
                if in_match {
                    base_style.fg(accent).add_modifier(Modifier::BOLD)
                } else {
                    base_style
                },
            ));
            current.clear();
        }
        in_match = is_match;
        current.push(ch);
    }

    if !current.is_empty() {
        spans.push(Span::styled(
            current,
            if in_match {
                base_style.fg(accent).add_modifier(Modifier::BOLD)
            } else {
                base_style
            },
        ));
    }

    spans
}

/// Compute edit distance and return the closest command names.
fn did_you_mean_commands(query: &str, app: &TuiApp) -> Vec<String> {
    let query_lower = query
        .to_ascii_lowercase()
        .trim_start_matches('/')
        .to_string();
    let mut candidates: Vec<(&str, usize)> = app
        .command_registry
        .commands()
        .map(|cmd| {
            let name = cmd.name.trim_start_matches('/');
            let dist = levenshtein(name, &query_lower);
            (cmd.name, dist)
        })
        .collect();
    candidates.sort_by_key(|&(_, dist)| dist);
    candidates
        .into_iter()
        .take(3)
        .map(|(name, _)| name.to_string())
        .collect()
}

/// Simple Levenshtein distance for strings.
fn levenshtein(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let n = a_chars.len();
    let m = b_chars.len();
    if n == 0 {
        return m;
    }
    if m == 0 {
        return n;
    }
    let mut prev = (0..=m).collect::<Vec<_>>();
    let mut curr = vec![0usize; m + 1];
    for i in 1..=n {
        curr[0] = i;
        for j in 1..=m {
            let cost = if a_chars[i - 1] == b_chars[j - 1] {
                0
            } else {
                1
            };
            curr[j] = (curr[j - 1] + 1).min(prev[j] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[m]
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

pub fn render_workspace_switcher(f: &mut Frame, app: &TuiApp, area: Rect) {
    let popup_area = centered_rect(72, 60, area);
    let block = Block::default()
        .title(" Workspaces ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.tokens.tone.info))
        .style(Style::default().bg(app.theme.tokens.surface.bg_elev));

    let inner = block.inner(popup_area);
    f.render_widget(Clear, popup_area);
    f.render_widget(block, popup_area);

    let items = &app.workspace_switcher_items;
    if items.is_empty() {
        let empty = Paragraph::new("No workspaces found.").style(
            Style::default()
                .fg(app.theme.tokens.fg.faint)
                .bg(app.theme.tokens.surface.bg_elev),
        );
        f.render_widget(empty, inner);
        return;
    }

    let mut lines = Vec::new();
    lines.push(Line::from(vec![
        Span::styled("Current ", Style::default().fg(app.theme.tokens.fg.faint)),
        Span::styled(
            app.workspace.display_name.clone(),
            Style::default()
                .fg(app.theme.tokens.fg.strong)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(""));

    for (idx, root) in items.iter().enumerate() {
        let selected = idx == app.workspace_switcher_selected;
        let marker = if selected { "› " } else { "  " };
        let display = std::path::Path::new(root)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| root.clone());
        let style = if selected {
            Style::default()
                .fg(app.theme.tokens.fg.strong)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(app.theme.tokens.fg.body)
        };
        lines.push(Line::from(vec![
            Span::styled(marker, style),
            Span::styled(display, style),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("enter", Style::default().fg(app.theme.tokens.tone.info)),
        Span::styled(" switch  ", Style::default().fg(app.theme.tokens.fg.faint)),
        Span::styled("esc/q", Style::default().fg(app.theme.tokens.tone.info)),
        Span::styled(" close", Style::default().fg(app.theme.tokens.fg.faint)),
    ]));

    f.render_widget(
        Paragraph::new(Text::from(lines))
            .wrap(Wrap { trim: true })
            .style(Style::default().bg(app.theme.tokens.surface.bg_elev)),
        inner,
    );
}

pub fn render_file_picker(f: &mut Frame, app: &TuiApp, area: Rect) {
    let popup_area = centered_rect(72, 70, area);
    f.render_widget(Clear, popup_area);

    let Some(state) = app.file_picker_state.as_ref() else {
        let block = Block::default()
            .title(" Attach File ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.theme.tokens.tone.info))
            .style(Style::default().bg(app.theme.tokens.surface.bg_elev));
        f.render_widget(block, popup_area);
        return;
    };

    let block = Block::default()
        .title(" Attach File Context ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.tokens.tone.info))
        .style(Style::default().bg(app.theme.tokens.surface.bg_elev));
    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let chunks = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            ratatui::layout::Constraint::Length(1),
            ratatui::layout::Constraint::Min(3),
            ratatui::layout::Constraint::Length(1),
        ])
        .split(inner);

    let filter_label = if app.file_picker_filtering {
        format!(" /{}_", state.filter_query())
    } else if state.filter_query().is_empty() {
        " / filter files".to_string()
    } else {
        format!(" /{}  (/ edit, esc/q close)", state.filter_query())
    };
    let filter_style = if app.file_picker_filtering {
        Style::default().fg(app.theme.tokens.tone.info)
    } else {
        Style::default().fg(app.theme.tokens.fg.faint)
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(filter_label, filter_style)))
            .style(Style::default().bg(app.theme.tokens.surface.bg_elev)),
        chunks[0],
    );

    let (list, mut list_state) = state.render(chunks[1]);
    f.render_stateful_widget(list, chunks[1], &mut list_state);

    let hint = Paragraph::new(Line::from(vec![
        Span::styled("/", Style::default().fg(app.theme.tokens.tone.info)),
        Span::styled(" filter  ", Style::default().fg(app.theme.tokens.fg.faint)),
        Span::styled("enter", Style::default().fg(app.theme.tokens.tone.info)),
        Span::styled(
            " attach file/open dir  ",
            Style::default().fg(app.theme.tokens.fg.faint),
        ),
        Span::styled("space", Style::default().fg(app.theme.tokens.tone.info)),
        Span::styled(" toggle  ", Style::default().fg(app.theme.tokens.fg.faint)),
        Span::styled("esc/q", Style::default().fg(app.theme.tokens.tone.info)),
        Span::styled(" close", Style::default().fg(app.theme.tokens.fg.faint)),
    ]))
    .style(Style::default().bg(app.theme.tokens.surface.bg_elev));
    f.render_widget(hint, chunks[2]);
}
