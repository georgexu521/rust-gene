//! TUI application state support.
//!
//! Keeps runtime state, memory panels, slash commands, and status tools separate from rendering.

use crate::engine::human_review::PermissionReviewDecision;
use crate::permissions::{PermissionMode, PermissionRules, RuleSource, SourcedRule};
use crate::state::{RuntimeTerminalTask, RuntimeToolStatus, RuntimeToolUse};
use crate::tui::tool_view::{ToolRunStatus, ToolRunView};

#[derive(Debug, Clone)]
pub(super) struct SkillOutcomeAttribution {
    pub(super) success: bool,
    pub(super) acceptance_passed: Option<bool>,
    pub(super) tests_passed: Option<bool>,
    pub(super) user_satisfaction: Option<f32>,
    pub(super) risk_penalty: f32,
    pub(super) confidence: f32,
    pub(super) source: &'static str,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct StreamUsageSnapshot {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub reasoning_tokens: Option<u32>,
    pub cached_tokens: Option<u32>,
    pub cache_write_tokens: Option<u32>,
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

pub(super) fn skill_outcome_attribution(
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

pub(super) fn runtime_tool_use_from_view(run: &ToolRunView) -> RuntimeToolUse {
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

pub(super) fn runtime_terminal_task_from_view(run: &ToolRunView) -> Option<RuntimeTerminalTask> {
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

pub(super) fn tool_run_status_label(status: ToolRunStatus) -> &'static str {
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

pub(super) fn read_git_branch_fast(cwd: &std::path::Path) -> Option<String> {
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

pub(super) fn provider_name_from_base_url(base_url: &str) -> &'static str {
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

pub(super) fn permission_review_decision_for_response(
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
