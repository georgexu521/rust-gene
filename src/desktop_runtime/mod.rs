//! Desktop-facing runtime facade.
//!
//! The Tauri app should depend on this boundary instead of reaching into
//! conversation-loop internals directly.
//!
//! Full-agent turns are routed through `RuntimeController` (Phase 1 Reasonix
//! alignment). Lightweight turns remain a separate direct-provider lane.

use crate::engine::runtime_controller::RuntimeController;
use crate::engine::streaming::{StreamEvent, StreamingQueryEngine};
use crate::engine::turn_ingress::{lightweight_user_text, TurnIngressLane};
use crate::services::api::{sanitize_assistant_content, ChatRequest, Message, Usage};
use crate::session_store::{MessageRecord, SessionStore};
use futures::Stream;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;

#[derive(Clone)]
pub struct DesktopRuntime {
    controller: RuntimeController,
    streaming_engine: Arc<StreamingQueryEngine>,
    working_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct DesktopContextSnapshot {
    pub history_messages: usize,
    pub history_tokens: u64,
    pub tool_schema_tokens: u64,
    pub memory_snapshot_tokens: u64,
    pub total_estimated_tokens: u64,
    pub max_context_tokens: u64,
    pub usage_percent: u64,
    pub stable_prefix_fingerprint: String,
    pub prompt_cache_cached_tokens: u64,
    pub prompt_cache_miss_tokens: u64,
    pub prompt_cache_hit_rate_percent: f64,
    pub prompt_cache_diagnostic_count: usize,
    pub prompt_cache_last_reason: Option<String>,
    pub compact: DesktopCompactState,
}

#[derive(Debug, Clone, Serialize)]
pub struct DesktopCompactState {
    pub compression_count: u32,
    pub circuit_open: bool,
    pub latest_strategy: Option<String>,
    pub latest_boundary_id: Option<String>,
    pub latest_attempt_decision: Option<String>,
    pub latest_attempt_reason: Option<String>,
    pub latest_attempt_trigger: Option<String>,
    pub latest_attempt_tokens_before: Option<u64>,
    pub latest_attempt_tokens_after: Option<u64>,
}

impl DesktopRuntime {
    pub fn from_streaming_engine(
        streaming_engine: Arc<StreamingQueryEngine>,
        working_dir: impl Into<PathBuf>,
    ) -> Self {
        let controller = RuntimeController::new(streaming_engine.clone());
        Self {
            controller,
            streaming_engine,
            working_dir: working_dir.into(),
        }
    }

    /// The frontend-neutral runtime controller for full-agent turns.
    pub fn controller(&self) -> &RuntimeController {
        &self.controller
    }

    pub async fn initialize(working_dir: impl AsRef<Path>) -> anyhow::Result<Self> {
        let working_dir = working_dir.as_ref().to_path_buf();
        let (provider, model) = crate::bootstrap::init_provider()?;
        let tool_registry = crate::bootstrap::init_tool_registry(&working_dir);
        let components =
            crate::bootstrap::init_components(provider, model, tool_registry, &working_dir).await?;

        Ok(Self::from_streaming_engine(
            components.streaming_engine,
            working_dir,
        ))
    }

    pub async fn initialize_for_session(
        working_dir: impl AsRef<Path>,
        session_id: impl AsRef<str>,
    ) -> anyhow::Result<Self> {
        let runtime = Self::initialize(working_dir).await?;
        let bootstrap_session_id = runtime.streaming_engine.current_session_id();
        let store = SessionStore::open(SessionStore::default_path())?;
        runtime
            .restore_session_history(&store, session_id.as_ref())
            .await?;
        runtime.cleanup_empty_bootstrap_session(
            &store,
            session_id.as_ref(),
            bootstrap_session_id.as_deref(),
        );
        Ok(runtime)
    }

    pub async fn restore_session_history(
        &self,
        store: &SessionStore,
        session_id: &str,
    ) -> anyhow::Result<()> {
        let records = store.restore_compacted_messages(session_id)?;
        let history = records
            .into_iter()
            .filter_map(message_record_to_history_message)
            .collect::<Vec<_>>();
        self.streaming_engine.set_session_id(session_id.to_string());
        self.streaming_engine.set_history(history).await;
        Ok(())
    }

    pub fn streaming_engine(&self) -> Arc<StreamingQueryEngine> {
        self.streaming_engine.clone()
    }

    pub fn current_session_id(&self) -> Option<String> {
        self.streaming_engine.current_session_id()
    }

    /// Full agent turn through the frontend-neutral `RuntimeController`.
    ///
    /// This is the canonical full-agent lane. TUI and desktop should both
    /// route full-agent turns through `RuntimeController::submit_turn()`.
    /// For now, the stream returns `StreamEvent` for backward compatibility
    /// with the Tauri event loop; Phase 6 can migrate to `TurnEvent`.
    pub async fn run_full_turn(
        &self,
        user_message: impl Into<String>,
    ) -> Pin<Box<dyn Stream<Item = StreamEvent> + Send>> {
        self.controller.submit_stream_turn(user_message).await
    }

    pub async fn run_lightweight_turn(
        &self,
        user_message: &str,
        lane: TurnIngressLane,
    ) -> anyhow::Result<DesktopLightweightTurnOutcome> {
        let prompt = lightweight_user_text(user_message, lane);
        let request = ChatRequest {
            max_tokens: Some(512),
            ..ChatRequest::new(self.streaming_engine.model_name()).with_messages(vec![
                Message::system(LIGHTWEIGHT_CHAT_SYSTEM_PROMPT),
                Message::user(prompt.clone()),
            ])
        };
        let response = self.streaming_engine.provider().chat(request).await?;
        let answer = sanitize_lightweight_answer(&response.content);
        Ok(DesktopLightweightTurnOutcome {
            lane,
            answer,
            usage: response.usage,
        })
    }

    pub async fn compact_context(
        &self,
    ) -> Option<crate::engine::context_compressor::CompactionAttemptRecord> {
        self.controller.compact().await
    }

    /// Context snapshot for frontend rendering.
    ///
    /// Delegates to `RuntimeController::context_snapshot()` (Phase 1 boundary).
    pub async fn context_snapshot(&self) -> DesktopContextSnapshot {
        self.controller.context_snapshot().await
    }

    pub fn working_dir(&self) -> &Path {
        &self.working_dir
    }

    fn cleanup_empty_bootstrap_session(
        &self,
        store: &SessionStore,
        restored_session_id: &str,
        bootstrap_session_id: Option<&str>,
    ) {
        let Some(current_session_id) = bootstrap_session_id else {
            return;
        };
        if current_session_id == restored_session_id {
            return;
        }
        if store
            .message_count(current_session_id)
            .map(|count| count == 0)
            .unwrap_or(false)
        {
            let _ = store.delete_session(current_session_id);
        }
    }
}

const LIGHTWEIGHT_CHAT_SYSTEM_PROMPT: &str = "You are Liz, gex's concise AI coding partner. Answer this one user message directly in plain prose. Reply in the user's language. You have no tools in this lightweight lane: do not claim to inspect files, run commands, edit files, or verify anything. If the request requires project inspection or code changes, say it needs the full agent lane.";

#[derive(Debug, Clone)]
pub struct DesktopLightweightTurnOutcome {
    pub lane: TurnIngressLane,
    pub answer: String,
    pub usage: Option<Usage>,
}

fn sanitize_lightweight_answer(content: &str) -> String {
    let sanitized = sanitize_assistant_content(content);
    let sanitized = strip_hallucinated_tool_envelopes(&sanitized);
    if sanitized.trim().is_empty() {
        "I could not produce a plain-text answer from the lightweight lane.".to_string()
    } else {
        sanitized.trim().to_string()
    }
}

fn strip_hallucinated_tool_envelopes(content: &str) -> String {
    let mut out = content.to_string();
    for (open, close) in [
        ("<function_calls>", "</function_calls>"),
        ("<|DSML|function_calls>", "</|DSML|function_calls>"),
        ("<｜DSML｜function_calls>", "</｜DSML｜function_calls>"),
    ] {
        out = strip_literal_block(&out, open, close);
    }
    for open in ["<｜DSML｜", "<|DSML|"] {
        if let Some(index) = out.find(open) {
            out.truncate(index);
        }
    }
    out
}

fn strip_literal_block(input: &str, open: &str, close: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(start) = rest.find(open) {
        output.push_str(&rest[..start]);
        let after_open = &rest[start + open.len()..];
        let Some(end) = after_open.find(close) else {
            return output;
        };
        rest = &after_open[end + close.len()..];
    }
    output.push_str(rest);
    output
}

fn message_record_to_history_message(record: MessageRecord) -> Option<Message> {
    match record.role.as_str() {
        "user" => Some(Message::user(record.content)),
        "assistant" => Some(Message::assistant(record.content)),
        "tool" => record
            .tool_call_id
            .map(|tool_call_id| Message::tool(tool_call_id, record.content)),
        _ => None,
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DesktopRunEvent {
    RunStarted {
        run_id: String,
        session_id: Option<String>,
    },
    AssistantDelta {
        text: String,
    },
    ThinkingStarted,
    ThinkingDelta {
        text: String,
    },
    ThinkingCompleted,
    ToolStarted {
        id: String,
        name: String,
    },
    ToolArgsDelta {
        id: String,
        delta: String,
    },
    ToolCallCompleted {
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
    PermissionRequest {
        id: String,
        tool_name: String,
        arguments: serde_json::Value,
        prompt: String,
        metadata: Option<serde_json::Value>,
        review: Option<Box<crate::engine::human_review::HumanReviewAuditRecord>>,
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
        status: String,
        evidence_summary: Option<String>,
    },
    RunCompleted,
    OutputTruncated,
    RunError {
        message: String,
    },
}

impl DesktopRunEvent {
    pub fn from_stream_event(event: StreamEvent) -> Self {
        match event {
            StreamEvent::Start => Self::RunStarted {
                run_id: uuid::Uuid::new_v4().to_string(),
                session_id: None,
            },
            StreamEvent::TextChunk(text) => Self::AssistantDelta { text },
            StreamEvent::ToolCallStart { id, name } => Self::ToolStarted { id, name },
            StreamEvent::ToolCallArgs { id, args_delta } => Self::ToolArgsDelta {
                id,
                delta: args_delta,
            },
            StreamEvent::ToolCallComplete { id } => Self::ToolCallCompleted { id },
            StreamEvent::ToolExecutionStart { id, name, .. } => Self::ToolStarted { id, name },
            StreamEvent::ToolExecutionProgress { id, progress } => {
                Self::ToolExecutionProgress { id, progress }
            }
            StreamEvent::ToolExecutionComplete {
                id,
                result,
                metadata,
                ..
            } => Self::ToolCompleted {
                id,
                result_preview: truncate_preview(&result, 2000),
                metadata,
            },
            StreamEvent::ThinkingStart => Self::ThinkingStarted,
            StreamEvent::ThinkingChunk(text) => Self::ThinkingDelta { text },
            StreamEvent::ThinkingComplete => Self::ThinkingCompleted,
            StreamEvent::Usage {
                prompt_tokens,
                completion_tokens,
                reasoning_tokens,
                cached_tokens,
            } => Self::Usage {
                prompt_tokens,
                completion_tokens,
                reasoning_tokens,
                cached_tokens,
            },
            StreamEvent::RuntimeDiagnostic { diagnostic } => Self::RuntimeDiagnostic { diagnostic },
            StreamEvent::Closeout {
                status,
                evidence_summary,
            } => Self::Closeout {
                status,
                evidence_summary,
            },
            StreamEvent::Complete => Self::RunCompleted,
            StreamEvent::OutputTruncated => Self::OutputTruncated,
            StreamEvent::Error(message) => Self::RunError { message },
            StreamEvent::PermissionRequest {
                id,
                tool_name,
                arguments,
                prompt,
                metadata,
                review,
            } => Self::PermissionRequest {
                id,
                tool_name,
                arguments,
                prompt,
                metadata,
                review,
            },
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
    use crate::services::api::{ChatRequest, ChatResponse, LlmProvider};
    use crate::tools::ToolRegistry;
    use async_openai::types::ChatCompletionResponseStream;
    use async_trait::async_trait;
    use std::sync::Mutex as StdMutex;

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
            "mock://desktop-runtime"
        }

        fn default_model(&self) -> &str {
            "mock-desktop"
        }
    }

    struct RecordingProvider {
        request: StdMutex<Option<ChatRequest>>,
    }

    #[async_trait]
    impl LlmProvider for RecordingProvider {
        async fn chat(&self, request: ChatRequest) -> anyhow::Result<ChatResponse> {
            *self.request.lock().unwrap() = Some(request);
            Ok(ChatResponse {
                content: "你好，我在。".to_string(),
                tool_calls: None,
                usage: Some(crate::services::api::Usage {
                    prompt_tokens: 12,
                    completion_tokens: 4,
                    total_tokens: 16,
                    reasoning_tokens: None,
                    cached_tokens: Some(8),
                }),
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
            "mock://recording"
        }

        fn default_model(&self) -> &str {
            "recording-model"
        }
    }

    #[test]
    fn desktop_runtime_can_own_streaming_engine() {
        let engine = Arc::new(StreamingQueryEngine::new(
            Arc::new(MockProvider),
            Arc::new(ToolRegistry::default_registry()),
            "mock-desktop",
        ));
        let runtime = DesktopRuntime::from_streaming_engine(engine, "/tmp/project");

        assert_eq!(runtime.working_dir(), Path::new("/tmp/project"));
        assert_eq!(runtime.streaming_engine().model_name(), "mock-desktop");
        assert!(runtime.current_session_id().is_none());
    }

    #[test]
    fn maps_stream_events_to_stable_desktop_events() {
        let event = DesktopRunEvent::from_stream_event(StreamEvent::ToolExecutionComplete {
            id: "tool-1".to_string(),
            result: "done".to_string(),
            metadata: Some(serde_json::json!({ "ok": true })),
            result_data: None,
        });

        assert_eq!(
            event,
            DesktopRunEvent::ToolCompleted {
                id: "tool-1".to_string(),
                result_preview: "done".to_string(),
                metadata: Some(serde_json::json!({ "ok": true })),
            }
        );
    }

    #[test]
    fn maps_runtime_diagnostic_stream_event() {
        let diagnostic = serde_json::json!({
            "schema": "desktop_runtime_diagnostic.v1",
            "control_loop": { "coverage": "2/7" },
        });
        let event = DesktopRunEvent::from_stream_event(StreamEvent::RuntimeDiagnostic {
            diagnostic: diagnostic.clone(),
        });

        assert_eq!(event, DesktopRunEvent::RuntimeDiagnostic { diagnostic });
    }

    #[test]
    fn maps_closeout_stream_event() {
        let event = DesktopRunEvent::from_stream_event(StreamEvent::Closeout {
            status: "verified".to_string(),
            evidence_summary: Some("tests passed".to_string()),
        });

        assert_eq!(
            event,
            DesktopRunEvent::Closeout {
                status: "verified".to_string(),
                evidence_summary: Some("tests passed".to_string()),
            }
        );
    }

    #[test]
    fn truncates_long_tool_result_preview_on_char_boundaries() {
        let event = DesktopRunEvent::from_stream_event(StreamEvent::ToolExecutionComplete {
            id: "tool-1".to_string(),
            result: "好".repeat(2100),
            metadata: None,
            result_data: None,
        });

        let DesktopRunEvent::ToolCompleted { result_preview, .. } = event else {
            panic!("expected tool completion event");
        };

        assert_eq!(result_preview.chars().count(), 2000);
        assert!(result_preview.ends_with('…'));
    }

    #[tokio::test]
    async fn restores_session_id_and_history_from_store() {
        let store = SessionStore::in_memory().unwrap();
        store
            .create_session("session-1", "Desktop Session", "mock-desktop")
            .unwrap();
        store
            .add_message("session-1", "user", "hello", None, None)
            .unwrap();
        store
            .add_message("session-1", "assistant", "hi", None, None)
            .unwrap();
        let engine = Arc::new(StreamingQueryEngine::new(
            Arc::new(MockProvider),
            Arc::new(ToolRegistry::default_registry()),
            "mock-desktop",
        ));
        let runtime = DesktopRuntime::from_streaming_engine(engine, "/tmp/project");

        runtime
            .restore_session_history(&store, "session-1")
            .await
            .unwrap();

        assert_eq!(
            runtime.streaming_engine().current_session_id().as_deref(),
            Some("session-1")
        );
        assert_eq!(runtime.streaming_engine().get_history().await.len(), 2);
    }

    #[tokio::test]
    async fn desktop_runtime_restored_compact_session_reports_boundary_state() {
        let store = Arc::new(SessionStore::in_memory().unwrap());
        store
            .create_session("session-compact", "Desktop Session", "mock-desktop")
            .unwrap();
        store
            .add_message("session-compact", "user", "hello", None, None)
            .unwrap();
        store
            .add_compact_boundary(&crate::session_store::CompactBoundaryInsert {
                session_id: "session-compact".to_string(),
                boundary_id: "boundary-1".to_string(),
                sequence: Some(1),
                strategy: "session_memory_compact".to_string(),
                trigger: Some("manual compact".to_string()),
                before_tokens: 1200,
                after_tokens: 400,
                messages_before: 9,
                messages_after: 3,
                preserved_tail_count: Some(2),
                retained_items: serde_json::json!([]),
                provenance: serde_json::json!([]),
                summary: "compacted state".to_string(),
                payload: serde_json::json!({}),
            })
            .unwrap();
        let engine = Arc::new(
            StreamingQueryEngine::new(
                Arc::new(MockProvider),
                Arc::new(ToolRegistry::default_registry()),
                "mock-desktop",
            )
            .with_session_store(store.clone(), "session-compact".to_string()),
        );
        let runtime = DesktopRuntime::from_streaming_engine(engine, "/tmp/project");
        runtime
            .restore_session_history(&store, "session-compact")
            .await
            .unwrap();

        let snapshot = runtime.context_snapshot().await;

        assert_eq!(
            snapshot.compact.latest_boundary_id.as_deref(),
            Some("boundary-1")
        );
        assert_eq!(
            snapshot.compact.latest_strategy.as_deref(),
            Some("session_memory_compact")
        );
    }

    #[tokio::test]
    async fn side_question_turn_uses_no_tools_and_does_not_touch_agent_history() {
        let provider = Arc::new(RecordingProvider {
            request: StdMutex::new(None),
        });
        let store = Arc::new(SessionStore::in_memory().unwrap());
        store
            .create_session("session-light", "CLI Session", "recording-model")
            .unwrap();
        let engine = Arc::new(
            StreamingQueryEngine::new(
                provider.clone(),
                Arc::new(ToolRegistry::default_registry()),
                "recording-model",
            )
            .with_session_store(store.clone(), "session-light".to_string()),
        );
        let runtime = DesktopRuntime::from_streaming_engine(engine, "/tmp/project");

        let outcome = runtime
            .run_lightweight_turn("/btw Rust 的 trait 是什么？", TurnIngressLane::SideQuestion)
            .await
            .unwrap();

        assert_eq!(outcome.lane, TurnIngressLane::SideQuestion);
        assert_eq!(outcome.answer, "你好，我在。");
        assert_eq!(
            outcome.usage.as_ref().and_then(|u| u.cached_tokens),
            Some(8)
        );
        assert!(runtime.streaming_engine().get_history().await.is_empty());
        assert!(store.get_messages("session-light").unwrap().is_empty());

        let request = provider.request.lock().unwrap().clone().unwrap();
        assert!(request.tools.is_none());
        assert!(request.tool_choice.is_none());
        assert_eq!(request.max_tokens, Some(512));
        assert_eq!(request.messages.len(), 2);
        assert!(matches!(
            &request.messages[1],
            Message::User { content } if content == "Rust 的 trait 是什么？"
        ));
    }

    #[test]
    fn lightweight_sanitizer_strips_hallucinated_tool_markup() {
        assert_eq!(
            sanitize_lightweight_answer("Answer\n<function_calls>{}</function_calls>"),
            "Answer"
        );
        assert_eq!(
            sanitize_lightweight_answer("Visible\n<｜DSML｜function_calls>{}"),
            "Visible"
        );
    }
}
