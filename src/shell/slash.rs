//! CLI wrappers for slash command handlers that only need the session manager.
//!
//! These functions extract the session manager from a `ShellHost` and call the
//! underlying TUI handler logic. They exist because some handlers only touch
//! session state and do not require Ratatui widgets.

use crate::engine::streaming::StreamingQueryEngine;
use crate::shell::host::ShellHost;

pub fn handle_undo(host: &mut dyn ShellHost, args: &str) -> String {
    let session_id = match host.session_manager().current_session_id() {
        Some(id) => id,
        None => return "No active session.".to_string(),
    };

    let n = match crate::tui::slash_handler::utils::parse_optional_count(args, "/undo") {
        Ok(v) => v,
        Err(e) => return e,
    };

    let mut successes = 0usize;
    let mut last_error = None::<String>;
    for _ in 0..n {
        match host.session_manager().rewind_last_edit(session_id) {
            Ok(_) => successes += 1,
            Err(e) => {
                last_error = Some(e.to_string());
                break;
            }
        }
    }

    if successes == 0 {
        format!(
            "Nothing to undo or undo failed{}",
            last_error.map(|e| format!(": {e}")).unwrap_or_default()
        )
    } else {
        format!(
            "Undid {successes} edit{}.",
            if successes > 1 { "s" } else { "" }
        )
    }
}

pub fn handle_redo(host: &mut dyn ShellHost, args: &str) -> String {
    let session_id = match host.session_manager().current_session_id() {
        Some(id) => id,
        None => return "No active session.".to_string(),
    };

    let n = match crate::tui::slash_handler::utils::parse_optional_count(args, "/redo") {
        Ok(v) => v,
        Err(e) => return e,
    };

    let mut successes = 0usize;
    let mut last_error = None::<String>;
    for _ in 0..n {
        match host.session_manager().redo_last_edit(session_id) {
            Ok(_) => successes += 1,
            Err(e) => {
                last_error = Some(e.to_string());
                break;
            }
        }
    }

    if successes == 0 {
        format!(
            "Nothing to redo or redo failed{}",
            last_error.map(|e| format!(": {e}")).unwrap_or_default()
        )
    } else {
        format!(
            "Redone {successes} edit{}.",
            if successes > 1 { "s" } else { "" }
        )
    }
}

pub async fn handle_diff(host: &mut dyn ShellHost, args: &str) -> String {
    let trimmed = args.trim();
    let Some(session_id) = host.session_manager().current_session_id() else {
        return "No active session.".to_string();
    };

    if trimmed.is_empty() {
        match host.session_manager().list_edits(session_id) {
            Ok(edits) if edits.is_empty() => {
                return "No edits to diff. Use /diff <file_path> for a specific file.".to_string();
            }
            Ok(edits) => {
                let mut lines = vec!["Recent edits:".to_string()];
                for edit in edits.iter().take(10) {
                    lines.push(format!(
                        "  {} · {} · {}",
                        edit.timestamp, edit.tool_name, edit.file_path
                    ));
                }
                return lines.join("\n");
            }
            Err(e) => return format!("Failed to list edits: {e}"),
        }
    }

    // Try to build a checkpoint diff for the target file.
    match checkpoint_diff_for_target(host, trimmed).await {
        Some((title, content)) => format!("{title}\n{content}"),
        None => "No checkpoint diff available for this file.".to_string(),
    }
}

async fn checkpoint_diff_for_target(
    host: &mut dyn ShellHost,
    target: &str,
) -> Option<(String, String)> {
    let session_id = host.session_manager().current_session_id()?;
    let edits = host.session_manager().list_edits(session_id).ok()?;
    let edit = edits
        .iter()
        .find(|e| e.file_path == target || e.file_path.ends_with(target))?;
    let snapshot = edit.snapshot_path();
    let current = std::fs::read_to_string(&edit.file_path).ok()?;
    let previous = std::fs::read_to_string(snapshot).ok()?;

    let title = format!("Diff for {}", edit.file_path);
    let diff =
        crate::shell::permission_diff::generate_unified_diff(&previous, &current, &edit.file_path)
            .unwrap_or_else(|| "No differences.".to_string());
    Some((title, diff))
}

pub async fn handle_export_data(host: &dyn ShellHost, args: &str) -> String {
    let session_id = match host.session_manager().current_session_id() {
        Some(id) => id,
        None => return "No active session.".to_string(),
    };

    let parts: Vec<&str> = args.split_whitespace().collect();
    let format = parts
        .iter()
        .find(|p| matches!(**p, "json" | "markdown" | "md"))
        .map(|p| match *p {
            "markdown" | "md" => crate::session_store::export::SessionExportFormat::Markdown,
            _ => crate::session_store::export::SessionExportFormat::Json,
        })
        .unwrap_or(crate::session_store::export::SessionExportFormat::Json);
    let privacy = parts
        .iter()
        .find(|p| **p == "&public")
        .map(|_| crate::session_store::export::SessionExportPrivacy::Redacted)
        .unwrap_or(crate::session_store::export::SessionExportPrivacy::Full);

    match host
        .session_manager()
        .write_session_export(session_id, format, privacy)
    {
        Ok(path) => format!("Session exported to {}", path.display()),
        Err(e) => format!("Failed to export session: {e}"),
    }
}

pub async fn handle_save_session(host: &dyn ShellHost) -> String {
    let Some(session_id) = host.session_manager().current_session_id() else {
        return "No active session.".to_string();
    };
    // Session messages and edit history are persisted continuously. /save
    // forces a checkpoint snapshot of the current workspace files so the
    // session can be rewound to this exact state later.
    let mgr = crate::engine::checkpoint::get_checkpoint_manager(session_id).await;
    let mut guard = mgr.lock().await;
    let result = guard
        .create_checkpoint("manual_save", None, None, &[])
        .await;
    drop(guard);
    match result {
        Ok(_) => format!("Session {} saved.", &session_id[..8.min(session_id.len())]),
        Err(e) => format!("Failed to save session: {e}"),
    }
}

pub async fn handle_doctor(host: &dyn ShellHost, args: &str) -> String {
    if args.trim() == "product" {
        return crate::engine::product_readiness::readiness_report();
    }

    let mut lines = vec!["Environment diagnostics:".to_string()];
    lines.push(format!(
        "  session: {}",
        host.session_manager()
            .current_session_id()
            .unwrap_or("none")
    ));
    lines.push(format!("  workspace: {}", host.workspace_root().display()));
    if let Some(engine) = host.engine() {
        lines.push(format!("  model: {}", engine.model_name()));
        lines.push(format!("  provider: {}", engine.provider_base_url()));
    }
    lines.join("\n")
}

pub async fn handle_audit(host: &dyn ShellHost, args: &str) -> String {
    let Some(engine) = host.engine() else {
        return "No engine available.".to_string();
    };
    let parts: Vec<&str> = args.split_whitespace().collect();
    let sub = parts.first().copied().unwrap_or("summary");

    match sub {
        "summary" => {
            let tracker = engine.cost_tracker().lock().await;
            format!("Token usage summary:\n{}", tracker.generate_report())
        }
        "tools" => {
            let names: Vec<String> = engine
                .tool_registry()
                .tool_names()
                .into_iter()
                .map(|n| n.to_string())
                .collect();
            format!("Registered tools:\n{}", names.join("\n"))
        }
        _ => "Usage: /audit [summary|tools]".to_string(),
    }
}

pub async fn handle_provider(host: &dyn ShellHost, args: &str) -> String {
    let registry = crate::services::api::provider::ProviderRegistry::from_env();
    let trimmed = args.trim();

    if trimmed.is_empty()
        || trimmed == "status"
        || trimmed == "status --json"
        || trimmed == "status json"
    {
        if let Some(engine) = host.engine() {
            format!(
                "Provider: {}\nModel: {}\nBase URL: {}\n\nUse /provider list or /provider switch <name>.",
                provider_label_for_base_url(&engine.provider_base_url()),
                engine.model_name(),
                engine.provider_base_url(),
            )
        } else {
            "No engine available.".to_string()
        }
    } else if trimmed == "list" {
        let statuses = crate::services::api::provider_catalog::provider_status_list();
        if statuses.is_empty() {
            return "No providers configured.".to_string();
        }
        let mut lines = vec!["Providers:".to_string()];
        for s in statuses {
            let marker = if s.configured { "*" } else { "-" };
            lines.push(format!(
                "{} {:<12} {:<12} {}",
                marker,
                s.id,
                s.default_model,
                if s.configured {
                    "configured"
                } else {
                    "not configured"
                }
            ));
        }
        lines.join("\n")
    } else if let Some(name) = trimmed
        .strip_prefix("switch ")
        .or_else(|| trimmed.strip_prefix("set "))
        .map(str::trim)
        .filter(|p| !p.is_empty())
    {
        let name_lower = name.to_ascii_lowercase();
        let provider = registry.get(&name_lower);
        let config = registry.get_config(&name_lower).cloned();
        match (provider, config) {
            (Some(provider), Some(config)) => {
                if let Some(engine) = host.engine() {
                    engine.set_provider(provider, config.default_model.clone());
                }
                if let Ok(mut app_config) = crate::services::config::AppConfig::load() {
                    app_config.api.provider_name = Some(name_lower.clone());
                    app_config.api.model = config.default_model.clone();
                    app_config.api.base_url = config.base_url.clone().unwrap_or_default();
                    if app_config.save().is_ok() {
                        crate::services::config::init_runtime_config(app_config);
                    }
                }
                format!(
                    "Provider switched to {}\nModel: {}\nBase URL: {}",
                    config.name,
                    config.default_model,
                    config.base_url.as_deref().unwrap_or("default")
                )
            }
            _ => format!(
                "Provider '{}' is not configured. Use /provider list to see available providers.",
                name
            ),
        }
    } else {
        "Usage: /provider [list|switch <name>|status]".to_string()
    }
}

pub async fn handle_resume(host: &mut dyn ShellHost, args: &str) -> String {
    if args.is_empty() {
        match host.session_manager().list_resumable_sessions(10) {
            Ok(sessions) => {
                if sessions.is_empty() {
                    "No saved sessions found. Start chatting to create one!".to_string()
                } else {
                    let mut lines = vec!["Recent resumable sessions:".to_string()];
                    for (i, session) in sessions.iter().enumerate() {
                        let title = if session.title.is_empty() {
                            "(untitled)"
                        } else {
                            &session.title
                        };
                        let msg_count = host
                            .session_manager()
                            .message_count(&session.id)
                            .unwrap_or(0);
                        lines.push(format!(
                            "{}. [{}] {} ({} msgs) - {}",
                            i + 1,
                            &session.id[..8.min(session.id.len())],
                            title,
                            msg_count,
                            session.updated_at
                        ));
                    }
                    lines.push(
                        "\nUse /resume <number>, /resume <id>, /resume <search>, or /resume latest."
                            .to_string(),
                    );
                    lines.join("\n")
                }
            }
            Err(e) => format!("Failed to list sessions: {}", e),
        }
    } else {
        match host.session_manager().resolve_resume_selection(args, 40) {
            Ok(Some(session)) => host.restore_session(&session.id).await,
            Ok(None) => {
                "No matching session found. Use /resume without arguments to see recent sessions."
                    .to_string()
            }
            Err(e) => format!("Failed to resolve session: {}", e),
        }
    }
}

pub async fn handle_validate(host: &dyn ShellHost) -> String {
    let Some(session_id) = host.session_manager().current_session_id() else {
        return "No active session.".to_string();
    };
    let sid = session_id.to_string();
    let mgr = crate::engine::checkpoint::get_checkpoint_manager(&sid).await;
    let cp = mgr.lock().await;
    let changes = cp.list_file_changes();
    let rounds = cp.list_file_change_rounds();
    let mut lines = vec![
        "Validation Summary".to_string(),
        "==================".to_string(),
        String::new(),
        format!("File changes: {}", changes.len()),
        format!("Tool rounds: {}", rounds.len()),
        String::new(),
    ];
    if changes.is_empty() {
        lines.push("No file changes to validate.".to_string());
    } else {
        lines.push("Changed files:".to_string());
        for c in changes.iter().rev().take(10) {
            lines.push(format!("  {} ({})", c.path, c.tool_name));
        }
        lines.push(String::new());
        lines.push("Run your test suite to validate changes.".to_string());
        lines.push(
            "Use /diff for details or /changes in --tui for a round-by-round breakdown."
                .to_string(),
        );
    }
    lines.join(
        "
",
    )
}

pub async fn handle_token_cost(engine: &StreamingQueryEngine) -> String {
    let tracker = engine.cost_tracker().lock().await;
    tracker.generate_report()
}

fn provider_label_for_base_url(base_url: &str) -> String {
    let u = base_url.to_ascii_lowercase();
    if u.contains("minimax") {
        "MiniMax".to_string()
    } else if u.contains("api.kimi.com") {
        "Kimi Code".to_string()
    } else if u.contains("moonshot") {
        "Kimi".to_string()
    } else if u.contains("deepseek") {
        "DeepSeek".to_string()
    } else if u.contains("bigmodel") || u.contains("z.ai") {
        "GLM".to_string()
    } else if u.contains("openai.com") {
        "OpenAI".to_string()
    } else {
        "Custom".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_session_returns_message() {
        struct DummyHost;
        impl ShellHost for DummyHost {
            fn engine(
                &self,
            ) -> Option<std::sync::Arc<crate::engine::streaming::StreamingQueryEngine>>
            {
                None
            }
            fn session_manager(&self) -> &crate::tui::session_manager::TuiSessionManager {
                static MANAGER: std::sync::OnceLock<
                    crate::tui::session_manager::TuiSessionManager,
                > = std::sync::OnceLock::new();
                MANAGER.get_or_init(|| {
                    crate::tui::session_manager::TuiSessionManager::in_memory().unwrap()
                })
            }
            fn build_tool_context(&self) -> crate::tools::ToolContext {
                crate::tools::ToolContext::new(std::path::PathBuf::from("."), "test")
            }
            fn restore_session(
                &mut self,
                _session_id: &str,
            ) -> std::pin::Pin<Box<dyn std::future::Future<Output = String> + Send + '_>>
            {
                Box::pin(async move { String::new() })
            }
            fn show_message(&mut self, _message: String) {}
            fn memory_use(&self) -> bool {
                false
            }
            fn set_memory_use(&mut self, _value: bool) {}
            fn memory_generate(&self) -> bool {
                false
            }
            fn set_memory_generate(&mut self, _value: bool) {}
            fn memory_recall_mode(&self) -> &str {
                ""
            }
            fn set_memory_recall_mode(&mut self, _value: String) {}
        }

        let host = DummyHost;
        assert_eq!(
            futures::executor::block_on(handle_save_session(&host)),
            "No active session."
        );
    }
}
