use crate::engine::trace::{TraceCollector, TraceEvent};
use crate::memory::manager::MemoryWriteOutcomeStatus;
use crate::memory::{MemoryManager, MemoryWriteTarget};
use crate::services::api::{LlmProvider, Message};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::debug;

pub(super) struct MemorySyncContext<'a> {
    pub(super) memory_manager: Option<&'a Arc<Mutex<MemoryManager>>>,
    pub(super) llm_memory_extraction: bool,
    pub(super) provider: Option<Arc<dyn LlmProvider>>,
    pub(super) model: &'a str,
    pub(super) trace: &'a TraceCollector,
    pub(super) messages: &'a [Message],
    pub(super) final_content: &'a str,
    pub(super) tool_results_text: &'a str,
}

pub(super) struct MemorySyncController;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AutoMemoryWritePolicy {
    ReviewOnly,
    Narrow,
    Legacy,
}

impl AutoMemoryWritePolicy {
    fn from_env() -> Self {
        match std::env::var("PRIORITY_AGENT_AUTO_MEMORY_WRITE")
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "legacy" | "unsafe" | "all" | "1" | "true" | "on" => Self::Legacy,
            "narrow" | "verified" | "explicit" => Self::Narrow,
            _ => Self::ReviewOnly,
        }
    }

    fn status(self, llm_memory_extraction: bool) -> String {
        match self {
            Self::ReviewOnly => "review_only_default".to_string(),
            Self::Narrow => "narrow_auto_write_enabled".to_string(),
            Self::Legacy if llm_memory_extraction => "legacy_llm_sync_enabled".to_string(),
            Self::Legacy => "legacy_heuristic_sync_enabled".to_string(),
        }
    }

    fn reason(self) -> &'static str {
        match self {
            Self::ReviewOnly => {
                "default memory boundary is review-only; closeout writes MemoryProposal/progress evidence but does not persist long-term memory"
            }
            Self::Narrow => {
                "narrow auto-write is enabled; only explicit user preference statements may persist automatically"
            }
            Self::Legacy => {
                "legacy automatic memory sync is explicitly enabled by PRIORITY_AGENT_AUTO_MEMORY_WRITE"
            }
        }
    }
}

impl MemorySyncController {
    pub(super) async fn sync_turn(context: MemorySyncContext<'_>) {
        let Some(memory_manager) = context.memory_manager else {
            context.trace.record(TraceEvent::MemoryBoundaryEvaluated {
                read_status: "not_applicable".to_string(),
                stale_conflict_demotion_status: "not_applicable".to_string(),
                closeout_write_candidate_status: "skipped_no_memory_manager".to_string(),
                reason: "no memory manager configured for closeout sync".to_string(),
            });
            return;
        };
        let mut memory = memory_manager.lock().await;
        let user_msg = Self::latest_user_message(context.messages);
        let policy = AutoMemoryWritePolicy::from_env();
        if !user_msg.is_empty() {
            let assistant_text =
                Self::assistant_memory_text(context.final_content, context.tool_results_text);
            context.trace.record(TraceEvent::MemoryBoundaryEvaluated {
                read_status: "not_applicable".to_string(),
                stale_conflict_demotion_status: "not_applicable".to_string(),
                closeout_write_candidate_status: policy.status(context.llm_memory_extraction),
                reason: policy.reason().to_string(),
            });

            match policy {
                AutoMemoryWritePolicy::ReviewOnly => {}
                AutoMemoryWritePolicy::Narrow => {
                    Self::sync_narrow_turn(&mut memory, user_msg, context.trace);
                }
                AutoMemoryWritePolicy::Legacy => {
                    Self::sync_legacy_turn(&mut memory, user_msg, &assistant_text, &context).await;
                }
            }
        }
        memory.increment_turn();

        // ── Nudge check: trigger background review if LLM hasn't used memory tools ──
        let had_memory_tool = Self::turn_used_memory_tool(context.messages);
        let should_review = memory.advance_nudge(had_memory_tool);
        if should_review {
            debug!("Memory nudge triggered background review ({} turns without memory tool)",
                memory.nudge_interval());
            let user_msg = Self::latest_user_message(context.messages).to_string();
            let assistant_text =
                Self::assistant_memory_text(context.final_content, context.tool_results_text);
            if let Some(provider) = context.provider.clone() {
                let model = context.model.to_string();
                let memory_arc = context.memory_manager
                    .expect("memory_manager is Some when should_review is true")
                    .clone();
                tokio::spawn(async move {
                    let mut mem = memory_arc.lock().await;
                    mem.run_background_review(
                        &user_msg,
                        &assistant_text,
                        provider.as_ref(),
                        &model,
                    ).await;
                });
                context.trace.record(TraceEvent::MemorySynced {
                    mode: "background_review_nudge".to_string(),
                });
            }
        }

    }

    fn turn_used_memory_tool(messages: &[Message]) -> bool {
        messages.iter().any(|msg| {
            if let Message::Assistant {
                tool_calls: Some(calls),
                ..
            } = msg
            {
                calls.iter().any(|tc| {
                    tc.name == "memory_save" || tc.name == "memory_load" || tc.name == "memory_clear"
                })
            } else {
                false
            }
        })
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

    async fn sync_legacy_turn(
        memory: &mut MemoryManager,
        user_msg: &str,
        assistant_text: &str,
        context: &MemorySyncContext<'_>,
    ) {
        if context.llm_memory_extraction {
            if memory.should_extract_with_llm() {
                if memory.is_forked_mode() {
                    if let Some(provider) = context.provider.clone() {
                        memory.mark_llm_extraction_started();
                        memory.sync_turn_llm_background(
                            user_msg.to_string(),
                            assistant_text.to_string(),
                            provider,
                            context.model.to_string(),
                        );
                        context.trace.record(TraceEvent::MemorySynced {
                            mode: "llm_background_forked".to_string(),
                        });
                    } else {
                        memory
                            .sync_turn_llm(user_msg, assistant_text, None, context.model)
                            .await;
                        memory.mark_main_agent_wrote();
                        context.trace.record(TraceEvent::MemorySynced {
                            mode: "llm".to_string(),
                        });
                    }
                } else {
                    memory
                        .sync_turn_llm(
                            user_msg,
                            assistant_text,
                            context.provider.as_deref(),
                            context.model,
                        )
                        .await;
                    memory.mark_main_agent_wrote();
                    context.trace.record(TraceEvent::MemorySynced {
                        mode: "llm".to_string(),
                    });
                }
            }
        } else {
            memory.sync_turn(user_msg, assistant_text);
            memory.mark_main_agent_wrote();
            context.trace.record(TraceEvent::MemorySynced {
                mode: "heuristic".to_string(),
            });
        }
    }

    fn sync_narrow_turn(memory: &mut MemoryManager, user_msg: &str, trace: &TraceCollector) {
        let Some(content) = Self::explicit_user_preference_memory(user_msg) else {
            return;
        };
        let candidate = memory
            .candidate_from_content(&content, "preference", "memory_sync_controller.narrow")
            .explicit(true);
        let outcome = memory.submit_candidate(candidate, MemoryWriteTarget::User);
        if matches!(outcome.status, MemoryWriteOutcomeStatus::Saved) {
            memory.mark_main_agent_wrote();
        }
        trace.record(TraceEvent::MemorySynced {
            mode: format!("narrow_user_preference_{:?}", outcome.status).to_ascii_lowercase(),
        });
    }

    fn explicit_user_preference_memory(user_msg: &str) -> Option<String> {
        let trimmed = user_msg.trim();
        if trimmed.len() < 8 || trimmed.len() > 500 {
            return None;
        }
        let lower = trimmed.to_ascii_lowercase();
        let has_marker = trimmed.contains("我喜欢")
            || trimmed.contains("我更喜欢")
            || trimmed.contains("我希望")
            || trimmed.contains("我的偏好")
            || lower.contains("i prefer")
            || lower.contains("my preference");
        has_marker.then(|| format!("User preference: {trimmed}"))
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
    async fn sync_turn_defaults_to_review_only_without_memory_write() {
        let mut env = crate::test_utils::env_guard::EnvVarGuard::acquire().await;
        env.remove("PRIORITY_AGENT_AUTO_MEMORY_WRITE");
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
        assert!(!memory.has_memory_writes_since(0));
        assert_eq!(memory.pending_count(), 0);
        drop(memory);

        let finished = trace.finish(crate::engine::trace::TurnStatus::Completed);
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::MemoryBoundaryEvaluated {
                closeout_write_candidate_status,
                ..
            } if closeout_write_candidate_status == "review_only_default"
        )));
        assert!(!finished
            .events
            .iter()
            .any(|event| matches!(event, TraceEvent::MemorySynced { .. })));
    }

    #[tokio::test]
    async fn sync_turn_legacy_policy_keeps_old_heuristic_path() {
        let mut env = crate::test_utils::env_guard::EnvVarGuard::acquire().await;
        env.set("PRIORITY_AGENT_AUTO_MEMORY_WRITE", "legacy");
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

    #[tokio::test]
    async fn sync_turn_llm_path_marks_extraction_throttle_when_due() {
        let mut env = crate::test_utils::env_guard::EnvVarGuard::acquire().await;
        env.set("PRIORITY_AGENT_AUTO_MEMORY_WRITE", "legacy");
        let tmp = tempfile::tempdir().expect("tempdir");
        let mut memory = MemoryManager::with_base_dir(tmp.path().to_path_buf());
        for _ in 0..MemoryManager::llm_extraction_interval() {
            memory.increment_turn();
        }
        let memory_manager = Arc::new(Mutex::new(memory));
        let trace = TraceCollector::new(crate::engine::trace::TurnTrace::new(
            "session".to_string(),
            1,
            "test",
        ));
        let messages = vec![Message::user("remember this preference")];

        MemorySyncController::sync_turn(MemorySyncContext {
            memory_manager: Some(&memory_manager),
            llm_memory_extraction: true,
            provider: None,
            model: "test",
            trace: &trace,
            messages: &messages,
            final_content: "final",
            tool_results_text: "tools",
        })
        .await;

        let memory = memory_manager.lock().await;
        let (llm_count, turns, last_llm_turn) = memory.extraction_stats();
        assert_eq!(llm_count, 1);
        assert_eq!(turns, MemoryManager::llm_extraction_interval() + 1);
        assert_eq!(last_llm_turn, MemoryManager::llm_extraction_interval());
        assert!(memory.has_memory_writes_since(0));
        drop(memory);

        let finished = trace.finish(crate::engine::trace::TurnStatus::Completed);
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::MemorySynced { mode } if mode == "llm"
        )));
    }

    #[tokio::test]
    async fn sync_turn_narrow_policy_only_persists_explicit_user_preference() {
        let mut env = crate::test_utils::env_guard::EnvVarGuard::acquire().await;
        env.set("PRIORITY_AGENT_AUTO_MEMORY_WRITE", "narrow");
        let tmp = tempfile::tempdir().expect("tempdir");
        let memory_manager = Arc::new(Mutex::new(MemoryManager::with_base_dir(
            tmp.path().to_path_buf(),
        )));
        let trace = TraceCollector::new(crate::engine::trace::TurnTrace::new(
            "session".to_string(),
            1,
            "test",
        ));
        let messages = vec![Message::user("I prefer concise Chinese progress updates.")];

        MemorySyncController::sync_turn(MemorySyncContext {
            memory_manager: Some(&memory_manager),
            llm_memory_extraction: true,
            provider: None,
            model: "test",
            trace: &trace,
            messages: &messages,
            final_content: "final",
            tool_results_text: "tools",
        })
        .await;

        let memory = memory_manager.lock().await;
        assert!(memory.has_memory_writes_since(0));
        drop(memory);

        let finished = trace.finish(crate::engine::trace::TurnStatus::Completed);
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::MemorySynced { mode } if mode == "narrow_user_preference_saved"
        )));
    }
}
