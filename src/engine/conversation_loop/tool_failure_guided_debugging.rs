//! Conversation-loop controller module.
//!
//! Owns one focused stage of turn execution so permissions, validation, repair, and closeout stay explicit in the runtime.

use super::risk_signal_controller::{RiskSignalController, RuntimeRiskSignalInput};
use super::turn_tool_round_step_controller::TurnToolRoundState;
use super::workflow_runtime::{persist_workflow_learning_event, workflow_contract_enabled};
use super::workflow_trace::apply_workflow_feedback_and_trace;
use crate::engine::task_context::TaskContextBundle;
use crate::engine::trace::{TraceCollector, TraceEvent};
use crate::engine::workflow_contract::{
    GuidedDebuggingPrompt, WeightFeedbackEvent, WeightFeedbackKind, WeightFeedbackSeverity,
    WorkflowContractAnalyzer,
};
use crate::services::api::{LlmProvider, Message};
use crate::session_store::SessionStore;
use std::sync::Arc;
use tracing::warn;

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
    pub(super) messages: &'a mut Vec<Message>,
}

pub(super) enum TurnToolFailureFollowupFlow {
    Continue,
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

        TurnToolFailureFollowupFlow::Continue
    }
}

pub(super) struct GuidedToolFailureDebuggingContext<'a> {
    pub(super) provider: &'a dyn LlmProvider,
    pub(super) model: String,
    pub(super) session_store: Option<&'a Arc<SessionStore>>,
    pub(super) session_id: &'a str,
    pub(super) trace: &'a TraceCollector,
    pub(super) any_tool_success: bool,
    pub(super) last_user_preview: &'a str,
    pub(super) task_bundle: &'a mut TaskContextBundle,
    pub(super) failed_tool_names: &'a [String],
    pub(super) failed_tool_evidence: &'a [String],
    pub(super) tool_results_text: &'a mut String,
    pub(super) messages: &'a mut Vec<Message>,
}

pub(super) struct GuidedToolFailureDebuggingController;

impl GuidedToolFailureDebuggingController {
    pub(super) async fn run(context: GuidedToolFailureDebuggingContext<'_>) {
        let should_run = Self::should_run(context.any_tool_success, context.failed_tool_evidence);
        if should_run {
            if let Some(assessment) =
                RiskSignalController::assess_runtime_failure(RuntimeRiskSignalInput {
                    failed_validation_commands: &[],
                    failed_tool_evidence: context.failed_tool_evidence,
                    syntax_error: false,
                })
            {
                context.trace.record(TraceEvent::RiskSignalAssessed {
                    phase: "runtime".to_string(),
                    level: assessment.level.label().to_string(),
                    entry_contract: assessment.entry_contract,
                    reasons: assessment.reasons,
                });
            }
        }
        if !should_run || !workflow_contract_enabled(context.provider) {
            return;
        }

        let analyzer = WorkflowContractAnalyzer::new(context.provider, context.model);
        let prompt = GuidedDebuggingPrompt::new(
            context.last_user_preview,
            context
                .task_bundle
                .workflow_judgment
                .as_ref()
                .map(|judgment| judgment.to_turn_context()),
            context.failed_tool_names.to_vec(),
            context.failed_tool_evidence.to_vec(),
        );
        match analyzer.analyze_debugging(prompt).await {
            Ok(debugging) => {
                context.trace.record(TraceEvent::GuidedDebuggingCompleted {
                    blocker: debugging.blocker,
                    next_action: format!("{:?}", debugging.next_action),
                    causes: debugging.likely_causes.len(),
                    evidence_items: debugging.evidence_to_collect.len(),
                    ask_user: debugging.ask_user,
                });
                persist_workflow_learning_event(
                    context.session_store,
                    context.session_id,
                    "guided_debugging",
                    format!(
                        "Guided debugging selected {:?}: {}",
                        debugging.next_action, debugging.symptom
                    ),
                    if debugging.blocker { 0.85 } else { 0.7 },
                    serde_json::json!({
                        "blocker": debugging.blocker,
                        "symptom": debugging.symptom.clone(),
                        "likely_causes": debugging.likely_causes.clone(),
                        "evidence_to_collect": debugging.evidence_to_collect.clone(),
                        "smallest_safe_action": debugging.smallest_safe_action.clone(),
                        "ask_user": debugging.ask_user,
                        "questions": debugging.questions.clone(),
                        "next_action": format!("{:?}", debugging.next_action),
                        "failed_tools": context.failed_tool_names,
                    }),
                );
                let debugging_text = debugging.format_for_prompt();
                context.tool_results_text.push('\n');
                context.tool_results_text.push_str(&debugging_text);
                context.messages.push(Message::system(format!(
                    "<recent_observation>\n{}\n</recent_observation>",
                    debugging_text
                )));
                apply_workflow_feedback_and_trace(
                    context.task_bundle,
                    context.trace,
                    WeightFeedbackEvent {
                        kind: WeightFeedbackKind::ToolFailure,
                        severity: if debugging.blocker {
                            WeightFeedbackSeverity::High
                        } else {
                            WeightFeedbackSeverity::Medium
                        },
                        confidence: 0.85,
                        reason: Some(debugging.symptom.clone()),
                    },
                );
            }
            Err(err) => {
                warn!("Guided debugging analysis failed: {}", err);
                context.trace.record(TraceEvent::WorkflowFallback {
                    error: format!("guided debugging analysis failed: {}", err),
                });
            }
        }
    }

    fn should_run(any_tool_success: bool, failed_tool_evidence: &[String]) -> bool {
        !any_tool_success && !failed_tool_evidence.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guided_tool_failure_debugging_only_runs_after_failed_tool_evidence() {
        let evidence = vec!["bash failed: command not found".to_string()];

        assert!(GuidedToolFailureDebuggingController::should_run(
            false, &evidence
        ));
        assert!(!GuidedToolFailureDebuggingController::should_run(
            true, &evidence
        ));
        assert!(!GuidedToolFailureDebuggingController::should_run(
            false,
            &[]
        ));
    }

    struct MockProvider;

    #[async_trait::async_trait]
    impl LlmProvider for MockProvider {
        async fn chat(
            &self,
            _request: crate::services::api::ChatRequest,
        ) -> anyhow::Result<crate::services::api::ChatResponse> {
            Err(anyhow::anyhow!("chat not used in this test"))
        }

        async fn chat_stream(
            &self,
            _request: crate::services::api::ChatRequest,
        ) -> anyhow::Result<async_openai::types::ChatCompletionResponseStream> {
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
        TraceCollector::new(crate::engine::trace::TurnTrace::new(
            "session-test",
            1,
            "tool failure",
        ))
    }

    fn task_bundle() -> TaskContextBundle {
        let route = crate::engine::intent_router::IntentRouter::new().route("fix bug");
        TaskContextBundle::new("fix bug", ".", route, None)
    }

    fn round_state(any_tool_success: bool) -> TurnToolRoundState {
        use std::path::PathBuf;
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
            duplicate_successful_read_only_tools: Vec::new(),
            should_closeout_after_verified_change: false,
        }
    }

    #[tokio::test]
    async fn run_continues_after_repeated_failed_tool_without_success() {
        let provider = MockProvider;
        let trace = trace();
        let mut task_bundle = task_bundle();
        let mut round_state = round_state(false);
        round_state.repeated_failed_tools = vec!["bash".to_string()];
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
            messages: &mut messages,
        })
        .await;

        assert!(matches!(flow, TurnToolFailureFollowupFlow::Continue));
    }
}
