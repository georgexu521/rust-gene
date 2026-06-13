use crate::tui::{
    app::{StatusBarDensity, TuiApp},
    tool_view::ToolRunStatus,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FooterTone {
    Mode,
    Error,
    Warning,
    Faint,
    Info,
    Accent,
    Ok,
    Violet,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FooterItem {
    pub label: String,
    pub tone: FooterTone,
}

impl FooterItem {
    fn new(label: impl Into<String>, tone: FooterTone) -> Self {
        Self {
            label: label.into(),
            tone,
        }
    }
}

pub fn footer_items(app: &TuiApp) -> Vec<FooterItem> {
    let mut items = Vec::new();
    let runtime = app.runtime_status_snapshot_now();

    if let Some(ref error) = runtime.last_error {
        items.push(FooterItem::new(format!("✗ {}", error), FooterTone::Error));
    } else {
        items.push(FooterItem::new(
            format!("{} {}", mode_glyph(app), app.current_agent_mode_label()),
            FooterTone::Mode,
        ));
    }

    if let Some(session_id) = app.session_manager.current_session_id() {
        items.push(FooterItem::new(
            short_session_id(session_id),
            FooterTone::Faint,
        ));
    }

    let permission_label = app.current_permission_label();
    if permission_label != default_permission_label() {
        items.push(FooterItem::new(permission_label, FooterTone::Warning));
    }
    items.push(FooterItem::new(
        format!(
            "{} / {}",
            app.current_provider_label(),
            app.current_model_label()
        ),
        FooterTone::Faint,
    ));

    if app.status_bar_density != StatusBarDensity::Compact {
        if let Some(usage) = app.stream_usage_label() {
            items.push(FooterItem::new(
                format!("last {}", usage),
                FooterTone::Faint,
            ));
        }
        if runtime.mcp_server_count > 0 {
            items.push(FooterItem::new(
                format!(
                    "mcp:{}/{}",
                    runtime.mcp_available_count, runtime.mcp_server_count
                ),
                FooterTone::Faint,
            ));
        }
    }

    if app.vim_mode {
        items.push(FooterItem::new("vim", FooterTone::Violet));
    }

    if app.memory_use {
        items.push(FooterItem::new(
            memory_label(app.memory_recall_mode.as_str()),
            FooterTone::Info,
        ));
    }

    if app.status_bar_density == StatusBarDensity::Debug {
        push_debug_items(app, &runtime, &mut items);
    }

    items.push(FooterItem::new("? shortcuts", FooterTone::Faint));

    items
}

fn push_debug_items(
    app: &TuiApp,
    runtime: &crate::state::RuntimeStatusSnapshot,
    items: &mut Vec<FooterItem>,
) {
    if let Some(provider) = provider_debug_label(app) {
        items.push(FooterItem::new(provider, FooterTone::Faint));
    }
    if let Some(context) = context_usage_label(app) {
        items.push(FooterItem::new(context, FooterTone::Ok));
    }
    let changed = changed_file_count(app);
    if changed > 0 {
        items.push(FooterItem::new(
            format!("changed:{}", changed),
            FooterTone::Accent,
        ));
    }
    if let Some(validation) = last_validation_label(app) {
        let tone = match validation.as_str() {
            "tested" | "verified" => FooterTone::Ok,
            "failed" => FooterTone::Error,
            _ => FooterTone::Warning,
        };
        items.push(FooterItem::new(format!("validation:{}", validation), tone));
    }
    items.push(FooterItem::new(
        format!("scroll:{}", app.scroll_offset),
        FooterTone::Faint,
    ));
    items.push(FooterItem::new(
        format!(
            "tools:{}/{}",
            runtime.active_tool_count, runtime.total_tools
        ),
        FooterTone::Faint,
    ));
    items.push(FooterItem::new(
        format!("msgs:{}", runtime.messages),
        FooterTone::Faint,
    ));
    items.push(FooterItem::new(
        format!("v{}", env!("CARGO_PKG_VERSION")),
        FooterTone::Faint,
    ));
}

fn mode_glyph(app: &TuiApp) -> &'static str {
    match app.agent_mode {
        crate::engine::agent_mode::AgentMode::Auto => "●",
        crate::engine::agent_mode::AgentMode::Build => "●",
        crate::engine::agent_mode::AgentMode::Plan => "⊞",
        crate::engine::agent_mode::AgentMode::Explore => "⊙",
        crate::engine::agent_mode::AgentMode::Review => "◐",
    }
}

fn short_session_id(session_id: &str) -> String {
    if session_id.len() > 8 {
        session_id[..8].to_string()
    } else {
        session_id.to_string()
    }
}

fn memory_label(mode: &str) -> &'static str {
    match mode {
        "off" => "mem:off",
        "strict" => "mem:strict",
        "balanced" => "mem:bal",
        "preference-only" => "mem:pref",
        _ => "mem",
    }
}

fn default_permission_label() -> &'static str {
    "auto"
}

fn provider_debug_label(app: &TuiApp) -> Option<String> {
    let provider = &app.facade_snapshot.provider_request;
    provider.phase.is_active().then(|| {
        let label = provider.status_label();
        if label.is_empty() {
            "provider:active".to_string()
        } else {
            format!("provider:{label}")
        }
    })
}

fn context_usage_label(app: &TuiApp) -> Option<String> {
    let usage = app.stream_usage_snapshot?;
    let cap: u32 = 128_000;
    let used = usage.prompt_tokens;
    let ratio = (used as f64 / cap as f64).min(1.0);
    let pct = (ratio * 100.0) as u32;
    let filled = (ratio * 8.0).round() as usize;
    let empty = 8usize.saturating_sub(filled);
    let bar = "█".repeat(filled) + &"░".repeat(empty);
    Some(format!("ctx {bar} {pct}%"))
}

fn changed_file_count(app: &TuiApp) -> usize {
    app.tool_runs_snapshot
        .iter()
        .filter(|run| matches!(run.name.as_str(), "file_write" | "file_edit" | "file_patch"))
        .count()
}

fn last_validation_label(app: &TuiApp) -> Option<String> {
    let has_tests = app.tool_runs_snapshot.iter().any(|run| {
        (run.name == "run_tests" || run.name == "bash")
            && matches!(run.status, ToolRunStatus::Completed)
    });
    let has_edits = app.tool_runs_snapshot.iter().any(|run| {
        matches!(run.name.as_str(), "file_write" | "file_edit" | "file_patch")
            && matches!(run.status, ToolRunStatus::Completed)
    });
    let has_failures = app
        .tool_runs_snapshot
        .iter()
        .any(|run| matches!(run.status, ToolRunStatus::Failed | ToolRunStatus::TimedOut));

    if has_edits && !has_tests && !has_failures {
        Some("pending".to_string())
    } else if has_tests && !has_failures {
        Some("tested".to_string())
    } else if has_failures {
        Some("failed".to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::runtime_facade::ProviderPhase;

    #[test]
    fn normal_footer_does_not_duplicate_active_provider_wait() {
        let mut app = TuiApp::new();
        app.is_querying = true;
        app.facade_snapshot.provider_request.phase = ProviderPhase::Started;
        app.facade_snapshot.provider_request.provider_family =
            Some("openai_compatible".to_string());

        let labels = footer_items(&app)
            .into_iter()
            .map(|item| item.label)
            .collect::<Vec<_>>();

        assert!(!labels.iter().any(|label| label.contains("waiting on")));
        assert!(labels.iter().any(|label| label.contains("? shortcuts")));
    }

    #[test]
    fn debug_footer_keeps_diagnostics_behind_density() {
        let mut app = TuiApp::new();
        app.set_status_bar_density(StatusBarDensity::Debug);
        app.facade_snapshot.provider_request.phase = ProviderPhase::Started;
        app.facade_snapshot.provider_request.provider_family = Some("deepseek".to_string());

        let labels = footer_items(&app)
            .into_iter()
            .map(|item| item.label)
            .collect::<Vec<_>>();

        assert!(labels.iter().any(|label| label.contains("provider:")));
        assert!(labels.iter().any(|label| label.starts_with("tools:")));
        assert!(labels.iter().any(|label| label.starts_with("msgs:")));
        assert!(labels
            .iter()
            .any(|label| label == &format!("v{}", env!("CARGO_PKG_VERSION"))));
    }

    #[test]
    fn normal_footer_hides_default_permission_auto_to_avoid_mode_duplication() {
        let app = TuiApp::new();

        let labels = footer_items(&app)
            .into_iter()
            .map(|item| item.label)
            .collect::<Vec<_>>();

        assert!(labels.iter().any(|label| label == "● auto"));
        assert!(!labels.iter().any(|label| label == "auto"));
    }

    #[test]
    fn normal_footer_keeps_version_out_of_the_daily_surface() {
        let app = TuiApp::new();

        let labels = footer_items(&app)
            .into_iter()
            .map(|item| item.label)
            .collect::<Vec<_>>();

        assert!(!labels
            .iter()
            .any(|label| label == &format!("v{}", env!("CARGO_PKG_VERSION"))));
        assert!(labels.iter().any(|label| label == "? shortcuts"));
    }
}
