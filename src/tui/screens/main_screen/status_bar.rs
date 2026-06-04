use crate::tui::app::{StatusBarDensity, TuiApp};
use ratatui::{
    layout::{Alignment, Rect},
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

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
        let provider_label = app.facade_snapshot.provider_request.status_label();
        let label = if !provider_label.is_empty() {
            provider_label
        } else {
            runtime
                .current_tool_label
                .clone()
                .unwrap_or_else(|| "Thinking".to_string())
        };
        let spinner_color = if app.facade_snapshot.provider_request.is_known_slow_path {
            app.theme.tokens.tone.err
        } else if app.facade_snapshot.provider_request.phase.is_active() {
            app.theme.tokens.tone.warn
        } else {
            app.theme.tokens.tone.warn
        };
        parts.push(Span::styled(
            format!("◌ {}", label),
            Style::default().fg(spinner_color),
        ));
        if app.facade_snapshot.provider_request.phase.is_active() {
            let elapsed = app.facade_snapshot.provider_request.elapsed_ms;
            if elapsed > 0 {
                parts.push(Span::styled(
                    format!("{:.1}s", elapsed as f64 / 1000.0),
                    Style::default().fg(app.theme.tokens.fg.faint),
                ));
            }
        }
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
