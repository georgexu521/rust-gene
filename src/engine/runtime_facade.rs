//! Runtime facade for unified frontend behavior.
//!
//! This module is the single source of truth for product runtime state that
//! should be shared across frontends. TUI and desktop render facade events
//! rather than duplicating runtime policy.

use crate::session_store::SessionProjectionEvent;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Provider request lifecycle state shared across frontends.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderRequestLifecycle {
    pub phase: ProviderPhase,
    pub provider_family: Option<String>,
    pub model: Option<String>,
    pub request_shape: Option<String>,
    pub elapsed_ms: u64,
    pub timeout_ms: u64,
    pub slow_warning_threshold_ms: u64,
    pub is_known_slow_path: bool,
    pub slow_warning_emitted: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderPhase {
    #[default]
    Idle,
    Started,
    Retrying,
    SlowWarning,
    Completed,
    TimedOut,
    Cancelled,
}

impl ProviderPhase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "",
            Self::Started => "waiting for provider",
            Self::Retrying => "retrying provider",
            Self::SlowWarning => "slow provider",
            Self::Completed => "provider done",
            Self::TimedOut => "provider timeout",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn is_active(self) -> bool {
        matches!(self, Self::Started | Self::Retrying | Self::SlowWarning)
    }
}

impl ProviderRequestLifecycle {
    pub fn status_label(&self) -> String {
        match self.phase {
            ProviderPhase::Idle => String::new(),
            ProviderPhase::Started => {
                if self.is_known_slow_path {
                    format!(
                        "non-streaming tool request ({})",
                        self.provider_family.as_deref().unwrap_or("unknown")
                    )
                } else {
                    format!(
                        "waiting on {}",
                        self.provider_family.as_deref().unwrap_or("provider")
                    )
                }
            }
            ProviderPhase::Retrying => {
                format!(
                    "retrying {}",
                    self.provider_family.as_deref().unwrap_or("provider")
                )
            }
            ProviderPhase::SlowWarning => {
                format!(
                    "slow {} ({:.1}s)",
                    self.provider_family.as_deref().unwrap_or("provider"),
                    self.elapsed_ms as f64 / 1000.0
                )
            }
            ProviderPhase::Completed => String::new(),
            ProviderPhase::TimedOut => {
                format!(
                    "{} timed out ({:.1}s)",
                    self.provider_family.as_deref().unwrap_or("provider"),
                    self.elapsed_ms as f64 / 1000.0
                )
            }
            ProviderPhase::Cancelled => "cancelled".to_string(),
        }
    }

    pub fn update_from_diagnostic(&mut self, diagnostic: &serde_json::Value) {
        let schema = diagnostic
            .get("schema")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let stage = diagnostic
            .get("stage")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        match (schema, stage) {
            ("api_request_stage.v1", "api_request_started")
            | ("provider_request.v1", "provider_request_started") => {
                self.phase = ProviderPhase::Started;
                self.elapsed_ms = 0;
                self.update_metadata(diagnostic, false);
                self.slow_warning_emitted = false;
                self.message = None;
            }
            ("provider_request.v1", "provider_request_retrying") => {
                self.phase = ProviderPhase::Retrying;
                self.update_metadata(diagnostic, true);
                self.update_elapsed(diagnostic);
                self.message = diagnostic
                    .get("message")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
                    .or_else(|| self.message.clone());
            }
            ("provider_request.v1", "provider_request_slow_warning") => {
                self.phase = ProviderPhase::SlowWarning;
                self.update_metadata(diagnostic, true);
                self.update_elapsed(diagnostic);
                self.slow_warning_emitted = true;
                self.message = diagnostic
                    .get("message")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
            }
            ("provider_request.v1", "provider_request_completed") => {
                self.phase = ProviderPhase::Completed;
                self.update_metadata(diagnostic, true);
                self.update_elapsed(diagnostic);
            }
            ("provider_request.v1", "provider_request_timeout") => {
                self.phase = ProviderPhase::TimedOut;
                self.update_metadata(diagnostic, true);
                self.update_elapsed(diagnostic);
                self.message = diagnostic
                    .get("message")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
                    .or_else(|| self.message.clone());
            }
            ("provider_request.v1", "provider_request_cancelled") => {
                self.phase = ProviderPhase::Cancelled;
                self.update_metadata(diagnostic, true);
                self.update_elapsed(diagnostic);
            }
            _ => {}
        }
    }

    pub fn check_slow_warning(&mut self) -> bool {
        if self.phase != ProviderPhase::Started || self.slow_warning_emitted {
            return false;
        }
        if self.elapsed_ms >= self.slow_warning_threshold_ms && self.slow_warning_threshold_ms > 0 {
            self.phase = ProviderPhase::SlowWarning;
            self.slow_warning_emitted = true;
            return true;
        }
        false
    }

    pub fn check_timeout(&mut self) -> bool {
        if !self.phase.is_active() || self.timeout_ms == 0 || self.elapsed_ms < self.timeout_ms {
            return false;
        }
        self.phase = ProviderPhase::TimedOut;
        self.message = Some(format!(
            "provider request timed out after {:.1}s",
            self.timeout_ms as f64 / 1000.0
        ));
        true
    }

    pub fn mark_cancelled(&mut self) {
        if self.phase.is_active() {
            self.phase = ProviderPhase::Cancelled;
        }
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    fn update_metadata(&mut self, diagnostic: &serde_json::Value, preserve_existing: bool) {
        self.provider_family = string_field(diagnostic, "provider_family").or_else(|| {
            preserve_existing
                .then(|| self.provider_family.clone())
                .flatten()
        });
        self.model = string_field(diagnostic, "model")
            .or_else(|| preserve_existing.then(|| self.model.clone()).flatten());
        self.request_shape = string_field(diagnostic, "request_shape").or_else(|| {
            preserve_existing
                .then(|| self.request_shape.clone())
                .flatten()
        });
        self.timeout_ms = diagnostic
            .get("timeout_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(if preserve_existing {
                self.timeout_ms
            } else {
                0
            });
        self.slow_warning_threshold_ms = diagnostic
            .get("slow_warning_threshold_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(if preserve_existing {
                self.slow_warning_threshold_ms
            } else {
                0
            });
        self.is_known_slow_path = diagnostic
            .get("is_known_slow_path")
            .and_then(|v| v.as_bool())
            .or_else(|| {
                diagnostic
                    .get("nonstreaming_tool_request")
                    .and_then(|v| v.as_bool())
            })
            .unwrap_or(if preserve_existing {
                self.is_known_slow_path
            } else {
                false
            });
    }

    fn update_elapsed(&mut self, diagnostic: &serde_json::Value) {
        self.elapsed_ms = diagnostic
            .get("elapsed_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(self.elapsed_ms);
    }
}

fn string_field(diagnostic: &serde_json::Value, key: &str) -> Option<String> {
    diagnostic
        .get(key)
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

/// End-to-end tool turn phase shared across frontends.
///
/// This is the product contract TUI should render. It deliberately describes
/// the lifecycle rather than a single widget state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolTurnPhase {
    #[default]
    Requested,
    Accepted,
    Executing,
    ResultObserved,
    SentBackToModel,
    FinalAnswer,
    Persisted,
    Failed,
    Cancelled,
    TimedOut,
}

impl ToolTurnPhase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Requested => "requested",
            Self::Accepted => "accepted",
            Self::Executing => "executing",
            Self::ResultObserved => "result observed",
            Self::SentBackToModel => "sent back to model",
            Self::FinalAnswer => "final answer",
            Self::Persisted => "persisted",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::TimedOut => "timed out",
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Persisted | Self::Failed | Self::Cancelled | Self::TimedOut
        )
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolTurnSnapshot {
    pub id: String,
    pub name: String,
    pub parent_message_id: Option<String>,
    pub phase: ToolTurnPhase,
    pub arguments_preview: Option<String>,
    pub result_preview: Option<String>,
    pub failure: Option<String>,
}

impl ToolTurnSnapshot {
    fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            parent_message_id: None,
            phase: ToolTurnPhase::Requested,
            arguments_preview: None,
            result_preview: None,
            failure: None,
        }
    }

    fn advance_to(&mut self, phase: ToolTurnPhase) {
        if !self.phase.is_terminal() {
            self.phase = phase;
        }
    }
}

/// Runtime facade state snapshot for frontends.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuntimeStateSnapshot {
    pub provider_request: ProviderRequestLifecycle,
    pub tool_turns: Vec<ToolTurnSnapshot>,
    pub is_querying: bool,
    pub assistant_streaming: bool,
    pub current_tool_label: Option<String>,
    pub stream_usage: Option<StreamUsageSnapshot>,
    pub turn_counter: u64,
    pub checkpoint_boundaries: Vec<CheckpointBoundary>,
}

/// Checkpoint boundary for precise turn-level rewind.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CheckpointBoundary {
    pub turn: u64,
    pub message_index: usize,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
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

/// Shared runtime facade state.
///
/// This is the single source of truth for runtime state. TUI and desktop
/// should read from this facade rather than maintaining their own state.
///
/// Both `state` and `started_at` are in a single Mutex to avoid nested
/// lock acquisition which could cause deadlocks.
#[derive(Clone)]
pub struct RuntimeFacadeState {
    inner: Arc<Mutex<FacadeInner>>,
}

#[derive(Debug, Clone, Default)]
struct FacadeInner {
    state: RuntimeStateSnapshot,
    started_at: Option<std::time::Instant>,
}

impl RuntimeFacadeState {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(FacadeInner::default())),
        }
    }

    pub async fn snapshot(&self) -> RuntimeStateSnapshot {
        let mut inner = self.inner.lock().await;
        // Update elapsed time from started_at if active
        if inner.state.provider_request.phase.is_active() {
            if let Some(started) = inner.started_at {
                inner.state.provider_request.elapsed_ms = started.elapsed().as_millis() as u64;
            }
            if inner.state.provider_request.check_timeout() {
                inner.started_at = None;
            }
        }
        inner.state.clone()
    }

    pub async fn process_diagnostic(&self, diagnostic: &serde_json::Value) {
        let schema = diagnostic
            .get("schema")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let stage = diagnostic
            .get("stage")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let is_start = matches!(
            (schema, stage),
            ("api_request_stage.v1", "api_request_started")
                | ("provider_request.v1", "provider_request_started")
        );

        let mut inner = self.inner.lock().await;

        if is_start {
            inner.started_at = Some(std::time::Instant::now());
        } else if let Some(started) = inner.started_at {
            inner.state.provider_request.elapsed_ms = started.elapsed().as_millis() as u64;
        }

        inner
            .state
            .provider_request
            .update_from_diagnostic(diagnostic);

        if !inner.state.provider_request.phase.is_active() {
            inner.started_at = None;
        }
    }

    pub async fn process_stream_event(&self, event: &crate::engine::streaming::StreamEvent) {
        self.process_stream_event_with_parent(event, None).await;
    }

    pub async fn process_stream_event_with_parent(
        &self,
        event: &crate::engine::streaming::StreamEvent,
        parent_message_id: Option<&str>,
    ) {
        let projection_event =
            SessionProjectionEvent::from_stream_event(event, parent_message_id, None);
        self.process_projection_event(&projection_event).await;
    }

    pub async fn process_projection_event(&self, event: &SessionProjectionEvent) {
        let mut inner = self.inner.lock().await;
        match event {
            SessionProjectionEvent::ToolCallStarted {
                message_id,
                tool_call_id,
                tool_name,
            } => {
                inner.state.assistant_streaming = false;
                upsert_tool_turn(
                    &mut inner.state.tool_turns,
                    tool_call_id,
                    tool_name,
                    message_id.as_deref(),
                )
                .advance_to(ToolTurnPhase::Requested);
            }
            SessionProjectionEvent::ToolArgumentsDelta {
                tool_call_id,
                arguments_delta,
            } => {
                let turn = upsert_tool_turn(&mut inner.state.tool_turns, tool_call_id, "", None);
                turn.arguments_preview = Some(append_preview(
                    turn.arguments_preview.take().unwrap_or_default(),
                    arguments_delta,
                    240,
                ));
            }
            SessionProjectionEvent::ToolCallAccepted { tool_call_id } => {
                upsert_tool_turn(&mut inner.state.tool_turns, tool_call_id, "", None)
                    .advance_to(ToolTurnPhase::Accepted);
            }
            SessionProjectionEvent::ToolExecutionStarted {
                tool_call_id,
                tool_name,
                ..
            } => {
                let turn =
                    upsert_tool_turn(&mut inner.state.tool_turns, tool_call_id, tool_name, None);
                if !tool_name.is_empty() {
                    turn.name = tool_name.clone();
                }
                turn.advance_to(ToolTurnPhase::Executing);
            }
            SessionProjectionEvent::ToolExecutionProgress { tool_call_id, .. } => {
                upsert_tool_turn(&mut inner.state.tool_turns, tool_call_id, "", None)
                    .advance_to(ToolTurnPhase::Executing);
            }
            SessionProjectionEvent::PermissionRequested {
                message_id,
                tool_call_id,
                tool_name,
                arguments,
                ..
            } => {
                let turn = upsert_tool_turn(
                    &mut inner.state.tool_turns,
                    tool_call_id,
                    tool_name,
                    message_id.as_deref(),
                );
                turn.name = tool_name.clone();
                turn.arguments_preview = Some(truncate_text(&arguments.to_string(), 240));
                turn.advance_to(ToolTurnPhase::Accepted);
            }
            SessionProjectionEvent::ToolExecutionCompleted {
                tool_call_id,
                result,
                metadata,
                ..
            } => {
                let turn = upsert_tool_turn(&mut inner.state.tool_turns, tool_call_id, "", None);
                turn.result_preview = Some(truncate_text(result, 300));
                let terminal = tool_terminal_phase(result, metadata.as_ref());
                turn.advance_to(terminal.unwrap_or(ToolTurnPhase::ResultObserved));
                if matches!(
                    turn.phase,
                    ToolTurnPhase::Failed | ToolTurnPhase::Cancelled | ToolTurnPhase::TimedOut
                ) {
                    turn.failure = turn.result_preview.clone();
                }
            }
            SessionProjectionEvent::ToolPartUpdated {
                message_id,
                tool_call_id,
                tool_name,
                status,
                result,
                ..
            } => {
                let turn = upsert_tool_turn(
                    &mut inner.state.tool_turns,
                    tool_call_id,
                    tool_name,
                    message_id.as_deref(),
                );
                if let Some(result) = result {
                    turn.result_preview = Some(truncate_text(result, 300));
                }
                let phase = match status.as_deref() {
                    Some("failed") => ToolTurnPhase::Failed,
                    Some("timed_out") => ToolTurnPhase::TimedOut,
                    Some("cancelled") => ToolTurnPhase::Cancelled,
                    Some("completed") => ToolTurnPhase::Persisted,
                    Some("running") => ToolTurnPhase::Executing,
                    _ if result.is_some() => ToolTurnPhase::Persisted,
                    _ => ToolTurnPhase::Requested,
                };
                turn.advance_to(phase);
                if matches!(
                    turn.phase,
                    ToolTurnPhase::Failed | ToolTurnPhase::Cancelled | ToolTurnPhase::TimedOut
                ) {
                    turn.failure = turn.result_preview.clone();
                }
            }
            SessionProjectionEvent::ToolResultsReadyForModel { tool_call_ids } => {
                if !mark_tool_turns_by_id(
                    &mut inner.state.tool_turns,
                    tool_call_ids,
                    ToolTurnPhase::SentBackToModel,
                ) {
                    mark_result_observed_turns(
                        &mut inner.state.tool_turns,
                        ToolTurnPhase::SentBackToModel,
                    );
                }
            }
            SessionProjectionEvent::RuntimeDiagnostic { diagnostic } => {
                let schema = diagnostic
                    .get("schema")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default();
                let stage = diagnostic
                    .get("stage")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default();
                if matches!(
                    (schema, stage),
                    ("api_request_stage.v1", "api_request_started")
                        | ("provider_request.v1", "provider_request_started")
                ) {
                    inner.state.assistant_streaming = false;
                    mark_result_observed_turns(
                        &mut inner.state.tool_turns,
                        ToolTurnPhase::SentBackToModel,
                    );
                }
            }
            SessionProjectionEvent::AssistantTextDelta { text, .. } if !text.trim().is_empty() => {
                inner.state.assistant_streaming = true;
                mark_result_observed_turns(&mut inner.state.tool_turns, ToolTurnPhase::FinalAnswer);
            }
            SessionProjectionEvent::AssistantTextUpdated {
                text, streaming, ..
            } if !text.trim().is_empty() => {
                inner.state.assistant_streaming = *streaming;
                mark_result_observed_turns(&mut inner.state.tool_turns, ToolTurnPhase::FinalAnswer);
            }
            SessionProjectionEvent::Completed => {
                inner.state.assistant_streaming = false;
                mark_result_observed_turns(&mut inner.state.tool_turns, ToolTurnPhase::FinalAnswer);
            }
            SessionProjectionEvent::Error { message } => {
                inner.state.assistant_streaming = false;
                for turn in inner
                    .state
                    .tool_turns
                    .iter_mut()
                    .filter(|turn| !turn.phase.is_terminal())
                {
                    turn.phase = ToolTurnPhase::Failed;
                    turn.failure = Some(truncate_text(message, 300));
                }
            }
            _ => {}
        }
    }

    pub async fn mark_cancelled(&self) {
        let mut inner = self.inner.lock().await;
        if inner.state.provider_request.phase.is_active() {
            if let Some(started) = inner.started_at {
                inner.state.provider_request.elapsed_ms = started.elapsed().as_millis() as u64;
            }
            inner.state.provider_request.mark_cancelled();
            inner.started_at = None;
        }
    }

    pub async fn check_slow_warning(&self) -> bool {
        let mut inner = self.inner.lock().await;
        if inner.state.provider_request.phase != ProviderPhase::Started
            || inner.state.provider_request.slow_warning_emitted
        {
            return false;
        }
        if let Some(started) = inner.started_at {
            inner.state.provider_request.elapsed_ms = started.elapsed().as_millis() as u64;
        }
        inner.state.provider_request.check_slow_warning()
    }

    pub async fn check_timeout(&self) -> bool {
        let mut inner = self.inner.lock().await;
        if inner.state.provider_request.phase.is_active() {
            if let Some(started) = inner.started_at {
                inner.state.provider_request.elapsed_ms = started.elapsed().as_millis() as u64;
            }
        }
        let timed_out = inner.state.provider_request.check_timeout();
        if timed_out {
            inner.started_at = None;
        }
        timed_out
    }

    pub async fn set_querying(&self, querying: bool) {
        let mut inner = self.inner.lock().await;
        inner.state.is_querying = querying;
    }

    pub async fn mark_tool_turns_persisted(&self) {
        let mut inner = self.inner.lock().await;
        mark_result_observed_turns(&mut inner.state.tool_turns, ToolTurnPhase::Persisted);
    }

    pub async fn mark_active_tool_turns_timed_out(&self, reason: &str) {
        let mut inner = self.inner.lock().await;
        for turn in inner.state.tool_turns.iter_mut().filter(|turn| {
            matches!(
                turn.phase,
                ToolTurnPhase::Requested | ToolTurnPhase::Accepted | ToolTurnPhase::Executing
            )
        }) {
            turn.phase = ToolTurnPhase::TimedOut;
            turn.failure = Some(truncate_text(reason, 300));
        }
    }

    pub async fn set_tool_label(&self, label: Option<String>) {
        let mut inner = self.inner.lock().await;
        inner.state.current_tool_label = label;
    }

    pub async fn set_stream_usage(&self, usage: Option<StreamUsageSnapshot>) {
        let mut inner = self.inner.lock().await;
        inner.state.stream_usage = usage;
    }

    pub async fn increment_turn(&self, message_index: usize) -> u64 {
        let mut inner = self.inner.lock().await;
        inner.state.turn_counter += 1;
        let turn = inner.state.turn_counter;
        inner.state.checkpoint_boundaries.push(CheckpointBoundary {
            turn,
            message_index,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        });
        // Bound checkpoint boundaries to prevent unbounded growth
        if inner.state.checkpoint_boundaries.len() > 1000 {
            inner.state.checkpoint_boundaries.drain(0..500);
        }
        turn
    }

    pub async fn turn_counter(&self) -> u64 {
        let inner = self.inner.lock().await;
        inner.state.turn_counter
    }

    pub async fn checkpoint_boundaries(&self) -> Vec<CheckpointBoundary> {
        let inner = self.inner.lock().await;
        inner.state.checkpoint_boundaries.clone()
    }

    pub async fn message_index_for_turn(&self, turn: u64) -> Option<usize> {
        let inner = self.inner.lock().await;
        inner
            .state
            .checkpoint_boundaries
            .iter()
            .find(|b| b.turn == turn)
            .map(|b| b.message_index)
    }

    pub async fn reset(&self) {
        let mut inner = self.inner.lock().await;
        *inner = FacadeInner::default();
    }
}

impl Default for RuntimeFacadeState {
    fn default() -> Self {
        Self::new()
    }
}

fn upsert_tool_turn<'a>(
    turns: &'a mut Vec<ToolTurnSnapshot>,
    id: &str,
    name: &str,
    parent_message_id: Option<&str>,
) -> &'a mut ToolTurnSnapshot {
    if let Some(index) = turns.iter().position(|turn| turn.id == id) {
        let turn = &mut turns[index];
        if !name.is_empty() && turn.name.is_empty() {
            turn.name = name.to_string();
        }
        if turn.parent_message_id.is_none() {
            turn.parent_message_id = parent_message_id.map(str::to_string);
        }
        return turn;
    }
    let mut turn = ToolTurnSnapshot::new(id, name);
    turn.parent_message_id = parent_message_id.map(str::to_string);
    turns.push(turn);
    turns.last_mut().expect("tool turn was just inserted")
}

fn mark_result_observed_turns(turns: &mut [ToolTurnSnapshot], phase: ToolTurnPhase) {
    for turn in turns.iter_mut().filter(|turn| {
        matches!(
            turn.phase,
            ToolTurnPhase::ResultObserved
                | ToolTurnPhase::SentBackToModel
                | ToolTurnPhase::FinalAnswer
        )
    }) {
        turn.advance_to(phase);
    }
}

fn mark_tool_turns_by_id(
    turns: &mut [ToolTurnSnapshot],
    ids: &[String],
    phase: ToolTurnPhase,
) -> bool {
    let mut matched = false;
    for turn in turns
        .iter_mut()
        .filter(|turn| ids.iter().any(|id| id == &turn.id))
    {
        turn.advance_to(phase);
        matched = true;
    }
    matched
}

fn append_preview(mut current: String, delta: &str, max_chars: usize) -> String {
    current.push_str(delta);
    truncate_text(&current, max_chars)
}

fn truncate_text(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    let mut text = value
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect::<String>();
    text.push_str("...");
    text
}

fn tool_terminal_phase(
    result: &str,
    metadata: Option<&serde_json::Value>,
) -> Option<ToolTurnPhase> {
    let metadata_status = metadata
        .and_then(|value| value.get("status"))
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let body = result.to_ascii_lowercase();
    if metadata_status.contains("timed_out")
        || metadata_status.contains("timeout")
        || body.contains("command timed out after")
        || body.contains(" is timed_out.")
        || body.contains(" is timed out.")
    {
        return Some(ToolTurnPhase::TimedOut);
    }
    if metadata_status.contains("cancelled")
        || metadata_status.contains("canceled")
        || body.contains(" is cancelled.")
        || body.contains(" is canceled.")
    {
        return Some(ToolTurnPhase::Cancelled);
    }
    if metadata_status.contains("failed")
        || metadata_status.contains("error")
        || result.contains("Result: ERROR")
        || result.contains("[Error:")
    {
        return Some(ToolTurnPhase::Failed);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn provider_lifecycle_tracks_full_diagnostic_sequence() {
        let mut lifecycle = ProviderRequestLifecycle::default();

        lifecycle.update_from_diagnostic(&json!({
            "schema": "api_request_stage.v1",
            "stage": "api_request_started",
            "provider_family": "openai",
            "model": "gpt-test",
            "request_shape": "streaming_tool_request",
            "timeout_ms": 120_000,
            "slow_warning_threshold_ms": 45_000,
            "is_known_slow_path": true
        }));

        assert_eq!(lifecycle.phase, ProviderPhase::Started);
        assert_eq!(lifecycle.provider_family.as_deref(), Some("openai"));
        assert_eq!(lifecycle.model.as_deref(), Some("gpt-test"));
        assert_eq!(
            lifecycle.request_shape.as_deref(),
            Some("streaming_tool_request")
        );
        assert_eq!(lifecycle.timeout_ms, 120_000);
        assert_eq!(lifecycle.slow_warning_threshold_ms, 45_000);
        assert!(lifecycle.is_known_slow_path);

        lifecycle.update_from_diagnostic(&json!({
            "schema": "provider_request.v1",
            "stage": "provider_request_retrying",
            "elapsed_ms": 7_500
        }));
        assert_eq!(lifecycle.phase, ProviderPhase::Retrying);
        assert_eq!(lifecycle.elapsed_ms, 7_500);
        assert_eq!(lifecycle.provider_family.as_deref(), Some("openai"));

        lifecycle.update_from_diagnostic(&json!({
            "schema": "provider_request.v1",
            "stage": "provider_request_slow_warning",
            "elapsed_ms": 46_000,
            "message": "provider is slow"
        }));
        assert_eq!(lifecycle.phase, ProviderPhase::SlowWarning);
        assert_eq!(lifecycle.elapsed_ms, 46_000);
        assert!(lifecycle.slow_warning_emitted);
        assert_eq!(lifecycle.message.as_deref(), Some("provider is slow"));

        lifecycle.update_from_diagnostic(&json!({
            "schema": "provider_request.v1",
            "stage": "provider_request_completed",
            "elapsed_ms": 51_000
        }));
        assert_eq!(lifecycle.phase, ProviderPhase::Completed);
        assert_eq!(lifecycle.elapsed_ms, 51_000);
        assert!(!lifecycle.phase.is_active());
    }

    #[test]
    fn provider_lifecycle_tracks_timeout_and_cancelled_terminal_states() {
        let mut lifecycle = ProviderRequestLifecycle::default();
        lifecycle.update_from_diagnostic(&json!({
            "schema": "api_request_stage.v1",
            "stage": "api_request_started",
            "provider_family": "minimax",
            "nonstreaming_tool_request": true,
            "timeout_ms": 90_000
        }));
        lifecycle.update_from_diagnostic(&json!({
            "schema": "provider_request.v1",
            "stage": "provider_request_timeout",
            "elapsed_ms": 90_001,
            "message": "timeout"
        }));

        assert_eq!(lifecycle.phase, ProviderPhase::TimedOut);
        assert_eq!(lifecycle.elapsed_ms, 90_001);
        assert_eq!(lifecycle.timeout_ms, 90_000);
        assert!(lifecycle.is_known_slow_path);
        assert_eq!(lifecycle.message.as_deref(), Some("timeout"));
        assert!(!lifecycle.phase.is_active());

        lifecycle.update_from_diagnostic(&json!({
            "schema": "api_request_stage.v1",
            "stage": "api_request_started",
            "provider_family": "openai"
        }));
        lifecycle.update_from_diagnostic(&json!({
            "schema": "provider_request.v1",
            "stage": "provider_request_cancelled",
            "elapsed_ms": 125
        }));

        assert_eq!(lifecycle.phase, ProviderPhase::Cancelled);
        assert_eq!(lifecycle.elapsed_ms, 125);
        assert!(!lifecycle.phase.is_active());
    }

    #[test]
    fn provider_lifecycle_marks_declared_timeout_without_provider_event() {
        let mut lifecycle = ProviderRequestLifecycle::default();
        lifecycle.update_from_diagnostic(&json!({
            "schema": "api_request_stage.v1",
            "stage": "api_request_started",
            "provider_family": "deepseek",
            "model": "deepseek-v4-flash",
            "nonstreaming_tool_request": true,
            "timeout_ms": 1_200
        }));
        lifecycle.elapsed_ms = 1_201;

        assert!(lifecycle.check_timeout());
        assert_eq!(lifecycle.phase, ProviderPhase::TimedOut);
        assert_eq!(
            lifecycle.message.as_deref(),
            Some("provider request timed out after 1.2s")
        );
        assert!(!lifecycle.phase.is_active());
        assert!(!lifecycle.check_timeout());
    }

    #[tokio::test]
    async fn facade_snapshot_marks_stale_provider_timeout() {
        let facade = RuntimeFacadeState::new();
        facade
            .process_diagnostic(&json!({
                "schema": "api_request_stage.v1",
                "stage": "api_request_started",
                "provider_family": "deepseek",
                "model": "deepseek-v4-flash",
                "nonstreaming_tool_request": true,
                "timeout_ms": 1
            }))
            .await;
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;

        let snapshot = facade.snapshot().await;

        assert_eq!(snapshot.provider_request.phase, ProviderPhase::TimedOut);
        assert!(snapshot.provider_request.elapsed_ms >= 1);
        assert!(snapshot
            .provider_request
            .message
            .as_deref()
            .is_some_and(|message| message.contains("provider request timed out")));
    }

    #[tokio::test]
    async fn tool_turn_spine_tracks_successful_tool_round() {
        let facade = RuntimeFacadeState::new();
        facade
            .process_stream_event(&crate::engine::streaming::StreamEvent::ToolCallStart {
                id: "call_1".to_string(),
                name: "bash".to_string(),
            })
            .await;
        facade
            .process_stream_event(&crate::engine::streaming::StreamEvent::ToolCallArgs {
                id: "call_1".to_string(),
                args_delta: "{\"command\":\"pwd\"}".to_string(),
            })
            .await;
        facade
            .process_stream_event(&crate::engine::streaming::StreamEvent::ToolCallComplete {
                id: "call_1".to_string(),
            })
            .await;
        facade
            .process_stream_event(&crate::engine::streaming::StreamEvent::ToolExecutionStart {
                id: "call_1".to_string(),
                name: "bash".to_string(),
                metadata: None,
            })
            .await;
        facade
            .process_stream_event(
                &crate::engine::streaming::StreamEvent::ToolExecutionComplete {
                    id: "call_1".to_string(),
                    result: "Result: OK\n/Users/georgexu/Desktop/rust-agent".to_string(),
                    metadata: None,
                    result_data: None,
                },
            )
            .await;

        let snapshot = facade.snapshot().await;
        let turn = snapshot.tool_turns.first().expect("tool turn");
        assert_eq!(turn.id, "call_1");
        assert_eq!(turn.name, "bash");
        assert_eq!(turn.phase, ToolTurnPhase::ResultObserved);
        assert!(turn
            .arguments_preview
            .as_deref()
            .is_some_and(|args| args.contains("pwd")));
        assert!(turn
            .result_preview
            .as_deref()
            .is_some_and(|result| result.contains("Result: OK")));

        facade
            .process_stream_event(&crate::engine::streaming::StreamEvent::RuntimeDiagnostic {
                diagnostic: json!({
                    "schema": "api_request_stage.v1",
                    "stage": "api_request_started"
                }),
            })
            .await;
        assert_eq!(
            facade.snapshot().await.tool_turns[0].phase,
            ToolTurnPhase::SentBackToModel
        );

        facade
            .process_stream_event(&crate::engine::streaming::StreamEvent::TextChunk(
                "pwd returned the project path.".to_string(),
            ))
            .await;
        let snapshot = facade.snapshot().await;
        assert!(snapshot.assistant_streaming);
        assert_eq!(snapshot.tool_turns[0].phase, ToolTurnPhase::FinalAnswer);

        facade
            .process_stream_event(&crate::engine::streaming::StreamEvent::Complete)
            .await;
        let snapshot = facade.snapshot().await;
        assert!(!snapshot.assistant_streaming);
        assert_eq!(snapshot.tool_turns[0].phase, ToolTurnPhase::FinalAnswer);

        facade.mark_tool_turns_persisted().await;
        assert_eq!(
            facade.snapshot().await.tool_turns[0].phase,
            ToolTurnPhase::Persisted
        );
    }

    #[tokio::test]
    async fn tool_turn_spine_records_failed_terminal_result() {
        let facade = RuntimeFacadeState::new();
        facade
            .process_stream_event(&crate::engine::streaming::StreamEvent::ToolCallStart {
                id: "call_1".to_string(),
                name: "bash".to_string(),
            })
            .await;
        facade
            .process_stream_event(
                &crate::engine::streaming::StreamEvent::ToolExecutionComplete {
                    id: "call_1".to_string(),
                    result: "Result: ERROR\ncommand failed".to_string(),
                    metadata: None,
                    result_data: None,
                },
            )
            .await;

        let snapshot = facade.snapshot().await;
        let turn = snapshot.tool_turns.first().expect("tool turn");
        assert_eq!(turn.phase, ToolTurnPhase::Failed);
        assert!(turn
            .failure
            .as_deref()
            .is_some_and(|failure| failure.contains("command failed")));
    }

    #[tokio::test]
    async fn tool_turn_spine_records_parent_message_anchor() {
        let facade = RuntimeFacadeState::new();
        facade
            .process_stream_event_with_parent(
                &crate::engine::streaming::StreamEvent::ToolCallStart {
                    id: "call_1".to_string(),
                    name: "bash".to_string(),
                },
                Some("user_1"),
            )
            .await;
        facade
            .process_stream_event_with_parent(
                &crate::engine::streaming::StreamEvent::ToolExecutionStart {
                    id: "call_1".to_string(),
                    name: "bash".to_string(),
                    metadata: None,
                },
                Some("user_1"),
            )
            .await;

        let snapshot = facade.snapshot().await;
        let turn = snapshot.tool_turns.first().expect("tool turn");
        assert_eq!(turn.parent_message_id.as_deref(), Some("user_1"));
        assert_eq!(turn.phase, ToolTurnPhase::Executing);
    }

    #[tokio::test]
    async fn tool_turn_spine_consumes_projection_events_directly() {
        let facade = RuntimeFacadeState::new();
        facade
            .process_projection_event(&SessionProjectionEvent::ToolCallStarted {
                message_id: Some("user_1".to_string()),
                tool_call_id: "call_1".to_string(),
                tool_name: "bash".to_string(),
            })
            .await;
        facade
            .process_projection_event(&SessionProjectionEvent::ToolPartUpdated {
                message_id: Some("user_1".to_string()),
                tool_call_id: "call_1".to_string(),
                tool_name: "bash".to_string(),
                status: Some("completed".to_string()),
                input_args: Some("{\"command\":\"pwd\"}".to_string()),
                result: Some("Result: OK\n/tmp/project".to_string()),
                metadata: None,
                result_data: None,
            })
            .await;
        facade
            .process_projection_event(&SessionProjectionEvent::AssistantTextUpdated {
                message_id: Some("assistant_1".to_string()),
                text: "done".to_string(),
                streaming: false,
            })
            .await;

        let snapshot = facade.snapshot().await;
        let turn = snapshot.tool_turns.first().expect("tool turn");
        assert_eq!(turn.parent_message_id.as_deref(), Some("user_1"));
        assert_eq!(turn.phase, ToolTurnPhase::Persisted);
        assert_eq!(
            turn.result_preview.as_deref(),
            Some("Result: OK\n/tmp/project")
        );
    }

    #[tokio::test]
    async fn tool_turn_spine_marks_results_ready_for_model_without_provider_diagnostic() {
        let facade = RuntimeFacadeState::new();
        facade
            .process_stream_event(&crate::engine::streaming::StreamEvent::ToolCallStart {
                id: "call_1".to_string(),
                name: "bash".to_string(),
            })
            .await;
        facade
            .process_stream_event(
                &crate::engine::streaming::StreamEvent::ToolExecutionComplete {
                    id: "call_1".to_string(),
                    result: "Result: OK\n/Users/georgexu/Desktop/rust-agent".to_string(),
                    metadata: None,
                    result_data: None,
                },
            )
            .await;
        assert_eq!(
            facade.snapshot().await.tool_turns[0].phase,
            ToolTurnPhase::ResultObserved
        );

        facade
            .process_stream_event(
                &crate::engine::streaming::StreamEvent::ToolResultsReadyForModel {
                    ids: vec!["call_1".to_string()],
                },
            )
            .await;

        assert_eq!(
            facade.snapshot().await.tool_turns[0].phase,
            ToolTurnPhase::SentBackToModel
        );
    }

    #[tokio::test]
    async fn tool_turn_spine_preserves_sent_back_tool_when_provider_times_out() {
        let facade = RuntimeFacadeState::new();
        facade
            .process_stream_event(&crate::engine::streaming::StreamEvent::ToolCallStart {
                id: "call_1".to_string(),
                name: "bash".to_string(),
            })
            .await;
        facade
            .process_stream_event(
                &crate::engine::streaming::StreamEvent::ToolExecutionComplete {
                    id: "call_1".to_string(),
                    result: "Result: OK\npreview".to_string(),
                    metadata: None,
                    result_data: None,
                },
            )
            .await;
        facade
            .process_stream_event(
                &crate::engine::streaming::StreamEvent::ToolResultsReadyForModel {
                    ids: vec!["call_1".to_string()],
                },
            )
            .await;

        facade
            .mark_active_tool_turns_timed_out("provider request timed out after 1.0s")
            .await;

        let snapshot = facade.snapshot().await;
        assert_eq!(snapshot.tool_turns[0].phase, ToolTurnPhase::SentBackToModel);
        assert_eq!(snapshot.tool_turns[0].failure, None);
    }

    #[tokio::test]
    async fn tool_turn_spine_marks_executing_tool_timed_out() {
        let facade = RuntimeFacadeState::new();
        facade
            .process_stream_event(&crate::engine::streaming::StreamEvent::ToolExecutionStart {
                id: "call_1".to_string(),
                name: "bash".to_string(),
                metadata: None,
            })
            .await;

        facade
            .mark_active_tool_turns_timed_out("provider request timed out after 1.0s")
            .await;

        let snapshot = facade.snapshot().await;
        assert_eq!(snapshot.tool_turns[0].phase, ToolTurnPhase::TimedOut);
        assert!(snapshot.tool_turns[0]
            .failure
            .as_deref()
            .is_some_and(|failure| failure.contains("provider request timed out")));
    }
}
