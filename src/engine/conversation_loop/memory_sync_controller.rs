use crate::engine::trace::{TraceCollector, TraceEvent};
use crate::memory::MemoryManager;
use crate::services::api::{LlmProvider, Message};
use std::sync::Arc;
use tokio::sync::Mutex;

pub(super) struct MemorySyncContext<'a> {
    pub(super) memory_manager: Option<&'a Arc<Mutex<MemoryManager>>>,
    pub(super) llm_memory_extraction: bool,
    pub(super) provider: Option<&'a dyn LlmProvider>,
    pub(super) model: &'a str,
    pub(super) trace: &'a TraceCollector,
    pub(super) messages: &'a [Message],
    pub(super) final_content: &'a str,
    pub(super) tool_results_text: &'a str,
}

pub(super) struct MemorySyncController;

impl MemorySyncController {
    pub(super) async fn sync_turn(context: MemorySyncContext<'_>) {
        let Some(memory_manager) = context.memory_manager else {
            return;
        };
        let mut memory = memory_manager.lock().await;
        let user_msg = Self::latest_user_message(context.messages);
        if !user_msg.is_empty() {
            let assistant_text =
                Self::assistant_memory_text(context.final_content, context.tool_results_text);
            if context.llm_memory_extraction {
                if memory.should_extract_with_llm() {
                    memory
                        .sync_turn_llm(user_msg, &assistant_text, context.provider, context.model)
                        .await;
                    memory.mark_main_agent_wrote();
                    context.trace.record(TraceEvent::MemorySynced {
                        mode: "llm".to_string(),
                    });
                }
            } else {
                memory.sync_turn(user_msg, &assistant_text);
                memory.mark_main_agent_wrote();
                context.trace.record(TraceEvent::MemorySynced {
                    mode: "heuristic".to_string(),
                });
            }
        }
        memory.increment_turn();
    }

    fn latest_user_message(messages: &[Message]) -> &str {
        messages
            .iter()
            .rposition(|message| matches!(message, Message::User { .. }))
            .and_then(|index| match &messages[index] {
                Message::User { content } => Some(content.as_str()),
                _ => None,
            })
            .unwrap_or("")
    }

    fn assistant_memory_text(final_content: &str, tool_results_text: &str) -> String {
        format!("{} {}", final_content, tool_results_text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latest_user_message_uses_last_user_turn() {
        let messages = vec![
            Message::system("system"),
            Message::user("first"),
            Message::assistant("assistant"),
            Message::user("second"),
        ];

        assert_eq!(
            MemorySyncController::latest_user_message(&messages),
            "second"
        );
    }

    #[test]
    fn assistant_memory_text_preserves_existing_join_shape() {
        assert_eq!(
            MemorySyncController::assistant_memory_text("final", "tools"),
            "final tools"
        );
    }

    #[tokio::test]
    async fn sync_turn_records_heuristic_memory_and_advances_turn() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let memory_manager = Arc::new(Mutex::new(MemoryManager::with_base_dir(
            tmp.path().to_path_buf(),
        )));
        let trace = TraceCollector::new(crate::engine::trace::TurnTrace::new(
            "session".to_string(),
            1,
            "test",
        ));
        let messages = vec![Message::user("remember this preference")];

        MemorySyncController::sync_turn(MemorySyncContext {
            memory_manager: Some(&memory_manager),
            llm_memory_extraction: false,
            provider: None,
            model: "test",
            trace: &trace,
            messages: &messages,
            final_content: "final",
            tool_results_text: "tools",
        })
        .await;

        let memory = memory_manager.lock().await;
        assert_eq!(memory.extraction_stats().1, 1);
        assert!(memory.has_memory_writes_since(0));
        drop(memory);

        let finished = trace.finish(crate::engine::trace::TurnStatus::Completed);
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::MemorySynced { mode } if mode == "heuristic"
        )));
    }
}
