use super::runtime_diet::RuntimeDietSnapshot;
use crate::engine::intent_router::RetrievalPolicy;
use crate::engine::retrieval_context::{RetrievalContext, RetrievalSource};
use crate::engine::trace::{TraceCollector, TraceEvent};
use crate::memory::MemoryManager;
use crate::services::api::Message;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::debug;

pub(super) struct MemorySnapshotInjectionContext<'a> {
    pub(super) retrieval_policy: RetrievalPolicy,
    pub(super) memory_manager: Option<&'a Arc<Mutex<MemoryManager>>>,
    pub(super) messages: &'a mut Vec<Message>,
    pub(super) runtime_diet: &'a mut RuntimeDietSnapshot,
    pub(super) trace: &'a TraceCollector,
}

pub(super) struct MemorySnapshotController;

impl MemorySnapshotController {
    pub(super) async fn inject(context: MemorySnapshotInjectionContext<'_>) -> bool {
        if !context.retrieval_policy.allows_memory_context() {
            return false;
        }
        // Phase 0 Risk 2: Always inject pinned snapshot when memory is enabled,
        // even when dynamic recall also exists. The pinned snapshot is a compact
        // index that keeps the stable prefix consistent across turns.
        // Dynamic recall details are in the user tail / relevant_material zone.

        let Some(memory_manager) = context.memory_manager else {
            return false;
        };
        let memory = memory_manager.lock().await;
        let snapshot = memory.get_snapshot();
        Self::inject_snapshot(
            context.messages,
            &snapshot,
            context.runtime_diet,
            context.trace,
        )
    }

    #[allow(dead_code)] // kept for diagnostics/trace (Phase 0 Risk 2)
    fn has_dynamic_memory_recall(retrieval_context: Option<&RetrievalContext>) -> bool {
        retrieval_context
            .map(|ctx| ctx.item_count_by_source(RetrievalSource::Memory) > 0)
            .unwrap_or(false)
    }

    fn inject_snapshot(
        messages: &mut Vec<Message>,
        snapshot: &str,
        runtime_diet: &mut RuntimeDietSnapshot,
        trace: &TraceCollector,
    ) -> bool {
        if snapshot.is_empty()
            || messages
                .iter()
                .any(|message| matches!(message, Message::System { content } if content.contains("<memory-context>")))
        {
            return false;
        }

        runtime_diet.observe_memory_snapshot(snapshot);
        trace.record(TraceEvent::MemorySnapshotInjected {
            chars: snapshot.chars().count(),
        });
        let insert_pos = messages
            .iter()
            .position(|message| !matches!(message, Message::System { .. }))
            .unwrap_or(messages.len());
        messages.insert(insert_pos, Message::system(snapshot));
        debug!("Injected memory context fence at position {}", insert_pos);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::retrieval_context::RetrievalContext;
    use crate::engine::trace::{TurnStatus, TurnTrace};

    fn trace() -> TraceCollector {
        TraceCollector::new(TurnTrace::new("session", 1, "test"))
    }

    #[test]
    fn injects_snapshot_before_first_non_system_message_and_records_diet() {
        let trace = trace();
        let mut runtime_diet = RuntimeDietSnapshot::new(true);
        let mut messages = vec![Message::system("base"), Message::user("hello")];
        let snapshot = "<memory-context>\nremember this\n</memory-context>";

        let injected = MemorySnapshotController::inject_snapshot(
            &mut messages,
            snapshot,
            &mut runtime_diet,
            &trace,
        );

        assert!(injected);
        assert!(matches!(
            &messages[1],
            Message::System { content } if content == snapshot
        ));
        assert_eq!(runtime_diet.memory_snapshot_chars, snapshot.chars().count());
        assert!(runtime_diet.memory_snapshot_tokens > 0);
        let finished = trace.finish(TurnStatus::Completed);
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::MemorySnapshotInjected { chars } if *chars == snapshot.chars().count()
        )));
    }

    #[test]
    fn dynamic_memory_recall_detection_still_works_for_diagnostics() {
        let memory_context = RetrievalContext::from_memory_prefetch(
            "fix bug",
            "Run cargo check after edits.",
            RetrievalPolicy::Project,
        )
        .expect("memory context");

        // Phase 0 Risk 2: dynamic recall no longer blocks pinned snapshot,
        // but the detection is still available for diagnostics/trace
        assert!(MemorySnapshotController::has_dynamic_memory_recall(Some(
            &memory_context
        )));
    }

    #[test]
    fn allows_snapshot_when_retrieval_context_has_no_memory_recall() {
        let project_context = RetrievalContext::from_project_summary(
            "fix bug",
            "src/main.rs",
            "/tmp/project",
            RetrievalPolicy::Project,
        )
        .expect("project context");

        assert!(!MemorySnapshotController::has_dynamic_memory_recall(Some(
            &project_context
        )));
    }

    #[test]
    fn skips_empty_or_existing_memory_context() {
        let trace = trace();
        let mut runtime_diet = RuntimeDietSnapshot::new(true);
        let mut messages = vec![Message::user("hello")];

        assert!(!MemorySnapshotController::inject_snapshot(
            &mut messages,
            "",
            &mut runtime_diet,
            &trace,
        ));
        assert_eq!(messages.len(), 1);

        messages.insert(
            0,
            Message::system("<memory-context>existing</memory-context>"),
        );
        assert!(!MemorySnapshotController::inject_snapshot(
            &mut messages,
            "<memory-context>new</memory-context>",
            &mut runtime_diet,
            &trace,
        ));
        assert_eq!(messages.len(), 2);
        assert_eq!(runtime_diet.memory_snapshot_chars, 0);
    }
}
