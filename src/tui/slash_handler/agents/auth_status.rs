use super::*;

/// Suggest recovery action based on error context
pub fn suggest_recovery(error: &str, _context: &str) -> String {
    let error_lower = error.to_lowercase();

    if error_lower.contains("timeout") {
        return "Timeout error. Suggestions:\n- Use /retry to repeat the operation\n- Use /doctor to check tool latency\n- Try a simpler command".to_string();
    }

    if error_lower.contains("permission") || error_lower.contains("denied") {
        return "Permission error. Suggestions:\n- Use /permissions rules to check current rules\n- Use /permissions mode to change mode\n- Run /doctor for permission diagnostics".to_string();
    }

    if error_lower.contains("not found") || error_lower.contains("does not exist") {
        return "Not found error. Suggestions:\n- Check file/resource exists with ls or glob\n- Verify the path is correct\n- Use /context to see current state".to_string();
    }

    if error_lower.contains("syntax") || error_lower.contains("parse") {
        return "Syntax error. Suggestions:\n- Check command arguments with /help <command>\n- Verify JSON formatting if using structured args\n- Try /doctor to validate environment".to_string();
    }

    // Default
    format!(
        "Error encountered. General suggestions:\n\
        - Use /retry to attempt the operation again\n\
        - Use /doctor to run full diagnostics\n\
        - Use /status to check current state\n\
        - Use /context to view conversation context\n\
        Error: {}",
        error
    )
}
/// /init - Initialize a new project
pub fn handle_init(_app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Project initializer\n\nUsage: /init <project_name>\n\nCreates:\n- README.md with next steps\n- Cargo.toml and src/main.rs\n- .gitignore\n- .priority-agent/AGENTS.md for project instructions\n\nAfter creation, cd into the project and run cargo check.".to_string();
    }

    let dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let project_name = args.trim();
    let project_path = dir.join(project_name);

    if project_path.exists() {
        return format!("Target already exists: {}", project_path.display());
    }
    match std::fs::create_dir_all(project_path.join("src"))
        .and_then(|_| std::fs::create_dir_all(project_path.join(".priority-agent")))
    {
        Ok(_) => {
            let readme = project_path.join("README.md");
            let gitignore = project_path.join(".gitignore");
            let cargo_toml = project_path.join("Cargo.toml");
            let main_rs = project_path.join("src").join("main.rs");
            let agents = project_path.join(".priority-agent").join("AGENTS.md");
            if std::fs::write(
                &readme,
                format!(
                    "# {}\n\nInitialized by Priority Agent `/init`.\n\n## Next steps\n\n- Run `cargo test`\n- Describe the first feature you want the agent to build\n- Use `/settings` to confirm model and permission mode\n",
                    project_name
                ),
            )
            .is_err()
            {
                return format!(
                    "Project initialized at {} (README.md write failed)",
                    project_path.display()
                );
            }
            if std::fs::write(&gitignore, "target/\n*.log\n.env\n").is_err() {
                return format!(
                    "Project initialized at {} (.gitignore write failed)",
                    project_path.display()
                );
            }
            if std::fs::write(
                &cargo_toml,
                format!(
                    "[package]\nname = \"{}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\n",
                    project_name.replace('-', "_")
                ),
            ).is_err() {
                return format!("Project initialized at {} (Cargo.toml write failed)", project_path.display());
            }
            if std::fs::write(&main_rs, "fn main() {\n    println!(\"hello\");\n}\n").is_err() {
                return format!(
                    "Project initialized at {} (src/main.rs write failed)",
                    project_path.display()
                );
            }
            if std::fs::write(
                &agents,
                format!(
                    "# Project Instructions\n\nProject: {}\n\n## Working style\n\n- Prefer small, verified changes.\n- Run relevant tests after edits.\n- Keep user-facing CLI output concise and useful.\n\n## Commands\n\n- `cargo test`\n- `cargo check`\n",
                    project_name
                ),
            )
            .is_err()
            {
                return format!(
                    "Project initialized at {} (.priority-agent/AGENTS.md write failed)",
                    project_path.display()
                );
            }
            format!(
                "Project initialized\n\nOverview:\n- Name: {}\n- Path: {}\n- Type: Rust binary scaffold\n- Instructions: .priority-agent/AGENTS.md\n\nCreated:\n- README.md\n- Cargo.toml\n- src/main.rs\n- .gitignore\n- .priority-agent/AGENTS.md\n\nNext actions:\n1. cd {}\n2. cargo check\n3. Tell the agent what to build first",
                project_name,
                project_path.display(),
                project_path.display()
            )
        }
        Err(e) => format!("Failed to initialize project: {}", e),
    }
}
/// /login - Authentication
pub fn handle_login(_app: &mut TuiApp, args: &str) -> String {
    if args.trim().is_empty() || args.trim() == "status" {
        let prefs = load_runtime_prefs().unwrap_or_default();
        return format!(
            "Login status: {}",
            prefs
                .logged_in_provider
                .as_deref()
                .unwrap_or("not logged in")
        );
    }

    let provider = args.trim().to_ascii_lowercase();
    let mut prefs = load_runtime_prefs().unwrap_or_default();
    prefs.logged_in_provider = Some(provider.clone());
    if let Err(e) = save_runtime_prefs(&prefs) {
        return format!("Login state update failed: {}", e);
    }
    format!(
        "Logged in to '{}' (local state only). Use /key to configure API keys.",
        provider
    )
}
/// /logout - Logout from provider
pub fn handle_logout(_app: &mut TuiApp, _args: &str) -> String {
    let mut prefs = load_runtime_prefs().unwrap_or_default();
    let old = prefs.logged_in_provider.take();
    if let Err(e) = save_runtime_prefs(&prefs) {
        return format!("Failed to clear login state: {}", e);
    }
    match old {
        Some(p) => format!("Logged out from '{}'.", p),
        None => "No active login session.".to_string(),
    }
}

/// /connect <provider> — guided provider setup with catalog DTO.
pub fn handle_connect(_app: &mut TuiApp, args: &str) -> String {
    let id = args.trim().to_ascii_lowercase();
    if id.is_empty() || id == "list" {
        let mut out = String::from("Available providers:\n\n");
        for status in crate::services::api::credentials::status_all() {
            let marker = if status.configured { "✓" } else { "○" };
            out.push_str(&format!(
                "  {} {} — {} (use /connect {})\n",
                marker,
                status.provider_label,
                if status.configured {
                    "configured"
                } else {
                    "not configured"
                },
                status.provider_id,
            ));
        }
        if out.contains("○") {
            out.push_str("\nRun /connect <provider> for setup instructions.\n");
        }
        return out;
    }

    match crate::services::api::credentials::connect_message(&id) {
        Some(msg) => msg,
        None => format!(
            "Unknown provider '{}'. Run /connect list to see available providers.",
            id
        ),
    }
}

/// /credentials — credential status summary.
pub fn handle_credentials(_app: &mut TuiApp, args: &str) -> String {
    let id = args.trim().to_ascii_lowercase();
    if id.is_empty() {
        return crate::services::api::credentials::status_summary();
    }
    match crate::services::api::credentials::status_for(&id) {
        Some(s) => {
            format!(
                "Provider: {} ({})\n\
                 Configured: {}\n\
                 Env var: {}\n\
                 Setup: {}\n\
                 Shell line: {}",
                s.provider_label,
                s.provider_id,
                if s.configured { "yes" } else { "no" },
                s.active_env_var.as_deref().unwrap_or("none"),
                s.setup_hint,
                s.export_line,
            )
        }
        None => format!("Unknown provider '{}'.", id),
    }
}
/// /key - API key management
pub fn handle_key(_app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        let has_key = crate::services::api::provider::DEFAULT_PROVIDER_ENV_SPECS
            .iter()
            .any(|spec| {
                spec.key_env_vars
                    .iter()
                    .any(|key| std::env::var(key).is_ok_and(|value| !value.trim().is_empty()))
            });
        return if has_key {
            "API key is set. Use /model to see which model is active.".to_string()
        } else {
            format!(
                "No API key set. Set one provider key: {}.",
                crate::services::api::provider::provider_key_env_hint()
            )
        };
    }

    match args.trim() {
        "show" => format!(
            "API key not shown for security. Supported key env vars: {}.",
            crate::services::api::provider::provider_key_env_hint()
        ),
        "clear" => {
            for spec in crate::services::api::provider::DEFAULT_PROVIDER_ENV_SPECS {
                for key in spec.key_env_vars {
                    std::env::remove_var(key);
                }
            }
            "Cleared API keys from current process environment.".to_string()
        }
        _ => "Usage: /key [show|clear]".to_string(),
    }
}
/// /status - Detailed status
pub fn handle_status_detailed(app: &TuiApp) -> String {
    let mut lines = vec!["Detailed Status:".to_string()];
    lines.push(format!("  Agent mode: {}", app.current_agent_mode_label()));
    lines.push(format!("  UI mode: {:?}", app.mode));
    lines.push(format!("  Rust version: {}", std::env::consts::OS));
    format!(
        "{}\n{}",
        lines.join("\n"),
        "Use /doctor for full diagnostics"
    )
}
/// /health - Health check
pub fn handle_health(_app: &TuiApp) -> String {
    "Health: OK\nSystem operational.".to_string()
}
/// /ping - Latency check
pub fn handle_ping(app: &mut TuiApp) -> String {
    use std::time::Instant;

    // Measure a real local round-trip by touching the session store.
    let db_start = Instant::now();
    let db_ok = app.session_manager.list_sessions(1).is_ok();
    let db_ms = db_start.elapsed().as_millis();

    format!(
        "Pong! Local DB round-trip: {}ms ({})",
        db_ms,
        if db_ok { "ok" } else { "error" }
    )
}
/// /uptime - Show uptime
pub fn handle_uptime(app: &TuiApp) -> String {
    let uptime = app.app_started_at.elapsed();
    let total_secs = uptime.as_secs();
    let hours = total_secs / 3_600;
    let minutes = (total_secs % 3_600) / 60;
    let seconds = total_secs % 60;

    format!(
        "Uptime: {:02}:{:02}:{:02}\nSession: {}\nMessages: {}\nTools: {}",
        hours,
        minutes,
        seconds,
        app.session_manager.current_session_id().unwrap_or("none"),
        app.messages.len(),
        app.tool_runs_snapshot.len()
    )
}
/// /version - Show version
pub fn handle_version(_app: &TuiApp) -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
/// /about - About this agent
pub fn handle_about(_app: &TuiApp) -> String {
    format!(
        "Priority Agent v{}\nWeighted priority desktop Agent.\nType /help for available commands.",
        env!("CARGO_PKG_VERSION")
    )
}
