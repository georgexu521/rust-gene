//! Frontend-neutral runtime controller.
//!
//! Reasonix alignment target (Phase 1): make one canonical command/event
//! boundary the way TUI, desktop, CLI, and headless frontends drive agent work.
//!
//! This module wraps `StreamingQueryEngine` and exposes a stable command/event
//! API. The first win is the boundary; the internal loop stays in
//! `conversation_loop/`.
//!
//! ## Commands
//!
//! - `submit_turn(message)` → stream of `TurnEvent`
//! - `cancel()` → cancel in-flight turn
//! - `approve_pending(approved)` → answer a pending permission request
//! - `compact()` → manual context compaction
//! - `set_session(id)` / `current_session()` → session management
//! - `context_snapshot()` → frontend-facing context stats
//!
//! ## Events
//!
//! `TurnEvent` mirrors `StreamEvent` but adds an explicit `Closeout` variant
//! so frontends can render verified/partial/not-verified status without
//! re-deriving meaning from tool results.

use crate::engine::agent_mode::AgentMode;
use crate::engine::runtime_facade::RuntimeFacadeState;
use crate::engine::streaming::{StreamEvent, StreamingQueryEngine};
use futures::Stream;
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

/// The product command/event controller shared across frontends.
///
/// Frontends should use this instead of calling `StreamingQueryEngine` directly
/// for full-agent turns. Lightweight (non-agent) turns are explicitly NOT
/// routed through this controller; they use a separate direct-provider lane.
#[derive(Clone)]
pub struct RuntimeController {
    engine: Arc<StreamingQueryEngine>,
    runtime_state: Arc<RuntimeFacadeState>,
}

impl RuntimeController {
    pub fn new(engine: Arc<StreamingQueryEngine>) -> Self {
        let runtime_state = Arc::new(RuntimeFacadeState::default());
        Self {
            engine,
            runtime_state,
        }
    }

    pub fn with_runtime_state(
        engine: Arc<StreamingQueryEngine>,
        runtime_state: Arc<RuntimeFacadeState>,
    ) -> Self {
        Self {
            engine,
            runtime_state,
        }
    }

    pub fn engine(&self) -> &Arc<StreamingQueryEngine> {
        &self.engine
    }

    pub fn runtime_state(&self) -> &Arc<RuntimeFacadeState> {
        &self.runtime_state
    }

    // ---- Turn lifecycle ----

    /// Submit a full agent turn. Returns a stream of `TurnEvent` that
    /// frontends render without re-deriving runtime meaning.
    pub async fn submit_turn(
        &self,
        user_message: impl Into<String>,
    ) -> Pin<Box<dyn Stream<Item = TurnEvent> + Send>> {
        let stream = self.submit_stream_turn(user_message).await;
        Box::pin(StreamEventToTurnEvent { inner: stream })
    }

    /// Submit a full agent turn while preserving the legacy `StreamEvent`
    /// stream. Frontends that have not migrated to `TurnEvent` still use this
    /// method so the full-agent command enters through `RuntimeController`.
    pub async fn submit_stream_turn(
        &self,
        user_message: impl Into<String>,
    ) -> Pin<Box<dyn Stream<Item = StreamEvent> + Send>> {
        let stream = self.engine.query_stream(user_message).await;
        self.mirror_stream(stream)
    }

    /// Submit a full agent turn with an explicit agent mode, preserving the
    /// legacy `StreamEvent` stream for TUI compatibility.
    pub async fn submit_stream_turn_with_agent_mode(
        &self,
        user_message: impl Into<String>,
        agent_mode: AgentMode,
    ) -> Pin<Box<dyn Stream<Item = StreamEvent> + Send>> {
        let stream = self
            .engine
            .query_stream_with_agent_mode(user_message, agent_mode)
            .await;
        self.mirror_stream(stream)
    }

    fn mirror_stream(
        &self,
        stream: Pin<Box<dyn Stream<Item = StreamEvent> + Send>>,
    ) -> Pin<Box<dyn Stream<Item = StreamEvent> + Send>> {
        let mirror = self.engine.session_binding().and_then(|(store, sid)| {
            crate::session_store::event_mirror::StreamEventMirror::shared(&store, &sid)
        });
        match mirror {
            Some(mirror) => Box::pin(MirroredStream {
                inner: stream,
                mirror,
            }),
            None => stream,
        }
    }

    /// Cancel the in-flight turn (if any).
    ///
    /// The runtime marks the provider phase as cancelled. The concrete
    /// tokio handle abort is the frontend's responsibility because the
    /// controller does not own the spawned task handle.
    pub async fn cancel(&self) {
        self.runtime_state.mark_cancelled().await;
    }

    /// Answer a pending permission request.
    ///
    /// Returns true if there was a pending request and the answer was sent.
    pub async fn approve_pending(&self, approved: bool) -> bool {
        let Some(channel) = self.engine.approval_channel() else {
            return false;
        };
        for _ in 0..20 {
            if let Some((_request, tx)) = channel.take_pending().await {
                let response = if approved {
                    crate::engine::conversation_loop::ToolApprovalResponse::approved_once()
                } else {
                    crate::engine::conversation_loop::ToolApprovalResponse::rejected_once()
                };
                let _ = tx.send(response);
                return true;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        false
    }

    /// Manually compact context. Returns the compaction attempt record.
    pub async fn compact(
        &self,
    ) -> Option<crate::engine::context_compressor::CompactionAttemptRecord> {
        self.engine.compact_context_manually().await
    }

    // ---- Session ----

    pub fn set_session(&self, session_id: impl Into<String>) {
        self.engine.set_session_id(session_id);
    }

    pub fn current_session(&self) -> Option<String> {
        self.engine.current_session_id()
    }

    // ---- Memory policy ----

    /// Set memory usage policy for this session.
    ///
    /// This replaces the previous pattern where TUI app.rs mutated memory
    /// controls directly on the engine before each turn.
    pub fn set_memory_policy(
        &self,
        use_memory: bool,
        generate_memory: bool,
        recall_mode: impl Into<String>,
    ) {
        self.engine.set_memory_use(use_memory);
        self.engine.set_memory_generate(generate_memory);
        self.engine.set_memory_recall_mode(recall_mode);
    }

    // ---- Diagnostics ----

    /// Context snapshot for frontend rendering.
    pub async fn context_snapshot(&self) -> crate::desktop_runtime::DesktopContextSnapshot {
        // Reuse DesktopContextSnapshot for now; Phase 6 can extract a shared type.
        let usage = self.engine.context_usage_report().await;
        let (stats, circuit_open, latest_record, latest_attempt) = {
            let compressor = self.engine.compressor().expect("compressor");
            let compressor = compressor.lock().await;
            (
                compressor.stats(),
                compressor.compaction_circuit_open(),
                compressor.latest_compaction_record().cloned(),
                compressor.compaction_attempt_records().last().cloned(),
            )
        };
        let restored_boundary = if latest_record.is_none() {
            self.engine
                .session_binding()
                .and_then(|(store, session_id)| {
                    store.latest_compact_boundary(&session_id).ok().flatten()
                })
        } else {
            None
        };
        let usage_percent = if usage.max_context_tokens > 0 {
            usage
                .total_estimated_tokens
                .saturating_mul(100)
                .saturating_div(usage.max_context_tokens)
        } else {
            0
        };
        let (
            prompt_cache_cached_tokens,
            prompt_cache_miss_tokens,
            prompt_cache_hit_rate_percent,
            prompt_cache_diagnostic_count,
            prompt_cache_last_reason,
        ) = {
            let tracker = self.engine.cost_tracker().lock().await;
            let ledger_summary = self.current_session().and_then(|session_id| {
                crate::cost_tracker::usage_ledger::summarize_usage_ledger(Some(&session_id)).ok()
            });
            if let Some(summary) = ledger_summary.filter(|summary| summary.entries > 0) {
                (
                    summary.cache_hit_tokens,
                    summary.cache_miss_tokens,
                    summary.hit_rate * 100.0,
                    summary.entries as usize,
                    summary.last_miss_reason,
                )
            } else {
                let prompt_tokens = tracker.total_tokens.prompt;
                let cached_tokens = tracker.total_tokens.cached;
                let hit_rate = if prompt_tokens == 0 {
                    0.0
                } else {
                    cached_tokens.min(prompt_tokens) as f64 / prompt_tokens as f64 * 100.0
                };
                (
                    cached_tokens,
                    tracker.total_tokens.cache_miss,
                    hit_rate,
                    tracker.prompt_cache_diagnostics.len(),
                    tracker
                        .prompt_cache_diagnostics
                        .last()
                        .map(|entry| entry.miss_reason.clone()),
                )
            }
        };

        crate::desktop_runtime::DesktopContextSnapshot {
            history_messages: usage.history_messages,
            history_tokens: usage.history_tokens,
            tool_schema_tokens: usage.tool_schema_tokens,
            memory_snapshot_tokens: usage.memory_snapshot_tokens,
            total_estimated_tokens: usage.total_estimated_tokens,
            max_context_tokens: usage.max_context_tokens,
            usage_percent,
            stable_prefix_fingerprint: usage.stable_prefix_fingerprint,
            prompt_cache_cached_tokens,
            prompt_cache_miss_tokens,
            prompt_cache_hit_rate_percent,
            prompt_cache_diagnostic_count,
            prompt_cache_last_reason,
            compact: crate::desktop_runtime::DesktopCompactState {
                compression_count: stats.compression_count,
                circuit_open,
                latest_strategy: latest_record
                    .as_ref()
                    .map(|record| record.strategy.label().to_string())
                    .or_else(|| {
                        restored_boundary
                            .as_ref()
                            .map(|boundary| boundary.strategy.clone())
                    }),
                latest_boundary_id: latest_record.and_then(|record| record.boundary_id).or_else(
                    || {
                        restored_boundary
                            .as_ref()
                            .map(|boundary| boundary.boundary_id.clone())
                    },
                ),
                latest_attempt_decision: latest_attempt
                    .as_ref()
                    .map(|attempt| attempt.decision.label().to_string()),
                latest_attempt_reason: latest_attempt
                    .as_ref()
                    .map(|attempt| attempt.reason.clone()),
                latest_attempt_trigger: latest_attempt
                    .as_ref()
                    .map(|attempt| attempt.trigger.clone()),
                latest_attempt_tokens_before: latest_attempt
                    .as_ref()
                    .map(|attempt| attempt.before_tokens),
                latest_attempt_tokens_after: latest_attempt
                    .and_then(|attempt| attempt.after_tokens),
            },
        }
    }

    pub fn model_name(&self) -> String {
        self.engine.model_name()
    }

    pub fn provider_base_url(&self) -> String {
        self.engine.provider_base_url().to_string()
    }

    pub fn permission_mode(&self) -> crate::permissions::PermissionMode {
        self.engine.permission_mode()
    }
}

// ---- Events ----

/// Product turn events that frontends render.
///
/// Mirrors `StreamEvent` but adds `Closeout` so frontends can distinguish
/// verified/partial/not-verified turn completion without inspecting tool
/// results or trace events.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TurnEvent {
    TurnStarted,
    ThinkingStarted,
    ThinkingDelta {
        text: String,
    },
    ThinkingCompleted,
    TextDelta {
        text: String,
    },
    ToolStarted {
        id: String,
        name: String,
    },
    ToolArgsDelta {
        id: String,
        delta: String,
    },
    ToolCallReady {
        id: String,
    },
    ToolExecutionProgress {
        id: String,
        progress: String,
    },
    ToolCompleted {
        id: String,
        result_preview: String,
        metadata: Option<serde_json::Value>,
    },
    ToolResultsReadyForModel {
        ids: Vec<String>,
    },
    PermissionRequested {
        id: String,
        tool_name: String,
        arguments: serde_json::Value,
        prompt: String,
        metadata: Option<serde_json::Value>,
    },
    Usage {
        prompt_tokens: u32,
        completion_tokens: u32,
        reasoning_tokens: Option<u32>,
        cached_tokens: Option<u32>,
    },
    RuntimeDiagnostic {
        diagnostic: serde_json::Value,
    },
    Closeout {
        status: CloseoutStatus,
        evidence_summary: Option<String>,
    },
    TurnCompleted,
    TurnError {
        message: String,
    },
    OutputTruncated,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CloseoutStatus {
    Verified,
    Partial,
    NotVerified,
    Failed,
}

impl From<StreamEvent> for TurnEvent {
    fn from(event: StreamEvent) -> Self {
        match event {
            StreamEvent::Start => TurnEvent::TurnStarted,
            StreamEvent::TextChunk(text) => TurnEvent::TextDelta { text },
            StreamEvent::ToolCallStart { id, name } => TurnEvent::ToolStarted { id, name },
            StreamEvent::ToolCallArgs { id, args_delta } => TurnEvent::ToolArgsDelta {
                id,
                delta: args_delta,
            },
            StreamEvent::ToolCallComplete { id } => TurnEvent::ToolCallReady { id },
            StreamEvent::ToolExecutionStart { id, name, .. } => TurnEvent::ToolStarted { id, name },
            StreamEvent::ToolExecutionProgress { id, progress } => {
                TurnEvent::ToolExecutionProgress { id, progress }
            }
            StreamEvent::ToolExecutionComplete {
                id,
                result,
                metadata,
                ..
            } => TurnEvent::ToolCompleted {
                id,
                result_preview: truncate_preview(&result, 2000),
                metadata,
            },
            StreamEvent::ToolResultsReadyForModel { ids } => {
                TurnEvent::ToolResultsReadyForModel { ids }
            }
            StreamEvent::ThinkingStart => TurnEvent::ThinkingStarted,
            StreamEvent::ThinkingChunk(text) => TurnEvent::ThinkingDelta { text },
            StreamEvent::ThinkingComplete => TurnEvent::ThinkingCompleted,
            StreamEvent::Usage {
                prompt_tokens,
                completion_tokens,
                reasoning_tokens,
                cached_tokens,
            } => TurnEvent::Usage {
                prompt_tokens,
                completion_tokens,
                reasoning_tokens,
                cached_tokens,
            },
            StreamEvent::RuntimeDiagnostic { diagnostic } => {
                TurnEvent::RuntimeDiagnostic { diagnostic }
            }
            StreamEvent::Closeout {
                status,
                evidence_summary,
            } => TurnEvent::Closeout {
                status: CloseoutStatus::from_runtime_label(&status),
                evidence_summary,
            },
            StreamEvent::Complete => TurnEvent::TurnCompleted,
            StreamEvent::OutputTruncated => TurnEvent::OutputTruncated,
            StreamEvent::Error(message) => TurnEvent::TurnError { message },
            StreamEvent::PermissionRequest {
                id,
                tool_name,
                arguments,
                prompt,
                metadata,
                ..
            } => TurnEvent::PermissionRequested {
                id,
                tool_name,
                arguments,
                prompt,
                metadata,
            },
        }
    }
}

impl CloseoutStatus {
    fn from_runtime_label(label: &str) -> Self {
        match label {
            "verified" | "complete" | "passed" => Self::Verified,
            "partial" => Self::Partial,
            "failed" | "error" => Self::Failed,
            _ => Self::NotVerified,
        }
    }
}

// ---- Stream adapter ----

struct StreamEventToTurnEvent {
    inner: Pin<Box<dyn Stream<Item = StreamEvent> + Send>>,
}

impl Stream for StreamEventToTurnEvent {
    type Item = TurnEvent;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        Pin::new(&mut self.inner)
            .poll_next(cx)
            .map(|opt| opt.map(TurnEvent::from))
    }
}

struct MirroredStream {
    inner: Pin<Box<dyn Stream<Item = StreamEvent> + Send>>,
    mirror: Arc<std::sync::Mutex<crate::session_store::event_mirror::StreamEventMirror>>,
}

impl Stream for MirroredStream {
    type Item = StreamEvent;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.inner.as_mut().poll_next(cx) {
            Poll::Ready(Some(event)) => {
                if let Ok(mut mirror) = self.mirror.lock() {
                    mirror.mirror(&event);
                }
                Poll::Ready(Some(event))
            }
            other => other,
        }
    }
}

fn truncate_preview(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut preview: String = text.chars().take(max_chars.saturating_sub(1)).collect();
    preview.push('…');
    preview
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::streaming::StreamEvent;
    use crate::services::api::{ChatRequest, ChatResponse, LlmProvider};
    use crate::tools::ToolRegistry;
    use async_openai::types::ChatCompletionResponseStream;
    use async_trait::async_trait;

    struct MockProvider;

    #[async_trait]
    impl LlmProvider for MockProvider {
        async fn chat(&self, _request: ChatRequest) -> anyhow::Result<ChatResponse> {
            Ok(ChatResponse {
                content: "ok".to_string(),
                tool_calls: None,
                usage: None,
                tool_call_repair: None,
            })
        }

        async fn chat_stream(
            &self,
            _request: ChatRequest,
        ) -> anyhow::Result<ChatCompletionResponseStream> {
            anyhow::bail!("streaming is not used by this test")
        }

        fn base_url(&self) -> &str {
            "mock://controller"
        }

        fn default_model(&self) -> &str {
            "mock-controller"
        }
    }

    #[test]
    fn turn_event_maps_all_stream_event_variants() {
        let events: Vec<StreamEvent> = vec![
            StreamEvent::Start,
            StreamEvent::TextChunk("hello".to_string()),
            StreamEvent::ThinkingStart,
            StreamEvent::ThinkingChunk("hmm".to_string()),
            StreamEvent::ThinkingComplete,
            StreamEvent::ToolCallStart {
                id: "t1".to_string(),
                name: "bash".to_string(),
            },
            StreamEvent::ToolCallArgs {
                id: "t1".to_string(),
                args_delta: r#"{"cmd":"ls"}"#.to_string(),
            },
            StreamEvent::ToolCallComplete {
                id: "t1".to_string(),
            },
            StreamEvent::ToolExecutionStart {
                id: "t1".to_string(),
                name: "bash".to_string(),
                metadata: Some(serde_json::json!({"risk": "safe"})),
            },
            StreamEvent::ToolExecutionProgress {
                id: "t1".to_string(),
                progress: "running...".to_string(),
            },
            StreamEvent::ToolExecutionComplete {
                id: "t1".to_string(),
                result: "README.md\n".to_string(),
                metadata: Some(serde_json::json!({"exit_code": 0})),
                result_data: None,
            },
            StreamEvent::Usage {
                prompt_tokens: 100,
                completion_tokens: 50,
                reasoning_tokens: Some(20),
                cached_tokens: Some(80),
            },
            StreamEvent::RuntimeDiagnostic {
                diagnostic: serde_json::json!({"ok": true}),
            },
            StreamEvent::Closeout {
                status: "verified".to_string(),
                evidence_summary: Some("tests passed".to_string()),
            },
            StreamEvent::Complete,
            StreamEvent::OutputTruncated,
            StreamEvent::Error("fail".to_string()),
            StreamEvent::PermissionRequest {
                id: "p1".to_string(),
                tool_name: "file_write".to_string(),
                arguments: serde_json::json!({"path": "/tmp/x"}),
                prompt: "approve?".to_string(),
                metadata: None,
                review: None,
            },
        ];

        let turn_events: Vec<TurnEvent> = events.into_iter().map(TurnEvent::from).collect();

        assert_eq!(turn_events.len(), 18);
        assert!(matches!(turn_events[0], TurnEvent::TurnStarted));
        assert!(matches!(turn_events[1], TurnEvent::TextDelta { .. }));
        assert!(matches!(turn_events[2], TurnEvent::ThinkingStarted));
        assert!(matches!(
            turn_events[13],
            TurnEvent::Closeout {
                status: CloseoutStatus::Verified,
                ..
            }
        ));
        assert!(matches!(turn_events[14], TurnEvent::TurnCompleted));
        assert!(matches!(turn_events[15], TurnEvent::OutputTruncated));
        assert!(matches!(turn_events[16], TurnEvent::TurnError { .. }));
        assert!(matches!(
            turn_events[17],
            TurnEvent::PermissionRequested { .. }
        ));
    }

    #[test]
    fn closeout_status_serialization_is_stable() {
        assert_eq!(
            serde_json::to_string(&CloseoutStatus::Verified).unwrap(),
            r#""verified""#
        );
        assert_eq!(
            serde_json::to_string(&CloseoutStatus::Partial).unwrap(),
            r#""partial""#
        );
        assert_eq!(
            serde_json::to_string(&CloseoutStatus::NotVerified).unwrap(),
            r#""not_verified""#
        );
        assert_eq!(
            serde_json::to_string(&CloseoutStatus::Failed).unwrap(),
            r#""failed""#
        );
    }

    #[test]
    fn turn_event_serialization_is_stable() {
        let event = TurnEvent::Closeout {
            status: CloseoutStatus::Verified,
            evidence_summary: Some("2/2 tests pass".to_string()),
        };
        let json = serde_json::to_string_pretty(&event).unwrap();
        assert!(json.contains(r#""status": "verified""#));
        assert!(json.contains(r#""evidence_summary": "2/2 tests pass""#));
    }

    #[test]
    fn controller_constructs_with_engine() {
        let engine = Arc::new(StreamingQueryEngine::new(
            Arc::new(MockProvider),
            Arc::new(ToolRegistry::default_registry()),
            "mock-controller",
        ));
        let controller = RuntimeController::new(engine);

        assert_eq!(controller.model_name(), "mock-controller");
        assert_eq!(controller.current_session(), None);
    }

    #[test]
    fn controller_session_roundtrip() {
        let engine = Arc::new(StreamingQueryEngine::new(
            Arc::new(MockProvider),
            Arc::new(ToolRegistry::default_registry()),
            "mock-controller",
        ));
        let controller = RuntimeController::new(engine);

        controller.set_session("session-42");
        assert_eq!(controller.current_session().as_deref(), Some("session-42"));
    }

    #[test]
    fn controller_memory_policy_propagates_to_engine() {
        let engine = Arc::new(StreamingQueryEngine::new(
            Arc::new(MockProvider),
            Arc::new(ToolRegistry::default_registry()),
            "mock-controller",
        ));
        let controller = RuntimeController::new(engine);

        controller.set_memory_policy(false, false, "minimal");

        // engine reflects policy
        assert!(!controller.engine().memory_use_enabled());
        assert!(!controller.engine().memory_generate_enabled());
        assert_eq!(controller.engine().memory_recall_mode(), "minimal");
    }

    #[tokio::test]
    async fn controller_approve_pending_returns_false_when_no_channel() {
        let engine = Arc::new(StreamingQueryEngine::new(
            Arc::new(MockProvider),
            Arc::new(ToolRegistry::default_registry()),
            "mock-controller",
        ));
        let controller = RuntimeController::new(engine);

        let answered = controller.approve_pending(true).await;
        assert!(!answered);
    }

    #[tokio::test]
    async fn controller_cancel_is_idempotent_when_no_request_in_flight() {
        let engine = Arc::new(StreamingQueryEngine::new(
            Arc::new(MockProvider),
            Arc::new(ToolRegistry::default_registry()),
            "mock-controller",
        ));
        let controller = RuntimeController::new(engine);

        // Calling cancel with no active request is safe (no-op).
        controller.cancel().await;
        let snapshot = controller.runtime_state().snapshot().await;
        // Phase remains Idle because cancel only affects active requests.
        assert!(!snapshot.is_querying);
    }

    #[test]
    fn controller_runtime_state_is_accessible() {
        let engine = Arc::new(StreamingQueryEngine::new(
            Arc::new(MockProvider),
            Arc::new(ToolRegistry::default_registry()),
            "mock-controller",
        ));
        let controller = RuntimeController::new(engine);

        let rt_state = controller.runtime_state();
        assert!(Arc::ptr_eq(rt_state, controller.runtime_state()));
    }

    #[test]
    fn turn_event_closeout_roundtrip() {
        let event = TurnEvent::Closeout {
            status: CloseoutStatus::NotVerified,
            evidence_summary: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: TurnEvent = serde_json::from_str(&json).unwrap();
        let TurnEvent::Closeout { status, .. } = parsed else {
            panic!("expected closeout");
        };
        assert_eq!(status, CloseoutStatus::NotVerified);
    }
}
