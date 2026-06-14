use super::*;

/// /history - 会话历史查看
pub fn handle_history(app: &TuiApp, args: &str) -> String {
    let limit = args.parse::<usize>().unwrap_or(20);
    let messages = &app.messages;

    if messages.is_empty() {
        return "No messages in current session.".to_string();
    }

    let start = if messages.len() > limit {
        messages.len() - limit
    } else {
        0
    };

    let mut lines = vec![format!("Recent {} messages: ", messages.len() - start)];
    for (i, msg) in messages.iter().enumerate().skip(start) {
        let role_str = match msg.role {
            crate::state::MessageRole::User => "user",
            crate::state::MessageRole::Assistant => "assistant",
            crate::state::MessageRole::System => "system",
            crate::state::MessageRole::Tool => "tool",
        };
        let preview = if msg.content.len() > 60 {
            format!("{}...", &msg.content[..60])
        } else {
            msg.content.clone()
        };
        lines.push(format!("{}. [{}] {}", i + 1, role_str, preview));
    }
    lines.join("\n")
}

/// /prompt-history - submitted prompt history for composer reuse.
pub fn handle_prompt_history(app: &TuiApp, args: &str) -> String {
    let limit = args.trim().parse::<usize>().unwrap_or(20);
    let lines = app.prompt_history_lines(limit);
    if lines.is_empty() {
        return "No submitted prompts in this TUI session.".to_string();
    }

    let mut output = vec![format!("Recent {} submitted prompts:", lines.len())];
    output.extend(lines);
    output.push("Use Ctrl+J / Ctrl+K to cycle prompt history in the composer.".to_string());
    output.join("\n")
}

/// /prompt-stash - save or restore the current composer draft.
pub fn handle_prompt_stash(app: &mut TuiApp, args: &str) -> String {
    match args.trim() {
        "" => {
            if !app.composer.text.value().trim().is_empty() {
                if app.save_prompt_stash_from_input() {
                    "Prompt draft stashed. Run /prompt-stash again to restore it.".to_string()
                } else {
                    "No prompt draft to stash.".to_string()
                }
            } else if app.restore_prompt_stash_to_input() {
                "Prompt draft restored to the composer.".to_string()
            } else {
                "No stashed prompt draft.".to_string()
            }
        }
        "save" => {
            if app.save_prompt_stash_from_input() {
                "Prompt draft stashed.".to_string()
            } else {
                "No prompt draft to stash.".to_string()
            }
        }
        "restore" => {
            if app.restore_prompt_stash_to_input() {
                "Prompt draft restored to the composer.".to_string()
            } else {
                "No stashed prompt draft.".to_string()
            }
        }
        "clear" => {
            if app.clear_prompt_stash() {
                "Prompt stash cleared.".to_string()
            } else {
                "Prompt stash is already empty.".to_string()
            }
        }
        "show" => app
            .prompt_stash_summary()
            .map(|summary| format!("Stashed prompt: {}", summary))
            .unwrap_or_else(|| "No stashed prompt draft.".to_string()),
        other => {
            format!("Unknown prompt-stash action: {other}. Use save, restore, clear, or show.")
        }
    }
}

/// /attach - attach file/context paths to the next composer prompt.
pub fn handle_attach(app: &mut TuiApp, args: &str) -> String {
    let args = args.trim();
    if args.is_empty() || args == "list" {
        if app.composer_attachment_count() == 0 {
            return "No composer attachments. Use /attach <path> to add one.".to_string();
        }
        let mut lines = vec![format!(
            "Composer attachments ({}):",
            app.composer_attachment_count()
        )];
        for summary in app.composer_attachment_summaries() {
            lines.push(format!(
                "- {}  preview:/attach preview <n>  remove:/attach remove <n>",
                summary
            ));
        }
        return lines.join("\n");
    }

    if args == "browse" || args.starts_with("browse ") {
        let root = args.strip_prefix("browse").map(str::trim);
        return app.open_composer_file_picker(root.filter(|value| !value.is_empty()), false);
    }

    if args == "preview" || args.starts_with("preview ") {
        let index = args
            .strip_prefix("preview")
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .and_then(|value| value.parse::<usize>().ok());
        if app.open_attachment_viewer(index) {
            return "Opened attachment preview.".to_string();
        }
        return "No composer attachment to preview.".to_string();
    }

    if args == "clear" {
        let removed = app.clear_composer_attachments();
        return format!("Cleared {removed} composer attachment(s).");
    }

    if let Some(rest) = args.strip_prefix("remove ") {
        let Some(index) = rest.trim().parse::<usize>().ok() else {
            return "Usage: /attach remove <n>".to_string();
        };
        return app
            .remove_composer_attachment(index)
            .map(|path| format!("Removed attachment: {path}"))
            .unwrap_or_else(|| format!("No attachment at index {index}."));
    }

    app.attach_context_path(args)
        .unwrap_or_else(|message| message)
}
/// /mode - 切换交互模式
pub fn handle_mode(app: &mut TuiApp, args: &str) -> String {
    let current_agent_mode = app.current_agent_mode_label();
    let current_ui_mode = format!("{:?}", app.mode);
    if args.is_empty() {
        return format!(
            "Current agent mode: {}\nUI mode: {}\n\nAgent modes:\n\
             - auto: Route each request from its content\n\
             - build: Bias ambiguous coding requests toward implementation\n\
             - plan: Inspect and plan before implementation\n\
             - explore: Inspect and answer from evidence\n\
             - review: Findings-first code review stance\n\n\
             UI modes: chat, settings, vim\n\
             Usage: /mode <auto|build|plan|explore|review>",
            current_agent_mode, current_ui_mode
        );
    }

    let new_mode = args.trim().to_lowercase();
    if let Some(mode) = AgentMode::parse(&new_mode) {
        app.set_agent_mode(mode);
        return format!("Agent mode switched to {}.", mode.label());
    }

    match new_mode.as_str() {
        "chat" => {
            app.mode = AppMode::Chat;
            "UI mode switched to chat.".to_string()
        }
        "settings" => {
            let config = crate::services::config::AppConfig::load().unwrap_or_default();
            app.settings_state = Some(crate::tui::components::settings::SettingsState::new(
                config,
                app.keybindings.clone(),
            ));
            app.mode = AppMode::Settings;
            "UI mode switched to settings.".to_string()
        }
        "vim" | "vim_normal" => {
            app.mode = AppMode::VimNormal;
            "UI mode switched to vim_normal. Use j/k to navigate, i to return to insert mode."
                .to_string()
        }
        _ => format!(
            "Unknown mode: {}. Agent modes: auto, build, plan, explore, review. UI modes: chat, settings, vim",
            new_mode
        ),
    }
}
