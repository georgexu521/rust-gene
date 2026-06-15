//! Dedicated renderer for the `/connect` provider setup wizard.

use super::{centered_rect, TuiApp};
use crate::tui::app::connect_wizard::{ConnectStep, WizardStatus};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

pub fn render_connect_wizard(f: &mut Frame, app: &TuiApp, area: Rect) {
    let popup_area = centered_rect(74, 60, area);
    let block = Block::default()
        .title(" Connect Provider ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.tokens.tone.info))
        .style(Style::default().bg(Color::Black));

    let Some(wizard) = app.connect_wizard_state.as_ref() else {
        f.render_widget(Clear, popup_area);
        f.render_widget(Paragraph::new("").block(block), popup_area);
        return;
    };

    let mut lines = vec![Line::from("")];

    match wizard.step {
        ConnectStep::SelectProvider => {
            lines.push(Line::from(vec![
                Span::styled(
                    "Select a provider ",
                    Style::default().fg(app.theme.tokens.fg.body),
                ),
                Span::styled(
                    "(↑/↓ Enter)",
                    Style::default().fg(app.theme.tokens.fg.faint),
                ),
            ]));
            lines.push(Line::from(vec![Span::styled(
                if wizard.query.is_empty() {
                    "type to filter"
                } else {
                    wizard.query.as_str()
                },
                Style::default().fg(if wizard.query.is_empty() {
                    app.theme.tokens.fg.faint
                } else {
                    app.theme.tokens.fg.body
                }),
            )]));
            lines.push(Line::from(""));

            let choices = wizard.provider_choices();
            if choices.is_empty() {
                lines.push(Line::from(Span::styled(
                    "No providers matched.",
                    Style::default().fg(app.theme.tokens.fg.faint),
                )));
            } else {
                for (idx, entry) in choices.iter().enumerate() {
                    let selected = idx == wizard.selected;
                    let marker = if selected { "› " } else { "  " };
                    let style = Style::default().fg(app.theme.tokens.fg.body);
                    let configured = entry.key_env_vars.iter().any(|v| {
                        std::env::var(v)
                            .map(|val| !val.trim().is_empty())
                            .unwrap_or(false)
                    });
                    let note = if configured {
                        "configured"
                    } else {
                        "not configured"
                    };
                    lines.push(Line::from(vec![
                        Span::styled(marker, Style::default().fg(app.theme.tokens.tone.info)),
                        Span::styled(format!("{:<12}", entry.label), style),
                        Span::styled(
                            format!("{:<24}", entry.default_model),
                            Style::default().fg(app.theme.tokens.fg.faint),
                        ),
                        Span::styled(note, Style::default().fg(app.theme.tokens.fg.faint)),
                    ]));
                }
            }
        }
        ConnectStep::InputKey => {
            let provider_label = wizard
                .selected_provider()
                .map(|e| e.label)
                .unwrap_or_else(|| "Provider".to_string());
            let env_var = wizard
                .selected_key_env_var()
                .unwrap_or_else(|| "API_KEY".to_string());
            lines.push(Line::from(vec![
                Span::styled(
                    format!("Enter API key for {} ", provider_label),
                    Style::default().fg(app.theme.tokens.fg.body),
                ),
                Span::styled(
                    "(Enter to save, Esc to cancel)",
                    Style::default().fg(app.theme.tokens.fg.faint),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Env var: ", Style::default().fg(app.theme.tokens.fg.faint)),
                Span::styled(
                    env_var,
                    Style::default()
                        .fg(app.theme.tokens.fg.strong)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
            lines.push(Line::from(""));
            lines.push(Line::from(vec![Span::styled(
                if wizard.input_buffer.is_empty() {
                    "paste key here"
                } else if wizard.mask_input {
                    "●"
                } else {
                    wizard.input_buffer.as_str()
                },
                Style::default()
                    .fg(if wizard.input_buffer.is_empty() {
                        app.theme.tokens.fg.faint
                    } else {
                        app.theme.tokens.fg.body
                    })
                    .add_modifier(if wizard.input_buffer.is_empty() {
                        Modifier::empty()
                    } else {
                        Modifier::BOLD
                    }),
            )]));
            lines.push(Line::from(Span::styled(
                "Ctrl+T to toggle visibility",
                Style::default().fg(app.theme.tokens.fg.faint),
            )));
        }
        ConnectStep::Validating => {
            lines.push(Line::from(Span::styled(
                "Validating credential...",
                Style::default().fg(app.theme.tokens.tone.info),
            )));
        }
        ConnectStep::Done => {
            let provider_label = wizard
                .selected_provider()
                .map(|e| e.label)
                .unwrap_or_else(|| "Provider".to_string());
            match &wizard.status {
                WizardStatus::None => {
                    lines.push(Line::from("Done."));
                }
                WizardStatus::Success(msg) => {
                    lines.push(Line::from(vec![Span::styled(
                        format!("{} connected", provider_label),
                        Style::default()
                            .fg(app.theme.tokens.tone.ok)
                            .add_modifier(Modifier::BOLD),
                    )]));
                    lines.push(Line::from(Span::styled(
                        msg.clone(),
                        Style::default().fg(app.theme.tokens.fg.body),
                    )));
                    lines.push(Line::from(Span::styled(
                        "Press Enter or Esc to close.",
                        Style::default().fg(app.theme.tokens.fg.faint),
                    )));
                }
                WizardStatus::Error(err) => {
                    lines.push(Line::from(vec![Span::styled(
                        "Connection failed",
                        Style::default()
                            .fg(app.theme.tokens.tone.err)
                            .add_modifier(Modifier::BOLD),
                    )]));
                    lines.push(Line::from(Span::styled(
                        err.clone(),
                        Style::default().fg(app.theme.tokens.fg.body),
                    )));
                    lines.push(Line::from(Span::styled(
                        "Press Esc to close, Enter to try again.",
                        Style::default().fg(app.theme.tokens.fg.faint),
                    )));
                }
            }
        }
    }

    let text = Text::from(lines);
    let paragraph = Paragraph::new(text).wrap(Wrap { trim: true }).block(block);
    f.render_widget(Clear, popup_area);
    f.render_widget(paragraph, popup_area);
}
