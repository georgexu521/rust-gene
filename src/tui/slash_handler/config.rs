//! Config / permissions / integration handlers extracted from slash_handler.
//!
//! All functions here are `pub` so they remain callable from the TUI command
//! dispatcher via `pub use config::*;` in the parent `mod.rs`.

use super::utils::*;

use std::sync::Arc;
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

// ─── Batch 1: /reload ───────────────────────────────────────────────

pub async fn handle_reload(app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() || args == "config" {
        match crate::services::config::AppConfig::load() {
            Ok(config) => {
                // Apply visible UI config immediately.
                app.theme = Arc::new(crate::tui::theme::Theme::from_name(&config.ui.theme));
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
        let working_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let mut registry = crate::tools::ToolRegistry::default_registry();
        let report = crate::tools::plugin_tool::register_enabled_plugin_tools_with_report(
            &mut registry,
            &working_dir,
        );
        format!(
            "{}\nRuntime note: this command validates the current plugin registry snapshot; newly injected plugin tools are available after the next engine registry rebuild.",
            report.summary()
        )
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

// Hook and profiling commands live in `observability.rs`.

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
    format!(
        "Focus mode {} (session-only, restart will reset).",
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

// Integration commands live in `integrations.rs`.
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

    if args == "schema" {
        return crate::services::config::format_config_schema_text();
    }

    if args == "paths" {
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let paths = crate::services::config::config_scope_paths(&cwd);
        return format!(
            "Config paths:\n  user = {}\n  project = {}\n  legacy = {}",
            paths.user_config.display(),
            paths.project_config.display(),
            paths.legacy_config_dir.display()
        );
    }

    if args == "doctor" {
        return match crate::services::config::AppConfig::load() {
            Ok(config) => {
                let issues = crate::services::config::validate_config(&config);
                if issues.is_empty() {
                    "Config doctor: ok".to_string()
                } else {
                    format!("Config doctor: warning\n- {}", issues.join("\n- "))
                }
            }
            Err(e) => format!("Config doctor: error\n- failed to load config: {}", e),
        };
    }

    if args == "export" || args == "export json" {
        return match crate::services::config::AppConfig::load() {
            Ok(config) => {
                let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
                let export = crate::services::config::redacted_config_export(&config, &cwd);
                serde_json::to_string_pretty(&export)
                    .unwrap_or_else(|e| format!("Failed to serialize config export: {}", e))
            }
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

    "Usage: /config [list|schema|paths|doctor|export|get <key>|set <key> <value>]".to_string()
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

// Runtime preference commands live in `runtime.rs`.

// Observability commands live in `observability.rs`.

/// /theme - Theme customization
pub fn handle_theme(app: &mut TuiApp, args: &str) -> String {
    let args = args.trim();
    if args.is_empty() || args == "show" {
        let current = crate::services::config::AppConfig::load()
            .map(|c| c.ui.theme)
            .unwrap_or_else(|_| "dark".to_string());
        return format!(
            "Current theme: {}\nAvailable: graphite, porcelain, nord, dracula, gruvbox-dark, catppuccin-mocha, dark, light, high-contrast\nUsage: /theme <preset> or /theme set <preset>",
            current
        );
    }

    if args == "list" {
        return "Available themes:\n- graphite\n- porcelain\n- nord\n- dracula\n- gruvbox-dark\n- catppuccin-mocha\n- dark\n- light\n- high-contrast".to_string();
    }

    let preset_raw = args.strip_prefix("set ").unwrap_or(args).trim();
    let preset = match preset_raw.parse::<crate::tui::theme::ThemePreset>() {
        Ok(v) => v,
        Err(_) => {
            return format!(
                "Unknown theme '{}'. Available: graphite, porcelain, nord, dracula, gruvbox-dark, catppuccin-mocha, dark, light, high-contrast",
                preset_raw
            );
        }
    };
    let preset_name = preset.to_string();

    app.theme = Arc::new(crate::tui::theme::Theme::from_preset(preset));

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
