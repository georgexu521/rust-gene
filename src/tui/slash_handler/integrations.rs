//! Integration slash command handlers.

use super::utils::*;

use crate::tui::app::TuiApp;

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
            if connected {
                "connected"
            } else {
                "disconnected"
            },
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
            let _ =
                tx.send(crate::engine::conversation_loop::ToolApprovalResponse::rejected_once());
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
