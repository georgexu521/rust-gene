use super::memory_snapshot_controller::{MemorySnapshotController, MemorySnapshotInjectionContext};
use super::preflight_compression_controller::{
    PreflightCompressionContext, PreflightCompressionController,
};
use super::runtime_diet::RuntimeDietSnapshot;
use super::StreamEvent;
use crate::engine::context_compressor::ContextCompressor;
use crate::engine::intent_router::RetrievalPolicy;
use crate::engine::retrieval_context::RetrievalContext;
use crate::engine::trace::{TraceCollector, TraceEvent};
use crate::memory::MemoryManager;
use crate::services::api::{Message, Tool};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

struct RetrievalPromptContext<'a> {
    retrieval_context: Option<&'a RetrievalContext>,
    messages: &'a mut Vec<Message>,
}

struct RetrievalPromptController;

impl RetrievalPromptController {
    fn inject(context: RetrievalPromptContext<'_>) -> bool {
        let Some(retrieval_context) = context.retrieval_context else {
            return false;
        };
        let block = retrieval_context.format_for_prompt();
        Self::inject_block(context.messages, &block)
    }

    fn inject_block(messages: &mut Vec<Message>, block: &str) -> bool {
        let block = block.trim();
        if block.is_empty()
            || messages
                .iter()
                .any(|message| matches!(message, Message::System { content } if content.contains("<retrieval-context") || content.contains("project.index:")))
        {
            return false;
        }
        let block = if block.contains("<relevant_material>") {
            block.to_string()
        } else {
            format!("<relevant_material>\n{block}\n</relevant_material>")
        };
        super::request_preparation_controller::prepend_to_last_user_message(messages, &block);
        true
    }
}

pub(super) struct TurnRequestBootstrapContext<'a> {
    pub(super) retrieval_policy: RetrievalPolicy,
    pub(super) memory_manager: Option<&'a Arc<Mutex<MemoryManager>>>,
    pub(super) compressor: Option<&'a Arc<Mutex<ContextCompressor>>>,
    pub(super) session_store: Option<&'a Arc<crate::session_store::SessionStore>>,
    pub(super) session_id: &'a str,
    pub(super) messages: &'a mut Vec<Message>,
    pub(super) tools: &'a [Tool],
    pub(super) retrieval_context: Option<&'a RetrievalContext>,
    pub(super) runtime_diet: &'a mut RuntimeDietSnapshot,
    pub(super) trace: &'a TraceCollector,
    pub(super) tx: Option<&'a mpsc::Sender<StreamEvent>>,
    pub(super) inject_dynamic_context: bool,
}

pub(super) struct TurnRequestBootstrapController;

impl TurnRequestBootstrapController {
    pub(super) async fn run(context: TurnRequestBootstrapContext<'_>) {
        let TurnRequestBootstrapContext {
            retrieval_policy,
            memory_manager,
            compressor,
            session_store,
            session_id,
            messages,
            tools,
            retrieval_context,
            runtime_diet,
            trace,
            tx,
            inject_dynamic_context,
        } = context;

        if inject_dynamic_context {
            MemorySnapshotController::inject(MemorySnapshotInjectionContext {
                retrieval_policy,
                memory_manager,
                messages,
                runtime_diet,
                trace,
            })
            .await;
        }

        let prune_report =
            crate::engine::message_compression::background_prune_tool_outputs(messages);
        if prune_report.pruned_count > 0 {
            trace.record(TraceEvent::WorkflowFallback {
                error: format!(
                    "background prune: pruned={} evidence_preserved={} chars_before={} chars_after={} saved={}",
                    prune_report.pruned_count,
                    prune_report.evidence_preserved,
                    prune_report.chars_before,
                    prune_report.chars_after,
                    prune_report
                        .chars_before
                        .saturating_sub(prune_report.chars_after),
                ),
            });
        }

        match crate::engine::context_collapse::apply_session_context_collapse_if_needed(
            session_id, messages,
        )
        .await
        {
            Ok(collapsed) if collapsed > 0 => {
                trace.record(TraceEvent::WorkflowFallback {
                    error: format!("context collapse: collapsed_messages={collapsed}"),
                });
            }
            Err(err) => {
                trace.record(TraceEvent::WorkflowFallback {
                    error: format!("context collapse failed: {err}"),
                });
            }
            _ => {}
        }

        PreflightCompressionController::run(PreflightCompressionContext {
            compressor,
            session_store,
            session_id,
            messages,
            tools,
            runtime_diet,
            trace,
        })
        .await;

        if let Some(tx) = tx {
            let _ = tx.send(StreamEvent::Start).await;
        }

        if inject_dynamic_context {
            RetrievalPromptController::inject(RetrievalPromptContext {
                retrieval_context,
                messages,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::retrieval_context::RetrievalContext;
    use crate::engine::trace::TurnTrace;

    #[tokio::test]
    async fn run_sends_stream_start_and_injects_retrieval_prompt() {
        let mut messages = vec![Message::user("inspect repo")];
        let tools = Vec::new();
        let retrieval_context = RetrievalContext::from_project_summary(
            "inspect repo",
            "src/main.rs",
            "/tmp/project",
            RetrievalPolicy::Project,
        )
        .expect("project context");
        let mut runtime_diet = RuntimeDietSnapshot::new(true);
        let trace = TraceCollector::new(TurnTrace::new("session-test", 1, "inspect repo"));
        let (tx, mut rx) = mpsc::channel(1);

        TurnRequestBootstrapController::run(TurnRequestBootstrapContext {
            retrieval_policy: RetrievalPolicy::Project,
            memory_manager: None,
            compressor: None,
            session_store: None,
            session_id: "session-test",
            messages: &mut messages,
            tools: &tools,
            retrieval_context: Some(&retrieval_context),
            runtime_diet: &mut runtime_diet,
            trace: &trace,
            tx: Some(&tx),
            inject_dynamic_context: true,
        })
        .await;

        assert!(matches!(rx.recv().await, Some(StreamEvent::Start)));
        // Phase 0 Risk 3: retrieval is now in user message, not system
        assert!(messages.iter().any(|message| matches!(
            message,
            Message::User { content } if content.contains("project.index:")
        )));
    }

    #[tokio::test]
    async fn run_background_prunes_old_tool_outputs_before_request() {
        let mut env = crate::test_utils::env_guard::EnvVarGuard::acquire().await;
        env.set("PRIORITY_AGENT_BACKGROUND_PRUNE", "1");
        env.remove("PRIORITY_AGENT_CONTEXT_COLLAPSE");
        let mut messages = vec![
            Message::user("first"),
            Message::tool("old_tool", "ordinary output\n".repeat(80)),
            Message::assistant("ok"),
            Message::user("second"),
            Message::tool("recent_one", "recent"),
            Message::user("third"),
            Message::tool("recent_two", "recent"),
        ];
        let tools = Vec::new();
        let mut runtime_diet = RuntimeDietSnapshot::new(true);
        let trace = TraceCollector::new(TurnTrace::new("session-test", 1, "inspect repo"));

        TurnRequestBootstrapController::run(TurnRequestBootstrapContext {
            retrieval_policy: RetrievalPolicy::Project,
            memory_manager: None,
            compressor: None,
            session_store: None,
            session_id: "session-test",
            messages: &mut messages,
            tools: &tools,
            retrieval_context: None,
            runtime_diet: &mut runtime_diet,
            trace: &trace,
            tx: None,
            inject_dynamic_context: false,
        })
        .await;

        let old_tool = match &messages[1] {
            Message::Tool { content, .. } => content,
            _ => panic!("expected tool"),
        };
        assert!(old_tool.contains("[compressed-tool-output]"));
        assert!(old_tool.contains("evidence_safe_for_closeout=false"));
    }

    #[tokio::test]
    async fn run_applies_context_collapse_when_enabled() {
        let mut env = crate::test_utils::env_guard::EnvVarGuard::acquire().await;
        env.set("PRIORITY_AGENT_CONTEXT_COLLAPSE", "1");
        env.set("PRIORITY_AGENT_CONTEXT_COLLAPSE_WINDOW", "2");
        env.set("PRIORITY_AGENT_CONTEXT_COLLAPSE_THRESHOLD", "3");
        let storage_dir = std::env::temp_dir().join(format!(
            "priority-agent-bootstrap-collapse-{}",
            uuid::Uuid::new_v4()
        ));
        env.set(
            "PRIORITY_AGENT_CONTEXT_COLLAPSE_DIR",
            storage_dir.to_string_lossy().as_ref(),
        );

        let mut messages = vec![
            Message::user("one"),
            Message::assistant("two"),
            Message::user("three"),
            Message::assistant("four"),
            Message::user("five"),
        ];
        let tools = Vec::new();
        let mut runtime_diet = RuntimeDietSnapshot::new(true);
        let session_id = format!("collapse-session-test-{}", uuid::Uuid::new_v4());
        let trace = TraceCollector::new(TurnTrace::new(&session_id, 1, "collapse"));

        TurnRequestBootstrapController::run(TurnRequestBootstrapContext {
            retrieval_policy: RetrievalPolicy::Project,
            memory_manager: None,
            compressor: None,
            session_store: None,
            session_id: &session_id,
            messages: &mut messages,
            tools: &tools,
            retrieval_context: None,
            runtime_diet: &mut runtime_diet,
            trace: &trace,
            tx: None,
            inject_dynamic_context: false,
        })
        .await;

        assert_eq!(messages.len(), 2);
        assert!(matches!(
            messages.first(),
            Some(Message::Assistant { content, .. }) if content == "four"
        ));
        let _ = tokio::fs::remove_dir_all(storage_dir).await;
    }

    #[test]
    fn injects_nonempty_retrieval_block_as_relevant_material_before_user() {
        let mut messages = vec![
            Message::system("base system prompt"),
            Message::user("inspect repo"),
        ];

        assert!(RetrievalPromptController::inject_block(
            &mut messages,
            "<retrieval-context>\nproject.index: src/main.rs\n</retrieval-context>",
        ));

        assert_eq!(messages.len(), 2);
        assert!(matches!(
            &messages[1],
            Message::User { content }
                if content.contains("<relevant_material>")
                    && content.contains("<retrieval-context>")
                    && content.contains("project.index: src/main.rs")
                    && content.ends_with("inspect repo")
        ));
    }

    #[test]
    fn skips_empty_or_existing_project_index_block() {
        let mut messages = vec![Message::user("inspect repo")];

        assert!(!RetrievalPromptController::inject_block(&mut messages, ""));
        assert_eq!(messages.len(), 1);

        messages.push(Message::system("project.index: existing"));
        assert!(!RetrievalPromptController::inject_block(
            &mut messages,
            "project.index: new",
        ));
        assert_eq!(messages.len(), 2);
    }
}
