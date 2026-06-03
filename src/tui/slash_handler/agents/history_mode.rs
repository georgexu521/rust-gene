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
