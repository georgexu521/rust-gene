// Session, skills, batch, and extended command handlers
//
// Extracted from mod.rs to reduce the main slash_handler file size.
// Each handler function takes `&mut TuiApp` + args and returns a String response.

use super::utils::*;

mod actions;
use crate::agent::agent::AgentConfig;
use crate::agent::roles::AgentRole;
use crate::agent::types::{AgentId, AgentMessage, AgentMessageType};
use crate::engine::checkpoint::{FileChangeRecord, FileChangeRoundSummary, RestoreResult};
use crate::tools::Tool;
use crate::tui::app::TuiApp;
pub use actions::*;
use tokio::process::Command;

// ═══════════════════════════════════════════════════════════════════════
// Section 1: Session Management
// ═══════════════════════════════════════════════════════════════════════

pub async fn handle_resume(app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        match app.session_manager.list_resumable_sessions(10) {
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
                        let msg_count = app.session_manager.message_count(&session.id).unwrap_or(0);
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
        match app.session_manager.resolve_resume_selection(args, 40) {
            Ok(Some(session)) => app.restore_session(&session.id).await,
            Ok(None) => {
                "No matching session found. Use /resume without arguments to see recent sessions."
                    .to_string()
            }
            Err(e) => format!("Failed to resolve session: {}", e),
        }
    }
}

pub async fn handle_rewind(app: &mut TuiApp, args: &str) -> String {
    let raw_session_id = match app.session_manager.current_session_id() {
        Some(id) => id.to_string(),
        None => return "No active session.".to_string(),
    };
    let checkpoint_session_id = raw_session_id.to_string();
    let trimmed = args.trim();

    let checkpoint_manager =
        crate::engine::checkpoint::get_checkpoint_manager(&checkpoint_session_id).await;
    let checkpoint_guard = checkpoint_manager.lock().await;
    let file_changes = checkpoint_guard.list_file_changes().to_vec();
    let file_change_rounds = checkpoint_guard.list_file_change_rounds();

    if trimmed.is_empty() {
        if !file_changes.is_empty() {
            return format_rewind_file_history(&file_changes, &file_change_rounds);
        }
        match app.session_manager.list_edits(&raw_session_id) {
            Ok(edits) => {
                if edits.is_empty() {
                    "No edits recorded in this session.".to_string()
                } else {
                    let mut lines = vec!["Recent edits (most recent first):".to_string()];
                    for (i, edit) in edits.iter().rev().enumerate().take(10) {
                        lines.push(format!(
                            "{}. [{}] {} - {}",
                            i + 1,
                            edit.tool_name,
                            edit.file_path,
                            edit.timestamp
                        ));
                    }
                    lines.push("\nUse /rewind <n> to rewind the last n edits, or /rewind <file_path> to rewind a specific file.".to_string());
                    lines.join("\n")
                }
            }
            Err(e) => format!("Failed to list edits: {}", e),
        }
    } else if let Ok(n) = trimmed.parse::<usize>() {
        if n == 0 {
            return "Usage: /rewind [last-file|<file_change_id>|<file_path>|1]".to_string();
        }
        if !file_changes.is_empty() {
            if n == 1 {
                return match checkpoint_guard.restore_latest_file_change().await {
                    Ok(result) => format_rewind_restore_result(result),
                    Err(err) => format!("Failed to rewind latest file change: {}", err),
                };
            }
            let Some(change) = file_changes.iter().rev().nth(n - 1) else {
                return format!(
                    "Only {} checkpoint-backed file change(s) are available.",
                    file_changes.len()
                );
            };
            return match checkpoint_guard.restore_file_change(&change.id).await {
                Ok(result) => format!(
                    "{}\n\nNote: /rewind {} restored the checkpoint before the {} most recent tracked file change. Use /checkpoints for exact IDs.",
                    format_rewind_restore_result(result),
                    n,
                    ordinal(n)
                ),
                Err(err) => format!("Failed to rewind file change {}: {}", change.id, err),
            };
        }
        let mut results = Vec::new();
        for _ in 0..n {
            match app.session_manager.rewind_last_edit(&raw_session_id) {
                Ok(msg) => results.push(msg),
                Err(e) => {
                    results.push(format!("Error: {}", e));
                    break;
                }
            }
        }
        results.join("\n")
    } else if matches!(trimmed, "last-round" | "latest-round") {
        let summary = checkpoint_guard.latest_file_change_round();
        match checkpoint_guard.restore_latest_tool_round().await {
            Ok(result) => format_rewind_round_restore_result(result, summary),
            Err(err) => format!(
                "Failed to rewind latest tool round: {}\nUse /checkpoints to list recent file changes.",
                err
            ),
        }
    } else if trimmed.starts_with("round_") {
        let summary = checkpoint_guard.file_change_round(trimmed);
        match checkpoint_guard.restore_tool_round(trimmed).await {
            Ok(result) => format_rewind_round_restore_result(result, summary),
            Err(err) => format!(
                "Failed to rewind tool round {}: {}\nUse /checkpoints to list recent file changes.",
                trimmed, err
            ),
        }
    } else if matches!(trimmed, "last-file" | "latest-file") {
        match checkpoint_guard.restore_latest_file_change().await {
            Ok(result) => format_rewind_restore_result(result),
            Err(err) => format!(
                "Failed to rewind latest file change: {}\nUse /checkpoints to list recent file changes.",
                err
            ),
        }
    } else if trimmed.starts_with("fc_") {
        match checkpoint_guard.restore_file_change(trimmed).await {
            Ok(result) => format_rewind_restore_result(result),
            Err(err) => format!(
                "Failed to rewind file change {}: {}\nUse /checkpoints to list recent file changes.",
                trimmed, err
            ),
        }
    } else if let Some(change) = latest_file_change_for_path(&file_changes, trimmed) {
        match checkpoint_guard.restore_file_change(&change.id).await {
            Ok(result) => format!(
                "{}\n\nRestored latest tracked change for path: {}",
                format_rewind_restore_result(result),
                trimmed
            ),
            Err(err) => format!("Failed to rewind file {}: {}", trimmed, err),
        }
    } else {
        match app.session_manager.rewind_file(&raw_session_id, trimmed) {
            Ok(msg) => msg,
            Err(e) => format!("Failed to rewind file: {}", e),
        }
    }
}

pub async fn handle_diff(app: &mut TuiApp, args: &str) -> String {
    let trimmed = args.trim();
    if let Some((title, content)) = checkpoint_diff_for_target(app, trimmed).await {
        app.diff_title = title;
        app.diff_content = content;
        app.diff_scroll_offset = 0;
        app.mode = crate::tui::app::AppMode::DiffViewer;
        return String::new();
    }

    let tool = crate::tools::GitTool;
    let range = if trimmed.is_empty() {
        "HEAD~3..HEAD".to_string()
    } else {
        trimmed.to_string()
    };
    let params = serde_json::json!({ "action": "diff", "range": range });
    let result = tool.execute(params, app.build_tool_context().await).await;
    if result.success {
        app.diff_title = if trimmed.is_empty() {
            "Recent changes (last 3 commits)".to_string()
        } else {
            format!("Diff: {}", trimmed)
        };
        app.diff_content = result.content;
    } else {
        app.diff_title = "Error".to_string();
        app.diff_content = result.error.unwrap_or_else(|| "Unknown error".to_string());
    }
    app.diff_scroll_offset = 0;
    app.mode = crate::tui::app::AppMode::DiffViewer;
    String::new()
}

async fn checkpoint_diff_for_target(app: &TuiApp, target: &str) -> Option<(String, String)> {
    if looks_like_git_diff_target(target) {
        return None;
    }
    let raw_session_id = app.session_manager.current_session_id()?;
    let checkpoint_session_id = raw_session_id.to_string();
    let checkpoint_manager =
        crate::engine::checkpoint::get_checkpoint_manager(&checkpoint_session_id).await;
    let checkpoint_guard = checkpoint_manager.lock().await;
    let file_changes = checkpoint_guard.list_file_changes();
    let file_change_rounds = checkpoint_guard.list_file_change_rounds();
    if file_changes.is_empty() {
        return None;
    }

    if target == "history" {
        return Some((
            "File change history".to_string(),
            format_rewind_file_history(file_changes, &file_change_rounds),
        ));
    }

    if target.is_empty() || matches!(target, "last-round" | "latest-round") {
        if let Some(summary) = file_change_rounds.last() {
            return Some(format_tool_round_diff(summary));
        }
    } else if target.starts_with("round_") {
        if let Some(summary) = file_change_rounds
            .iter()
            .find(|summary| summary.tool_round_id.as_deref() == Some(target))
        {
            return Some(format_tool_round_diff(summary));
        }
    }

    let change = if target.is_empty() || matches!(target, "last-file" | "latest-file") {
        file_changes.last()
    } else if target.starts_with("fc_") {
        file_changes.iter().find(|change| change.id == target)
    } else {
        latest_file_change_for_path(file_changes, target)
    }?;
    let diff = change
        .diff
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            format!(
                "No stored diff for file change {}.\nPath: {}\nTool: {}\nCheckpoint: {}",
                change.id, change.path, change.tool_name, change.checkpoint_id
            )
        });
    Some((
        format!("File change diff: {} {}", change.id, change.path),
        diff,
    ))
}

fn looks_like_git_diff_target(target: &str) -> bool {
    target.contains("..")
        || target.starts_with("HEAD")
        || target.starts_with('@')
        || target.starts_with('-')
}

fn format_rewind_file_history(
    file_changes: &[FileChangeRecord],
    file_change_rounds: &[FileChangeRoundSummary],
) -> String {
    let mut lines = vec!["Recent checkpoint-backed file changes:".to_string()];
    if !file_change_rounds.is_empty() {
        lines.push("Recent tool rounds:".to_string());
        for (idx, summary) in file_change_rounds.iter().rev().take(10).enumerate() {
            let round = summary
                .tool_round_id
                .as_deref()
                .unwrap_or("<single change>");
            lines.push(format!(
                "{}. {} | {} change(s), {} file(s), {} bytes",
                idx + 1,
                round,
                summary.change_count,
                summary.paths.len(),
                summary.total_bytes_written
            ));
        }
        lines.push(String::new());
        lines.push("Recent file changes:".to_string());
    }
    for (idx, change) in file_changes.iter().rev().enumerate().take(10) {
        let before = change
            .before_hash
            .as_deref()
            .map(short_hash)
            .unwrap_or("new");
        let after = change
            .after_hash
            .as_deref()
            .map(short_hash)
            .unwrap_or("unknown");
        let round = change
            .tool_round_id
            .as_deref()
            .map(|round| format!(" | round {}", round))
            .unwrap_or_default();
        lines.push(format!(
            "{}. {} [{}] {} bytes {} -> {} | {}{}",
            idx + 1,
            change.id,
            change.tool_name,
            change.bytes_written,
            before,
            after,
            change.path,
            round
        ));
    }
    lines.push(
        "\nUse /rewind 1, /rewind last-file, /rewind last-round, /rewind <file_change_id>, /rewind <tool_round_id>, or /rewind <file_path>."
            .to_string(),
    );
    lines.join("\n")
}

fn format_tool_round_diff(summary: &FileChangeRoundSummary) -> (String, String) {
    let title = format!(
        "Tool round diff: {}",
        summary
            .tool_round_id
            .as_deref()
            .unwrap_or("<single change>")
    );
    let content = summary
        .combined_diff
        .clone()
        .filter(|diff| !diff.trim().is_empty())
        .unwrap_or_else(|| {
            format!(
                "No stored diff for tool round {}.\nChanges: {}\nFiles: {}",
                summary
                    .tool_round_id
                    .as_deref()
                    .unwrap_or("<single change>"),
                summary.change_count,
                summary.paths.join(", ")
            )
        });
    (title, content)
}

fn latest_file_change_for_path<'a>(
    file_changes: &'a [FileChangeRecord],
    path: &str,
) -> Option<&'a FileChangeRecord> {
    file_changes
        .iter()
        .rev()
        .find(|change| change.path == path || change.path.ends_with(path))
}

fn format_rewind_restore_result(result: RestoreResult) -> String {
    let mut lines = vec![format!(
        "Rewound file change using checkpoint: {}",
        result.checkpoint_id
    )];
    if !result.restored_files.is_empty() {
        lines.push(format!("Restored {} file(s):", result.restored_files.len()));
        lines.extend(
            result
                .restored_files
                .iter()
                .map(|path| format!("  {}", path)),
        );
    }
    if !result.removed_files.is_empty() {
        lines.push(format!(
            "Removed {} file(s) that did not exist before the change:",
            result.removed_files.len()
        ));
        lines.extend(
            result
                .removed_files
                .iter()
                .map(|path| format!("  {}", path)),
        );
    }
    if !result.failed_files.is_empty() {
        lines.push(format!(
            "Failed to restore {} file(s):",
            result.failed_files.len()
        ));
        lines.extend(
            result
                .failed_files
                .iter()
                .map(|(path, err)| format!("  {}: {}", path, err)),
        );
    }
    lines.join("\n")
}

fn format_rewind_round_restore_result(
    result: crate::engine::checkpoint::ToolRoundRestoreResult,
    summary: Option<FileChangeRoundSummary>,
) -> String {
    let mut lines = vec![format!(
        "Rewound {} file change(s) from tool round.",
        result.restored_changes.len()
    )];
    if let Some(round_id) = result.tool_round_id.as_deref() {
        lines.push(format!("Tool round: {}", round_id));
    }
    if let Some(summary) = summary.as_ref() {
        lines.push(format!(
            "Round summary: {} file(s), {} bytes.",
            summary.paths.len(),
            summary.total_bytes_written
        ));
    }
    for restore in result.results {
        lines.push(format_rewind_restore_result(restore));
    }
    lines.join("\n")
}

fn ordinal(n: usize) -> String {
    let suffix = match n % 100 {
        11..=13 => "th",
        _ => match n % 10 {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        },
    };
    format!("{n}{suffix}")
}

pub fn handle_sessions(app: &TuiApp) -> String {
    match app.session_manager.list_sessions(10) {
        Ok(sessions) => {
            if sessions.is_empty() {
                "No saved sessions. Start chatting to create one!".to_string()
            } else {
                let mut lines = vec!["Recent sessions:".to_string()];
                for (i, session) in sessions.iter().enumerate() {
                    let title = if session.title.is_empty() {
                        "(untitled)"
                    } else {
                        &session.title
                    };
                    let msg_count = app.session_manager.message_count(&session.id).unwrap_or(0);
                    lines.push(format!(
                        "{}. [{}] {} ({} msgs) - {}",
                        i + 1,
                        &session.id[..8.min(session.id.len())],
                        title,
                        msg_count,
                        session.updated_at
                    ));
                }
                lines.push("\nUse /resume <number|id|search> to restore a session.".to_string());
                lines.join("\n")
            }
        }
        Err(e) => format!("Failed to list sessions: {}", e),
    }
}

pub async fn handle_session(app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        let current = app.session_manager.current_session_id().unwrap_or("none");
        let title = app.session_manager.current_session_title();
        let msg_count = app
            .session_manager
            .current_session_id()
            .and_then(|id| app.session_manager.message_count(id).ok())
            .unwrap_or(0);
        format!(
            "Current session: {}\nTitle: {}\nMessages: {}",
            current, title, msg_count
        )
    } else if let Ok(index) = args.parse::<usize>() {
        match app.session_manager.list_sessions(20) {
            Ok(sessions) => {
                if index == 0 || index > sessions.len() {
                    "Invalid session number. Use /sessions to see available sessions.".to_string()
                } else {
                    let session = &sessions[index - 1];
                    app.restore_session(&session.id).await
                }
            }
            Err(e) => format!("Failed to list sessions: {}", e),
        }
    } else {
        app.restore_session(args).await
    }
}

pub async fn handle_new(app: &mut TuiApp) -> String {
    if let Some(ref engine) = app.streaming_engine {
        engine
            .flush_memory_for_current_history(crate::memory::MemoryFlushReason::ResumeSwitch)
            .await;
    }
    let model = app
        .streaming_engine
        .as_ref()
        .map(|_| "kimi-k2.5")
        .unwrap_or("unknown");
    match app.session_manager.start_session("New Session", model) {
        Ok(id) => {
            use crate::state::{MessageItem, MessageRole};
            app.messages.clear();
            app.clear_tool_transcript();
            if let Some(ref engine) = app.streaming_engine {
                engine.set_session_id(id.clone());
                engine.set_history(Vec::new()).await;
            }
            let welcome = MessageItem {
                id: "welcome".to_string(),
                role: MessageRole::System,
                content: "Started a new session. Previous messages cleared from view but saved to database.".to_string(),
                timestamp: std::time::SystemTime::now(),
                metadata: Default::default(),
            };
            app.messages.push(welcome);
            format!("New session started: {}", id)
        }
        Err(e) => format!("Failed to start new session: {}", e),
    }
}

pub fn handle_export(app: &TuiApp) -> String {
    if let Some(id) = app.session_manager.current_session_id() {
        match app.session_manager.export_session(id) {
            Ok(json) => {
                let filename = format!("session_{}.json", &id[..8.min(id.len())]);
                let path = dirs::home_dir()
                    .unwrap_or_else(|| std::path::PathBuf::from("."))
                    .join(".priority-agent")
                    .join(&filename);
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent).ok();
                }
                match std::fs::write(&path, &json) {
                    Ok(_) => format!("Session exported to: {}", path.display()),
                    Err(e) => format!("Failed to write export file: {}", e),
                }
            }
            Err(e) => format!("Failed to export session: {}", e),
        }
    } else {
        "No active session to export.".to_string()
    }
}

pub fn handle_search(app: &TuiApp, args: &str) -> String {
    if args.is_empty() {
        "Usage: /search <query> - Search through all session messages".to_string()
    } else {
        match app.session_manager.search_sessions(args, 10) {
            Ok(sessions) => {
                if sessions.is_empty() {
                    format!("No sessions found matching '{}'", args)
                } else {
                    let mut lines = vec![format!("Sessions matching '{}':", args)];
                    for (i, session) in sessions.iter().enumerate() {
                        lines.push(format!(
                            "{}. [{}] {}",
                            i + 1,
                            &session.id[..8.min(session.id.len())],
                            session.title
                        ));
                    }
                    lines.join("\n")
                }
            }
            Err(e) => format!("Search failed: {}", e),
        }
    }
}

pub fn handle_stats(app: &TuiApp) -> String {
    match app.session_manager.stats() {
        Ok(stats) => {
            format!(
                "Session Statistics:\n\
                 Total sessions: {}\n\
                 Total messages: {}\n\
                 Total input tokens: {}\n\
                 Total output tokens: {}",
                stats.session_count,
                stats.message_count,
                stats.total_input_tokens,
                stats.total_output_tokens
            )
        }
        Err(e) => format!("Failed to get stats: {}", e),
    }
}

pub async fn handle_batch(_app: &mut TuiApp, args: &str) -> String {
    if args.trim().is_empty() {
        return "Usage: /batch <task description> [--files <patterns>...]\n\
                Example: /batch Rename all User references to Account --files src/**/*.rs\n\
                Set PRIORITY_AGENT_BATCH_REFACTOR=1 to enable."
            .to_string();
    }

    // 解析参数
    let parts: Vec<&str> = args.split(" --files ").collect();
    let description = parts[0].trim();
    let files = if parts.len() > 1 {
        parts[1].split_whitespace().map(|s| s.to_string()).collect()
    } else {
        Vec::new()
    };

    let working_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let batch = crate::engine::batch_refactor::BatchRefactor::new(working_dir);

    if !batch.is_enabled() {
        return "Batch refactor is not enabled.\n\
                Set environment variable: PRIORITY_AGENT_BATCH_REFACTOR=1\n\
                Optional: PRIORITY_AGENT_BATCH_MAX_PARALLEL=10"
            .to_string();
    }

    // 如果没有指定文件，尝试自动发现
    let files = if files.is_empty() {
        let mut scanner = crate::tools::project_tool::ProjectScanner::new();
        scanner.scan(std::path::Path::new("."));
        let files: Vec<String> = scanner.files().iter().take(50).cloned().collect();
        if files.is_empty() {
            return "No files specified and auto-discovery found none.".to_string();
        }
        files
    } else {
        files
    };

    match batch.execute(description, files).await {
        Ok(result) => {
            let mut lines = vec![
                format!("## Batch Refactor Result: {:?}", result.status),
                format!(
                    "Units: {} | Duration: {}ms",
                    result.units.len(),
                    result.total_duration_ms
                ),
                String::new(),
            ];

            let success_count = result.units.iter().filter(|u| u.success).count();
            let fail_count = result.units.len() - success_count;
            lines.push(format!(
                "✅ Success: {} | ❌ Failed: {}",
                success_count, fail_count
            ));

            for unit in &result.units {
                let icon = if unit.success { "✅" } else { "❌" };
                lines.push(format!(
                    "{} {} ({}ms)",
                    icon, unit.unit_id, unit.duration_ms
                ));
                if !unit.output.is_empty() {
                    for line in unit.output.lines().take(5) {
                        lines.push(format!("   {}", line));
                    }
                }
            }

            lines.join("\n")
        }
        Err(e) => format!("Batch refactor failed: {}", e),
    }
}

pub async fn handle_checkpoints(app: &TuiApp) -> String {
    let session_id = match app.session_manager.current_session_id() {
        Some(id) => id.to_string(),
        None => return "No active session. Start a conversation first.".to_string(),
    };

    let mgr = crate::engine::checkpoint::get_checkpoint_manager(&session_id).await;
    let cp = mgr.lock().await;
    let checkpoints = cp.list_checkpoints();
    let stats = cp.stats();

    if checkpoints.is_empty() {
        return "No checkpoints for this session yet.\nCheckpoints are created automatically before file edits.".to_string();
    }

    let mut lines = vec![
        format!(
            "Checkpoints for session (total: {}, files tracked: {}, file changes: {})",
            stats.total_checkpoints, stats.total_files_tracked, stats.total_file_changes
        ),
        String::new(),
    ];

    for c in checkpoints.iter().rev().take(20) {
        let files: Vec<String> = c
            .file_backups
            .iter()
            .map(|f| {
                format!(
                    "{} {}",
                    if f.existed_before { "📝" } else { "🆕" },
                    f.original_path
                )
            })
            .collect();
        lines.push(format!(
            "[{}] {} ({} files)\n  tool: {} | {}",
            c.sequence,
            c.id.split('_').next_back().unwrap_or(&c.id),
            c.file_backups.len(),
            c.tool_name,
            files.join(", ")
        ));
    }

    if checkpoints.len() > 20 {
        lines.push(format!(
            "\n... and {} more checkpoints",
            checkpoints.len() - 20
        ));
    }

    let file_changes = cp.list_file_changes();
    if !file_changes.is_empty() {
        let file_change_rounds = cp.list_file_change_rounds();
        if !file_change_rounds.is_empty() {
            lines.push(String::new());
            lines.push("Recent tool rounds:".to_string());
            for summary in file_change_rounds.iter().rev().take(10) {
                let round = summary
                    .tool_round_id
                    .as_deref()
                    .unwrap_or("<single change>");
                lines.push(format!(
                    "{} | {} change(s), {} file(s), {} bytes | {}",
                    round,
                    summary.change_count,
                    summary.paths.len(),
                    summary.total_bytes_written,
                    summary.paths.join(", ")
                ));
            }
        }
        lines.push(String::new());
        lines.push("Recent file changes:".to_string());
        for change in file_changes.iter().rev().take(10) {
            let before = change
                .before_hash
                .as_deref()
                .map(short_hash)
                .unwrap_or("new");
            let after = change
                .after_hash
                .as_deref()
                .map(short_hash)
                .unwrap_or("unknown");
            let round = change
                .tool_round_id
                .as_deref()
                .map(|round| format!(" | round {}", round))
                .unwrap_or_default();
            lines.push(format!(
                "{} [{}] {} bytes {} -> {} | {}{}",
                change.id,
                change.tool_name,
                change.bytes_written,
                before,
                after,
                change.path,
                round
            ));
        }
        lines.push(
            "\nUse /rollback last-file --yes, /rollback last-round --yes, /rollback <file_change_id> --yes, or /rollback <tool_round_id> --yes."
                .to_string(),
        );
    }

    lines.join("\n")
}

fn short_hash(hash: &str) -> &str {
    &hash[..hash.len().min(8)]
}

pub async fn handle_restore(app: &mut TuiApp, args: &str) -> String {
    if args.trim().is_empty() {
        return "Usage: /restore <checkpoint_id>\nUse /checkpoints to list available checkpoints."
            .to_string();
    }

    let session_id = match app.session_manager.current_session_id() {
        Some(id) => id.to_string(),
        None => return "No active session.".to_string(),
    };

    let checkpoint_id = args.trim();
    let mgr = crate::engine::checkpoint::get_checkpoint_manager(&session_id).await;
    let cp = mgr.lock().await;

    match cp.restore_checkpoint(checkpoint_id).await {
        Ok(result) => {
            let mut lines = vec![format!("Restored checkpoint: {}", result.checkpoint_id)];
            if !result.restored_files.is_empty() {
                lines.push(format!(
                    "\nRestored {} file(s):",
                    result.restored_files.len()
                ));
                for f in &result.restored_files {
                    lines.push(format!("  ✅ {}", f));
                }
            }
            if !result.removed_files.is_empty() {
                lines.push(format!(
                    "\nRemoved {} file(s) (did not exist before checkpoint):",
                    result.removed_files.len()
                ));
                for f in &result.removed_files {
                    lines.push(format!("  🗑️  {}", f));
                }
            }
            if !result.failed_files.is_empty() {
                lines.push(format!(
                    "\nFailed to restore {} file(s):",
                    result.failed_files.len()
                ));
                for (f, e) in &result.failed_files {
                    lines.push(format!("  ❌ {} — {}", f, e));
                }
            }
            lines.join("\n")
        }
        Err(e) => format!("Failed to restore checkpoint: {}", e),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Section 3: Skills
// ═══════════════════════════════════════════════════════════════════════

pub async fn handle_commit(app: &mut TuiApp) -> String {
    match app.bundled_skills.get("commit") {
        Some(skill) => {
            let tool = crate::tools::GitTool;
            let params = serde_json::json!({ "action": "diff", "cached": true });
            let result = tool.execute(params, app.build_tool_context().await).await;
            let diff = if result.success {
                result.content
            } else {
                result
                    .error
                    .unwrap_or_else(|| "No staged changes or unable to read diff.".to_string())
            };
            let prompt = format!(
                "{}\n\nStaged changes:\n```diff\n{}\n```",
                skill.content, diff
            );
            app.send_message(prompt).await;
            String::new()
        }
        None => "Skill 'commit' not found.".to_string(),
    }
}

pub async fn handle_commit_push_pr(app: &mut TuiApp, args: &str) -> String {
    let tool = crate::tools::GitTool;
    let ctx = app.build_tool_context().await;

    // 收集 git 上下文
    let status_result = tool
        .execute(serde_json::json!({ "action": "status" }), ctx.clone())
        .await;
    let status = if status_result.success {
        status_result.content
    } else {
        "Unable to get git status.".to_string()
    };

    let diff_result = tool
        .execute(serde_json::json!({ "action": "diff" }), ctx.clone())
        .await;
    let diff = if diff_result.success {
        diff_result.content
    } else {
        "Unable to get git diff.".to_string()
    };

    // 获取当前分支
    let branch = match Command::new("git")
        .args(["branch", "--show-current"])
        .output()
        .await
    {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => "unknown".to_string(),
    };

    let log_result = tool
        .execute(serde_json::json!({ "action": "log", "n": 5 }), ctx.clone())
        .await;
    let log = if log_result.success {
        log_result.content
    } else {
        "Unable to get git log.".to_string()
    };

    let user_desc = if args.trim().is_empty() {
        "No specific description provided. Infer the purpose from the changes.".to_string()
    } else {
        args.trim().to_string()
    };

    let prompt = format!(
        "You are a git workflow assistant. Complete the following steps to commit changes and create a PR.\n\
         \n\
         ## Steps\n\
         1. Use the `git` tool with action='add' to stage all changes (path='' or paths=[]).\n\
         2. Use the `git` tool with action='commit' to create a commit.\n\
            - Generate a concise conventional commit message from the changes.\n\
            - Format: `<type>(<scope>): <description>` (e.g., `feat(auth): add login flow`)\n\
            - Keep subject under 72 characters, use imperative mood.\n\
         3. Check current branch. If on main/master, create a new feature branch first using git action='checkout' with create_branch=true.\n\
         4. Use the `git` tool with action='push' to push the branch to origin.\n\
         5. Use the `github` tool with action='pr_create' to create a Pull Request.\n\
            - Use the commit message as PR title.\n\
            - Provide a brief PR body summarizing the changes.\n\
         \n\
         ## Safety Rules\n\
         - NEVER use force push.\n\
         - NEVER use `git commit --amend`.\n\
         - Do not commit secret files (.env, credentials.json, id_rsa, etc.).\n\
         - If there are no changes to commit, report that and stop.\n\
         \n\
         ## Context\n\
         Current branch: {}\n\
         User description: {}\n\
         \n\
         Git status:\n\
         ```\n{}\n```\n\
         \n\
         Git diff:\n\
         ```diff\n{}\n```\n\
         \n\
         Recent commits:\n\
         ```\n{}\n```",
        branch, user_desc, status, diff, log
    );

    app.send_message(prompt).await;
    String::new()
}

pub async fn handle_review_pr(app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /review-pr <number>".to_string();
    }
    match app.bundled_skills.get("review_pr") {
        Some(skill) => {
            let pr_number = args.trim();
            if !pr_number.chars().all(|c| c.is_ascii_digit()) {
                return "Invalid PR number. Must be numeric.".to_string();
            }
            let output = tokio::process::Command::new("gh")
                .args(["pr", "diff", pr_number])
                .output()
                .await;
            let diff = match output {
                Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).to_string(),
                Ok(out) => format!(
                    "Failed to fetch PR diff: {}",
                    String::from_utf8_lossy(&out.stderr)
                ),
                Err(e) => format!("Failed to run gh: {}", e),
            };
            let prompt = format!(
                "{}\n\nPR #{} diff:\n```diff\n{}\n```",
                skill.content, pr_number, diff
            );
            app.send_message(prompt).await;
            String::new()
        }
        None => "Skill 'review_pr' not found.".to_string(),
    }
}

pub async fn handle_review(app: &mut TuiApp) -> String {
    match app.bundled_skills.get("review") {
        Some(skill) => {
            let started = std::time::Instant::now();
            let tool = crate::tools::GitTool;
            let params = serde_json::json!({ "action": "diff" });
            let result = tool.execute(params, app.build_tool_context().await).await;
            let error_for_audit = result.error.clone();
            if let Some(ref engine) = app.streaming_engine {
                let mut tracker = engine.cost_tracker().lock().await;
                tracker.record_tool_execution(
                    "slash_review",
                    result.success,
                    started.elapsed().as_millis() as u64,
                    error_for_audit.as_deref(),
                );
            }
            let diff = if result.success {
                result.content
            } else {
                result
                    .error
                    .unwrap_or_else(|| "No uncommitted changes or unable to read diff.".to_string())
            };
            let prompt = format!(
                "{}\n\nLocal changes diff:\n```diff\n{}\n```",
                skill.content, diff
            );
            app.send_message(prompt).await;
            String::new()
        }
        None => "Skill 'review' not found.".to_string(),
    }
}

pub async fn handle_security_review(app: &mut TuiApp) -> String {
    match app.bundled_skills.get("security_review") {
        Some(skill) => {
            let started = std::time::Instant::now();
            let tool = crate::tools::GitTool;
            let params = serde_json::json!({ "action": "diff" });
            let result = tool.execute(params, app.build_tool_context().await).await;
            let error_for_audit = result.error.clone();
            if let Some(ref engine) = app.streaming_engine {
                let mut tracker = engine.cost_tracker().lock().await;
                tracker.record_tool_execution(
                    "slash_security_review",
                    result.success,
                    started.elapsed().as_millis() as u64,
                    error_for_audit.as_deref(),
                );
            }
            let diff = if result.success {
                result.content
            } else {
                result
                    .error
                    .unwrap_or_else(|| "No uncommitted changes or unable to read diff.".to_string())
            };
            let prompt = format!(
                "{}\n\nLocal changes diff:\n```diff\n{}\n```",
                skill.content, diff
            );
            app.send_message(prompt).await;
            String::new()
        }
        None => "Skill 'security_review' not found.".to_string(),
    }
}

pub async fn handle_explain(app: &mut TuiApp, args: &str) -> String {
    match app.bundled_skills.get("explain") {
        Some(skill) => {
            let context = if args.is_empty() {
                "No specific target provided. Please explain the current codebase context or answer generally.".to_string()
            } else {
                let working_dir =
                    std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
                match crate::tools::file_tool::resolve_path(args.trim(), &working_dir) {
                    Ok(path) => {
                        if path.exists() && path.is_file() {
                            match tokio::fs::read_to_string(&path).await {
                                Ok(content) => content,
                                Err(e) => format!("Failed to read file: {}", e),
                            }
                        } else {
                            format!("Path '{}' does not exist or is not a file.", args.trim())
                        }
                    }
                    Err(e) => format!("Invalid path '{}': {}", args.trim(), e),
                }
            };
            let prompt = format!("{}\n\n{}", skill.content, context);
            app.send_message(prompt).await;
            String::new()
        }
        None => "Skill 'explain' not found.".to_string(),
    }
}

pub async fn handle_fix(app: &mut TuiApp) -> String {
    match app.bundled_skills.get("fix") {
        Some(skill) => {
            let tool = crate::tools::GitTool;
            let params = serde_json::json!({ "action": "diff" });
            let result = tool.execute(params, app.build_tool_context().await).await;
            let diff = if result.success {
                result.content
            } else {
                result
                    .error
                    .unwrap_or_else(|| "No uncommitted changes or unable to read diff.".to_string())
            };
            let prompt = format!(
                "{}\n\nCurrent changes:\n```diff\n{}\n```",
                skill.content, diff
            );
            app.send_message(prompt).await;
            String::new()
        }
        None => "Skill 'fix' not found.".to_string(),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Section 6: Skill Commands
// ═══════════════════════════════════════════════════════════════════════

pub async fn handle_simplify(app: &mut TuiApp, _args: &str) -> String {
    match app.bundled_skills.get("simplify") {
        Some(skill) => {
            let started = std::time::Instant::now();
            let tool = crate::tools::GitTool;
            let params = serde_json::json!({ "action": "diff" });
            let result = tool.execute(params, app.build_tool_context().await).await;
            let error_for_audit = result.error.clone();
            if let Some(ref engine) = app.streaming_engine {
                let mut tracker = engine.cost_tracker().lock().await;
                tracker.record_tool_execution(
                    "slash_simplify",
                    result.success,
                    started.elapsed().as_millis() as u64,
                    error_for_audit.as_deref(),
                );
            }
            let diff = if result.success {
                result.content
            } else {
                result
                    .error
                    .unwrap_or_else(|| "No uncommitted changes or unable to read diff.".to_string())
            };

            // Launch 3 parallel sub-agents: Reuse, Quality, Efficiency
            let agent_manager = match app
                .streaming_engine
                .as_ref()
                .and_then(|e| e.agent_manager())
            {
                Some(am) => am,
                None => {
                    return "Agent manager not available. Cannot run simplify.".to_string();
                }
            };
            let _working_dir = if let Some(ref wt) = app.worktree_manager {
                wt.current_worktree().await.unwrap_or_else(|| {
                    std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
                })
            } else {
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
            };

            let reuse_prompt = format!(
                "{}\n\n## Focus: Code Reuse Review\n\nAnalyze this diff for:\n- Repeated code that should be extracted\n- Copy-paste patterns\n- Missing abstractions\n- DRY violations\n\nDiff:\n```diff\n{}\n```",
                skill.content, diff
            );
            let quality_prompt = format!(
                "{}\n\n## Focus: Code Quality Review\n\nAnalyze this diff for:\n- Redundant state or parameter sprawl\n- Leaky abstractions\n- Error handling issues\n- Missing validation\n- Poor naming or documentation\n\nDiff:\n```diff\n{}\n```",
                skill.content, diff
            );
            let efficiency_prompt = format!(
                "{}\n\n## Focus: Efficiency Review\n\nAnalyze this diff for:\n- Unnecessary computations\n- Missed concurrency opportunities\n- Hot-path bloat\n- Memory allocation issues\n- Inefficient data structures\n\nDiff:\n```diff\n{}\n```",
                skill.content, diff
            );

            let spawn_agent = |description: String, prompt: String, role: AgentRole| {
                let am = agent_manager.clone();
                async move {
                    let config = AgentConfig::new(format!("simplify: {}", description))
                        .with_description(&description)
                        .with_system_prompt(&prompt)
                        .with_max_turns(10)
                        .with_max_cost_usd(0.05)
                        .with_role(role);
                    let agent_id = am.spawn(config, None).await?;
                    let task_msg = AgentMessage::new(
                        AgentId::new(),
                        agent_id.clone(),
                        prompt,
                        AgentMessageType::Task,
                    );
                    am.send_message(&agent_id, task_msg).await?;
                    am.wait_for_result(&agent_id, 120).await
                }
            };

            let (reuse_result, quality_result, efficiency_result) = tokio::join!(
                spawn_agent("code-reuse".to_string(), reuse_prompt, AgentRole::Default),
                spawn_agent(
                    "code-quality".to_string(),
                    quality_prompt,
                    AgentRole::Default
                ),
                spawn_agent(
                    "efficiency".to_string(),
                    efficiency_prompt,
                    AgentRole::Default
                ),
            );

            let mut report = "# Simplify Report\n\n".to_string();
            report += "Running 3 parallel review agents: Reuse / Quality / Efficiency...\n\n";

            match reuse_result {
                Ok(r) => {
                    let status = if r.status == crate::agent::types::AgentStatus::Completed {
                        "✓"
                    } else {
                        "✗"
                    };
                    report += &format!("## Code Reuse Review {}:\n{}\n\n", status, r.content);
                }
                Err(e) => {
                    report += &format!("## Code Reuse Review ✗:\nError: {}\n\n", e);
                }
            }
            match quality_result {
                Ok(r) => {
                    let status = if r.status == crate::agent::types::AgentStatus::Completed {
                        "✓"
                    } else {
                        "✗"
                    };
                    report += &format!("## Code Quality Review {}:\n{}\n\n", status, r.content);
                }
                Err(e) => {
                    report += &format!("## Code Quality Review ✗:\nError: {}\n\n", e);
                }
            }
            match efficiency_result {
                Ok(r) => {
                    let status = if r.status == crate::agent::types::AgentStatus::Completed {
                        "✓"
                    } else {
                        "✗"
                    };
                    report += &format!("## Efficiency Review {}:\n{}\n\n", status, r.content);
                }
                Err(e) => {
                    report += &format!("## Efficiency Review ✗:\nError: {}\n\n", e);
                }
            }

            app.add_system_message(report.clone());
            app.send_message(format!(
                "I've run a comprehensive simplify analysis on your changes. Here's the summary:\n\n{}\n\nSending to main agent for detailed recommendations...",
                report
            )).await;
            String::new()
        }
        None => "Skill 'simplify' not found.".to_string(),
    }
}

/// /karpathy - Apply Karpathy-style coding guidelines to a task
pub async fn handle_karpathy(app: &mut TuiApp, args: &str) -> String {
    let task = args.trim();
    if task.is_empty() {
        match app.skill_runtime.get("karpathy-guidelines") {
            Some(skill) => {
                return format!(
                    "Karpathy Guidelines\n\n{}\n\nUsage:\n  /karpathy <coding task>\n\nThis applies the bundled skill to a concrete coding, review, refactor, or debugging task.",
                    skill.meta.description
                );
            }
            None => return "Skill 'karpathy-guidelines' not found.".to_string(),
        }
    }
    match app.skill_runtime.invocation("karpathy-guidelines", task) {
        Some(invocation) => {
            app.send_message(invocation.prompt).await;
            String::new()
        }
        None => "Skill 'karpathy-guidelines' not found or not user-invocable.".to_string(),
    }
}

pub async fn handle_verify(app: &mut TuiApp) -> String {
    match app.bundled_skills.get("verify") {
        Some(_skill) => {
            let started = std::time::Instant::now();
            let tool = crate::tools::BashTool;

            // Detect project type
            let (test_cmd, project_type) = if std::path::Path::new("Cargo.toml").exists() {
                ("cargo test 2>&1", "Rust")
            } else if std::path::Path::new("package.json").exists() {
                ("npm test 2>&1", "Node.js")
            } else if std::path::Path::new("pyproject.toml").exists()
                || std::path::Path::new("setup.py").exists()
            {
                ("python -m pytest 2>&1", "Python")
            } else {
                ("echo 'No recognized project type found'", "Unknown")
            };

            let params = serde_json::json!({
                "command": test_cmd,
                "description": "Run project tests"
            });
            let result = tool.execute(params, app.build_tool_context().await).await;
            let error_for_audit = result.error.clone();
            if let Some(ref engine) = app.streaming_engine {
                let mut tracker = engine.cost_tracker().lock().await;
                tracker.record_tool_execution(
                    "slash_verify",
                    result.success,
                    started.elapsed().as_millis() as u64,
                    error_for_audit.as_deref(),
                );
            }

            let output = &result.content;
            let passed = count_test_passed(output);
            let failed = count_test_failed(output);
            let summary = format!(
                "# Verify Report\n\nProject: {}\nCommand: `{}`\n\n**Result**: {} passed, {} failed\n\n```\n{}\n```",
                project_type, test_cmd, passed, failed,
                if output.len() > 2000 { &output[..2000] } else { output }
            );
            app.add_system_message(summary.clone());
            String::new()
        }
        None => "Skill 'verify' not found.".to_string(),
    }
}

pub async fn handle_debug(app: &mut TuiApp) -> String {
    match app.bundled_skills.get("debug") {
        Some(_skill) => {
            let started = std::time::Instant::now();
            let tool = crate::tools::BashTool;

            // Find recent log files
            let log_dir = dirs::data_local_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("priority-agent")
                .join("logs");

            let params = serde_json::json!({
                "command": format!("tail -n 100 {}/*.log 2>/dev/null | grep -E 'ERROR|WARN|panic' | tail -50 || echo 'No recent error logs found'", log_dir.display()),
                "description": "Check recent debug logs"
            });
            let result = tool.execute(params, app.build_tool_context().await).await;
            let error_for_audit = result.error.clone();
            if let Some(ref engine) = app.streaming_engine {
                let mut tracker = engine.cost_tracker().lock().await;
                tracker.record_tool_execution(
                    "slash_debug",
                    result.success,
                    started.elapsed().as_millis() as u64,
                    error_for_audit.as_deref(),
                );
            }

            let logs = if result.success && !result.content.trim().is_empty() {
                result.content
            } else {
                "No ERROR/WARN entries found in recent logs.".to_string()
            };

            let report = format!(
                "# Debug Report\n\nRecent errors/warnings in logs:\n\n```\n{}\n```\n\nTo get more details, run:\n- `tail -f ~/.priority-agent/logs/*.log` to watch logs live\n- Set `RUST_LOG=debug` for more verbose output",
                logs
            );
            app.add_system_message(report);
            String::new()
        }
        None => "Skill 'debug' not found.".to_string(),
    }
}

pub async fn handle_stuck(app: &mut TuiApp) -> String {
    match app.bundled_skills.get("stuck") {
        Some(_skill) => {
            let started = std::time::Instant::now();
            let tool = crate::tools::BashTool;

            // Scan for claude/priority-agent processes
            let params = serde_json::json!({
                "command": r#"ps aux | grep -E 'priority-agent|claude' | grep -v grep | awk '{print $2, $3, $4, $11}' | head -20"#,
                "description": "Scan for hung processes"
            });
            let result = tool.execute(params, app.build_tool_context().await).await;
            let error_for_audit = result.error.clone();
            if let Some(ref engine) = app.streaming_engine {
                let mut tracker = engine.cost_tracker().lock().await;
                tracker.record_tool_execution(
                    "slash_stuck",
                    result.success,
                    started.elapsed().as_millis() as u64,
                    error_for_audit.as_deref(),
                );
            }

            let processes = if result.success && !result.content.trim().is_empty() {
                result.content
            } else {
                "No other priority-agent or claude processes found.".to_string()
            };

            let report = format!(
                "# Stuck Process Report\n\nProcesses found (PID CPU% MEM% COMMAND):\n\n```\n{}\n```\n\nIf a process appears stuck (high CPU with no progress, D/T/Z state), you may want to:\n- Kill it: `kill -9 <PID>`\n- Check for zombie processes: `ps aux | grep Z`",
                processes
            );
            app.add_system_message(report);
            String::new()
        }
        None => "Skill 'stuck' not found.".to_string(),
    }
}

pub async fn handle_remember(app: &mut TuiApp, _args: &str) -> String {
    match app.bundled_skills.get("remember") {
        Some(skill) => {
            let started = std::time::Instant::now();
            if let Some(ref engine) = app.streaming_engine {
                let mut tracker = engine.cost_tracker().lock().await;
                tracker.record_tool_execution(
                    "slash_remember",
                    true,
                    started.elapsed().as_millis() as u64,
                    None,
                );
            }

            // Load existing memory files if they exist
            let claude_md = std::path::Path::new("CLAUDE.md");

            let report = format!(
                "# Remember Report\n\n## Memory Files\n\n{}\n\n## Suggestions\n\nBased on {}, consider:\n1. **CLAUDE.md** - Project-wide conventions, architecture decisions\n2. **CLAUDE.local.md** - User-specific preferences (git ignored)\n3. **Team memory** - Cross-project shared knowledge\n\nTo add a memory, use `/memory_save <content>` or manually edit CLAUDE.md.",
                if claude_md.exists() {
                    "CLAUDE.md exists in current directory."
                } else {
                    "No CLAUDE.md found in current directory."
                },
                skill.content
            );
            app.add_system_message(report);
            String::new()
        }
        None => "Skill 'remember' not found.".to_string(),
    }
}

pub fn handle_keybindings(app: &mut TuiApp, args: &str) -> String {
    match app.bundled_skills.get("keybindings") {
        Some(_skill) => {
            let config_dir = dirs::config_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("priority-agent");
            let kb_path = config_dir.join("keybindings.json");

            if args.is_empty() || args == "list" {
                // Show current keybindings
                if kb_path.exists() {
                    match std::fs::read_to_string(&kb_path) {
                        Ok(content) => format!("Current keybindings:\n\n```json\n{}\n```", content),
                        Err(e) => format!("Failed to read keybindings: {}", e),
                    }
                } else {
                    let default_kb = get_default_keybindings();
                    format!(
                        "No custom keybindings found. Default keybindings:\n\n```json\n{}\n```\n\nTo customize, use `/keybindings edit <json>`",
                        default_kb
                    )
                }
            } else if let Some(json_str) = args.strip_prefix("edit ") {
                // Basic validation
                if json_str.trim().starts_with("{") {
                    if let Some(parent) = kb_path.parent() {
                        std::fs::create_dir_all(parent).ok();
                    }
                    match std::fs::write(&kb_path, json_str) {
                        Ok(_) => format!("Keybindings saved to {}", kb_path.display()),
                        Err(e) => format!("Failed to save keybindings: {}", e),
                    }
                } else {
                    "Invalid JSON. Use `/keybindings edit <json>`".to_string()
                }
            } else {
                "Usage: /keybindings [list|edit <json>]".to_string()
            }
        }
        None => "Skill 'keybindings' not found.".to_string(),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Section 9: Batch 1 Commands (Phase 10)
// ═══════════════════════════════════════════════════════════════════════

/// /session - 会话管理
pub async fn handle_session_cmd(app: &mut TuiApp, args: &str) -> String {
    let args = args.trim();
    if args.is_empty() || args == "list" {
        // List sessions
        match app.session_manager.list_sessions(10) {
            Ok(sessions) => {
                if sessions.is_empty() {
                    "No sessions found.".to_string()
                } else {
                    let current = app.session_manager.current_session_id();
                    let mut lines = vec!["Sessions:".to_string()];
                    for (i, s) in sessions.iter().enumerate() {
                        let marker = if current == Some(s.id.as_str()) {
                            " (current)"
                        } else {
                            ""
                        };
                        lines.push(format!(
                            "{}. {}{} - {} [{}]",
                            i + 1,
                            s.title,
                            marker,
                            &s.id[..8.min(s.id.len())],
                            s.updated_at
                        ));
                    }
                    lines.push("\nUse /session <n> to switch.".to_string());
                    lines.join("\n")
                }
            }
            Err(e) => format!("Failed to list sessions: {}", e),
        }
    } else if let Ok(n) = args.parse::<usize>() {
        // Switch by index
        match app.session_manager.list_sessions(20) {
            Ok(sessions) if n > 0 && n <= sessions.len() => {
                let session = &sessions[n - 1];
                app.restore_session(&session.id).await
            }
            _ => "Invalid session number. Use /session list to see available.".to_string(),
        }
    } else if args == "new" || args.starts_with("new ") {
        // Create new session
        let title = args
            .strip_prefix("new ")
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .unwrap_or("New Session");
        match app.session_manager.start_session(title, "kimi-k2.5") {
            Ok(id) => {
                let _ = app.restore_session(&id).await;
                format!("Created new session: {}", title)
            }
            Err(e) => format!("Failed to create session: {}", e),
        }
    } else if args == "current" {
        // Show current session
        let id = app
            .session_manager
            .current_session_id()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "none".to_string());
        let title = app.session_manager.current_session_title();
        format!("Current session: {} ({})", title, &id[..8.min(id.len())])
    } else if args.starts_with("new") {
        "Usage: /session new <title>".to_string()
    } else {
        // Fallback: try switch by full session ID
        app.restore_session(args).await
    }
}
