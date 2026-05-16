use super::focused_repair_recovery::FocusedRepairRecoveryController;
use super::tool_failure_guided_debugging::{
    GuidedToolFailureDebuggingContext, GuidedToolFailureDebuggingController,
};
use super::tool_failure_stop_controller::{ToolFailureStopController, ToolFailureStopRequest};
use super::turn_tool_round_outcome_controller::TurnToolRoundState;
use super::StreamEvent;
use crate::engine::task_context::TaskContextBundle;
use crate::engine::trace::TraceCollector;
use crate::services::api::{LlmProvider, Message};
use crate::session_store::SessionStore;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

pub(super) struct TurnToolFailureFollowupContext<'a> {
    pub(super) provider: &'a dyn LlmProvider,
    pub(super) model: String,
    pub(super) session_store: Option<&'a Arc<SessionStore>>,
    pub(super) session_id: &'a str,
    pub(super) trace: &'a TraceCollector,
    pub(super) any_tool_success: bool,
    pub(super) last_user_preview: &'a str,
    pub(super) task_bundle: &'a mut TaskContextBundle,
    pub(super) round_state: &'a mut TurnToolRoundState,
    pub(super) failed_tool_names: &'a HashMap<String, usize>,
    pub(super) tx: Option<&'a mpsc::Sender<StreamEvent>>,
    pub(super) final_content: &'a mut String,
    pub(super) messages: &'a mut Vec<Message>,
}

pub(super) enum TurnToolFailureFollowupFlow {
    Continue,
    Stop,
}

pub(super) struct TurnToolFailureFollowupController;

impl TurnToolFailureFollowupController {
    pub(super) async fn run(
        context: TurnToolFailureFollowupContext<'_>,
    ) -> TurnToolFailureFollowupFlow {
        GuidedToolFailureDebuggingController::run(GuidedToolFailureDebuggingContext {
            provider: context.provider,
            model: context.model,
            session_store: context.session_store,
            session_id: context.session_id,
            trace: context.trace,
            any_tool_success: context.any_tool_success,
            last_user_preview: context.last_user_preview,
            task_bundle: context.task_bundle,
            failed_tool_names: &context.round_state.failed_tool_names_this_round,
            failed_tool_evidence: &context.round_state.failed_tool_evidence,
            tool_results_text: &mut context.round_state.tool_results_text,
            messages: context.messages,
        })
        .await;

        if let Some(stop) = ToolFailureStopController::decide(ToolFailureStopRequest {
            any_tool_success: context.any_tool_success,
            repeated_failed_tools: &context.round_state.repeated_failed_tools,
            failed_tool_names: context.failed_tool_names,
        }) {
            FocusedRepairRecoveryController::stop_with_message(
                context.tx,
                context.final_content,
                &stop.message,
            )
            .await;
            return TurnToolFailureFollowupFlow::Stop;
        }

        TurnToolFailureFollowupFlow::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::super::turn_tool_round_outcome_controller::TurnToolRoundState;
    use super::*;
    use crate::engine::intent_router::IntentRouter;
    use crate::engine::trace::TurnTrace;
    use crate::services::api::{ChatRequest, ChatResponse};
    use async_openai::types::ChatCompletionResponseStream;
    use std::path::PathBuf;

    struct MockProvider;

    #[async_trait::async_trait]
    impl LlmProvider for MockProvider {
        async fn chat(&self, _request: ChatRequest) -> anyhow::Result<ChatResponse> {
            Err(anyhow::anyhow!("chat not used in this test"))
        }

        async fn chat_stream(
            &self,
            _request: ChatRequest,
        ) -> anyhow::Result<ChatCompletionResponseStream> {
            Err(anyhow::anyhow!("stream not used in this test"))
        }

        fn base_url(&self) -> &str {
            "mock://local"
        }

        fn default_model(&self) -> &str {
            "mock-model"
        }
    }

    fn trace() -> TraceCollector {
        TraceCollector::new(TurnTrace::new("session-test", 1, "tool failure"))
    }

    fn task_bundle() -> TaskContextBundle {
        let route = IntentRouter::new().route("fix bug");
        TaskContextBundle::new("fix bug", ".", route, None)
    }

    fn round_state(any_tool_success: bool) -> TurnToolRoundState {
        TurnToolRoundState {
            tool_results_text: String::new(),
            changed_files: Vec::<PathBuf>::new(),
            batch_has_unsuccessful_tools: !any_tool_success,
            used_write_tool: false,
            successful_write_tool: false,
            used_action_checkpoint_lookup: false,
            any_tool_success,
            repeated_failed_tools: Vec::new(),
            failed_tool_names_this_round: Vec::new(),
            failed_tool_evidence: Vec::new(),
            file_edit_failure_correction_added: false,
            successful_validation_commands: Vec::new(),
            should_closeout_after_verified_change: false,
        }
    }

    #[tokio::test]
    async fn run_stops_after_repeated_failed_tool_without_success() {
        let provider = MockProvider;
        let trace = trace();
        let mut task_bundle = task_bundle();
        let mut round_state = round_state(false);
        round_state.repeated_failed_tools = vec!["bash".to_string()];
        let failed_tool_names = HashMap::from([("bash".to_string(), 2)]);
        let mut final_content = String::new();
        let mut messages = vec![Message::user("fix bug")];

        let flow = TurnToolFailureFollowupController::run(TurnToolFailureFollowupContext {
            provider: &provider,
            model: "mock-model".to_string(),
            session_store: None,
            session_id: "session-test",
            trace: &trace,
            any_tool_success: false,
            last_user_preview: "fix bug",
            task_bundle: &mut task_bundle,
            round_state: &mut round_state,
            failed_tool_names: &failed_tool_names,
            tx: None,
            final_content: &mut final_content,
            messages: &mut messages,
        })
        .await;

        assert!(matches!(flow, TurnToolFailureFollowupFlow::Stop));
        assert_eq!(
            final_content,
            "[Stopped repeated failed tool attempts: bash]"
        );
    }

    #[tokio::test]
    async fn run_continues_when_a_tool_succeeded() {
        let provider = MockProvider;
        let trace = trace();
        let mut task_bundle = task_bundle();
        let mut round_state = round_state(true);
        round_state.repeated_failed_tools = vec!["bash".to_string()];
        let failed_tool_names = HashMap::from([("bash".to_string(), 2)]);
        let mut final_content = String::new();
        let mut messages = vec![Message::user("fix bug")];

        let flow = TurnToolFailureFollowupController::run(TurnToolFailureFollowupContext {
            provider: &provider,
            model: "mock-model".to_string(),
            session_store: None,
            session_id: "session-test",
            trace: &trace,
            any_tool_success: true,
            last_user_preview: "fix bug",
            task_bundle: &mut task_bundle,
            round_state: &mut round_state,
            failed_tool_names: &failed_tool_names,
            tx: None,
            final_content: &mut final_content,
            messages: &mut messages,
        })
        .await;

        assert!(matches!(flow, TurnToolFailureFollowupFlow::Continue));
        assert!(final_content.is_empty());
    }
}
