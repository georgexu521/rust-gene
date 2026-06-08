use super::memory_snapshot_controller::{MemorySnapshotController, MemorySnapshotInjectionContext};
use super::preflight_compression_controller::{
    PreflightCompressionContext, PreflightCompressionController,
};
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
