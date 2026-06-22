//! Main TUI screen support.
//!
//! Splits composer, approvals, popups, and status bar rendering into focused modules.

use crate::tui::{
    app::TuiApp,
    view_model::footer::{footer_items, FooterItem, FooterTone},
};
use ratatui::{
    layout::{Alignment, Rect},
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// 渲染状态栏（Reasonix 风格：mode glyph · session · cost · cache · ctx）
pub fn render_status_bar(f: &mut Frame, app: &TuiApp, area: Rect) {
    let parts = fit_footer_items(footer_items(app), usize::from(area.width))
        .into_iter()
        .map(|item| Span::styled(item.label, Style::default().fg(tone_color(item.tone, app))))
        .collect::<Vec<_>>();

    // 用 " · " 连接所有部分
    let mut spans = vec![Span::styled(" ", Style::default())];
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

fn fit_footer_items(mut items: Vec<FooterItem>, width: usize) -> Vec<FooterItem> {
    if width == 0 {
        return Vec::new();
    }

    while footer_width(&items) > width {
        let Some(index) = drop_candidate(&items) else {
            break;
        };
        items.remove(index);
    }

    while footer_width(&items) > width {
        let Some(index) = truncation_candidate(&items) else {
            break;
        };
        let current = display_width(&items[index].label);
        let overflow = footer_width(&items).saturating_sub(width);
        let target = current.saturating_sub(overflow).max(1);
        if target >= current {
            break;
        }
        items[index].label = truncate_display_width(&items[index].label, target);
    }

    while footer_width(&items) > width {
        let Some(index) = forced_drop_candidate(&items) else {
            break;
        };
        items.remove(index);
    }

    items
}

fn drop_candidate(items: &[FooterItem]) -> Option<usize> {
    items
        .iter()
        .enumerate()
        .filter(|(index, item)| footer_priority(item, *index) < 900)
        .min_by_key(|(index, item)| (footer_priority(item, *index), *index))
        .map(|(index, _)| index)
}

fn truncation_candidate(items: &[FooterItem]) -> Option<usize> {
    items
        .iter()
        .enumerate()
        .filter(|(index, item)| *index != 0 && item.label != "? shortcuts")
        .max_by_key(|(_, item)| (item.label.contains(" / "), display_width(&item.label)))
        .map(|(index, _)| index)
}

fn forced_drop_candidate(items: &[FooterItem]) -> Option<usize> {
    items
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != 0)
        .min_by_key(|(index, item)| (forced_drop_priority(item, *index), *index))
        .map(|(index, _)| index)
}

fn forced_drop_priority(item: &FooterItem, _index: usize) -> u16 {
    if item.label == "? shortcuts" {
        0
    } else if item.label.contains(" / ") {
        1
    } else {
        2
    }
}

fn footer_priority(item: &FooterItem, index: usize) -> u16 {
    if index == 0 {
        return 1_000;
    }
    if item.label.contains(" / ") {
        return 950;
    }
    if item.label == "? shortcuts" {
        return 900;
    }
    match item.label.as_str() {
        "auto" | "ask" | "review" | "read-only" => 800,
        "vim" | "mem" | "mem:bal" | "mem:strict" | "mem:pref" | "mem:off" => 650,
        label if label.starts_with('v') => 300,
        label if label.starts_with("last ") => 250,
        label if label.starts_with("mcp:") => 240,
        label
            if label.starts_with("provider:")
                || label.starts_with("ctx ")
                || label.starts_with("changed:")
                || label.starts_with("validation:")
                || label.starts_with("scroll:")
                || label.starts_with("tools:")
                || label.starts_with("msgs:") =>
        {
            200
        }
        _ => 400,
    }
}

fn footer_width(items: &[FooterItem]) -> usize {
    let labels = items
        .iter()
        .map(|item| display_width(&item.label))
        .sum::<usize>();
    let separators = items.len().saturating_sub(1) * 3;
    1 + labels + separators
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
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
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
    UnicodeWidthStr::width(value)
}

fn tone_color(tone: FooterTone, app: &TuiApp) -> ratatui::style::Color {
    match tone {
        FooterTone::Mode => match app.agent_mode {
            crate::engine::agent_mode::AgentMode::Auto
            | crate::engine::agent_mode::AgentMode::Build => app.theme.tokens.tone.ok,
            crate::engine::agent_mode::AgentMode::Plan
            | crate::engine::agent_mode::AgentMode::Explore => app.theme.tokens.tone.accent,
            crate::engine::agent_mode::AgentMode::Review => app.theme.tokens.tone.warn,
        },
        FooterTone::Error => app.theme.tokens.tone.err,
        FooterTone::Warning => app.theme.tokens.tone.warn,
        FooterTone::Faint => app.theme.tokens.fg.faint,
        FooterTone::Info => app.theme.tokens.tone.info,
        FooterTone::Accent => app.theme.tokens.tone.accent,
        FooterTone::Ok => app.theme.tokens.tone.ok,
        FooterTone::Violet => app.theme.tokens.tone.violet,
    }
}
