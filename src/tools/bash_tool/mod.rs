//! Bash 工具 - 执行 shell 命令
//!
//! 对应 Claude Code 中的 BashTool

mod background;
pub mod command_classifier;
mod pty;

use crate::engine::context_ledger::{record_bash_read, BashReadLedgerInput};
use crate::tools::{
    Tool, ToolContext, ToolErrorCode, ToolOperationKind, ToolResult, ToolSearchOrReadSemantics,
};
use async_trait::async_trait;
use background::{background_shell_result_data, background_started_content};
pub use background::{BashCancelTool, BashOutputTool, BashTasksTool};
use command_classifier::{
    classify_command, CommandClassification, CommandKind, ShellCommandCategory,
};
use serde_json::json;
use std::process::Stdio;
use std::{
    hash::{Hash, Hasher},
    time::{Instant, SystemTime, UNIX_EPOCH},
};
use tokio::process::Command;
use tracing::{debug, error, info, warn};

/// Bash 工具
pub struct BashTool;

const DEFAULT_OUTPUT_ARTIFACT_MIN_BYTES: usize = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BashExecutionBackend {
    Local,
    Restricted,
    External,
}

impl BashExecutionBackend {
    fn as_str(self) -> &'static str {
        match self {
            BashExecutionBackend::Local => "local",
            BashExecutionBackend::Restricted => "restricted",
            BashExecutionBackend::External => "external",
        }
    }
}

fn parse_backend(value: &str) -> Option<BashExecutionBackend> {
    match value.trim().to_ascii_lowercase().as_str() {
        "local" => Some(BashExecutionBackend::Local),
        "restricted" | "sandbox" | "soft_sandbox" => Some(BashExecutionBackend::Restricted),
        "external" => Some(BashExecutionBackend::External),
        _ => None,
    }
}

fn default_backend() -> BashExecutionBackend {
    match std::env::var("PRIORITY_AGENT_BASH_BACKEND") {
        Ok(raw) => {
            let trimmed = raw.trim();
            match parse_backend(trimmed) {
                Some(backend) => backend,
                None => {
                    warn!(
                        "Invalid PRIORITY_AGENT_BASH_BACKEND='{}', expected 'local'/'restricted'/'external'. Falling back to 'local'.",
                        trimmed
                    );
                    BashExecutionBackend::Local
                }
            }
        }
        Err(_) => BashExecutionBackend::Local,
    }
}

fn effective_timeout_secs(requested: Option<u64>) -> u64 {
    let requested = requested.unwrap_or(60).min(3600);
    let floor = std::env::var("PRIORITY_AGENT_BASH_TIMEOUT_FLOOR_SECS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(0)
        .min(3600);
    requested.max(floor).min(3600)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AutoBackgroundDecision {
    reason: &'static str,
    threshold_secs: u64,
    timeout_secs: u64,
}

fn auto_background_enabled() -> bool {
    std::env::var("PRIORITY_AGENT_BASH_AUTO_BACKGROUND")
        .map(|value| {
            !matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "0" | "false" | "off" | "no"
            )
        })
        .unwrap_or(true)
}

fn auto_background_threshold_secs() -> u64 {
    std::env::var("PRIORITY_AGENT_BASH_AUTO_BACKGROUND_SECS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(30)
        .clamp(1, 3600)
}

fn auto_background_decision(
    command: &str,
    classification: &CommandClassification,
    mode: &str,
    timeout_secs: u64,
) -> Option<AutoBackgroundDecision> {
    if mode != "foreground" || !auto_background_enabled() || classification.requires_pty() {
        return None;
    }

    let threshold_secs = auto_background_threshold_secs();
    if timeout_secs < threshold_secs {
        return None;
    }

    let reason = if classification.category == ShellCommandCategory::DevServer {
        Some("dev_server")
    } else {
        watch_like_command_reason(command)
    }?;

    Some(AutoBackgroundDecision {
        reason,
        threshold_secs,
        timeout_secs,
    })
}

fn watch_like_command_reason(command: &str) -> Option<&'static str> {
    let lower = command.trim().to_ascii_lowercase();
    let words = lower.split_whitespace().collect::<Vec<_>>();
    let first = words.first().copied();
    let second = words.get(1).copied();

    if matches!(first, Some("watch" | "watchexec")) {
        return Some("watch_mode");
    }
    if matches!((first, second), (Some("cargo"), Some("watch"))) {
        return Some("watch_mode");
    }
    if matches!(first, Some("npm" | "pnpm" | "yarn")) && words.contains(&"watch") {
        return Some("watch_mode");
    }
    if matches!(
        first,
        Some("tsc" | "jest" | "vitest" | "webpack" | "rollup" | "parcel" | "deno" | "bun")
    ) && words
        .iter()
        .any(|word| *word == "--watch" || word.starts_with("--watch="))
    {
        return Some("watch_mode");
    }
    if matches!((first, second), (Some("tail"), Some("-f"))) {
        return Some("follow_output");
    }
    None
}

fn attach_auto_background_metadata(
    data: &mut serde_json::Value,
    decision: &AutoBackgroundDecision,
) {
    let metadata = json!({
        "enabled": true,
        "reason": decision.reason,
        "threshold_secs": decision.threshold_secs,
        "timeout_secs": decision.timeout_secs,
    });
    if let Some(object) = data.as_object_mut() {
        object.insert("auto_background".to_string(), metadata.clone());
        for key in ["shell_result", "shell_background", "terminal_task"] {
            if let Some(child) = object
                .get_mut(key)
                .and_then(serde_json::Value::as_object_mut)
            {
                child.insert("auto_background".to_string(), metadata.clone());
            }
        }
    }
}

fn sanitize_agent_runtime_env(cmd: &mut Command) {
    for key in [
        "PRIORITY_AGENT_A2A_TRANSCRIPT_PATH",
        "PRIORITY_AGENT_AUTO_REVIEW",
        "PRIORITY_AGENT_AUTO_TEST",
        "PRIORITY_AGENT_BASH_BACKEND",
        "PRIORITY_AGENT_BASH_EXTERNAL_ALLOWLIST",
        "PRIORITY_AGENT_BASH_EXTERNAL_CMD",
        "PRIORITY_AGENT_BASH_EXTERNAL_FALLBACK",
        "PRIORITY_AGENT_BASH_EXTERNAL_WRAPPER_ALLOWLIST",
        "PRIORITY_AGENT_BASH_SANDBOX_CMD",
        "PRIORITY_AGENT_BASH_SANDBOX_FALLBACK",
        "PRIORITY_AGENT_BASH_TIMEOUT_FLOOR_SECS",
        "PRIORITY_AGENT_CLOSEOUT_VISIBILITY",
        "PRIORITY_AGENT_DEBUG_TOOL_EXPOSURE",
        "PRIORITY_AGENT_EVAL_EVENTS",
        "PRIORITY_AGENT_LEGACY_WORKFLOW_ENABLED",
        "PRIORITY_AGENT_LLM_MEMORY_EXTRACTION",
        "PRIORITY_AGENT_ROUTE_SCOPED_TOOLS",
        "PRIORITY_AGENT_TOOL_PROFILE",
        "PRIORITY_AGENT_WORKFLOW_CONTRACT",
        "PRIORITY_AGENT_WORKFLOW_ENABLED",
    ] {
        cmd.env_remove(key);
    }
}

fn restricted_command(command: &str) -> String {
    // 受限后端说明：
    // - 仅应用软资源限制和最小化环境变量
    // - 不是容器/命名空间级别隔离
    format!(
        "ulimit -n 64; ulimit -u 32; ulimit -t 60; \
         export PATH=/usr/bin:/bin; \
         unset http_proxy https_proxy HTTP_PROXY HTTPS_PROXY ALL_PROXY all_proxy; \
         {}",
        command
    )
}

fn shell_single_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\"'\"'"))
}

fn external_wrapper_template() -> Option<String> {
    std::env::var("PRIORITY_AGENT_BASH_EXTERNAL_CMD")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            std::env::var("PRIORITY_AGENT_BASH_SANDBOX_CMD")
                .ok()
                .filter(|s| !s.trim().is_empty())
        })
}

fn external_wrapper_allowlist() -> Option<Vec<String>> {
    let value = std::env::var("PRIORITY_AGENT_BASH_EXTERNAL_ALLOWLIST")
        .ok()
        .or_else(|| std::env::var("PRIORITY_AGENT_BASH_EXTERNAL_WRAPPER_ALLOWLIST").ok())?;
    let items: Vec<String> = value
        .split(|c: char| c == ',' || c == ';' || c.is_ascii_whitespace())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
        .collect();
    if items.is_empty() {
        None
    } else {
        Some(items)
    }
}

fn external_fallback_backend() -> Option<BashExecutionBackend> {
    let value = std::env::var("PRIORITY_AGENT_BASH_EXTERNAL_FALLBACK")
        .ok()
        .or_else(|| std::env::var("PRIORITY_AGENT_BASH_SANDBOX_FALLBACK").ok())?;
    match value.trim().to_ascii_lowercase().as_str() {
        "none" | "deny" => None,
        other => parse_backend(other).filter(|b| *b != BashExecutionBackend::External),
    }
}

fn first_shell_token(s: &str) -> Option<String> {
    s.split_whitespace().next().map(ToString::to_string)
}

fn short_command_summary(command: &str) -> String {
    let mut chars = command.chars();
    let summary: String = chars.by_ref().take(120).collect();
    if chars.next().is_some() {
        format!("{summary}...")
    } else {
        summary
    }
}

fn validate_external_wrapper(template: &str) -> Result<(), String> {
    let allowlist = match external_wrapper_allowlist() {
        Some(v) => v,
        None => return Ok(()),
    };
    let wrapper = first_shell_token(template)
        .ok_or_else(|| "external wrapper template is empty".to_string())?;
    let allowed = allowlist.iter().any(|x| x == &wrapper);
    if allowed {
        Ok(())
    } else {
        Err(format!(
            "external wrapper '{}' is not in PRIORITY_AGENT_BASH_EXTERNAL_ALLOWLIST",
            wrapper
        ))
    }
}

fn external_command_with_template(template: &str, command: &str) -> String {
    let quoted = shell_single_quote(command);
    if template.contains("{command}") {
        template.replace("{command}", &quoted)
    } else {
        format!("{} -- bash -lc {}", template, quoted)
    }
}

fn external_command(command: &str) -> Result<String, String> {
    let template = external_wrapper_template().ok_or_else(|| {
        "external backend requires PRIORITY_AGENT_BASH_EXTERNAL_CMD (or PRIORITY_AGENT_BASH_SANDBOX_CMD)".to_string()
    })?;
    validate_external_wrapper(&template)?;
    Ok(external_command_with_template(&template, command))
}

fn build_audit(
    backend_requested: &str,
    backend_effective: &str,
    fallback_reason: Option<&str>,
    sandbox: bool,
    timeout: u64,
    working_dir: &std::path::Path,
) -> serde_json::Value {
    json!({
        "backend_requested": backend_requested,
        "backend_effective": backend_effective,
        "fallback_used": fallback_reason.is_some(),
        "fallback_reason": fallback_reason,
        "sandbox": sandbox,
        "timeout_secs": timeout,
        "working_dir": working_dir.display().to_string(),
        "external_wrapper_configured": external_wrapper_template().is_some(),
        "external_allowlist_configured": external_wrapper_allowlist().is_some(),
        "external_fallback_configured": std::env::var("PRIORITY_AGENT_BASH_EXTERNAL_FALLBACK").is_ok()
            || std::env::var("PRIORITY_AGENT_BASH_SANDBOX_FALLBACK").is_ok(),
    })
}

fn classification_data(command: &str) -> serde_json::Value {
    serde_json::to_value(classify_command(command)).unwrap_or_else(|_| json!({}))
}

fn bash_permission_review_data(
    command: &str,
    classification: &CommandClassification,
    backend: BashExecutionBackend,
    mode: &str,
    sandbox: bool,
) -> serde_json::Value {
    let mut facts = Vec::new();
    if classification.command_kind == CommandKind::Dangerous
        || classification.category == ShellCommandCategory::Destructive
    {
        facts.push("destructive_command");
    }
    if classification.risky_shell_wrapper {
        facts.push("risky_shell_wrapper");
    }
    if classification.network_access {
        facts.push("network_access");
    }
    if classification.command_plan.fail_closed {
        facts.push("shell_structure_review");
    }
    if classification.external_path_access {
        facts.push("external_path_access");
    }
    if classification.compound_command {
        facts.push("compound_shell_command");
    }
    if classification.requires_pty() {
        facts.push("requires_pty");
    }
    if classification.category == ShellCommandCategory::PackageInstall {
        facts.push("package_install");
    }
    if matches!(
        classification.category,
        ShellCommandCategory::FileMutation | ShellCommandCategory::GitMutation
    ) {
        facts.push("mutation_command");
    }
    if backend == BashExecutionBackend::External {
        facts.push("external_backend");
    }
    if sandbox {
        facts.push("soft_sandbox");
    }
    if mode == "background" {
        facts.push("background_task");
    }

    let risk_level = if classification.command_kind == CommandKind::Dangerous
        || classification.category == ShellCommandCategory::Destructive
        || classification.risky_shell_wrapper
        || classification.command_plan.fail_closed
    {
        "high"
    } else if classification.network_access
        || classification.external_path_access
        || classification.compound_command
        || classification.requires_pty()
        || matches!(
            classification.category,
            ShellCommandCategory::PackageInstall
                | ShellCommandCategory::FileMutation
                | ShellCommandCategory::GitMutation
                | ShellCommandCategory::DevServer
                | ShellCommandCategory::Interactive
        )
        || backend == BashExecutionBackend::External
        || mode == "background"
    {
        "medium"
    } else {
        "low"
    };

    let review_required = risk_level != "low";
    let suggested_action = match risk_level {
        "high" => "require explicit user approval or choose a lower-risk command before retrying",
        "medium" => "review command scope and approve the exact command if intended",
        _ => "allow as low-risk shell command",
    };

    json!({
        "command": command,
        "risk_level": risk_level,
        "review_required": review_required,
        "facts": facts,
        "backend": backend.as_str(),
        "mode": mode,
        "sandbox": sandbox,
        "suggested_action": suggested_action,
        "command_plan": classification.command_plan,
        "permission_rule_suggestions": classification.permission_rule_suggestions,
    })
}

fn preview_text(text: &str, max_chars: usize) -> (String, bool) {
    let mut preview = String::new();
    let mut truncated = false;
    for (idx, ch) in text.chars().enumerate() {
        if idx >= max_chars {
            truncated = true;
            break;
        }
        preview.push(ch);
    }
    (preview, truncated)
}

fn shell_output_artifact_path(
    context: &ToolContext,
    working_dir: &std::path::Path,
    command: &str,
    output: &str,
) -> Option<String> {
    if output.trim().is_empty() {
        return None;
    }
    let session = context
        .session_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    let session = if session.trim().is_empty() {
        "session".to_string()
    } else {
        session
    };
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    command.hash(&mut hasher);
    output.len().hash(&mut hasher);
    let hash = hasher.finish();
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    let relative = std::path::PathBuf::from(".priority-agent")
        .join("tool-results")
        .join(session)
        .join(format!("bash-{millis}-{hash:x}.log"));
    let path = working_dir.join(&relative);
    if let Some(parent) = path.parent() {
        if std::fs::create_dir_all(parent).is_err() {
            return None;
        }
    }
    std::fs::write(&path, output).ok()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).ok();
    }
    Some(relative.to_string_lossy().to_string())
}

fn shell_output_artifact_min_bytes() -> usize {
    std::env::var("PRIORITY_AGENT_BASH_OUTPUT_ARTIFACT_MIN_BYTES")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(DEFAULT_OUTPUT_ARTIFACT_MIN_BYTES)
        .min(10_000_000)
}

fn should_write_shell_output_artifact(output: &str, preview_truncated: bool) -> bool {
    should_write_shell_output_artifact_with_min(
        output,
        preview_truncated,
        shell_output_artifact_min_bytes(),
    )
}

fn should_write_shell_output_artifact_with_min(
    output: &str,
    preview_truncated: bool,
    min_bytes: usize,
) -> bool {
    if output.trim().is_empty() {
        return false;
    }
    preview_truncated || output.len() >= min_bytes
}

struct ShellResultData<'a> {
    audit: &'a serde_json::Value,
    command: &'a str,
    working_dir: &'a std::path::Path,
    stdout: &'a str,
    stderr: &'a str,
    combined_output: &'a str,
    exit_code: i32,
    backend: BashExecutionBackend,
    timed_out: bool,
    context: &'a ToolContext,
    terminal_kind: &'a str,
    pty: bool,
    sandbox: bool,
    started_at_ms: u64,
    ended_at_ms: u64,
    duration_ms: u64,
}

struct TerminalTaskTiming {
    started_at_ms: u64,
    started_at: Instant,
}

struct BashRuntimeRef<'a> {
    audit: &'a serde_json::Value,
    command: &'a str,
    working_dir: &'a std::path::Path,
    backend: BashExecutionBackend,
    sandbox: bool,
    context: &'a ToolContext,
}

impl TerminalTaskTiming {
    fn start() -> Self {
        Self {
            started_at_ms: system_time_millis(SystemTime::now()),
            started_at: Instant::now(),
        }
    }

    fn started_at_ms(&self) -> u64 {
        self.started_at_ms
    }

    fn ended_at_ms(&self) -> u64 {
        system_time_millis(SystemTime::now())
    }

    fn duration_ms(&self) -> u64 {
        self.started_at.elapsed().as_millis() as u64
    }
}

fn shell_result_data(input: ShellResultData<'_>) -> (String, serde_json::Value) {
    const MAX_OUTPUT_LEN: usize = 10000;
    const STREAM_PREVIEW_CHARS: usize = 1200;

    let (mut content_preview, content_truncated) =
        preview_text(input.combined_output, MAX_OUTPUT_LEN);
    if content_truncated {
        content_preview.push_str(&format!(
            "\n\n[Output truncated: {} bytes total]",
            input.combined_output.len()
        ));
    }
    content_preview = append_shell_compatibility_hint(content_preview);

    let output_path = should_write_shell_output_artifact(input.combined_output, content_truncated)
        .then(|| {
            shell_output_artifact_path(
                input.context,
                input.working_dir,
                input.command,
                input.combined_output,
            )
        })
        .flatten();
    let (stdout_preview, stdout_truncated) = preview_text(input.stdout, STREAM_PREVIEW_CHARS);
    let (stderr_preview, stderr_truncated) = preview_text(input.stderr, STREAM_PREVIEW_CHARS);
    let classification = classification_data(input.command);
    let command_classification = classify_command(input.command);
    let permission_review = bash_permission_review_data(
        input.command,
        &command_classification,
        input.backend,
        input.terminal_kind,
        input.sandbox,
    );
    let evidence_status = if input.timed_out {
        "timed_out"
    } else if input.exit_code == 0 {
        "passed"
    } else {
        "failed"
    };
    let terminal_status = terminal_task_status(input.exit_code, input.timed_out);
    let output_path_for_task = output_path.clone();
    let read_tool = if output_path_for_task.is_some() {
        serde_json::Value::String("file_read".to_string())
    } else {
        serde_json::Value::Null
    };
    let output_persisted = output_path.is_some();
    let output_available = output_persisted || !input.combined_output.trim().is_empty();
    let recovery = shell_recovery_metadata(
        input.command,
        input.exit_code,
        input.timed_out,
        input.stdout,
        input.stderr,
        &command_classification,
    );
    let data = json!({
        "audit": input.audit,
        "command_classification": classification.clone(),
        "permission_review": permission_review,
        "shell_result": {
            "command": input.command,
            "cwd": input.working_dir.display().to_string(),
            "exit_code": input.exit_code,
            "stdout_preview": stdout_preview,
            "stderr_preview": stderr_preview,
            "output_path": output_path,
            "output_persisted": output_persisted,
            "output_bytes": input.combined_output.len(),
            "stdout_bytes": input.stdout.len(),
            "stderr_bytes": input.stderr.len(),
            "duration_ms": serde_json::Value::Null,
            "timed_out": input.timed_out,
            "truncated": content_truncated || stdout_truncated || stderr_truncated,
            "classification": classification,
            "evidence_status": evidence_status,
        },
        "terminal_task": {
            "task_id": terminal_task_id(input.command, input.terminal_kind, input.started_at_ms),
            "handle": serde_json::Value::Null,
            "command": input.command,
            "cwd": input.working_dir.display().to_string(),
            "status": terminal_status,
            "started_at_ms": input.started_at_ms,
            "ended_at_ms": input.ended_at_ms,
            "duration_ms": input.duration_ms,
            "exit_code": input.exit_code,
            "output_path": output_path_for_task,
            "output_available": output_available,
            "output_persisted": output_persisted,
            "output_bytes": input.combined_output.len(),
            "stdout_bytes": input.stdout.len(),
            "stderr_bytes": input.stderr.len(),
            "read_tool": read_tool,
            "cancel_tool": serde_json::Value::Null,
            "cancel_handle": serde_json::Value::Null,
            "terminal_kind": input.terminal_kind,
            "pty": input.pty,
            "failure_reason": recovery.get("reason").cloned().unwrap_or(serde_json::Value::Null),
            "recovery_action": recovery.get("action").cloned().unwrap_or(serde_json::Value::Null)
        },
        "recovery": recovery,
        "execution": {
            "exit_code": input.exit_code,
            "stdout_length": input.stdout.len(),
            "stderr_length": input.stderr.len(),
            "backend": input.backend.as_str(),
            "truncated": content_truncated || stdout_truncated || stderr_truncated
        }
    });

    (content_preview, data)
}

fn shell_recovery_metadata(
    command: &str,
    exit_code: i32,
    timed_out: bool,
    stdout: &str,
    stderr: &str,
    classification: &CommandClassification,
) -> serde_json::Value {
    let output = format!("{stdout}\n{stderr}").to_ascii_lowercase();
    let (category, action, reason) = if timed_out {
        (
            "timeout",
            "retry_background_or_increase_timeout",
            "command timed out before completion",
        )
    } else if exit_code == 0 {
        ("none", "none", "command completed successfully")
    } else if output.contains("command not found") || output.contains("not found") {
        (
            "command_not_found",
            "install_or_fix_path",
            "executable was not found in PATH",
        )
    } else if output.contains("permission denied") || output.contains("operation not permitted") {
        (
            "permission_denied",
            "check_file_permissions_or_request_access",
            "command failed because the OS denied access",
        )
    } else if classification.requires_pty() {
        (
            "interactive_needs_pty",
            "retry_with_pty_mode",
            "interactive command should run in PTY mode",
        )
    } else if classification.is_safe_validation() {
        (
            "validation_failed",
            "inspect_output_then_fix_code",
            "validation command returned a non-zero exit code",
        )
    } else {
        (
            "nonzero_exit",
            "inspect_output_before_retry",
            "command returned a non-zero exit code",
        )
    };

    json!({
        "category": category,
        "action": action,
        "reason": reason,
        "exit_code": exit_code,
        "command": command,
    })
}

fn terminal_task_status(exit_code: i32, timed_out: bool) -> &'static str {
    if timed_out {
        "timed_out"
    } else if exit_code == 0 {
        "completed"
    } else {
        "failed"
    }
}

fn terminal_task_id(command: &str, terminal_kind: &str, started_at_ms: u64) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    command.hash(&mut hasher);
    terminal_kind.hash(&mut hasher);
    started_at_ms.hash(&mut hasher);
    format!(
        "shell_{}_{}_{:x}",
        terminal_kind,
        started_at_ms,
        hasher.finish()
    )
}

fn shell_output_hash(stdout: &str, stderr: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    stdout.hash(&mut hasher);
    stderr.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn shell_category_name(category: ShellCommandCategory) -> &'static str {
    match category {
        ShellCommandCategory::Read => "read",
        ShellCommandCategory::List => "list",
        ShellCommandCategory::Search => "search",
        ShellCommandCategory::Validation => "validation",
        ShellCommandCategory::PackageInstall => "package_install",
        ShellCommandCategory::DevServer => "dev_server",
        ShellCommandCategory::Interactive => "interactive",
        ShellCommandCategory::TestRun => "test_run",
        ShellCommandCategory::FileMutation => "file_mutation",
        ShellCommandCategory::GitMutation => "git_mutation",
        ShellCommandCategory::Destructive => "destructive",
        ShellCommandCategory::Unknown => "unknown",
    }
}

fn system_time_millis(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn shell_compatibility_hint(output: &str) -> Option<&'static str> {
    if output.contains("declare: -A: invalid option")
        || (output.contains("declare:")
            && output.contains("-A")
            && output.contains("invalid option"))
    {
        Some(
            "Shell compatibility hint: this bash is likely macOS bash 3.x, which does not support associative arrays (`declare -A`). Use POSIX-compatible shell logic, indexed arrays, temp files, awk, or an existing Python helper for map-style aggregation.",
        )
    } else {
        None
    }
}

fn append_shell_compatibility_hint(output: String) -> String {
    match shell_compatibility_hint(&output) {
        Some(hint) if !output.contains(hint) => format!("{output}\n\n{hint}"),
        _ => output,
    }
}

fn error_with_audit(
    error: impl Into<String>,
    content: Option<String>,
    audit: &serde_json::Value,
    command: &str,
) -> ToolResult {
    let classification = classify_command(command);
    let permission_review = bash_permission_review_data(
        command,
        &classification,
        BashExecutionBackend::Local,
        "not_run",
        false,
    );
    let mut result = if let Some(content) = content {
        ToolResult::error_with_content(error, content)
    } else {
        ToolResult::error(error)
    };
    result.data = Some(json!({
        "audit": audit,
        "command_classification": serde_json::to_value(classification).unwrap_or_else(|_| json!({})),
        "permission_review": permission_review
    }));
    result
}

fn timeout_result(
    timeout: u64,
    runtime: &BashRuntimeRef<'_>,
    timing: &TerminalTaskTiming,
) -> ToolResult {
    let message = format!("Command timed out after {} seconds", timeout);
    let (result_preview, result_data) = shell_result_data(ShellResultData {
        audit: runtime.audit,
        command: runtime.command,
        working_dir: runtime.working_dir,
        stdout: "",
        stderr: &message,
        combined_output: &format!("[stderr]:\n{message}"),
        exit_code: -1,
        backend: runtime.backend,
        timed_out: true,
        context: runtime.context,
        terminal_kind: "foreground_shell",
        pty: false,
        sandbox: runtime.sandbox,
        started_at_ms: timing.started_at_ms(),
        ended_at_ms: timing.ended_at_ms(),
        duration_ms: timing.duration_ms(),
    });
    let mut result = ToolResult::error_with_content(message, result_preview);
    result.data = Some(result_data);
    result
}

fn pty_unavailable_result(
    audit: &serde_json::Value,
    command: &str,
    working_dir: &std::path::Path,
    backend: BashExecutionBackend,
    sandbox: bool,
) -> ToolResult {
    let classification = classification_data(command);
    let command_classification = classify_command(command);
    let permission_review = bash_permission_review_data(
        command,
        &command_classification,
        backend,
        "foreground_shell",
        sandbox,
    );
    let message = "Interactive command requires mode=pty";
    let content = "This command looks interactive and requires a PTY-backed terminal. \
Current bash execution mode is non-interactive, so the command was not started. \
Retry with mode=\"pty\" for PTY-backed foreground execution.";
    let mut result = ToolResult::error_with_content(message, content);
    result.error_code = Some(ToolErrorCode::InvalidParams);
    result.data = Some(json!({
        "audit": audit,
        "command_classification": classification.clone(),
        "permission_review": permission_review,
        "terminal_requirement": {
            "requires_pty": true,
            "pty_available": true,
            "pty_used": false,
            "reason": "interactive command requires a PTY-backed execution mode",
            "suggested_recovery": "Retry this bash command with mode=\"pty\", or use a non-interactive command/script with explicit arguments."
        },
        "recovery": {
            "category": "interactive_needs_pty",
            "action": "retry_with_pty_mode",
            "reason": "interactive command requires a PTY-backed execution mode",
            "exit_code": serde_json::Value::Null,
            "command": command
        },
        "shell_result": {
            "command": command,
            "cwd": working_dir.display().to_string(),
            "exit_code": serde_json::Value::Null,
            "stdout_preview": "",
            "stderr_preview": content,
            "output_path": serde_json::Value::Null,
            "duration_ms": serde_json::Value::Null,
            "timed_out": false,
            "truncated": false,
            "classification": classification,
            "evidence_status": "not_run"
        }
    }));
    result
}

fn attach_pty_metadata(data: &mut serde_json::Value, command_requires_pty: bool) {
    if let Some(object) = data.as_object_mut() {
        object.insert(
            "terminal_requirement".to_string(),
            json!({
                "requires_pty": command_requires_pty,
                "pty_available": true,
                "pty_used": true,
                "backend": "portable_pty"
            }),
        );
        if let Some(shell_result) = object
            .get_mut("shell_result")
            .and_then(serde_json::Value::as_object_mut)
        {
            shell_result.insert("pty".to_string(), serde_json::Value::Bool(true));
        }
    }
}

async fn execute_pty_command(
    runtime: &BashRuntimeRef<'_>,
    actual_command: &str,
    timeout: u64,
) -> ToolResult {
    let classification = classify_command(runtime.command);
    let timing = TerminalTaskTiming::start();
    let pty_output = match pty::run_pty_shell(
        actual_command.to_string(),
        runtime.working_dir.to_path_buf(),
        timeout,
    )
    .await
    {
        Ok(output) => output,
        Err(err) => {
            let mut result = error_with_audit(err, None, runtime.audit, runtime.command);
            result.error_code = Some(ToolErrorCode::Unavailable);
            return result;
        }
    };
    let (result_preview, mut result_data) = shell_result_data(ShellResultData {
        audit: runtime.audit,
        command: runtime.command,
        working_dir: runtime.working_dir,
        stdout: &pty_output.output,
        stderr: "",
        combined_output: &pty_output.output,
        exit_code: pty_output.exit_code,
        backend: runtime.backend,
        timed_out: pty_output.timed_out,
        context: runtime.context,
        terminal_kind: "pty_shell",
        pty: true,
        sandbox: runtime.sandbox,
        started_at_ms: timing.started_at_ms(),
        ended_at_ms: timing.ended_at_ms(),
        duration_ms: timing.duration_ms(),
    });
    attach_pty_metadata(&mut result_data, classification.requires_pty());

    if pty_output.exit_code == 0 && !pty_output.timed_out {
        ToolResult::success_with_data(result_preview, result_data)
    } else if pty_output.timed_out {
        let mut result = ToolResult::error_with_content(
            format!("PTY command timed out after {} seconds", timeout),
            result_preview,
        );
        result.error_code = Some(ToolErrorCode::Timeout);
        result.data = Some(result_data);
        result
    } else {
        let mut result = ToolResult::error_with_content(
            format!(
                "PTY command failed with exit code: {}",
                pty_output.exit_code
            ),
            result_preview,
        );
        result.data = Some(result_data);
        result
    }
}

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn description(&self) -> &str {
        "Run a shell-only command; returns stdout+stderr. \
         Read-only/test/lint/typecheck commands run immediately; mutating/network/install \
         commands require confirmation. For file ops use file_read, file_write, \
         or file_edit for validation and rollback. Supports pipes/chains/redirects; \
         rejects background, heredoc, command substitution, and subshells. \
         Do not use bash output as user-facing communication; summarize results."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The bash command to execute"
                },
                "description": {
                    "type": "string",
                    "description": "A brief internal description of what this command does (for logging, not user-facing communication)"
                },
                "timeout": {
                    "type": "integer",
                    "description": "Timeout in seconds (default: 60)",
                    "default": 60
                },
                "mode": {
                    "type": "string",
                    "enum": ["foreground", "background", "pty"],
                    "description": "Run normally, start a background command and return a handle, or run a foreground command through a PTY-backed terminal.",
                    "default": "foreground"
                },
                "working_dir": {
                    "type": "string",
                    "description": "Working directory for the command (optional, defaults to current)"
                },
                "sandbox": {
                    "type": "boolean",
                    "description": "Apply soft resource limits (ulimit) only. NOTE: This is NOT a real sandbox and does NOT prevent filesystem or network access. For true isolation, use OS-level containers.",
                    "default": false
                },
                "backend": {
                    "type": "string",
                    "enum": ["local", "restricted", "external"],
                    "description": "Execution backend. local=normal shell, restricted=soft-limited env, external=wrapper command from PRIORITY_AGENT_BASH_EXTERNAL_CMD (or PRIORITY_AGENT_BASH_SANDBOX_CMD)."
                }
            },
            "required": ["command"]
        })
    }

    fn to_classifier_input(&self, params: &serde_json::Value) -> String {
        let cmd = params["command"].as_str().unwrap_or("");
        format!("bash: {}", cmd)
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["shell"]
    }

    fn search_hint(&self) -> Option<&'static str> {
        Some("shell validation git package managers")
    }

    fn strict_schema(&self) -> bool {
        true
    }

    fn operation_kind(&self, params: &serde_json::Value) -> ToolOperationKind {
        let command = params["command"].as_str().unwrap_or("");
        match classify_command(command).category {
            ShellCommandCategory::Read => ToolOperationKind::Read,
            ShellCommandCategory::List => ToolOperationKind::List,
            ShellCommandCategory::Search => ToolOperationKind::Search,
            _ => ToolOperationKind::Shell,
        }
    }

    fn is_read_only(&self, params: &serde_json::Value) -> bool {
        matches!(
            self.operation_kind(params),
            ToolOperationKind::Read | ToolOperationKind::List | ToolOperationKind::Search
        )
    }

    fn is_concurrency_safe(&self, params: &serde_json::Value) -> bool {
        self.is_read_only(params)
    }

    fn is_destructive(&self, params: &serde_json::Value) -> bool {
        let command = params["command"].as_str().unwrap_or("");
        classify_command(command).category == ShellCommandCategory::Destructive
    }

    fn is_search_or_read_command(&self, params: &serde_json::Value) -> ToolSearchOrReadSemantics {
        let command = params["command"].as_str().unwrap_or("");
        match classify_command(command).category {
            ShellCommandCategory::Search => ToolSearchOrReadSemantics {
                is_search: true,
                ..Default::default()
            },
            ShellCommandCategory::Read => ToolSearchOrReadSemantics {
                is_read: true,
                ..Default::default()
            },
            ShellCommandCategory::List => ToolSearchOrReadSemantics {
                is_list: true,
                ..Default::default()
            },
            _ => ToolSearchOrReadSemantics::default(),
        }
    }

    fn input_paths(&self, params: &serde_json::Value) -> Vec<String> {
        let command = params["command"].as_str().unwrap_or("");
        classify_command(command).path_patterns
    }

    fn permission_matcher_input(&self, params: &serde_json::Value) -> Option<String> {
        params["command"]
            .as_str()
            .map(str::trim)
            .filter(|command| !command.is_empty())
            .map(str::to_string)
    }

    fn tool_use_summary(&self, params: &serde_json::Value) -> Option<String> {
        let command = params["command"].as_str()?.trim();
        if command.is_empty() {
            None
        } else {
            Some(short_command_summary(command))
        }
    }

    fn activity_description(&self, params: &serde_json::Value) -> Option<String> {
        let summary = self.tool_use_summary(params)?;
        let verb = match self.operation_kind(params) {
            ToolOperationKind::Read | ToolOperationKind::List | ToolOperationKind::Search => {
                "Inspecting"
            }
            _ => "Running",
        };
        Some(format!("{verb}: {summary}"))
    }

    async fn execute(&self, params: serde_json::Value, context: ToolContext) -> ToolResult {
        let command = params["command"].as_str().unwrap_or("");
        if command.is_empty() {
            return ToolResult::error("Command cannot be empty");
        }

        let description = params["description"].as_str().unwrap_or(command);
        let timeout = effective_timeout_secs(params["timeout"].as_u64());
        let mode = params["mode"].as_str().unwrap_or("foreground");
        if !matches!(mode, "foreground" | "background" | "pty") {
            return ToolResult::error("mode must be 'foreground', 'background', or 'pty'");
        }

        // working_dir 安全校验
        let working_dir = if let Some(wd_str) = params["working_dir"].as_str() {
            let wd = std::path::PathBuf::from(wd_str);
            // 拒绝包含 .. 的路径
            if wd
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
            {
                return ToolResult::error("working_dir cannot contain '..'");
            }
            // 如果是绝对路径，必须位于项目目录或临时目录下
            if wd.is_absolute() {
                let project_root = if context.working_dir.is_absolute() {
                    context.working_dir.clone()
                } else {
                    std::env::current_dir()
                        .unwrap_or_else(|_| std::path::PathBuf::from("."))
                        .join(&context.working_dir)
                };
                let project_root = project_root.canonicalize().unwrap_or(project_root);
                let checked_wd = wd.canonicalize().unwrap_or_else(|_| wd.clone());
                let in_project = checked_wd.starts_with(&project_root);
                let in_tmp = wd.starts_with("/tmp") || wd.starts_with("/var/tmp");
                if !in_project && !in_tmp {
                    return ToolResult::error(
                        "absolute working_dir must be within project directory or /tmp",
                    );
                }
                wd
            } else {
                // 相对路径：相对于 context.working_dir 解析
                context.working_dir.join(wd)
            }
        } else {
            context.working_dir.clone()
        };

        let sandbox = params["sandbox"].as_bool().unwrap_or(false);
        let requested_backend_raw = params["backend"].as_str().map(ToString::to_string);
        let requested_backend = params["backend"].as_str().and_then(parse_backend);
        let mut backend = requested_backend.unwrap_or_else(default_backend);
        let backend_requested_name = requested_backend_raw.unwrap_or_else(|| {
            std::env::var("PRIORITY_AGENT_BASH_BACKEND").unwrap_or_else(|_| "local".to_string())
        });
        let mut fallback_reason: Option<String> = None;
        if sandbox && backend == BashExecutionBackend::Local {
            backend = BashExecutionBackend::Restricted;
        }

        if backend == BashExecutionBackend::External {
            if let Err(external_err) = external_command(command) {
                if let Some(fallback) = external_fallback_backend() {
                    fallback_reason = Some(format!(
                        "external backend unavailable: {}; fallback to {}",
                        external_err,
                        fallback.as_str()
                    ));
                    backend = fallback;
                } else {
                    let audit = build_audit(
                        &backend_requested_name,
                        backend.as_str(),
                        Some(&external_err),
                        sandbox,
                        timeout,
                        &working_dir,
                    );
                    return error_with_audit(external_err, None, &audit, command);
                }
            }
        }

        let audit = build_audit(
            &backend_requested_name,
            backend.as_str(),
            fallback_reason.as_deref(),
            sandbox,
            timeout,
            &working_dir,
        );
        let runtime = BashRuntimeRef {
            audit: &audit,
            command,
            working_dir: &working_dir,
            backend,
            sandbox,
            context: &context,
        };

        if sandbox || backend == BashExecutionBackend::Restricted {
            warn!("restricted backend only applies soft resource limits (ulimit) and minimal env; it does NOT provide true process isolation and will NOT block all dangerous filesystem or network operations");
        } else if backend == BashExecutionBackend::External {
            warn!("external backend delegates isolation to wrapper command; safety depends on PRIORITY_AGENT_BASH_EXTERNAL_CMD");
        }

        info!(
            "Executing bash command: {} (description: {}, timeout: {}s, sandbox: {}, backend: {})",
            command,
            description,
            timeout,
            sandbox,
            backend.as_str()
        );
        debug!("Working directory: {:?}", working_dir);

        // 检查危险命令
        if is_dangerous_command(command) {
            warn!("Potentially dangerous command detected: {}", command);
            if !context.permissions.allow_all_bash {
                return error_with_audit(
                    format!(
                        "Dangerous command detected: {}. \
                             This command appears to be destructive. \
                             Use with caution.",
                        command
                    ),
                    None,
                    &audit,
                    command,
                );
            }
        }

        let classification = classify_command(command);
        if classification.requires_pty() && mode != "pty" {
            return pty_unavailable_result(&audit, command, &working_dir, backend, sandbox);
        }

        // 执行命令（带超时 + 子进程 kill）
        let mut cmd = Command::new("bash");

        // 后端选择：restricted 走受限执行包装
        let actual_command = match backend {
            BashExecutionBackend::Local => command.to_string(),
            BashExecutionBackend::Restricted => restricted_command(command),
            BashExecutionBackend::External => match external_command(command) {
                Ok(cmd) => cmd,
                Err(msg) => return error_with_audit(msg, None, &audit, command),
            },
        };

        let auto_background = auto_background_decision(command, &classification, mode, timeout);
        if mode == "background" || auto_background.is_some() {
            return match background::start_background_shell(
                command,
                &actual_command,
                &working_dir,
                backend,
                timeout,
            )
            .await
            {
                Ok(snapshot) => {
                    let mut content = background_started_content(&snapshot);
                    let mut data = background_shell_result_data(&snapshot);
                    if let Some(decision) = auto_background.as_ref() {
                        content.push_str(&format!(
                            "\nAuto-background: {} command exceeded {}s foreground threshold.",
                            decision.reason, decision.threshold_secs
                        ));
                        attach_auto_background_metadata(&mut data, decision);
                    }
                    ToolResult::success_with_data(content, data)
                }
                Err(err) => error_with_audit(err, None, &audit, command),
            };
        }

        if mode == "pty" {
            return execute_pty_command(&runtime, &actual_command, timeout).await;
        }

        let timing = TerminalTaskTiming::start();
        cmd.arg("-c")
            .arg(&actual_command)
            .current_dir(&working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        sanitize_agent_runtime_env(&mut cmd);

        #[cfg(unix)]
        unsafe {
            // 让子进程成为新的进程组 leader，超时时可一次性 kill 整棵进程树。
            cmd.pre_exec(|| {
                if libc::setpgid(0, 0) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }

        let child = match cmd.spawn() {
            Ok(child) => child,
            Err(e) => {
                error!("Failed to spawn command: {}", e);
                return error_with_audit(
                    format!("Failed to spawn command: {}", e),
                    None,
                    &audit,
                    command,
                );
            }
        };

        let child_pid = child.id().map(|id| id as i32);
        let wait_fut = child.wait_with_output();
        tokio::pin!(wait_fut);

        let (output, timed_out) = tokio::select! {
            res = &mut wait_fut => {
                match res {
                    Ok(output) => (output, false),
                    Err(e) => {
                        error!("Failed to execute command: {}", e);
                        return error_with_audit(format!("Failed to execute command: {}", e), None, &audit, command);
                    }
                }
            }
            _ = tokio::time::sleep(std::time::Duration::from_secs(timeout)) => {
                warn!("Command timed out after {}s, killing process tree (pid: {:?})", timeout, child_pid);
                kill_process_tree(child_pid);

                match tokio::time::timeout(std::time::Duration::from_secs(2), &mut wait_fut).await {
                    Ok(Ok(output)) => (output, true),
                    Ok(Err(e)) => {
                        error!("Command timed out and failed while collecting output: {}", e);
                        return timeout_result(timeout, &runtime, &timing);
                    }
                    Err(_) => {
                        return timeout_result(timeout, &runtime, &timing);
                    }
                }
            }
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let exit_code = output.status.code().unwrap_or(-1);

        debug!("Command exit code: {}", exit_code);
        debug!("Stdout length: {} bytes", stdout.len());
        debug!("Stderr length: {} bytes", stderr.len());

        // 构建结果
        let mut result_content = String::new();

        if !stdout.is_empty() {
            result_content.push_str(&stdout);
        }

        if !stderr.is_empty() {
            if !result_content.is_empty() {
                result_content.push_str("\n\n[stderr]:\n");
            } else {
                result_content.push_str("[stderr]:\n");
            }
            result_content.push_str(&stderr);
        }

        let (result_preview, result_data) = shell_result_data(ShellResultData {
            audit: &audit,
            command,
            working_dir: &working_dir,
            stdout: &stdout,
            stderr: &stderr,
            combined_output: &result_content,
            exit_code,
            backend,
            timed_out,
            context: &context,
            terminal_kind: "foreground_shell",
            pty: false,
            sandbox,
            started_at_ms: timing.started_at_ms(),
            ended_at_ms: timing.ended_at_ms(),
            duration_ms: timing.duration_ms(),
        });
        if matches!(
            classification.category,
            ShellCommandCategory::Read | ShellCommandCategory::List | ShellCommandCategory::Search
        ) {
            if let Some(store) = context.session_store.as_ref() {
                let output_hash = shell_output_hash(&stdout, &stderr);
                record_bash_read(
                    store,
                    &BashReadLedgerInput {
                        session_id: &context.session_id,
                        command,
                        cwd: &working_dir.display().to_string(),
                        category: shell_category_name(classification.category),
                        exit_code,
                        stdout_bytes: stdout.len(),
                        stderr_bytes: stderr.len(),
                        output_hash: &output_hash,
                        timed_out,
                    },
                );
            }
        }

        if output.status.success() {
            ToolResult::success_with_data(result_preview, result_data)
        } else if timed_out {
            let mut result = ToolResult::error_with_content(
                format!("Command timed out after {} seconds", timeout),
                result_preview,
            );
            result.data = Some(result_data);
            result
        } else {
            let mut result = ToolResult::error_with_content(
                format!("Command failed with exit code: {}", exit_code),
                result_preview,
            );
            result.data = Some(result_data);
            result
        }
    }

    fn requires_confirmation(&self, params: &serde_json::Value) -> bool {
        if let Some(cmd) = params["command"].as_str() {
            is_dangerous_command(cmd)
        } else {
            false
        }
    }

    fn confirmation_prompt(&self, params: &serde_json::Value) -> Option<String> {
        params["command"]
            .as_str()
            .map(|cmd| format!("This command may be destructive: {}\nAllow execution?", cmd))
    }
}

fn kill_process_tree(child_pid: Option<i32>) {
    #[cfg(unix)]
    {
        if let Some(pid) = child_pid {
            // kill(-pgid) 发送到整个进程组，避免遗留后台子进程。
            let _ = unsafe { libc::kill(-pid, libc::SIGKILL) };
        }
    }

    #[cfg(not(unix))]
    {
        if let Some(pid) = child_pid {
            if pid > 0 {
                let _ = std::process::Command::new("taskkill")
                    .args(["/PID", &pid.to_string(), "/T", "/F"])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status();
            }
        }
    }
}

// Re-export is_dangerous_command from security module
pub use crate::security::is_dangerous_command;

#[cfg(test)]
mod tests;
