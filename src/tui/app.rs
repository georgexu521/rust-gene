//! 交互式终端 CLI 应用状态管理
//!
//! 对应 Claude Code 中的 AppState 概念

use crate::engine::agent_mode::AgentMode;
use crate::engine::conversation_loop::ToolApprovalResponse;
use crate::engine::human_review::PermissionReviewDecision;
use crate::engine::streaming::{StreamEvent, StreamingQueryEngine};
use crate::permissions::{PermissionMode, PermissionRules, RuleSource, SourcedRule};
use crate::state::{
    select_runtime_status, select_tool_viewer_tool_id, AppContext, AppState, MessageItem,
    MessageRole, RuntimeAppState, RuntimeBridgeState, RuntimeMcpState, RuntimePermissionState,
    RuntimeStatusSnapshot, RuntimeTerminalTask, RuntimeToolStatus, RuntimeToolUse, TaskItem,
};
use crate::tui::components::input::InputState;
use crate::tui::tool_view::{upsert_tool_run, with_tool_run, ToolRunStatus, ToolRunView};
use futures::StreamExt;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use super::commands::{default_command_registry, CommandRegistry};

const LONG_PASTE_CHAR_THRESHOLD: usize = 600;
const LONG_PASTE_LINE_THRESHOLD: usize = 12;

#[derive(Debug, Clone)]
struct PastedBlock {
    placeholder: String,
    content: String,
}

#[derive(Debug, Clone)]
struct PendingSkillInvocation {
    name: String,
    version: String,
    started_at: std::time::Instant,
}

#[derive(Debug, Clone)]
struct SkillOutcomeAttribution {
    success: bool,
    acceptance_passed: Option<bool>,
    tests_passed: Option<bool>,
    user_satisfaction: Option<f32>,
    risk_penalty: f32,
    confidence: f32,
    source: &'static str,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct StreamUsageSnapshot {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub reasoning_tokens: Option<u32>,
    pub cached_tokens: Option<u32>,
}

impl StreamUsageSnapshot {
    pub fn total_tokens(self) -> u32 {
        self.prompt_tokens + self.completion_tokens
    }

    pub fn cache_miss_tokens(self) -> Option<u32> {
        self.cached_tokens.map(|cached| {
            self.prompt_tokens
                .saturating_sub(cached.min(self.prompt_tokens))
        })
    }

    pub fn cache_hit_rate_percent(self) -> Option<f64> {
        self.cached_tokens.map(|cached| {
            if self.prompt_tokens == 0 {
                0.0
            } else {
                cached.min(self.prompt_tokens) as f64 / self.prompt_tokens as f64 * 100.0
            }
        })
    }
}

fn skill_outcome_attribution(
    trace: Option<&crate::engine::trace::TurnTrace>,
    has_response: bool,
    stream_error: bool,
    failed_tool: bool,
) -> SkillOutcomeAttribution {
    let mut latest_acceptance = None;
    let mut latest_verification = None;
    if let Some(trace) = trace {
        for event in trace.events.iter().rev() {
            match event {
                crate::engine::trace::TraceEvent::AcceptanceReviewCompleted {
                    accepted,
                    unresolved,
                    ..
                } if latest_acceptance.is_none() => {
                    latest_acceptance = Some((*accepted, *unresolved));
                }
                crate::engine::trace::TraceEvent::VerificationCompleted { passed, .. }
                    if latest_verification.is_none() =>
                {
                    latest_verification = Some(*passed);
                }
                _ => {}
            }
            if latest_acceptance.is_some() && latest_verification.is_some() {
                break;
            }
        }
    }

    if let Some((accepted, unresolved)) = latest_acceptance {
        let verified = latest_verification.unwrap_or(accepted);
        let success = accepted && verified && !stream_error && !failed_tool;
        return SkillOutcomeAttribution {
            success,
            acceptance_passed: Some(accepted),
            tests_passed: Some(verified),
            user_satisfaction: Some(if success { 0.85 } else { 0.20 }),
            risk_penalty: if success {
                0.05
            } else if unresolved > 0 {
                0.45
            } else {
                0.30
            },
            confidence: 0.90,
            source: "acceptance_review",
        };
    }

    if let Some(verified) = latest_verification {
        let success = verified && has_response && !stream_error && !failed_tool;
        return SkillOutcomeAttribution {
            success,
            acceptance_passed: None,
            tests_passed: Some(verified),
            user_satisfaction: Some(if success { 0.75 } else { 0.25 }),
            risk_penalty: if success { 0.10 } else { 0.35 },
            confidence: 0.78,
            source: "verification",
        };
    }

    let success = has_response && !stream_error && !failed_tool;
    SkillOutcomeAttribution {
        success,
        acceptance_passed: Some(success),
        tests_passed: None,
        user_satisfaction: Some(if success { 0.70 } else { 0.25 }),
        risk_penalty: if success { 0.05 } else { 0.30 },
        confidence: 0.65,
        source: "heuristic",
    }
}

pub(crate) fn permission_mode_name(mode: PermissionMode) -> &'static str {
    match mode {
        PermissionMode::Default => "default",
        PermissionMode::AutoLowRisk => "auto_low_risk",
        PermissionMode::AutoAll => "auto",
        PermissionMode::ReadOnly => "read_only",
        PermissionMode::Once => "once",
    }
}

pub(crate) fn parse_permission_mode(mode: &str) -> Option<PermissionMode> {
    match mode.to_ascii_lowercase().as_str() {
        "default" => Some(PermissionMode::Default),
        "auto_low_risk" | "autolowrisk" | "low_risk" => Some(PermissionMode::AutoLowRisk),
        "auto" | "developer_auto" | "developer-auto" | "auto_all" | "autoall" => {
            Some(PermissionMode::AutoAll)
        }
        "read_only" | "readonly" => Some(PermissionMode::ReadOnly),
        "once" => Some(PermissionMode::Once),
        _ => None,
    }
}

fn runtime_tool_use_from_view(run: &ToolRunView) -> RuntimeToolUse {
    RuntimeToolUse {
        id: run.id.clone(),
        name: run.name.clone(),
        summary: run.summary(),
        status: match run.status {
            ToolRunStatus::Queued => RuntimeToolStatus::Queued,
            ToolRunStatus::Running => RuntimeToolStatus::Running,
            ToolRunStatus::Backgrounded => RuntimeToolStatus::Backgrounded,
            ToolRunStatus::WaitingPermission => RuntimeToolStatus::WaitingPermission,
            ToolRunStatus::TimedOut => RuntimeToolStatus::TimedOut,
            ToolRunStatus::Cancelled => RuntimeToolStatus::Cancelled,
            ToolRunStatus::Completed => RuntimeToolStatus::Completed,
            ToolRunStatus::Failed => RuntimeToolStatus::Failed,
        },
        active: run.is_active(),
        arguments: run.arguments.clone(),
        latest_progress: run.progress.last().cloned(),
        result_preview: run.result_preview.clone(),
        elapsed_ms: u64::try_from(run.elapsed().as_millis()).ok(),
        operation_kind: metadata_string(run.metadata.as_ref(), "operation_kind"),
        ui_render_kind: metadata_string(run.metadata.as_ref(), "ui_render_kind"),
        read_only: metadata_bool(run.metadata.as_ref(), "read_only"),
        concurrency_safe: metadata_bool(run.metadata.as_ref(), "concurrency_safe"),
        destructive: metadata_bool(run.metadata.as_ref(), "destructive"),
        input_paths: metadata_string_array(run.metadata.as_ref(), "input_paths"),
        transcript_summary: metadata_string(run.metadata.as_ref(), "transcript_summary"),
    }
}

fn metadata_string(metadata: Option<&serde_json::Value>, key: &str) -> Option<String> {
    metadata?
        .get(key)?
        .as_str()
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn metadata_bool(metadata: Option<&serde_json::Value>, key: &str) -> Option<bool> {
    metadata?.get(key)?.as_bool()
}

fn metadata_string_array(metadata: Option<&serde_json::Value>, key: &str) -> Vec<String> {
    metadata
        .and_then(|metadata| metadata.get(key))
        .and_then(serde_json::Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(serde_json::Value::as_str)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn runtime_terminal_task_from_view(run: &ToolRunView) -> Option<RuntimeTerminalTask> {
    let task = run.metadata.as_ref()?.get("terminal_task")?;
    let id = task
        .get("task_id")
        .or_else(|| task.get("handle"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or(run.id.as_str())
        .to_string();
    let status = task
        .get("status")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(match run.status {
            ToolRunStatus::Queued => "queued",
            ToolRunStatus::Running => "running",
            ToolRunStatus::Backgrounded => "running",
            ToolRunStatus::WaitingPermission => "waiting_permission",
            ToolRunStatus::TimedOut => "timed_out",
            ToolRunStatus::Cancelled => "cancelled",
            ToolRunStatus::Completed => "completed",
            ToolRunStatus::Failed => "failed",
        })
        .to_string();
    Some(RuntimeTerminalTask {
        id,
        status,
        terminal_kind: task
            .get("terminal_kind")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        command: task
            .get("command")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        handle: task
            .get("handle")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        output_path: task
            .get("output_path")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        read_tool: task
            .get("read_tool")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        cancel_handle: task
            .get("cancel_handle")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
    })
}

fn tool_run_status_label(status: ToolRunStatus) -> &'static str {
    match status {
        ToolRunStatus::Queued => "queued",
        ToolRunStatus::Running => "running",
        ToolRunStatus::Backgrounded => "backgrounded",
        ToolRunStatus::WaitingPermission => "waiting_permission",
        ToolRunStatus::TimedOut => "timed_out",
        ToolRunStatus::Cancelled => "cancelled",
        ToolRunStatus::Completed => "completed",
        ToolRunStatus::Failed => "failed",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MemorySaveTarget {
    Auto,
    User,
    Topic,
}

fn parse_memory_save_args(args: &str) -> (MemorySaveTarget, Option<&str>, &str) {
    let trimmed = args.trim();
    if let Some(rest) = trimmed.strip_prefix("--user ") {
        return (MemorySaveTarget::User, None, rest.trim());
    }
    if let Some(rest) = trimmed.strip_prefix("--topic=") {
        let mut parts = rest.trim().splitn(2, char::is_whitespace);
        let topic = parts.next().filter(|part| !part.trim().is_empty());
        let content = parts.next().unwrap_or("").trim();
        return (MemorySaveTarget::Topic, topic, content);
    }
    if let Some(rest) = trimmed.strip_prefix("--topic ") {
        let mut parts = rest.trim().splitn(2, char::is_whitespace);
        let topic = parts.next().filter(|part| !part.trim().is_empty());
        let content = parts.next().unwrap_or("").trim();
        return (MemorySaveTarget::Topic, topic, content);
    }
    (MemorySaveTarget::Auto, None, trimmed)
}

fn format_memory_write_outcome(
    content: &str,
    outcome: &crate::memory::manager::MemoryWriteOutcome,
) -> String {
    use crate::memory::manager::MemoryWriteOutcomeStatus;

    let score = outcome
        .quality_score
        .map(|score| format!("quality {:.2}", score))
        .unwrap_or_else(|| "quality n/a".to_string());
    let path = outcome
        .path
        .as_ref()
        .map(|path| format!("\nPath: {}", path.display()))
        .unwrap_or_default();
    match outcome.status {
        MemoryWriteOutcomeStatus::Saved => {
            format!("Saved memory: {}\n{}{}", content, score, path)
        }
        MemoryWriteOutcomeStatus::Duplicate => {
            format!(
                "Memory already exists; not saved again: {}\nReason: {}{}",
                content, outcome.reason, path
            )
        }
        MemoryWriteOutcomeStatus::Proposed => {
            format!(
                "Memory was not saved to long-term memory yet: quality gate proposed review.\n{}; reason: {}",
                score, outcome.reason
            )
        }
        MemoryWriteOutcomeStatus::Rejected => {
            format!(
                "Memory was not saved: quality gate rejected it.\n{}; reason: {}",
                score, outcome.reason
            )
        }
        MemoryWriteOutcomeStatus::Blocked => {
            format!("Memory was blocked for safety: {}", outcome.reason)
        }
        MemoryWriteOutcomeStatus::Failed => {
            format!("Memory save failed: {}{}", outcome.reason, path)
        }
        MemoryWriteOutcomeStatus::InvalidTarget => {
            format!("Memory save target is invalid: {}", outcome.reason)
        }
    }
}

fn dedupe_palette_commands(commands: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut deduped = Vec::new();
    for command in commands {
        if seen.insert(command.clone()) {
            deduped.push(command);
        }
    }
    deduped
}

fn build_welcome_content(is_first_run: bool) -> String {
    if is_first_run {
        return format!(
            "Priority Agent\n\nWelcome. Press Enter to start onboarding, or type /skip to skip.\n\nRuntime:\n- Agent mode: {}\n\nGetting started:\n- Ctrl+P opens the command palette\n- Ctrl+M changes model; Ctrl+L changes provider\n- Type ? on an empty prompt for shortcuts\n- Use /init <name> to create a project scaffold",
            AgentMode::Auto.label()
        );
    }

    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let project_name = cwd
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("workspace");
    let branch = std::process::Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(&cwd)
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
                (!text.is_empty()).then_some(text)
            } else {
                None
            }
        });
    let markers = [
        ("Rust", "Cargo.toml"),
        ("Node", "package.json"),
        ("Python", "pyproject.toml"),
        ("Git", ".git"),
        ("Agent", "AGENTS.md"),
    ];
    let detected = markers
        .iter()
        .filter_map(|(label, path)| cwd.join(path).exists().then_some(*label))
        .collect::<Vec<_>>();
    let detected = if detected.is_empty() {
        "plain workspace".to_string()
    } else {
        detected.join(", ")
    };

    format!(
        "Priority Agent\n\nWelcome back.\n\nProject overview:\n- Name: {}\n- Path: {}\n- Agent mode: {}\n- {}\n- {}\n- Detected: {}\n{}\n\n{}\n\nNext actions:\n1. Ask a question about this codebase\n2. Run /quick for the command dashboard\n3. Run /init <name> to scaffold a new project\n4. Press Ctrl+P for commands, Ctrl+M for model, Ctrl+L for provider",
        project_name,
        cwd.display(),
        AgentMode::Auto.label(),
        branch
            .map(|b| format!("Branch: {}", b))
            .unwrap_or_else(|| "Branch: none".to_string()),
        workspace_change_preview(&cwd),
        detected,
        workspace_entries_preview(&cwd),
        recent_activity_preview()
    )
}

fn workspace_change_preview(cwd: &std::path::Path) -> String {
    let Ok(out) = std::process::Command::new("git")
        .args(["status", "--short"])
        .current_dir(cwd)
        .output()
    else {
        return "Changes: not a git repository".to_string();
    };
    if !out.status.success() {
        return "Changes: not a git repository".to_string();
    }
    let lines = String::from_utf8_lossy(&out.stdout);
    let changed = lines.lines().filter(|line| !line.trim().is_empty()).count();
    if changed == 0 {
        "Changes: clean".to_string()
    } else {
        format!("Changes: {} files", changed)
    }
}

fn workspace_entries_preview(cwd: &std::path::Path) -> String {
    let Ok(entries) = std::fs::read_dir(cwd) else {
        return "- Entries: unavailable".to_string();
    };

    let mut dirs = Vec::new();
    let mut files = Vec::new();
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') && name != ".priority-agent" {
            continue;
        }
        if file_type.is_dir() {
            dirs.push(name);
        } else if file_type.is_file() {
            files.push(name);
        }
    }

    dirs.sort();
    files.sort();
    let dir_count = dirs.len();
    let file_count = files.len();
    let mut highlights = dirs
        .iter()
        .take(4)
        .map(|name| format!("{}/", name))
        .chain(files.iter().take(4).cloned())
        .collect::<Vec<_>>();
    if highlights.is_empty() {
        highlights.push("empty workspace".to_string());
    }

    format!(
        "- Entries: {} dirs, {} files ({})",
        dir_count,
        file_count,
        highlights.join(", ")
    )
}

fn recent_activity_preview() -> String {
    let Ok(manager) = crate::tui::session_manager::TuiSessionManager::new() else {
        return "Recent activity:\n- unavailable".to_string();
    };
    let Ok(sessions) = manager.list_sessions(3) else {
        return "Recent activity:\n- unavailable".to_string();
    };
    if sessions.is_empty() {
        return "Recent activity:\n- no prior sessions".to_string();
    }

    let mut lines = vec!["Recent activity:".to_string()];
    for session in sessions {
        let count = manager.message_count(&session.id).unwrap_or_default();
        let mut title = if session.title.trim().is_empty() {
            format!("Session {}", &session.id[..8.min(session.id.len())])
        } else {
            session.title
        };
        if title.chars().count() > 42 {
            title = format!("{}…", title.chars().take(41).collect::<String>());
        }
        lines.push(format!(
            "  - {} ({} msgs, {})",
            title, count, session.updated_at
        ));
    }
    lines.join("\n")
}

fn format_memory_retrieval_context(
    ctx: &crate::engine::retrieval_context::RetrievalContext,
) -> String {
    let mut lines = vec![
        "Memory Search".to_string(),
        format!(
            "Query: {} · items: {} · tokens~{} · conflicts: {}",
            ctx.query,
            ctx.items.len(),
            ctx.token_estimate,
            ctx.conflict_count()
        ),
        String::new(),
    ];
    if let Some(trace) = &ctx.memory_trace {
        lines.push(format!(
            "Trace: selected={} chars={}/{} skipped unrelated={} unsafe={} stale_conflict={} budget={}",
            trace.selected_records,
            trace.selected_chars,
            trace.max_chars,
            trace.skipped_unrelated,
            trace.skipped_unsafe,
            trace.skipped_stale_conflict,
            trace.skipped_budget
        ));
        for scope in &trace.per_scope {
            lines.push(format!(
                "  scope {}: selected={} skipped={} cap={}",
                scope.scope, scope.selected, scope.skipped, scope.cap
            ));
        }
        for decision in trace.decisions.iter().take(6) {
            let score = decision
                .score_explanation
                .as_ref()
                .map(|explanation| {
                    format!(
                        " lexical={:.2} recency={:.2} scope_match={:.2} confidence={:.2} status={} conflict_penalty={:.2} pinned_bonus={:.2} final={:.2}",
                        explanation.lexical_match,
                        explanation.recency,
                        explanation.scope_match,
                        explanation.confidence,
                        explanation.status,
                        explanation.conflict_penalty,
                        explanation.user_pinned_bonus,
                        explanation.final_score
                    )
                })
                .unwrap_or_default();
            lines.push(format!(
                "  decision {} {} scope={} score={} chars={}{} reason={}",
                decision.action,
                decision.source,
                decision.scope,
                decision.score,
                decision.chars,
                score,
                decision.reason
            ));
        }
        lines.push(String::new());
    }
    for item in &ctx.items {
        let preview = item
            .content_preview
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .take(2)
            .collect::<Vec<_>>()
            .join(" ");
        lines.push(format!(
            "- {} · {} · score {:.2} · {:?}{}",
            item.id,
            item.title,
            item.score,
            item.trust,
            if item.conflict { " · conflict" } else { "" }
        ));
        lines.push(format!("  reason: {}", item.reason));
        lines.push(format!("  provenance: {}", item.provenance));
        lines.push(format!(
            "  {}",
            preview.chars().take(240).collect::<String>()
        ));
    }
    lines.join("\n")
}

fn format_memory_snapshot_report(snapshot: &crate::memory::MemorySnapshotReport) -> String {
    format!(
        "Memory Snapshot\n- Status: {}\n- Snapshot id: {}\n- Fingerprint: {}\n- Scope: {}\n- Stable prompt chars: {}\n- Project chars: {}\n- User chars: {}\n- Memory files: {} ({} chars)\n- Skipped records: {} (status={} unsafe={} stale={} conflicts={})",
        if snapshot.frozen {
            "frozen"
        } else {
            "live/not frozen"
        },
        snapshot.snapshot_id,
        snapshot.fingerprint,
        snapshot.scope,
        snapshot.char_count,
        snapshot.project_chars,
        snapshot.user_chars,
        snapshot.memory_file_count,
        snapshot.memory_file_chars,
        snapshot.skipped_record_count,
        snapshot.skipped_status_count,
        snapshot.skipped_unsafe_count,
        snapshot.skipped_stale_count,
        snapshot.skipped_conflict_count
    )
}

fn format_memory_migration_command(mem: &crate::memory::MemoryManager, args: &str) -> String {
    let mut parts = args.split_whitespace();
    match parts.next().unwrap_or("--dry-run") {
        "--dry-run" | "dry-run" | "status" => mem.memory_migration_dry_run().format(),
        "--backup" | "backup" => match mem.memory_migration_backup() {
            Ok(report) => report.format(),
            Err(error) => format!("Memory migration backup failed: {}", error),
        },
        "--rollback" | "rollback" => {
            let Some(backup_id) = parts.next() else {
                return "Usage: /memory migrate --rollback <backup_id>".to_string();
            };
            match mem.memory_migration_rollback(backup_id) {
                Ok(report) => report.format(),
                Err(error) => format!("Memory migration rollback failed: {}", error),
            }
        }
        _ => "Usage: /memory migrate [--dry-run|--backup|--rollback <backup_id>]".to_string(),
    }
}

fn format_memory_records(records: &[crate::memory::MemoryRecord], args: &str) -> String {
    let scope_filter = parse_memory_scope_filter(args);
    let mut filtered = records
        .iter()
        .filter(|record| {
            scope_filter
                .as_deref()
                .map(|scope| memory_record_scope_label(record).contains(scope))
                .unwrap_or(true)
        })
        .collect::<Vec<_>>();
    filtered.sort_by(|left, right| {
        right
            .updated_at
            .cmp(&left.updated_at)
            .then_with(|| left.id.cmp(&right.id))
    });
    if filtered.is_empty() {
        return match scope_filter {
            Some(scope) => format!("Memory Records\n- none for scope '{}'", scope),
            None => "Memory Records\n- none".to_string(),
        };
    }
    let mut lines = vec![format!(
        "Memory Records ({} shown{})",
        filtered.len().min(30),
        scope_filter
            .as_deref()
            .map(|scope| format!(" · scope={scope}"))
            .unwrap_or_default()
    )];
    for record in filtered.into_iter().take(30) {
        lines.push(format!(
            "- {} [{} {:?}] scope={} confidence={:.2} utility={:.2} evidence={} used={} updated={}",
            record.id,
            memory_status_label(record.status),
            record.kind,
            memory_record_scope_label(record),
            record.confidence,
            record.utility,
            record.evidence.len(),
            record.use_count,
            record.updated_at.to_rfc3339()
        ));
        lines.push(format!(
            "  {}",
            record
                .content
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .chars()
                .take(260)
                .collect::<String>()
        ));
    }
    lines.join("\n")
}

fn parse_memory_scope_filter(args: &str) -> Option<String> {
    let parts = args.split_whitespace().collect::<Vec<_>>();
    for (idx, part) in parts.iter().enumerate() {
        if let Some(scope) = part.strip_prefix("--scope=") {
            return Some(scope.to_ascii_lowercase());
        }
        if *part == "--scope" {
            return parts.get(idx + 1).map(|scope| scope.to_ascii_lowercase());
        }
    }
    parts
        .first()
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.to_ascii_lowercase())
}

fn memory_record_scope_label(record: &crate::memory::MemoryRecord) -> String {
    if matches!(record.kind, crate::memory::MemoryKind::UserPreference) {
        return "user".to_string();
    }
    if record.scope.project_root.is_some() {
        return "project".to_string();
    }
    if !record.scope.session_id.trim().is_empty() {
        return "session".to_string();
    }
    record.scope.platform.clone()
}

fn memory_status_label(status: crate::memory::MemoryStatus) -> &'static str {
    match status {
        crate::memory::MemoryStatus::Proposed => "proposed",
        crate::memory::MemoryStatus::Accepted => "accepted",
        crate::memory::MemoryStatus::Rejected => "rejected",
        crate::memory::MemoryStatus::Superseded => "superseded",
        crate::memory::MemoryStatus::Archived => "archived",
    }
}

fn format_memory_proposal_queue() -> String {
    let queue = memory_proposal_queue_json();
    let mut lines = vec![format!(
        "Pending memory candidates\n- Proposed: {} · accepted: {} · rejected: {} · applied: {} · background: {} · closeout: {}",
        queue["proposed"].as_u64().unwrap_or(0),
        queue["accepted"].as_u64().unwrap_or(0),
        queue["rejected"].as_u64().unwrap_or(0),
        queue["applied"].as_u64().unwrap_or(0),
        queue["background"].as_u64().unwrap_or(0),
        queue["closeout"].as_u64().unwrap_or(0)
    )];
    if let Some(recent) = queue["recent"].as_array() {
        for item in recent.iter().take(5) {
            lines.push(format!(
                "- {} [{}] source={} candidates={} reason={}",
                item["id"].as_str().unwrap_or("unknown"),
                item["status"].as_str().unwrap_or("unknown"),
                item["source"].as_str().unwrap_or("unknown"),
                item["candidates"].as_u64().unwrap_or(0),
                item["reason"].as_str().unwrap_or("")
            ));
        }
    }
    lines.join("\n")
}

fn memory_proposal_queue_json() -> serde_json::Value {
    use crate::engine::task_contract::{MemoryProposalReviewStore, MemoryProposalStatus};

    let mut records = MemoryProposalReviewStore::default().list_records();
    let mut proposed = 0usize;
    let mut accepted = 0usize;
    let mut rejected = 0usize;
    let mut applied = 0usize;
    let mut background = 0usize;
    let mut closeout = 0usize;
    for record in &records {
        match record.proposal.status {
            MemoryProposalStatus::Proposed => proposed += 1,
            MemoryProposalStatus::Accepted => accepted += 1,
            MemoryProposalStatus::Rejected => rejected += 1,
            MemoryProposalStatus::Applied => applied += 1,
            MemoryProposalStatus::NotApplicable => {}
        }
        match record.source.as_str() {
            "background" => background += 1,
            "closeout" => closeout += 1,
            _ => {}
        }
    }
    records.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    let recent = records
        .into_iter()
        .take(5)
        .map(|record| {
            serde_json::json!({
                "id": record.proposal.task_id,
                "status": record.proposal.status.label(),
                "source": record.source,
                "candidates": record.proposal.candidates.len(),
                "updated_at": record.updated_at,
                "reason": record.proposal.reason.chars().take(120).collect::<String>(),
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "total": proposed + accepted + rejected + applied,
        "proposed": proposed,
        "accepted": accepted,
        "rejected": rejected,
        "applied": applied,
        "background": background,
        "closeout": closeout,
        "recent": recent,
    })
}

fn explain_memory_retrieval_item(
    ctx: &crate::engine::retrieval_context::RetrievalContext,
    selector: &str,
) -> String {
    let selector = selector.to_lowercase();
    let Some(item) = ctx.items.iter().find(|item| {
        item.id.to_lowercase().contains(&selector)
            || item.title.to_lowercase().contains(&selector)
            || item.provenance.to_lowercase().contains(&selector)
    }) else {
        return format!(
            "No retrieval item matching '{}'. Run /memory search <query> to see ids.",
            selector
        );
    };
    format!(
        "Memory Retrieval Explanation\n\nid: {}\nsource: {:?}\ntitle: {}\nscore: {:.2}\ntrust: {:?}\nconflict: {}\nprovenance: {}\nreason: {}\n\n{}",
        item.id,
        item.source,
        item.title,
        item.score,
        item.trust,
        item.conflict,
        item.provenance,
        item.reason,
        item.content_preview
    )
}

fn read_git_branch_fast(cwd: &std::path::Path) -> Option<String> {
    let head_path = cwd.join(".git").join("HEAD");
    let head = std::fs::read_to_string(head_path).ok()?;
    let head = head.trim();
    if let Some(branch) = head.strip_prefix("ref: refs/heads/") {
        Some(branch.to_string())
    } else if head.len() >= 7 {
        Some(head.chars().take(7).collect())
    } else {
        None
    }
}

fn provider_name_from_base_url(base_url: &str) -> &'static str {
    let u = base_url.to_ascii_lowercase();
    if u.contains("minimax") {
        "MiniMax"
    } else if u.contains("api.kimi.com") {
        "Kimi Code"
    } else if u.contains("moonshot") {
        "Kimi"
    } else if u.contains("deepseek") {
        "DeepSeek"
    } else if u.contains("bigmodel") || u.contains("z.ai") {
        "GLM"
    } else if u.contains("openai.com") {
        "OpenAI"
    } else {
        "Custom"
    }
}

pub(crate) fn permission_rule_pattern(tool_name: &str, args: &serde_json::Value) -> String {
    crate::engine::human_review::permission_rule_pattern(tool_name, args)
}

#[derive(serde::Deserialize, Default)]
struct LegacyPermissionRules {
    #[serde(default)]
    always_allow: Vec<String>,
    #[serde(default)]
    always_deny: Vec<String>,
    #[serde(default)]
    always_ask: Vec<String>,
}

fn load_rules_for_edit(path: &std::path::Path) -> anyhow::Result<PermissionRules> {
    if !path.exists() {
        return Ok(PermissionRules::new());
    }
    let content = std::fs::read_to_string(path)?;
    if content.trim().is_empty() {
        return Ok(PermissionRules::new());
    }
    if let Ok(rules) = toml::from_str::<PermissionRules>(&content) {
        return Ok(rules);
    }
    let legacy = toml::from_str::<LegacyPermissionRules>(&content)?;
    let mut rules = PermissionRules::new();
    rules.always_allow = legacy
        .always_allow
        .into_iter()
        .map(|p| SourcedRule::new(p, RuleSource::User))
        .collect();
    rules.always_deny = legacy
        .always_deny
        .into_iter()
        .map(|p| SourcedRule::new(p, RuleSource::User))
        .collect();
    rules.always_ask = legacy
        .always_ask
        .into_iter()
        .map(|p| SourcedRule::new(p, RuleSource::User))
        .collect();
    Ok(rules)
}

pub(crate) fn persist_permission_rule(
    scope: RuleSource,
    decision: &str,
    pattern: &str,
    working_dir: &std::path::Path,
) -> anyhow::Result<std::path::PathBuf> {
    let path = match scope {
        RuleSource::Global => dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".priority-agent")
            .join("permissions.toml"),
        _ => working_dir.join(".priority-agent").join("permissions.toml"),
    };

    let mut rules = load_rules_for_edit(&path)?;
    let source_for_file = match scope {
        RuleSource::Global => RuleSource::Global,
        _ => RuleSource::Project,
    };
    let rule = SourcedRule::new(pattern, source_for_file);
    let target = match decision {
        "allow" => &mut rules.always_allow,
        "deny" => &mut rules.always_deny,
        "ask" => &mut rules.always_ask,
        _ => anyhow::bail!("invalid decision: {}", decision),
    };
    if !target.iter().any(|r| r.pattern == pattern) {
        target.push(rule);
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = toml::to_string_pretty(&rules)?;
    std::fs::write(&path, content)?;
    Ok(path)
}

fn permission_review_decision_for_response(
    approved: bool,
    decision: Option<&str>,
    scope: Option<RuleSource>,
) -> Option<PermissionReviewDecision> {
    match (approved, decision, scope) {
        (true, Some("allow"), Some(RuleSource::User)) => {
            Some(PermissionReviewDecision::ApproveSession)
        }
        (true, Some("allow"), Some(RuleSource::Project)) => {
            Some(PermissionReviewDecision::ApproveProject)
        }
        (true, Some("allow"), Some(RuleSource::Global)) => {
            Some(PermissionReviewDecision::ApproveGlobal)
        }
        (false, Some("deny"), Some(RuleSource::Global)) => {
            Some(PermissionReviewDecision::RejectAlways)
        }
        (true, None, None) => Some(PermissionReviewDecision::ApproveOnce),
        (false, None, None) => Some(PermissionReviewDecision::RejectOnce),
        (true, _, _) => Some(PermissionReviewDecision::ApproveOnce),
        (false, _, _) => Some(PermissionReviewDecision::RejectOnce),
    }
}

/// 交互式 CLI 应用模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Chat,
    Settings,
    PlanApproval,
    PermissionApproval,
    AskUser,
    DiffViewer,
    ToolViewer,
    VimNormal,
    Onboarding,
    MessageSearch,
    CommandPalette,
    ShortcutHelp,
    ModelSelect,
    ProviderSelect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelChoice {
    pub provider: String,
    pub model: String,
    pub note: String,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderChoice {
    pub name: String,
    pub provider_type: String,
    pub model: String,
    pub base_url: String,
    pub configured: bool,
    pub active: bool,
    pub note: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusBarDensity {
    Compact,
    Normal,
    Debug,
}

impl StatusBarDensity {
    pub fn next(self) -> Self {
        match self {
            Self::Compact => Self::Normal,
            Self::Normal => Self::Debug,
            Self::Debug => Self::Compact,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Normal => "normal",
            Self::Debug => "debug",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "compact" | "min" | "minimal" => Some(Self::Compact),
            "normal" | "default" => Some(Self::Normal),
            "debug" | "verbose" | "full" => Some(Self::Debug),
            _ => None,
        }
    }
}

/// 交互式 CLI 应用状态
pub struct TuiApp {
    /// 当前模式
    pub mode: AppMode,
    /// 当前 coding agent 产品模式
    pub agent_mode: AgentMode,
    /// 输入状态
    pub input: InputState,
    /// 消息列表
    pub messages: Vec<MessageItem>,
    /// 任务列表
    pub tasks: Vec<TaskItem>,
    /// 是否正在查询中
    pub is_querying: bool,
    /// Streaming start time for t/s calculation
    pub stream_started_at: Option<std::time::Instant>,
    /// 是否处于暂停态（不接受新消息发送）
    pub paused: bool,
    /// 是否启用聚焦模式（仅显示 user/assistant）
    pub focus_mode: bool,
    /// 状态栏信息密度
    pub status_bar_density: StatusBarDensity,
    /// 命令注册表
    command_registry: CommandRegistry,
    /// 滚动位置
    pub scroll_offset: usize,
    /// 是否自动贴底（用户手动上滚后变为 false，滚到底或新消息时恢复）
    pub pinned_to_bottom: bool,
    /// 应用上下文
    pub context: AppContext,
    /// 最后错误信息
    pub error_message: Option<String>,
    /// 命令历史
    pub history: VecDeque<String>,
    /// 历史索引
    pub history_index: Option<usize>,
    /// 流式查询引擎
    pub streaming_engine: Option<Arc<StreamingQueryEngine>>,
    /// 当前流式响应缓冲
    current_response: Arc<Mutex<String>>,
    /// 工具运行视图状态（后台流更新，前台 tick 同步快照）
    tool_runs: Arc<Mutex<Vec<ToolRunView>>>,
    /// 当前工具运行视图快照
    pub tool_runs_snapshot: Vec<ToolRunView>,
    /// Shared runtime-state snapshot used by status/tool selectors.
    pub runtime_state_snapshot: RuntimeAppState,
    /// 历史工具运行视图，按触发该轮的用户消息 id 锚定
    pub tool_runs_by_message_id: HashMap<String, Vec<ToolRunView>>,
    current_tool_anchor_id: Option<String>,
    /// 是否展开工具 transcript 细节
    pub transcript_expanded: bool,
    /// 当前展开的单个工具 id；None 表示全部折叠
    pub expanded_tool_run_id: Option<String>,
    stream_usage: Arc<Mutex<Option<StreamUsageSnapshot>>>,
    pub stream_usage_snapshot: Option<StreamUsageSnapshot>,
    /// 流是否已完成（由后台任务设置）
    stream_done: Arc<AtomicBool>,
    /// 后台流式任务句柄（用于取消）
    stream_handle: Option<tokio::task::JoinHandle<()>>,
    /// 会话管理器
    pub session_manager: crate::tui::session_manager::TuiSessionManager,
    /// 设置状态
    pub settings_state: Option<crate::tui::components::settings::SettingsState>,
    /// 待审批的计划
    pub pending_plan: Option<crate::engine::plan_mode::Plan>,
    /// 计划审批响应发送器
    pub plan_response_tx:
        Option<tokio::sync::oneshot::Sender<crate::engine::plan_mode::PlanApproval>>,
    /// 计划修改输入缓冲
    pub plan_modification_input: String,
    /// 待审批的工具权限请求
    pub pending_permission_request: Option<crate::engine::conversation_loop::ToolApprovalRequest>,
    /// 工具权限审批响应发送器
    pub permission_response_tx: Option<tokio::sync::oneshot::Sender<ToolApprovalResponse>>,
    /// 待回答的用户问题
    pub pending_question: Option<String>,
    /// 用户问题的选项
    pub pending_question_options: Vec<String>,
    /// 用户问题响应发送器
    pub question_response_tx: Option<tokio::sync::oneshot::Sender<String>>,
    /// Diff 查看器内容
    pub diff_content: String,
    /// Diff 查看器标题
    pub diff_title: String,
    /// Diff 查看器滚动偏移
    pub diff_scroll_offset: u16,
    /// 工具输出查看器内容
    pub tool_viewer_content: String,
    /// 工具输出查看器标题
    pub tool_viewer_title: String,
    /// 工具输出查看器滚动偏移
    pub tool_viewer_scroll_offset: u16,
    /// 消息搜索状态
    pub message_search_state: crate::tui::components::message_search::MessageSearchState,
    /// 折叠的消息索引（Vim Normal 模式下 Tab 折叠/展开）
    pub collapsed_indices: std::collections::HashSet<usize>,
    /// 会话侧边栏是否可见
    pub sidebar_visible: bool,
    /// 侧边栏选中索引
    pub sidebar_selected: usize,
    /// 打字机效果当前显示位置（字符数）
    pub typewriter_position: usize,
    /// LSP 管理器
    pub lsp_manager: Option<Arc<crate::engine::lsp::LspManager>>,
    /// Worktree 管理器
    pub worktree_manager: Option<Arc<crate::engine::worktree::WorktreeManager>>,
    /// CLI app start time for uptime and diagnostics.
    pub app_started_at: std::time::Instant,
    /// Bundled skills
    pub bundled_skills: std::collections::HashMap<String, crate::skills::Skill>,
    /// Unified skill runtime for bundled, project, and user skills.
    pub skill_runtime: crate::skills::SkillRuntime,
    /// 是否启用 Vim 模式
    pub vim_mode: bool,
    /// 键位映射
    pub keybindings: crate::tui::keybindings::Keybindings,
    /// 当前主题
    pub theme: Arc<crate::tui::theme::Theme>,
    /// 引导状态
    pub onboarding_state: Option<crate::onboarding::OnboardingState>,
    /// Plan Mode 状态标签缓存（用于状态栏显示，避免渲染时异步查询）
    pub plan_mode_label: Option<String>,
    /// Tick 计数器（用于 spinner 等动画）
    pub tick_count: usize,
    /// 被折叠的长粘贴块，发送时还原
    pasted_blocks: Vec<PastedBlock>,
    /// 命令面板搜索词
    pub command_palette_query: String,
    /// 命令面板选中项
    pub command_palette_selected: usize,
    /// 最近从命令面板执行/选择的命令
    pub recent_palette_commands: VecDeque<String>,
    /// User-scoped temporary permission rules installed by the active skill.
    active_skill_permission_rules: Vec<(String, String)>,
    /// 模型选择器选中项
    pub model_select_selected: usize,
    /// 模型选择器搜索词
    pub model_select_query: String,
    /// 最近一次模型切换提示
    pub model_notice: Option<String>,
    /// Provider 选择器选中项
    pub provider_select_selected: usize,
    /// Provider 选择器搜索词
    pub provider_select_query: String,
    /// 最近一次 provider 切换提示
    pub provider_notice: Option<String>,
    /// Skill invocations waiting for final assistant outcome attribution.
    pending_skill_invocations: Vec<PendingSkillInvocation>,
}

impl TuiApp {
    pub fn new() -> Self {
        Self::create(None, None, None)
    }

    /// 创建带流式引擎的 TuiApp
    pub fn with_engine(
        engine: Arc<StreamingQueryEngine>,
        lsp_manager: Option<Arc<crate::engine::lsp::LspManager>>,
        worktree_manager: Option<Arc<crate::engine::worktree::WorktreeManager>>,
    ) -> Self {
        Self::create(Some(engine), lsp_manager, worktree_manager)
    }

    fn create(
        engine: Option<Arc<StreamingQueryEngine>>,
        lsp_manager: Option<Arc<crate::engine::lsp::LspManager>>,
        worktree_manager: Option<Arc<crate::engine::worktree::WorktreeManager>>,
    ) -> Self {
        info!("Creating new TuiApp");

        let context = AppContext::new();

        // 初始化会话管理器。优先复用引擎会话，这样 UI 历史、
        // trace 与 learning events 会写入同一条 conversation。
        let model = engine
            .as_ref()
            .map(|engine| engine.model_name())
            .unwrap_or_else(|| "unknown".to_string());
        let mut session_manager = if let Some((store, session_id)) =
            engine.as_ref().and_then(|engine| engine.session_binding())
        {
            crate::tui::session_manager::TuiSessionManager::from_store(
                store,
                session_id,
                "New Session",
                &model,
            )
            .unwrap_or_else(|e| {
                warn!("Failed to bind TUI session to engine session: {}", e);
                crate::tui::session_manager::TuiSessionManager::new().unwrap_or_else(|e| {
                    warn!("Failed to initialize session manager: {}", e);
                    crate::tui::session_manager::TuiSessionManager::in_memory()
                        .expect("Failed to create in-memory session manager")
                })
            })
        } else {
            crate::tui::session_manager::TuiSessionManager::new().unwrap_or_else(|e| {
                warn!("Failed to initialize session manager: {}", e);
                crate::tui::session_manager::TuiSessionManager::in_memory()
                    .expect("Failed to create in-memory session manager")
            })
        };

        if session_manager.current_session_id().is_none() {
            let _ = session_manager.start_session("New Session", &model);
        }

        // 检测首次启动
        let onboarding_manager = crate::onboarding::OnboardingManager::new();
        let is_first_run = onboarding_manager.is_first_run();

        // 添加欢迎消息
        let welcome_content = build_welcome_content(is_first_run);
        let welcome_message = MessageItem {
            id: "welcome".to_string(),
            role: MessageRole::System,
            content: welcome_content,
            timestamp: std::time::SystemTime::now(),
            metadata: Default::default(),
        };

        let onboarding_state = if is_first_run {
            Some(crate::onboarding::OnboardingState::new())
        } else {
            None
        };

        Self {
            mode: if is_first_run {
                AppMode::Onboarding
            } else {
                AppMode::Chat
            },
            agent_mode: AgentMode::Auto,
            input: InputState::new(),
            messages: vec![welcome_message],
            tasks: Vec::new(),
            is_querying: false,
            stream_started_at: None,
            paused: false,
            focus_mode: false,
            status_bar_density: StatusBarDensity::Normal,
            command_registry: default_command_registry(),
            scroll_offset: 0,
            pinned_to_bottom: true,
            context,
            error_message: None,
            history: VecDeque::with_capacity(100),
            history_index: None,
            streaming_engine: engine,
            current_response: Arc::new(Mutex::new(String::new())),
            tool_runs: Arc::new(Mutex::new(Vec::new())),
            tool_runs_snapshot: Vec::new(),
            runtime_state_snapshot: RuntimeAppState::default(),
            tool_runs_by_message_id: HashMap::new(),
            current_tool_anchor_id: None,
            transcript_expanded: false,
            expanded_tool_run_id: None,
            stream_usage: Arc::new(Mutex::new(None)),
            stream_usage_snapshot: None,
            stream_done: Arc::new(AtomicBool::new(true)),
            stream_handle: None,
            session_manager,
            settings_state: None,
            pending_plan: None,
            plan_response_tx: None,
            plan_modification_input: String::new(),
            plan_mode_label: None,
            pending_permission_request: None,
            permission_response_tx: None,
            pending_question: None,
            pending_question_options: Vec::new(),
            question_response_tx: None,
            diff_content: String::new(),
            diff_title: String::new(),
            diff_scroll_offset: 0,
            tool_viewer_content: String::new(),
            tool_viewer_title: String::new(),
            tool_viewer_scroll_offset: 0,
            message_search_state: crate::tui::components::message_search::MessageSearchState::new(),
            collapsed_indices: std::collections::HashSet::new(),
            sidebar_visible: false,
            sidebar_selected: 0,
            typewriter_position: 0,
            tick_count: 0,
            lsp_manager,
            worktree_manager,
            app_started_at: std::time::Instant::now(),
            bundled_skills: {
                let mut map = std::collections::HashMap::new();
                for skill in crate::skills::loader::load_bundled_skills() {
                    map.insert(skill.meta.name.clone(), skill);
                }
                map
            },
            skill_runtime: crate::skills::SkillRuntime::load(
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
            ),
            vim_mode: false,
            keybindings: crate::tui::keybindings::Keybindings::load(),
            theme: {
                let config = crate::services::config::AppConfig::load().unwrap_or_default();
                Arc::new(crate::tui::theme::Theme::from_name(&config.ui.theme))
            },
            onboarding_state,
            pasted_blocks: Vec::new(),
            command_palette_query: String::new(),
            command_palette_selected: 0,
            recent_palette_commands: VecDeque::with_capacity(16),
            active_skill_permission_rules: Vec::new(),
            model_select_selected: 0,
            model_select_query: String::new(),
            model_notice: None,
            provider_select_selected: 0,
            provider_select_query: String::new(),
            provider_notice: None,
            pending_skill_invocations: Vec::new(),
        }
    }

    /// 提交用户消息
    pub async fn submit_message(&mut self) {
        let content = self.expand_paste_placeholders(self.input.value());
        if content.trim().is_empty() {
            return;
        }

        // 清空输入
        self.input.clear();
        self.pasted_blocks.clear();

        // 处理斜杠命令
        if content.starts_with('/') {
            self.handle_slash_command(&content).await;
            return;
        }

        self.send_message(content).await;
    }

    /// 插入粘贴内容；长粘贴折叠为占位符，避免输入区撑满屏幕
    pub fn insert_paste(&mut self, text: String) {
        if text.is_empty() {
            return;
        }

        let char_count = text.chars().count();
        let line_count = text.lines().count().max(1);
        if char_count < LONG_PASTE_CHAR_THRESHOLD && line_count < LONG_PASTE_LINE_THRESHOLD {
            self.input.insert_str(&text);
            return;
        }

        let paste_id = self.pasted_blocks.len() + 1;
        let placeholder = format!(
            "[[paste:{} {} lines {} chars]]",
            paste_id, line_count, char_count
        );
        self.pasted_blocks.push(PastedBlock {
            placeholder: placeholder.clone(),
            content: text,
        });
        self.input.insert_str(&placeholder);
    }

    pub fn pasted_block_count(&self) -> usize {
        self.pasted_blocks
            .iter()
            .filter(|block| self.input.value().contains(&block.placeholder))
            .count()
    }

    fn expand_paste_placeholders(&self, content: &str) -> String {
        let mut expanded = content.to_string();
        for block in &self.pasted_blocks {
            expanded = expanded.replace(&block.placeholder, &block.content);
        }
        expanded
    }

    pub fn open_command_palette(&mut self) {
        self.command_palette_query.clear();
        self.command_palette_selected = 0;
        self.mode = AppMode::CommandPalette;
    }

    pub fn close_command_palette(&mut self) {
        self.command_palette_query.clear();
        self.command_palette_selected = 0;
        self.mode = if self.vim_mode {
            AppMode::VimNormal
        } else {
            AppMode::Chat
        };
    }

    pub fn command_palette_items(&self) -> Vec<&crate::tui::commands::CommandDef> {
        let boosted_commands = self.command_palette_boosted_commands();
        let mut items = self.command_registry.palette_items(
            &self.command_palette_query,
            18,
            boosted_commands.as_slice(),
        );
        let contextual = self.contextual_palette_commands();
        if self.command_palette_query.is_empty() && !contextual.is_empty() {
            items.sort_by_key(|cmd| {
                contextual
                    .iter()
                    .position(|name| name == cmd.name)
                    .unwrap_or(usize::MAX)
            });
        }
        items
    }

    pub fn contextual_palette_commands(&self) -> Vec<String> {
        let mut commands = Vec::new();
        if self.pending_permission_request.is_some() {
            commands.push("/reject".to_string());
            commands.push("/permissions".to_string());
            commands.push("/quick".to_string());
        }
        if self.pending_plan.is_some() || self.pending_question.is_some() {
            commands.push("/quick".to_string());
            commands.push("/reject".to_string());
        }
        if self.messages.len() > 1 {
            commands.push("/search".to_string());
            commands.push("/session".to_string());
            commands.push("/export".to_string());
        }
        dedupe_palette_commands(commands)
    }

    pub fn is_contextual_palette_command(&self, name: &str) -> bool {
        self.contextual_palette_commands()
            .iter()
            .any(|command| command == name)
    }

    fn command_palette_boosted_commands(&self) -> Vec<String> {
        let mut commands = self
            .recent_palette_commands
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        commands.extend(self.contextual_palette_commands().into_iter().rev());
        dedupe_palette_commands(commands)
    }

    pub fn command_palette_next(&mut self) {
        let len = self.command_palette_items().len();
        if len > 0 {
            self.command_palette_selected = (self.command_palette_selected + 1).min(len - 1);
        }
    }

    pub fn command_palette_prev(&mut self) {
        self.command_palette_selected = self.command_palette_selected.saturating_sub(1);
    }

    pub fn command_palette_push(&mut self, c: char) {
        self.command_palette_query.push(c);
        self.command_palette_selected = 0;
    }

    pub fn command_palette_backspace(&mut self) {
        self.command_palette_query.pop();
        self.command_palette_selected = 0;
    }

    pub async fn accept_command_palette_selection(&mut self) {
        let selected = self
            .command_palette_items()
            .get(self.command_palette_selected)
            .map(|cmd| {
                (
                    (*cmd).clone(),
                    crate::tui::commands::command_accept_behavior(cmd),
                )
            });
        if let Some((cmd, behavior)) = selected {
            self.record_palette_command(cmd.name);
            match behavior {
                crate::tui::commands::CommandAcceptBehavior::Execute => {
                    self.close_command_palette();
                    self.handle_slash_command(cmd.name).await;
                    return;
                }
                crate::tui::commands::CommandAcceptBehavior::Insert => {
                    self.input.set_value(format!("{} ", cmd.name));
                }
            }
        }
        self.close_command_palette();
    }

    fn record_palette_command(&mut self, name: &str) {
        self.recent_palette_commands.retain(|cmd| cmd != name);
        self.recent_palette_commands.push_back(name.to_string());
        while self.recent_palette_commands.len() > 8 {
            self.recent_palette_commands.pop_front();
        }
    }

    pub fn open_shortcut_help(&mut self) {
        self.mode = AppMode::ShortcutHelp;
    }

    pub fn close_shortcut_help(&mut self) {
        self.mode = if self.vim_mode {
            AppMode::VimNormal
        } else {
            AppMode::Chat
        };
    }

    pub fn open_model_select(&mut self) {
        self.model_select_query.clear();
        self.model_select_selected = self
            .model_choices()
            .iter()
            .position(|choice| choice.active)
            .unwrap_or(0);
        self.mode = AppMode::ModelSelect;
    }

    pub fn close_model_select(&mut self) {
        self.mode = if self.vim_mode {
            AppMode::VimNormal
        } else {
            AppMode::Chat
        };
    }

    pub fn model_choices(&self) -> Vec<ModelChoice> {
        let provider = self.current_provider_label();
        let current = self.current_model_label();
        let mut models = match provider.as_str() {
            "MiniMax" => vec![
                "MiniMax-M3",
                "MiniMax-M2.7",
                "MiniMax-M2.7-highspeed",
                "MiniMax-M2.5",
                "MiniMax-M2",
            ],
            "Kimi Code" => vec!["kimi-for-coding"],
            "DeepSeek" => vec!["deepseek-v4-pro", "deepseek-v4-flash", "deepseek-chat"],
            "GLM" => vec!["glm-5.1", "glm-4.7", "glm-4.6"],
            "OpenAI" => vec!["gpt-4o", "gpt-4o-mini"],
            "Kimi" => vec!["kimi-k2.5", "kimi-k2.5-thinking"],
            _ => vec![current.as_str()],
        };
        if !models.iter().any(|m| *m == current) {
            models.insert(0, current.as_str());
        }
        models
            .into_iter()
            .map(|model| ModelChoice {
                provider: provider.clone(),
                model: model.to_string(),
                note: if model == current {
                    "current".to_string()
                } else {
                    "same provider, takes effect next request".to_string()
                },
                active: model == current,
            })
            .filter(|choice| {
                self.model_select_query.is_empty()
                    || choice
                        .model
                        .to_ascii_lowercase()
                        .contains(&self.model_select_query.to_ascii_lowercase())
                    || choice
                        .provider
                        .to_ascii_lowercase()
                        .contains(&self.model_select_query.to_ascii_lowercase())
            })
            .collect()
    }

    pub fn model_select_next(&mut self) {
        let len = self.model_choices().len();
        if len > 0 {
            self.model_select_selected = (self.model_select_selected + 1).min(len - 1);
        }
    }

    pub fn model_select_prev(&mut self) {
        self.model_select_selected = self.model_select_selected.saturating_sub(1);
    }

    pub fn model_select_push(&mut self, c: char) {
        self.model_select_query.push(c);
        self.model_select_selected = 0;
    }

    pub fn model_select_backspace(&mut self) {
        self.model_select_query.pop();
        self.model_select_selected = 0;
    }

    pub fn accept_model_selection(&mut self) {
        let Some(choice) = self
            .model_choices()
            .get(self.model_select_selected)
            .cloned()
        else {
            self.close_model_select();
            return;
        };
        if let Some(engine) = &self.streaming_engine {
            engine.set_model(choice.model.clone());
        }
        if let Ok(mut config) = crate::services::config::AppConfig::load() {
            config.api.model = choice.model.clone();
            let _ = config.save();
        }
        self.model_notice = Some(format!("Model switched to {}", choice.model));
        self.close_model_select();
    }

    /// Switch theme at runtime (in-memory only).
    ///
    /// Does NOT persist to config — callers that need persistence must
    /// also update `AppConfig::ui.theme` and call `.save()`.  The
    /// `/theme` slash handler and `save_settings()` already do this.
    pub fn set_theme(&mut self, name: &str) {
        self.theme = Arc::new(crate::tui::theme::Theme::from_name(name));
    }

    /// List available theme names
    pub fn theme_names(&self) -> Vec<String> {
        vec![
            "graphite".into(),
            "porcelain".into(),
            "nord".into(),
            "dracula".into(),
            "gruvbox-dark".into(),
            "catppuccin-mocha".into(),
            "dark".into(),
            "light".into(),
            "high-contrast".into(),
        ]
    }

    pub fn open_provider_select(&mut self) {
        self.provider_select_query.clear();
        self.provider_select_selected = self
            .provider_choices()
            .iter()
            .position(|choice| choice.active)
            .unwrap_or(0);
        self.mode = AppMode::ProviderSelect;
    }

    pub fn close_provider_select(&mut self) {
        self.mode = if self.vim_mode {
            AppMode::VimNormal
        } else {
            AppMode::Chat
        };
    }

    pub fn provider_choices(&self) -> Vec<ProviderChoice> {
        let active_base = self.current_provider_base_url();
        let registry = crate::services::api::provider::ProviderRegistry::from_env();
        let mut choices = registry
            .list_configs()
            .into_iter()
            .map(|cfg| {
                let base_url = cfg.base_url.unwrap_or_default();
                let active = !active_base.is_empty() && active_base == base_url;
                ProviderChoice {
                    name: cfg.name,
                    provider_type: format!("{:?}", cfg.provider_type),
                    model: cfg.default_model,
                    base_url,
                    configured: true,
                    active,
                    note: if active {
                        "current".to_string()
                    } else {
                        "configured".to_string()
                    },
                }
            })
            .collect::<Vec<_>>();

        for spec in crate::services::api::provider::DEFAULT_PROVIDER_ENV_SPECS {
            if choices.iter().any(|choice| choice.name == spec.id) {
                continue;
            }
            choices.push(ProviderChoice {
                name: spec.id.to_string(),
                provider_type: format!("{:?}", spec.provider_type),
                model: spec.default_model.to_string(),
                base_url: String::new(),
                configured: false,
                active: false,
                note: format!("missing {}", spec.key_env_hint()),
            });
        }

        let query = self.provider_select_query.to_ascii_lowercase();
        if !query.is_empty() {
            choices.retain(|choice| {
                choice.name.to_ascii_lowercase().contains(&query)
                    || choice.provider_type.to_ascii_lowercase().contains(&query)
                    || choice.model.to_ascii_lowercase().contains(&query)
                    || choice.note.to_ascii_lowercase().contains(&query)
            });
        }
        choices.sort_by_key(|choice| (!choice.active, !choice.configured, choice.name.clone()));
        choices
    }

    pub fn provider_select_next(&mut self) {
        let len = self.provider_choices().len();
        if len > 0 {
            self.provider_select_selected = (self.provider_select_selected + 1).min(len - 1);
        }
    }

    pub fn provider_select_prev(&mut self) {
        self.provider_select_selected = self.provider_select_selected.saturating_sub(1);
    }

    pub fn provider_select_push(&mut self, c: char) {
        self.provider_select_query.push(c);
        self.provider_select_selected = 0;
    }

    pub fn provider_select_backspace(&mut self) {
        self.provider_select_query.pop();
        self.provider_select_selected = 0;
    }

    pub fn accept_provider_selection(&mut self) -> String {
        let Some(choice) = self
            .provider_choices()
            .get(self.provider_select_selected)
            .cloned()
        else {
            self.close_provider_select();
            return "No provider selected.".to_string();
        };
        let result = self.switch_provider_by_name(&choice.name);
        self.close_provider_select();
        result
    }

    pub fn switch_provider_by_name(&mut self, name: &str) -> String {
        let registry = crate::services::api::provider::ProviderRegistry::from_env();
        let Some(provider) = registry.get(name) else {
            return format!("Provider '{}' is not configured. Use /provider list to inspect required environment variables.", name);
        };
        let Some(config) = registry.get_config(name).cloned() else {
            return format!("Provider '{}' has no config.", name);
        };
        if let Some(engine) = &self.streaming_engine {
            engine.set_provider(provider, config.default_model.clone());
        }
        if let Ok(mut app_config) = crate::services::config::AppConfig::load() {
            app_config.api.model = config.default_model.clone();
            app_config.api.base_url = config.base_url.clone().unwrap_or_default();
            let _ = app_config.save();
        }
        self.provider_notice = Some(format!(
            "Provider switched to {} ({})",
            config.name, config.default_model
        ));
        format!(
            "Provider switched to {}\nModel: {}\nBase URL: {}",
            config.name,
            config.default_model,
            config.base_url.unwrap_or_default()
        )
    }

    fn clear_active_skill_rules(&mut self) {
        let Some(engine) = &self.streaming_engine else {
            self.active_skill_permission_rules.clear();
            return;
        };
        for (decision, pattern) in self.active_skill_permission_rules.drain(..) {
            engine.remove_session_permission_rule(&decision, &pattern);
        }
    }

    fn apply_skill_invocation_policy(&mut self, invocation: &crate::skills::SkillInvocation) {
        self.clear_active_skill_rules();
        let Some(engine) = &self.streaming_engine else {
            return;
        };
        for pattern in &invocation.allowed_tools {
            engine.add_session_permission_rule("allow", pattern);
            self.active_skill_permission_rules
                .push(("allow".to_string(), pattern.to_string()));
        }
        for pattern in &invocation.disallowed_tools {
            engine.add_session_permission_rule("deny", pattern);
            self.active_skill_permission_rules
                .push(("deny".to_string(), pattern.to_string()));
        }
    }

    /// 发送消息到 LLM（核心逻辑，可被 skill 调用复用）
    pub(crate) async fn send_message(&mut self, content: String) {
        if content.trim().is_empty() {
            return;
        }
        if self.paused {
            self.add_system_message(
                "Agent is paused. Use `/pause resume` to continue sending messages.".to_string(),
            );
            return;
        }

        debug!("Submitting message: {}", content);

        // 取消之前的流式任务（如果有）
        if let Some(handle) = self.stream_handle.take() {
            handle.abort();
        }

        // 添加到历史
        self.history.push_back(content.clone());
        if self.history.len() > 100 {
            self.history.pop_front();
        }

        // 添加用户消息
        let user_msg_id = format!("msg_{}", self.messages.len());
        let user_msg = MessageItem {
            id: user_msg_id.clone(),
            role: MessageRole::User,
            content: content.clone(),
            timestamp: std::time::SystemTime::now(),
            metadata: Default::default(),
        };
        self.messages.push(user_msg);

        // 如果流式引擎已经绑定同一条持久化会话，消息由引擎统一写入，
        // 避免 UI 和引擎重复插入 user/assistant 历史。
        if self.should_persist_messages_from_tui() {
            if let Err(e) = self
                .session_manager
                .add_message(MessageRole::User, &content)
            {
                warn!("Failed to save user message: {}", e);
            }
        }

        // 更新会话标题（基于第一条用户消息）
        if self.session_manager.current_session_title() == "New Session" {
            let title = self.session_manager.generate_title(&self.messages);
            if let Err(e) = self.session_manager.update_title(&title) {
                warn!("Failed to update session title: {}", e);
            }
        }

        // 标记正在查询
        self.is_querying = true;
        self.stream_started_at = Some(std::time::Instant::now());

        // Only auto-scroll when pinned
        if self.pinned_to_bottom {
            self.scroll_to_bottom();
        }

        // 使用流式引擎发送查询
        if let Some(engine) = self.streaming_engine.clone() {
            // 清空当前响应缓冲
            {
                let mut resp = self.current_response.lock().await;
                resp.clear();
            }
            {
                let mut tool_runs = self.tool_runs.lock().await;
                tool_runs.clear();
            }
            {
                let mut usage = self.stream_usage.lock().await;
                *usage = None;
            }
            self.tool_runs_snapshot.clear();
            self.current_tool_anchor_id = Some(user_msg_id);
            self.stream_usage_snapshot = None;
            self.runtime_state_snapshot = self.build_runtime_state_snapshot();
            self.sync_context_runtime_state().await;
            // 标记流未完成
            self.stream_done.store(false, Ordering::SeqCst);

            // 创建助手消息占位符
            let assistant_msg = MessageItem {
                id: format!("msg_{}", self.messages.len()),
                role: MessageRole::Assistant,
                content: String::new(),
                timestamp: std::time::SystemTime::now(),
                metadata: Default::default(),
            };
            self.messages.push(assistant_msg);
            self.scroll_to_bottom();

            // 启动流式查询（在后台任务中）
            let engine_clone = engine.clone();
            let response_clone = self.current_response.clone();
            let tool_runs_clone = self.tool_runs.clone();
            let usage_clone = self.stream_usage.clone();
            let done_flag = self.stream_done.clone();
            let user_msg = content.clone();
            let agent_mode = self.agent_mode;

            let handle = tokio::spawn(async move {
                let mut stream = engine_clone
                    .query_stream_with_agent_mode(user_msg, agent_mode)
                    .await;

                while let Some(event) = stream.next().await {
                    match event {
                        StreamEvent::TextChunk(text) => {
                            let mut resp = response_clone.lock().await;
                            resp.push_str(&text);
                        }
                        StreamEvent::ToolCallStart { id, name } => {
                            let mut runs = tool_runs_clone.lock().await;
                            upsert_tool_run(&mut runs, id, name);
                        }
                        StreamEvent::ToolCallArgs { id, args_delta } => {
                            let mut runs = tool_runs_clone.lock().await;
                            with_tool_run(&mut runs, &id, |run| run.push_args_delta(&args_delta));
                        }
                        StreamEvent::ToolExecutionStart { id, name, .. } => {
                            let mut runs = tool_runs_clone.lock().await;
                            with_tool_run(&mut runs, &id, |run| run.mark_running(name));
                        }
                        StreamEvent::ToolExecutionProgress { id, progress } => {
                            let mut runs = tool_runs_clone.lock().await;
                            with_tool_run(&mut runs, &id, |run| run.push_progress(progress));
                        }
                        StreamEvent::ToolExecutionComplete {
                            id,
                            result,
                            metadata,
                        } => {
                            let mut runs = tool_runs_clone.lock().await;
                            with_tool_run(&mut runs, &id, |run| {
                                run.mark_complete_with_metadata(result, metadata)
                            });
                        }
                        StreamEvent::Complete => {
                            done_flag.store(true, Ordering::SeqCst);
                            break;
                        }
                        StreamEvent::PermissionRequest {
                            id,
                            tool_name,
                            arguments,
                            prompt: _,
                            ..
                        } => {
                            let mut runs = tool_runs_clone.lock().await;
                            with_tool_run(&mut runs, &id, |run| {
                                run.mark_waiting_permission(tool_name, arguments)
                            });
                        }
                        StreamEvent::Usage {
                            prompt_tokens,
                            completion_tokens,
                            reasoning_tokens,
                            cached_tokens,
                        } => {
                            let mut usage = usage_clone.lock().await;
                            *usage = Some(StreamUsageSnapshot {
                                prompt_tokens,
                                completion_tokens,
                                reasoning_tokens,
                                cached_tokens,
                            });
                        }
                        StreamEvent::Error(e) => {
                            let mut resp = response_clone.lock().await;
                            resp.push_str(&format!("\n[Error: {}]", e));
                            done_flag.store(true, Ordering::SeqCst);
                            break;
                        }
                        _ => {}
                    }
                }
                // 确保即使流结束也标记完成
                done_flag.store(true, Ordering::SeqCst);
            });
            self.stream_handle = Some(handle);
        } else {
            // 没有引擎，使用占位响应
            self.add_assistant_response(format!(
                "AI engine not available. Set one provider key: {}.",
                crate::services::api::provider::provider_key_env_hint()
            ))
            .await;
        }
    }

    fn should_persist_messages_from_tui(&self) -> bool {
        let Some(engine) = &self.streaming_engine else {
            return true;
        };
        let Some((_store, session_id)) = engine.session_binding() else {
            return true;
        };
        !self.session_manager.is_current_session(&session_id)
    }

    /// 刷新当前响应（从缓冲区读取最新的流式内容，带打字机效果）
    pub async fn refresh_response(&mut self) {
        if !self.is_querying {
            return;
        }

        // 读取响应长度（最小化锁持有时间，避免克隆整个字符串）
        let total_chars = {
            let resp = self.current_response.lock().await;
            resp.chars().count()
        };

        // 更新打字机位置
        if self.typewriter_position < total_chars {
            let remaining = total_chars - self.typewriter_position;
            self.typewriter_position += remaining.min(12); // ~48 chars/sec at 4Hz tick
        }

        // 读取需要显示的内容和工具状态
        let (display_response, tool_runs_snapshot) = {
            let resp = self.current_response.lock().await;
            let tool_runs = self.tool_runs.lock().await;
            let display: String = resp.chars().take(self.typewriter_position).collect();
            (display, tool_runs.clone())
        };
        self.tool_runs_snapshot = tool_runs_snapshot;
        if let Some(anchor_id) = &self.current_tool_anchor_id {
            if self.tool_runs_snapshot.is_empty() {
                self.tool_runs_by_message_id.remove(anchor_id);
            } else {
                self.tool_runs_by_message_id
                    .insert(anchor_id.clone(), self.tool_runs_snapshot.clone());
            }
        }
        self.stream_usage_snapshot = *self.stream_usage.lock().await;
        self.runtime_state_snapshot = self.build_runtime_state_snapshot();
        self.sync_context_runtime_state().await;

        // 更新最后一条助手消息
        if let Some(last_msg) = self.messages.last_mut() {
            if last_msg.role == MessageRole::Assistant {
                last_msg.content = display_response;
            }
        }

        self.scroll_to_bottom();
    }

    /// 定时更新 - 处理流式响应刷新和计划审批检查
    pub async fn on_tick(&mut self) {
        self.tick_count += 1;

        if self.is_querying {
            self.refresh_response().await;

            // 使用 AtomicBool 检测流是否完成（由后台任务设置）
            if self.stream_done.load(Ordering::SeqCst) {
                // 确保显示完整内容（跳过打字机效果的剩余部分）
                let mut final_response_to_persist = None;
                if let Some(last_msg) = self.messages.last_mut() {
                    if last_msg.role == MessageRole::Assistant {
                        let response = self.current_response.lock().await.clone();
                        self.tool_runs_snapshot = self.tool_runs.lock().await.clone();
                        if let Some(anchor_id) = &self.current_tool_anchor_id {
                            if self.tool_runs_snapshot.is_empty() {
                                self.tool_runs_by_message_id.remove(anchor_id);
                            } else {
                                self.tool_runs_by_message_id
                                    .insert(anchor_id.clone(), self.tool_runs_snapshot.clone());
                            }
                        }
                        self.stream_usage_snapshot = *self.stream_usage.lock().await;
                        last_msg.content = response;
                        final_response_to_persist = Some(last_msg.content.clone());
                    }
                }
                self.runtime_state_snapshot = self.build_runtime_state_snapshot();
                self.sync_context_runtime_state().await;
                let final_response_for_outcome =
                    final_response_to_persist.clone().unwrap_or_default();
                if self.should_persist_messages_from_tui() {
                    if let Some(response) = final_response_to_persist {
                        if let Err(e) = self
                            .session_manager
                            .add_message(MessageRole::Assistant, &response)
                        {
                            warn!("Failed to save assistant message: {}", e);
                        }
                    }
                }
                self.record_pending_skill_outcomes(&final_response_for_outcome);
                self.typewriter_position = 0;
                // 流式响应完成，发送终端通知
                crate::tui::notify::send_notification("Priority Agent", "Response ready");
                self.is_querying = false;
                self.stream_started_at = None;
                self.current_tool_anchor_id = None;
            }
        }

        // 检查是否有待审批的计划（仅在 Chat 模式下）
        if self.mode == AppMode::Chat && self.pending_plan.is_none() {
            self.check_pending_plan().await;
        }

        // 检查是否有待审批的工具权限请求（仅在 Chat 模式下）
        if self.mode == AppMode::Chat && self.pending_permission_request.is_none() {
            self.check_pending_permission_request().await;
        }

        // 检查是否有待回答的用户问题（仅在 Chat 模式下）
        if self.mode == AppMode::Chat && self.pending_question.is_none() {
            self.check_pending_question().await;
        }

        // 更新 Plan Mode 状态标签缓存
        self.update_plan_mode_label().await;
    }

    /// 异步更新 Plan Mode 状态标签缓存
    async fn update_plan_mode_label(&mut self) {
        let plan_manager = &crate::engine::plan_mode::GLOBAL_PLAN_MANAGER;
        let state = plan_manager.get_state().await;
        self.plan_mode_label = match state {
            crate::engine::plan_mode::PlanModeState::Off => None,
            crate::engine::plan_mode::PlanModeState::Generating => {
                Some("[PLAN: generating]".to_string())
            }
            crate::engine::plan_mode::PlanModeState::Clarifying { ref question } => {
                let q = if question.len() > 20 {
                    format!("{}...", &question[..20])
                } else {
                    question.clone()
                };
                Some(format!("[PLAN: clarifying \"{}\"]", q))
            }
            crate::engine::plan_mode::PlanModeState::WaitingApproval => {
                Some("[PLAN: awaiting approval]".to_string())
            }
            crate::engine::plan_mode::PlanModeState::Executing { current_step } => {
                Some(format!("[PLAN: step {}]", current_step + 1))
            }
            crate::engine::plan_mode::PlanModeState::Completed => Some("[PLAN: done]".to_string()),
            crate::engine::plan_mode::PlanModeState::Rejected => None,
        };
    }

    /// 检查是否有待审批的计划
    async fn check_pending_plan(&mut self) {
        let plan_manager = &crate::engine::plan_mode::GLOBAL_PLAN_MANAGER;
        if !plan_manager.approval_channel().has_pending().await {
            return;
        }

        if let Some((plan, tx)) = plan_manager.approval_channel().take_pending().await {
            info!("CLI received pending plan: {}", plan.title);
            self.pending_plan = Some(plan);
            self.plan_response_tx = Some(tx);
            self.plan_modification_input.clear();
            self.mode = AppMode::PlanApproval;
        }
    }

    /// 响应计划审批
    pub fn respond_to_plan(&mut self, approval: crate::engine::plan_mode::PlanApproval) {
        if let Some(tx) = self.plan_response_tx.take() {
            let _ = tx.send(approval);
        }
        self.pending_plan = None;
        self.plan_modification_input.clear();
        self.mode = AppMode::Chat;
    }

    /// 获取 Plan Mode 状态标签（用于状态栏显示，返回缓存值）
    pub fn plan_mode_status_label(&self) -> Option<String> {
        self.plan_mode_label.clone()
    }

    /// 检查是否有待审批的工具权限请求
    async fn check_pending_permission_request(&mut self) {
        let Some(ref engine) = self.streaming_engine else {
            return;
        };
        let Some(ref channel) = engine.approval_channel() else {
            return;
        };

        if !channel.has_pending().await {
            return;
        }

        if let Some((request, tx)) = channel.take_pending().await {
            info!(
                "CLI received pending permission request: {}",
                request.prompt
            );
            self.pending_permission_request = Some(request);
            self.permission_response_tx = Some(tx);
            self.mode = AppMode::PermissionApproval;
        }
    }

    /// 响应工具权限审批
    pub fn respond_to_permission(&mut self, approved: bool) {
        self.respond_to_permission_with_rule(approved, None, None);
    }

    pub fn respond_to_permission_with_rule(
        &mut self,
        approved: bool,
        decision: Option<&str>,
        scope: Option<RuleSource>,
    ) {
        let mut rule_note = None;
        let mut response = if approved {
            ToolApprovalResponse::approved_once()
        } else {
            ToolApprovalResponse::rejected_once()
        };
        if let Some(ref req) = self.pending_permission_request {
            let pattern = permission_rule_pattern(&req.tool_call.name, &req.tool_call.arguments);
            if let Some(review_decision) =
                permission_review_decision_for_response(approved, decision, scope)
            {
                response =
                    ToolApprovalResponse::with_rule(review_decision, pattern.clone(), None, None);
            }
            if let (Some(decision), Some(scope)) = (decision, scope) {
                match scope {
                    RuleSource::User => {
                        if let Some(engine) = &self.streaming_engine {
                            engine.add_session_permission_rule(decision, &pattern);
                            let note =
                                format!("Session permission rule saved: {} {}", decision, pattern);
                            response.note = Some(note.clone());
                            rule_note = Some(note);
                        }
                    }
                    RuleSource::Project | RuleSource::Global => {
                        let cwd = std::env::current_dir()
                            .unwrap_or_else(|_| std::path::PathBuf::from("."));
                        match persist_permission_rule(scope, decision, &pattern, &cwd) {
                            Ok(path) => {
                                response.persisted_path = Some(path.display().to_string());
                                let note = format!(
                                    "Permission rule saved to {}: {} {}",
                                    path.display(),
                                    decision,
                                    pattern
                                );
                                response.note = Some(note.clone());
                                rule_note = Some(note);
                            }
                            Err(err) => {
                                let note = format!("Failed to save permission rule: {}", err);
                                response.note = Some(note.clone());
                                rule_note = Some(note);
                            }
                        }
                    }
                    RuleSource::System => {}
                }
            }
            let log_msg = format!(
                "Permission {} for tool '{}' with arguments: {}",
                if approved { "approved" } else { "denied" },
                req.tool_call.name,
                serde_json::to_string(&req.tool_call.arguments).unwrap_or_default()
            );
            let _ = self
                .session_manager
                .add_message(MessageRole::System, &log_msg);
        }
        if let Some(note) = rule_note {
            self.add_system_message(note);
        }
        if let Some(tx) = self.permission_response_tx.take() {
            let _ = tx.send(response);
        }
        self.pending_permission_request = None;
        self.mode = AppMode::Chat;
    }

    /// 计算待审批工具的 Diff 预览
    pub fn compute_permission_diff(&self) -> Option<(String, String)> {
        let req = self.pending_permission_request.as_ref()?;
        let name = req.tool_call.name.as_str();
        let args = &req.tool_call.arguments;

        match name {
            "file_write" => {
                let path = args["path"].as_str().unwrap_or("unknown");
                let content = args["content"].as_str().unwrap_or("");
                let line_count = content.lines().count();
                let mut lines = vec![
                    format!("--- /dev/null"),
                    format!("+++ b/{}", path),
                    format!("@@ -0,0 +1,{} @@", line_count),
                ];
                for line in content.lines() {
                    lines.push(format!("+{}", line));
                }
                Some((format!("Preview: {}", path), lines.join("\n")))
            }
            "file_edit" => {
                let path = args["path"].as_str().unwrap_or("unknown");
                // 尝试读取原始文件并生成真实的 unified diff
                if let Ok(original) = std::fs::read_to_string(path) {
                    if let Ok(new_content) =
                        crate::tools::file_tool::FileEditTool::preview_edit(args, &original)
                    {
                        if let Some(diff) = generate_unified_diff(&original, &new_content, path) {
                            return Some((format!("Diff: {}", path), diff));
                        }
                    }
                }
                // 回退：显示旧版本的参数展示
                let old_string = args["old_string"].as_str().unwrap_or("");
                let new_string = args["new_string"].as_str().unwrap_or("");
                let insert_after = args["insert_after"].as_str();
                let insert_before = args["insert_before"].as_str();

                let mut lines = vec![format!("File: {}", path), "".to_string()];

                if let Some(after) = insert_after {
                    lines.push("Insert after:".to_string());
                    lines.push(format!("  {}", after));
                    lines.push("New text:".to_string());
                    for line in new_string.lines() {
                        lines.push(format!("  {}", line));
                    }
                } else if let Some(before) = insert_before {
                    lines.push("Insert before:".to_string());
                    lines.push(format!("  {}", before));
                    lines.push("New text:".to_string());
                    for line in new_string.lines() {
                        lines.push(format!("  {}", line));
                    }
                } else {
                    lines.push("--- old_string ---".to_string());
                    for line in old_string.lines() {
                        lines.push(format!("-{}", line));
                    }
                    lines.push("".to_string());
                    lines.push("+++ new_string +++".to_string());
                    for line in new_string.lines() {
                        lines.push(format!("+{}", line));
                    }
                }
                Some((format!("Preview: {}", path), lines.join("\n")))
            }
            "bash" => {
                let command = args["command"].as_str().unwrap_or("");
                let working_dir = args["working_dir"].as_str().unwrap_or("current directory");
                let mut lines = vec![
                    format!("Command: {}", command),
                    format!("Working directory: {}", working_dir),
                ];
                if let Some(timeout) = args["timeout"].as_u64() {
                    lines.push(format!("Timeout: {}s", timeout));
                }
                Some(("Preview: bash command".to_string(), lines.join("\n")))
            }
            _ => None,
        }
    }
}

/// 生成 unified diff（通过 diff -u 命令）
fn generate_unified_diff(old_content: &str, new_content: &str, path: &str) -> Option<String> {
    let old_file = std::env::temp_dir().join(format!("diff_old_{}", uuid::Uuid::new_v4()));
    let new_file = std::env::temp_dir().join(format!("diff_new_{}", uuid::Uuid::new_v4()));

    std::fs::write(&old_file, old_content).ok()?;
    std::fs::write(&new_file, new_content).ok()?;

    let output = std::process::Command::new("diff")
        .args(["-u", old_file.to_str()?, new_file.to_str()?])
        .output()
        .ok()?;

    let _ = std::fs::remove_file(&old_file).ok();
    let _ = std::fs::remove_file(&new_file).ok();

    let diff = String::from_utf8_lossy(&output.stdout);
    if diff.is_empty() {
        Some(format!("No differences in {}", path))
    } else {
        Some(diff.to_string())
    }
}

impl TuiApp {
    /// 检查是否有待回答的用户问题
    async fn check_pending_question(&mut self) {
        let Some(ref engine) = self.streaming_engine else {
            return;
        };
        let Some(ref channel) = engine.tool_registry().ask_channel() else {
            return;
        };

        if let Some((question, options, tx)) = channel.take_pending().await {
            info!("CLI received pending question: {}", question);
            self.pending_question = Some(question);
            self.pending_question_options = options;
            self.question_response_tx = Some(tx);
            self.mode = AppMode::AskUser;
            self.input.clear();
        }
    }

    /// 响应用户问题
    pub fn respond_to_question(&mut self, answer: String) {
        if let Some(tx) = self.question_response_tx.take() {
            let _ = tx.send(answer);
        }
        self.pending_question = None;
        self.pending_question_options.clear();
        self.mode = AppMode::Chat;
        self.input.clear();
    }

    /// 构建工具上下文（复用 LSP / Worktree 管理器注入）
    pub(crate) async fn build_tool_context(&self) -> crate::tools::ToolContext {
        let session_id = self.session_manager.current_session_id().unwrap_or("tui");
        let working_dir = if let Some(ref wt) = self.worktree_manager {
            wt.current_worktree().await.unwrap_or_else(|| {
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
            })
        } else {
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
        };
        let mut context = crate::tools::ToolContext::new(working_dir, session_id);
        context = context.with_session_store(self.session_manager.store());
        if let Some(ref lsp) = self.lsp_manager {
            context = context.with_lsp_manager(lsp.clone());
        }
        if let Some(ref wt) = self.worktree_manager {
            context = context.with_worktree_manager(wt.clone());
        }
        context = context.with_file_cache(crate::tools::file_cache::GLOBAL_FILE_CACHE.clone());
        if let Some(ref engine) = self.streaming_engine {
            context.permission_context.mode = engine.permission_mode();
            let session_rules = engine.session_permission_rules();
            context
                .permission_context
                .rules
                .always_allow
                .extend(session_rules.always_allow);
            context
                .permission_context
                .rules
                .always_deny
                .extend(session_rules.always_deny);
            context
                .permission_context
                .rules
                .always_ask
                .extend(session_rules.always_ask);
            context = context.with_cost_tracker(engine.cost_tracker().clone());
            context = context
                .with_llm_provider(engine.provider())
                .with_model(engine.model_name());
            if let Some(agent_manager) = engine.agent_manager() {
                context = context.with_agent_manager(agent_manager);
            }
            if let Some(mcp_manager) = engine.mcp_manager() {
                context = context.with_mcp_manager(mcp_manager);
            }
            if let Some(memory_manager) = engine.memory_manager() {
                context = context.with_memory_manager(memory_manager);
            }
            if let Some(tracker) = engine.read_tracker() {
                context = context.with_read_tracker(tracker.clone());
            }
        }
        context
    }

    /// 处理斜杠命令
    async fn handle_slash_command(&mut self, input: &str) {
        let parts: Vec<&str> = input.trim().splitn(2, ' ').collect();
        let cmd = parts[0].to_lowercase();
        let args = parts.get(1).unwrap_or(&"");

        use crate::tui::slash_handler as slash;

        let response = match cmd.as_str() {
            "/help" | "/h" => {
                if args.trim() == "maturity" {
                    return self.add_system_message(self.command_registry.maturity_report());
                }
                let mut help = self.command_registry.help_text();
                help.push_str("\n\nSession Commands:\n");
                help.push_str("  /sessions    - List recent sessions\n");
                help.push_str("  /session     - Show current session or restore by number/ID\n");
                help.push_str("  /new         - Start a new session\n");
                help.push_str("  /export      - Export current session to JSON\n");
                help.push_str("  /search      - Search through all sessions\n");
                help.push_str("  /stats       - Show session statistics\n");
                help.push_str("\nSettings:\n");
                help.push_str("  /settings    - Open settings interface\n");
                help.push_str("  /permissions - View/update permission mode and policy rules\n");
                help.push_str("  /mcp         - Manage MCP server approvals\n");
                help.push_str("  /voice       - Check voice TTS/STT status\n");
                help.push_str("  /telemetry   - View telemetry status\n");
                help.push_str("  /onboarding  - Restart the onboarding guide\n");
                help.push_str("\nThe agent has 30+ tools (file, bash, web, github, memory, cron, swarm, MCP, skills, project).\nJust ask naturally - the agent will use the right tools.");
                help
            }
            "/clear" => {
                if let Some(ref engine) = self.streaming_engine {
                    engine.clear_history().await;
                }
                self.messages.clear();
                self.clear_tool_transcript();
                "Conversation history cleared.".to_string()
            }
            "/memory" => {
                let query = args.trim();
                let maintain = query == "--maintain";
                let doctor = matches!(
                    query,
                    "--doctor" | "doctor" | "doctor json" | "doctor --json" | "--doctor json"
                );
                let doctor_json =
                    matches!(query, "doctor json" | "doctor --json" | "--doctor json");
                let (memory_action, memory_arg) = query
                    .split_once(' ')
                    .map(|(action, rest)| (action, rest.trim()))
                    .unwrap_or((query, ""));
                let latest_user_message = self
                    .messages
                    .iter()
                    .rev()
                    .find(|m| m.role == MessageRole::User)
                    .map(|m| m.content.as_str())
                    .unwrap_or("");

                let memory_manager = if let Some(ref engine) = self.streaming_engine {
                    engine.memory_manager()
                } else {
                    None
                };

                if let Some(memory_manager) = memory_manager {
                    let mem = memory_manager.lock().await;
                    if maintain {
                        let report = mem.maintain_memory();
                        report.format()
                    } else if doctor {
                        let summary = mem.memory_summary();
                        let decisions = mem.memory_decision_counts();
                        let flushes = mem.memory_flush_summary();
                        let calibration = crate::memory::run_memory_calibration_samples();
                        let eval_suite = crate::memory::run_memory_eval_suite();
                        let calibration_passed =
                            calibration.iter().filter(|result| result.passed).count();
                        let conflicts = mem.memory_conflicts(8);
                        if doctor_json {
                            let snapshot = mem.memory_snapshot_report();
                            let proposal_queue = memory_proposal_queue_json();
                            serde_json::json!({
                                "summary": {
                                    "project_memory_chars": summary.project_memory_chars,
                                    "project_memory_files": summary.project_memory_files,
                                    "project_memory_file_chars": summary.project_memory_file_chars,
                                    "user_memory_chars": summary.user_memory_chars,
                                    "session_memory_items": summary.session_memory_items,
                                    "has_frozen_snapshot": summary.has_frozen_snapshot,
                                },
                                "snapshot": snapshot,
                                "proposal_queue": proposal_queue,
                                "decisions": decisions,
                                "flushes": flushes,
                                "quality_gates": {
                                    "accept_threshold": 0.65,
                                    "propose_threshold": 0.45,
                                    "explicit_override_threshold": 0.60,
                                    "hard_stops": ["unsafe_content", "secret_like_content", "duplicate_memory"],
                                },
                                "calibration": {
                                    "passed": calibration_passed,
                                    "total": calibration.len(),
                                    "results": calibration,
                                },
                                "eval_suite": eval_suite,
                                "conflicts": conflicts,
                            })
                            .to_string()
                        } else {
                            let snapshot = mem.memory_snapshot_report();
                            let proposal_queue = format_memory_proposal_queue();
                            format!(
                                "# Memory Doctor\n\n{}\n\n{}\n\n{}\n\nDecisions:\n  Accepted: {}\n  Proposed: {}\n  Rejected: {}\n  Blocked: {}\n\n{}\n\nQuality gates:\n  accept>=0.65 · propose>=0.45 · explicit>=0.60 with safety/duplicate hard stops\n\nCalibration: {}/{} passed\nMemory evals: {}/{} passed",
                                summary.format(),
                                format_memory_snapshot_report(&snapshot),
                                proposal_queue,
                                decisions.accepted,
                                decisions.proposed,
                                decisions.rejected,
                                decisions.blocked,
                                flushes.format(),
                                calibration_passed,
                                calibration.len(),
                                eval_suite.passed,
                                eval_suite.total
                            )
                        }
                    } else if memory_action == "snapshot" {
                        format_memory_snapshot_report(&mem.memory_snapshot_report())
                    } else if memory_action == "eval" {
                        crate::memory::run_memory_eval_suite().format()
                    } else if memory_action == "records" {
                        format_memory_records(&mem.memory_records(), memory_arg)
                    } else if memory_action == "migrate" {
                        format_memory_migration_command(&mem, memory_arg)
                    } else if memory_action == "repair-proposals" {
                        let limit = memory_arg.parse::<usize>().ok().unwrap_or(20).clamp(1, 200);
                        let created = mem.upsert_projection_repair_proposals(limit);
                        format!(
                            "Memory repair proposal scan complete\n- projection drift proposals: {}\n- review: /memory-proposals list --source repair",
                            created
                        )
                    } else if memory_action == "conflicts" {
                        let conflicts = mem.memory_conflicts(20);
                        if conflicts.is_empty() {
                            "Memory conflicts: none".to_string()
                        } else {
                            format!("Memory Conflicts\n{}", conflicts.join("\n"))
                        }
                    } else if memory_action == "review" {
                        let review = mem.memory_review_report(8);
                        let decisions = mem.memory_decision_counts();
                        let flushes = mem.memory_flush_summary();
                        let conflicts = mem.memory_conflicts(8);
                        format!(
                            "Memory Review\n\n{}\n\nDecisions: {} accepted · {} proposed · {} rejected · {} blocked\n{}\n\nConflicts:\n{}",
                            review.format(),
                            decisions.accepted,
                            decisions.proposed,
                            decisions.rejected,
                            decisions.blocked,
                            flushes.format(),
                            if conflicts.is_empty() {
                                "none".to_string()
                            } else {
                                conflicts.join("\n")
                            }
                        )
                    } else if memory_action == "search" {
                        let search_query = if memory_arg.is_empty() {
                            latest_user_message
                        } else {
                            memory_arg
                        };
                        match mem.preview_retrieval_context(
                            search_query,
                            8,
                            crate::engine::intent_router::RetrievalPolicy::Memory,
                        ) {
                            Some(ctx) => {
                                if let Err(error) =
                                    crate::tools::memory_tool::record_last_memory_retrieval_trace(
                                        &ctx,
                                    )
                                {
                                    warn!("failed to write last memory retrieval trace: {}", error);
                                }
                                format_memory_retrieval_context(&ctx)
                            }
                            None => format!("No memory retrieval hits for '{}'.", search_query),
                        }
                    } else if memory_action == "explain" {
                        if memory_arg.is_empty() {
                            "Usage: /memory explain <retrieval-id-or-source>".to_string()
                        } else {
                            let search_query = if latest_user_message.is_empty() {
                                memory_arg
                            } else {
                                latest_user_message
                            };
                            match mem.preview_retrieval_context(
                                search_query,
                                20,
                                crate::engine::intent_router::RetrievalPolicy::Memory,
                            ) {
                                Some(ctx) => explain_memory_retrieval_item(&ctx, memory_arg),
                                None => format!(
                                    "No memory retrieval context available for '{}'.",
                                    search_query
                                ),
                            }
                        }
                    } else {
                        let summary = mem.memory_summary();
                        let project = mem.load_tier(crate::memory::manager::MemoryTier::Project);
                        let user = mem.load_tier(crate::memory::manager::MemoryTier::User);
                        let preview_query = if query.is_empty() {
                            latest_user_message
                        } else {
                            query
                        };
                        let relevant = mem.preview_relevant_memories(preview_query, 5);

                        let mut lines = vec![
                            "# Memory".to_string(),
                            "".to_string(),
                            summary.format(),
                            "".to_string(),
                        ];

                        if !query.is_empty() {
                            let hits = mem.search(query);
                            lines.push("## Search".to_string());
                            if hits.is_empty() {
                                lines.push(format!("No memories matching '{}'.", query));
                            } else {
                                for hit in hits {
                                    let hit = hit.lines().take(4).collect::<Vec<_>>().join(" ");
                                    lines.push(format!(
                                        "- {}",
                                        hit.chars().take(220).collect::<String>()
                                    ));
                                }
                            }
                            lines.push("".to_string());
                        }

                        if !relevant.is_empty() {
                            lines.push("## Relevant Preview".to_string());
                            for item in relevant {
                                let snippet = item
                                    .snippet
                                    .lines()
                                    .map(str::trim)
                                    .filter(|line| !line.is_empty())
                                    .take(2)
                                    .collect::<Vec<_>>()
                                    .join(" ");
                                lines.push(format!(
                                    "- {} (score {}): {}",
                                    item.source,
                                    item.score,
                                    snippet.chars().take(220).collect::<String>()
                                ));
                            }
                            lines.push("".to_string());
                        }

                        if !project.trim().is_empty() {
                            lines.push("## Project Memory Index".to_string());
                            lines.push(project.chars().take(1800).collect());
                            lines.push("".to_string());
                        }
                        if !user.trim().is_empty() {
                            lines.push("## User Preferences".to_string());
                            lines.push(user.chars().take(1000).collect());
                        }

                        if lines.len() <= 4 {
                            "No memory saved yet. Use /save <text> to save.".to_string()
                        } else {
                            lines.join("\n")
                        }
                    }
                } else {
                    let mut mem = crate::memory::MemoryManager::new();
                    mem.freeze_snapshot();
                    if maintain {
                        let report = mem.maintain_memory();
                        report.format()
                    } else if doctor {
                        let summary = mem.memory_summary();
                        let decisions = mem.memory_decision_counts();
                        let flushes = mem.memory_flush_summary();
                        let calibration = crate::memory::run_memory_calibration_samples();
                        let eval_suite = crate::memory::run_memory_eval_suite();
                        let calibration_passed =
                            calibration.iter().filter(|result| result.passed).count();
                        if doctor_json {
                            let snapshot = mem.memory_snapshot_report();
                            let proposal_queue = memory_proposal_queue_json();
                            serde_json::json!({
                                "summary": {
                                    "project_memory_chars": summary.project_memory_chars,
                                    "project_memory_files": summary.project_memory_files,
                                    "project_memory_file_chars": summary.project_memory_file_chars,
                                    "user_memory_chars": summary.user_memory_chars,
                                    "session_memory_items": summary.session_memory_items,
                                    "has_frozen_snapshot": summary.has_frozen_snapshot,
                                },
                                "snapshot": snapshot,
                                "proposal_queue": proposal_queue,
                                "decisions": decisions,
                                "flushes": flushes,
                                "quality_gates": {
                                    "accept_threshold": 0.65,
                                    "propose_threshold": 0.45,
                                    "explicit_override_threshold": 0.60,
                                    "hard_stops": ["unsafe_content", "secret_like_content", "duplicate_memory"],
                                },
                                "calibration": {
                                    "passed": calibration_passed,
                                    "total": calibration.len(),
                                    "results": calibration,
                                },
                                "eval_suite": eval_suite,
                                "conflicts": mem.memory_conflicts(8),
                            })
                            .to_string()
                        } else {
                            let snapshot = mem.memory_snapshot_report();
                            let proposal_queue = format_memory_proposal_queue();
                            format!(
                                "# Memory Doctor\n\n{}\n\n{}\n\n{}\n\nDecisions:\n  Accepted: {}\n  Proposed: {}\n  Rejected: {}\n  Blocked: {}\n\n{}\n\nQuality gates:\n  accept>=0.65 · propose>=0.45 · explicit>=0.60 with safety/duplicate hard stops\n\nCalibration: {}/{} passed\nMemory evals: {}/{} passed",
                                summary.format(),
                                format_memory_snapshot_report(&snapshot),
                                proposal_queue,
                                decisions.accepted,
                                decisions.proposed,
                                decisions.rejected,
                                decisions.blocked,
                                flushes.format(),
                                calibration_passed,
                                calibration.len(),
                                eval_suite.passed,
                                eval_suite.total
                            )
                        }
                    } else if memory_action == "snapshot" {
                        format_memory_snapshot_report(&mem.memory_snapshot_report())
                    } else if memory_action == "eval" {
                        crate::memory::run_memory_eval_suite().format()
                    } else if memory_action == "records" {
                        format_memory_records(&mem.memory_records(), memory_arg)
                    } else if memory_action == "migrate" {
                        format_memory_migration_command(&mem, memory_arg)
                    } else if memory_action == "repair-proposals" {
                        let limit = memory_arg.parse::<usize>().ok().unwrap_or(20).clamp(1, 200);
                        let created = mem.upsert_projection_repair_proposals(limit);
                        format!(
                            "Memory repair proposal scan complete\n- projection drift proposals: {}\n- review: /memory-proposals list --source repair",
                            created
                        )
                    } else {
                        let summary = mem.memory_summary();
                        let project = mem.load_tier(crate::memory::manager::MemoryTier::Project);
                        if project.trim().is_empty() {
                            "No memory saved yet. Use /save <text> to save.".to_string()
                        } else {
                            format!("# Memory\n\n{}\n\n{}", summary.format(), project)
                        }
                    }
                }
            }
            "/save" => {
                if args.is_empty() {
                    "Usage: /save <text> | /save --topic <name> <text> | /save --user <text>"
                        .to_string()
                } else {
                    let (save_target, save_topic, save_content) = parse_memory_save_args(args);
                    if save_content.trim().is_empty() {
                        "Usage: /save <text> | /save --topic <name> <text> | /save --user <text>"
                            .to_string()
                    } else {
                        let memory_manager = if let Some(ref engine) = self.streaming_engine {
                            engine.memory_manager()
                        } else {
                            None
                        };

                        if let Some(memory_manager) = memory_manager {
                            let mem = memory_manager.lock().await;
                            let outcome = match save_target {
                                MemorySaveTarget::User => {
                                    mem.add_learning_async(save_content, "preference").await
                                }
                                MemorySaveTarget::Topic => {
                                    mem.add_topic_learning_async(
                                        save_content,
                                        "note",
                                        save_topic.unwrap_or("notes"),
                                    )
                                    .await
                                }
                                MemorySaveTarget::Auto => {
                                    mem.add_auto_learning_async(save_content, "note").await
                                }
                            };
                            format_memory_write_outcome(save_content, &outcome)
                        } else {
                            let mem = crate::memory::MemoryManager::new();
                            let outcome = match save_target {
                                MemorySaveTarget::User => {
                                    mem.add_learning_async(save_content, "preference").await
                                }
                                MemorySaveTarget::Topic => {
                                    mem.add_topic_learning_async(
                                        save_content,
                                        "note",
                                        save_topic.unwrap_or("notes"),
                                    )
                                    .await
                                }
                                MemorySaveTarget::Auto => {
                                    mem.add_auto_learning_async(save_content, "note").await
                                }
                            };
                            format_memory_write_outcome(save_content, &outcome)
                        }
                    }
                }
            }
            "/model" => {
                let args = args.trim();
                if let Some(model) = args
                    .strip_prefix("set ")
                    .or_else(|| args.strip_prefix("switch "))
                    .map(str::trim)
                    .filter(|m| !m.is_empty())
                {
                    if let Some(engine) = &self.streaming_engine {
                        engine.set_model(model.to_string());
                    }
                    if let Ok(mut config) = crate::services::config::AppConfig::load() {
                        config.api.model = model.to_string();
                        let _ = config.save();
                    }
                    self.model_notice = Some(format!("Model switched to {}", model));
                    format!("Model switched to {}. Next request will use it.", model)
                } else if args == "list" {
                    let lines = self
                        .model_choices()
                        .into_iter()
                        .map(|choice| {
                            format!(
                                "{} {} ({})",
                                if choice.active { "*" } else { "-" },
                                choice.model,
                                choice.note
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    format!("Models for {}:\n{}", self.current_provider_label(), lines)
                } else if self.streaming_engine.is_some() {
                    let model = self.current_model_label();
                    let provider = self.current_provider_label();
                    let base = self.current_provider_base_url();
                    format!(
                        "Model: {} (via {})\nBase URL: {}\n\nUse Ctrl+M for the model picker, /model list, or /model set <name>.",
                        model, provider, base
                    )
                } else {
                    "Model: unavailable (no engine connected)".to_string()
                }
            }
            "/provider" => {
                let args = args.trim();
                if let Some(provider) = args
                    .strip_prefix("set ")
                    .or_else(|| args.strip_prefix("switch "))
                    .map(str::trim)
                    .filter(|p| !p.is_empty())
                {
                    self.switch_provider_by_name(provider)
                } else if args == "list" {
                    let lines = self
                        .provider_choices()
                        .into_iter()
                        .map(|choice| {
                            format!(
                                "{} {:<10} {:<12} {:<20} {}{}",
                                if choice.active { "*" } else { "-" },
                                choice.name,
                                choice.provider_type,
                                choice.model,
                                choice.note,
                                if choice.base_url.is_empty() {
                                    String::new()
                                } else {
                                    format!(" - {}", choice.base_url)
                                }
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    format!("Providers:\n{}", lines)
                } else if self.streaming_engine.is_some() {
                    format!(
                        "Provider: {}\nModel: {}\nBase URL: {}\n\nUse Ctrl+L for the provider picker, /provider list, or /provider switch <name>.",
                        self.current_provider_label(),
                        self.current_model_label(),
                        self.current_provider_base_url()
                    )
                } else {
                    "Provider: unavailable (no engine connected)".to_string()
                }
            }
            "/status" => slash::handle_status(self).await,
            "/panel" | "/panels" | "/runtime" => slash::handle_panel(self, args).await,
            "/tool-output" | "/tool" => {
                let args = args.trim();
                if args.is_empty() || args == "latest" {
                    if self.open_tool_viewer() {
                        String::new()
                    } else {
                        "No tool output to view yet.".to_string()
                    }
                } else if args == "list" {
                    let lines = self.tool_output_index_lines();
                    if lines.is_empty() {
                        "No tool output to view yet.".to_string()
                    } else {
                        format!("Tool outputs:\n{}", lines.join("\n"))
                    }
                } else if self.open_tool_viewer_for(args) {
                    String::new()
                } else {
                    format!("Tool output '{}' not found. Use /tool-output list.", args)
                }
            }
            "/statusbar" => {
                let args = args.trim();
                if args.is_empty() {
                    format!(
                        "Status bar density: {}\nOptions: compact, normal, debug\nShortcut: Ctrl+Shift+S cycles density.",
                        self.status_bar_density.name()
                    )
                } else if let Some(density) = StatusBarDensity::parse(args) {
                    self.set_status_bar_density(density);
                    format!("Status bar density: {}", density.name())
                } else {
                    "Usage: /statusbar [compact|normal|debug]".to_string()
                }
            }
            "/resume" => slash::handle_resume(self, args).await,
            "/rewind" => slash::handle_rewind(self, args).await,
            // Phase 10 Batch 1: Session & Control Commands
            "/session" => slash::handle_session_cmd(self, args).await,
            "/undo" => slash::handle_undo(self, args),
            "/redo" => slash::handle_redo(self, args),
            "/retry" => slash::handle_retry(self, args).await,
            "/stop" => slash::handle_stop(self, args),
            "/reload" => slash::handle_reload(self, args).await,
            "/share" => slash::handle_share(self, args),
            "/cost" | "/token" => slash::handle_token(self).await,
            "/diff" => slash::handle_diff(self, args).await,
            "/quit" | "/exit" | "/q" => {
                if let Some(ref engine) = self.streaming_engine {
                    engine
                        .flush_memory_for_current_history(crate::memory::MemoryFlushReason::Exit)
                        .await;
                }
                "Use Ctrl+C to exit".to_string()
            }
            "/sessions" => slash::handle_sessions(self),
            "/new" => slash::handle_new(self).await,
            "/stats" => slash::handle_stats(self),
            "/checkpoints" => slash::handle_checkpoints(self).await,
            "/restore" | "/r" => slash::handle_restore(self, args).await,
            "/batch" => slash::handle_batch(self, args).await,
            "/settings" => {
                let config = crate::services::config::AppConfig::load().unwrap_or_default();
                self.settings_state = Some(crate::tui::components::settings::SettingsState::new(
                    config,
                    self.keybindings.clone(),
                ));
                self.mode = AppMode::Settings;
                "Entering settings mode...".to_string()
            }
            "/tools" => {
                let registry = crate::tools::ToolRegistry::default_registry();
                let context = self.build_tool_context().await;
                let mut available = Vec::new();
                let mut unavailable = Vec::new();
                for tool in registry.iter_tools() {
                    if tool.is_available(&context) {
                        available.push(tool.name().to_string());
                    } else {
                        unavailable.push(format!(
                            "{} ({})",
                            tool.name(),
                            tool.unavailable_reason(&context)
                                .unwrap_or_else(|| "unavailable".to_string())
                        ));
                    }
                }
                available.sort();
                unavailable.sort();
                let unavailable_line = if unavailable.is_empty() {
                    String::new()
                } else {
                    format!(
                        "\n\nUnavailable in this session ({}):\n{}",
                        unavailable.len(),
                        unavailable.join(", ")
                    )
                };
                format!(
                    "Available tools ({}):\n{}{}",
                    available.len(),
                    available.join(", "),
                    unavailable_line
                )
            }
            "/tasks" => slash::handle_tasks(self).await,
            "/agents" => slash::handle_agents(self, args).await,
            "/doctor" => slash::handle_doctor(self, args).await,
            "/audit" => slash::handle_audit(self, args).await,
            "/permissions" | "/perm" => slash::handle_permissions(self, args),
            "/commit" => slash::handle_commit(self).await,
            "/commit-push-pr" => slash::handle_commit_push_pr(self, args).await,
            "/review-pr" => slash::handle_review_pr(self, args).await,
            "/review" => slash::handle_review(self).await,
            "/security-review" => slash::handle_security_review(self).await,
            "/explain" => slash::handle_explain(self, args).await,
            "/fix" => slash::handle_fix(self).await,
            "/simplify" => slash::handle_simplify(self, args).await,
            "/karpathy" => slash::handle_karpathy(self, args).await,
            "/verify" => slash::handle_verify(self).await,
            "/debug" => slash::handle_debug(self).await,
            "/stuck" => slash::handle_stuck(self).await,
            "/remember" => slash::handle_remember(self, args).await,
            "/keybindings" => slash::handle_keybindings(self, args),
            "/mcp" => slash::handle_mcp(self, args).await,
            "/voice" => slash::handle_voice(),
            "/telemetry" => slash::handle_telemetry(),
            "/lsp" => slash::handle_lsp(self, args),
            "/npm" => slash::handle_npm(self, args).await,
            // Phase 10 Batch 2: hooks, profiling, prompt, migrate, focus, pause, install, skeleton, branch, color
            "/hooks" => slash::handle_hooks(self),
            "/profiling" => slash::handle_profiling(self),
            "/prompt" => slash::handle_prompt(self, args).await,
            "/migrate" => slash::handle_migrate(self, args).await,
            "/focus" => slash::handle_focus(self, args),
            "/pause" => slash::handle_pause(self, args),
            "/install" => slash::handle_install(self, args).await,
            "/skeleton" => slash::handle_skeleton(self, args),
            "/branch" => slash::handle_branch(self, args).await,
            "/color" => slash::handle_color(self, args),
            // Phase 10 Batch 3: webhook, wizard, workspace, slack, stealth, shadow, reject, subscribe, slots, ticker
            "/webhook" => slash::handle_webhook(self, args).await,
            "/wizard" => slash::handle_wizard(self),
            "/workspace" => slash::handle_workspace(self, args),
            "/slack" => slash::handle_slack(self, args).await,
            "/stealth" => slash::handle_stealth(self, args),
            "/shadow" => slash::handle_shadow(self, args),
            "/reject" => slash::handle_reject(self, args),
            "/subscribe" => slash::handle_subscribe(self, args),
            "/slots" => slash::handle_slots(self, args),
            "/ticker" => slash::handle_ticker(self, args),
            // Phase 10 Batch 4: config, copy, desktop, chrome, effort, preamble, untrap, verbose, write
            "/config" => slash::handle_config(self, args),
            "/copy" => slash::handle_copy(self, args).await,
            "/desktop" => slash::handle_desktop(self, args),
            "/chrome" => slash::handle_chrome(self, args),
            "/effort" => slash::handle_effort(self, args),
            "/preamble" => slash::handle_preamble(self, args),
            "/untrap" => slash::handle_untrap(self, args),
            "/verbose" => slash::handle_verbose(self, args),
            "/write" => slash::handle_write(self, args).await,
            "/vim" => slash::handle_vim(self),
            "/onboarding" | "/onboard" => slash::handle_onboarding(self),
            "/skip" => slash::handle_skip(self),
            // Phase 9 Task 3: New high-value commands
            "/btw" => slash::handle_btw(self, args).await,
            "/context" => slash::handle_context(self).await,
            "/git" => slash::handle_git(self, args).await,
            "/history" => slash::handle_history(self, args),
            "/mode" => slash::handle_mode(self, args),
            "/package" => slash::handle_package(self, args).await,
            // Phase 9 Task 1: Advanced Agent Types
            "/teammate" => slash::handle_teammate(self, args).await,
            "/critic" => slash::handle_critic(self, args).await,
            "/assistant" => slash::handle_assistant(self, args).await,
            "/remote" => slash::handle_remote(self, args).await,
            "/dream" => slash::handle_dream(self, args).await,
            "/custom" => slash::handle_custom(self, args).await,
            "/orchestrate" => slash::handle_orchestrate(self, args).await,
            // Phase 10 Extended: More commands
            "/rollback" => slash::handle_rollback(self, args).await,
            "/project" => slash::handle_project(self, args),
            "/backend" => slash::handle_backend(self, args),
            "/sandbox" => slash::handle_sandbox(self, args),
            "/env" => slash::handle_env(self, args),
            "/cache" => slash::handle_cache(self, args).await,
            "/benchmark" => slash::handle_benchmark(self, args).await,
            "/test" => slash::handle_test(self, args).await,
            "/trace" => slash::handle_trace(self, args),
            "/eval" => slash::handle_eval(self, args),
            "/resource" => slash::handle_resource(self),
            // Phase 10 Extended 2: More commands
            "/init" => slash::handle_init(self, args),
            "/login" => slash::handle_login(self, args),
            "/logout" => slash::handle_logout(self, args),
            "/key" => slash::handle_key(self, args),
            "/health" => slash::handle_health(self),
            "/ping" => slash::handle_ping(self),
            "/uptime" => slash::handle_uptime(self),
            "/version" => slash::handle_version(self),
            "/about" => slash::handle_about(self),
            // Phase 10 Extended 3: Session management and utility commands
            "/reset" => slash::handle_reset(self, args),
            "/export" => slash::handle_export_data(self, args).await,
            "/import" => slash::handle_import(self, args).await,
            "/save-session" => slash::handle_save_session(self),
            "/load-session" => slash::handle_load_session(self, args).await,
            "/merge" => slash::handle_merge(self, args).await,
            "/cleanup" => slash::handle_cleanup(self, args),
            "/compact" => slash::handle_compact(self).await,
            "/snippet" => slash::handle_snippet(self, args),
            "/bookmark" => slash::handle_bookmark(self, args).await,
            "/tag" => slash::handle_tag(self, args),
            "/search" => slash::handle_search(self, args),
            "/filter" => slash::handle_filter(self, args),
            // Phase 10 Final: Complete commands
            "/profile" => slash::handle_profile(self, args),
            "/theme" => slash::handle_theme(self, args),
            "/shortcuts" => slash::handle_shortcuts(self),
            "/quick" => slash::handle_quick(self),
            "/active-task" | "/progress" => slash::handle_active_task(self),
            "/goal" => slash::handle_goal(self, args),
            "/learn" => slash::handle_learn(self, args),
            "/experience" => slash::handle_experience(self, args),
            "/memory-proposals" | "/memory-proposal" => slash::handle_memory_proposals(self, args),
            "/evolution" => slash::handle_evolution(self, args),
            "/improvements" => slash::handle_improvements(self, args),
            "/skill-proposals" => slash::handle_skill_proposals(self, args),
            "/recover" => slash::handle_recover(self, args),
            "/feedback" => slash::handle_feedback(self, args),
            _ => {
                if let Some(invocation) = self.skill_runtime.invocation(&cmd, args) {
                    let skill_version = self
                        .skill_runtime
                        .get(&cmd)
                        .map(|skill| skill.meta.version.clone())
                        .unwrap_or_else(|| "unknown".to_string());
                    self.apply_skill_invocation_policy(&invocation);
                    let mut notice = format!("Skill /{} applied", invocation.name);
                    if !invocation.allowed_tools.is_empty() {
                        notice.push_str(&format!(
                            " · allowed tools: {}",
                            invocation.allowed_tools.join(", ")
                        ));
                    }
                    if !invocation.disallowed_tools.is_empty() {
                        notice.push_str(&format!(
                            " · denied tools: {}",
                            invocation.disallowed_tools.join(", ")
                        ));
                    }
                    if let Some(model) = &invocation.model {
                        notice.push_str(&format!(" · preferred model: {}", model));
                    }
                    if let Some(effort) = &invocation.effort {
                        notice.push_str(&format!(" · effort: {}", effort));
                    }
                    if let Some(context) = &invocation.context {
                        notice.push_str(&format!(" · context: {}", context));
                    }
                    self.add_system_message(notice);
                    self.record_skill_invocation_usage(&invocation.name, &skill_version);
                    self.send_message(invocation.prompt).await;
                    String::new()
                } else {
                    format!(
                        "Unknown command: {}. Type /help for available commands.",
                        cmd
                    )
                }
            }
        };

        self.add_system_message(response);
    }

    fn record_skill_invocation_usage(&mut self, skill_name: &str, skill_version: &str) {
        let event = crate::engine::skill_evolution::SkillUsageEvent {
            skill_name: skill_name.to_string(),
            skill_version: skill_version.to_string(),
            provisional: true,
            success: false,
            acceptance_passed: None,
            tests_passed: None,
            user_satisfaction: None,
            duration_ms: None,
            tool_calls: 0,
            risk_penalty: 0.05,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        let store = crate::engine::skill_evolution::SkillProposalStore::default();
        if let Err(e) = store.record_usage(&event) {
            warn!("Failed to record skill usage event: {}", e);
        }
        if let Ok(payload) = serde_json::to_value(&event) {
            let _ = self.session_manager.add_learning_event(
                "skill_usage",
                "skill_runtime",
                &format!("Skill /{} invoked", skill_name),
                0.75,
                &payload,
            );
        }
        self.pending_skill_invocations.push(PendingSkillInvocation {
            name: skill_name.to_string(),
            version: skill_version.to_string(),
            started_at: std::time::Instant::now(),
        });
    }

    fn record_pending_skill_outcomes(&mut self, assistant_response: &str) {
        if self.pending_skill_invocations.is_empty() {
            return;
        }
        let failed_tool = self
            .tool_runs_snapshot
            .iter()
            .any(|run| matches!(run.status, ToolRunStatus::Failed | ToolRunStatus::TimedOut));
        let stream_error = assistant_response.contains("[Error:");
        let has_response = !assistant_response.trim().is_empty();
        let trace = self
            .streaming_engine
            .as_ref()
            .and_then(|engine| engine.trace_store().latest())
            .or_else(|| self.session_manager.latest_trace().ok().flatten());
        let attribution =
            skill_outcome_attribution(trace.as_ref(), has_response, stream_error, failed_tool);
        let tool_calls = self.tool_runs_snapshot.len();
        let store = crate::engine::skill_evolution::SkillProposalStore::default();
        for pending in self.pending_skill_invocations.drain(..) {
            let event = crate::engine::skill_evolution::SkillUsageEvent {
                skill_name: pending.name.clone(),
                skill_version: pending.version.clone(),
                provisional: false,
                success: attribution.success,
                acceptance_passed: attribution.acceptance_passed,
                tests_passed: attribution.tests_passed,
                user_satisfaction: attribution.user_satisfaction,
                duration_ms: Some(
                    pending
                        .started_at
                        .elapsed()
                        .as_millis()
                        .min(u128::from(u64::MAX)) as u64,
                ),
                tool_calls,
                risk_penalty: attribution.risk_penalty,
                created_at: chrono::Utc::now().to_rfc3339(),
            };
            if let Err(e) = store.record_usage(&event) {
                warn!("Failed to record skill outcome event: {}", e);
            }
            if let Ok(payload) = serde_json::to_value(&event) {
                let _ = self.session_manager.add_learning_event(
                    "skill_usage",
                    "skill_runtime",
                    &format!(
                        "Skill /{} outcome inferred from {}: {}",
                        pending.name,
                        attribution.source,
                        if attribution.success {
                            "success"
                        } else {
                            "fail"
                        }
                    ),
                    f64::from(attribution.confidence),
                    &payload,
                );
            }
        }
    }

    /// 恢复会话
    pub(crate) async fn restore_session(&mut self, session_id: &str) -> String {
        if let Some(ref engine) = self.streaming_engine {
            engine
                .flush_memory_for_current_history(crate::memory::MemoryFlushReason::ResumeSwitch)
                .await;
        }
        match self.session_manager.switch_to_session(session_id) {
            Ok(messages) => {
                // 清空当前消息
                self.messages.clear();
                self.clear_tool_transcript();

                // 加载会话消息到 UI
                for msg in messages {
                    self.messages.push(msg);
                }

                // 同步恢复引擎的对话历史
                if let Some(ref engine) = self.streaming_engine {
                    match self.session_manager.load_api_messages(session_id) {
                        Ok(api_messages) => {
                            engine.set_history(api_messages).await;
                            engine.set_session_id(session_id.to_string());
                        }
                        Err(e) => {
                            warn!("Failed to restore engine history: {}", e);
                        }
                    }
                }

                let mut lines = vec![format!(
                    "Restored session {} ({} messages). Previous conversation loaded.",
                    &session_id[..8.min(session_id.len())],
                    self.messages.len()
                )];
                if let Ok(preview) = self.session_manager.recent_preview_lines(session_id, 4) {
                    if !preview.is_empty() {
                        lines.push("Recent context:".to_string());
                        lines.extend(preview);
                    }
                }
                lines.join("\n")
            }
            Err(e) => format!("Failed to restore session: {}", e),
        }
    }

    /// 退出设置模式
    pub fn exit_settings(&mut self) {
        self.mode = AppMode::Chat;
        self.settings_state = None;
    }

    /// 保存设置
    pub fn save_settings(&mut self) -> anyhow::Result<()> {
        if let Some(ref mut state) = self.settings_state {
            state.save_config()?;
            // 如果主题发生变化，同步更新 TuiApp 的主题
            self.theme = Arc::new(crate::tui::theme::Theme::from_name(&state.config.ui.theme));
        }
        Ok(())
    }

    /// 历史记录：上一条
    pub fn history_prev(&mut self) {
        if self.history.is_empty() {
            return;
        }

        let new_index = match self.history_index {
            None => Some(self.history.len() - 1),
            Some(0) => Some(0),
            Some(i) => Some(i - 1),
        };

        if let Some(idx) = new_index {
            self.history_index = new_index;
            if let Some(cmd) = self.history.get(idx) {
                self.input.set_value(cmd.clone());
            }
        }
    }

    /// 历史记录：下一条
    pub fn history_next(&mut self) {
        if self.history.is_empty() {
            self.history_index = None;
            return;
        }
        match self.history_index {
            None => {}
            Some(i) if i + 1 >= self.history.len() => {
                self.history_index = None;
                self.input.set_value(String::new());
            }
            Some(i) => {
                let new_i = i + 1;
                self.history_index = Some(new_i);
                if let Some(cmd) = self.history.get(new_i) {
                    self.input.set_value(cmd.clone());
                }
            }
        }
    }

    /// 添加助手响应
    pub async fn add_assistant_response(&mut self, content: String) {
        self.is_querying = false;
                self.stream_started_at = None;

        // 保存助手消息到数据库。流式引擎绑定同一会话时由引擎负责持久化。
        if self.should_persist_messages_from_tui() {
            if let Err(e) = self
                .session_manager
                .add_message(MessageRole::Assistant, &content)
            {
                warn!("Failed to save assistant message: {}", e);
            }
        }

        let assistant_msg = MessageItem {
            id: format!("msg_{}", self.messages.len()),
            role: MessageRole::Assistant,
            content,
            timestamp: std::time::SystemTime::now(),
            metadata: Default::default(),
        };
        self.messages.push(assistant_msg);
        self.scroll_to_bottom();
    }

    /// 添加系统消息
    pub fn add_system_message(&mut self, content: String) {
        let system_msg = MessageItem {
            id: format!("msg_{}", self.messages.len()),
            role: MessageRole::System,
            content,
            timestamp: std::time::SystemTime::now(),
            metadata: Default::default(),
        };
        self.messages.push(system_msg);
        self.scroll_to_bottom();
    }

    /// 添加工具消息
    pub fn add_tool_message(&mut self, tool_call_id: String, content: String) {
        let tool_msg = MessageItem {
            id: format!("msg_{}", self.messages.len()),
            role: MessageRole::Tool,
            content,
            timestamp: std::time::SystemTime::now(),
            metadata: {
                let mut map = std::collections::HashMap::new();
                map.insert("tool_call_id".to_string(), tool_call_id);
                map
            },
        };
        self.messages.push(tool_msg);
        self.scroll_to_bottom();
    }

    /// 向上滚动
    pub fn scroll_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
        self.pinned_to_bottom = false;
    }

    /// 向下滚动
    pub fn scroll_down(&mut self) {
        self.scroll_offset += 1;
        // Re-pin if scrolled past the last message
        if self.scroll_offset >= self.messages.len() {
            self.pinned_to_bottom = true;
        }
    }

    /// 滚动到底部（显示最新消息）
    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = self.messages.len();
        self.pinned_to_bottom = true;
    }

    /// 向上滚动半页（Vim Ctrl+U）
    pub fn scroll_up_half_page(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(5);
        self.pinned_to_bottom = false;
    }

    /// 向下滚动半页（Vim Ctrl+D）
    pub fn scroll_down_half_page(&mut self) {
        self.scroll_offset += 5;
    }

    /// 获取可见消息数量
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// 当前模型名称（用于状态展示）
    pub fn current_model_label(&self) -> String {
        if let Some(ref engine) = self.streaming_engine {
            engine.model_name().to_string()
        } else {
            "unknown".to_string()
        }
    }

    /// 当前 Provider 名称（用于状态展示）
    pub fn current_provider_label(&self) -> String {
        if let Some(ref engine) = self.streaming_engine {
            provider_name_from_base_url(&engine.provider_base_url()).to_string()
        } else {
            "unknown".to_string()
        }
    }

    /// 当前 Provider Base URL（用于状态展示）
    pub fn current_provider_base_url(&self) -> String {
        if let Some(ref engine) = self.streaming_engine {
            engine.provider_base_url()
        } else {
            "unknown".to_string()
        }
    }

    pub fn current_agent_mode_label(&self) -> &'static str {
        self.agent_mode.label()
    }

    pub fn set_agent_mode(&mut self, mode: AgentMode) {
        self.agent_mode = mode;
    }

    pub fn workspace_status_label(&self) -> String {
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let name = cwd
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("workspace");
        if let Some(branch) = read_git_branch_fast(&cwd) {
            format!("{}@{}", name, branch)
        } else {
            name.to_string()
        }
    }

    pub fn current_permission_label(&self) -> String {
        if let Some(ref engine) = self.streaming_engine {
            permission_mode_name(engine.permission_mode()).replace('_', "-")
        } else {
            "unknown".to_string()
        }
    }

    fn runtime_permission_state(&self) -> RuntimePermissionState {
        let mut state = RuntimePermissionState {
            mode: self.current_permission_label(),
            ..RuntimePermissionState::default()
        };
        if let Some(request) = self.pending_permission_request.as_ref() {
            state.pending_call_id = Some(request.tool_call.id.clone());
            state.pending_tool = Some(request.tool_call.name.clone());
            state.pending_prompt = Some(request.prompt.clone());
        }
        state
    }

    fn runtime_mcp_state(&self) -> RuntimeMcpState {
        let Some(engine) = self.streaming_engine.as_ref() else {
            return RuntimeMcpState::default();
        };
        let Some(manager) = engine.mcp_manager() else {
            return RuntimeMcpState::default();
        };
        let diagnostics = manager.health_diagnostics();
        let available_count = diagnostics
            .iter()
            .filter(|diag| {
                diag.approved && diag.health == crate::engine::mcp::McpHealthStatus::Healthy
            })
            .count();
        let repair_hints = diagnostics
            .iter()
            .filter(|diag| diag.repair_hint != "none")
            .map(|diag| format!("{}=>{}", diag.name, diag.repair_hint))
            .collect::<Vec<_>>();
        RuntimeMcpState {
            server_count: diagnostics.len(),
            available_count,
            repair_hints,
        }
    }

    fn runtime_bridge_state(&self) -> RuntimeBridgeState {
        let bridge = crate::bridge::runtime_snapshot();
        let remote_env = crate::remote::RemoteEnvDetector::detect();
        let saved_session_count = crate::remote::RemoteSessionManager::new()
            .list_sessions()
            .len();
        let remote_trigger_tool_available = self
            .streaming_engine
            .as_ref()
            .map(|engine| engine.tool_registry().has("remote_trigger"))
            .unwrap_or(false);
        let remote_dev_tool_available = self
            .streaming_engine
            .as_ref()
            .map(|engine| engine.tool_registry().has("remote_dev"))
            .unwrap_or(false);

        RuntimeBridgeState {
            bridge_url_configured: bridge.bridge_url.is_some(),
            bridge_url_source: bridge.bridge_url_source,
            auth_token_configured: bridge.auth_token_configured,
            tenant_configured: bridge.tenant_id.is_some(),
            cursor_count: bridge.cursor_count,
            saved_session_count,
            remote_env_type: remote_env.env_type.to_string(),
            is_remote_env: remote_env.is_remote,
            remote_trigger_tool_available,
            remote_dev_tool_available,
        }
    }

    fn build_runtime_state_snapshot(&self) -> RuntimeAppState {
        let tool_uses = self
            .tool_runs_snapshot
            .iter()
            .map(runtime_tool_use_from_view)
            .collect();
        let terminal_tasks = self
            .tool_runs_snapshot
            .iter()
            .filter_map(runtime_terminal_task_from_view)
            .collect();
        RuntimeAppState {
            tool_uses,
            terminal_tasks,
            permission: self.runtime_permission_state(),
            mcp: self.runtime_mcp_state(),
            bridge: self.runtime_bridge_state(),
        }
    }

    async fn sync_context_runtime_state(&self) {
        let runtime = self.runtime_state_snapshot.clone();
        let messages = self.messages.clone();
        let is_querying = self.is_querying;
        let last_error = self.error_message.clone();
        self.context
            .set_state(move |state| {
                state.messages = messages;
                state.is_querying = is_querying;
                state.last_error = last_error;
                state.runtime = runtime;
            })
            .await;
    }

    pub async fn runtime_status_snapshot(&self) -> RuntimeStatusSnapshot {
        let mut state = self.context.get_state().await;
        state.messages = self.messages.clone();
        state.is_querying = self.is_querying;
        state.last_error = self.error_message.clone();
        state.runtime = self.build_runtime_state_snapshot();
        select_runtime_status(&state)
    }

    pub fn runtime_status_snapshot_now(&self) -> RuntimeStatusSnapshot {
        let mut state = AppState::new();
        state.messages = self.messages.clone();
        state.is_querying = self.is_querying;
        state.last_error = self.error_message.clone();
        state.runtime = self.build_runtime_state_snapshot();
        for task in &self.tasks {
            state.tasks.insert(task.id.clone(), task.clone());
        }
        select_runtime_status(&state)
    }

    pub fn current_goal_label(&self) -> Option<String> {
        self.streaming_engine
            .as_ref()
            .and_then(|engine| engine.goal_manager().current())
            .map(|goal| {
                let max_chars = 36;
                let mut title = goal.title.chars().take(max_chars).collect::<String>();
                if goal.title.chars().count() > max_chars {
                    title.push_str("...");
                }
                format!("goal:{}", title)
            })
    }

    pub fn cycle_status_bar_density(&mut self) -> StatusBarDensity {
        self.status_bar_density = self.status_bar_density.next();
        self.status_bar_density
    }

    pub fn set_status_bar_density(&mut self, density: StatusBarDensity) {
        self.status_bar_density = density;
    }

    pub fn active_tool_count(&self) -> usize {
        if self.runtime_state_snapshot.tool_uses.is_empty() {
            self.tool_runs_snapshot
                .iter()
                .filter(|run| run.is_active())
                .count()
        } else {
            self.runtime_state_snapshot
                .tool_uses
                .iter()
                .filter(|tool| tool.active)
                .count()
        }
    }

    pub fn current_tool_status_label(&self) -> Option<String> {
        if let Some(tool) = self
            .runtime_state_snapshot
            .tool_uses
            .iter()
            .rev()
            .find(|tool| tool.active)
        {
            let elapsed = tool.elapsed_ms.unwrap_or_default() / 1000;
            return Some(format!("{} {}s", tool.summary, elapsed));
        }
        let active = self
            .tool_runs_snapshot
            .iter()
            .rev()
            .find(|run| run.is_active())?;
        Some(format!(
            "{} {}s",
            active.summary(),
            active.elapsed().as_secs()
        ))
    }

    pub fn terminal_task_status_label(&self) -> Option<String> {
        let terminal_count = self.runtime_state_snapshot.terminal_tasks.len();
        let running = self
            .runtime_state_snapshot
            .terminal_tasks
            .iter()
            .filter(|task| task.status == "running")
            .count();
        let backgrounded = self
            .runtime_state_snapshot
            .tool_uses
            .iter()
            .filter(|tool| tool.status == RuntimeToolStatus::Backgrounded)
            .count();
        let pty = self
            .runtime_state_snapshot
            .terminal_tasks
            .iter()
            .filter(|task| task.terminal_kind.as_deref() == Some("pty_shell"))
            .count();
        if terminal_count == 0 && backgrounded == 0 {
            return None;
        }
        let mut parts = vec![format!("terminal:{}", terminal_count.max(backgrounded))];
        if running > 0 || backgrounded > 0 {
            parts.push(format!("running:{}", running.max(backgrounded)));
        }
        if pty > 0 {
            parts.push(format!("pty:{}", pty));
        }
        Some(parts.join(" "))
    }

    pub fn stream_usage_label(&self) -> Option<String> {
        let usage = self.stream_usage_snapshot?;
        let mut label = format!("{} tokens", usage.total_tokens());
        if let Some(reasoning) = usage.reasoning_tokens {
            label.push_str(&format!(" / {} reasoning", reasoning));
        }
        if let Some(cached) = usage.cached_tokens {
            label.push_str(&format!(" / {} cached", cached));
            if let Some(miss) = usage.cache_miss_tokens() {
                label.push_str(&format!(" / {} miss", miss));
            }
            if let Some(hit_rate) = usage.cache_hit_rate_percent() {
                label.push_str(&format!(" / {:.1}% hit", hit_rate));
            }
        }
        Some(label)
    }

    pub fn toggle_transcript_expanded(&mut self) {
        self.transcript_expanded = !self.transcript_expanded;
        self.expanded_tool_run_id = None;
    }

    pub fn cycle_expanded_tool_run(&mut self) {
        let ids = self
            .visible_tool_run_ids()
            .into_iter()
            .collect::<Vec<String>>();
        if ids.is_empty() {
            self.transcript_expanded = !self.transcript_expanded;
            self.expanded_tool_run_id = None;
            return;
        }

        self.transcript_expanded = false;
        self.expanded_tool_run_id = match self.expanded_tool_run_id.as_deref() {
            None => ids.first().cloned(),
            Some(current) => ids
                .iter()
                .position(|id| id == current)
                .and_then(|idx| ids.get(idx + 1).cloned()),
        };
    }

    pub fn open_tool_viewer(&mut self) -> bool {
        let runtime_selected_id = select_tool_viewer_tool_id(
            &self.runtime_state_snapshot,
            self.expanded_tool_run_id.as_deref(),
        );
        let selected = runtime_selected_id
            .as_deref()
            .and_then(|id| self.find_visible_tool_run(id))
            .or_else(|| {
                self.expanded_tool_run_id
                    .as_deref()
                    .and_then(|id| self.find_visible_tool_run(id))
            })
            .or_else(|| self.visible_tool_runs().into_iter().next_back());

        let Some(run) = selected else {
            return false;
        };

        let title = run.summary();
        let content = run.full_details();
        self.tool_viewer_title = title;
        self.tool_viewer_content = content;
        self.tool_viewer_scroll_offset = 0;
        self.mode = AppMode::ToolViewer;
        true
    }

    pub fn open_tool_viewer_for(&mut self, id: &str) -> bool {
        let Some((title, content)) = self
            .find_visible_tool_run(id)
            .map(|run| (run.summary(), run.full_details()))
        else {
            return false;
        };
        self.tool_viewer_title = title;
        self.tool_viewer_content = content;
        self.tool_viewer_scroll_offset = 0;
        self.mode = AppMode::ToolViewer;
        true
    }

    pub fn tool_output_index_lines(&self) -> Vec<String> {
        self.visible_tool_runs()
            .into_iter()
            .map(|run| {
                format!(
                    "- {} [{}] {}",
                    run.id,
                    tool_run_status_label(run.status),
                    run.summary()
                )
            })
            .collect()
    }

    fn find_visible_tool_run(&self, id: &str) -> Option<&ToolRunView> {
        self.visible_tool_runs()
            .into_iter()
            .find(|run| run.id.as_str() == id)
    }

    fn visible_tool_runs(&self) -> Vec<&ToolRunView> {
        let mut runs = Vec::new();
        for msg in &self.messages {
            if let Some(group) = self.tool_runs_for_message(&msg.id) {
                runs.extend(group.iter());
            }
        }
        runs
    }

    fn visible_tool_run_ids(&self) -> Vec<String> {
        self.visible_tool_runs()
            .into_iter()
            .map(|run| run.id.clone())
            .collect()
    }

    pub fn is_tool_run_expanded(&self, run: &ToolRunView) -> bool {
        self.transcript_expanded || self.expanded_tool_run_id.as_deref() == Some(run.id.as_str())
    }

    pub fn tool_runs_for_message(&self, message_id: &str) -> Option<&[ToolRunView]> {
        self.tool_runs_by_message_id
            .get(message_id)
            .map(Vec::as_slice)
    }

    pub fn clear_tool_transcript(&mut self) {
        self.tool_runs_snapshot.clear();
        self.tool_runs_by_message_id.clear();
        self.current_tool_anchor_id = None;
        self.expanded_tool_run_id = None;
        self.stream_usage_snapshot = None;
    }

    /// 获取消息（考虑滚动）
    pub fn visible_messages(&self) -> &[MessageItem] {
        &self.messages
    }

    /// 设置错误信息
    pub fn set_error(&mut self, error: String) {
        self.error_message = Some(error);
        self.is_querying = false;
                self.stream_started_at = None;
    }

    /// 清除错误
    pub fn clear_error(&mut self) {
        self.error_message = None;
    }
}

impl Default for TuiApp {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::api::{
        ChatRequest as LlmChatRequest, ChatResponse as LlmChatResponse, LlmProvider, Usage,
    };
    use async_openai::types::ChatCompletionResponseStream;
    use async_trait::async_trait;
    use ratatui::{backend::TestBackend, Terminal};

    struct MockProvider;

    #[async_trait]
    impl LlmProvider for MockProvider {
        async fn chat(&self, _request: LlmChatRequest) -> anyhow::Result<LlmChatResponse> {
            Ok(LlmChatResponse {
                content: "ok".to_string(),
                tool_calls: None,
                usage: Some(Usage {
                    prompt_tokens: 1,
                    completion_tokens: 1,
                    total_tokens: 2,
                    reasoning_tokens: None,
                    cached_tokens: None,
                }),
                tool_call_repair: None,
            })
        }

        async fn chat_stream(
            &self,
            _request: LlmChatRequest,
        ) -> anyhow::Result<ChatCompletionResponseStream> {
            Err(anyhow::anyhow!("not implemented in TUI test"))
        }

        fn base_url(&self) -> &str {
            "https://api.openai.com/v1"
        }

        fn default_model(&self) -> &str {
            "mock-model"
        }
    }

    fn render_command_palette_text(app: &TuiApp) -> String {
        let backend = TestBackend::new(160, 70);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                crate::tui::screens::main_screen::render_command_palette(frame, app, frame.area());
            })
            .unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>()
    }

    #[test]
    fn test_cli_app_new() {
        let app = TuiApp::new();
        assert_eq!(app.messages.len(), 1); // 欢迎消息
        assert!(!app.is_querying);
        assert!(!app.paused);
        assert!(!app.focus_mode);
    }

    #[test]
    fn test_memory_snapshot_panel_includes_skip_reasons() {
        let snapshot = crate::memory::MemorySnapshotReport {
            frozen: true,
            snapshot_id: "memsnap-test".to_string(),
            fingerprint: "abc123".to_string(),
            scope: "project".to_string(),
            char_count: 120,
            project_chars: 80,
            user_chars: 40,
            memory_file_count: 2,
            memory_file_chars: 64,
            skipped_record_count: 4,
            skipped_status_count: 1,
            skipped_unsafe_count: 1,
            skipped_stale_count: 1,
            skipped_conflict_count: 1,
        };

        let formatted = format_memory_snapshot_report(&snapshot);

        assert!(formatted.contains("Skipped records: 4"));
        assert!(formatted.contains("status=1"));
        assert!(formatted.contains("unsafe=1"));
        assert!(formatted.contains("stale=1"));
        assert!(formatted.contains("conflicts=1"));
    }

    #[test]
    fn test_tui_reuses_engine_session_binding() {
        let store = Arc::new(crate::session_store::SessionStore::in_memory().unwrap());
        store
            .create_session("engine-session", "Engine Session", "mock-model")
            .unwrap();
        let engine = Arc::new(
            crate::engine::streaming::StreamingQueryEngine::new(
                Arc::new(MockProvider),
                Arc::new(crate::tools::ToolRegistry::new()),
                "mock-model",
            )
            .with_session_store(store, "engine-session".to_string()),
        );

        let app = TuiApp::with_engine(engine, None, None);

        assert_eq!(
            app.session_manager.current_session_id(),
            Some("engine-session")
        );
        assert!(!app.should_persist_messages_from_tui());
    }

    #[test]
    fn test_tui_persists_when_engine_has_no_session_binding() {
        let engine = Arc::new(
            crate::engine::streaming::StreamingQueryEngine::new(
                Arc::new(MockProvider),
                Arc::new(crate::tools::ToolRegistry::new()),
                "mock-model",
            )
            .with_disable_session_auto_init(),
        );

        let app = TuiApp::with_engine(engine, None, None);

        assert!(app.should_persist_messages_from_tui());
    }

    #[tokio::test]
    async fn test_tui_persists_streaming_assistant_when_engine_has_no_session_binding() {
        let engine = Arc::new(
            crate::engine::streaming::StreamingQueryEngine::new(
                Arc::new(MockProvider),
                Arc::new(crate::tools::ToolRegistry::new()),
                "mock-model",
            )
            .with_disable_session_auto_init(),
        );
        let mut app = TuiApp::with_engine(engine, None, None);
        let session_id = app
            .session_manager
            .current_session_id()
            .unwrap()
            .to_string();
        app.messages.push(MessageItem {
            id: "assistant-placeholder".to_string(),
            role: MessageRole::Assistant,
            content: String::new(),
            timestamp: std::time::SystemTime::now(),
            metadata: Default::default(),
        });
        {
            let mut response = app.current_response.lock().await;
            *response = "final answer".to_string();
        }
        app.is_querying = true;
        app.stream_done.store(true, Ordering::SeqCst);

        app.on_tick().await;

        let messages = app.session_manager.load_messages(&session_id).unwrap();
        assert!(messages
            .iter()
            .any(|message| message.role == MessageRole::Assistant
                && message.content == "final answer"));
    }

    #[test]
    fn test_parse_memory_save_args() {
        assert_eq!(
            parse_memory_save_args("remember this"),
            (MemorySaveTarget::Auto, None, "remember this")
        );
        assert_eq!(
            parse_memory_save_args("--user reply in Chinese"),
            (MemorySaveTarget::User, None, "reply in Chinese")
        );
        assert_eq!(
            parse_memory_save_args("--topic tui-design keep bottom anchored"),
            (
                MemorySaveTarget::Topic,
                Some("tui-design"),
                "keep bottom anchored"
            )
        );
        assert_eq!(
            parse_memory_save_args("--topic=context-management track token budget"),
            (
                MemorySaveTarget::Topic,
                Some("context-management"),
                "track token budget"
            )
        );
    }

    #[test]
    fn test_format_memory_write_outcome_reports_safety_block() {
        let outcome = crate::memory::manager::MemoryWriteOutcome {
            status: crate::memory::manager::MemoryWriteOutcomeStatus::Blocked,
            quality_score: None,
            reason: "secret_like_content: memory appears to contain a raw token".to_string(),
            path: None,
            record: None,
        };

        let rendered = format_memory_write_outcome("api_key = [redacted]", &outcome);

        assert!(rendered.contains("blocked for safety"));
        assert!(rendered.contains("secret_like_content"));
        assert!(!rendered.contains("Saved memory"));
    }

    #[test]
    fn test_format_memory_records_filters_by_scope() {
        let mut project = crate::memory::MemoryRecord::new(
            "Project convention: run cargo test before commit.",
            crate::memory::MemoryKind::ProjectFact,
            crate::memory::MemoryScope::local("records-test"),
            crate::memory::MemoryProvenance::local("test"),
        );
        project.status = crate::memory::MemoryStatus::Accepted;
        let mut user = crate::memory::MemoryRecord::new(
            "User preference: answer in Chinese.",
            crate::memory::MemoryKind::UserPreference,
            crate::memory::MemoryScope::local("records-test"),
            crate::memory::MemoryProvenance::local("test"),
        );
        user.status = crate::memory::MemoryStatus::Accepted;

        let rendered = format_memory_records(&[project, user], "--scope project");

        assert!(rendered.contains("Memory Records"));
        assert!(rendered.contains("Project convention"));
        assert!(!rendered.contains("User preference"));
    }

    #[test]
    fn test_stream_usage_label_includes_reasoning_and_cached_tokens() {
        let mut app = TuiApp::new();
        app.stream_usage_snapshot = Some(StreamUsageSnapshot {
            prompt_tokens: 100,
            completion_tokens: 25,
            reasoning_tokens: Some(12),
            cached_tokens: Some(80),
        });

        assert_eq!(
            app.stream_usage_label().as_deref(),
            Some("125 tokens / 12 reasoning / 80 cached / 20 miss / 80.0% hit")
        );
    }

    #[test]
    fn test_status_bar_density_cycle_and_parse() {
        let mut app = TuiApp::new();
        assert_eq!(app.status_bar_density, StatusBarDensity::Normal);
        assert_eq!(app.cycle_status_bar_density(), StatusBarDensity::Debug);
        assert_eq!(app.cycle_status_bar_density(), StatusBarDensity::Compact);
        assert_eq!(
            StatusBarDensity::parse("verbose"),
            Some(StatusBarDensity::Debug)
        );
        assert_eq!(
            StatusBarDensity::parse("minimal"),
            Some(StatusBarDensity::Compact)
        );
    }

    #[test]
    fn test_short_paste_inserts_directly() {
        let mut app = TuiApp::new();
        app.input.insert_str("prefix ");
        app.insert_paste("你好\nworld".to_string());

        assert_eq!(app.input.value(), "prefix 你好\nworld");
        assert_eq!(app.pasted_block_count(), 0);
    }

    #[test]
    fn test_long_paste_uses_placeholder_and_expands() {
        let mut app = TuiApp::new();
        let pasted = (0..20)
            .map(|idx| format!("line {}", idx))
            .collect::<Vec<_>>()
            .join("\n");

        app.input.insert_str("please inspect ");
        app.insert_paste(pasted.clone());

        assert_eq!(app.pasted_block_count(), 1);
        assert!(app.input.value().contains("[[paste:1 20 lines"));
        assert_eq!(
            app.expand_paste_placeholders(app.input.value()),
            format!("please inspect {}", pasted)
        );
    }

    #[tokio::test]
    async fn test_command_palette_accept_inserts_command_that_needs_args() {
        let mut app = TuiApp::new();
        app.open_command_palette();
        app.command_palette_push('s');
        app.command_palette_push('a');
        app.command_palette_push('v');
        app.command_palette_push('e');
        app.accept_command_palette_selection().await;

        assert_eq!(app.mode, AppMode::Chat);
        assert_eq!(app.input.value(), "/save ");
        assert!(app.recent_palette_commands.iter().any(|cmd| cmd == "/save"));
    }

    #[tokio::test]
    async fn test_command_palette_accept_executes_no_arg_command() {
        let mut app = TuiApp::new();
        app.open_command_palette();
        app.command_palette_push('s');
        app.command_palette_push('t');
        app.command_palette_push('a');
        app.command_palette_push('t');
        app.command_palette_push('u');
        app.command_palette_push('s');
        app.accept_command_palette_selection().await;

        assert_eq!(app.mode, AppMode::Chat);
        assert!(app.input.value().is_empty());
        assert!(app
            .recent_palette_commands
            .iter()
            .any(|cmd| cmd == "/status"));
        assert!(app.messages.len() > 1);
    }

    #[test]
    fn test_model_select_filters_choices() {
        let mut app = TuiApp::new();
        app.streaming_engine = Some(Arc::new(
            crate::engine::streaming::StreamingQueryEngine::new(
                Arc::new(MockProvider),
                Arc::new(crate::tools::ToolRegistry::new()),
                "gpt-4o",
            ),
        ));
        app.model_select_push('m');
        app.model_select_push('i');
        app.model_select_push('n');
        app.model_select_push('i');

        let choices = app.model_choices();
        assert!(choices.iter().all(|choice| choice.model.contains("mini")));
    }

    #[test]
    fn test_model_select_empty_filter_returns_no_choices() {
        let mut app = TuiApp::new();
        app.streaming_engine = Some(Arc::new(
            crate::engine::streaming::StreamingQueryEngine::new(
                Arc::new(MockProvider),
                Arc::new(crate::tools::ToolRegistry::new()),
                "gpt-4o",
            ),
        ));
        app.model_select_query = "not-a-real-model".to_string();

        assert!(app.model_choices().is_empty());
    }

    #[test]
    fn test_provider_select_filters_missing_providers() {
        let mut app = TuiApp::new();
        app.provider_select_push('k');
        app.provider_select_push('i');
        app.provider_select_push('m');
        app.provider_select_push('i');

        let choices = app.provider_choices();
        assert!(!choices.is_empty());
        assert!(choices
            .iter()
            .all(|choice| choice.name.contains("kimi") || choice.provider_type.contains("Kimi")));
    }

    #[test]
    fn test_provider_select_empty_filter_returns_no_choices() {
        let mut app = TuiApp::new();
        app.provider_select_query = "not-a-real-provider".to_string();

        assert!(app.provider_choices().is_empty());
    }

    #[test]
    fn test_workspace_entries_preview_summarizes_top_level_entries() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "[package]\n").unwrap();

        let preview = workspace_entries_preview(dir.path());

        assert!(preview.contains("1 dirs"));
        assert!(preview.contains("1 files"));
        assert!(preview.contains("src/"));
        assert!(preview.contains("Cargo.toml"));
    }

    #[test]
    fn test_contextual_palette_prioritizes_pending_permission_actions() {
        let mut app = TuiApp::new();
        app.pending_permission_request =
            Some(crate::engine::conversation_loop::ToolApprovalRequest {
                tool_call: crate::services::api::ToolCall {
                    id: "tool_1".to_string(),
                    name: "bash".to_string(),
                    arguments: serde_json::json!({ "command": "ls" }),
                },
                prompt: "Allow?".to_string(),
                review: None,
                audit: None,
            });

        let commands = app.contextual_palette_commands();
        assert_eq!(commands.first().map(String::as_str), Some("/reject"));
        assert!(commands.iter().any(|command| command == "/permissions"));
        assert!(app.is_contextual_palette_command("/reject"));

        let items = app.command_palette_items();
        assert_eq!(items.first().map(|cmd| cmd.name), Some("/reject"));
    }

    #[test]
    fn test_contextual_palette_includes_session_actions_after_chat() {
        let mut app = TuiApp::new();
        app.messages.push(MessageItem {
            id: "user_1".to_string(),
            role: MessageRole::User,
            content: "hello".to_string(),
            timestamp: std::time::SystemTime::now(),
            metadata: Default::default(),
        });

        let commands = app.contextual_palette_commands();

        assert!(commands.iter().any(|command| command == "/search"));
        assert!(commands.iter().any(|command| command == "/session"));
        assert!(commands.iter().any(|command| command == "/export"));
    }

    #[test]
    fn test_command_palette_render_marks_placeholder_commands() {
        let mut app = TuiApp::new();
        app.open_command_palette();
        for ch in "desktop".chars() {
            app.command_palette_push(ch);
        }

        let rendered = render_command_palette_text(&app);

        assert!(rendered.contains("Command Palette"));
        assert!(rendered.contains("/desktop"));
        assert!(rendered.contains("[placeholder]"));
        assert!(rendered.contains("Maturity:"));
        assert!(rendered.contains("placeholder"));
    }

    #[test]
    fn test_command_palette_render_marks_usable_commands() {
        let mut app = TuiApp::new();
        app.open_command_palette();
        for ch in "agents".chars() {
            app.command_palette_push(ch);
        }

        let rendered = render_command_palette_text(&app);

        assert!(rendered.contains("/agents"));
        assert!(rendered.contains("[usable]"));
        assert!(rendered.contains("Maturity:"));
        assert!(rendered.contains("usable"));
    }

    #[tokio::test]
    async fn test_help_maturity_slash_reports_buckets() {
        let mut app = TuiApp::new();

        app.handle_slash_command("/help maturity").await;

        let content = app
            .messages
            .last()
            .map(|message| message.content.as_str())
            .unwrap_or("");
        assert!(content.contains("Command maturity:"));
        assert!(content.contains("- usable"));
        assert!(content.contains("/panel"));
        assert!(content.contains("- placeholder"));
    }

    #[tokio::test]
    async fn test_mcp_status_slash_uses_runtime_panel_without_engine() {
        let mut app = TuiApp::new();

        app.handle_slash_command("/mcp status").await;

        let content = app
            .messages
            .last()
            .map(|message| message.content.as_str())
            .unwrap_or("");
        assert!(content.contains("# MCP Panel"));
        assert!(content.contains("Manager: engine unavailable"));
    }

    #[tokio::test]
    async fn test_permissions_slash_includes_pending_approval_panel() {
        let mut app = TuiApp::new();
        app.pending_permission_request =
            Some(crate::engine::conversation_loop::ToolApprovalRequest {
                tool_call: crate::services::api::ToolCall {
                    id: "call_1".to_string(),
                    name: "bash".to_string(),
                    arguments: serde_json::json!({ "command": "cargo check -q" }),
                },
                prompt: "Approve shell command?".to_string(),
                review: None,
                audit: None,
            });

        app.handle_slash_command("/permissions").await;

        let content = app
            .messages
            .last()
            .map(|message| message.content.as_str())
            .unwrap_or("");
        assert!(content.contains("Permission mode:"));
        assert!(content.contains("# Approval Panel"));
        assert!(content.contains("Pending approval: Tool approval"));
    }

    #[tokio::test]
    async fn test_context_slash_uses_runtime_context_panel() {
        let mut app = TuiApp::new();

        app.handle_slash_command("/context").await;

        let content = app
            .messages
            .last()
            .map(|message| message.content.as_str())
            .unwrap_or("");
        assert!(content.contains("# Context Panel"));
        assert!(content.contains("Context budget: engine unavailable"));
        assert!(!content.contains("# Context Status"));
    }

    #[test]
    fn test_command_palette_render_prioritizes_contextual_permission_actions() {
        let mut app = TuiApp::new();
        app.pending_permission_request =
            Some(crate::engine::conversation_loop::ToolApprovalRequest {
                tool_call: crate::services::api::ToolCall {
                    id: "tool_1".to_string(),
                    name: "bash".to_string(),
                    arguments: serde_json::json!({ "command": "ls" }),
                },
                prompt: "Allow?".to_string(),
                review: None,
                audit: None,
            });
        app.open_command_palette();

        let rendered = render_command_palette_text(&app);

        assert!(rendered.contains("Context"));
        assert!(rendered.contains("/reject"));
        assert!(rendered.contains("/permissions"));
        assert!(rendered.contains("Maturity:"));
    }

    #[test]
    fn test_cycle_expanded_tool_run_moves_through_visible_tools() {
        let mut app = TuiApp::new();
        let user = MessageItem {
            id: "user_1".to_string(),
            role: MessageRole::User,
            content: "run tools".to_string(),
            timestamp: std::time::SystemTime::now(),
            metadata: Default::default(),
        };
        app.messages.push(user);
        app.tool_runs_by_message_id.insert(
            "user_1".to_string(),
            vec![
                ToolRunView::new("tool_1".to_string(), "bash".to_string()),
                ToolRunView::new("tool_2".to_string(), "grep".to_string()),
            ],
        );

        app.cycle_expanded_tool_run();
        assert_eq!(app.expanded_tool_run_id.as_deref(), Some("tool_1"));
        app.cycle_expanded_tool_run();
        assert_eq!(app.expanded_tool_run_id.as_deref(), Some("tool_2"));
        app.cycle_expanded_tool_run();
        assert_eq!(app.expanded_tool_run_id, None);
    }

    #[test]
    fn test_open_tool_viewer_uses_expanded_tool_or_latest() {
        let mut app = TuiApp::new();
        let user = MessageItem {
            id: "user_1".to_string(),
            role: MessageRole::User,
            content: "run tools".to_string(),
            timestamp: std::time::SystemTime::now(),
            metadata: Default::default(),
        };
        app.messages.push(user);
        let mut first = ToolRunView::new("tool_1".to_string(), "bash".to_string());
        first.mark_complete("Result: OK\nfirst\n".to_string());
        let mut second = ToolRunView::new("tool_2".to_string(), "grep".to_string());
        second.mark_complete("Result: OK\nsecond\n".to_string());
        app.tool_runs_by_message_id
            .insert("user_1".to_string(), vec![first.clone(), second.clone()]);

        assert!(app.open_tool_viewer());
        assert_eq!(app.mode, AppMode::ToolViewer);
        assert!(app.tool_viewer_content.contains("second"));

        app.mode = AppMode::Chat;
        app.expanded_tool_run_id = Some("tool_1".to_string());
        assert!(app.open_tool_viewer());
        assert!(app.tool_viewer_content.contains("first"));
    }

    #[test]
    fn test_tool_output_index_and_open_by_id() {
        let mut app = TuiApp::new();
        let user = MessageItem {
            id: "user_1".to_string(),
            role: MessageRole::User,
            content: "run tools".to_string(),
            timestamp: std::time::SystemTime::now(),
            metadata: Default::default(),
        };
        app.messages.push(user);
        let mut first = ToolRunView::new("tool_1".to_string(), "bash".to_string());
        first.mark_complete("Result: OK\nfirst\n".to_string());
        app.tool_runs_by_message_id
            .insert("user_1".to_string(), vec![first]);

        let lines = app.tool_output_index_lines();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("tool_1"));
        assert!(lines[0].contains("[completed]"));
        assert!(app.open_tool_viewer_for("tool_1"));
        assert!(app.tool_viewer_content.contains("first"));
        assert!(!app.open_tool_viewer_for("missing"));
    }

    #[test]
    fn test_runtime_snapshot_keeps_terminal_task_metadata() {
        let mut app = TuiApp::new();
        let mut run = ToolRunView::new("tool_bg".to_string(), "bash".to_string());
        run.mark_complete_with_metadata(
            "Result: OK\nStarted background shell\n".to_string(),
            Some(serde_json::json!({
                "terminal_task": {
                    "task_id": "shell_bg_1",
                    "status": "running",
                    "terminal_kind": "background_shell",
                    "command": "npm run dev",
                    "handle": "shell_bg_1",
                    "read_tool": "bash_output",
                    "cancel_handle": "shell_bg_1"
                },
                "operation_kind": "shell",
                "ui_render_kind": "shell",
                "read_only": false,
                "concurrency_safe": false,
                "destructive": false,
                "input_paths": ["package.json"],
                "transcript_summary": "npm run dev"
            })),
        );
        app.tool_runs_snapshot.push(run);
        app.runtime_state_snapshot = app.build_runtime_state_snapshot();

        assert_eq!(
            app.runtime_state_snapshot.tool_uses[0]
                .operation_kind
                .as_deref(),
            Some("shell")
        );
        assert_eq!(
            app.runtime_state_snapshot.tool_uses[0].input_paths,
            vec!["package.json"]
        );
        assert_eq!(app.runtime_state_snapshot.terminal_tasks.len(), 1);
        assert_eq!(
            app.terminal_task_status_label().as_deref(),
            Some("terminal:1 running:1")
        );
    }

    #[test]
    fn test_session_permission_rule_is_added_when_approving_for_session() {
        let engine = Arc::new(crate::engine::streaming::StreamingQueryEngine::new(
            Arc::new(MockProvider),
            Arc::new(crate::tools::ToolRegistry::new()),
            "gpt-4o",
        ));
        let mut app = TuiApp::with_engine(engine.clone(), None, None);
        let (tx, mut rx) = tokio::sync::oneshot::channel();
        app.pending_permission_request =
            Some(crate::engine::conversation_loop::ToolApprovalRequest {
                tool_call: crate::services::api::ToolCall {
                    id: "tc_1".to_string(),
                    name: "mcp_tool".to_string(),
                    arguments: serde_json::json!({
                        "server_name": "filesystem",
                        "tool_name": "write_file"
                    }),
                },
                prompt: "Approve MCP?".to_string(),
                review: None,
                audit: None,
            });
        app.permission_response_tx = Some(tx);
        app.mode = AppMode::PermissionApproval;

        app.respond_to_permission_with_rule(true, Some("allow"), Some(RuleSource::User));

        let response = rx.try_recv().unwrap();
        assert!(response.approved);
        assert_eq!(
            response.decision,
            Some(PermissionReviewDecision::ApproveSession)
        );
        assert_eq!(response.persistence_scope.as_deref(), Some("session"));
        assert_eq!(
            response.rule_pattern.as_deref(),
            Some("mcp/filesystem/write_file")
        );
        let rules = engine.session_permission_rules();
        assert!(rules
            .always_allow
            .iter()
            .any(|rule| rule.pattern == "mcp/filesystem/write_file"));
        assert_eq!(app.mode, AppMode::Chat);
    }

    #[test]
    fn test_bash_session_permission_rule_uses_command_scope() {
        let engine = Arc::new(crate::engine::streaming::StreamingQueryEngine::new(
            Arc::new(MockProvider),
            Arc::new(crate::tools::ToolRegistry::new()),
            "gpt-4o",
        ));
        let mut app = TuiApp::with_engine(engine.clone(), None, None);
        let (tx, mut rx) = tokio::sync::oneshot::channel();
        app.pending_permission_request =
            Some(crate::engine::conversation_loop::ToolApprovalRequest {
                tool_call: crate::services::api::ToolCall {
                    id: "tc_bash".to_string(),
                    name: "bash".to_string(),
                    arguments: serde_json::json!({"command": "cargo test -q"}),
                },
                prompt: "Approve bash?".to_string(),
                review: None,
                audit: None,
            });
        app.permission_response_tx = Some(tx);
        app.mode = AppMode::PermissionApproval;

        app.respond_to_permission_with_rule(true, Some("allow"), Some(RuleSource::User));

        let response = rx.try_recv().unwrap();
        assert!(response.approved);
        assert_eq!(response.rule_pattern.as_deref(), Some("bash:cargo test*"));
        let rules = engine.session_permission_rules();
        assert!(rules
            .always_allow
            .iter()
            .any(|rule| rule.pattern == "bash:cargo test*"));
        assert_eq!(app.mode, AppMode::Chat);
    }

    #[test]
    fn test_model_selection_updates_engine_model() {
        let mut app = TuiApp::new();
        app.streaming_engine = Some(Arc::new(
            crate::engine::streaming::StreamingQueryEngine::new(
                Arc::new(MockProvider),
                Arc::new(crate::tools::ToolRegistry::new()),
                "gpt-4o",
            ),
        ));
        app.open_model_select();
        let choices = app.model_choices();
        let target = choices
            .iter()
            .position(|choice| choice.model == "gpt-4o-mini")
            .expect("openai preset expected");
        app.model_select_selected = target;
        app.accept_model_selection();

        assert_eq!(app.current_model_label(), "gpt-4o-mini");
        assert_eq!(app.mode, AppMode::Chat);
    }

    #[tokio::test]
    async fn test_send_message_blocked_when_paused() {
        let mut app = TuiApp::new();
        app.paused = true;
        let before = app.messages.len();
        app.send_message("hello".to_string()).await;
        assert_eq!(app.messages.len(), before + 1);
        let last = app.messages.last().expect("system message expected");
        assert_eq!(last.role, MessageRole::System);
        assert!(last.content.contains("Agent is paused"));
    }

    #[tokio::test]
    async fn test_send_message_keeps_bottom_anchor_after_assistant_placeholder() {
        let mut app = TuiApp::new();
        app.streaming_engine = Some(Arc::new(
            crate::engine::streaming::StreamingQueryEngine::new(
                Arc::new(MockProvider),
                Arc::new(crate::tools::ToolRegistry::new()),
                "mock-model",
            ),
        ));

        app.send_message("hello".to_string()).await;

        assert_eq!(app.messages.last().unwrap().role, MessageRole::Assistant);
        assert_eq!(app.scroll_offset, app.messages.len());
    }

    #[tokio::test]
    async fn test_restore_session() {
        let mut app = TuiApp::new();

        // 创建一个测试会话并添加消息
        let session_id = app
            .session_manager
            .start_session("Test Session", "kimi-k2.5")
            .unwrap();
        app.session_manager
            .add_message(MessageRole::User, "Hello")
            .unwrap();
        app.session_manager
            .add_message(MessageRole::Assistant, "Hi there!")
            .unwrap();

        // 验证消息已保存
        let count = app.session_manager.message_count(&session_id).unwrap();
        assert_eq!(count, 2);

        // 清空当前消息（模拟切换到新会话后的状态）
        app.messages.clear();
        app.messages.push(MessageItem {
            id: "temp".to_string(),
            role: MessageRole::System,
            content: "Temp".to_string(),
            timestamp: std::time::SystemTime::now(),
            metadata: Default::default(),
        });

        // 恢复会话
        let result = app.restore_session(&session_id).await;
        assert!(result.contains("Restored session"));
        assert!(result.contains("2 messages"));
        assert!(result.contains("Recent context:"));
        assert!(result.contains("Hello"));
        assert!(result.contains("Hi there!"));

        // 验证 UI 消息已恢复
        assert_eq!(app.messages.len(), 2);
        assert_eq!(app.messages[0].role, MessageRole::User);
        assert_eq!(app.messages[0].content, "Hello");
        assert_eq!(app.messages[1].role, MessageRole::Assistant);
        assert_eq!(app.messages[1].content, "Hi there!");

        // 验证当前会话 ID 已更新
        assert_eq!(
            app.session_manager.current_session_id(),
            Some(session_id.as_str())
        );
    }

    #[tokio::test]
    async fn test_restore_session_not_found() {
        let mut app = TuiApp::new();
        let result = app.restore_session("nonexistent_session").await;
        assert!(result.contains("Failed to restore session"));
    }

    #[test]
    fn test_respond_to_permission() {
        let mut app = TuiApp::new();
        let (tx, mut rx) = tokio::sync::oneshot::channel();
        app.pending_permission_request =
            Some(crate::engine::conversation_loop::ToolApprovalRequest {
                tool_call: crate::services::api::ToolCall {
                    id: "tc_1".to_string(),
                    name: "bash".to_string(),
                    arguments: serde_json::json!({"command": "echo hello"}),
                },
                prompt: "Approve bash?".to_string(),
                review: None,
                audit: None,
            });
        app.permission_response_tx = Some(tx);
        app.mode = AppMode::PermissionApproval;

        app.respond_to_permission(true);

        assert_eq!(app.mode, AppMode::Chat);
        assert!(app.pending_permission_request.is_none());
        assert!(app.permission_response_tx.is_none());
        let response = rx.try_recv().unwrap();
        assert!(response.approved);
        assert_eq!(
            response.decision,
            Some(PermissionReviewDecision::ApproveOnce)
        );
        assert!(response.persistence_scope.is_none());
    }

    #[test]
    fn test_compute_permission_diff_file_write() {
        let mut app = TuiApp::new();
        app.pending_permission_request =
            Some(crate::engine::conversation_loop::ToolApprovalRequest {
                tool_call: crate::services::api::ToolCall {
                    id: "tc_1".to_string(),
                    name: "file_write".to_string(),
                    arguments: serde_json::json!({
                        "path": "src/main.rs",
                        "content": "fn main() {\n    println!(\"hello\");\n}"
                    }),
                },
                prompt: "Approve file write?".to_string(),
                review: None,
                audit: None,
            });

        let (title, diff) = app.compute_permission_diff().unwrap();
        assert_eq!(title, "Preview: src/main.rs");
        assert!(diff.contains("+++ b/src/main.rs"));
        assert!(diff.contains("+fn main() {"));
        assert!(diff.contains("+    println!(\"hello\");"));
    }

    #[test]
    fn test_compute_permission_diff_file_edit_replace() {
        let mut app = TuiApp::new();
        // 使用不存在的文件路径确保回退到旧行为
        app.pending_permission_request =
            Some(crate::engine::conversation_loop::ToolApprovalRequest {
                tool_call: crate::services::api::ToolCall {
                    id: "tc_1".to_string(),
                    name: "file_edit".to_string(),
                    arguments: serde_json::json!({
                        "path": "nonexistent_file.rs",
                        "old_string": "println!(\"hello\");",
                        "new_string": "println!(\"world\");"
                    }),
                },
                prompt: "Approve file edit?".to_string(),
                review: None,
                audit: None,
            });

        let (title, diff) = app.compute_permission_diff().unwrap();
        assert_eq!(title, "Preview: nonexistent_file.rs");
        assert!(diff.contains("--- old_string ---"));
        assert!(diff.contains("-println!(\"hello\");"));
        assert!(diff.contains("+++ new_string +++"));
        assert!(diff.contains("+println!(\"world\");"));
    }

    #[test]
    fn test_compute_permission_diff_file_edit_insert() {
        let mut app = TuiApp::new();
        // 使用不存在的文件路径确保回退到旧行为
        app.pending_permission_request =
            Some(crate::engine::conversation_loop::ToolApprovalRequest {
                tool_call: crate::services::api::ToolCall {
                    id: "tc_1".to_string(),
                    name: "file_edit".to_string(),
                    arguments: serde_json::json!({
                        "path": "nonexistent_file.rs",
                        "insert_after": "fn main() {",
                        "new_string": "    // new line"
                    }),
                },
                prompt: "Approve file edit?".to_string(),
                review: None,
                audit: None,
            });

        let (title, diff) = app.compute_permission_diff().unwrap();
        assert_eq!(title, "Preview: nonexistent_file.rs");
        assert!(diff.contains("Insert after:"));
        assert!(diff.contains("fn main() {"));
        assert!(diff.contains("// new line"));
    }

    #[test]
    fn test_compute_permission_diff_bash() {
        let mut app = TuiApp::new();
        app.pending_permission_request =
            Some(crate::engine::conversation_loop::ToolApprovalRequest {
                tool_call: crate::services::api::ToolCall {
                    id: "tc_1".to_string(),
                    name: "bash".to_string(),
                    arguments: serde_json::json!({
                        "command": "cargo test",
                        "working_dir": "/tmp",
                        "timeout": 60
                    }),
                },
                prompt: "Approve bash?".to_string(),
                review: None,
                audit: None,
            });

        let (title, diff) = app.compute_permission_diff().unwrap();
        assert_eq!(title, "Preview: bash command");
        assert!(diff.contains("cargo test"));
        assert!(diff.contains("/tmp"));
        assert!(diff.contains("60s"));
    }

    #[test]
    fn memory_search_output_shows_retrieval_score_explanation() {
        let matches = vec![crate::memory::manager::MemoryMatch {
            source: "USER.md".to_string(),
            score: 36,
            rerank_score: Some(0.92),
            snippet: "User preference: answer concise Chinese status updates.".to_string(),
        }];
        let ctx = crate::engine::retrieval_context::RetrievalContext::from_memory_matches(
            "Chinese status",
            matches,
            &[],
            crate::engine::intent_router::RetrievalPolicy::Memory,
        )
        .expect("retrieval context");

        let output = format_memory_retrieval_context(&ctx);

        assert!(output.contains("decision selected USER.md"));
        assert!(output.contains("lexical="));
        assert!(output.contains("scope_match="));
        assert!(output.contains("confidence="));
        assert!(output.contains("conflict_penalty="));
        assert!(output.contains("pinned_bonus="));
    }

    #[test]
    fn test_compute_permission_diff_unsupported_tool() {
        let mut app = TuiApp::new();
        app.pending_permission_request =
            Some(crate::engine::conversation_loop::ToolApprovalRequest {
                tool_call: crate::services::api::ToolCall {
                    id: "tc_1".to_string(),
                    name: "grep".to_string(),
                    arguments: serde_json::json!({"pattern": "foo"}),
                },
                prompt: "Approve grep?".to_string(),
                review: None,
                audit: None,
            });

        assert!(app.compute_permission_diff().is_none());
    }

    #[test]
    fn skill_outcome_prefers_acceptance_review_signal() {
        let mut trace = crate::engine::trace::TurnTrace::new("s1", 1, "use skill");
        trace
            .events
            .push(crate::engine::trace::TraceEvent::VerificationCompleted {
                changed_files: 2,
                passed: true,
                check_passed: true,
                tests_passed: true,
                review_passed: true,
                failed_commands: Vec::new(),
            });
        trace.events.push(
            crate::engine::trace::TraceEvent::AcceptanceReviewCompleted {
                accepted: true,
                confidence: "high".to_string(),
                criteria: 3,
                unresolved: 0,
                next_action: "close".to_string(),
            },
        );

        let outcome = skill_outcome_attribution(Some(&trace), true, false, false);

        assert!(outcome.success);
        assert_eq!(outcome.acceptance_passed, Some(true));
        assert_eq!(outcome.tests_passed, Some(true));
        assert_eq!(outcome.source, "acceptance_review");
        assert!(outcome.confidence > 0.8);
    }

    #[test]
    fn skill_outcome_blocks_on_unresolved_acceptance() {
        let mut trace = crate::engine::trace::TurnTrace::new("s1", 1, "use skill");
        trace
            .events
            .push(crate::engine::trace::TraceEvent::VerificationCompleted {
                changed_files: 2,
                passed: true,
                check_passed: true,
                tests_passed: true,
                review_passed: true,
                failed_commands: Vec::new(),
            });
        trace.events.push(
            crate::engine::trace::TraceEvent::AcceptanceReviewCompleted {
                accepted: false,
                confidence: "medium".to_string(),
                criteria: 3,
                unresolved: 1,
                next_action: "repair".to_string(),
            },
        );

        let outcome = skill_outcome_attribution(Some(&trace), true, false, false);

        assert!(!outcome.success);
        assert_eq!(outcome.acceptance_passed, Some(false));
        assert_eq!(outcome.tests_passed, Some(true));
        assert!(outcome.risk_penalty >= 0.45);
    }
}
