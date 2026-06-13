use crate::{
    engine::runtime_facade::{ProviderPhase, ToolTurnPhase},
    state::RuntimeToolStatus,
    tui::{app::TuiApp, tool_view::ToolRunStatus},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivePhase {
    ProviderWaiting,
    ProviderRetrying,
    ProviderSlow,
    ProviderTimedOut,
    ToolRunning,
    PermissionWaiting,
    Writing,
    Thinking,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveTurnStatus {
    pub phase: ActivePhase,
    pub label: String,
    pub detail: Option<String>,
    pub elapsed_ms: Option<u64>,
    pub interrupt_hint: bool,
}

const SLOW_PROVIDER_WAIT_MS: u64 = 10_000;

impl ActiveTurnStatus {
    fn new(phase: ActivePhase, label: impl Into<String>) -> Self {
        Self {
            phase,
            label: label.into(),
            detail: None,
            elapsed_ms: None,
            interrupt_hint: true,
        }
    }

    fn with_detail(mut self, detail: impl Into<String>) -> Self {
        let detail = detail.into();
        if !detail.trim().is_empty() {
            self.detail = Some(detail);
        }
        self
    }

    fn with_elapsed(mut self, elapsed_ms: Option<u64>) -> Self {
        self.elapsed_ms = elapsed_ms.filter(|ms| *ms > 0);
        self
    }
}

pub fn active_turn_status(app: &TuiApp) -> Option<ActiveTurnStatus> {
    let runtime = app.runtime_status_snapshot_now();
    let local_elapsed_ms = query_elapsed_ms(app);
    if !app.is_querying && !app.facade_snapshot.provider_request.phase.is_active() {
        return None;
    }

    if let Some(tool) = app
        .runtime_state_snapshot
        .tool_uses
        .iter()
        .rev()
        .find(|tool| tool.active || tool.status == RuntimeToolStatus::WaitingPermission)
    {
        let phase = if tool.status == RuntimeToolStatus::WaitingPermission {
            ActivePhase::PermissionWaiting
        } else {
            ActivePhase::ToolRunning
        };
        let label = non_empty(&tool.transcript_summary)
            .or_else(|| non_empty(&tool.latest_progress))
            .unwrap_or_else(|| non_empty_str(&tool.summary).unwrap_or_else(|| tool.name.clone()));
        return Some(
            ActiveTurnStatus::new(phase, label)
                .with_detail(tool.status.label().replace('_', " "))
                .with_elapsed(tool.elapsed_ms),
        );
    }

    if let Some(run) = app
        .tool_runs_snapshot
        .iter()
        .rev()
        .find(|run| run.is_active())
    {
        let phase = if run.status == ToolRunStatus::WaitingPermission {
            ActivePhase::PermissionWaiting
        } else {
            ActivePhase::ToolRunning
        };
        return Some(
            ActiveTurnStatus::new(phase, run.summary())
                .with_detail(tool_run_status_label(run.status))
                .with_elapsed(Some(run.elapsed().as_millis() as u64)),
        );
    }

    if let Some(pending) = runtime.pending_permission.as_deref().or(app
        .runtime_state_snapshot
        .permission
        .pending_tool
        .as_deref())
    {
        return Some(
            ActiveTurnStatus::new(ActivePhase::PermissionWaiting, "Permission needed")
                .with_detail(pending.to_string()),
        );
    }

    if let Some(turn) = app
        .facade_snapshot
        .tool_turns
        .iter()
        .rev()
        .find(|turn| !turn.phase.is_terminal())
    {
        let (phase, label) = tool_turn_active_label(turn.phase, &turn.name);
        return Some(
            ActiveTurnStatus::new(phase, label)
                .with_detail(turn.phase.label().to_string())
                .with_elapsed(local_elapsed_ms),
        );
    }

    let provider = &app.facade_snapshot.provider_request;
    if app.is_querying || provider.phase.is_active() {
        let elapsed_ms = if provider.phase.is_active() {
            provider.elapsed_ms
        } else {
            local_elapsed_ms
                .or((provider.elapsed_ms > 0).then_some(provider.elapsed_ms))
                .unwrap_or_default()
        };
        if let Some(timeout_ms) = effective_provider_timeout_ms(provider.timeout_ms) {
            if elapsed_ms >= timeout_ms {
                let label = format!(
                    "Error: provider request timed out after {:.1}s",
                    timeout_ms as f64 / 1000.0
                );
                let detail = provider
                    .message
                    .clone()
                    .or_else(|| provider.model.clone())
                    .or_else(|| provider.request_shape.clone());
                return Some(
                    ActiveTurnStatus::new(ActivePhase::ProviderTimedOut, label)
                        .with_detail(detail.unwrap_or_default())
                        .with_elapsed(Some(elapsed_ms)),
                );
            }
        }
    }
    if provider.phase == ProviderPhase::TimedOut {
        let elapsed_ms = (provider.elapsed_ms > 0)
            .then_some(provider.elapsed_ms)
            .or((provider.timeout_ms > 0).then_some(provider.timeout_ms));
        return Some(
            ActiveTurnStatus::new(
                ActivePhase::ProviderTimedOut,
                provider_label(
                    ProviderPhase::TimedOut,
                    &app.current_provider_label(),
                    provider.is_known_slow_path,
                    elapsed_ms.unwrap_or_default(),
                ),
            )
            .with_detail(
                provider
                    .message
                    .clone()
                    .or_else(|| provider.model.clone())
                    .unwrap_or_default(),
            )
            .with_elapsed(elapsed_ms),
        );
    }
    if provider.phase.is_active() {
        let elapsed_ms = provider.elapsed_ms;
        let phase = match provider.phase {
            ProviderPhase::Retrying => ActivePhase::ProviderRetrying,
            ProviderPhase::SlowWarning => ActivePhase::ProviderSlow,
            ProviderPhase::Started if elapsed_ms >= SLOW_PROVIDER_WAIT_MS => {
                ActivePhase::ProviderSlow
            }
            _ => ActivePhase::ProviderWaiting,
        };
        let label = provider_label(
            if phase == ActivePhase::ProviderSlow {
                ProviderPhase::SlowWarning
            } else {
                provider.phase
            },
            &app.current_provider_label(),
            provider.is_known_slow_path,
            elapsed_ms,
        );
        let detail = provider
            .message
            .clone()
            .or_else(|| provider.model.clone())
            .or_else(|| provider.request_shape.clone());
        return Some(
            ActiveTurnStatus::new(phase, label)
                .with_detail(detail.unwrap_or_default())
                .with_elapsed(Some(elapsed_ms)),
        );
    }

    if app.facade_snapshot.assistant_streaming {
        return Some(
            ActiveTurnStatus::new(ActivePhase::Writing, "Writing").with_elapsed(local_elapsed_ms),
        );
    }

    if app.is_querying {
        let elapsed_ms = local_elapsed_ms;
        if elapsed_ms
            .map(|elapsed| elapsed >= SLOW_PROVIDER_WAIT_MS)
            .unwrap_or(false)
        {
            return Some(
                ActiveTurnStatus::new(
                    ActivePhase::ProviderSlow,
                    provider_label(
                        ProviderPhase::SlowWarning,
                        &app.current_provider_label(),
                        false,
                        elapsed_ms.unwrap_or_default(),
                    ),
                )
                .with_detail(app.current_model_label())
                .with_elapsed(elapsed_ms),
            );
        }
        let label = runtime
            .current_tool_label
            .filter(|label| !label.trim().is_empty())
            .unwrap_or_else(|| "Thinking".to_string());
        return Some(ActiveTurnStatus::new(ActivePhase::Thinking, label).with_elapsed(elapsed_ms));
    }

    None
}

fn query_elapsed_ms(app: &TuiApp) -> Option<u64> {
    app.stream_started_at
        .map(|started| started.elapsed().as_millis() as u64)
        .filter(|elapsed| *elapsed > 0)
}

fn effective_provider_timeout_ms(provider_timeout_ms: u64) -> Option<u64> {
    let runtime_config = crate::services::config::runtime_config();
    let explicit_timeout_ms = runtime_config
        .explicit_llm_request_timeout()
        .map(|timeout| timeout.as_millis() as u64);
    let timeout_ms = if provider_timeout_ms > 0 {
        explicit_timeout_ms
            .map(|explicit| explicit.min(provider_timeout_ms))
            .unwrap_or(provider_timeout_ms)
    } else {
        explicit_timeout_ms?
    };
    (timeout_ms > 0).then_some(timeout_ms)
}

pub fn format_elapsed(elapsed_ms: u64) -> String {
    if elapsed_ms < 1_000 {
        format!("{}ms", elapsed_ms)
    } else {
        format!("{:.1}s", elapsed_ms as f64 / 1000.0)
    }
}

fn non_empty(value: &Option<String>) -> Option<String> {
    value.as_deref().and_then(non_empty_str)
}

fn non_empty_str(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn provider_label(
    phase: ProviderPhase,
    provider_label: &str,
    is_known_slow_path: bool,
    elapsed_ms: u64,
) -> String {
    let provider = non_empty_str(provider_label).unwrap_or_else(|| "provider".to_string());
    match phase {
        ProviderPhase::Started => {
            if is_known_slow_path {
                format!("non-streaming tool request ({provider})")
            } else {
                format!("waiting on {provider}")
            }
        }
        ProviderPhase::Retrying => format!("retrying {provider}"),
        ProviderPhase::SlowWarning => {
            format!("slow {provider} ({:.1}s)", elapsed_ms as f64 / 1000.0)
        }
        _ => phase.label().to_string(),
    }
}

fn tool_turn_active_label(phase: ToolTurnPhase, name: &str) -> (ActivePhase, String) {
    let tool_name = non_empty_str(name).unwrap_or_else(|| "tool".to_string());
    match phase {
        ToolTurnPhase::Requested => (
            ActivePhase::ProviderWaiting,
            format!("tool requested ({tool_name})"),
        ),
        ToolTurnPhase::Accepted => (
            ActivePhase::ProviderWaiting,
            format!("tool accepted ({tool_name})"),
        ),
        ToolTurnPhase::Executing => (ActivePhase::ToolRunning, format!("running {tool_name}")),
        ToolTurnPhase::ResultObserved => (
            ActivePhase::ProviderWaiting,
            format!("tool result observed ({tool_name})"),
        ),
        ToolTurnPhase::SentBackToModel => (
            ActivePhase::ProviderWaiting,
            format!("sent tool result to model ({tool_name})"),
        ),
        ToolTurnPhase::FinalAnswer => (
            ActivePhase::Thinking,
            format!("final answer ready ({tool_name})"),
        ),
        ToolTurnPhase::Persisted
        | ToolTurnPhase::Failed
        | ToolTurnPhase::Cancelled
        | ToolTurnPhase::TimedOut => (ActivePhase::Thinking, tool_name),
    }
}

fn tool_run_status_label(status: ToolRunStatus) -> &'static str {
    match status {
        ToolRunStatus::Queued => "queued",
        ToolRunStatus::Running => "running",
        ToolRunStatus::Backgrounded => "backgrounded",
        ToolRunStatus::WaitingPermission => "waiting permission",
        ToolRunStatus::TimedOut => "timed out",
        ToolRunStatus::Cancelled => "cancelled",
        ToolRunStatus::Completed => "completed",
        ToolRunStatus::Failed => "failed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        engine::runtime_facade::ProviderPhase,
        state::{RuntimeToolStatus, RuntimeToolUse},
    };

    #[test]
    fn selector_prefers_active_tool_over_provider_wait() {
        let mut app = TuiApp::new();
        app.is_querying = true;
        app.facade_snapshot.provider_request.phase = ProviderPhase::Started;
        app.facade_snapshot.provider_request.provider_family = Some("deepseek".to_string());
        app.runtime_state_snapshot.tool_uses.push(RuntimeToolUse {
            id: "tool_1".to_string(),
            name: "bash".to_string(),
            summary: "Running cargo test".to_string(),
            status: RuntimeToolStatus::Running,
            active: true,
            arguments: None,
            latest_progress: None,
            result_preview: None,
            elapsed_ms: Some(2_500),
            operation_kind: None,
            ui_render_kind: None,
            read_only: None,
            concurrency_safe: None,
            destructive: None,
            input_paths: Vec::new(),
            transcript_summary: None,
        });

        let status = active_turn_status(&app).expect("active status");

        assert_eq!(status.phase, ActivePhase::ToolRunning);
        assert_eq!(status.label, "Running cargo test");
        assert_eq!(status.elapsed_ms, Some(2_500));
    }

    #[test]
    fn selector_uses_provider_when_no_tool_is_active() {
        let mut app = TuiApp::new();
        app.is_querying = true;
        app.facade_snapshot.provider_request.phase = ProviderPhase::Started;
        app.facade_snapshot.provider_request.provider_family =
            Some("openai_compatible".to_string());
        app.facade_snapshot.provider_request.model = Some("deepseek-v4-flash".to_string());
        app.facade_snapshot.provider_request.elapsed_ms = 2_700;

        let status = active_turn_status(&app).expect("active status");

        assert_eq!(status.phase, ActivePhase::ProviderWaiting);
        assert_eq!(status.label, "waiting on DeepSeek");
        assert_eq!(status.elapsed_ms, Some(2_700));
    }

    #[test]
    fn selector_keeps_elapsed_for_generic_thinking_state() {
        let mut app = TuiApp::new();
        app.is_querying = true;
        app.stream_started_at = Some(std::time::Instant::now() - std::time::Duration::from_secs(3));

        let status = active_turn_status(&app).expect("active status");

        assert_eq!(status.phase, ActivePhase::Thinking);
        assert_eq!(status.label, "Thinking");
        assert!(status.elapsed_ms.is_some_and(|elapsed| elapsed >= 3_000));
    }

    #[test]
    fn selector_promotes_long_generic_wait_to_slow_provider() {
        let mut app = TuiApp::new();
        app.is_querying = true;
        app.stream_started_at =
            Some(std::time::Instant::now() - std::time::Duration::from_secs(12));
        app.facade_snapshot.provider_request.provider_family = Some("deepseek".to_string());
        app.facade_snapshot.provider_request.model = Some("deepseek-v4-flash".to_string());

        let status = active_turn_status(&app).expect("active status");

        assert_eq!(status.phase, ActivePhase::ProviderSlow);
        assert_eq!(status.label, "slow DeepSeek (12.0s)");
        assert_eq!(status.detail.as_deref(), Some("deepseek-v4-flash"));
        assert!(status.elapsed_ms.is_some_and(|elapsed| elapsed >= 12_000));
    }

    #[test]
    fn selector_uses_current_provider_elapsed_for_active_provider_phase() {
        let mut app = TuiApp::new();
        app.is_querying = true;
        app.stream_started_at =
            Some(std::time::Instant::now() - std::time::Duration::from_secs(11));
        app.facade_snapshot.provider_request.phase = ProviderPhase::Started;
        app.facade_snapshot.provider_request.provider_family = Some("deepseek".to_string());
        app.facade_snapshot.provider_request.elapsed_ms = 1_000;
        app.facade_snapshot.provider_request.model = Some("deepseek-v4-flash".to_string());

        let status = active_turn_status(&app).expect("active status");

        assert_eq!(status.phase, ActivePhase::ProviderWaiting);
        assert_eq!(status.label, "waiting on DeepSeek");
        assert_eq!(status.elapsed_ms, Some(1_000));
    }

    #[test]
    fn selector_shows_provider_timeout_before_generic_slow_wait() {
        let mut guard = crate::test_utils::env_guard::EnvVarGuard::acquire_blocking();
        guard.remove("PRIORITY_AGENT_LLM_REQUEST_TIMEOUT_SECS");

        let mut app = TuiApp::new();
        app.is_querying = true;
        app.stream_started_at =
            Some(std::time::Instant::now() - std::time::Duration::from_secs(130));
        app.facade_snapshot.provider_request.phase = ProviderPhase::TimedOut;
        app.facade_snapshot.provider_request.provider_family = Some("deepseek".to_string());
        app.facade_snapshot.provider_request.model = Some("deepseek-v4-flash".to_string());
        app.facade_snapshot.provider_request.timeout_ms = 120_000;
        app.facade_snapshot.provider_request.elapsed_ms = 120_001;
        app.facade_snapshot.provider_request.message =
            Some("provider request timed out after 120.0s".to_string());

        let status = active_turn_status(&app).expect("active status");

        assert_eq!(status.phase, ActivePhase::ProviderTimedOut);
        assert_eq!(
            status.label,
            "Error: provider request timed out after 120.0s"
        );
        assert_eq!(
            status.detail.as_deref(),
            Some("provider request timed out after 120.0s")
        );
        assert!(status.elapsed_ms.is_some_and(|elapsed| elapsed >= 120_000));
    }

    #[test]
    fn selector_renders_explicit_timeout_as_error_state() {
        let mut guard = crate::test_utils::env_guard::EnvVarGuard::acquire_blocking();
        guard.set("PRIORITY_AGENT_LLM_REQUEST_TIMEOUT_SECS", "30");

        let mut app = TuiApp::new();
        app.is_querying = true;
        app.stream_started_at =
            Some(std::time::Instant::now() - std::time::Duration::from_secs(31));
        app.facade_snapshot.provider_request.provider_family = Some("deepseek".to_string());
        app.facade_snapshot.provider_request.model = Some("deepseek-v4-flash".to_string());
        app.facade_snapshot.provider_request.timeout_ms = 120_000;

        let status = active_turn_status(&app).expect("active status");

        assert_eq!(status.phase, ActivePhase::ProviderTimedOut);
        assert_eq!(
            status.label,
            "Error: provider request timed out after 30.0s"
        );
        assert_eq!(status.detail.as_deref(), Some("deepseek-v4-flash"));
    }

    #[test]
    fn selector_uses_tool_turn_spine_when_no_legacy_tool_is_active() {
        let mut app = TuiApp::new();
        app.is_querying = true;
        app.facade_snapshot
            .tool_turns
            .push(crate::engine::runtime_facade::ToolTurnSnapshot {
                id: "call_1".to_string(),
                name: "bash".to_string(),
                parent_message_id: None,
                phase: ToolTurnPhase::SentBackToModel,
                arguments_preview: Some("{\"command\":\"pwd\"}".to_string()),
                result_preview: Some("Result: OK".to_string()),
                failure: None,
            });

        let status = active_turn_status(&app).expect("active status");

        assert_eq!(status.phase, ActivePhase::ProviderWaiting);
        assert_eq!(status.label, "sent tool result to model (bash)");
        assert_eq!(status.detail.as_deref(), Some("sent back to model"));
    }

    #[test]
    fn selector_renders_assistant_streaming_as_writing() {
        let mut app = TuiApp::new();
        app.is_querying = true;
        app.stream_started_at = Some(std::time::Instant::now() - std::time::Duration::from_secs(2));
        app.facade_snapshot.assistant_streaming = true;

        let status = active_turn_status(&app).expect("active status");

        assert_eq!(status.phase, ActivePhase::Writing);
        assert_eq!(status.label, "Writing");
        assert!(status.elapsed_ms.is_some_and(|elapsed| elapsed >= 2_000));
    }
}
