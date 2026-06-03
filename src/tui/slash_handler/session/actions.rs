use super::*;

/// /undo - 撤销上一次操作
pub fn handle_undo(app: &mut TuiApp, args: &str) -> String {
    let session_id = match app.session_manager.current_session_id() {
        Some(id) => id,
        None => return "No active session.".to_string(),
    };

    let n = match parse_optional_count(args, "/undo") {
        Ok(v) => v,
        Err(e) => return e,
    };

    let mut results = Vec::new();
    for _ in 0..n {
        match app.session_manager.rewind_last_edit(session_id) {
            Ok(msg) => results.push(msg),
            Err(e) => {
                results.push(format!("Nothing to undo or undo failed: {}", e));
                break;
            }
        }
    }
    results.join("\n")
}

/// /redo - 重做
pub fn handle_redo(app: &mut TuiApp, args: &str) -> String {
    let session_id = match app.session_manager.current_session_id() {
        Some(id) => id,
        None => return "No active session.".to_string(),
    };

    let n = match parse_optional_count(args, "/redo") {
        Ok(v) => v,
        Err(e) => return e,
    };

    let mut results = Vec::new();
    for _ in 0..n {
        match app.session_manager.redo_last_edit(session_id) {
            Ok(msg) => results.push(msg),
            Err(e) => {
                results.push(format!("Nothing to redo or redo failed: {}", e));
                break;
            }
        }
    }
    results.join("\n")
}

/// /retry - 重试上一次 LLM 调用
pub async fn handle_retry(app: &mut TuiApp, args: &str) -> String {
    if !args.trim().is_empty() {
        return "Usage: /retry".to_string();
    }

    // Retry the last user turn: remove that user message and everything after it,
    // then resend the same content to regenerate downstream responses coherently.
    let Some(last_user_idx) = app
        .messages
        .iter()
        .rposition(|m| m.role == crate::state::MessageRole::User)
    else {
        return "No user message to retry.".to_string();
    };
    let content = app.messages[last_user_idx].content.clone();
    app.messages.truncate(last_user_idx);

    // Keep persistence and engine history consistent with truncated UI messages.
    if let Some(session_id) = app.session_manager.current_session_id() {
        if let Err(e) = app
            .session_manager
            .replace_messages(session_id, &app.messages)
        {
            return format!("Retry failed to rewrite session messages: {}", e);
        }
    }
    if let Some(ref engine) = app.streaming_engine {
        engine
            .set_history(message_items_to_api_messages(&app.messages))
            .await;
    }

    app.send_message(content).await;
    String::new()
}

/// /stop - 停止当前操作
pub fn handle_stop(app: &mut TuiApp, _args: &str) -> String {
    if app.is_querying {
        app.is_querying = false;
        crate::engine::workflow::metrics::record_drift_interruption();
        "Stopping current operation...".to_string()
    } else {
        "No operation in progress.".to_string()
    }
}

/// /share - 分享当前会话
pub fn handle_share(app: &mut TuiApp, _args: &str) -> String {
    if let Some(id) = app.session_manager.current_session_id() {
        match app.session_manager.export_session(id) {
            Ok(json) => {
                let path = dirs::home_dir()
                    .unwrap_or_else(|| std::path::PathBuf::from("."))
                    .join(".priority-agent")
                    .join(format!("share_{}.json", &id[..8.min(id.len())]));
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent).ok();
                }
                match std::fs::write(&path, &json) {
                    Ok(_) => format!("Session exported to: {}", path.display()),
                    Err(e) => format!("Failed to write: {}", e),
                }
            }
            Err(e) => format!("Failed to export: {}", e),
        }
    } else {
        "No active session to share.".to_string()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Extended 3: More commands
// ═══════════════════════════════════════════════════════════════════════

/// /export - Export data
pub async fn handle_export_data(app: &mut TuiApp, args: &str) -> String {
    let tool = crate::tools::BashTool;
    let ctx = app.build_tool_context().await;

    let format = if args.is_empty() { "json" } else { args };
    let session_id = app
        .session_manager
        .current_session_id()
        .unwrap_or("unknown");

    let cmd = match format {
        "json" => format!("echo 'Session {}' > /tmp/export.json && cat /tmp/export.json", &session_id[..8.min(session_id.len())]),
        "md" => format!("echo '# Session Export' > /tmp/export.md && echo 'Session: {}' >> /tmp/export.md && cat /tmp/export.md", &session_id[..8.min(session_id.len())]),
        _ => return "Usage: /export [json|md]".to_string(),
    };

    let params = serde_json::json!({
        "command": cmd,
        "description": "Export session data"
    });
    let result = tool.execute(params, ctx).await;
    if result.success {
        result.content
    } else {
        result.error.unwrap_or_default()
    }
}

/// /import - Import data
pub async fn handle_import(app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /import <file_path>".to_string();
    }

    let path = std::path::Path::new(args.trim());
    if !path.exists() {
        format!("File not found: {}", args)
    } else if !path.is_file() {
        format!("Not a file: {}", args)
    } else {
        let text = match tokio::fs::read_to_string(path).await {
            Ok(v) => v,
            Err(e) => return format!("Failed to read import file: {}", e),
        };
        let value: serde_json::Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => return format!("Invalid JSON import file: {}", e),
        };
        let messages = match value.get("messages").and_then(|v| v.as_array()) {
            Some(v) => v,
            None => return "Import file missing `messages` array.".to_string(),
        };
        if messages.is_empty() {
            return "Import file has no messages.".to_string();
        }
        let mut imported = 0usize;
        for m in messages {
            let role_str = m.get("role").and_then(|v| v.as_str()).unwrap_or("system");
            let content = m
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            if content.is_empty() {
                continue;
            }
            let role = match role_str {
                "user" => crate::state::MessageRole::User,
                "assistant" => crate::state::MessageRole::Assistant,
                "tool" => crate::state::MessageRole::Tool,
                _ => crate::state::MessageRole::System,
            };
            let item = crate::state::MessageItem {
                id: format!("import_{}", app.messages.len() + imported),
                role,
                content: content.to_string(),
                timestamp: std::time::SystemTime::now(),
                metadata: Default::default(),
            };
            app.messages.push(item.clone());
            let _ = app.session_manager.add_message(role, &item.content);
            imported += 1;
        }
        if let Some(ref engine) = app.streaming_engine {
            let _ = engine
                .set_history(message_items_to_api_messages(&app.messages))
                .await;
        }
        format!("Imported {} message(s) from {}.", imported, path.display())
    }
}

/// /save-session - Save current session
pub fn handle_save_session(app: &TuiApp) -> String {
    if let Some(id) = app.session_manager.current_session_id() {
        format!("Session {} auto-saved.", &id[..8.min(id.len())])
    } else {
        "No active session to save.".to_string()
    }
}

/// /load-session - Load a session
pub async fn handle_load_session(app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /load-session <session_id>".to_string();
    }
    app.restore_session(args).await
}

/// /merge - Merge sessions
pub async fn handle_merge(app: &mut TuiApp, args: &str) -> String {
    let args = args.trim();
    if args.is_empty() {
        return "Usage: /merge <session_id> into current".to_string();
    }
    let source_ref = args
        .strip_suffix("into current")
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or(args);

    let current_id = match app.session_manager.current_session_id() {
        Some(id) => id.to_string(),
        None => return "No active session.".to_string(),
    };

    let source_id = if let Ok(n) = source_ref.parse::<usize>() {
        match app.session_manager.list_sessions(100) {
            Ok(sessions) if n > 0 && n <= sessions.len() => sessions[n - 1].id.clone(),
            _ => {
                return "Invalid session number. Use /session list to see available sessions."
                    .to_string();
            }
        }
    } else {
        source_ref.to_string()
    };

    if source_id == current_id {
        return "Cannot merge current session into itself.".to_string();
    }

    let source_messages = match app.session_manager.load_messages(&source_id) {
        Ok(msgs) => msgs,
        Err(e) => return format!("Failed to load source session: {}", e),
    };
    if source_messages.is_empty() {
        return format!("Source session {} has no messages.", source_id);
    }

    let mut imported = 0usize;
    for msg in source_messages {
        if app
            .session_manager
            .add_message(msg.role, &msg.content)
            .is_ok()
        {
            app.messages.push(msg);
            imported += 1;
        }
    }

    if let Some(ref engine) = app.streaming_engine {
        engine
            .set_history(message_items_to_api_messages(&app.messages))
            .await;
    }

    format!(
        "Merged {} message(s) from session {} into current session.",
        imported, source_id
    )
}

/// /compact - Compact context
pub async fn handle_compact(app: &mut TuiApp) -> String {
    let Some(ref engine) = app.streaming_engine else {
        return "Engine not initialized; cannot compact context.".to_string();
    };
    let history_before = engine.get_history().await;
    if history_before.is_empty() {
        return "No history to compact.".to_string();
    }
    let Some(attempt) = engine.compact_context_manually().await else {
        return "No history to compact.".to_string();
    };
    if attempt.circuit_open
        && attempt.decision == crate::engine::context_compressor::CompactionDecision::CircuitOpen
    {
        return format!("Context compact skipped: {}.", attempt.reason);
    }
    let compacted = engine.get_history().await;
    let after_msgs = compacted.len();
    let after_tokens = crate::engine::context_compressor::estimate_messages_tokens(&compacted);

    app.messages = compacted
        .into_iter()
        .enumerate()
        .map(|(idx, m)| {
            let (role, content) = match m {
                crate::services::api::Message::System { content } => {
                    (crate::state::MessageRole::System, content)
                }
                crate::services::api::Message::User { content } => {
                    (crate::state::MessageRole::User, content)
                }
                crate::services::api::Message::Assistant { content, .. } => {
                    (crate::state::MessageRole::Assistant, content)
                }
                crate::services::api::Message::Tool { content, .. } => {
                    (crate::state::MessageRole::Tool, content)
                }
            };
            crate::state::MessageItem {
                id: format!("compact_{}", idx),
                role,
                content,
                timestamp: std::time::SystemTime::now(),
                metadata: Default::default(),
            }
        })
        .collect();
    if let Some(session_id) = app.session_manager.current_session_id() {
        let _ = app
            .session_manager
            .replace_messages(session_id, &app.messages);
    }
    format!(
        "Context compacted: messages {} -> {}, tokens {} -> {}.",
        attempt.messages_before, after_msgs, attempt.before_tokens, after_tokens
    )
}

/// /cleanup - Cleanup old data
pub fn handle_cleanup(app: &mut TuiApp, args: &str) -> String {
    let mut parts = args.split_whitespace();
    let target = parts.next().unwrap_or("all");

    match target {
        "sessions" => {
            let mut keep: usize = 20;
            let mut confirmed = false;
            for token in parts {
                if token == "--yes" {
                    confirmed = true;
                } else if let Ok(v) = token.parse::<usize>() {
                    keep = v.max(1);
                } else {
                    return "Usage: /cleanup sessions [keep_count] --yes".to_string();
                }
            }
            if !confirmed {
                return format!(
                    "Session cleanup is destructive.\nUsage: /cleanup sessions [keep_count] --yes\nExample: /cleanup sessions {} --yes",
                    keep
                );
            }
            cleanup_sessions(app, keep)
        }
        "cache" => cleanup_cache(),
        "logs" => cleanup_logs(),
        "all" => {
            let confirmed = parts.any(|p| p == "--yes");
            if !confirmed {
                return "Full cleanup will remove old sessions, cache, and logs.\nUsage: /cleanup all --yes"
                    .to_string();
            }
            let session_msg = cleanup_sessions(app, 20);
            let cache_msg = cleanup_cache();
            let logs_msg = cleanup_logs();
            format!("{}\n{}\n{}", session_msg, cache_msg, logs_msg)
        }
        _ => "Usage: /cleanup [sessions|cache|logs|all]".to_string(),
    }
}

/// /reset - Reset session state
pub fn handle_reset(app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() || args == "session" {
        app.messages.clear();
        app.clear_tool_transcript();
        "Session reset. Messages cleared.".to_string()
    } else if args == "all" {
        app.messages.clear();
        app.clear_tool_transcript();
        "Full reset not yet implemented.".to_string()
    } else {
        "Usage: /reset [session|all]".to_string()
    }
}

/// /snippet - Save/load code snippets
pub fn handle_snippet(app: &mut TuiApp, args: &str) -> String {
    let args = args.trim();
    if args.is_empty() {
        return "Usage: /snippet [save <name>|load <name>|list]".to_string();
    }

    let mut parts = args.splitn(2, ' ');
    let action = parts.next().unwrap_or_default();
    let rest = parts.next().unwrap_or("").trim();

    match action {
        "save" => {
            if rest.is_empty() {
                return "Usage: /snippet save <name> [content]".to_string();
            }

            let mut save_parts = rest.splitn(2, ' ');
            let name = save_parts.next().unwrap_or_default().trim();
            let content = save_parts.next().map(str::trim).unwrap_or("");
            let Some(safe_name) = sanitize_snippet_name(name) else {
                return "Invalid snippet name. Use letters, digits, '-', '_' or '.'".to_string();
            };

            let content_to_save = if content.is_empty() {
                match app.messages.last() {
                    Some(msg) => msg.content.clone(),
                    None => {
                        return "No message available to save. Provide content explicitly."
                            .to_string();
                    }
                }
            } else {
                content.to_string()
            };

            let dir = snippet_dir();
            if let Err(e) = std::fs::create_dir_all(&dir) {
                return format!("Failed to create snippet directory: {}", e);
            }
            let path = dir.join(format!("{}.md", safe_name));
            match std::fs::write(&path, content_to_save) {
                Ok(_) => format!("Snippet '{}' saved to {}", safe_name, path.display()),
                Err(e) => format!("Failed to save snippet '{}': {}", safe_name, e),
            }
        }
        "load" => {
            if rest.is_empty() {
                return "Usage: /snippet load <name>".to_string();
            }
            let Some(safe_name) = sanitize_snippet_name(rest) else {
                return "Invalid snippet name. Use letters, digits, '-', '_' or '.'".to_string();
            };
            let path = snippet_dir().join(format!("{}.md", safe_name));
            match std::fs::read_to_string(&path) {
                Ok(content) => format!("Snippet '{}':\n{}", safe_name, content),
                Err(e) => format!("Failed to load snippet '{}': {}", safe_name, e),
            }
        }
        "list" => match list_snippets() {
            Ok(names) if names.is_empty() => "No snippets saved.".to_string(),
            Ok(names) => format!("Snippets:\n- {}", names.join("\n- ")),
            Err(e) => format!("Failed to list snippets: {}", e),
        },
        _ => "Usage: /snippet [save|load|list]".to_string(),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Bookmark, Tag, Search commands
// ═══════════════════════════════════════════════════════════════════════

/// /bookmark - Bookmark locations
pub async fn handle_bookmark(app: &mut TuiApp, args: &str) -> String {
    let args = args.trim();
    if args.is_empty() || args == "list" {
        return match load_bookmarks() {
            Ok(map) if map.is_empty() => "No bookmarks saved.".to_string(),
            Ok(map) => {
                let mut names: Vec<_> = map.keys().cloned().collect();
                names.sort();
                let mut lines = vec!["Bookmarks:".to_string()];
                for n in names {
                    if let Some(target) = map.get(&n) {
                        lines.push(format!("- {} -> {}", n, target));
                    }
                }
                lines.join("\n")
            }
            Err(e) => format!("Failed to load bookmarks: {}", e),
        };
    }

    let mut parts = args.splitn(2, ' ');
    let action = parts.next().unwrap_or_default();
    let rest = parts.next().unwrap_or("").trim();

    match action {
        "add" => {
            if rest.is_empty() {
                return "Usage: /bookmark add <name> [target]".to_string();
            }
            let mut add_parts = rest.splitn(2, ' ');
            let raw_name = add_parts.next().unwrap_or_default();
            let Some(name) = sanitize_note_name(raw_name) else {
                return "Invalid bookmark name. Use letters, digits, '-', '_' or '.'".to_string();
            };

            let target = add_parts
                .next()
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(std::string::ToString::to_string)
                .or_else(|| {
                    app.session_manager
                        .current_session_id()
                        .map(|id| format!("session:{}", id))
                });
            let Some(target) = target else {
                return "No active session; provide explicit target: /bookmark add <name> <target>"
                    .to_string();
            };

            let mut map = match load_bookmarks() {
                Ok(v) => v,
                Err(e) => return format!("Failed to load bookmarks: {}", e),
            };
            map.insert(name.clone(), target.clone());
            match save_bookmarks(&map) {
                Ok(_) => format!("Bookmark '{}' saved -> {}", name, target),
                Err(e) => format!("Failed to save bookmark '{}': {}", name, e),
            }
        }
        "go" => {
            if rest.is_empty() {
                return "Usage: /bookmark go <name>".to_string();
            }
            let Some(name) = sanitize_note_name(rest) else {
                return "Invalid bookmark name.".to_string();
            };
            let map = match load_bookmarks() {
                Ok(v) => v,
                Err(e) => return format!("Failed to load bookmarks: {}", e),
            };
            let Some(target) = map.get(&name) else {
                return format!("Bookmark '{}' not found.", name);
            };

            if let Some(session_id) = target.strip_prefix("session:") {
                return app.restore_session(session_id).await;
            }
            if target.starts_with("sess_") {
                return app.restore_session(target).await;
            }
            format!("Bookmark '{}' -> {}", name, target)
        }
        _ => "Usage: /bookmark [add <name> [target]|go <name>|list]".to_string(),
    }
}

/// /tag - Tag items
pub fn handle_tag(_app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /tag [add <item> <tag>|list <item>|find <tag>]".to_string();
    }

    let parts: Vec<&str> = args.split_whitespace().collect();
    match parts[0] {
        "add" => {
            if parts.len() < 3 {
                return "Usage: /tag add <item> <tag>".to_string();
            }
            let Some(item) = sanitize_note_name(parts[1]) else {
                return "Invalid item name. Use letters, digits, '-', '_' or '.'".to_string();
            };
            let Some(tag) = sanitize_note_name(parts[2]) else {
                return "Invalid tag name. Use letters, digits, '-', '_' or '.'".to_string();
            };
            let mut tags = match load_tags() {
                Ok(v) => v,
                Err(e) => return format!("Failed to load tags: {}", e),
            };
            let entry = tags.entry(item.clone()).or_default();
            if !entry.iter().any(|t| t == &tag) {
                entry.push(tag.clone());
                entry.sort();
            }
            match save_tags(&tags) {
                Ok(_) => format!("Added tag '{}' to '{}'.", tag, item),
                Err(e) => format!("Failed to save tags: {}", e),
            }
        }
        "list" => {
            if parts.len() < 2 {
                return "Usage: /tag list <item>".to_string();
            }
            let Some(item) = sanitize_note_name(parts[1]) else {
                return "Invalid item name.".to_string();
            };
            let tags = match load_tags() {
                Ok(v) => v,
                Err(e) => return format!("Failed to load tags: {}", e),
            };
            match tags.get(&item) {
                Some(v) if !v.is_empty() => format!("Tags for '{}': {}", item, v.join(", ")),
                _ => format!("No tags for '{}'.", item),
            }
        }
        "find" => {
            if parts.len() < 2 {
                return "Usage: /tag find <tag>".to_string();
            }
            let Some(tag) = sanitize_note_name(parts[1]) else {
                return "Invalid tag name.".to_string();
            };
            let tags = match load_tags() {
                Ok(v) => v,
                Err(e) => return format!("Failed to load tags: {}", e),
            };
            let mut items: Vec<String> = tags
                .iter()
                .filter(|(_, v)| v.iter().any(|t| t == &tag))
                .map(|(k, _)| k.clone())
                .collect();
            items.sort();
            if items.is_empty() {
                format!("No items found with tag '{}'.", tag)
            } else {
                format!("Items with tag '{}':\n- {}", tag, items.join("\n- "))
            }
        }
        _ => "Usage: /tag [add|list|find]".to_string(),
    }
}

/// /search - Search within session
pub fn handle_search_cmd(app: &TuiApp, args: &str) -> String {
    handle_search(app, args)
}

/// /filter - Filter messages
pub fn handle_filter(app: &mut TuiApp, args: &str) -> String {
    let args = args.trim();
    if args.is_empty() {
        return "Usage: /filter <user|assistant|tool|system|all> [query]".to_string();
    }

    let mut parts = args.splitn(2, ' ');
    let role = parts.next().unwrap_or_default();
    let query = parts.next().unwrap_or("").trim().to_ascii_lowercase();

    let role_filter = match role {
        "user" => Some(crate::state::MessageRole::User),
        "assistant" => Some(crate::state::MessageRole::Assistant),
        "tool" => Some(crate::state::MessageRole::Tool),
        "system" => Some(crate::state::MessageRole::System),
        "all" => None,
        _ => return "Usage: /filter <user|assistant|tool|system|all> [query]".to_string(),
    };

    let total = app.messages.len();
    let mut matched: Vec<(usize, &crate::state::MessageItem)> = app
        .messages
        .iter()
        .enumerate()
        .filter(|(_, m)| role_filter.is_none_or(|r| m.role == r))
        .filter(|(_, m)| query.is_empty() || m.content.to_ascii_lowercase().contains(&query))
        .collect();

    if matched.is_empty() {
        return "No messages matched this filter.".to_string();
    }

    const MAX_PREVIEW: usize = 20;
    if matched.len() > MAX_PREVIEW {
        matched = matched[matched.len() - MAX_PREVIEW..].to_vec();
    }

    let mut lines = vec![format!(
        "Matched {} / {} messages (showing last {}).",
        matched.len(),
        total,
        matched.len()
    )];
    for (idx, m) in matched {
        let preview: String = m.content.replace('\n', " ").chars().take(80).collect();
        lines.push(format!(
            "{}. [{}] {}",
            idx + 1,
            message_role_label(m.role),
            preview
        ));
    }
    lines.join("\n")
}

// ═══════════════════════════════════════════════════════════════════════
// Private helper functions (non-exported, used only within this module)
// ═══════════════════════════════════════════════════════════════════════

// All helper functions below (get_default_keybindings, count_test_passed,
// count_test_failed, cleanup_sessions, cleanup_cache, cleanup_logs,
// bookmarks_file, tags_file, load_bookmarks, save_bookmarks, load_tags,
// save_tags, priority_agent_home_dir) are provided via `use super::utils::*;`
// and no longer duplicated here.
