use super::action_checkpoint::FocusedRepairActionProposal;
use super::focused_repair_recovery::{
    DisabledPatchSynthesisRecovery, DisabledPatchSynthesisRecoveryRequest,
    FocusedRepairRecoveryController, PatchSynthesisFailureRecovery,
};
use super::focused_repair_state_controller::FocusedRepairStateController;
use super::patch_recovery::{PatchSynthesisOutcome, PatchSynthesisSource};
use super::patch_synthesis_executor::{PatchSynthesisExecutionContext, PatchSynthesisExecutor};
use super::turn_runtime_state::{FocusedRepairRuntimeState, TurnRuntimeState};
use super::ConversationLoop;
use crate::engine::destructive_scope::DestructiveScopeContract;
use crate::engine::resource_policy::ResourcePolicy;
use crate::engine::streaming::StreamEvent;
use crate::engine::trace::{TraceCollector, TraceEvent};
use crate::services::api::{Message, ToolCall};
use std::collections::HashSet;
use std::path::PathBuf;
use tokio::sync::mpsc;

pub(super) struct PatchSynthesisCallExecutionContext<'a> {
    pub(super) conversation: &'a ConversationLoop,
    pub(super) tool_calls: Vec<ToolCall>,
    pub(super) assistant_message: &'static str,
    pub(super) tx: Option<&'a mpsc::Sender<StreamEvent>>,
    pub(super) trace: &'a TraceCollector,
    pub(super) resource_policy: &'a ResourcePolicy,
    pub(super) destructive_scope: &'a DestructiveScopeContract,
    pub(super) turn_state: &'a mut TurnRuntimeState,
    pub(super) tool_results_text: &'a mut String,
    pub(super) messages: &'a mut Vec<Message>,
    pub(super) changed_files: &'a mut Vec<PathBuf>,
    pub(super) baseline_git_status_files: &'a HashSet<PathBuf>,
    pub(super) is_programming_workflow: bool,
    pub(super) mark_patch_requirement_on_success: bool,
    pub(super) final_tool_calls: &'a mut Vec<ToolCall>,
}

pub(super) struct ModelPatchSynthesisExecutionContext<'a> {
    pub(super) proposal: &'a FocusedRepairActionProposal,
    pub(super) synthesis_outcome: PatchSynthesisOutcome,
    pub(super) conversation: &'a ConversationLoop,
    pub(super) tx: Option<&'a mpsc::Sender<StreamEvent>>,
    pub(super) trace: &'a TraceCollector,
    pub(super) resource_policy: &'a ResourcePolicy,
    pub(super) destructive_scope: &'a DestructiveScopeContract,
    pub(super) turn_state: &'a mut TurnRuntimeState,
    pub(super) tool_results_text: &'a mut String,
    pub(super) messages: &'a mut Vec<Message>,
    pub(super) changed_files: &'a mut Vec<PathBuf>,
    pub(super) baseline_git_status_files: &'a HashSet<PathBuf>,
    pub(super) is_programming_workflow: bool,
    pub(super) final_tool_calls: &'a mut Vec<ToolCall>,
}

pub(super) struct DisabledPatchSynthesisContext<'a> {
    pub(super) proposal: &'a FocusedRepairActionProposal,
    pub(super) conversation: &'a ConversationLoop,
    pub(super) last_user_preview: &'a str,
    pub(super) exposed_tool_names: &'a HashSet<String>,
    pub(super) tx: Option<&'a mpsc::Sender<StreamEvent>>,
    pub(super) trace: &'a TraceCollector,
    pub(super) resource_policy: &'a ResourcePolicy,
    pub(super) destructive_scope: &'a DestructiveScopeContract,
    pub(super) turn_state: &'a mut TurnRuntimeState,
    pub(super) tool_results_text: &'a mut String,
    pub(super) messages: &'a mut Vec<Message>,
    pub(super) changed_files: &'a mut Vec<PathBuf>,
    pub(super) baseline_git_status_files: &'a HashSet<PathBuf>,
    pub(super) is_programming_workflow: bool,
    pub(super) final_content: &'a mut String,
    pub(super) final_tool_calls: &'a mut Vec<ToolCall>,
}

pub(super) struct EnterPatchSynthesisContext<'a> {
    pub(super) proposal: &'a FocusedRepairActionProposal,
    pub(super) conversation: &'a ConversationLoop,
    pub(super) code_write_tools_forbidden: bool,
    pub(super) last_user_preview: &'a str,
    pub(super) exposed_tool_names: &'a HashSet<String>,
    pub(super) any_tool_success: &'a mut bool,
    pub(super) tx: Option<&'a mpsc::Sender<StreamEvent>>,
    pub(super) trace: &'a TraceCollector,
    pub(super) resource_policy: &'a ResourcePolicy,
    pub(super) destructive_scope: &'a DestructiveScopeContract,
    pub(super) turn_state: &'a mut TurnRuntimeState,
    pub(super) tool_results_text: &'a mut String,
    pub(super) messages: &'a mut Vec<Message>,
    pub(super) changed_files: &'a mut Vec<PathBuf>,
    pub(super) baseline_git_status_files: &'a HashSet<PathBuf>,
    pub(super) is_programming_workflow: bool,
    pub(super) final_content: &'a mut String,
    pub(super) final_tool_calls: &'a mut Vec<ToolCall>,
}

pub(super) struct PatchSynthesisCallExecutionOutcome {
    pub(super) any_tool_success: bool,
    pub(super) changed_files_available: bool,
}

pub(super) struct PatchSynthesisPostExecutionContext<'a> {
    pub(super) execution: PatchSynthesisCallExecutionOutcome,
    pub(super) any_tool_success: &'a mut bool,
    pub(super) tx: Option<&'a mpsc::Sender<StreamEvent>>,
    pub(super) final_content: &'a mut String,
}

pub(super) struct DisabledPatchSynthesisRecoveryApplicationContext<'a> {
    pub(super) recovery: DisabledPatchSynthesisRecovery,
    pub(super) state: &'a mut FocusedRepairRuntimeState,
    pub(super) trace: &'a TraceCollector,
    pub(super) messages: &'a mut Vec<Message>,
    pub(super) tool_results_text: &'a mut String,
    pub(super) tx: Option<&'a mpsc::Sender<StreamEvent>>,
    pub(super) final_content: &'a mut String,
}

pub(super) struct PatchSynthesisFailureRecoveryApplicationContext<'a> {
    pub(super) recovery: PatchSynthesisFailureRecovery,
    pub(super) state: &'a mut FocusedRepairRuntimeState,
    pub(super) trace: &'a TraceCollector,
    pub(super) messages: &'a mut Vec<Message>,
    pub(super) tool_results_text: &'a mut String,
    pub(super) tx: Option<&'a mpsc::Sender<StreamEvent>>,
    pub(super) final_content: &'a mut String,
}

pub(super) struct PatchSynthesisFailureHandlingContext<'a> {
    pub(super) error_text: String,
    pub(super) state: &'a mut FocusedRepairRuntimeState,
    pub(super) trace: &'a TraceCollector,
    pub(super) messages: &'a mut Vec<Message>,
    pub(super) tool_results_text: &'a mut String,
    pub(super) tx: Option<&'a mpsc::Sender<StreamEvent>>,
    pub(super) final_content: &'a mut String,
}

pub(super) struct CodeWriteForbiddenRecoveryContext<'a> {
    pub(super) state: &'a mut FocusedRepairRuntimeState,
    pub(super) trace: &'a TraceCollector,
    pub(super) messages: &'a mut Vec<Message>,
    pub(super) tool_results_text: &'a mut String,
}

pub(super) struct PatchSynthesisProposalContext<'a> {
    pub(super) proposal: &'a FocusedRepairActionProposal,
    pub(super) state: &'a mut FocusedRepairRuntimeState,
    pub(super) trace: &'a TraceCollector,
    pub(super) messages: &'a mut Vec<Message>,
    pub(super) tool_results_text: &'a mut String,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum PatchSynthesisProposalFlow {
    Continue,
    EnterPatchSynthesis,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum PatchSynthesisRecoveryFlow {
    Continue,
    Stop,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum PatchSynthesisPostExecutionFlow {
    Proceed,
    Stop,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum DisabledPatchSynthesisFlow {
    Continue,
    Stop,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum EnterPatchSynthesisFlow {
    Continue,
    Stop,
    Proceed,
}

pub(super) struct PatchSynthesisFlowController;

impl PatchSynthesisFlowController {
    pub(super) fn deterministic_seed(last_user_preview: &str, evidence: &str) -> String {
        if last_user_preview.trim().is_empty() {
            evidence.to_string()
        } else if evidence.trim().is_empty() {
            format!("TASK:\n{}", last_user_preview)
        } else {
            format!("TASK:\n{}\n\nEVIDENCE:\n{}", last_user_preview, evidence)
        }
    }

    pub(super) fn assistant_message_for_source(source: PatchSynthesisSource) -> &'static str {
        match source {
            PatchSynthesisSource::DeterministicFallback => {
                "Applying deterministic patch fallback from prior evidence."
            }
            PatchSynthesisSource::ModelJson | PatchSynthesisSource::ModelToolFallback => {
                "Applying synthesized patch from prior evidence."
            }
        }
    }

    pub(super) fn apply_repair_proposal(
        context: PatchSynthesisProposalContext<'_>,
    ) -> PatchSynthesisProposalFlow {
        context.state.action_checkpoint_no_change_rounds = context.proposal.next_no_change_rounds;
        if context.proposal.enter_patch_synthesis {
            context.trace.record(TraceEvent::WorkflowFallback {
                error: context.proposal.trace_error.clone(),
            });
            return PatchSynthesisProposalFlow::EnterPatchSynthesis;
        }

        FocusedRepairRecoveryController::append_system_prompt(
            &mut *context.messages,
            &mut *context.tool_results_text,
            context.proposal.reminder.clone(),
        );
        PatchSynthesisProposalFlow::Continue
    }

    pub(super) async fn execute_calls(
        context: PatchSynthesisCallExecutionContext<'_>,
    ) -> PatchSynthesisCallExecutionOutcome {
        context.messages.push(Message::assistant_with_tools(
            context.assistant_message,
            context.tool_calls.clone(),
        ));
        let execution = PatchSynthesisExecutor::execute(PatchSynthesisExecutionContext {
            conversation: context.conversation,
            tool_calls: &context.tool_calls,
            tx: context.tx,
            trace: context.trace,
            resource_policy: context.resource_policy,
            destructive_scope: context.destructive_scope,
            turn_state: &mut *context.turn_state,
            tool_results_text: &mut *context.tool_results_text,
            messages: &mut *context.messages,
            changed_files: &mut *context.changed_files,
            baseline_git_status_files: context.baseline_git_status_files,
            is_programming_workflow: context.is_programming_workflow,
            mark_patch_requirement_on_success: context.mark_patch_requirement_on_success,
        })
        .await;

        context.final_tool_calls.extend(context.tool_calls);
        let changed_files_available = !context.changed_files.is_empty();
        if changed_files_available {
            FocusedRepairStateController::record_patch_synthesis_success(
                &mut context.turn_state.focused_repair,
            );
        }

        PatchSynthesisCallExecutionOutcome {
            any_tool_success: execution.any_tool_success,
            changed_files_available,
        }
    }

    pub(super) async fn execute_model_synthesis_outcome(
        context: ModelPatchSynthesisExecutionContext<'_>,
    ) -> PatchSynthesisCallExecutionOutcome {
        let PatchSynthesisOutcome {
            tool_calls,
            source,
            fallback_reason,
        } = context.synthesis_outcome;
        let synthesis_reason = fallback_reason
            .as_deref()
            .unwrap_or(&context.proposal.fallback_reason)
            .to_string();
        context.trace.record(TraceEvent::WorkflowFallback {
            error: format!(
                "patch synthesis owner={} reason={} source={} produced {} file_edit action(s)",
                context.proposal.fallback_owner,
                synthesis_reason,
                source.label(),
                tool_calls.len()
            ),
        });
        Self::execute_calls(PatchSynthesisCallExecutionContext {
            conversation: context.conversation,
            tool_calls,
            assistant_message: Self::assistant_message_for_source(source),
            tx: context.tx,
            trace: context.trace,
            resource_policy: context.resource_policy,
            destructive_scope: context.destructive_scope,
            turn_state: context.turn_state,
            tool_results_text: context.tool_results_text,
            messages: context.messages,
            changed_files: context.changed_files,
            baseline_git_status_files: context.baseline_git_status_files,
            is_programming_workflow: context.is_programming_workflow,
            mark_patch_requirement_on_success: false,
            final_tool_calls: context.final_tool_calls,
        })
        .await
    }

    pub(super) async fn handle_disabled_patch_synthesis(
        context: DisabledPatchSynthesisContext<'_>,
    ) -> DisabledPatchSynthesisFlow {
        let deterministic_calls = if ConversationLoop::deterministic_patch_synthesis_enabled() {
            let evidence = ConversationLoop::patch_synthesis_evidence(context.messages);
            let deterministic_seed = Self::deterministic_seed(context.last_user_preview, &evidence);
            let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            context
                .conversation
                .deterministic_patch_tool_calls(&deterministic_seed, &cwd)
        } else {
            Vec::new()
        };

        if !deterministic_calls.is_empty() {
            context.trace.record(TraceEvent::WorkflowFallback {
                error: format!(
                    "deterministic patch synthesis fallback owner={} reason={} produced {} file_edit action(s)",
                    context.proposal.fallback_owner,
                    context.proposal.fallback_reason,
                    deterministic_calls.len()
                ),
            });
            let deterministic_execution = Self::execute_calls(PatchSynthesisCallExecutionContext {
                conversation: context.conversation,
                tool_calls: deterministic_calls,
                assistant_message: "Applying deterministic patch from prior evidence.",
                tx: context.tx,
                trace: context.trace,
                resource_policy: context.resource_policy,
                destructive_scope: context.destructive_scope,
                turn_state: &mut *context.turn_state,
                tool_results_text: &mut *context.tool_results_text,
                messages: &mut *context.messages,
                changed_files: &mut *context.changed_files,
                baseline_git_status_files: context.baseline_git_status_files,
                is_programming_workflow: context.is_programming_workflow,
                mark_patch_requirement_on_success: true,
                final_tool_calls: &mut *context.final_tool_calls,
            })
            .await;
            if deterministic_execution.changed_files_available {
                return DisabledPatchSynthesisFlow::Continue;
            }
        }

        context.trace.record(TraceEvent::WorkflowFallback {
            error: "patch synthesis disabled by default; returning control to model-led repair"
                .to_string(),
        });
        let recovery = FocusedRepairRecoveryController::disabled_patch_synthesis_recovery(
            DisabledPatchSynthesisRecoveryRequest {
                patch_synthesis_recovery_used: context
                    .turn_state
                    .focused_repair
                    .patch_synthesis_recovery_used,
                action_checkpoint_reopen_used: context
                    .turn_state
                    .focused_repair
                    .action_checkpoint_reopen_used,
                action_checkpoint_lookup_count: context
                    .turn_state
                    .focused_repair
                    .action_checkpoint_lookup_count,
                exposed_tool_names: context.exposed_tool_names,
            },
        );
        match Self::apply_disabled_recovery(DisabledPatchSynthesisRecoveryApplicationContext {
            recovery,
            state: &mut context.turn_state.focused_repair,
            trace: context.trace,
            messages: &mut *context.messages,
            tool_results_text: &mut *context.tool_results_text,
            tx: context.tx,
            final_content: &mut *context.final_content,
        })
        .await
        {
            PatchSynthesisRecoveryFlow::Continue => DisabledPatchSynthesisFlow::Continue,
            PatchSynthesisRecoveryFlow::Stop => DisabledPatchSynthesisFlow::Stop,
        }
    }

    pub(super) async fn handle_enter_patch_synthesis(
        context: EnterPatchSynthesisContext<'_>,
    ) -> EnterPatchSynthesisFlow {
        if context.code_write_tools_forbidden {
            Self::apply_code_write_forbidden_recovery(CodeWriteForbiddenRecoveryContext {
                state: &mut context.turn_state.focused_repair,
                trace: context.trace,
                messages: context.messages,
                tool_results_text: context.tool_results_text,
            });
            return EnterPatchSynthesisFlow::Continue;
        }

        if !ConversationLoop::patch_synthesis_enabled() {
            return match Self::handle_disabled_patch_synthesis(DisabledPatchSynthesisContext {
                proposal: context.proposal,
                conversation: context.conversation,
                last_user_preview: context.last_user_preview,
                exposed_tool_names: context.exposed_tool_names,
                tx: context.tx,
                trace: context.trace,
                resource_policy: context.resource_policy,
                destructive_scope: context.destructive_scope,
                turn_state: context.turn_state,
                tool_results_text: context.tool_results_text,
                messages: context.messages,
                changed_files: context.changed_files,
                baseline_git_status_files: context.baseline_git_status_files,
                is_programming_workflow: context.is_programming_workflow,
                final_content: context.final_content,
                final_tool_calls: context.final_tool_calls,
            })
            .await
            {
                DisabledPatchSynthesisFlow::Continue => EnterPatchSynthesisFlow::Continue,
                DisabledPatchSynthesisFlow::Stop => EnterPatchSynthesisFlow::Stop,
            };
        }

        match context
            .conversation
            .synthesize_patch_tool_calls(context.messages, context.last_user_preview)
            .await
        {
            Ok(synthesis_outcome) => {
                let synthesis_execution =
                    Self::execute_model_synthesis_outcome(ModelPatchSynthesisExecutionContext {
                        proposal: context.proposal,
                        synthesis_outcome,
                        conversation: context.conversation,
                        tx: context.tx,
                        trace: context.trace,
                        resource_policy: context.resource_policy,
                        destructive_scope: context.destructive_scope,
                        turn_state: context.turn_state,
                        tool_results_text: context.tool_results_text,
                        messages: context.messages,
                        changed_files: context.changed_files,
                        baseline_git_status_files: context.baseline_git_status_files,
                        is_programming_workflow: context.is_programming_workflow,
                        final_tool_calls: context.final_tool_calls,
                    })
                    .await;
                match Self::apply_model_execution_outcome(PatchSynthesisPostExecutionContext {
                    execution: synthesis_execution,
                    any_tool_success: context.any_tool_success,
                    tx: context.tx,
                    final_content: context.final_content,
                })
                .await
                {
                    PatchSynthesisPostExecutionFlow::Proceed => EnterPatchSynthesisFlow::Proceed,
                    PatchSynthesisPostExecutionFlow::Stop => EnterPatchSynthesisFlow::Stop,
                }
            }
            Err(err) => {
                match Self::recover_after_synthesis_failure(PatchSynthesisFailureHandlingContext {
                    error_text: err.to_string(),
                    state: &mut context.turn_state.focused_repair,
                    trace: context.trace,
                    messages: context.messages,
                    tool_results_text: context.tool_results_text,
                    tx: context.tx,
                    final_content: context.final_content,
                })
                .await
                {
                    PatchSynthesisRecoveryFlow::Continue => EnterPatchSynthesisFlow::Continue,
                    PatchSynthesisRecoveryFlow::Stop => EnterPatchSynthesisFlow::Stop,
                }
            }
        }
    }

    pub(super) async fn apply_model_execution_outcome(
        context: PatchSynthesisPostExecutionContext<'_>,
    ) -> PatchSynthesisPostExecutionFlow {
        if context.execution.any_tool_success {
            *context.any_tool_success = true;
        }
        if !context.execution.changed_files_available {
            FocusedRepairRecoveryController::stop_with_message(
                context.tx,
                &mut *context.final_content,
                FocusedRepairRecoveryController::NO_CHANGE_STOP_MESSAGE,
            )
            .await;
            return PatchSynthesisPostExecutionFlow::Stop;
        }

        PatchSynthesisPostExecutionFlow::Proceed
    }

    pub(super) fn apply_code_write_forbidden_recovery(
        context: CodeWriteForbiddenRecoveryContext<'_>,
    ) {
        context.trace.record(TraceEvent::WorkflowFallback {
            error: "patch synthesis blocked by prompt-forbidden code-write tools".to_string(),
        });
        FocusedRepairRecoveryController::append_system_prompt(
            &mut *context.messages,
            &mut *context.tool_results_text,
            FocusedRepairRecoveryController::code_write_forbidden_prompt(),
        );
        FocusedRepairStateController::record_code_write_forbidden_recovery(&mut *context.state);
    }

    pub(super) async fn apply_disabled_recovery(
        context: DisabledPatchSynthesisRecoveryApplicationContext<'_>,
    ) -> PatchSynthesisRecoveryFlow {
        match context.recovery {
            DisabledPatchSynthesisRecovery::ReturnToModel { prompt } => {
                FocusedRepairStateController::record_patch_synthesis_return_to_model(
                    &mut *context.state,
                );
                FocusedRepairRecoveryController::append_system_prompt(
                    &mut *context.messages,
                    &mut *context.tool_results_text,
                    prompt,
                );
                PatchSynthesisRecoveryFlow::Continue
            }
            DisabledPatchSynthesisRecovery::ReopenNormalTools {
                prompt,
                trace_error,
            } => {
                FocusedRepairStateController::record_patch_synthesis_reopen_normal_tools(
                    &mut *context.state,
                );
                context.trace.record(TraceEvent::WorkflowFallback {
                    error: trace_error.to_string(),
                });
                FocusedRepairRecoveryController::append_system_prompt(
                    &mut *context.messages,
                    &mut *context.tool_results_text,
                    prompt,
                );
                PatchSynthesisRecoveryFlow::Continue
            }
            DisabledPatchSynthesisRecovery::Stop { message } => {
                FocusedRepairRecoveryController::stop_with_message(
                    context.tx,
                    &mut *context.final_content,
                    message,
                )
                .await;
                PatchSynthesisRecoveryFlow::Stop
            }
        }
    }

    pub(super) async fn apply_failure_recovery(
        context: PatchSynthesisFailureRecoveryApplicationContext<'_>,
    ) -> PatchSynthesisRecoveryFlow {
        match context.recovery {
            PatchSynthesisFailureRecovery::InsufficientEvidence { prompt } => {
                FocusedRepairStateController::record_patch_synthesis_insufficient_evidence(
                    &mut *context.state,
                );
                FocusedRepairRecoveryController::append_system_prompt(
                    &mut *context.messages,
                    &mut *context.tool_results_text,
                    prompt,
                );
                PatchSynthesisRecoveryFlow::Continue
            }
            PatchSynthesisFailureRecovery::ReopenNormalTools {
                prompt,
                trace_error,
            } => {
                FocusedRepairStateController::record_patch_synthesis_reopen_normal_tools(
                    &mut *context.state,
                );
                context.trace.record(TraceEvent::WorkflowFallback {
                    error: trace_error.to_string(),
                });
                FocusedRepairRecoveryController::append_system_prompt(
                    &mut *context.messages,
                    &mut *context.tool_results_text,
                    prompt,
                );
                PatchSynthesisRecoveryFlow::Continue
            }
            PatchSynthesisFailureRecovery::Stop { message } => {
                context.trace.record(TraceEvent::StopCheckEvaluated {
                    status: "stop".to_string(),
                    reason: "model_output_invalid".to_string(),
                    stage: "Repair".to_string(),
                    terminal_status: Some("failed".to_string()),
                    action: "recover".to_string(),
                    no_code_progress_rounds: context.state.no_code_progress_rounds,
                    action_checkpoint_active: context.state.action_checkpoint_active,
                    summary: message.to_string(),
                    evidence_items: 1,
                    failure_type: Some("model_output_invalid".to_string()),
                    recovery_plan_id: None,
                    rollback_recommended: false,
                    next_action: Some(
                        "return control after bounded patch synthesis failure".to_string(),
                    ),
                });
                FocusedRepairRecoveryController::stop_with_message(
                    context.tx,
                    &mut *context.final_content,
                    message,
                )
                .await;
                PatchSynthesisRecoveryFlow::Stop
            }
        }
    }

    pub(super) async fn recover_after_synthesis_failure(
        context: PatchSynthesisFailureHandlingContext<'_>,
    ) -> PatchSynthesisRecoveryFlow {
        context.trace.record(TraceEvent::WorkflowFallback {
            error: format!("patch synthesis failed: {}", context.error_text),
        });
        let recovery = FocusedRepairRecoveryController::patch_synthesis_failure_recovery(
            &context.error_text,
            context.state.patch_synthesis_recovery_used,
            context.state.action_checkpoint_reopen_used,
        );
        Self::apply_failure_recovery(PatchSynthesisFailureRecoveryApplicationContext {
            recovery,
            state: context.state,
            trace: context.trace,
            messages: context.messages,
            tool_results_text: context.tool_results_text,
            tx: context.tx,
            final_content: context.final_content,
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::destructive_scope::DestructiveScopeContract;
    use crate::engine::intent_router::IntentRouter;
    use crate::engine::resource_policy::ResourcePolicy;
    use crate::engine::trace::TurnTrace;
    use crate::services::api::{ChatRequest, ChatResponse, LlmProvider};
    use crate::tools::ToolRegistry;
    use async_openai::types::ChatCompletionResponseStream;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    fn trace() -> TraceCollector {
        TraceCollector::new(TurnTrace::new("test", 1, "repair"))
    }

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

    fn conversation() -> ConversationLoop {
        ConversationLoop::new(
            Arc::new(MockProvider),
            Arc::new(ToolRegistry::new()),
            Arc::new(Mutex::new(crate::cost_tracker::CostTracker::new())),
            "mock-model".to_string(),
        )
    }

    #[test]
    fn deterministic_seed_uses_evidence_when_task_is_empty() {
        assert_eq!(
            PatchSynthesisFlowController::deterministic_seed("", "compile error"),
            "compile error"
        );
    }

    #[test]
    fn deterministic_seed_uses_task_when_evidence_is_empty() {
        assert_eq!(
            PatchSynthesisFlowController::deterministic_seed("fix the build", ""),
            "TASK:\nfix the build"
        );
    }

    #[test]
    fn deterministic_seed_combines_task_and_evidence() {
        assert_eq!(
            PatchSynthesisFlowController::deterministic_seed("fix the build", "compile error"),
            "TASK:\nfix the build\n\nEVIDENCE:\ncompile error"
        );
    }

    #[test]
    fn assistant_message_names_patch_source() {
        assert_eq!(
            PatchSynthesisFlowController::assistant_message_for_source(
                PatchSynthesisSource::DeterministicFallback,
            ),
            "Applying deterministic patch fallback from prior evidence."
        );
        assert_eq!(
            PatchSynthesisFlowController::assistant_message_for_source(
                PatchSynthesisSource::ModelJson,
            ),
            "Applying synthesized patch from prior evidence."
        );
        assert_eq!(
            PatchSynthesisFlowController::assistant_message_for_source(
                PatchSynthesisSource::ModelToolFallback,
            ),
            "Applying synthesized patch from prior evidence."
        );
    }

    fn proposal(enter_patch_synthesis: bool) -> FocusedRepairActionProposal {
        FocusedRepairActionProposal {
            reminder: "keep repairing".to_string(),
            next_no_change_rounds: 2,
            enter_patch_synthesis,
            trace_error: "enter patch synthesis".to_string(),
            fallback_owner: "focused_repair",
            fallback_reason: "no progress".to_string(),
        }
    }

    #[test]
    fn repair_proposal_reminder_updates_state_and_returns_to_model() {
        let trace = trace();
        let proposal = proposal(false);
        let mut state = FocusedRepairRuntimeState::default();
        let mut messages = Vec::new();
        let mut tool_results_text = String::new();

        let flow =
            PatchSynthesisFlowController::apply_repair_proposal(PatchSynthesisProposalContext {
                proposal: &proposal,
                state: &mut state,
                trace: &trace,
                messages: &mut messages,
                tool_results_text: &mut tool_results_text,
            });

        assert_eq!(flow, PatchSynthesisProposalFlow::Continue);
        assert_eq!(state.action_checkpoint_no_change_rounds, 2);
        assert_eq!(messages.len(), 1);
        assert!(tool_results_text.contains("keep repairing"));
    }

    #[test]
    fn repair_proposal_enter_records_trace() {
        let trace = trace();
        let proposal = proposal(true);
        let mut state = FocusedRepairRuntimeState::default();
        let mut messages = Vec::new();
        let mut tool_results_text = String::new();

        let flow =
            PatchSynthesisFlowController::apply_repair_proposal(PatchSynthesisProposalContext {
                proposal: &proposal,
                state: &mut state,
                trace: &trace,
                messages: &mut messages,
                tool_results_text: &mut tool_results_text,
            });

        assert_eq!(flow, PatchSynthesisProposalFlow::EnterPatchSynthesis);
        assert_eq!(state.action_checkpoint_no_change_rounds, 2);
        assert!(messages.is_empty());
        assert!(tool_results_text.is_empty());
        let finished = trace.finish(crate::engine::trace::TurnStatus::Completed);
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::WorkflowFallback { error } if error == "enter patch synthesis"
        )));
    }

    #[test]
    fn code_write_forbidden_recovery_updates_trace_prompt_and_state() {
        let trace = trace();
        let mut state = FocusedRepairRuntimeState::default();
        state.action_checkpoint_active = true;
        state.action_checkpoint_lookup_count = 2;
        state.action_checkpoint_no_change_rounds = 2;
        let mut messages = Vec::new();
        let mut tool_results_text = String::new();

        PatchSynthesisFlowController::apply_code_write_forbidden_recovery(
            CodeWriteForbiddenRecoveryContext {
                state: &mut state,
                trace: &trace,
                messages: &mut messages,
                tool_results_text: &mut tool_results_text,
            },
        );

        assert!(state.code_write_forbidden_checkpoint_sent);
        assert!(!state.action_checkpoint_active);
        assert_eq!(state.action_checkpoint_lookup_count, 0);
        assert_eq!(state.action_checkpoint_no_change_rounds, 0);
        assert_eq!(messages.len(), 1);
        assert!(tool_results_text.contains("Patch synthesis skipped"));
        let finished = trace.finish(crate::engine::trace::TurnStatus::Completed);
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::WorkflowFallback { error }
                if error == "patch synthesis blocked by prompt-forbidden code-write tools"
        )));
    }

    #[tokio::test]
    async fn enter_patch_synthesis_handles_code_write_forbidden() {
        let conversation = conversation();
        let route = IntentRouter::new().route("修改代码但不要写文件");
        let resource_policy = ResourcePolicy::from_route(&route);
        let destructive_scope = DestructiveScopeContract::from_user_request(
            "修改代码但不要写文件",
            std::path::Path::new("."),
        );
        let trace = trace();
        let proposal = proposal(true);
        let mut turn_state = TurnRuntimeState::new(true);
        turn_state.focused_repair.action_checkpoint_active = true;
        let mut tool_results_text = String::new();
        let mut messages = Vec::new();
        let mut changed_files = Vec::new();
        let baseline_git_status_files = HashSet::new();
        let exposed_tool_names = HashSet::new();
        let mut any_tool_success = false;
        let mut final_content = String::new();
        let mut final_tool_calls = Vec::new();

        let flow = PatchSynthesisFlowController::handle_enter_patch_synthesis(
            EnterPatchSynthesisContext {
                proposal: &proposal,
                conversation: &conversation,
                code_write_tools_forbidden: true,
                last_user_preview: "修改代码但不要写文件",
                exposed_tool_names: &exposed_tool_names,
                any_tool_success: &mut any_tool_success,
                tx: None,
                trace: &trace,
                resource_policy: &resource_policy,
                destructive_scope: &destructive_scope,
                turn_state: &mut turn_state,
                tool_results_text: &mut tool_results_text,
                messages: &mut messages,
                changed_files: &mut changed_files,
                baseline_git_status_files: &baseline_git_status_files,
                is_programming_workflow: true,
                final_content: &mut final_content,
                final_tool_calls: &mut final_tool_calls,
            },
        )
        .await;

        assert_eq!(flow, EnterPatchSynthesisFlow::Continue);
        assert!(
            turn_state
                .focused_repair
                .code_write_forbidden_checkpoint_sent
        );
        assert_eq!(messages.len(), 1);
        assert!(tool_results_text.contains("Patch synthesis skipped"));
        assert!(!any_tool_success);
        assert!(changed_files.is_empty());
        assert!(final_content.is_empty());
        assert!(final_tool_calls.is_empty());
        let finished = trace.finish(crate::engine::trace::TurnStatus::Completed);
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::WorkflowFallback { error }
                if error == "patch synthesis blocked by prompt-forbidden code-write tools"
        )));
    }

    #[tokio::test]
    async fn disabled_return_to_model_recovery_updates_state_and_prompt() {
        let trace = trace();
        let mut state = FocusedRepairRuntimeState::default();
        let mut messages = Vec::new();
        let mut tool_results_text = String::new();
        let mut final_content = String::new();

        let flow = PatchSynthesisFlowController::apply_disabled_recovery(
            DisabledPatchSynthesisRecoveryApplicationContext {
                recovery: DisabledPatchSynthesisRecovery::ReturnToModel {
                    prompt: "return to model".to_string(),
                },
                state: &mut state,
                trace: &trace,
                messages: &mut messages,
                tool_results_text: &mut tool_results_text,
                tx: None,
                final_content: &mut final_content,
            },
        )
        .await;

        assert_eq!(flow, PatchSynthesisRecoveryFlow::Continue);
        assert!(state.patch_synthesis_recovery_used);
        assert_eq!(messages.len(), 1);
        assert!(tool_results_text.contains("return to model"));
        assert!(final_content.is_empty());
    }

    #[tokio::test]
    async fn failure_reopen_recovery_updates_state_and_prompt() {
        let trace = trace();
        let mut state = FocusedRepairRuntimeState::default();
        let mut messages = Vec::new();
        let mut tool_results_text = String::new();
        let mut final_content = String::new();

        let flow = PatchSynthesisFlowController::apply_failure_recovery(
            PatchSynthesisFailureRecoveryApplicationContext {
                recovery: PatchSynthesisFailureRecovery::ReopenNormalTools {
                    prompt: "reopen tools".to_string(),
                    trace_error: "patch synthesis failed; reopening normal code-change tools once",
                },
                state: &mut state,
                trace: &trace,
                messages: &mut messages,
                tool_results_text: &mut tool_results_text,
                tx: None,
                final_content: &mut final_content,
            },
        )
        .await;

        assert_eq!(flow, PatchSynthesisRecoveryFlow::Continue);
        assert!(state.action_checkpoint_reopen_used);
        assert!(!state.action_checkpoint_active);
        assert_eq!(state.no_code_progress_rounds, 1);
        assert_eq!(messages.len(), 1);
        assert!(tool_results_text.contains("reopen tools"));
    }

    #[tokio::test]
    async fn stop_recovery_sets_final_content() {
        let trace = trace();
        let mut state = FocusedRepairRuntimeState::default();
        let mut messages = Vec::new();
        let mut tool_results_text = String::new();
        let mut final_content = String::new();

        let flow = PatchSynthesisFlowController::apply_failure_recovery(
            PatchSynthesisFailureRecoveryApplicationContext {
                recovery: PatchSynthesisFailureRecovery::Stop {
                    message: FocusedRepairRecoveryController::FAILURE_STOP_MESSAGE,
                },
                state: &mut state,
                trace: &trace,
                messages: &mut messages,
                tool_results_text: &mut tool_results_text,
                tx: None,
                final_content: &mut final_content,
            },
        )
        .await;

        assert_eq!(flow, PatchSynthesisRecoveryFlow::Stop);
        assert_eq!(
            final_content,
            FocusedRepairRecoveryController::FAILURE_STOP_MESSAGE
        );
        assert!(messages.is_empty());
        assert!(tool_results_text.is_empty());
    }

    #[tokio::test]
    async fn failure_handling_records_trace_and_applies_recovery() {
        let trace = trace();
        let mut state = FocusedRepairRuntimeState::default();
        let mut messages = Vec::new();
        let mut tool_results_text = String::new();
        let mut final_content = String::new();

        let flow = PatchSynthesisFlowController::recover_after_synthesis_failure(
            PatchSynthesisFailureHandlingContext {
                error_text: "not enough evidence for an edit".to_string(),
                state: &mut state,
                trace: &trace,
                messages: &mut messages,
                tool_results_text: &mut tool_results_text,
                tx: None,
                final_content: &mut final_content,
            },
        )
        .await;

        assert_eq!(flow, PatchSynthesisRecoveryFlow::Continue);
        assert!(state.patch_synthesis_recovery_used);
        assert_eq!(messages.len(), 1);
        assert!(tool_results_text.contains("Patch synthesis declined"));
        assert!(final_content.is_empty());
        let finished = trace.finish(crate::engine::trace::TurnStatus::Completed);
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::WorkflowFallback { error }
                if error == "patch synthesis failed: not enough evidence for an edit"
        )));
    }

    #[tokio::test]
    async fn model_execution_outcome_records_success_and_proceeds_on_change() {
        let mut any_tool_success = false;
        let mut final_content = String::new();

        let flow = PatchSynthesisFlowController::apply_model_execution_outcome(
            PatchSynthesisPostExecutionContext {
                execution: PatchSynthesisCallExecutionOutcome {
                    any_tool_success: true,
                    changed_files_available: true,
                },
                any_tool_success: &mut any_tool_success,
                tx: None,
                final_content: &mut final_content,
            },
        )
        .await;

        assert_eq!(flow, PatchSynthesisPostExecutionFlow::Proceed);
        assert!(any_tool_success);
        assert!(final_content.is_empty());
    }

    #[tokio::test]
    async fn model_execution_outcome_stops_without_changed_files() {
        let mut any_tool_success = false;
        let mut final_content = String::new();

        let flow = PatchSynthesisFlowController::apply_model_execution_outcome(
            PatchSynthesisPostExecutionContext {
                execution: PatchSynthesisCallExecutionOutcome {
                    any_tool_success: true,
                    changed_files_available: false,
                },
                any_tool_success: &mut any_tool_success,
                tx: None,
                final_content: &mut final_content,
            },
        )
        .await;

        assert_eq!(flow, PatchSynthesisPostExecutionFlow::Stop);
        assert!(any_tool_success);
        assert_eq!(
            final_content,
            FocusedRepairRecoveryController::NO_CHANGE_STOP_MESSAGE
        );
    }
}
