use super::memory_snapshot_controller::{MemorySnapshotController, MemorySnapshotInjectionContext};
use super::preflight_compression_controller::{
    PreflightCompressionContext, PreflightCompressionController,
};
use super::retrieval_prompt_controller::{RetrievalPromptContext, RetrievalPromptController};
use super::runtime_diet::RuntimeDietSnapshot;
use super::StreamEvent;
use crate::engine::context_compressor::ContextCompressor;
use crate::engine::intent_router::RetrievalPolicy;
use crate::engine::retrieval_context::RetrievalContext;
use crate::engine::trace::TraceCollector;
use crate::memory::MemoryManager;
use crate::services::api::{Message, Tool};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

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
                retrieval_context,
                messages,
                runtime_diet,
                trace,
            })
            .await;
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
}
