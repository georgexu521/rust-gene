use super::risk_signal_controller::{RiskSignalController, RuntimeRiskSignalInput};
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
                    "<recent_observation>\n{}\n</recent_observation>", debugging_text
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
}
