//! Session slash-command handler support.
//!
//! Handles session actions and rewinds while leaving persistence mechanics in the session store.

use super::*;

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
        app.push_mode(crate::tui::app::AppMode::DiffViewer);
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
    app.push_mode(crate::tui::app::AppMode::DiffViewer);
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
