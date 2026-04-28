//! Config / permissions / integration handlers extracted from slash_handler.
//!
//! All functions here are `pub` so they remain callable from the TUI command
//! dispatcher via `pub use config::*;` in the parent `mod.rs`.

use super::utils::*;

use crate::tools::Tool;
use crate::tui::app::{AppMode, TuiApp};

// ─── Section 4: vim / onboarding / skip ─────────────────────────────

pub fn handle_vim(app: &mut TuiApp) -> String {
    app.vim_mode = !app.vim_mode;
    if app.vim_mode {
        app.mode = AppMode::VimNormal;
        "Vim mode enabled. Press Ctrl+V or type /vim again to disable.".to_string()
    } else {
        app.mode = AppMode::Chat;
        "Vim mode disabled.".to_string()
    }
}

pub fn handle_onboarding(app: &mut TuiApp) -> String {
    let manager = crate::onboarding::OnboardingManager::new();
    let _ = manager.reset();
    app.onboarding_state = Some(crate::onboarding::OnboardingState::new());
    app.mode = AppMode::Onboarding;
    "Onboarding restarted. Press Enter or → to continue, ← to go back, Esc to skip.".to_string()
}

pub fn handle_skip(app: &mut TuiApp) -> String {
    if app.mode == AppMode::Onboarding {
        if let Some(ref state) = app.onboarding_state {
            let _ = state.complete();
        }
        app.onboarding_state = None;
        app.mode = AppMode::Chat;
        "Onboarding skipped. Type /onboarding to restart it.".to_string()
    } else {
        "Not in onboarding mode.".to_string()
    }
}

// ─── Permissions (complex, 128 lines) ─────────────────────────────────

use crate::permissions::{match_wildcard, PermissionMode, RuleSource, SourcedRule};
use crate::tui::app::{parse_permission_mode, permission_mode_name, persist_permission_rule};

pub fn handle_permissions(app: &mut TuiApp, args: &str) -> String {
    let mut parts = args.split_whitespace();
    let sub = parts.next();

    match sub {
        None => {
            let mode = app
                .streaming_engine
                .as_ref()
                .map(|e| e.permission_mode())
                .unwrap_or(PermissionMode::AutoAll);
            let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            let ctx = crate::permissions::PermissionContext::new(&cwd);
            format!(
                "Permission mode: {}\nRules: allow={} deny={} ask={}\nProject config: {}\nGlobal config: {}\n\nUsage:\n  /permissions mode <default|auto|auto_low_risk|auto_all|read_only>\n  /permissions rules [tool_name]\n  /permissions explain <tool_name> - explain why a decision was made (with confidence & warnings)\n  /permissions export [path] - export rules to a file\n  /permissions import <path> [project|global] [merge] - import rules (merge to append)\n  /permissions dry-run <allow|deny|ask> <pattern> - test a rule against all registered tools\n  /permissions <allow|deny|ask> <pattern> [project|global]",
                permission_mode_name(mode),
                ctx.rules.always_allow.len(),
                ctx.rules.always_deny.len(),
                ctx.rules.always_ask.len(),
                cwd.join(".priority-agent").join("permissions.toml").display(),
                dirs::home_dir()
                    .unwrap_or_else(|| std::path::PathBuf::from("."))
                    .join(".priority-agent")
                    .join("permissions.toml")
                    .display(),
            )
        }
        Some("explain") => {
            let tool_name = match parts.next() {
                Some(t) if !t.trim().is_empty() => t.trim(),
                _ => return "Usage: /permissions explain <tool_name>".to_string(),
            };
            let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            let ctx = crate::permissions::PermissionContext::new(&cwd);
            // Use ExplainableDecision for rich output (confidence, warnings, matched rules)
            let explainable = ctx.explain_decision(tool_name, &serde_json::Value::Null);
            let mut output = explainable.format();

            // Add mode context
            let mode = app
                .streaming_engine
                .as_ref()
                .map(|e| e.permission_mode())
                .unwrap_or(PermissionMode::AutoAll);
            output.push_str(&format!("\n\nCurrent mode: {}", permission_mode_name(mode)));
            match mode {
                PermissionMode::AutoAll => {
                    output.push_str(
                        "\n  (developer auto mode: common coding actions auto-run; high-risk actions still ask)",
                    )
                }
                PermissionMode::AutoLowRisk => {
                    output.push_str("\n  (low-risk operations auto-allowed, others follow rules)")
                }
                PermissionMode::ReadOnly => output.push_str("\n  (all write operations denied)"),
                PermissionMode::Once => {
                    output.push_str("\n  (each operation allowed once then denied)")
                }
                _ => {}
            }
            output
        }
        Some("export") => {
            let path = parts
                .next()
                .map(|p| {
                    if p == "global" || p == "project" {
                        return None;
                    }
                    Some(std::path::PathBuf::from(p))
                })
                .unwrap_or_else(|| {
                    let cwd =
                        std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
                    Some(cwd.join(".priority-agent").join("permissions_export.toml"))
                });

            let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            let ctx = crate::permissions::PermissionContext::new(&cwd);

            // Build export content (using standard TOML array format)
            let mut content = String::new();
            content.push_str("# Permission Rules Export\n");
            content.push_str(&format!(
                "# Exported at: {}\n\n",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
            ));

            content.push_str("[allow]\npatterns = [");
            for (i, r) in ctx.rules.always_allow.iter().enumerate() {
                if i > 0 {
                    content.push_str(", ");
                }
                content.push_str(&format!("\"{}\"", r.pattern));
            }
            content.push_str("]\n");

            content.push_str("\n[deny]\npatterns = [");
            for (i, r) in ctx.rules.always_deny.iter().enumerate() {
                if i > 0 {
                    content.push_str(", ");
                }
                content.push_str(&format!("\"{}\"", r.pattern));
            }
            content.push_str("]\n");

            content.push_str("\n[ask]\npatterns = [");
            for (i, r) in ctx.rules.always_ask.iter().enumerate() {
                if i > 0 {
                    content.push_str(", ");
                }
                content.push_str(&format!("\"{}\"", r.pattern));
            }
            content.push_str("]\n");

            if let Some(ref p) = path {
                if let Some(parent) = p.parent() {
                    std::fs::create_dir_all(parent).ok();
                }
                match std::fs::write(p, &content) {
                    Ok(_) => format!("Rules exported to: {}", p.display()),
                    Err(e) => format!("Failed to export: {}", e),
                }
            } else {
                content
            }
        }
        Some("import") => {
            let file_path = match parts.next() {
                Some(p) if !p.trim().is_empty() => p.trim(),
                _ => {
                    return "Usage: /permissions import <path> [project|global] [merge]".to_string()
                }
            };
            let scope = match parts.next().map(|s| s.to_ascii_lowercase()) {
                Some(s) if s == "global" => RuleSource::Global,
                Some(s) if s == "project" => RuleSource::Project,
                Some(other) => {
                    return format!("Invalid scope '{}'. Use 'project' or 'global'.", other)
                }
                None => RuleSource::Project,
            };
            let merge = match parts.next().map(|s| s.to_ascii_lowercase()) {
                Some(s) if s == "merge" => true,
                Some(other) => return format!("Invalid option '{}'. Use 'merge' or omit.", other),
                None => false,
            };

            let import_content = match std::fs::read_to_string(file_path) {
                Ok(c) => c,
                Err(e) => return format!("Failed to read file: {}", e),
            };

            let target_path = match scope {
                RuleSource::Global => dirs::home_dir()
                    .unwrap_or_else(|| std::path::PathBuf::from("."))
                    .join(".priority-agent")
                    .join("permissions.toml"),
                _ => std::env::current_dir()
                    .unwrap_or_else(|_| std::path::PathBuf::from("."))
                    .join(".priority-agent")
                    .join("permissions.toml"),
            };

            if let Some(parent) = target_path.parent() {
                std::fs::create_dir_all(parent).ok();
            }

            let final_content = if merge && target_path.exists() {
                // Read existing rules and merge with imported rules
                let existing = std::fs::read_to_string(&target_path).unwrap_or_default();
                match merge_permission_toml(&existing, &import_content) {
                    Ok(merged) => merged,
                    Err(e) => return format!("Failed to merge rules: {}", e),
                }
            } else {
                import_content
            };

            match std::fs::write(&target_path, &final_content) {
                Ok(_) => {
                    let action = if merge { "merged into" } else { "imported to" };
                    format!(
                        "Rules {} '{}' -> {}",
                        action,
                        file_path,
                        target_path.display()
                    )
                }
                Err(e) => format!("Failed to import: {}", e),
            }
        }
        Some("dry-run") => {
            let action = match parts.next() {
                Some(a) if a == "allow" || a == "deny" || a == "ask" => a,
                _ => return "Usage: /permissions dry-run <allow|deny|ask> <pattern>".to_string(),
            };
            let pattern = match parts.next() {
                Some(p) if !p.trim().is_empty() => p.trim(),
                _ => return "Usage: /permissions dry-run <allow|deny|ask> <pattern>".to_string(),
            };

            let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            let ctx = crate::permissions::PermissionContext::new(&cwd);

            // Simulate adding the rule temporarily
            let mut test_rules = ctx.rules.clone();
            let test_rule = SourcedRule::new(pattern, RuleSource::User);

            match action {
                "allow" => test_rules.always_allow.push(test_rule),
                "deny" => test_rules.always_deny.push(test_rule),
                "ask" => test_rules.always_ask.push(test_rule),
                _ => unreachable!(),
            }

            // Show what tools would match using full registry + explainable decisions
            let mut lines = vec![
                format!("Dry-run: {} '{}'", action, pattern),
                format!(
                    "Config path: {}/.priority-agent/permissions.toml",
                    cwd.display()
                ),
                "".to_string(),
                "This rule would affect:".to_string(),
            ];

            // Test against all registered tools
            let registry = crate::tools::ToolRegistry::default_registry();
            let mut affected = 0;
            for tool in &registry.tool_names() {
                if match_wildcard(pattern, tool) {
                    affected += 1;
                    let decision = test_rules.check(tool);
                    let explainable = ctx.explain_decision(tool, &serde_json::Value::Null);
                    let conf = (explainable.confidence * 100.0) as u32;
                    let warn = if explainable.warnings.is_empty() {
                        "".to_string()
                    } else {
                        format!(" ⚠️ {}", explainable.warnings.join(", "))
                    };
                    lines.push(format!(
                        "  {} -> {:?} (confidence: {}%){}",
                        tool, decision, conf, warn
                    ));
                }
            }
            if affected == 0 {
                lines.push("  (no registered tools match this pattern)".to_string());
            } else {
                lines.push(format!("\nTotal affected tools: {}", affected));
            }

            lines.join("\n")
        }
        Some("mode") => {
            if let Some(mode_arg) = parts.next() {
                if let Some(mode) = parse_permission_mode(mode_arg) {
                    if let Some(ref engine) = app.streaming_engine {
                        engine.set_permission_mode(mode);
                        format!("Permission mode set to '{}'.", permission_mode_name(mode))
                    } else {
                        "Cannot set permission mode: engine unavailable.".to_string()
                    }
                } else {
                    "Invalid mode. Use: default | auto | auto_low_risk | auto_all | read_only"
                        .to_string()
                }
            } else {
                let current = app
                    .streaming_engine
                    .as_ref()
                    .map(|e| e.permission_mode())
                    .unwrap_or(PermissionMode::AutoAll);
                format!(
                    "Current mode: {}\nAvailable: default | auto | auto_low_risk | auto_all | read_only",
                    permission_mode_name(current)
                )
            }
        }
        Some("rules") => {
            let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            let ctx = crate::permissions::PermissionContext::new(&cwd);
            if let Some(tool_name) = parts.next() {
                let (decision, details) = ctx.check_with_details(tool_name);
                let mut lines = vec![format!("Tool '{}': {:?}", tool_name, decision)];
                if details.is_empty() {
                    lines.push(
                        "No explicit matching rules (fallback behavior applies).".to_string(),
                    );
                } else {
                    lines.push("Matched rules:".to_string());
                    for d in details {
                        lines.push(format!("- {}", d));
                    }
                }
                lines.join("\n")
            } else {
                let mut lines = vec![
                    format!("Rules overview (cwd={}):", cwd.display()),
                    format!("allow({}):", ctx.rules.always_allow.len()),
                ];
                for r in ctx.rules.always_allow.iter().take(30) {
                    lines.push(format!("- [{:?}] {}", r.source, r.pattern));
                }
                lines.push(format!("deny({}):", ctx.rules.always_deny.len()));
                for r in ctx.rules.always_deny.iter().take(30) {
                    lines.push(format!("- [{:?}] {}", r.source, r.pattern));
                }
                lines.push(format!("ask({}):", ctx.rules.always_ask.len()));
                for r in ctx.rules.always_ask.iter().take(30) {
                    lines.push(format!("- [{:?}] {}", r.source, r.pattern));
                }
                lines.join("\n")
            }
        }
        Some(action @ ("allow" | "deny" | "ask")) => {
            let pattern = match parts.next() {
                Some(p) if !p.trim().is_empty() => p.trim(),
                _ => {
                    return "Usage: /permissions <allow|deny|ask> <pattern> [project|global]"
                        .to_string()
                }
            };
            let scope = match parts.next().map(|s| s.to_ascii_lowercase()) {
                Some(s) if s == "global" => RuleSource::Global,
                Some(s) if s == "project" => RuleSource::Project,
                Some(other) => {
                    return format!("Invalid scope '{}'. Use 'project' or 'global'.", other)
                }
                None => RuleSource::Project,
            };
            let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            match persist_permission_rule(scope, action, pattern, &cwd) {
                Ok(path) => {
                    let path: std::path::PathBuf = path;
                    format!(
                        "Rule saved: {} '{}' ({:?})\nConfig: {}",
                        action,
                        pattern,
                        scope,
                        path.display()
                    )
                }
                Err(e) => format!("Failed to save rule: {}", e),
            }
        }
        Some(_) => "Usage: /permissions [mode|rules|allow|deny|ask] ...".to_string(),
    }
}

// ─── Batch 1: /reload ───────────────────────────────────────────────

pub async fn handle_reload(app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() || args == "config" {
        match crate::services::config::AppConfig::load() {
            Ok(config) => {
                // Apply visible UI config immediately.
                app.theme = crate::tui::theme::Theme::from_name(&config.ui.theme);
                if let Some(ref mut settings) = app.settings_state {
                    settings.config = config.clone();
                }
                format!(
                    "Config reloaded:\n- API: {}\n- Model: {}",
                    config.api.base_url, config.api.model
                )
            }
            Err(e) => format!("Failed to reload config: {}", e),
        }
    } else if args == "plugins" {
        // Reload plugins
        let working_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let mut registry = crate::tools::ToolRegistry::default_registry();
        let injected =
            crate::tools::plugin_tool::register_enabled_plugin_tools(&mut registry, &working_dir);
        format!("Plugins reloaded. {} plugin tools injected.", injected)
    } else if args == "skills" {
        let count = app.skill_runtime.reload();
        format!(
            "Skills reloaded. {} skill(s) available. Use /skills or /<skill-name> <task>.",
            count
        )
    } else {
        "Usage: /reload [config|plugins|skills]".to_string()
    }
}

// ─── Batch 2: hooks, profiling, prompt, migrate, focus, pause, install, skeleton, branch, color

/// /hooks - Show hook configuration status
pub fn handle_hooks(_app: &TuiApp) -> String {
    use std::env;

    let pre_hook = env::var("PRIORITY_AGENT_PRE_TOOL_HOOK").ok();
    let post_hook = env::var("PRIORITY_AGENT_POST_TOOL_HOOK").ok();
    let mut tool_before = Vec::new();
    let mut tool_after = Vec::new();
    for (key, value) in env::vars() {
        if key.starts_with("PRIORITY_AGENT_TOOL_HOOK_BEFORE_") && !value.trim().is_empty() {
            tool_before.push(format!("{}={}", key, value));
        } else if key.starts_with("PRIORITY_AGENT_TOOL_HOOK_AFTER_") && !value.trim().is_empty() {
            tool_after.push(format!("{}={}", key, value));
        }
    }
    tool_before.sort();
    tool_after.sort();
    let timeout = env::var("PRIORITY_AGENT_HOOK_TIMEOUT_MS").ok();
    let fail_closed = env::var("PRIORITY_AGENT_HOOK_FAIL_CLOSED").ok();

    let mut lines = vec!["Hook Configuration:".to_string()];

    if let Some(ref h) = pre_hook {
        lines.push(format!("  PRE_TOOL_HOOK: {}", h));
    } else {
        lines.push("  PRE_TOOL_HOOK: not set".to_string());
    }
    if let Some(ref h) = post_hook {
        lines.push(format!("  POST_TOOL_HOOK: {}", h));
    } else {
        lines.push("  POST_TOOL_HOOK: not set".to_string());
    }
    if !tool_before.is_empty() {
        lines.push("  TOOL_HOOK_BEFORE:".to_string());
        for hook in &tool_before {
            lines.push(format!("    {}", hook));
        }
    } else {
        lines.push("  TOOL_HOOK_BEFORE: not set".to_string());
    }
    if !tool_after.is_empty() {
        lines.push("  TOOL_HOOK_AFTER:".to_string());
        for hook in &tool_after {
            lines.push(format!("    {}", hook));
        }
    } else {
        lines.push("  TOOL_HOOK_AFTER: not set".to_string());
    }
    lines.push(format!(
        "  HOOK_TIMEOUT_MS: {}",
        timeout.unwrap_or_else(|| "1000".to_string())
    ));
    lines.push(format!(
        "  HOOK_FAIL_CLOSED: {}",
        fail_closed.unwrap_or_else(|| "false".to_string())
    ));

    if pre_hook.is_none() && post_hook.is_none() && tool_before.is_empty() && tool_after.is_empty()
    {
        lines.push(
            "\nNo hooks configured. Set PRIORITY_AGENT_*_HOOK environment variables.".to_string(),
        );
    }

    lines.join("\n")
}

/// /profiling - Show runtime profiling info
pub fn handle_profiling(app: &TuiApp) -> String {
    let mut lines = vec!["Profiling Info:".to_string()];

    // Session info
    if let Some(id) = app.session_manager.current_session_id() {
        lines.push(format!("  Session: {}...", &id[..8.min(id.len())]));
    }
    lines.push(format!("  Messages: {}", app.messages.len()));

    // Engine info
    if app.streaming_engine.is_some() {
        lines.push("  Engine: StreamingQueryEngine".to_string());
    } else {
        lines.push("  Engine: not initialized".to_string());
    }

    // Memory
    if let Some(ref engine) = app.streaming_engine {
        if engine.memory_manager().is_some() {
            lines.push("  Memory: active (use /memory to view)".to_string());
        }
    }

    lines.join("\n")
}

/// /prompt - Show/edit system prompt
pub async fn handle_prompt(app: &mut TuiApp, args: &str) -> String {
    let args = args.trim();
    if args == "templates" || args == "list" {
        return crate::engine::prompt_templates::list_templates();
    }
    if let Some(rest) = args.strip_prefix("render ").map(str::trim) {
        let Some((name, goal)) = rest.split_once(' ') else {
            return "Usage: /prompt render <template> <goal>".to_string();
        };
        if goal.trim().is_empty() {
            return "Usage: /prompt render <template> <goal>".to_string();
        }
        return match crate::engine::prompt_templates::render_template(name, goal) {
            Ok(rendered) => rendered,
            Err(e) => e,
        };
    }
    if args.is_empty() || args == "show" {
        return match read_prompt_file() {
            Ok(Some(v)) => format!("System prompt:\n\n{}", v),
            Ok(None) => "No custom system prompt set.".to_string(),
            Err(e) => format!("Failed to read prompt: {}", e),
        };
    }
    if let Some(text) = args.strip_prefix("edit ").map(str::trim) {
        if text.is_empty() {
            return "Usage: /prompt edit <text>".to_string();
        }
        return match write_prompt_file(text) {
            Ok(_) => "Custom system prompt updated.".to_string(),
            Err(e) => format!("Failed to write prompt: {}", e),
        };
    }
    if let Some(text) = args.strip_prefix("append ").map(str::trim) {
        if text.is_empty() {
            return "Usage: /prompt append <text>".to_string();
        }
        return match append_prompt_file(text) {
            Ok(_) => "Custom system prompt appended.".to_string(),
            Err(e) => format!("Failed to append prompt: {}", e),
        };
    }
    if args == "reset" {
        return match reset_prompt_file() {
            Ok(_) => "Custom system prompt reset.".to_string(),
            Err(e) => format!("Failed to reset prompt: {}", e),
        };
    }
    if args == "apply" {
        let prompt = match read_prompt_file() {
            Ok(Some(v)) => v,
            Ok(None) => {
                return "No custom system prompt set. Use `/prompt edit <text>` first.".to_string()
            }
            Err(e) => return format!("Failed to read prompt: {}", e),
        };

        let content = format!("[Custom System Prompt]\n{}", prompt);
        app.add_system_message(content.clone());
        let _ = app
            .session_manager
            .add_message(crate::state::MessageRole::System, &content);
        if let Some(ref engine) = app.streaming_engine {
            engine
                .set_history(message_items_to_api_messages(&app.messages))
                .await;
        }
        return "Custom system prompt applied to current session context.".to_string();
    }
    "Usage: /prompt [show|templates|render <template> <goal>|edit <text>|append <text>|apply|reset]"
        .to_string()
}

/// /migrate - Migration helper
pub async fn handle_migrate(app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /migrate [up|down|status]".to_string();
    }

    let parts: Vec<&str> = args.split_whitespace().collect();
    match parts[0] {
        "up" => run_migrate_sqlx(app, true).await,
        "down" => run_migrate_sqlx(app, false).await,
        "status" => {
            let dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            let migrations_dir = dir.join("migrations");
            if migrations_dir.exists() && migrations_dir.is_dir() {
                let mut files: Vec<String> = match tokio::fs::read_dir(&migrations_dir).await {
                    Ok(mut read_dir) => {
                        let mut f = Vec::new();
                        while let Ok(Some(entry)) = read_dir.next_entry().await {
                            let p = entry.path();
                            if p.is_file() {
                                if let Some(n) = p.file_name() {
                                    f.push(n.to_string_lossy().to_string());
                                }
                            }
                        }
                        f
                    }
                    Err(_) => Vec::new(),
                };
                files.sort();
                let preview = files
                    .iter()
                    .take(10)
                    .map(|f| format!("- {}", f))
                    .collect::<Vec<_>>()
                    .join("\n");
                format!(
                    "Migrations dir: {}\nFiles: {}\n{}\n\nUse `/migrate up` or `/migrate down` (requires sqlx + DATABASE_URL).",
                    migrations_dir.display(),
                    files.len(),
                    if preview.is_empty() {
                        "(no migration files found)".to_string()
                    } else {
                        preview
                    }
                )
            } else {
                "No migrations directory found.".to_string()
            }
        }
        _ => "Usage: /migrate [up|down|status]".to_string(),
    }
}

/// /focus - Focus mode toggle
pub fn handle_focus(app: &mut TuiApp, args: &str) -> String {
    let args = args.trim();
    if args.is_empty() || args == "status" {
        return format!(
            "Focus mode: {}",
            if app.focus_mode {
                "enabled"
            } else {
                "disabled"
            }
        );
    }

    let enable = match args {
        "on" | "enable" => true,
        "off" | "disable" => false,
        "toggle" => !app.focus_mode,
        _ => return "Usage: /focus [on|off|toggle|status]".to_string(),
    };

    app.focus_mode = enable;
    if let Ok(mut config) = crate::services::config::AppConfig::load() {
        config.ui.compact_mode = enable;
        if config.save().is_err() {
            return format!(
                "Focus mode set to {} (config save failed)",
                if enable { "on" } else { "off" }
            );
        }
        if let Some(ref mut settings) = app.settings_state {
            settings.config.ui.compact_mode = enable;
        }
    }
    format!(
        "Focus mode {}.",
        if enable { "enabled" } else { "disabled" }
    )
}

/// /pause - Pause/resume agent
pub fn handle_pause(app: &mut TuiApp, args: &str) -> String {
    let args = args.trim();
    if args.is_empty() || args == "status" {
        return format!(
            "Pause state: {}",
            if app.paused { "paused" } else { "running" }
        );
    }

    if args == "pause" {
        app.paused = true;
        app.is_querying = false;
        "Agent paused. New messages are blocked until `/pause resume`.".to_string()
    } else if args == "resume" {
        app.paused = false;
        app.is_querying = false;
        "Agent resumed.".to_string()
    } else if args == "toggle" {
        app.paused = !app.paused;
        if app.paused {
            app.is_querying = false;
            "Agent paused. New messages are blocked until `/pause resume`.".to_string()
        } else {
            "Agent resumed.".to_string()
        }
    } else {
        "Usage: /pause [pause|resume|toggle|status]".to_string()
    }
}

/// /install - Dependency installer
pub async fn handle_install(app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /install [cargo|npm|pip] [package]".to_string();
    }

    let parts: Vec<&str> = args.split_whitespace().collect();
    let tool_name = parts[0];

    let (_tool, cmd) = match tool_name {
        "cargo" => ("BashTool", format!("cargo {}", parts.get(1).unwrap_or(&""))),
        "npm" => (
            "BashTool",
            format!("npm install {}", parts.get(1).unwrap_or(&"")),
        ),
        "pip" => (
            "BashTool",
            format!("pip install {}", parts.get(1).unwrap_or(&"")),
        ),
        _ => (
            "BashTool",
            format!("{} {}", tool_name, parts.get(1).unwrap_or(&"")),
        ),
    };

    let tool = crate::tools::BashTool;
    let ctx = app.build_tool_context().await;
    let params = serde_json::json!({
        "command": cmd.trim(),
        "description": format!("install {}", args)
    });
    let result = tool.execute(params, ctx).await;
    if result.success {
        result.content
    } else {
        result.error.unwrap_or_default()
    }
}

/// /skeleton - Generate code skeleton
pub fn handle_skeleton(_app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /skeleton <language> [filename]".to_string();
    }

    let parts: Vec<&str> = args.split_whitespace().collect();
    let lang = parts[0];
    let filename = parts.get(1).unwrap_or(&"main");

    let skeleton = match lang {
        "rust" => format!("// {}.rs\n\nfn main() {{\n    println!(\"Hello, world!\");\n}}\n", filename),
        "python" => format!("# {}.py\n\ndef main():\n    print(\"Hello, world!\")\n\nif __name__ == \"__main__\":\n    main()\n", filename),
        "typescript" | "ts" => format!("// {}.ts\n\nexport function main(): void {{\n    console.log(\"Hello, world!\");\n}}\n", filename),
        "javascript" | "js" => format!("// {}.js\n\nfunction main() {{\n    console.log(\"Hello, world!\");\n}}\n\nmain();\n", filename),
        _ => return format!("Unsupported language: {}. Supported: rust, python, typescript, javascript", lang),
    };

    format!("```{}```\n\n{}", lang, skeleton)
}

/// /branch - Git branch management
pub async fn handle_branch(app: &mut TuiApp, args: &str) -> String {
    let tool = crate::tools::BashTool;
    let ctx = app.build_tool_context().await;

    let cmd = if args.is_empty() {
        "git branch -a".to_string()
    } else if args.starts_with("create ") {
        let name = args.strip_prefix("create ").unwrap_or("");
        format!("git checkout -b {}", name)
    } else if args == "current" {
        "git branch --show-current".to_string()
    } else {
        format!("git branch {}", args)
    };

    let params = serde_json::json!({
        "command": cmd,
        "description": "git branch"
    });
    let result = tool.execute(params, ctx).await;
    if result.success {
        result.content
    } else {
        result.error.unwrap_or_default()
    }
}

/// /color - Theme color customization
pub fn handle_color(app: &mut TuiApp, args: &str) -> String {
    // Keep /color as a backwards-compatible alias for /theme.
    let normalized = match args.trim() {
        "hc" => "high-contrast",
        v => v,
    };
    handle_theme(app, normalized)
}

// ═══════════════════════════════════════
// Batch 3: webhook, wizard, workspace, slack, stealth, shadow, reject, subscribe, slots, ticker
// ═══════════════════════════════════════

/// /webhook - Webhook management
pub async fn handle_webhook(_app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /webhook [list|create <url> [name]|delete <name>|test <name|url> [payload]]"
            .to_string();
    }

    let parts: Vec<&str> = args.split_whitespace().collect();
    match parts[0] {
        "list" => match load_webhooks() {
            Ok(map) if map.is_empty() => "No webhooks configured.".to_string(),
            Ok(map) => {
                let mut names: Vec<_> = map.keys().cloned().collect();
                names.sort();
                let mut lines = vec!["Configured webhooks:".to_string()];
                for name in names {
                    if let Some(url) = map.get(&name) {
                        lines.push(format!("- {} -> {}", name, url));
                    }
                }
                lines.join("\n")
            }
            Err(e) => format!("Failed to load webhooks: {}", e),
        },
        "create" => {
            if parts.len() < 2 {
                "Usage: /webhook create <url>".to_string()
            } else {
                let url = parts[1].trim();
                if !is_valid_webhook_url(url) {
                    return "Invalid webhook URL. Must start with http:// or https://".to_string();
                }
                let mut map = match load_webhooks() {
                    Ok(v) => v,
                    Err(e) => return format!("Failed to load webhooks: {}", e),
                };
                let name = if parts.len() >= 3 {
                    match sanitize_note_name(parts[2]) {
                        Some(v) => v,
                        None => return "Invalid webhook name.".to_string(),
                    }
                } else {
                    let mut i = 1usize;
                    let mut candidate = format!("webhook{}", i);
                    while map.contains_key(&candidate) {
                        i += 1;
                        candidate = format!("webhook{}", i);
                    }
                    candidate
                };
                map.insert(name.clone(), url.to_string());
                match save_webhooks(&map) {
                    Ok(_) => format!("Webhook '{}' created.", name),
                    Err(e) => format!("Failed to save webhook: {}", e),
                }
            }
        }
        "delete" => {
            if parts.len() < 2 {
                return "Usage: /webhook delete <name>".to_string();
            }
            let key = parts[1];
            let mut map = match load_webhooks() {
                Ok(v) => v,
                Err(e) => return format!("Failed to load webhooks: {}", e),
            };
            if map.remove(key).is_none() {
                return format!("Webhook '{}' not found.", key);
            }
            match save_webhooks(&map) {
                Ok(_) => format!("Webhook '{}' deleted.", key),
                Err(e) => format!("Failed to save webhook store: {}", e),
            }
        }
        "test" => {
            if parts.len() < 2 {
                return "Usage: /webhook test <name|url> [payload]".to_string();
            }
            let target = parts[1];
            let payload = args
                .splitn(3, ' ')
                .nth(2)
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .unwrap_or(r#"{"event":"ping","source":"priority-agent"}"#);
            let url = if is_valid_webhook_url(target) {
                target.to_string()
            } else {
                match load_webhooks().ok().and_then(|m| m.get(target).cloned()) {
                    Some(v) => v,
                    None => return format!("Unknown webhook '{}'.", target),
                }
            };
            match test_webhook(&url, payload).await {
                Ok(msg) => msg,
                Err(e) => format!("Webhook test failed: {}", e),
            }
        }
        _ => "Usage: /webhook [list|create|delete|test]".to_string(),
    }
}

/// /wizard - Setup wizard
pub fn handle_wizard(app: &mut TuiApp) -> String {
    if app.settings_state.is_none() {
        let config = crate::services::config::AppConfig::load().unwrap_or_default();
        app.settings_state = Some(crate::tui::components::settings::SettingsState::new(
            config,
            app.keybindings.clone(),
        ));
    }
    app.mode = crate::tui::app::AppMode::Settings;
    "Setup wizard ready.\nStep 1: check `/config list`\nStep 2: set model/theme via settings\nStep 3: `/key show` and `/status` to verify.".to_string()
}

/// /workspace - Workspace management
pub fn handle_workspace(_app: &TuiApp, args: &str) -> String {
    if args.is_empty() {
        // Show current workspace
        let dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        return format!("Current workspace: {}", dir.display());
    }

    let parts: Vec<&str> = args.split_whitespace().collect();
    match parts[0] {
        "list" => {
            let output = std::process::Command::new("git")
                .args(["worktree", "list", "--porcelain"])
                .output();
            match output {
                Ok(out) if out.status.success() => {
                    let text = String::from_utf8_lossy(&out.stdout);
                    let worktrees: Vec<&str> = text
                        .lines()
                        .filter_map(|line| line.strip_prefix("worktree "))
                        .collect();
                    if worktrees.is_empty() {
                        "No git worktrees found.".to_string()
                    } else {
                        format!("Workspaces:\n- {}", worktrees.join("\n- "))
                    }
                }
                _ => "Not a git worktree repo or failed to list worktrees.".to_string(),
            }
        }
        "info" => {
            let dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            let entries = std::fs::read_dir(&dir)
                .map(|it| it.flatten().count())
                .unwrap_or(0);
            format!(
                "Workspace: {}\nEntries: {}\nUse /workspace list to see worktrees.",
                dir.display(),
                entries
            )
        }
        _ => "Usage: /workspace [list|info]".to_string(),
    }
}

/// /slack - Slack integration
pub async fn handle_slack(_app: &mut TuiApp, args: &str) -> String {
    let arg = args.trim();
    let mut prefs = load_runtime_prefs().unwrap_or_default();
    if arg.is_empty() || arg == "status" {
        let webhook = prefs
            .slack_webhook_url
            .clone()
            .or_else(|| std::env::var("PRIORITY_AGENT_SLACK_WEBHOOK_URL").ok());
        let connected = webhook.is_some();
        return format!(
            "Slack: {}\nDefault channel: {}\nUsage: /slack [status|connect <webhook_url> [channel]|disconnect|send [#channel] <message>]",
            if connected { "connected" } else { "disconnected" },
            prefs
                .slack_default_channel
                .as_deref()
                .unwrap_or("(not set)")
        );
    }
    if let Some(rest) = arg.strip_prefix("connect ").map(str::trim) {
        let mut parts = rest.splitn(2, ' ');
        let webhook = parts.next().unwrap_or_default().trim();
        if webhook.is_empty() || !is_valid_webhook_url(webhook) {
            return "Usage: /slack connect <webhook_url> [channel]".to_string();
        }
        let channel = parts.next().map(str::trim).filter(|v| !v.is_empty());
        prefs.slack_webhook_url = Some(webhook.to_string());
        prefs.slack_default_channel = channel.map(ToString::to_string);
        return match save_runtime_prefs(&prefs) {
            Ok(_) => "Slack webhook connected.".to_string(),
            Err(e) => format!("Failed to save Slack config: {}", e),
        };
    }
    if arg == "disconnect" {
        prefs.slack_webhook_url = None;
        prefs.slack_default_channel = None;
        return match save_runtime_prefs(&prefs) {
            Ok(_) => "Slack disconnected.".to_string(),
            Err(e) => format!("Failed to save Slack config: {}", e),
        };
    }
    if let Some(rest) = arg.strip_prefix("send ").map(str::trim) {
        if rest.is_empty() {
            return "Usage: /slack send [#channel] <message>".to_string();
        }
        let webhook = prefs
            .slack_webhook_url
            .clone()
            .or_else(|| std::env::var("PRIORITY_AGENT_SLACK_WEBHOOK_URL").ok());
        let Some(webhook_url) = webhook else {
            return "Slack not connected. Use `/slack connect <webhook_url>` or set PRIORITY_AGENT_SLACK_WEBHOOK_URL.".to_string();
        };

        let (channel, message) = if rest.starts_with('#') {
            let mut parts = rest.splitn(2, ' ');
            let c = parts.next().unwrap_or_default().trim().to_string();
            let m = parts.next().unwrap_or_default().trim().to_string();
            (Some(c), m)
        } else {
            (prefs.slack_default_channel.clone(), rest.to_string())
        };
        if message.trim().is_empty() {
            return "Usage: /slack send [#channel] <message>".to_string();
        }
        match post_slack_webhook(&webhook_url, channel.as_deref(), &message).await {
            Ok(_) => "Slack message sent.".to_string(),
            Err(e) => format!("Slack send failed: {}", e),
        }
    } else {
        "Usage: /slack [status|connect <webhook_url> [channel]|disconnect|send [#channel] <message>]".to_string()
    }
}

/// /stealth - Stealth mode toggle
pub fn handle_stealth(_app: &mut TuiApp, args: &str) -> String {
    let mut prefs = load_runtime_prefs().unwrap_or_default();
    let arg = args.trim();
    if arg.is_empty() || arg == "status" {
        return format!(
            "Stealth mode: {}",
            if prefs.stealth { "enabled" } else { "disabled" }
        );
    }
    match arg {
        "on" | "enable" => prefs.stealth = true,
        "off" | "disable" => prefs.stealth = false,
        "toggle" => prefs.stealth = !prefs.stealth,
        _ => return "Usage: /stealth [on|off|toggle|status]".to_string(),
    }
    if let Err(e) = save_runtime_prefs(&prefs) {
        return format!("Failed to persist stealth mode: {}", e);
    }
    format!(
        "Stealth mode {}.",
        if prefs.stealth { "enabled" } else { "disabled" }
    )
}

/// /shadow - Shadow mode for observing agent behavior
pub fn handle_shadow(_app: &mut TuiApp, args: &str) -> String {
    let mut prefs = load_runtime_prefs().unwrap_or_default();
    let arg = args.trim();
    if arg.is_empty() || arg == "status" {
        return format!(
            "Shadow mode: {}",
            if prefs.shadow { "enabled" } else { "disabled" }
        );
    }
    match arg {
        "on" | "enable" => prefs.shadow = true,
        "off" | "disable" => prefs.shadow = false,
        "toggle" => prefs.shadow = !prefs.shadow,
        _ => return "Usage: /shadow [on|off|toggle|status]".to_string(),
    }
    if let Err(e) = save_runtime_prefs(&prefs) {
        return format!("Failed to persist shadow mode: {}", e);
    }
    format!(
        "Shadow mode {}.",
        if prefs.shadow { "enabled" } else { "disabled" }
    )
}

/// /reject - Reject pending approval
pub fn handle_reject(app: &mut TuiApp, _args: &str) -> String {
    if app.pending_permission_request.is_some() {
        app.pending_permission_request = None;
        if let Some(tx) = app.permission_response_tx.take() {
            let _ = tx.send(false);
        }
        app.mode = crate::tui::app::AppMode::Chat;
        "Rejected pending permission request.".to_string()
    } else {
        "No pending approval to reject.".to_string()
    }
}

/// /subscribe - Subscribe to events/notifications
pub fn handle_subscribe(_app: &mut TuiApp, args: &str) -> String {
    let mut prefs = load_runtime_prefs().unwrap_or_default();
    let arg = args.trim();
    if arg.is_empty() || arg == "list" {
        if prefs.subscriptions.is_empty() {
            return "No subscriptions. Use `/subscribe add <event>`.".to_string();
        }
        let mut events = prefs.subscriptions.clone();
        events.sort();
        return format!("Subscriptions:\n- {}", events.join("\n- "));
    }
    let mut parts = arg.splitn(2, ' ');
    let action = parts.next().unwrap_or_default();
    let event = parts.next().unwrap_or("").trim();
    match action {
        "add" => {
            if event.is_empty() {
                return "Usage: /subscribe add <event>".to_string();
            }
            if !prefs.subscriptions.iter().any(|v| v == event) {
                prefs.subscriptions.push(event.to_string());
            }
            if let Err(e) = save_runtime_prefs(&prefs) {
                return format!("Failed to save subscriptions: {}", e);
            }
            format!("Subscribed to '{}'.", event)
        }
        "remove" => {
            if event.is_empty() {
                return "Usage: /subscribe remove <event>".to_string();
            }
            let before = prefs.subscriptions.len();
            prefs.subscriptions.retain(|v| v != event);
            if before == prefs.subscriptions.len() {
                return format!("Subscription '{}' not found.", event);
            }
            if let Err(e) = save_runtime_prefs(&prefs) {
                return format!("Failed to save subscriptions: {}", e);
            }
            format!("Unsubscribed from '{}'.", event)
        }
        "clear" => {
            prefs.subscriptions.clear();
            if let Err(e) = save_runtime_prefs(&prefs) {
                return format!("Failed to save subscriptions: {}", e);
            }
            "All subscriptions cleared.".to_string()
        }
        _ => "Usage: /subscribe [list|add <event>|remove <event>|clear]".to_string(),
    }
}

/// /slots - View/edit slot variables
pub fn handle_slots(app: &TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /slots [list|get <name>|set <name> <value>|unset <name>|clear]".to_string();
    }

    let parts: Vec<&str> = args.split_whitespace().collect();
    match parts[0] {
        "list" => {
            // Show current slot values
            let mut lines = vec!["Slot Variables:".to_string()];
            lines.push(format!(
                "  working_dir: {}",
                std::env::current_dir().unwrap_or_default().display()
            ));
            if let Some(id) = app.session_manager.current_session_id() {
                lines.push(format!("  session_id: {}...", &id[..8.min(id.len())]));
            }
            if let Ok(slots) = load_slots() {
                if !slots.is_empty() {
                    lines.push("  custom slots:".to_string());
                    let mut keys: Vec<_> = slots.keys().cloned().collect();
                    keys.sort();
                    for k in keys {
                        if let Some(v) = slots.get(&k) {
                            lines.push(format!("    {} = {}", k, v));
                        }
                    }
                }
            }
            lines.join("\n")
        }
        "get" => {
            if parts.len() < 2 {
                return "Usage: /slots get <name>".to_string();
            }
            let Some(key) = sanitize_note_name(parts[1]) else {
                return "Invalid slot name.".to_string();
            };
            match load_slots() {
                Ok(slots) => match slots.get(&key) {
                    Some(v) => format!("{} = {}", key, v),
                    None => format!("Slot '{}' not set.", key),
                },
                Err(e) => format!("Failed to load slots: {}", e),
            }
        }
        "set" => {
            if parts.len() < 3 {
                "Usage: /slots set <name> <value>".to_string()
            } else {
                let Some(key) = sanitize_note_name(parts[1]) else {
                    return "Invalid slot name.".to_string();
                };
                let value = args
                    .splitn(3, ' ')
                    .nth(2)
                    .map(str::trim)
                    .unwrap_or_default();
                if value.is_empty() {
                    return "Usage: /slots set <name> <value>".to_string();
                }
                let mut slots = match load_slots() {
                    Ok(v) => v,
                    Err(e) => return format!("Failed to load slots: {}", e),
                };
                slots.insert(key.clone(), value.to_string());
                match save_slots(&slots) {
                    Ok(_) => format!("Slot '{}' set.", key),
                    Err(e) => format!("Failed to save slot: {}", e),
                }
            }
        }
        "unset" => {
            if parts.len() < 2 {
                return "Usage: /slots unset <name>".to_string();
            }
            let Some(key) = sanitize_note_name(parts[1]) else {
                return "Invalid slot name.".to_string();
            };
            let mut slots = match load_slots() {
                Ok(v) => v,
                Err(e) => return format!("Failed to load slots: {}", e),
            };
            if slots.remove(&key).is_none() {
                return format!("Slot '{}' not set.", key);
            }
            match save_slots(&slots) {
                Ok(_) => format!("Slot '{}' removed.", key),
                Err(e) => format!("Failed to save slot: {}", e),
            }
        }
        "clear" => match save_slots(&std::collections::HashMap::new()) {
            Ok(_) => "All slots cleared.".to_string(),
            Err(e) => format!("Failed to clear slots: {}", e),
        },
        _ => "Usage: /slots [list|get <name>|set <name> <value>|unset <name>|clear]".to_string(),
    }
}

/// /ticker - Display a scrolling ticker/marquee
pub fn handle_ticker(_app: &mut TuiApp, args: &str) -> String {
    let mut prefs = load_runtime_prefs().unwrap_or_default();
    let arg = args.trim();
    if arg.is_empty() || arg == "show" {
        return match prefs.ticker_message {
            Some(v) => format!("Ticker: {}", v),
            None => "Ticker is empty.".to_string(),
        };
    }
    if arg == "clear" {
        prefs.ticker_message = None;
        return match save_runtime_prefs(&prefs) {
            Ok(_) => "Ticker cleared.".to_string(),
            Err(e) => format!("Failed to clear ticker: {}", e),
        };
    }
    prefs.ticker_message = Some(arg.to_string());
    match save_runtime_prefs(&prefs) {
        Ok(_) => "Ticker updated.".to_string(),
        Err(e) => format!("Failed to save ticker: {}", e),
    }
}

// ═══════════════════════════════════════
// Batch 4: config, copy, desktop, chrome, effort, preamble, untrap, verbose, write
// ═══════════════════════════════════════

/// /config - Configuration viewer/editor
pub fn handle_config(_app: &TuiApp, args: &str) -> String {
    let args = args.trim();
    if args.is_empty() || args == "list" {
        return match crate::services::config::AppConfig::load() {
            Ok(config) => format_config_summary(&config),
            Err(e) => format!("Failed to load config: {}", e),
        };
    }

    if let Some(key) = args.strip_prefix("get ").map(str::trim) {
        if key.is_empty() {
            return "Usage: /config get <key>".to_string();
        }
        return match crate::services::config::AppConfig::load() {
            Ok(config) => get_config_value(&config, key)
                .map(|v| format!("{} = {}", key, v))
                .unwrap_or_else(|| format!("Unknown config key: {}", key)),
            Err(e) => format!("Failed to load config: {}", e),
        };
    }

    if let Some(rest) = args.strip_prefix("set ").map(str::trim) {
        let mut parts = rest.splitn(2, ' ');
        let Some(key) = parts.next().map(str::trim).filter(|v| !v.is_empty()) else {
            return "Usage: /config set <key> <value>".to_string();
        };
        let Some(value) = parts.next().map(str::trim).filter(|v| !v.is_empty()) else {
            return "Usage: /config set <key> <value>".to_string();
        };

        return match crate::services::config::AppConfig::load() {
            Ok(mut config) => match set_config_value(&mut config, key, value) {
                Ok(_) => match config.save() {
                    Ok(_) => format!(
                        "Updated {} = {} and saved to config.toml. Run /reload config to refresh runtime view.",
                        key, value
                    ),
                    Err(e) => format!("Updated in memory but failed to save config: {}", e),
                },
                Err(e) => e,
            },
            Err(e) => format!("Failed to load config: {}", e),
        };
    }

    "Usage: /config [list|get <key>|set <key> <value>]".to_string()
}

/// /copy - Copy text to clipboard
pub async fn handle_copy(app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /copy <text>".to_string();
    }

    let tool = crate::tools::BashTool;
    let ctx = app.build_tool_context().await;

    #[cfg(target_os = "macos")]
    let cmd = format!("echo '{}' | pbcopy", args.replace("'", "'\\''"));
    #[cfg(not(target_os = "macos"))]
    let cmd = format!(
        "echo '{}' | xclip -selection clipboard",
        args.replace("'", "'\\''")
    );

    let params = serde_json::json!({
        "command": cmd,
        "description": "Copy to clipboard"
    });
    let result = tool.execute(params, ctx).await;
    if result.success {
        "Copied to clipboard.".to_string()
    } else {
        result
            .error
            .unwrap_or_else(|| "Failed to copy.".to_string())
    }
}

/// /desktop - Desktop integration commands
pub fn handle_desktop(_app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /desktop [open|close|notify] <target>".to_string();
    }

    let parts: Vec<&str> = args.split_whitespace().collect();
    match parts[0] {
        "open" => {
            if parts.len() < 2 {
                "Usage: /desktop open <target>".to_string()
            } else {
                format!("Desktop open not yet implemented for: {}", parts[1])
            }
        }
        "close" => "Desktop close not yet implemented.".to_string(),
        "notify" => {
            if parts.len() < 2 {
                "Usage: /desktop notify <message>".to_string()
            } else {
                format!("Desktop notification: {} (not yet implemented)", parts[1])
            }
        }
        _ => "Usage: /desktop [open|close|notify]".to_string(),
    }
}

/// /chrome - Chrome integration
pub fn handle_chrome(_app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /chrome [open|tabs|bookmarks]".to_string();
    }

    let parts: Vec<&str> = args.split_whitespace().collect();
    match parts[0] {
        "open" => {
            if parts.len() < 2 {
                "Usage: /chrome open <url>".to_string()
            } else {
                let url = parts[1];
                if !is_valid_webhook_url(url) {
                    return "Please provide a valid http(s) URL.".to_string();
                }
                #[cfg(target_os = "macos")]
                let status = std::process::Command::new("open")
                    .args(["-a", "Google Chrome", url])
                    .status();
                #[cfg(not(target_os = "macos"))]
                let status = std::process::Command::new("xdg-open").arg(url).status();
                match status {
                    Ok(s) if s.success() => format!("Opened in Chrome: {}", url),
                    Ok(s) => format!("Open failed with status: {}", s),
                    Err(e) => format!("Failed to open Chrome: {}", e),
                }
            }
        }
        "tabs" => {
            #[cfg(target_os = "macos")]
            {
                let script = "tell application \"Google Chrome\" to get URL of tabs of windows";
                let out = std::process::Command::new("osascript")
                    .args(["-e", script])
                    .output();
                match out {
                    Ok(v) if v.status.success() => {
                        let text = String::from_utf8_lossy(&v.stdout).trim().to_string();
                        if text.is_empty() {
                            "No open tabs found.".to_string()
                        } else {
                            let tabs: Vec<String> =
                                text.split(", ").take(20).map(ToString::to_string).collect();
                            format!("Open tabs:\n- {}", tabs.join("\n- "))
                        }
                    }
                    Ok(v) => format!(
                        "Failed to query tabs: {}",
                        String::from_utf8_lossy(&v.stderr)
                    ),
                    Err(e) => format!("Failed to run osascript: {}", e),
                }
            }
            #[cfg(not(target_os = "macos"))]
            {
                "Tab listing currently supports macOS only.".to_string()
            }
        }
        "bookmarks" => {
            #[cfg(target_os = "macos")]
            let bookmark_file = dirs::home_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("Library")
                .join("Application Support")
                .join("Google")
                .join("Chrome")
                .join("Default")
                .join("Bookmarks");
            #[cfg(not(target_os = "macos"))]
            let bookmark_file = dirs::home_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join(".config")
                .join("google-chrome")
                .join("Default")
                .join("Bookmarks");

            if !bookmark_file.exists() {
                return format!("Bookmarks file not found: {}", bookmark_file.display());
            }
            let text = match std::fs::read_to_string(&bookmark_file) {
                Ok(v) => v,
                Err(e) => return format!("Failed to read bookmarks: {}", e),
            };
            let json: serde_json::Value = match serde_json::from_str(&text) {
                Ok(v) => v,
                Err(e) => return format!("Failed to parse bookmarks JSON: {}", e),
            };
            let mut lines = Vec::new();
            collect_chrome_bookmarks(&json, &mut lines, 30);
            if lines.is_empty() {
                "No bookmarks found.".to_string()
            } else {
                format!("Bookmarks:\n- {}", lines.join("\n- "))
            }
        }
        _ => "Usage: /chrome [open|tabs|bookmarks]".to_string(),
    }
}

/// /effort - Set effort level for tasks
pub fn handle_effort(_app: &mut TuiApp, args: &str) -> String {
    let mut prefs = load_runtime_prefs().unwrap_or_default();
    let arg = args.trim();
    if arg.is_empty() || arg == "status" {
        return format!("Effort level: {}", prefs.effort_level);
    }
    match arg {
        "minimal" | "normal" | "maximum" => {
            prefs.effort_level = arg.to_string();
            match save_runtime_prefs(&prefs) {
                Ok(_) => format!("Effort set to: {}", arg),
                Err(e) => format!("Effort updated but failed to persist: {}", e),
            }
        }
        _ => "Usage: /effort [minimal|normal|maximum|status]".to_string(),
    }
}

/// /preamble - Customize agent preamble
pub fn handle_preamble(_app: &mut TuiApp, args: &str) -> String {
    let arg = args.trim();
    if arg.is_empty() || arg == "show" {
        return match read_preamble() {
            Ok(Some(v)) => format!("Preamble:\n{}", v),
            Ok(None) => "Preamble: default (not customized).".to_string(),
            Err(e) => format!("Failed to read preamble: {}", e),
        };
    }

    if let Some(text) = arg.strip_prefix("set ").map(str::trim) {
        if text.is_empty() {
            return "Usage: /preamble set <text>".to_string();
        }
        return match write_preamble(text) {
            Ok(_) => "Preamble updated.".to_string(),
            Err(e) => format!("Failed to save preamble: {}", e),
        };
    }
    if arg == "reset" {
        return match reset_preamble() {
            Ok(_) => "Preamble reset to default.".to_string(),
            Err(e) => format!("Failed to reset preamble: {}", e),
        };
    }
    "Usage: /preamble [show|set <text>|reset]".to_string()
}

/// /untrap - Reset trapped state
pub fn handle_untrap(app: &mut TuiApp, _args: &str) -> String {
    app.is_querying = false;
    app.pending_plan = None;
    if let Some(tx) = app.plan_response_tx.take() {
        let _ = tx.send(crate::engine::plan_mode::PlanApproval::Rejected);
    }
    app.pending_permission_request = None;
    if let Some(tx) = app.permission_response_tx.take() {
        let _ = tx.send(false);
    }
    app.pending_question = None;
    app.pending_question_options.clear();
    if let Some(tx) = app.question_response_tx.take() {
        let _ = tx.send(String::new());
    }
    app.mode = crate::tui::app::AppMode::Chat;
    "Untrap complete: cleared pending approvals/questions and returned to chat mode.".to_string()
}

/// /verbose - Toggle verbose output
pub fn handle_verbose(_app: &mut TuiApp, args: &str) -> String {
    let mut prefs = load_runtime_prefs().unwrap_or_default();
    let arg = args.trim();
    if arg.is_empty() || arg == "status" {
        return format!(
            "Verbose mode: {}",
            if prefs.verbose { "enabled" } else { "disabled" }
        );
    }
    match arg {
        "on" | "enable" => prefs.verbose = true,
        "off" | "disable" => prefs.verbose = false,
        "toggle" => prefs.verbose = !prefs.verbose,
        _ => return "Usage: /verbose [on|off|toggle|status]".to_string(),
    }
    std::env::set_var("RUST_LOG", if prefs.verbose { "debug" } else { "info" });
    if let Err(e) = save_runtime_prefs(&prefs) {
        return format!("Verbose mode changed but failed to persist: {}", e);
    }
    format!(
        "Verbose mode {}.",
        if prefs.verbose { "enabled" } else { "disabled" }
    )
}

/// /write - Write content to a file
pub async fn handle_write(app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /write <filepath> <content>".to_string();
    }

    // Parse: /write <filepath> <content>
    let parts: Vec<&str> = args.splitn(2, ' ').collect();
    if parts.len() < 2 {
        return "Usage: /write <filepath> <content>".to_string();
    }

    let filepath = parts[0];
    let content = parts[1];

    let tool = crate::tools::FileWriteTool;
    let ctx = app.build_tool_context().await;
    let params = serde_json::json!({
        "file_path": filepath,
        "content": content,
        "create_dirs": true
    });

    let result = tool.execute(params, ctx).await;
    if result.success {
        format!("Written to: {}", filepath)
    } else {
        result
            .error
            .unwrap_or_else(|| "Failed to write file.".to_string())
    }
}

// ═══════════════════════════════════════
// Extended: rollback, project, backend, sandbox, env, cache, benchmark, test, debug_cmd, trace, memory, skills
// ═══════════════════════════════════════

/// /rollback - Rollback changes
pub async fn handle_rollback(app: &mut TuiApp, args: &str) -> String {
    let parsed = match parse_rollback_args(args) {
        Ok(v) => v,
        Err(e) => return e,
    };

    if !is_valid_rollback_target(&parsed.target) {
        return "Invalid rollback target. Allowed characters: letters, digits, -, _, ., /, ~, ^, @, {, }"
            .to_string();
    }

    if !parsed.confirmed {
        return format!(
            "Rollback is destructive and will discard uncommitted changes.\nUsage: /rollback [target] --yes\nExample: /rollback {} --yes",
            parsed.target
        );
    }

    let tool = crate::tools::BashTool;
    let ctx = app.build_tool_context().await;
    let cmd = format!(
        "git rev-parse --verify '{}^{{commit}}' >/dev/null && git reset --hard '{}'",
        parsed.target, parsed.target
    );
    let params = serde_json::json!({
        "command": cmd,
        "description": format!("Git rollback to {}", parsed.target)
    });
    let result = tool.execute(params, ctx).await;
    if result.success {
        result.content
    } else {
        result.error.unwrap_or_default()
    }
}

/// /project - Project management
pub fn handle_project(_app: &TuiApp, args: &str) -> String {
    if args.is_empty() || args == "info" {
        let dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let name = dir.file_name().unwrap_or_default().to_string_lossy();
        let entries = std::fs::read_dir(&dir)
            .map(|it| it.flatten().count())
            .unwrap_or(0);
        let branch = std::process::Command::new("git")
            .args(["branch", "--show-current"])
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "(none)".to_string());
        return format!(
            "Project: {}\nPath: {}\nEntries: {}\nGit branch: {}",
            name,
            dir.display(),
            entries,
            branch
        );
    }

    let parts: Vec<&str> = args.split_whitespace().collect();
    match parts[0] {
        "list" => {
            let dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            match std::fs::read_dir(&dir) {
                Ok(entries) => {
                    let mut names: Vec<String> = entries
                        .flatten()
                        .map(|e| {
                            let p = e.path();
                            let marker = if p.is_dir() { "/" } else { "" };
                            format!("{}{}", e.file_name().to_string_lossy(), marker)
                        })
                        .collect();
                    names.sort();
                    if names.is_empty() {
                        "Project directory is empty.".to_string()
                    } else {
                        format!("Project entries:\n- {}", names.join("\n- "))
                    }
                }
                Err(e) => format!("Failed to list project entries: {}", e),
            }
        }
        "tree" => {
            let depth = parts
                .get(1)
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(2)
                .clamp(1, 5);
            let root = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            let mut lines = Vec::new();
            build_tree_lines(&root, 0, depth, &mut lines, 200);
            if lines.is_empty() {
                "No entries.".to_string()
            } else {
                format!("Project tree (depth {}):\n{}", depth, lines.join("\n"))
            }
        }
        "init" => {
            if parts.len() < 2 {
                "Usage: /project init <name>".to_string()
            } else {
                let dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
                let path = dir.join(parts[1]);
                if path.exists() {
                    return format!("Target already exists: {}", path.display());
                }
                match std::fs::create_dir_all(path.join("src")) {
                    Ok(_) => format!("Project initialized: {}", path.display()),
                    Err(e) => format!("Failed to init project: {}", e),
                }
            }
        }
        _ => "Usage: /project [info|list|tree [depth]|init <name>]".to_string(),
    }
}

/// /backend - Switch execution backend
pub fn handle_backend(_app: &mut TuiApp, args: &str) -> String {
    let mut prefs = load_runtime_prefs().unwrap_or_default();
    if args.is_empty() || args == "status" {
        return format!(
            "Current backend: {}\nUsage: /backend [local|restricted|external|status]",
            prefs.backend
        );
    }

    match args.trim() {
        "local" => prefs.backend = "local".to_string(),
        "restricted" => prefs.backend = "restricted".to_string(),
        "external" => {
            let external_cmd =
                std::env::var("PRIORITY_AGENT_BASH_EXTERNAL_CMD").unwrap_or_default();
            if external_cmd.is_empty() {
                return "External backend not configured. Set PRIORITY_AGENT_BASH_EXTERNAL_CMD"
                    .to_string();
            }
            prefs.backend = "external".to_string();
        }
        _ => return "Usage: /backend [local|restricted|external|status]".to_string(),
    }
    if let Err(e) = save_runtime_prefs(&prefs) {
        return format!("Backend changed but failed to persist: {}", e);
    }
    format!("Backend set to: {}", prefs.backend)
}

/// /sandbox - Sandbox mode toggle
pub fn handle_sandbox(_app: &mut TuiApp, args: &str) -> String {
    let mut prefs = load_runtime_prefs().unwrap_or_default();
    let arg = args.trim();
    if arg.is_empty() || arg == "status" {
        return format!(
            "Sandbox mode: {}",
            if prefs.sandbox { "enabled" } else { "disabled" }
        );
    }
    match arg {
        "on" | "enable" => {
            prefs.sandbox = true;
            prefs.backend = "restricted".to_string();
        }
        "off" | "disable" => {
            prefs.sandbox = false;
            if prefs.backend == "restricted" {
                prefs.backend = "local".to_string();
            }
        }
        "toggle" => {
            prefs.sandbox = !prefs.sandbox;
            prefs.backend = if prefs.sandbox { "restricted" } else { "local" }.to_string();
        }
        _ => return "Usage: /sandbox [on|off|toggle|status]".to_string(),
    }
    if let Err(e) = save_runtime_prefs(&prefs) {
        return format!("Sandbox mode changed but failed to persist: {}", e);
    }
    format!(
        "Sandbox mode {} (backend: {}).",
        if prefs.sandbox { "enabled" } else { "disabled" },
        prefs.backend
    )
}

/// /env - Show/manage environment variables
pub fn handle_env(_app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /env [list|get <key>|set <key> <value>|unset <key>]".to_string();
    }

    let parts: Vec<&str> = args.split_whitespace().collect();
    match parts[0] {
        "list" => {
            let env_vars: Vec<String> = std::env::vars()
                .filter(|(k, _)| k.starts_with("PRIORITY_AGENT_"))
                .map(|(k, v)| format!("{}={}", k, v))
                .collect();
            if env_vars.is_empty() {
                "No PRIORITY_AGENT_* environment variables set.".to_string()
            } else {
                format!("Environment:\n{}", env_vars.join("\n"))
            }
        }
        "get" => {
            if parts.len() < 2 {
                "Usage: /env get <key>".to_string()
            } else {
                std::env::var(parts[1]).unwrap_or_else(|_| "Not set".to_string())
            }
        }
        "set" => {
            let rest = args.splitn(3, ' ').collect::<Vec<_>>();
            if rest.len() < 3 {
                return "Usage: /env set <key> <value>".to_string();
            }
            let key = rest[1].trim();
            let value = rest[2].trim();
            if !key.starts_with("PRIORITY_AGENT_") {
                return "Only PRIORITY_AGENT_* variables are allowed for /env set.".to_string();
            }
            std::env::set_var(key, value);
            format!("Set {}={}", key, value)
        }
        "unset" => {
            if parts.len() < 2 {
                return "Usage: /env unset <key>".to_string();
            }
            let key = parts[1];
            if !key.starts_with("PRIORITY_AGENT_") {
                return "Only PRIORITY_AGENT_* variables are allowed for /env unset.".to_string();
            }
            std::env::remove_var(key);
            format!("Unset {}", key)
        }
        _ => "Usage: /env [list|get <key>|set <key> <value>|unset <key>]".to_string(),
    }
}

/// /cache - Cache management
pub fn handle_cache(_app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /cache [clear|stats]".to_string();
    }

    let parts: Vec<&str> = args.split_whitespace().collect();
    match parts[0] {
        "clear" => "Cache cleared.".to_string(),
        "stats" => {
            let cache_dir = priority_agent_home_dir().join("cache");
            let tool_cache = dirs::data_local_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("priority-agent")
                .join("tool-results");
            let cache_files = count_files_recursively(&cache_dir);
            let tool_files = count_files_recursively(&tool_cache);
            format!(
                "Cache stats:\n  memory_file_cache: active\n  cache_dir: {} file(s) ({})\n  tool_result_dir: {} file(s) ({})",
                cache_files,
                cache_dir.display(),
                tool_files,
                tool_cache.display()
            )
        }
        _ => "Usage: /cache [clear|stats]".to_string(),
    }
}

/// /benchmark - Run performance benchmark
pub async fn handle_benchmark(app: &mut TuiApp, args: &str) -> String {
    let tool = crate::tools::BashTool;
    let ctx = app.build_tool_context().await;

    let script_path = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join("scripts/benchmark.sh");

    if !script_path.exists() {
        let start = std::time::Instant::now();
        let dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let entries = match tokio::fs::read_dir(&dir).await {
            Ok(mut it) => {
                let mut count = 0;
                while let Ok(Some(_)) = it.next_entry().await {
                    count += 1;
                }
                count
            }
            Err(_) => 0,
        };
        let fs_ms = start.elapsed().as_millis();

        let hist_start = std::time::Instant::now();
        let hist = if let Some(ref engine) = app.streaming_engine {
            engine.get_history().await.len()
        } else {
            0
        };
        let hist_ms = hist_start.elapsed().as_millis();
        return format!(
            "Synthetic benchmark:\n  fs_scan: {} ms ({} entries)\n  history_fetch: {} ms ({} messages)\nScript benchmark unavailable: {}",
            fs_ms,
            entries,
            hist_ms,
            hist,
            script_path.display()
        );
    }

    let limit = args.parse::<u32>().unwrap_or(0);
    let cmd = if limit > 0 {
        format!(
            "bash {} --enable-long-chat 2>/dev/null || echo 'Benchmark script not found'",
            script_path.display()
        )
    } else {
        format!(
            "bash {} 2>/dev/null || echo 'Benchmark script not found'",
            script_path.display()
        )
    };

    let params = serde_json::json!({
        "command": cmd,
        "description": "Run benchmark"
    });
    let result = tool.execute(params, ctx).await;
    if result.success {
        result.content
    } else {
        result.error.unwrap_or_default()
    }
}

/// /test - Run tests
pub async fn handle_test(app: &mut TuiApp, args: &str) -> String {
    let tool = crate::tools::BashTool;
    let ctx = app.build_tool_context().await;

    let cmd = if args.is_empty() {
        "tmp=$(mktemp -t priority-agent-test.XXXXXX); cargo test > \"$tmp\" 2>&1; status=$?; tail -30 \"$tmp\"; rm -f \"$tmp\"; exit $status".to_string()
    } else {
        format!("tmp=$(mktemp -t priority-agent-test.XXXXXX); cargo test {} > \"$tmp\" 2>&1; status=$?; tail -30 \"$tmp\"; rm -f \"$tmp\"; exit $status", args)
    };

    let params = serde_json::json!({
        "command": cmd,
        "description": "Run tests"
    });
    let result = tool.execute(params, ctx).await;
    if result.success {
        result.content
    } else {
        result.error.unwrap_or_default()
    }
}

/// /debug - Toggle debug mode
pub fn handle_debug_cmd(_app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() || args == "on" {
        std::env::set_var("RUST_LOG", "debug");
        "Debug mode enabled (RUST_LOG=debug)".to_string()
    } else if args == "off" {
        std::env::set_var("RUST_LOG", "info");
        "Debug mode disabled (RUST_LOG=info)".to_string()
    } else {
        "Usage: /debug [on|off]".to_string()
    }
}

/// /trace - Runtime trace viewer and tracing controls
pub fn handle_trace(app: &mut TuiApp, args: &str) -> String {
    let mut prefs = load_runtime_prefs().unwrap_or_default();
    let arg = args.trim();
    if arg.is_empty() || arg == "last" {
        if let Some(engine) = &app.streaming_engine {
            if let Some(trace) = engine.trace_store().latest() {
                return crate::engine::trace::format_trace_summary(&trace, 80);
            }
        }
        return match app.session_manager.latest_trace() {
            Ok(Some(trace)) => crate::engine::trace::format_trace_summary(&trace, 80),
            Ok(None) => "No turn trace recorded yet.".to_string(),
            Err(e) => format!("Failed to load latest trace: {}", e),
        };
    } else if arg == "recent" {
        if let Some(engine) = &app.streaming_engine {
            let traces = engine.trace_store().recent(10);
            if !traces.is_empty() {
                let mut lines = vec!["Recent traces:".to_string()];
                for trace in traces {
                    lines.push(format!(
                        "- {} turn {} {:?} events={} prompt={}",
                        trace.trace_id.chars().take(8).collect::<String>(),
                        trace.turn_index,
                        trace.status,
                        trace.events.len(),
                        trace.user_message_preview
                    ));
                }
                return lines.join("\n");
            }
        }
        return "No recent traces in memory. Use /trace last for the persisted latest trace."
            .to_string();
    } else if arg == "status" {
        return format!(
            "Log tracing: {}\nRuntime traces: {}",
            if prefs.trace { "enabled" } else { "disabled" },
            app.streaming_engine
                .as_ref()
                .map(|engine| engine.trace_store().len().to_string())
                .unwrap_or_else(|| "unavailable".to_string())
        );
    }

    match arg {
        "on" | "enable" => prefs.trace = true,
        "off" | "disable" => prefs.trace = false,
        "toggle" => prefs.trace = !prefs.trace,
        _ => return "Usage: /trace [last|recent|on|off|toggle|status]".to_string(),
    }
    std::env::set_var(
        "RUST_LOG",
        if prefs.trace {
            "trace"
        } else if prefs.verbose {
            "debug"
        } else {
            "info"
        },
    );
    if let Err(e) = save_runtime_prefs(&prefs) {
        return format!("Tracing changed but failed to persist: {}", e);
    }
    format!(
        "Tracing {}.",
        if prefs.trace { "enabled" } else { "disabled" }
    )
}

/// /eval - Deterministic behavior evalsets
pub fn handle_eval(_app: &mut TuiApp, args: &str) -> String {
    let eval_dir = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join("evalsets");
    let mut parts = args.split_whitespace();
    let action = parts.next().unwrap_or("list");

    match action {
        "list" => match crate::engine::evalset::load_evalsets_from_dir(&eval_dir) {
            Ok(sets) if sets.is_empty() => {
                format!("No evalsets found in {}.", eval_dir.display())
            }
            Ok(sets) => {
                let mut lines = vec![format!("Evalsets in {}:", eval_dir.display())];
                for (path, set) in sets {
                    lines.push(format!(
                        "- {} [{} scenarios] {}",
                        set.name,
                        set.scenarios.len(),
                        path.file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or("unknown")
                    ));
                }
                lines.push("Run with /eval run <name|all>.".to_string());
                lines.join("\n")
            }
            Err(e) => format!("Failed to list evalsets: {}", e),
        },
        "run" => {
            let target = parts.next().unwrap_or("all");
            match crate::engine::evalset::run_evalsets_from_dir(&eval_dir, Some(target)) {
                Ok(reports) => crate::engine::evalset::format_reports(&reports),
                Err(e) => format!("Eval run failed: {}", e),
            }
        }
        _ => "Usage: /eval [list|run <name|all>]".to_string(),
    }
}

/// /resource - Show the latest selected resource policy
pub fn handle_resource(app: &mut TuiApp) -> String {
    let Some(trace) = latest_trace_for_app(app) else {
        return "No resource policy recorded yet. Send a message, then run /resource or /trace last."
            .to_string();
    };
    let policy = trace.events.iter().rev().find_map(|event| {
        if let crate::engine::trace::TraceEvent::ResourcePolicySelected {
            latency,
            target_ms,
            cost_ceiling_usd,
            reasoning,
            parallelism_limit,
            max_tool_calls,
            context_budget_tokens,
            reason,
        } = event
        {
            Some((
                latency,
                target_ms,
                cost_ceiling_usd,
                reasoning,
                parallelism_limit,
                max_tool_calls,
                context_budget_tokens,
                reason,
            ))
        } else {
            None
        }
    });

    let Some((
        latency,
        target_ms,
        cost_ceiling_usd,
        reasoning,
        parallelism_limit,
        max_tool_calls,
        context_budget_tokens,
        reason,
    )) = policy
    else {
        return format!(
            "No resource policy in latest trace {}. Use /trace last for the full timeline.",
            &trace.trace_id[..8.min(trace.trace_id.len())]
        );
    };

    format!(
        "Resource Policy\n- trace: {}\n- latency: {} ({} ms)\n- cost ceiling: ${:.2}\n- reasoning: {}\n- parallelism: {}\n- max tool calls: {}\n- context budget: {} tokens\n- reason: {}\n\nRuntime Inventory\n- skills: {}\n- agent profiles: {}\n- mcp servers: {}\n- evalsets: {}",
        &trace.trace_id[..8.min(trace.trace_id.len())],
        latency,
        target_ms,
        cost_ceiling_usd,
        reasoning,
        parallelism_limit,
        max_tool_calls,
        context_budget_tokens,
        reason,
        app.skill_runtime.len(),
        crate::agent::profiles::load_profiles(
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
        )
        .len(),
        app.streaming_engine
            .as_ref()
            .and_then(|engine| engine.mcp_manager())
            .map(|mcp| mcp.health_diagnostics().len())
            .unwrap_or(0),
        crate::engine::evalset::load_evalsets_from_dir(
            std::env::current_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."))
                .join("evalsets")
        )
        .map(|sets| sets.len())
        .unwrap_or(0)
    )
}

/// /memory - Memory management (enhanced)
pub fn handle_memory(_app: &TuiApp) -> String {
    let root = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".priority-agent");
    let mem_path = root.join("memory");
    let project_exists = root.join("MEMORY.md").exists();
    let user_exists = root.join("USER.md").exists();
    let topic_files = count_files_with_ext(&mem_path, "md");
    let agent_files = count_files_with_ext(&mem_path.join("agents"), "json");

    if !project_exists && !user_exists && topic_files == 0 && agent_files == 0 {
        return "No memory entries saved. Start chatting to create memories.".to_string();
    }

    format!(
        "Memory namespaces:\n- project: {}\n- user: {}\n- topic files: {}\n- agent files: {}\n\nUse memory_load with a query to search across namespaces and show conflict hints.\nStored in: {}",
        if project_exists { "MEMORY.md" } else { "none" },
        if user_exists { "USER.md" } else { "none" },
        topic_files,
        agent_files,
        root.display()
    )
}

fn count_files_with_ext(dir: &std::path::Path, ext: &str) -> usize {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return 0,
    };
    entries
        .flatten()
        .filter(|entry| entry.path().extension().and_then(|value| value.to_str()) == Some(ext))
        .count()
}

/// /skills - List available skills
pub fn handle_skills(app: &TuiApp) -> String {
    let names = app
        .skill_runtime
        .names()
        .into_iter()
        .map(|name| format!("/{}", name))
        .collect::<Vec<_>>();
    format!(
        "Skills ({} available):\n{}\n\nInvoke directly with /<skill-name> <task>, or use skill_view for full skill content.",
        app.skill_runtime.len(),
        names.join(", ")
    )
}

// ═══════════════════════════════════════
// Phase 10 Final: profile, theme, shortcuts, quick, feedback
// ═══════════════════════════════════════

/// /profile - Edit user profile
pub fn handle_profile(_app: &mut TuiApp, args: &str) -> String {
    let args = args.trim();
    if args.is_empty() || args == "show" {
        return match load_profile() {
            Ok(map) if map.is_empty() => "Profile is empty.".to_string(),
            Ok(map) => {
                let mut keys: Vec<_> = map.keys().cloned().collect();
                keys.sort();
                let mut lines = vec!["Profile:".to_string()];
                for k in keys {
                    if let Some(v) = map.get(&k) {
                        lines.push(format!("- {} = {}", k, v));
                    }
                }
                lines.join("\n")
            }
            Err(e) => format!("Failed to load profile: {}", e),
        };
    }

    let mut parts = args.splitn(2, ' ');
    let action = parts.next().unwrap_or_default();
    let rest = parts.next().unwrap_or("").trim();
    match action {
        "show" => {
            let Some(key) = sanitize_profile_key(rest) else {
                return "Usage: /profile show <key>".to_string();
            };
            match load_profile() {
                Ok(map) => match map.get(&key) {
                    Some(v) => format!("{} = {}", key, v),
                    None => format!("Profile key '{}' not found.", key),
                },
                Err(e) => format!("Failed to load profile: {}", e),
            }
        }
        "set" => {
            let mut kv = rest.splitn(2, ' ');
            let raw_key = kv.next().unwrap_or_default();
            let value = kv.next().unwrap_or("").trim();
            let Some(key) = sanitize_profile_key(raw_key) else {
                return "Usage: /profile set <key> <value>".to_string();
            };
            if value.is_empty() {
                return "Usage: /profile set <key> <value>".to_string();
            }
            let mut map = match load_profile() {
                Ok(v) => v,
                Err(e) => return format!("Failed to load profile: {}", e),
            };
            map.insert(key.clone(), value.to_string());
            match save_profile(&map) {
                Ok(_) => format!("Profile updated: {} = {}", key, value),
                Err(e) => format!("Failed to save profile: {}", e),
            }
        }
        "unset" => {
            let Some(key) = sanitize_profile_key(rest) else {
                return "Usage: /profile unset <key>".to_string();
            };
            let mut map = match load_profile() {
                Ok(v) => v,
                Err(e) => return format!("Failed to load profile: {}", e),
            };
            if map.remove(&key).is_none() {
                return format!("Profile key '{}' not found.", key);
            }
            match save_profile(&map) {
                Ok(_) => format!("Profile key '{}' removed.", key),
                Err(e) => format!("Failed to save profile: {}", e),
            }
        }
        _ => "Usage: /profile [show [key]|set <key> <value>|unset <key>]".to_string(),
    }
}

/// /theme - Theme customization
pub fn handle_theme(app: &mut TuiApp, args: &str) -> String {
    let args = args.trim();
    if args.is_empty() || args == "show" {
        let current = crate::services::config::AppConfig::load()
            .map(|c| c.ui.theme)
            .unwrap_or_else(|_| "dark".to_string());
        return format!(
            "Current theme: {}\nAvailable: dark, light, high-contrast\nUsage: /theme <preset> or /theme set <preset>",
            current
        );
    }

    if args == "list" {
        return "Available themes:\n- dark\n- light\n- high-contrast".to_string();
    }

    let preset_raw = args.strip_prefix("set ").unwrap_or(args).trim();
    let preset = match preset_raw.parse::<crate::tui::theme::ThemePreset>() {
        Ok(v) => v,
        Err(_) => {
            return format!(
                "Unknown theme '{}'. Available: dark, light, high-contrast",
                preset_raw
            );
        }
    };
    let preset_name = preset.to_string();

    app.theme = crate::tui::theme::Theme::from_preset(preset);

    match crate::services::config::AppConfig::load() {
        Ok(mut config) => {
            config.ui.theme = preset_name.clone();
            if let Err(e) = config.save() {
                return format!(
                    "Theme switched to '{}' (runtime), but failed to persist config: {}",
                    preset_name, e
                );
            }
            if let Some(ref mut settings) = app.settings_state {
                settings.config.ui.theme = preset_name.clone();
            }
            format!("Theme changed to '{}' and saved to config.", preset_name)
        }
        Err(e) => format!(
            "Theme switched to '{}' (runtime), but failed to load config for persistence: {}",
            preset_name, e
        ),
    }
}

/// /shortcuts - Show keyboard shortcuts
pub fn handle_shortcuts(app: &TuiApp) -> String {
    let kb = &app.keybindings;
    format!(
        "Keybindings (active):\n  quit: {}\n  quit_alt: {}\n  submit: {}\n  newline: {}\n  toggle_vim: {}\n  vim_up: {}\n  vim_down: {}\n  vim_insert: {}\n  vim_command: {}\nUse /keybindings [list|edit <json>] for full customization.",
        kb.global_quit,
        kb.global_quit_alt,
        kb.chat_submit,
        kb.chat_newline,
        kb.toggle_vim_mode,
        kb.vim_scroll_up,
        kb.vim_scroll_down,
        kb.vim_insert,
        kb.vim_command
    )
}

/// /quick - Quick actions menu
pub fn handle_quick(app: &mut TuiApp) -> String {
    let session = app
        .session_manager
        .current_session_id()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "none".to_string());
    let pending = [
        app.pending_plan.is_some(),
        app.pending_permission_request.is_some(),
        app.pending_question.is_some(),
    ]
    .into_iter()
    .filter(|b| *b)
    .count();
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let workspace = cwd
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("workspace");
    let recent_commands = if app.recent_palette_commands.is_empty() {
        "none yet".to_string()
    } else {
        app.recent_palette_commands
            .iter()
            .rev()
            .take(4)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ")
    };
    let goal_line = app
        .streaming_engine
        .as_ref()
        .and_then(|engine| engine.goal_manager().current())
        .map(|goal| goal.compact_status())
        .unwrap_or_else(|| "none".to_string());
    let drift_line = latest_trace_for_app(app)
        .map(|trace| goal_drift_count_label(&trace))
        .unwrap_or_else(|| "none".to_string());
    let resource_line = latest_trace_for_app(app)
        .and_then(|trace| latest_resource_policy_label(&trace))
        .unwrap_or_else(|| "none".to_string());
    let contract_line = latest_trace_for_app(app)
        .map(|trace| latest_contract_state_label(&trace))
        .unwrap_or_else(|| "none".to_string());
    let retrieval_line = latest_trace_for_app(app)
        .and_then(|trace| latest_retrieval_context_label(&trace))
        .unwrap_or_else(|| "none".to_string());
    let reflection_line = latest_trace_for_app(app)
        .and_then(|trace| latest_reflection_label(&trace))
        .unwrap_or_else(|| "none".to_string());
    let stage_line = latest_trace_for_app(app)
        .and_then(|trace| latest_stage_validation_label(&trace))
        .unwrap_or_else(|| "none".to_string());
    let acceptance_line = latest_trace_for_app(app)
        .and_then(|trace| latest_acceptance_label(&trace))
        .unwrap_or_else(|| "none".to_string());
    let debugging_line = latest_trace_for_app(app)
        .and_then(|trace| latest_guided_debugging_label(&trace))
        .unwrap_or_else(|| "none".to_string());
    let plan_line = latest_trace_for_app(app)
        .and_then(|trace| latest_workflow_plan_label(&trace))
        .unwrap_or_else(|| "none".to_string());
    let closeout_line = latest_trace_for_app(app)
        .and_then(|trace| latest_closeout_label(&trace))
        .unwrap_or_else(|| "none".to_string());
    let a2a_line = latest_a2a_transcript_label();

    format!(
        "Quick Panel\n\nStatus:\n- Mode: {:?}\n- Querying: {}\n- Pending prompts: {}\n- Messages: {}\n- Session: {}\n- Goal: {}\n- Goal drift: {}\n\nRuntime:\n- Provider: {}\n- Model: {}\n- Permissions: {}\n- Resource policy: {}\n- Recent commands: {}\n\nContracts:\n- State: {}\n- Plan: {}\n- Stage: {}\n- Retrieval: {}\n- Reflection: {}\n- Acceptance: {}\n- Guided debug: {}\n- Closeout: {}\n- A2A: {}\n\nWorkspace:\n- Project: {}\n- Path: {}\n- {}\n\nNext actions:\n1. /resource     inspect latest resource budget\n2. /goal         inspect or pin the active goal\n3. /goal drift   inspect recent goal drift\n4. /doctor       run environment diagnostics\n5. /permissions  inspect or edit permission rules\n6. Ctrl+P        open command palette",
        app.mode,
        app.is_querying,
        pending,
        app.messages.len(),
        &session[..8.min(session.len())],
        goal_line,
        drift_line,
        app.current_provider_label(),
        app.current_model_label(),
        app.current_permission_label(),
        resource_line,
        recent_commands,
        contract_line,
        plan_line,
        stage_line,
        retrieval_line,
        reflection_line,
        acceptance_line,
        debugging_line,
        closeout_line,
        a2a_line,
        workspace,
        cwd.display(),
        quick_git_line(&cwd)
    )
}

fn latest_resource_policy_label(trace: &crate::engine::trace::TurnTrace) -> Option<String> {
    trace.events.iter().rev().find_map(|event| {
        if let crate::engine::trace::TraceEvent::ResourcePolicySelected {
            latency,
            cost_ceiling_usd,
            reasoning,
            parallelism_limit,
            max_tool_calls,
            ..
        } = event
        {
            Some(format!(
                "{} ${:.2} {} p{} tools{}",
                latency, cost_ceiling_usd, reasoning, parallelism_limit, max_tool_calls
            ))
        } else {
            None
        }
    })
}

fn latest_contract_state_label(trace: &crate::engine::trace::TurnTrace) -> String {
    let mut task = false;
    let mut judgment = false;
    let mut plan = false;
    let mut retrieval = false;
    let mut reflection = false;
    let mut verification = false;
    let mut acceptance = false;
    let mut debugging = false;
    let mut stage = false;
    let mut closeout = false;
    for event in &trace.events {
        match event {
            crate::engine::trace::TraceEvent::TaskContextBuilt { .. } => task = true,
            crate::engine::trace::TraceEvent::WorkflowJudgmentCompleted { .. } => judgment = true,
            crate::engine::trace::TraceEvent::WorkflowPlanProgress { .. } => plan = true,
            crate::engine::trace::TraceEvent::StageValidationCompleted { .. } => stage = true,
            crate::engine::trace::TraceEvent::RetrievalContextBuilt { .. } => retrieval = true,
            crate::engine::trace::TraceEvent::ReflectionPassCompleted { .. } => reflection = true,
            crate::engine::trace::TraceEvent::VerificationCompleted { .. } => verification = true,
            crate::engine::trace::TraceEvent::AcceptanceReviewCompleted { .. } => acceptance = true,
            crate::engine::trace::TraceEvent::GuidedDebuggingCompleted { .. } => debugging = true,
            crate::engine::trace::TraceEvent::FinalCloseoutPrepared { .. } => closeout = true,
            _ => {}
        }
    }
    let mut parts = Vec::new();
    if task {
        parts.push("task");
    }
    if judgment {
        parts.push("judgment");
    }
    if plan {
        parts.push("plan");
    }
    if stage {
        parts.push("stage");
    }
    if retrieval {
        parts.push("retrieval");
    }
    if reflection {
        parts.push("reflection");
    }
    if verification {
        parts.push("verification");
    }
    if acceptance {
        parts.push("acceptance");
    }
    if debugging {
        parts.push("debug");
    }
    if closeout {
        parts.push("closeout");
    }
    if parts.is_empty() {
        "none".to_string()
    } else {
        parts.join(", ")
    }
}

fn latest_retrieval_context_label(trace: &crate::engine::trace::TurnTrace) -> Option<String> {
    trace.events.iter().rev().find_map(|event| {
        if let crate::engine::trace::TraceEvent::RetrievalContextBuilt {
            policy,
            sources,
            items,
            estimated_tokens,
            conflicts,
            ..
        } = event
        {
            Some(format!(
                "{} {} item(s) from {} tokens~{} conflicts={}",
                policy,
                items,
                sources.join("+"),
                estimated_tokens,
                conflicts
            ))
        } else {
            None
        }
    })
}

fn latest_reflection_label(trace: &crate::engine::trace::TurnTrace) -> Option<String> {
    trace.events.iter().rev().find_map(|event| {
        if let crate::engine::trace::TraceEvent::ReflectionPassCompleted {
            status,
            findings,
            unresolved,
            ..
        } = event
        {
            Some(format!(
                "{} findings={} unresolved={}",
                status, findings, unresolved
            ))
        } else {
            None
        }
    })
}

fn latest_stage_validation_label(trace: &crate::engine::trace::TurnTrace) -> Option<String> {
    trace.events.iter().rev().find_map(|event| {
        if let crate::engine::trace::TraceEvent::StageValidationCompleted {
            step,
            status,
            changed_files,
            evidence_items,
        } = event
        {
            Some(format!(
                "{} step={} files={} evidence={}",
                status,
                step.as_deref()
                    .map(|step| compact_inline(step, 60))
                    .unwrap_or_else(|| "none".to_string()),
                changed_files,
                evidence_items
            ))
        } else {
            None
        }
    })
}

fn latest_workflow_plan_label(trace: &crate::engine::trace::TurnTrace) -> Option<String> {
    trace.events.iter().rev().find_map(|event| {
        if let crate::engine::trace::TraceEvent::WorkflowPlanProgress {
            total_steps,
            completed_steps,
            active_step,
            top_priority,
            top_importance_score: _,
            top_weight_share: _,
            weight_source: _,
            reweighted,
        } = event
        {
            let reweighted_suffix = if *reweighted { " reweighted" } else { "" };
            Some(format!(
                "{}/{} {} ({}){}",
                completed_steps,
                total_steps,
                active_step
                    .as_deref()
                    .map(|step| compact_inline(step, 60))
                    .unwrap_or_else(|| "none".to_string()),
                top_priority.as_deref().unwrap_or("none"),
                reweighted_suffix
            ))
        } else {
            None
        }
    })
}

fn latest_closeout_label(trace: &crate::engine::trace::TurnTrace) -> Option<String> {
    trace.events.iter().rev().find_map(|event| {
        if let crate::engine::trace::TraceEvent::FinalCloseoutPrepared {
            status,
            changed_files,
            validation_items,
            acceptance_items,
            residual_risks,
        } = event
        {
            Some(format!(
                "{} files={} validation={} acceptance={} risks={}",
                status, changed_files, validation_items, acceptance_items, residual_risks
            ))
        } else {
            None
        }
    })
}

fn latest_acceptance_label(trace: &crate::engine::trace::TurnTrace) -> Option<String> {
    trace.events.iter().rev().find_map(|event| {
        if let crate::engine::trace::TraceEvent::AcceptanceReviewCompleted {
            accepted,
            confidence,
            criteria,
            unresolved,
            next_action,
        } = event
        {
            Some(format!(
                "{} confidence={} criteria={} unresolved={} next={}",
                if *accepted {
                    "accepted"
                } else {
                    "not accepted"
                },
                confidence,
                criteria,
                unresolved,
                next_action
            ))
        } else {
            None
        }
    })
}

fn latest_guided_debugging_label(trace: &crate::engine::trace::TurnTrace) -> Option<String> {
    trace.events.iter().rev().find_map(|event| {
        if let crate::engine::trace::TraceEvent::GuidedDebuggingCompleted {
            blocker,
            next_action,
            causes,
            evidence_items,
            ask_user,
        } = event
        {
            Some(format!(
                "blocker={} next={} causes={} evidence={} ask_user={}",
                blocker, next_action, causes, evidence_items, ask_user
            ))
        } else {
            None
        }
    })
}

fn latest_a2a_transcript_label() -> String {
    match crate::agent::a2a_transcript::read_recent(1) {
        Ok(records) if !records.is_empty() => {
            let record = records.last().expect("checked non-empty");
            format!(
                "{:?} {} -> {} artifacts={} goal={}",
                record.status,
                record.from,
                record.to.as_deref().unwrap_or("unassigned"),
                record.artifacts,
                compact_inline(&record.goal, 60)
            )
        }
        _ => "none".to_string(),
    }
}

/// /goal - Show or pin the current session goal
pub fn handle_goal(app: &mut TuiApp, args: &str) -> String {
    let trimmed = args.trim();
    if trimmed.starts_with("drift") {
        let limit = trimmed
            .strip_prefix("drift")
            .unwrap_or_default()
            .trim()
            .parse::<usize>()
            .unwrap_or(8)
            .clamp(1, 50);
        return match latest_trace_for_app(app) {
            Some(trace) => format_goal_drift_report(&trace, limit),
            None => "Goal Drift\n- none yet".to_string(),
        };
    }

    let Some(engine) = app.streaming_engine.as_ref() else {
        return "Current Goal\n- unavailable (no engine connected)".to_string();
    };
    let manager = engine.goal_manager();
    if trimmed.is_empty() || trimmed == "status" || trimmed == "show" {
        return manager.format_current();
    }

    if trimmed == "clear" || trimmed == "reset" {
        manager.clear();
        return "Current Goal\n- cleared".to_string();
    }

    if let Some(title) = trimmed.strip_prefix("set ") {
        return manager
            .set_manual(title)
            .map(|goal| format!("Current Goal\n- pinned: {}", goal.compact_status()))
            .unwrap_or_else(|| "Usage: /goal set <text>".to_string());
    }

    "Usage: /goal [set <text>|clear|drift [limit]]".to_string()
}

fn latest_trace_for_app(app: &TuiApp) -> Option<crate::engine::trace::TurnTrace> {
    if let Some(engine) = app.streaming_engine.as_ref() {
        engine
            .trace_store()
            .latest()
            .or_else(|| app.session_manager.latest_trace().ok().flatten())
    } else {
        app.session_manager.latest_trace().ok().flatten()
    }
}

pub(crate) fn goal_drift_count_label(trace: &crate::engine::trace::TurnTrace) -> String {
    let mut medium = 0usize;
    let mut high = 0usize;
    for event in &trace.events {
        if let crate::engine::trace::TraceEvent::GoalDriftDetected { level, .. } = event {
            if level.eq_ignore_ascii_case("high") {
                high += 1;
            } else {
                medium += 1;
            }
        }
    }
    match (high, medium) {
        (0, 0) => "none".to_string(),
        (0, medium) => format!("{} advisory", medium),
        (high, 0) => format!("{} high", high),
        (high, medium) => format!("{} high, {} advisory", high, medium),
    }
}

pub(crate) fn format_goal_drift_report(
    trace: &crate::engine::trace::TurnTrace,
    limit: usize,
) -> String {
    let lines = trace
        .events
        .iter()
        .filter_map(|event| match event {
            crate::engine::trace::TraceEvent::GoalDriftDetected {
                goal_id,
                tool,
                call_id,
                level,
                reason,
                suggested_action,
            } => Some(format!(
                "- {} drift via {} {} goal={} reason={} suggested={}",
                level,
                tool,
                call_id.chars().take(8).collect::<String>(),
                goal_id.chars().take(8).collect::<String>(),
                compact_inline(reason, 120),
                suggested_action.as_deref().unwrap_or("none")
            )),
            _ => None,
        })
        .take(limit)
        .collect::<Vec<_>>();

    if lines.is_empty() {
        format!(
            "Goal Drift\n- none in latest trace {}\n\nUse /trace last for the full turn timeline.",
            trace.trace_id.chars().take(8).collect::<String>()
        )
    } else {
        format!(
            "Goal Drift from trace {} ({})\n{}",
            trace.trace_id.chars().take(8).collect::<String>(),
            goal_drift_count_label(trace),
            lines.join("\n")
        )
    }
}

fn compact_inline(text: &str, max_chars: usize) -> String {
    let mut value = text.replace('\n', " ");
    if value.chars().count() > max_chars {
        value = value.chars().take(max_chars).collect::<String>();
        value.push_str("...");
    }
    value
}

/// /learn - Show recent runtime learning events
pub fn handle_learn(app: &mut TuiApp, args: &str) -> String {
    let mut parts = args.split_whitespace();
    if matches!(parts.next(), Some("show")) {
        let Some(id) = parts.next().and_then(|value| value.parse::<i64>().ok()) else {
            return "Usage: /learn show <id>".to_string();
        };
        return match app.session_manager.learning_event(id) {
            Ok(Some(event)) => format_learning_event_detail(&event),
            Ok(None) => format!("Learning event #{} not found in current session.", id),
            Err(e) => format!("Learning event unavailable: {}", e),
        };
    }

    let limit = args.trim().parse::<i64>().unwrap_or(8).clamp(1, 50);
    let events = match app.session_manager.recent_learning_events(limit) {
        Ok(events) => events,
        Err(e) => return format!("Learning events unavailable: {}", e),
    };
    if events.is_empty() {
        return "Learning Events\n- none yet".to_string();
    }

    let mut lines = vec![format!("Learning Events ({} recent)", events.len())];
    for event in events {
        lines.push(format!(
            "- #{} {} [{}] conf={:.2}: {}",
            event.id, event.kind, event.source, event.confidence, event.summary
        ));
    }
    lines.join("\n")
}

fn format_learning_event_detail(event: &crate::session_store::LearningEventRecord) -> String {
    let pretty_payload =
        serde_json::to_string_pretty(&event.payload).unwrap_or_else(|_| event.payload.to_string());
    format!(
        "Learning Event #{}\nKind: {}\nSource: {}\nConfidence: {:.2}\nCreated: {}\nSummary: {}\n\nPayload:\n{}",
        event.id,
        event.kind,
        event.source,
        event.confidence,
        event.created_at,
        event.summary,
        pretty_payload
    )
}

/// /experience - Inspect typed ExperienceRecord payloads.
pub fn handle_experience(app: &mut TuiApp, args: &str) -> String {
    let mut parts = args.split_whitespace();
    let action = parts.next().unwrap_or("last");
    match action {
        "last" | "" => {
            let events = match app.session_manager.recent_learning_events(30) {
                Ok(events) => events,
                Err(e) => return format!("Experience ledger unavailable: {}", e),
            };
            match events
                .iter()
                .find(|event| event.payload.get("experience").is_some())
            {
                Some(event) => format_experience_event(event),
                None => "Experience Ledger\n- no structured experience records yet".to_string(),
            }
        }
        "list" => {
            let limit = parts
                .next()
                .and_then(|value| value.parse::<i64>().ok())
                .unwrap_or(10)
                .clamp(1, 50);
            let events = match app.session_manager.recent_learning_events(limit * 3) {
                Ok(events) => events,
                Err(e) => return format!("Experience ledger unavailable: {}", e),
            };
            let lines = events
                .iter()
                .filter(|event| event.payload.get("experience").is_some())
                .take(limit as usize)
                .map(|event| {
                    let experience = &event.payload["experience"];
                    format!(
                        "- #{} {} workflow={} outcome={} tools={}",
                        event.id,
                        event.kind,
                        experience["workflow"].as_str().unwrap_or("unknown"),
                        experience["final_outcome"].as_str().unwrap_or("unknown"),
                        experience["cost"]["tool_calls"].as_u64().unwrap_or(0)
                    )
                })
                .collect::<Vec<_>>();
            if lines.is_empty() {
                "Experience Ledger\n- no structured experience records yet".to_string()
            } else {
                format!("Experience Ledger\n{}", lines.join("\n"))
            }
        }
        "show" => {
            let Some(id) = parts.next().and_then(|value| value.parse::<i64>().ok()) else {
                return "Usage: /experience show <id>".to_string();
            };
            match app.session_manager.learning_event(id) {
                Ok(Some(event)) if event.payload.get("experience").is_some() => {
                    format_experience_event(&event)
                }
                Ok(Some(_)) => format!(
                    "Learning event #{} has no structured experience payload.",
                    id
                ),
                Ok(None) => format!("Experience event #{} not found in current session.", id),
                Err(e) => format!("Experience event unavailable: {}", e),
            }
        }
        _ => "Usage: /experience [last|list [limit]|show <id>]".to_string(),
    }
}

fn format_experience_event(event: &crate::session_store::LearningEventRecord) -> String {
    let experience = &event.payload["experience"];
    let pretty =
        serde_json::to_string_pretty(experience).unwrap_or_else(|_| experience.to_string());
    format!(
        "Experience #{}\nKind: {}\nSource: {}\nCreated: {}\nWorkflow: {}\nOutcome: {}\nTool calls: {}\n\n{}",
        event.id,
        event.kind,
        event.source,
        event.created_at,
        experience["workflow"].as_str().unwrap_or("unknown"),
        experience["final_outcome"].as_str().unwrap_or("unknown"),
        experience["cost"]["tool_calls"].as_u64().unwrap_or(0),
        pretty
    )
}

/// /improvements - Controlled self-evolution proposals
pub fn handle_improvements(app: &mut TuiApp, args: &str) -> String {
    use crate::engine::improvement::{ImprovementStore, ProposalStatus};

    let mut parts = args.split_whitespace();
    let action = parts.next().unwrap_or("list");
    let store = ImprovementStore::default();

    match action {
        "scan" | "propose" => {
            let limit = parts
                .next()
                .and_then(|value| value.parse::<i64>().ok())
                .unwrap_or(50)
                .clamp(5, 200);
            let events = match app.session_manager.recent_learning_events(limit) {
                Ok(events) => events,
                Err(e) => return format!("Improvement scan failed: {}", e),
            };
            match store.propose_from_learning_events(&events) {
                Ok(proposals) if proposals.is_empty() => {
                    "Improvement scan complete: no new proposals.".to_string()
                }
                Ok(proposals) => {
                    let mut lines = vec![format!(
                        "Improvement scan complete: {} new proposal(s)",
                        proposals.len()
                    )];
                    for proposal in proposals {
                        lines.push(format_improvement_line(&proposal));
                    }
                    lines.join("\n")
                }
                Err(e) => format!("Improvement scan failed: {}", e),
            }
        }
        "list" | "" => {
            let proposals = store.list();
            if proposals.is_empty() {
                "Improvements\n- none yet\n\nRun /improvements scan to generate proposals from recent learning events.".to_string()
            } else {
                let mut lines = vec![format!("Improvements ({} total)", proposals.len())];
                for proposal in proposals.iter().take(20) {
                    lines.push(format_improvement_line(proposal));
                }
                lines.join("\n")
            }
        }
        "show" => {
            let Some(id) = parts.next() else {
                return "Usage: /improvements show <id>".to_string();
            };
            match store.get(id) {
                Some(proposal) => format_improvement_detail(&proposal),
                None => format!("No improvement proposal matching '{}'.", id),
            }
        }
        "accept" | "reject" | "apply" => {
            let Some(id) = parts.next() else {
                return format!("Usage: /improvements {} <id>", action);
            };
            let desired = match action {
                "accept" => ProposalStatus::Accepted,
                "reject" => ProposalStatus::Rejected,
                "apply" => ProposalStatus::Applied,
                _ => unreachable!(),
            };
            let Some(current) = store.get(id) else {
                return format!("No improvement proposal matching '{}'.", id);
            };
            if desired == ProposalStatus::Applied && current.status != ProposalStatus::Accepted {
                return format!(
                    "Proposal {} is {:?}. Accept it before applying. High-risk and behavior-changing proposals require explicit approval.",
                    current.id, current.status
                );
            }
            if desired == ProposalStatus::Applied {
                let gate = improvement_evolution_gate(&current);
                if matches!(
                    gate.action,
                    crate::engine::evolution_controller::EvolutionAction::Reject
                        | crate::engine::evolution_controller::EvolutionAction::Monitor
                ) {
                    return format!(
                        "Proposal {} was not applied by evolution gate.\n{}",
                        current.id,
                        format_evolution_gate(&gate)
                    );
                }
            }
            match store.update_status(id, desired) {
                Ok(Some(updated)) => {
                    persist_improvement_learning_event(app, &updated, action);
                    format!(
                        "Updated proposal {}\n{}",
                        updated.id,
                        format_improvement_line(&updated)
                    )
                }
                Ok(None) => format!("No improvement proposal matching '{}'.", id),
                Err(e) => format!("Failed to update proposal: {}", e),
            }
        }
        _ => {
            "Usage: /improvements [list|scan [limit]|show <id>|accept <id>|reject <id>|apply <id>]"
                .to_string()
        }
    }
}

fn format_improvement_line(proposal: &crate::engine::improvement::ImprovementProposal) -> String {
    format!(
        "- {} [{:?}/{:?}/{:?}] events={}: {}",
        proposal.id,
        proposal.status,
        proposal.target,
        proposal.risk,
        proposal.trigger_event_ids.len(),
        proposal.proposed_change
    )
}

fn format_improvement_detail(proposal: &crate::engine::improvement::ImprovementProposal) -> String {
    format!(
        "Improvement Proposal {}\n\nStatus: {:?}\nTarget: {:?}\nRisk: {:?}\nEvents: {:?}\n\nProposed change:\n{}\n\nExpected benefit:\n{}\n\nValidation plan:\n{}\n\nEvidence:\n{}",
        proposal.id,
        proposal.status,
        proposal.target,
        proposal.risk,
        proposal.trigger_event_ids,
        proposal.proposed_change,
        proposal.expected_benefit,
        proposal
            .validation
            .iter()
            .map(|item| format!("- {}", item))
            .collect::<Vec<_>>()
            .join("\n"),
        proposal
            .evidence
            .iter()
            .map(|item| format!("- {}", item))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

fn persist_improvement_learning_event(
    app: &mut TuiApp,
    proposal: &crate::engine::improvement::ImprovementProposal,
    action: &str,
) {
    let mut payload = serde_json::to_value(proposal).unwrap_or_else(|_| serde_json::json!({}));
    if action == "apply" {
        payload["evolution_gate"] =
            serde_json::to_value(improvement_evolution_gate(proposal)).unwrap_or_default();
    }
    let _ = app.session_manager.add_learning_event(
        "improvement_proposal",
        "improvements",
        &format!("Improvement proposal {} {}", proposal.id, action),
        0.9,
        &payload,
    );
}

/// /skill-proposals - Review generated skill candidates before activation
pub fn handle_skill_proposals(app: &mut TuiApp, args: &str) -> String {
    use crate::engine::skill_evolution::{
        evaluate_skill_proposal, write_active_skill, SkillProposalStatus, SkillProposalStore,
    };

    let mut parts = args.split_whitespace();
    let action = parts.next().unwrap_or("list");
    let store = SkillProposalStore::default();

    match action {
        "scan" | "propose" => {
            let limit = parts
                .next()
                .and_then(|value| value.parse::<i64>().ok())
                .unwrap_or(80)
                .clamp(5, 300);
            let events = match app.session_manager.recent_learning_events(limit) {
                Ok(events) => events,
                Err(e) => return format!("Skill proposal scan failed: {}", e),
            };
            match store.propose_from_learning_events(&events) {
                Ok(proposals) if proposals.is_empty() => {
                    "Skill proposal scan complete: no repeated successful procedures found."
                        .to_string()
                }
                Ok(proposals) => {
                    let mut lines = vec![format!(
                        "Skill proposal scan complete: {} new candidate(s)",
                        proposals.len()
                    )];
                    for proposal in proposals {
                        lines.push(format_skill_proposal_line(&proposal));
                    }
                    lines.join("\n")
                }
                Err(e) => format!("Skill proposal scan failed: {}", e),
            }
        }
        "list" | "" => {
            let proposals = store.list();
            if proposals.is_empty() {
                "Skill Proposals\n- none yet\n\nRun /skill-proposals scan to generate candidates from repeated successful workflows.".to_string()
            } else {
                let mut lines = vec![format!("Skill Proposals ({} total)", proposals.len())];
                for proposal in proposals.iter().take(20) {
                    lines.push(format_skill_proposal_line(proposal));
                }
                lines.join("\n")
            }
        }
        "show" => {
            let Some(id) = parts.next() else {
                return "Usage: /skill-proposals show <id|name>".to_string();
            };
            match store.get(id) {
                Some(proposal) => format_skill_proposal_detail(&proposal),
                None => format!("No skill proposal matching '{}'.", id),
            }
        }
        "eval" => {
            let Some(id) = parts.next() else {
                return "Usage: /skill-proposals eval <id|name>".to_string();
            };
            match store.get(id) {
                Some(proposal) => format_skill_eval(&evaluate_skill_proposal(&proposal)),
                None => format!("No skill proposal matching '{}'.", id),
            }
        }
        "fitness" | "stats" => {
            let Some(name) = parts.next() else {
                return "Usage: /skill-proposals fitness <skill-name>".to_string();
            };
            match store.fitness_snapshot(name) {
                Some(snapshot) => format_skill_fitness(&snapshot),
                None => format!("No skill usage events found for '{}'.", name),
            }
        }
        "gate" => {
            let Some(name) = parts.next() else {
                return "Usage: /skill-proposals gate <skill-name> [old-fitness]".to_string();
            };
            let old_fitness = parts
                .next()
                .and_then(|value| value.parse::<f32>().ok())
                .unwrap_or(0.0)
                .clamp(0.0, 1.0);
            match store.fitness_snapshot(name) {
                Some(snapshot) => {
                    let gate = crate::engine::skill_evolution::compare_skill_versions_for_promotion(
                        old_fitness,
                        &snapshot,
                        0.0,
                        0.0,
                    );
                    format_skill_promotion_gate(&gate)
                }
                None => format!("No skill usage events found for '{}'.", name),
            }
        }
        "versions" => {
            let Some(name) = parts.next() else {
                return "Usage: /skill-proposals versions <skill-name>".to_string();
            };
            let records = store.version_records(name);
            if records.is_empty() {
                format!("No applied versions recorded for '{}'.", name)
            } else {
                let mut lines = vec![format!("Skill Versions /{}", name)];
                for record in records.iter().rev().take(10) {
                    lines.push(format!(
                        "- {} path={} rollback_to={} evalsets={}",
                        record.version,
                        record.applied_path,
                        record.rollback_to.as_deref().unwrap_or("none"),
                        if record.evalset_bindings.is_empty() {
                            "none".to_string()
                        } else {
                            record.evalset_bindings.join(",")
                        }
                    ));
                }
                lines.join("\n")
            }
        }
        "rollback-list" | "disabled" => {
            let filter = parts.next();
            let backups = disabled_skill_backups(&user_skill_root(), filter);
            if backups.is_empty() {
                match filter {
                    Some(name) => format!("No disabled rollback backups found for /{}.", name),
                    None => "No disabled rollback backups found.".to_string(),
                }
            } else {
                let mut lines = vec![format!("Disabled Skill Backups ({} total)", backups.len())];
                for backup in backups.iter().take(20) {
                    lines.push(format!(
                        "- /{} backup={} path={}",
                        backup.skill_name,
                        backup.backup_name,
                        backup.path.display()
                    ));
                }
                lines.push(
                    "Restore with: /skill-proposals restore <skill-name> [backup-name] --yes"
                        .to_string(),
                );
                lines.join("\n")
            }
        }
        "restore" => {
            let Some(name) = parts.next() else {
                return "Usage: /skill-proposals restore <skill-name> [backup-name] --yes"
                    .to_string();
            };
            if !is_safe_skill_dir_name(name) {
                return "Invalid skill name. Use only the skill directory name, not a path."
                    .to_string();
            }
            let mut backup_name: Option<&str> = None;
            let mut confirmed = false;
            for part in parts {
                if part == "--yes" {
                    confirmed = true;
                } else {
                    backup_name = Some(part);
                }
            }
            if !confirmed {
                return format!(
                    "Restore reactivates a disabled /{} skill backup.\nUsage: /skill-proposals restore {} [backup-name] --yes",
                    name, name
                );
            }
            if let Some(backup_name) = backup_name {
                if !is_safe_skill_dir_name(backup_name) {
                    return "Invalid backup name. Use the basename shown by /skill-proposals rollback-list."
                        .to_string();
                }
            }
            let root = user_skill_root();
            let active_dir = root.join(name);
            if active_dir.exists() {
                return format!(
                    "Refusing restore: active skill directory already exists: {}",
                    active_dir.display()
                );
            }
            let Some(backup) = resolve_disabled_skill_backup(&root, name, backup_name) else {
                return format!(
                    "No disabled backup found for /{}.\nUse /skill-proposals rollback-list {} to inspect backups.",
                    name, name
                );
            };
            if !backup.path.starts_with(&root) || !active_dir.starts_with(&root) {
                return "Refusing restore outside user skill root.".to_string();
            }
            match std::fs::rename(&backup.path, &active_dir) {
                Ok(()) => {
                    let loaded = app.skill_runtime.reload();
                    let payload = serde_json::json!({
                        "skill_name": name,
                        "backup_name": backup.backup_name,
                        "restored_path": active_dir,
                        "source_path": backup.path,
                    });
                    let _ = app.session_manager.add_learning_event(
                        "skill_rollback_restore",
                        "skill_evolution",
                        &format!("Restored disabled skill /{}", name),
                        0.9,
                        &payload,
                    );
                    format!(
                        "Restored /{}\n- from: {}\n- active: {}\n- reloaded skills: {}",
                        name,
                        backup.backup_name,
                        active_dir.display(),
                        loaded
                    )
                }
                Err(e) => format!("Failed to restore /{}: {}", name, e),
            }
        }
        "rollback" => {
            let Some(name) = parts.next() else {
                return "Usage: /skill-proposals rollback <skill-name> --yes".to_string();
            };
            if !is_safe_skill_dir_name(name) {
                return "Invalid skill name. Use only the skill directory name, not a path."
                    .to_string();
            }
            let confirmed = parts.any(|part| part == "--yes");
            if !confirmed {
                return format!(
                    "Rollback disables the active /{} skill by moving its directory aside.\nUsage: /skill-proposals rollback {} --yes",
                    name, name
                );
            }
            let records = store.version_records(name);
            let Some(latest) = records.last() else {
                return format!("No applied versions recorded for '{}'.", name);
            };
            let root = user_skill_root();
            let skill_dir = root.join(name);
            if !skill_dir.exists() {
                return format!("Active skill directory does not exist: {}", skill_dir.display());
            }
            if !skill_dir.starts_with(&root) {
                return format!("Refusing rollback outside user skill root: {}", skill_dir.display());
            }
            let disabled_dir = root.join(format!(
                "{}.disabled-{}",
                name,
                chrono::Utc::now().format("%Y%m%d%H%M%S")
            ));
            match std::fs::rename(&skill_dir, &disabled_dir) {
                Ok(()) => {
                    let _ = store.update_status(&latest.proposal_id, SkillProposalStatus::Accepted);
                    let loaded = app.skill_runtime.reload();
                    let payload = serde_json::json!({
                        "skill_name": name,
                        "disabled_path": disabled_dir,
                        "previous_path": skill_dir,
                        "version": latest.version,
                        "proposal_id": latest.proposal_id,
                    });
                    let _ = app.session_manager.add_learning_event(
                        "skill_rollback",
                        "skill_evolution",
                        &format!("Rolled back active skill /{}", name),
                        0.9,
                        &payload,
                    );
                    format!(
                        "Rolled back /{}\n- moved: {}\n- disabled: {}\n- proposal returned to Accepted\n- reloaded skills: {}",
                        name,
                        skill_dir.display(),
                        disabled_dir.display(),
                        loaded
                    )
                }
                Err(e) => format!("Failed to rollback /{}: {}", name, e),
            }
        }
        "bind-eval" => {
            let Some(id) = parts.next() else {
                return "Usage: /skill-proposals bind-eval <id|name> <evalset-name>".to_string();
            };
            let Some(evalset) = parts.next() else {
                return "Usage: /skill-proposals bind-eval <id|name> <evalset-name>".to_string();
            };
            match store.bind_evalset(id, evalset) {
                Ok(Some(updated)) => format!(
                    "Bound evalset '{}' to skill proposal {}\n{}",
                    evalset,
                    updated.id,
                    format_skill_proposal_line(&updated)
                ),
                Ok(None) => format!("No skill proposal matching '{}'.", id),
                Err(e) => format!("Failed to bind evalset: {}", e),
            }
        }
        "record" => {
            let Some(name) = parts.next() else {
                return "Usage: /skill-proposals record <skill-name> <success|fail> [version]"
                    .to_string();
            };
            let Some(outcome) = parts.next() else {
                return "Usage: /skill-proposals record <skill-name> <success|fail> [version]"
                    .to_string();
            };
            let success = match outcome {
                "success" | "pass" | "passed" => true,
                "fail" | "failed" | "failure" => false,
                _ => return "Outcome must be success or fail.".to_string(),
            };
            let version = parts.next().unwrap_or("manual");
            let event = crate::engine::skill_evolution::SkillUsageEvent {
                skill_name: name.to_string(),
                skill_version: version.to_string(),
                provisional: false,
                success,
                acceptance_passed: Some(success),
                tests_passed: None,
                user_satisfaction: if success { Some(0.80) } else { Some(0.25) },
                duration_ms: None,
                tool_calls: 0,
                risk_penalty: if success { 0.05 } else { 0.25 },
                created_at: chrono::Utc::now().to_rfc3339(),
            };
            match store.record_usage(&event) {
                Ok(()) => {
                    let _ = app.session_manager.add_learning_event(
                        "skill_usage",
                        "skill_proposals",
                        &format!("Skill /{} outcome recorded: {}", name, outcome),
                        0.85,
                        &serde_json::to_value(&event).unwrap_or_else(|_| serde_json::json!({})),
                    );
                    match store.fitness_snapshot(name) {
                        Some(snapshot) => format!(
                            "Recorded skill usage for /{}\n{}",
                            name,
                            format_skill_fitness(&snapshot)
                        ),
                        None => format!("Recorded skill usage for /{}.", name),
                    }
                }
                Err(e) => format!("Failed to record skill usage: {}", e),
            }
        }
        "accept" | "reject" => {
            let Some(id) = parts.next() else {
                return format!("Usage: /skill-proposals {} <id|name>", action);
            };
            let desired = if action == "accept" {
                SkillProposalStatus::Accepted
            } else {
                SkillProposalStatus::Rejected
            };
            match store.update_status(id, desired) {
                Ok(Some(updated)) => {
                    persist_skill_proposal_learning_event(app, &updated, action, None);
                    format!(
                        "Updated skill proposal {}\n{}",
                        updated.id,
                        format_skill_proposal_line(&updated)
                    )
                }
                Ok(None) => format!("No skill proposal matching '{}'.", id),
                Err(e) => format!("Failed to update skill proposal: {}", e),
            }
        }
        "apply" => {
            let Some(id) = parts.next() else {
                return "Usage: /skill-proposals apply <id|name>".to_string();
            };
            let Some(current) = store.get(id) else {
                return format!("No skill proposal matching '{}'.", id);
            };
            if current.status != SkillProposalStatus::Accepted {
                return format!(
                    "Skill proposal {} is {:?}. Accept it before applying; generated skills are not activated automatically.",
                    current.id, current.status
                );
            }
            let eval = evaluate_skill_proposal(&current);
            if !eval.passed {
                return format!(
                    "Skill proposal {} failed eval and was not applied.\n{}",
                    current.id,
                    format_skill_eval(&eval)
                );
            }
            if let Some(report) = run_bound_skill_evalsets(&current) {
                if !report.ok {
                    return format!(
                        "Skill proposal {} failed bound evalsets and was not applied.\n{}",
                        current.id, report.summary
                    );
                }
            }
            let gate = skill_evolution_gate(&current);
            if matches!(
                gate.action,
                crate::engine::evolution_controller::EvolutionAction::Reject
                    | crate::engine::evolution_controller::EvolutionAction::Monitor
            ) {
                return format!(
                    "Skill proposal {} was not applied by evolution gate.\n{}",
                    current.id,
                    format_evolution_gate(&gate)
                );
            }
            let root = user_skill_root();
            match write_active_skill(&current, &root) {
                Ok(path) => match store.record_applied_version(id, &path) {
                    Ok(Some((updated, _version))) => {
                        let loaded = app.skill_runtime.reload();
                        persist_skill_proposal_learning_event(
                            app,
                            &updated,
                            "apply",
                            Some(path.display().to_string()),
                        );
                        format!(
                            "Applied skill proposal {}\n- wrote: {}\n- trust: {:?}\n- reloaded skills: {}\n\nInvoke with /{} <task>",
                            updated.id,
                            path.display(),
                            updated.trust,
                            loaded,
                            updated.name
                        )
                    }
                    Ok(None) => format!(
                        "Skill file written, but version record update failed for '{}'.",
                        id
                    ),
                    Err(e) => format!("Skill file written, but status update failed: {}", e),
                },
                Err(e) => format!("Failed to apply skill proposal: {}", e),
            }
        }
        _ => "Usage: /skill-proposals [list|scan [limit]|show <id>|eval <id>|fitness <name>|gate <name>|versions <name>|rollback-list [name]|rollback <name> --yes|restore <name> [backup] --yes|bind-eval <id> <evalset>|record <name> <success|fail>|accept <id>|reject <id>|apply <id>]".to_string(),
    }
}

fn user_skill_root() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".priority-agent")
        .join("skills")
}

#[derive(Debug, Clone)]
struct DisabledSkillBackup {
    skill_name: String,
    backup_name: String,
    path: std::path::PathBuf,
}

fn is_safe_skill_dir_name(name: &str) -> bool {
    !name.is_empty()
        && !name.contains('/')
        && !name.contains('\\')
        && name != "."
        && name != ".."
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
}

fn disabled_skill_backups(
    root: &std::path::Path,
    filter: Option<&str>,
) -> Vec<DisabledSkillBackup> {
    let mut backups = Vec::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return backups;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(backup_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some((skill_name, _suffix)) = backup_name.split_once(".disabled-") else {
            continue;
        };
        if filter.is_some_and(|needle| needle != skill_name) {
            continue;
        }
        backups.push(DisabledSkillBackup {
            skill_name: skill_name.to_string(),
            backup_name: backup_name.to_string(),
            path,
        });
    }
    backups.sort_by(|a, b| b.backup_name.cmp(&a.backup_name));
    backups
}

fn resolve_disabled_skill_backup(
    root: &std::path::Path,
    skill_name: &str,
    backup_name: Option<&str>,
) -> Option<DisabledSkillBackup> {
    let backups = disabled_skill_backups(root, Some(skill_name));
    match backup_name {
        Some(name) => backups
            .into_iter()
            .find(|backup| backup.backup_name == name),
        None => backups.into_iter().next(),
    }
}

fn format_skill_proposal_line(proposal: &crate::engine::skill_evolution::SkillProposal) -> String {
    format!(
        "- {} /{} v{} [{:?}/{:?}] score={:.2} events={} evalsets={}: {}",
        proposal.id,
        proposal.name,
        proposal.skill_version(),
        proposal.status,
        proposal.trust,
        proposal.creation_score,
        proposal.trigger_event_ids.len(),
        if proposal.evalset_bindings.is_empty() {
            "none".to_string()
        } else {
            proposal.evalset_bindings.join(",")
        },
        proposal.procedure
    )
}

fn format_skill_proposal_detail(
    proposal: &crate::engine::skill_evolution::SkillProposal,
) -> String {
    format!(
        "Skill Proposal {}\n\nName: /{}\nVersion: {}\nStatus: {:?}\nTrust: {:?}\nScope: {}\nCreation score: {:.2}\nEvidence count: {}\nScope confidence: {:.2}\nEvalSets: {}\nRollback to: {}\nApplied path: {}\nEvents: {:?}\n\nProcedure:\n{}\n\nTriggers:\n{}\n\nWorkflow:\n{}\n\nValidation:\n{}\n\nTools:\n{}\n\nEvidence:\n{}",
        proposal.id,
        proposal.name,
        proposal.skill_version(),
        proposal.status,
        proposal.trust,
        proposal.scope,
        proposal.creation_score,
        proposal.evidence_count,
        proposal.scope_confidence,
        if proposal.evalset_bindings.is_empty() {
            "none".to_string()
        } else {
            proposal.evalset_bindings.join(", ")
        },
        proposal.rollback_to.as_deref().unwrap_or("none"),
        proposal.applied_path.as_deref().unwrap_or("none"),
        proposal.trigger_event_ids,
        proposal.procedure,
        proposal
            .trigger_conditions
            .iter()
            .map(|item| format!("- {}", item))
            .collect::<Vec<_>>()
            .join("\n"),
        proposal
            .workflow_steps
            .iter()
            .enumerate()
            .map(|(idx, item)| format!("{}. {}", idx + 1, item))
            .collect::<Vec<_>>()
            .join("\n"),
        proposal
            .validation
            .iter()
            .map(|item| format!("- {}", item))
            .collect::<Vec<_>>()
            .join("\n"),
        proposal.allowed_tools.join(", "),
        proposal
            .evidence
            .iter()
            .map(|item| format!("- {}", item))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

struct BoundSkillEvalReport {
    ok: bool,
    summary: String,
}

fn run_bound_skill_evalsets(
    proposal: &crate::engine::skill_evolution::SkillProposal,
) -> Option<BoundSkillEvalReport> {
    if proposal.evalset_bindings.is_empty() {
        return None;
    }
    let eval_dir = std::env::current_dir().ok()?.join("evalsets");
    let mut summaries = Vec::new();
    let mut ok = true;
    for binding in &proposal.evalset_bindings {
        match crate::engine::evalset::run_evalsets_from_dir(&eval_dir, Some(binding)) {
            Ok(reports) if reports.is_empty() => {
                ok = false;
                summaries.push(format!("- {}: no matching evalset", binding));
            }
            Ok(reports) => {
                let binding_ok = reports.iter().all(|report| report.ok());
                ok &= binding_ok;
                summaries.push(crate::engine::evalset::format_reports(&reports));
            }
            Err(e) => {
                ok = false;
                summaries.push(format!("- {}: {}", binding, e));
            }
        }
    }
    Some(BoundSkillEvalReport {
        ok,
        summary: summaries.join("\n\n"),
    })
}

fn format_skill_eval(eval: &crate::engine::skill_evolution::SkillEvalResult) -> String {
    let mut lines = vec![format!(
        "Skill Eval {}\nResult: {}",
        eval.proposal_id,
        if eval.passed { "pass" } else { "fail" }
    )];
    for check in &eval.quality.checks {
        lines.push(format!(
            "- {} {}: {}",
            if check.passed { "ok" } else { "fail" },
            check.name,
            check.detail
        ));
    }
    for note in &eval.notes {
        lines.push(format!("- note: {}", note));
    }
    lines.join("\n")
}

fn format_skill_fitness(snapshot: &crate::engine::skill_evolution::SkillFitnessSnapshot) -> String {
    format!(
        "Skill Fitness /{}\nVersion: {}\nEvents: {}\nFitness: {:.2}\n\nFactors:\n- task_success: {:.2}\n- acceptance_pass_rate: {:.2}\n- test_pass_rate: {:.2}\n- user_satisfaction: {:.2}\n- reuse_rate: {:.2}\n- time_saved: {:.2}\n- tool_efficiency: {:.2}\n- failure_rate: {:.2}\n- cost: {:.2}\n- risk_penalty: {:.2}",
        snapshot.skill_name,
        snapshot.skill_version,
        snapshot.events,
        snapshot.fitness,
        snapshot.stats.task_success,
        snapshot.stats.acceptance_pass_rate,
        snapshot.stats.test_pass_rate,
        snapshot.stats.user_satisfaction,
        snapshot.stats.reuse_rate,
        snapshot.stats.time_saved,
        snapshot.stats.tool_efficiency,
        snapshot.stats.failure_rate,
        snapshot.stats.cost,
        snapshot.stats.risk_penalty
    )
}

fn format_skill_promotion_gate(
    gate: &crate::engine::skill_evolution::SkillPromotionGate,
) -> String {
    let mut lines = vec![format!(
        "Skill Promotion Gate\nResult: {}\nOld fitness: {:.2}\nNew fitness: {:.2}\nDelta: {:.2}\nEval count: {}\nRegression rate: {:.2}\nRisk penalty: {:.2}\nSemantic drift: {:.2}",
        if gate.passed { "pass" } else { "blocked" },
        gate.old_fitness,
        gate.new_fitness,
        gate.delta,
        gate.eval_count,
        gate.regression_rate,
        gate.risk_penalty,
        gate.semantic_drift
    )];
    if !gate.reasons.is_empty() {
        lines.push("Reasons:".to_string());
        for reason in &gate.reasons {
            lines.push(format!("- {}", reason));
        }
    }
    lines.join("\n")
}

fn persist_skill_proposal_learning_event(
    app: &mut TuiApp,
    proposal: &crate::engine::skill_evolution::SkillProposal,
    action: &str,
    applied_path: Option<String>,
) {
    let mut payload = serde_json::to_value(proposal).unwrap_or_else(|_| serde_json::json!({}));
    if let Some(path) = applied_path {
        payload["applied_path"] = serde_json::json!(path);
    }
    if action == "apply" {
        payload["evolution_gate"] =
            serde_json::to_value(skill_evolution_gate(proposal)).unwrap_or_default();
    }
    let _ = app.session_manager.add_learning_event(
        "skill_proposal",
        "skill_evolution",
        &format!("Skill proposal {} {}", proposal.id, action),
        0.9,
        &payload,
    );
}

fn improvement_evolution_gate(
    proposal: &crate::engine::improvement::ImprovementProposal,
) -> crate::engine::evolution_controller::EvolutionGateDecision {
    use crate::engine::evolution_controller::{
        EvolutionController, EvolutionTarget, EvolutionTriggerFactors,
    };
    let risk = risk_value(proposal.risk);
    let target = match proposal.target {
        crate::engine::improvement::ImprovementTarget::Memory => EvolutionTarget::Memory,
        crate::engine::improvement::ImprovementTarget::Skill => EvolutionTarget::Skill,
        crate::engine::improvement::ImprovementTarget::Prompt => EvolutionTarget::PromptSection,
        crate::engine::improvement::ImprovementTarget::Routing => EvolutionTarget::WorkflowPolicy,
        crate::engine::improvement::ImprovementTarget::ToolGuidance => {
            EvolutionTarget::ToolDescription
        }
    };
    EvolutionController::new().gate(
        target,
        EvolutionTriggerFactors {
            repeated_failure: (proposal.trigger_event_ids.len() as f32 / 4.0).clamp(0.0, 1.0),
            reuse_frequency: 0.55,
            user_correction_frequency: if proposal
                .evidence
                .iter()
                .any(|item| item.to_lowercase().contains("correction"))
            {
                0.80
            } else {
                0.35
            },
            task_impact: if proposal.target == crate::engine::improvement::ImprovementTarget::Memory
            {
                0.55
            } else {
                0.75
            },
            optimization_potential: 0.70,
            evolution_cost: if matches!(
                proposal.target,
                crate::engine::improvement::ImprovementTarget::Prompt
                    | crate::engine::improvement::ImprovementTarget::Routing
            ) {
                0.65
            } else {
                0.35
            },
            risk,
        },
        0,
    )
}

fn skill_evolution_gate(
    proposal: &crate::engine::skill_evolution::SkillProposal,
) -> crate::engine::evolution_controller::EvolutionGateDecision {
    use crate::engine::evolution_controller::{
        EvolutionController, EvolutionTarget, EvolutionTriggerFactors,
    };
    EvolutionController::new().gate(
        EvolutionTarget::Skill,
        EvolutionTriggerFactors {
            repeated_failure: 0.0,
            reuse_frequency: (proposal.evidence_count as f32 / 6.0).clamp(0.0, 1.0),
            user_correction_frequency: proposal.creation_factors.user_correction_value,
            task_impact: proposal.creation_factors.future_utility,
            optimization_potential: proposal.creation_score,
            evolution_cost: proposal.creation_factors.over_specificity.max(0.20),
            risk: 1.0 - proposal.scope_confidence,
        },
        0,
    )
}

fn risk_value(risk: crate::engine::intent_router::RiskLevel) -> f32 {
    match risk {
        crate::engine::intent_router::RiskLevel::Low => 0.20,
        crate::engine::intent_router::RiskLevel::Medium => 0.50,
        crate::engine::intent_router::RiskLevel::High => 0.85,
    }
}

fn format_evolution_gate(
    gate: &crate::engine::evolution_controller::EvolutionGateDecision,
) -> String {
    let mut lines = vec![format!(
        "Evolution gate: {:?} target={:?} score={:.2} auto_apply={}",
        gate.action, gate.target, gate.score, gate.auto_apply_allowed
    )];
    for reason in &gate.reasons {
        lines.push(format!("- {}", reason));
    }
    lines.join("\n")
}

/// /recover - Show recent recovery plans
pub fn handle_recover(app: &mut TuiApp, args: &str) -> String {
    let limit = args.trim().parse::<usize>().unwrap_or(8).clamp(1, 50);
    let trace = if let Some(engine) = app.streaming_engine.as_ref() {
        engine
            .trace_store()
            .latest()
            .or_else(|| app.session_manager.latest_trace().ok().flatten())
    } else {
        app.session_manager.latest_trace().ok().flatten()
    };

    let Some(trace) = trace else {
        return "Recovery Plans\n- none yet".to_string();
    };

    let plans = trace
        .events
        .iter()
        .filter_map(|event| match event {
            crate::engine::trace::TraceEvent::RecoveryPlan {
                plan_id,
                source,
                category,
                action,
                retryable,
                safe_retry,
                suggested_command,
                status,
            } => Some(format!(
                "- {} [{}:{}] status={} retryable={} safe_retry={} suggested={} action={}",
                &plan_id[..8.min(plan_id.len())],
                source,
                category,
                status,
                retryable,
                safe_retry,
                suggested_command.as_deref().unwrap_or("none"),
                action
            )),
            _ => None,
        })
        .take(limit)
        .collect::<Vec<_>>();

    if plans.is_empty() {
        format!(
            "Recovery Plans\n- none in latest trace {}\n\nUse /trace last for the full turn timeline.",
            &trace.trace_id[..8.min(trace.trace_id.len())]
        )
    } else {
        format!(
            "Recovery Plans from trace {}\n{}",
            &trace.trace_id[..8.min(trace.trace_id.len())],
            plans.join("\n")
        )
    }
}

fn quick_git_line(cwd: &std::path::Path) -> String {
    let branch = std::process::Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(cwd)
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .filter(|branch| !branch.is_empty());

    let changes = std::process::Command::new("git")
        .args(["status", "--short"])
        .current_dir(cwd)
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| {
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .filter(|line| !line.trim().is_empty())
                .count()
        });

    match (branch, changes) {
        (Some(branch), Some(0)) => format!("Git: {} clean", branch),
        (Some(branch), Some(count)) => format!("Git: {} with {} changed files", branch, count),
        (Some(branch), None) => format!("Git: {}", branch),
        _ => "Git: not a repository".to_string(),
    }
}

/// /feedback - Send feedback
pub fn handle_feedback(app: &mut TuiApp, args: &str) -> String {
    let message = args.trim();
    if message.is_empty() {
        return "Usage: /feedback <message>".to_string();
    }
    let session_id = app
        .session_manager
        .current_session_id()
        .unwrap_or("none")
        .to_string();
    match append_feedback(&session_id, message) {
        Ok(path) => format!("Feedback recorded to {}.", path.display()),
        Err(e) => format!("Failed to record feedback: {}", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_skill_dir_name_rejects_paths() {
        assert!(is_safe_skill_dir_name("rust-debug"));
        assert!(is_safe_skill_dir_name("rust_debug.v1"));
        assert!(!is_safe_skill_dir_name("../rust-debug"));
        assert!(!is_safe_skill_dir_name("rust/debug"));
        assert!(!is_safe_skill_dir_name(".."));
    }

    #[test]
    fn disabled_skill_backups_filters_and_sorts_latest_first() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("lint.disabled-20260101000000")).unwrap();
        std::fs::create_dir_all(dir.path().join("lint.disabled-20260201000000")).unwrap();
        std::fs::create_dir_all(dir.path().join("other.disabled-20260101000000")).unwrap();
        std::fs::create_dir_all(dir.path().join("lint")).unwrap();

        let backups = disabled_skill_backups(dir.path(), Some("lint"));
        assert_eq!(backups.len(), 2);
        assert_eq!(backups[0].backup_name, "lint.disabled-20260201000000");
        assert_eq!(backups[0].skill_name, "lint");

        let latest = resolve_disabled_skill_backup(dir.path(), "lint", None).unwrap();
        assert_eq!(latest.backup_name, "lint.disabled-20260201000000");
    }
}
