use super::*;

/// /revert last-turn — 一键回退最近一个 assistant turn 的所有文件变更
pub async fn handle_revert_turn(app: &mut TuiApp) -> String {
    let session_id = match app.session_manager.current_session_id() {
        Some(id) => id.to_string(),
        None => return "No active session.".to_string(),
    };

    if app.run_coordinator.is_active() || app.is_querying {
        return "Cannot revert while a session run is active. Wait for the current turn to complete or cancel it first.".to_string();
    }

    let mgr = crate::engine::checkpoint::get_checkpoint_manager(&session_id).await;
    let mut cp = mgr.lock().await;
    let result = match cp.revert_latest_assistant_turn().await {
        Ok(result) => result,
        Err(err) => return err,
    };

    // Record the revert for potential unrevert later.
    let record = crate::engine::checkpoint::RevertRecord {
        session_id: result.session_id.clone(),
        target_message_id: result.message_id.clone(),
        target_part_ids: result.part_ids.clone(),
        checkpoint_ids: result.checkpoint_ids.clone(),
        snapshot_checkpoint_id: result.snapshot_checkpoint_id.clone(),
        paths: result.paths.clone(),
        restored_files: result.restored_files.clone(),
        removed_files: result.removed_files.clone(),
        diff_summary: result.diff_summary.clone(),
        status: result.status.clone(),
        timestamp: result.timestamp.clone().unwrap_or_default(),
        unreverted: false,
    };
    cp.record_revert(record);
    drop(cp);

    let payload = serde_json::to_value(&result).unwrap_or_else(|_| serde_json::json!({}));
    if let Err(err) =
        app.session_manager
            .record_session_revert(&crate::session_store::SessionRevertInsert {
                session_id: result.session_id.clone(),
                operation: "revert".to_string(),
                status: result.status.clone(),
                message_id: result.message_id.clone(),
                target_part_id: result.target_part_id.clone(),
                part_ids: result.part_ids.clone(),
                checkpoint_ids: result.checkpoint_ids.clone(),
                snapshot_checkpoint_id: result.snapshot_checkpoint_id.clone(),
                paths: result.paths.clone(),
                restored_files: result.restored_files.clone(),
                removed_files: result.removed_files.clone(),
                errors: result.errors.clone(),
                diff_summary: result.diff_summary.clone(),
                unrevert_possible: result.unrevert_possible,
                unreverted: false,
                payload: payload.clone(),
            })
    {
        tracing::warn!("Failed to record session revert row: {}", err);
    }
    if let Err(err) = app
        .session_manager
        .write_session_event(&session_id, "revert", &payload)
    {
        tracing::warn!("Failed to record revert session event: {}", err);
    }

    let file_count = result.paths.len();
    let mut restored_summary = Vec::new();
    restored_summary.extend(
        result
            .restored_files
            .iter()
            .map(|path| format!("  restored: {path}")),
    );
    restored_summary.extend(
        result
            .removed_files
            .iter()
            .map(|path| format!("  removed: {path}")),
    );

    let unrevert_hint = if result.unrevert_possible {
        "\nUse /unrevert to undo this revert."
    } else {
        ""
    };

    if restored_summary.is_empty() && result.errors.is_empty() {
        format!(
            "Reverted {} file(s) from last turn (round: {:?}).\nUse /changes to see what was reverted.{}",
            file_count,
            result.tool_round_id.as_deref().unwrap_or("<single>"),
            unrevert_hint,
        )
    } else if result.errors.is_empty() {
        format!(
            "Reverted last turn's file changes:\n{}{}",
            restored_summary.join("\n"),
            unrevert_hint,
        )
    } else {
        format!(
            "Partial revert of last turn:\n{}\nErrors:\n{}{}",
            restored_summary.join("\n"),
            result.errors.join("\n"),
            unrevert_hint,
        )
    }
}

/// /unrevert — undo the last revert operation.
pub async fn handle_unrevert(app: &mut TuiApp) -> String {
    let session_id = match app.session_manager.current_session_id() {
        Some(id) => id.to_string(),
        None => return "No active session.".to_string(),
    };

    if app.run_coordinator.is_active() || app.is_querying {
        return "Cannot unrevert while a session run is active. Wait for the current turn to complete or cancel it first.".to_string();
    }

    let mgr = crate::engine::checkpoint::get_checkpoint_manager(&session_id).await;
    let mut cp = mgr.lock().await;
    let result = match cp.unrevert_latest().await {
        Ok(result) => result,
        Err(err) => return format!("Unrevert failed: {err}"),
    };
    drop(cp);

    let payload = serde_json::to_value(&result).unwrap_or_else(|_| serde_json::json!({}));
    if let Err(err) = app
        .session_manager
        .mark_latest_revert_unreverted(&session_id)
    {
        tracing::warn!("Failed to mark session revert as unreverted: {}", err);
    }
    if let Err(err) =
        app.session_manager
            .record_session_revert(&crate::session_store::SessionRevertInsert {
                session_id: result.session_id.clone(),
                operation: "unrevert".to_string(),
                status: result.status.clone(),
                message_id: result.message_id.clone(),
                target_part_id: result.target_part_id.clone(),
                part_ids: result.part_ids.clone(),
                checkpoint_ids: result.checkpoint_ids.clone(),
                snapshot_checkpoint_id: result.snapshot_checkpoint_id.clone(),
                paths: result.paths.clone(),
                restored_files: result.restored_files.clone(),
                removed_files: result.removed_files.clone(),
                errors: result.errors.clone(),
                diff_summary: result.diff_summary.clone(),
                unrevert_possible: result.unrevert_possible,
                unreverted: true,
                payload: payload.clone(),
            })
    {
        tracing::warn!("Failed to record session unrevert row: {}", err);
    }
    if let Err(err) = app
        .session_manager
        .write_session_event(&session_id, "unrevert", &payload)
    {
        tracing::warn!("Failed to record unrevert session event: {}", err);
    }

    let file_count = result.paths.len();
    if result.errors.is_empty() {
        format!("Unreverted {} file(s). Changes restored.", file_count)
    } else {
        format!(
            "Unrevert completed with {} warnings:\n{}",
            result.errors.len(),
            result.errors.join("\n")
        )
    }
}

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

/// /fork - 创建当前会话的子会话并切换到它。
pub async fn handle_fork(app: &mut TuiApp, args: &str) -> String {
    let title = args.trim();
    let title = if title.is_empty() {
        format!("{} (fork)", app.session_manager.current_session_title())
    } else {
        title.to_string()
    };

    match app
        .session_manager
        .fork_current_session(&title, &app.workspace.root.to_string_lossy())
        .await
    {
        Ok(id) => format!(
            "Forked into new session: {} ({}). Switched to it automatically.",
            title,
            &id[..8.min(id.len())]
        ),
        Err(err) => format!("Fork failed: {err}"),
    }
}

/// /changes - 展示最近 turn 的文件变更（含增删行数）
pub async fn handle_changes(app: &TuiApp) -> String {
    let session_id = match app.session_manager.current_session_id() {
        Some(id) => id.to_string(),
        None => return "No active session.".to_string(),
    };

    let mgr = crate::engine::checkpoint::get_checkpoint_manager(&session_id).await;
    let cp = mgr.lock().await;
    let file_changes = cp.list_file_changes();
    let rounds = cp.list_file_change_rounds();

    if file_changes.is_empty() && rounds.is_empty() {
        return "No file changes tracked yet. Changes are recorded when the agent uses file_write, file_edit, or file_patch."
            .to_string();
    }

    let mut lines = vec![
        format!(
            "Recent changes ({} files in {} tool rounds):",
            file_changes.len(),
            rounds.len()
        ),
        String::new(),
    ];

    if !rounds.is_empty() {
        lines.push("By tool round:".to_string());
        for summary in rounds.iter().rev().take(10) {
            let round = summary.tool_round_id.as_deref().unwrap_or("<single>");
            let diff_info = if summary.additions > 0 || summary.deletions > 0 {
                format!(" (+{}/-{})", summary.additions, summary.deletions)
            } else {
                String::new()
            };
            lines.push(format!(
                "  {} | {} file(s){} | {}B | {}",
                round,
                summary.change_count,
                diff_info,
                summary.total_bytes_written,
                summary
                    .paths
                    .iter()
                    .take(3)
                    .map(|p| p.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            ));
            if summary.paths.len() > 3 {
                lines.push(format!(
                    "    ... and {} more file(s)",
                    summary.paths.len() - 3
                ));
            }
        }
    }

    if !file_changes.is_empty() {
        lines.push(String::new());
        lines.push("Most recent file changes:".to_string());
        for change in file_changes.iter().rev().take(10) {
            let round = change
                .tool_round_id
                .as_deref()
                .map(|r| format!(" [{}]", r))
                .unwrap_or_default();
            lines.push(format!(
                "  {} | {} | {}B{}",
                change.tool_name, change.path, change.bytes_written, round,
            ));
        }
    }

    lines.push(String::new());
    lines.push("Use /undo to revert the last edit, /rewind for specific files/rounds.".to_string());
    lines.join("\n")
}

/// /diagnostic - 一键导出诊断包 (run_report.json + ledger summary)
pub async fn handle_diagnostic(app: &TuiApp) -> String {
    let session_id = app
        .session_manager
        .current_session_id()
        .unwrap_or("unknown");

    let model = app.current_model_label();
    let provider = Some(app.current_provider_label());
    let usage = app.stream_usage_snapshot;
    let turns = app
        .messages
        .iter()
        .filter(|message| message.role == crate::state::MessageRole::User)
        .count();

    let mgr = crate::engine::checkpoint::get_checkpoint_manager(session_id).await;
    let cp = mgr.lock().await;
    let file_changes = cp.list_file_changes().to_vec();
    let file_change_rounds = cp.list_file_change_rounds();
    drop(cp);

    let mut changed_file_set = std::collections::BTreeSet::new();
    for change in &file_changes {
        changed_file_set.insert(change.path.clone());
    }
    let changed_files = changed_file_set.into_iter().collect::<Vec<_>>();

    let tool_runs = diagnostic_tool_runs(app);
    let failed_tool_names = diagnostic_failed_tool_names(&tool_runs);
    let validation_status = diagnostic_validation_status(&tool_runs, !changed_files.is_empty());

    // Query usage ledger for session totals
    let ledger_summary =
        crate::cost_tracker::usage_ledger::summarize_usage_ledger(Some(session_id)).ok();

    let prompt_tokens = ledger_summary
        .as_ref()
        .map(|s| s.prompt_tokens)
        .unwrap_or(usage.map(|u| u.prompt_tokens as u64).unwrap_or(0));
    let completion_tokens = ledger_summary
        .as_ref()
        .map(|s| s.completion_tokens)
        .unwrap_or(usage.map(|u| u.completion_tokens as u64).unwrap_or(0));
    let cost_usd = ledger_summary.as_ref().map(|s| s.cost_usd).unwrap_or(0.0);
    let cache_miss_reason = ledger_summary
        .as_ref()
        .and_then(|s| s.last_miss_reason.clone());
    let ledger_entries = ledger_summary.as_ref().map(|s| s.entries).unwrap_or(0);
    let revert_payloads = app
        .session_manager
        .load_session_events(session_id)
        .unwrap_or_default()
        .into_iter()
        .filter(|event| event.event_type == "revert")
        .filter_map(|event| serde_json::from_str::<serde_json::Value>(&event.payload).ok())
        .collect::<Vec<_>>();
    let latest_revert_event = revert_payloads.last().cloned();
    let revert_events = revert_payloads.len();

    let provider_request = &app.facade_snapshot.provider_request;
    let latency_ms = (provider_request.elapsed_ms > 0).then_some(provider_request.elapsed_ms);
    let failure_owner = if !failed_tool_names.is_empty() {
        Some("tool".to_string())
    } else if provider_request.phase == crate::engine::runtime_facade::ProviderPhase::TimedOut {
        Some("provider".to_string())
    } else {
        None
    };
    let status = if app.is_querying {
        "running"
    } else if failure_owner.is_some() {
        "failed"
    } else {
        "snapshot"
    };
    let evidence_category = if !failed_tool_names.is_empty() {
        Some("tool_failure".to_string())
    } else if !changed_files.is_empty() {
        Some("file_change".to_string())
    } else if ledger_entries > 0 {
        Some("usage_ledger".to_string())
    } else if revert_events > 0 {
        Some("revert".to_string())
    } else {
        None
    };
    let evidence_items = file_changes.len()
        + tool_runs.iter().filter(|run| !run.is_active()).count()
        + ledger_entries as usize
        + revert_events;

    let report = crate::engine::evalset::RunReport {
        schema: crate::engine::evalset::RunReport::CURRENT_SCHEMA.to_string(),
        session_id: session_id.to_string(),
        model,
        provider,
        timestamp_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
        status: status.to_string(),
        turns,
        tool_rounds: file_change_rounds.len().max(tool_runs.len()),
        changed_files,
        verification_proof_status: validation_status,
        evidence_category,
        evidence_items,
        prompt_tokens,
        completion_tokens,
        cost_usd,
        latency_ms,
        time_to_first_token_ms: None,
        cache_miss_reason: cache_miss_reason.clone(),
        failure_owner,
        failed_tool_names,
        revert_events,
        latest_revert_event,
        provider_profile: Some(serde_json::json!({
            "provider_id": app.current_provider_label(),
            "model_id": app.current_model_label(),
            "protocol_family": "openai_compatible",
        })),
        tool_output_policy: Some({
            let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            serde_json::to_value(crate::tool_output_store::ToolOutputPolicy::from_project_env(&cwd))
                .unwrap_or_default()
        }),
    };

    let json = report.to_json_string();
    let path = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".priority-agent")
        .join(format!(
            "diagnostic_{}.json",
            &session_id[..8.min(session_id.len())]
        ));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    match std::fs::write(&path, &json) {
        Ok(_) => format!(
            "Diagnostic report exported to: {}\n\
             Schema: run_report.v1\n\
             Session: {}\n\
             Prompt tokens: {}\n\
             Completion tokens: {}\n\
             Cost: ${:.4}\n\
             Cache miss: {}\n\n\
             Based on usage ledger data ({} entries).",
            path.display(),
            session_id,
            prompt_tokens,
            completion_tokens,
            cost_usd,
            cache_miss_reason.as_deref().unwrap_or("none"),
            ledger_entries,
        ),
        Err(e) => format!("Failed to write diagnostic report: {}", e),
    }
}

fn diagnostic_tool_runs(app: &TuiApp) -> Vec<crate::tui::tool_view::ToolRunView> {
    let mut runs = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for run in app.sync_snapshot.all_tool_runs() {
        if seen.insert(run.id.clone()) {
            runs.push(run);
        }
    }
    for run in app.projected_tool_runs() {
        if seen.insert(run.id.clone()) {
            runs.push(run);
        }
    }
    runs
}

fn diagnostic_failed_tool_names(runs: &[crate::tui::tool_view::ToolRunView]) -> Vec<String> {
    let mut names = std::collections::BTreeSet::new();
    for run in runs {
        if matches!(
            run.status,
            crate::tui::tool_view::ToolRunStatus::Failed
                | crate::tui::tool_view::ToolRunStatus::TimedOut
                | crate::tui::tool_view::ToolRunStatus::Cancelled
        ) {
            names.insert(run.name.clone());
        }
    }
    names.into_iter().collect()
}

fn diagnostic_validation_status(
    runs: &[crate::tui::tool_view::ToolRunView],
    has_file_changes: bool,
) -> Option<String> {
    let mut saw_validation = false;
    let mut saw_passed_validation = false;
    for run in runs {
        if !tool_run_looks_like_validation(run) {
            continue;
        }
        saw_validation = true;
        match run.status {
            crate::tui::tool_view::ToolRunStatus::Completed => saw_passed_validation = true,
            crate::tui::tool_view::ToolRunStatus::Failed
            | crate::tui::tool_view::ToolRunStatus::TimedOut
            | crate::tui::tool_view::ToolRunStatus::Cancelled => {
                return Some("failed".to_string());
            }
            _ => {}
        }
    }
    if saw_passed_validation {
        Some("verified".to_string())
    } else if has_file_changes && !saw_validation {
        Some("pending".to_string())
    } else {
        None
    }
}

fn tool_run_looks_like_validation(run: &crate::tui::tool_view::ToolRunView) -> bool {
    if run.name == "run_tests" {
        return true;
    }
    if run.name != "bash" {
        return false;
    }
    let command = run
        .arguments
        .as_ref()
        .and_then(|args| {
            args.get("command")
                .or_else(|| args.get("cmd"))
                .and_then(serde_json::Value::as_str)
        })
        .unwrap_or_default()
        .to_ascii_lowercase();
    [
        "cargo test",
        "cargo check",
        "cargo clippy",
        "cargo fmt --check",
        "npm test",
        "pnpm test",
        "yarn test",
        "pytest",
        "go test",
    ]
    .iter()
    .any(|needle| command.contains(needle))
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

/// /share - local-only session export.
pub fn handle_share(app: &mut TuiApp, args: &str) -> String {
    let args = args.trim();
    if args == "status" {
        return "Share mode: local-only. Public/network share is disabled.".to_string();
    }
    if args == "disabled" {
        return "Public/network share is disabled. Use /share local [json|md] [redacted|full|summary]."
            .to_string();
    }

    let mut parts = args.split_whitespace().collect::<Vec<_>>();
    if parts.first() == Some(&"local") {
        parts.remove(0);
    } else if !parts.is_empty() {
        return "Usage: /share local [json|md] [redacted|full|summary]".to_string();
    }

    write_current_session_export(
        app,
        parts.first().copied().unwrap_or("json"),
        parts.get(1).copied().unwrap_or("redacted"),
        "Local share export",
    )
}

// ═══════════════════════════════════════════════════════════════════════
// Extended 3: More commands
// ═══════════════════════════════════════════════════════════════════════

/// /export - Export data
pub async fn handle_export_data(app: &mut TuiApp, args: &str) -> String {
    let parts = args.split_whitespace().collect::<Vec<_>>();
    write_current_session_export(
        app,
        parts.first().copied().unwrap_or("json"),
        parts.get(1).copied().unwrap_or("full"),
        "Session export",
    )
}

fn write_current_session_export(app: &TuiApp, format: &str, privacy: &str, label: &str) -> String {
    let Some(session_id) = app.session_manager.current_session_id() else {
        return "No active session to export.".to_string();
    };

    let Some(format) = parse_export_format(format) else {
        return "Usage: /export [json|md] [full|redacted|summary]".to_string();
    };
    let Some(privacy) = parse_export_privacy(privacy) else {
        return "Usage: /export [json|md] [full|redacted|summary]".to_string();
    };

    match app
        .session_manager
        .write_session_export(session_id, format, privacy)
    {
        Ok(path) => {
            let file_url = format!("file://{}", path.display());
            if let Ok(mut ctx) = arboard::Clipboard::new() {
                let _ = ctx.set_text(file_url.clone());
            }
            format!(
                "{label} written to: {}\n{file_url}\nPrivacy: {}\nFormat: {:?}",
                path.display(),
                privacy.label(),
                format
            )
        }
        Err(err) => format!("Failed to export session: {err}"),
    }
}

fn parse_export_format(value: &str) -> Option<crate::session_store::export::SessionExportFormat> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "json" => Some(crate::session_store::export::SessionExportFormat::Json),
        "md" | "markdown" => Some(crate::session_store::export::SessionExportFormat::Markdown),
        _ => None,
    }
}

fn parse_export_privacy(value: &str) -> Option<crate::session_store::export::SessionExportPrivacy> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "full" => Some(crate::session_store::export::SessionExportPrivacy::Full),
        "redacted" => Some(crate::session_store::export::SessionExportPrivacy::Redacted),
        "summary" => Some(crate::session_store::export::SessionExportPrivacy::Summary),
        _ => None,
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

/// /compact-status — 显示当前压缩状态
pub async fn handle_compact_status(app: &TuiApp) -> String {
    let Some(ref engine) = app.streaming_engine else {
        return "Engine not initialized.".to_string();
    };
    let history = engine.get_history().await;
    let msg_count = history.len();

    if msg_count == 0 {
        return "No active conversation. Compaction applies when context grows.".to_string();
    }

    let session_id = app
        .session_manager
        .current_session_id()
        .unwrap_or("unknown");

    // Check compact boundaries from session store (v7 migration)
    let boundary_info = engine.session_binding().map(|(store, _)| {
        let conn = store.shared_conn();
        let conn = conn.lock().expect("session actions sqlite conn lock poisoned");
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM compact_boundaries WHERE session_id = ?1",
                [&session_id],
                |row| row.get(0),
            )
            .unwrap_or(0);
        let last_strategy: Option<String> = conn
            .query_row(
                "SELECT strategy FROM compact_boundaries WHERE session_id = ?1 ORDER BY id DESC LIMIT 1",
                [&session_id],
                |row| row.get(0),
            )
            .ok();
        let last_trigger: Option<String> = conn
            .query_row(
                "SELECT trigger FROM compact_boundaries WHERE session_id = ?1 ORDER BY id DESC LIMIT 1",
                [&session_id],
                |row| row.get(0),
            )
            .ok();
        (count, last_strategy, last_trigger)
    });

    let mut lines = vec![
        "Compact Status".to_string(),
        format!("Session: {}", &session_id[..8.min(session_id.len())]),
        format!("Messages: {}", msg_count),
    ];

    if let Some((count, strategy, trigger)) = boundary_info {
        lines.push(format!("Compact boundaries: {}", count));
        if count > 0 {
            lines.push(format!(
                "Last compaction: strategy={}, trigger={}",
                strategy.as_deref().unwrap_or("unknown"),
                trigger.as_deref().unwrap_or("unknown"),
            ));
        }
    } else {
        lines.push("Compact boundaries: unavailable".to_string());
    }

    lines.push(String::new());
    lines.push("Use /compact to trigger manual compaction.".to_string());
    lines.push("Use /cost for token and cache diagnostics.".to_string());
    lines.join("\n")
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

    let boundary = format!(
        "... {} messages summarized ({} -> {} tokens) ...",
        attempt.messages_before.saturating_sub(after_msgs),
        attempt.before_tokens,
        after_tokens
    );

    let mut compacted_items: Vec<crate::state::MessageItem> = compacted
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

    compacted_items.insert(
        0,
        crate::state::MessageItem {
            id: "compact_boundary".to_string(),
            role: crate::state::MessageRole::System,
            content: boundary,
            timestamp: std::time::SystemTime::now(),
            metadata: Default::default(),
        },
    );

    app.messages = compacted_items;
    if let Some(session_id) = app.session_manager.current_session_id() {
        let _ = app
            .session_manager
            .replace_messages(session_id, &app.messages);
    }

    app.add_toast(
        format!(
            "Compacted {} -> {} messages ({} -> {} tokens)",
            attempt.messages_before, after_msgs, attempt.before_tokens, after_tokens
        ),
        "✓",
    );

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

#[cfg(test)]
mod tests {
    use super::*;

    fn tool_run(
        name: &str,
        status: crate::tui::tool_view::ToolRunStatus,
        command: &str,
    ) -> crate::tui::tool_view::ToolRunView {
        crate::tui::tool_view::ToolRunView {
            id: "id-1".to_string(),
            name: name.to_string(),
            args_buffer: String::new(),
            arguments: Some(serde_json::json!({"command": command})),
            status,
            progress: Vec::new(),
            result_body: None,
            result_preview: None,
            result_data: None,
            metadata: None,
            started_at: std::time::Instant::now(),
            completed_at: None,
        }
    }

    #[test]
    fn parse_export_format_accepts_aliases() {
        assert!(parse_export_format("json").is_some());
        assert!(parse_export_format("md").is_some());
        assert!(parse_export_format("markdown").is_some());
        assert!(parse_export_format("").is_some()); // default
        assert!(parse_export_format("xml").is_none());
        assert!(parse_export_format("PDF").is_none());
    }

    #[test]
    fn parse_export_privacy_accepts_modes() {
        assert!(parse_export_privacy("full").is_some());
        assert!(parse_export_privacy("redacted").is_some());
        assert!(parse_export_privacy("summary").is_some());
        assert!(parse_export_privacy("").is_some()); // default
        assert!(parse_export_privacy("secret").is_none());
    }

    #[test]
    fn diagnostic_failed_tool_names_collects_failures() {
        let runs = vec![
            tool_run(
                "file_read",
                crate::tui::tool_view::ToolRunStatus::Completed,
                "",
            ),
            tool_run(
                "bash",
                crate::tui::tool_view::ToolRunStatus::Failed,
                "cargo test",
            ),
            tool_run(
                "file_write",
                crate::tui::tool_view::ToolRunStatus::TimedOut,
                "",
            ),
            tool_run(
                "bash",
                crate::tui::tool_view::ToolRunStatus::Cancelled,
                "npm test",
            ),
        ];
        let names = diagnostic_failed_tool_names(&runs);
        assert_eq!(names.len(), 2); // deduped
        assert!(names.contains(&"bash".to_string()));
        assert!(names.contains(&"file_write".to_string()));
    }

    #[test]
    fn diagnostic_validation_status_with_changes_and_passing() {
        let runs = vec![tool_run(
            "bash",
            crate::tui::tool_view::ToolRunStatus::Completed,
            "cargo test -q",
        )];
        let status = diagnostic_validation_status(&runs, true);
        assert_eq!(status.as_deref(), Some("verified"));
    }

    #[test]
    fn diagnostic_validation_status_without_changes() {
        let runs = vec![tool_run(
            "bash",
            crate::tui::tool_view::ToolRunStatus::Completed,
            "cargo test -q",
        )];
        let status = diagnostic_validation_status(&runs, false);
        // No file changes: should report no-diff pass
        assert!(status.is_some());
    }

    #[test]
    fn tool_run_looks_like_validation_detects_cargo_test() {
        let run = tool_run(
            "bash",
            crate::tui::tool_view::ToolRunStatus::Completed,
            "cargo test -q",
        );
        assert!(tool_run_looks_like_validation(&run));
    }

    #[test]
    fn tool_run_looks_like_validation_detects_npm_test() {
        let run = tool_run(
            "bash",
            crate::tui::tool_view::ToolRunStatus::Completed,
            "npm test",
        );
        assert!(tool_run_looks_like_validation(&run));
    }

    #[test]
    fn tool_run_looks_like_validation_rejects_echo() {
        let run = tool_run(
            "bash",
            crate::tui::tool_view::ToolRunStatus::Completed,
            "echo hello",
        );
        assert!(!tool_run_looks_like_validation(&run));
    }
}
