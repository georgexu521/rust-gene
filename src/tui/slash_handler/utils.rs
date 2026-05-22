//! Shared utility functions for slash command handlers.
//!
//! Extracted from `slash_handler.rs` to keep the main handler file focused on
//! command routing logic. All functions and structs here are `pub(crate)` so
//! they remain visible within the `tui` module but are not part of the public API.
#![allow(dead_code)]

use crate::tools::Tool;
pub(crate) use crate::tui::app::TuiApp;

// ─── File-Path Helpers ──────────────────────────────────────────────────

pub(crate) fn priority_agent_home_dir() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".priority-agent")
}

pub(crate) fn prompt_file() -> std::path::PathBuf {
    priority_agent_home_dir().join("prompt.txt")
}

pub(crate) fn webhooks_file() -> std::path::PathBuf {
    priority_agent_home_dir().join("webhooks.json")
}

pub(crate) fn runtime_prefs_file() -> std::path::PathBuf {
    priority_agent_home_dir().join("runtime_prefs.json")
}

pub(crate) fn preamble_file() -> std::path::PathBuf {
    priority_agent_home_dir().join("preamble.txt")
}

pub(crate) fn slots_file() -> std::path::PathBuf {
    priority_agent_home_dir().join("slots.json")
}

pub(crate) fn snippet_dir() -> std::path::PathBuf {
    priority_agent_home_dir().join("snippets")
}

// ─── RuntimePrefs ───────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub(crate) struct RuntimePrefs {
    #[serde(default)]
    pub(crate) verbose: bool,
    #[serde(default)]
    pub(crate) trace: bool,
    #[serde(default)]
    pub(crate) stealth: bool,
    #[serde(default)]
    pub(crate) shadow: bool,
    #[serde(default = "default_backend")]
    pub(crate) backend: String,
    #[serde(default)]
    pub(crate) sandbox: bool,
    #[serde(default)]
    pub(crate) subscriptions: Vec<String>,
    #[serde(default)]
    pub(crate) logged_in_provider: Option<String>,
    #[serde(default = "default_effort_level")]
    pub(crate) effort_level: String,
    #[serde(default)]
    pub(crate) ticker_message: Option<String>,
    #[serde(default)]
    pub(crate) slack_webhook_url: Option<String>,
    #[serde(default)]
    pub(crate) slack_default_channel: Option<String>,
}

fn default_backend() -> String {
    "local".to_string()
}

fn default_effort_level() -> String {
    "normal".to_string()
}

// ─── RuntimePrefs load / save ───────────────────────────────────────────

pub(crate) fn load_runtime_prefs() -> Result<RuntimePrefs, String> {
    let path = runtime_prefs_file();
    if !path.exists() {
        return Ok(RuntimePrefs::default());
    }
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {}", path.display(), e))?;
    serde_json::from_str::<RuntimePrefs>(&text).map_err(|e| format!("{}: {}", path.display(), e))
}

pub(crate) fn save_runtime_prefs(prefs: &RuntimePrefs) -> Result<(), String> {
    let path = runtime_prefs_file();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {}", parent.display(), e))?;
    }
    let text = serde_json::to_string_pretty(prefs).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| format!("{}: {}", path.display(), e))
}

// ─── Preamble helpers ───────────────────────────────────────────────────

pub(crate) fn read_preamble() -> Result<Option<String>, String> {
    let path = preamble_file();
    if !path.exists() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {}", path.display(), e))?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(trimmed.to_string()))
    }
}

pub(crate) fn write_preamble(text: &str) -> Result<(), String> {
    let path = preamble_file();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {}", parent.display(), e))?;
    }
    std::fs::write(&path, text.trim()).map_err(|e| format!("{}: {}", path.display(), e))
}

pub(crate) fn reset_preamble() -> Result<(), String> {
    let path = preamble_file();
    if !path.exists() {
        return Ok(());
    }
    std::fs::remove_file(&path).map_err(|e| format!("{}: {}", path.display(), e))
}

// ─── Slots helpers ──────────────────────────────────────────────────────

pub(crate) fn load_slots() -> Result<std::collections::HashMap<String, String>, String> {
    let path = slots_file();
    if !path.exists() {
        return Ok(std::collections::HashMap::new());
    }
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {}", path.display(), e))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {}", path.display(), e))
}

pub(crate) fn save_slots(map: &std::collections::HashMap<String, String>) -> Result<(), String> {
    let path = slots_file();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {}", parent.display(), e))?;
    }
    let payload = serde_json::to_string_pretty(map).map_err(|e| e.to_string())?;
    std::fs::write(&path, payload).map_err(|e| format!("{}: {}", path.display(), e))
}

// ─── Message conversion ─────────────────────────────────────────────────

pub(crate) fn message_items_to_api_messages(
    messages: &[crate::state::MessageItem],
) -> Vec<crate::services::api::Message> {
    messages
        .iter()
        .map(|m| match m.role {
            crate::state::MessageRole::User => {
                crate::services::api::Message::user(m.content.clone())
            }
            crate::state::MessageRole::Assistant => {
                crate::services::api::Message::assistant(m.content.clone())
            }
            crate::state::MessageRole::System => {
                crate::services::api::Message::system(m.content.clone())
            }
            crate::state::MessageRole::Tool => {
                crate::services::api::Message::tool(String::new(), m.content.clone())
            }
        })
        .collect()
}

// ─── Argument parsing ───────────────────────────────────────────────────

pub(crate) fn parse_optional_count(args: &str, cmd: &str) -> Result<usize, String> {
    if args.trim().is_empty() {
        return Ok(1);
    }
    let n = args
        .trim()
        .parse::<usize>()
        .map_err(|_| format!("Usage: {} [n]", cmd))?;
    if n == 0 {
        return Err(format!("Usage: {} [n] (n must be >= 1)", cmd));
    }
    Ok(n)
}

// ─── Config helpers ─────────────────────────────────────────────────────

pub(crate) fn format_config_summary(config: &crate::services::config::AppConfig) -> String {
    crate::services::config::format_config_summary(config)
}

pub(crate) fn get_config_value(
    config: &crate::services::config::AppConfig,
    key: &str,
) -> Option<String> {
    crate::services::config::get_config_value(config, key)
}

pub(crate) fn parse_bool(value: &str) -> Result<bool, String> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "1" | "on" | "yes" => Ok(true),
        "false" | "0" | "off" | "no" => Ok(false),
        _ => Err(format!("Invalid boolean value: {}", value)),
    }
}

pub(crate) fn set_config_value(
    config: &mut crate::services::config::AppConfig,
    key: &str,
    value: &str,
) -> Result<(), String> {
    crate::services::config::set_config_value(config, key, value)
}

// ─── Test result counters ───────────────────────────────────────────────

pub(crate) fn count_test_passed(output: &str) -> u32 {
    // Rust: "test result: ok. X passed"
    // Node: "X passing"
    // Python: "X passed"
    let candidates = [
        regex::Regex::new(r"(\d+) passed").ok(),
        regex::Regex::new(r"test result: ok\. (\d+) passed").ok(),
        regex::Regex::new(r"(\d+) passing").ok(),
    ];
    let mut max = 0u32;
    for re in candidates.iter().flatten() {
        if let Some(caps) = re.captures(output) {
            if let Ok(n) = caps.get(1).unwrap().as_str().parse::<u32>() {
                max = max.max(n);
            }
        }
    }
    max
}

pub(crate) fn count_test_failed(output: &str) -> u32 {
    let candidates = [
        regex::Regex::new(r"(\d+) failed").ok(),
        regex::Regex::new(r"test result: FAILED\. (\d+) failed").ok(),
    ];
    let mut max = 0u32;
    for re in candidates.iter().flatten() {
        if let Some(caps) = re.captures(output) {
            if let Ok(n) = caps.get(1).unwrap().as_str().parse::<u32>() {
                max = max.max(n);
            }
        }
    }
    max
}

// ─── Permission TOML merge ──────────────────────────────────────────────

/// Merge two permission TOML configs, deduplicating by pattern
pub(crate) fn merge_permission_toml(existing: &str, imported: &str) -> Result<String, String> {
    let mut existing_rules: crate::permissions::PermissionRules =
        toml::from_str(existing).map_err(|e| format!("Parse existing: {}", e))?;
    let imported_rules: crate::permissions::PermissionRules =
        toml::from_str(imported).map_err(|e| format!("Parse imported: {}", e))?;

    // Deduplicate helper
    let mut seen = std::collections::HashSet::new();
    let mut dedup = |rules: &mut Vec<crate::permissions::SourcedRule>| {
        rules.retain(|r| seen.insert(r.pattern.clone()));
    };

    existing_rules
        .always_allow
        .extend(imported_rules.always_allow);
    dedup(&mut existing_rules.always_allow);
    existing_rules
        .always_deny
        .extend(imported_rules.always_deny);
    dedup(&mut existing_rules.always_deny);
    existing_rules.always_ask.extend(imported_rules.always_ask);
    dedup(&mut existing_rules.always_ask);

    toml::to_string_pretty(&existing_rules).map_err(|e| format!("Serialize: {}", e))
}

// ─── Default keybindings ────────────────────────────────────────────────

pub(crate) fn get_default_keybindings() -> String {
    serde_json::json!({
        "version": 1,
        "contexts": {
            "global": {
                "Ctrl+C": "cancel",
                "Ctrl+Z": "undo",
                "Ctrl+S": "save"
            },
            "chat": {
                "Enter": "submit",
                "Shift+Enter": "newline",
                "Ctrl+J": "history_up",
                "Ctrl+K": "history_down",
                "Ctrl+B": "toggle_sidebar"
            },
            "vim_normal": {
                "j": "down",
                "k": "up",
                "i": "insert_mode",
                "Ctrl+V": "toggle_mode"
            }
        }
    })
    .to_string()
}

// ─── Rollback helpers ───────────────────────────────────────────────────

#[derive(Debug)]
pub(crate) struct ParsedRollbackArgs {
    pub(crate) target: String,
    pub(crate) confirmed: bool,
}

pub(crate) fn parse_rollback_args(args: &str) -> Result<ParsedRollbackArgs, String> {
    let mut target: Option<&str> = None;
    let mut confirmed = false;

    for part in args.split_whitespace() {
        if part == "--yes" {
            confirmed = true;
            continue;
        }
        if part.starts_with("--") {
            return Err(format!(
                "Unknown option: {}.\nUsage: /rollback [target|last-file|file_change_id] --yes",
                part
            ));
        }
        if target.is_some() {
            return Err(
                "Too many arguments.\nUsage: /rollback [target|last-file|file_change_id] --yes\nExample: /rollback HEAD~1 --yes"
                    .to_string(),
            );
        }
        target = Some(part);
    }

    Ok(ParsedRollbackArgs {
        target: target.unwrap_or("HEAD~1").to_string(),
        confirmed,
    })
}

pub(crate) fn is_valid_rollback_target(target: &str) -> bool {
    !target.is_empty()
        && !target.starts_with('-')
        && target.chars().all(|c| {
            c.is_ascii_alphanumeric()
                || matches!(c, '-' | '_' | '.' | '/' | '~' | '^' | '@' | '{' | '}')
        })
}

// ─── File / directory utilities ─────────────────────────────────────────

pub(crate) fn count_files_recursively(path: &std::path::Path) -> usize {
    if !path.exists() {
        return 0;
    }
    let mut count = 0usize;
    let mut stack = vec![path.to_path_buf()];
    while let Some(p) = stack.pop() {
        let Ok(read_dir) = std::fs::read_dir(&p) else {
            continue;
        };
        for entry in read_dir.flatten() {
            let ep = entry.path();
            if ep.is_file() {
                count += 1;
            } else if ep.is_dir() {
                stack.push(ep);
            }
        }
    }
    count
}

pub(crate) fn collect_chrome_bookmarks(
    node: &serde_json::Value,
    out: &mut Vec<String>,
    limit: usize,
) {
    if out.len() >= limit {
        return;
    }
    if let Some(obj) = node.as_object() {
        if let (Some(name), Some(url)) = (
            obj.get("name").and_then(|v| v.as_str()),
            obj.get("url").and_then(|v| v.as_str()),
        ) {
            out.push(format!("{} -> {}", name, url));
            if out.len() >= limit {
                return;
            }
        }
        for v in obj.values() {
            collect_chrome_bookmarks(v, out, limit);
            if out.len() >= limit {
                return;
            }
        }
        return;
    }
    if let Some(arr) = node.as_array() {
        for v in arr {
            collect_chrome_bookmarks(v, out, limit);
            if out.len() >= limit {
                return;
            }
        }
    }
}

pub(crate) fn build_tree_lines(
    root: &std::path::Path,
    level: usize,
    max_depth: usize,
    lines: &mut Vec<String>,
    max_lines: usize,
) {
    if level >= max_depth || lines.len() >= max_lines {
        return;
    }
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    let mut items: Vec<_> = entries.flatten().collect();
    items.sort_by_key(|e| e.file_name());
    for entry in items {
        if lines.len() >= max_lines {
            break;
        }
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let indent = "  ".repeat(level);
        if path.is_dir() {
            lines.push(format!("{}- {}/", indent, name));
            build_tree_lines(&path, level + 1, max_depth, lines, max_lines);
        } else {
            lines.push(format!("{}- {}", indent, name));
        }
    }
}

// ─── Prompt file helpers ────────────────────────────────────────────────

pub(crate) fn read_prompt_file() -> Result<Option<String>, String> {
    let path = prompt_file();
    if !path.exists() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {}", path.display(), e))?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(trimmed.to_string()))
    }
}

pub(crate) fn write_prompt_file(text: &str) -> Result<(), String> {
    let path = prompt_file();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {}", parent.display(), e))?;
    }
    std::fs::write(&path, text.trim()).map_err(|e| format!("{}: {}", path.display(), e))
}

pub(crate) fn append_prompt_file(text: &str) -> Result<(), String> {
    let next = text.trim();
    if next.is_empty() {
        return Err("Prompt content cannot be empty.".to_string());
    }
    let merged = match read_prompt_file()? {
        Some(existing) => format!("{}\n\n{}", existing, next),
        None => next.to_string(),
    };
    write_prompt_file(&merged)
}

pub(crate) fn reset_prompt_file() -> Result<(), String> {
    let path = prompt_file();
    if !path.exists() {
        return Ok(());
    }
    std::fs::remove_file(&path).map_err(|e| format!("{}: {}", path.display(), e))
}

// ─── Webhook helpers ────────────────────────────────────────────────────

pub(crate) fn load_webhooks() -> Result<std::collections::HashMap<String, String>, String> {
    let path = webhooks_file();
    if !path.exists() {
        return Ok(std::collections::HashMap::new());
    }
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {}", path.display(), e))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {}", path.display(), e))
}

pub(crate) fn save_webhooks(map: &std::collections::HashMap<String, String>) -> Result<(), String> {
    let path = webhooks_file();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {}", parent.display(), e))?;
    }
    let payload = serde_json::to_string_pretty(map).map_err(|e| e.to_string())?;
    std::fs::write(&path, payload).map_err(|e| format!("{}: {}", path.display(), e))
}

pub(crate) fn is_valid_webhook_url(raw: &str) -> bool {
    if !(raw.starts_with("http://") || raw.starts_with("https://")) {
        return false;
    }
    match reqwest::Url::parse(raw) {
        Ok(url) => matches!(url.scheme(), "http" | "https") && url.host_str().is_some(),
        Err(_) => false,
    }
}

pub(crate) async fn test_webhook(url: &str, payload: &str) -> Result<String, String> {
    let body: serde_json::Value = serde_json::from_str(payload).unwrap_or_else(|_| {
        serde_json::json!({
            "message": payload,
        })
    });

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("failed to build http client: {}", e))?;
    let response = client
        .post(url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("request error: {}", e))?;

    let status = response.status();
    let response_text = response
        .text()
        .await
        .unwrap_or_else(|_| "<unable to read body>".to_string());
    let preview: String = response_text.chars().take(200).collect();

    if status.is_success() {
        Ok(format!(
            "Webhook test delivered successfully (status {}). Response: {}",
            status, preview
        ))
    } else {
        Err(format!("status {}. Response: {}", status, preview))
    }
}

// ─── Slack webhook ──────────────────────────────────────────────────────

pub(crate) async fn post_slack_webhook(
    webhook_url: &str,
    channel: Option<&str>,
    message: &str,
) -> Result<(), String> {
    let mut payload = serde_json::json!({
        "text": message,
    });
    if let Some(ch) = channel {
        payload["channel"] = serde_json::Value::String(ch.to_string());
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("failed to build client: {}", e))?;
    let resp = client
        .post(webhook_url)
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("request error: {}", e))?;
    let status = resp.status();
    if status.is_success() {
        Ok(())
    } else {
        let body = resp.text().await.unwrap_or_default();
        Err(format!("status {}: {}", status, body))
    }
}

// ─── Snippet helpers ────────────────────────────────────────────────────

pub(crate) fn sanitize_snippet_name(name: &str) -> Option<String> {
    let n = name.trim();
    if n.is_empty() {
        return None;
    }
    if n.contains('/') || n.contains('\\') {
        return None;
    }
    if n == "." || n == ".." {
        return None;
    }
    if n.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        Some(n.to_string())
    } else {
        None
    }
}

pub(crate) fn list_snippets() -> std::io::Result<Vec<String>> {
    let dir = snippet_dir();
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut names = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                names.push(stem.to_string());
            }
        }
    }
    names.sort();
    Ok(names)
}

// ─── SQLx migration helper ─────────────────────────────────────────────

pub(crate) async fn run_migrate_sqlx(app: &mut TuiApp, is_up: bool) -> String {
    if std::env::var("DATABASE_URL").is_err() {
        return "DATABASE_URL is not set. Export DATABASE_URL first, then run /migrate up|down."
            .to_string();
    }

    let command = if is_up {
        "sqlx migrate run"
    } else {
        "sqlx migrate revert"
    };

    let tool = crate::tools::BashTool;
    let ctx = app.build_tool_context().await;
    let params = serde_json::json!({
        "command": command,
        "description": if is_up { "sqlx migrate up" } else { "sqlx migrate down" },
    });
    let result = tool.execute(params, ctx).await;
    if result.success {
        if result.content.trim().is_empty() {
            if is_up {
                "Migrations applied successfully.".to_string()
            } else {
                "Migration reverted successfully.".to_string()
            }
        } else {
            result.content
        }
    } else {
        format!(
            "Migration command failed: {}\nHint: ensure `sqlx` CLI is installed (`cargo install sqlx-cli --no-default-features --features native-tls,postgres`) and DATABASE_URL is valid.",
            result.error.unwrap_or_else(|| "unknown error".to_string())
        )
    }
}

// ─── Cleanup helpers ────────────────────────────────────────────────────

pub(crate) fn cleanup_sessions(app: &mut TuiApp, keep_count: usize) -> String {
    let sessions = match app.session_manager.list_sessions(10_000) {
        Ok(v) => v,
        Err(e) => return format!("Failed to list sessions: {}", e),
    };
    if sessions.len() <= keep_count {
        return format!(
            "No session cleanup needed. {} session(s) <= keep {}.",
            sessions.len(),
            keep_count
        );
    }

    let current = app
        .session_manager
        .current_session_id()
        .map(|s| s.to_string());
    let mut keep_ids: std::collections::HashSet<String> = sessions
        .iter()
        .take(keep_count)
        .map(|s| s.id.clone())
        .collect();
    if let Some(cur) = current {
        keep_ids.insert(cur);
    }

    let mut deleted = 0usize;
    let mut failed = 0usize;
    for sess in sessions {
        if keep_ids.contains(&sess.id) {
            continue;
        }
        match app.session_manager.delete_session(&sess.id) {
            Ok(_) => deleted += 1,
            Err(_) => failed += 1,
        }
    }

    format!(
        "Session cleanup complete: deleted {}, failed {}, kept {}.",
        deleted,
        failed,
        keep_ids.len()
    )
}

pub(crate) fn cleanup_cache() -> String {
    crate::tools::file_cache::GLOBAL_FILE_CACHE.clear();
    let mut cleared_items = 1usize; // in-memory file cache

    let paths = vec![
        priority_agent_home_dir().join("cache"),
        dirs::data_local_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("priority-agent")
            .join("tool-results"),
    ];

    let mut failures = Vec::new();
    for p in paths {
        if p.exists() {
            match std::fs::remove_dir_all(&p) {
                Ok(_) => cleared_items += 1,
                Err(e) => failures.push(format!("{}: {}", p.display(), e)),
            }
        }
    }

    if failures.is_empty() {
        format!("Cache cleaned ({} target(s) cleared).", cleared_items)
    } else {
        format!(
            "Cache partially cleaned ({} target(s) cleared).\nFailures:\n- {}",
            cleared_items,
            failures.join("\n- ")
        )
    }
}

pub(crate) fn cleanup_logs() -> String {
    let logs_dir = priority_agent_home_dir().join("logs");
    if !logs_dir.exists() {
        return "No logs directory found.".to_string();
    }
    let mut deleted = 0usize;
    let mut failed = 0usize;

    let entries = match std::fs::read_dir(&logs_dir) {
        Ok(v) => v,
        Err(e) => return format!("Failed to read logs directory: {}", e),
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        match std::fs::remove_file(&path) {
            Ok(_) => deleted += 1,
            Err(_) => failed += 1,
        }
    }
    format!(
        "Logs cleanup complete: deleted {}, failed {} (dir: {}).",
        deleted,
        failed,
        logs_dir.display()
    )
}

// ─── Bookmarks / Tags helpers ───────────────────────────────────────────

pub(crate) fn bookmarks_file() -> std::path::PathBuf {
    priority_agent_home_dir().join("bookmarks.json")
}

pub(crate) fn tags_file() -> std::path::PathBuf {
    priority_agent_home_dir().join("tags.json")
}

pub(crate) fn sanitize_note_name(name: &str) -> Option<String> {
    let n = name.trim();
    if n.is_empty() {
        return None;
    }
    if n == "." || n == ".." || n.contains('/') || n.contains('\\') {
        return None;
    }
    if n.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        Some(n.to_string())
    } else {
        None
    }
}

pub(crate) fn load_bookmarks() -> Result<std::collections::HashMap<String, String>, String> {
    let path = bookmarks_file();
    if !path.exists() {
        return Ok(std::collections::HashMap::new());
    }
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {}", path.display(), e))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {}", path.display(), e))
}

pub(crate) fn save_bookmarks(
    map: &std::collections::HashMap<String, String>,
) -> Result<(), String> {
    let path = bookmarks_file();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {}", parent.display(), e))?;
    }
    let text = serde_json::to_string_pretty(map).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| format!("{}: {}", path.display(), e))
}

pub(crate) fn load_tags() -> Result<std::collections::HashMap<String, Vec<String>>, String> {
    let path = tags_file();
    if !path.exists() {
        return Ok(std::collections::HashMap::new());
    }
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {}", path.display(), e))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {}", path.display(), e))
}

pub(crate) fn save_tags(
    map: &std::collections::HashMap<String, Vec<String>>,
) -> Result<(), String> {
    let path = tags_file();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {}", parent.display(), e))?;
    }
    let text = serde_json::to_string_pretty(map).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| format!("{}: {}", path.display(), e))
}

// ─── Profile / Feedback helpers ─────────────────────────────────────────

pub(crate) fn message_role_label(role: crate::state::MessageRole) -> &'static str {
    match role {
        crate::state::MessageRole::System => "system",
        crate::state::MessageRole::User => "user",
        crate::state::MessageRole::Assistant => "assistant",
        crate::state::MessageRole::Tool => "tool",
    }
}

pub(crate) fn profile_file() -> std::path::PathBuf {
    priority_agent_home_dir().join("profile.json")
}

pub(crate) fn feedback_file() -> std::path::PathBuf {
    priority_agent_home_dir().join("feedback.jsonl")
}

pub(crate) fn sanitize_profile_key(key: &str) -> Option<String> {
    let k = key.trim();
    if k.is_empty() {
        return None;
    }
    if k.contains('/') || k.contains('\\') || k == "." || k == ".." {
        return None;
    }
    if k.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
    {
        Some(k.to_string())
    } else {
        None
    }
}

pub(crate) fn load_profile() -> Result<std::collections::HashMap<String, String>, String> {
    let path = profile_file();
    if !path.exists() {
        return Ok(std::collections::HashMap::new());
    }
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {}", path.display(), e))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {}", path.display(), e))
}

pub(crate) fn save_profile(map: &std::collections::HashMap<String, String>) -> Result<(), String> {
    let path = profile_file();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {}", parent.display(), e))?;
    }
    let text = serde_json::to_string_pretty(map).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| format!("{}: {}", path.display(), e))
}

pub(crate) fn append_feedback(
    session_id: &str,
    message: &str,
) -> Result<std::path::PathBuf, String> {
    let path = feedback_file();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {}", parent.display(), e))?;
    }
    let record = serde_json::json!({
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "session_id": session_id,
        "message": message,
    });
    let mut payload = serde_json::to_string(&record).map_err(|e| e.to_string())?;
    payload.push('\n');
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| format!("{}: {}", path.display(), e))?;
    use std::io::Write as _;
    f.write_all(payload.as_bytes())
        .map_err(|e| format!("{}: {}", path.display(), e))?;
    Ok(path)
}

pub(crate) fn latest_trace_for_app(app: &TuiApp) -> Option<crate::engine::trace::TurnTrace> {
    app.streaming_engine
        .as_ref()
        .and_then(|engine| engine.trace_store().latest())
        .or_else(|| app.session_manager.latest_trace().ok().flatten())
}
