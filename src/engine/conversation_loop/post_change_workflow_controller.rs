use super::first_code_change_controller::{FirstCodeChangeContext, FirstCodeChangeController};
use super::post_edit_repair_controller::{
    PostEditRepairContext, PostEditRepairController, PostEditRepairRuntimeContext,
};
use super::post_edit_verification_controller::{
    PostEditVerificationContext, PostEditVerificationController,
};
use super::safe_prefix_by_bytes;
use super::turn_runtime_state::TurnRuntimeState;
use super::validation_runner::RequiredValidationController;
use super::ConversationLoop;
use crate::engine::code_change_workflow::{is_programming_workflow, CodeChangeWorkflowRunner};
use crate::engine::intent_router::IntentRoute;
use crate::engine::task_context::TaskContextBundle;
use crate::engine::trace::{TraceCollector, TraceEvent};
use crate::services::api::Message;
use std::collections::HashSet;
use std::path::PathBuf;

const NO_EFFECTIVE_DIFF_REPAIR_ROUND_LIMIT: usize = 2;

pub(super) struct PostChangeWorkflowContext<'a> {
    pub(super) conversation: &'a ConversationLoop,
    pub(super) trace: &'a TraceCollector,
    pub(super) route: &'a IntentRoute,
    pub(super) code_workflow: &'a mut CodeChangeWorkflowRunner,
    pub(super) task_bundle: &'a mut TaskContextBundle,
    pub(super) changed_files: &'a [PathBuf],
    pub(super) required_validation_commands: &'a [String],
    pub(super) successful_validation_commands: &'a [String],
    pub(super) successful_required_validation_commands: &'a mut HashSet<String>,
    pub(super) turn_state: &'a mut TurnRuntimeState,
    pub(super) should_closeout_after_verified_change: bool,
    pub(super) final_content: &'a mut String,
    pub(super) tool_results_text: &'a mut String,
    pub(super) messages: &'a mut Vec<Message>,
    pub(super) last_user_preview: &'a str,
}

pub(super) struct PostChangeWorkflowOutcome {
    pub(super) should_closeout_after_verified_change: bool,
    pub(super) break_loop: bool,
}

pub(super) struct PostChangeWorkflowController;

impl PostChangeWorkflowController {
    pub(super) async fn run(context: PostChangeWorkflowContext<'_>) -> PostChangeWorkflowOutcome {
        if context.changed_files.is_empty() {
            if !context.required_validation_commands.is_empty() {
                let working_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                let required_run = RequiredValidationController::run_pending_commands_with_trace(
                    &working_dir,
                    context.required_validation_commands,
                    context.successful_validation_commands,
                    &*context.successful_required_validation_commands,
                    Some(context.trace),
                )
                .await;
                let required_application =
                    RequiredValidationController::application_for_run(required_run);
                let required_source_context = if required_application.passed {
                    None
                } else {
                    RequiredValidationController::source_context_from_evidence(
                        &working_dir,
                        &required_application.post_edit_evidence,
                    )
                };
                for command in required_application.successful_commands {
                    context
                        .successful_required_validation_commands
                        .insert(command);
                }
                for record in required_application.ledger_records {
                    context.turn_state.evidence_ledger.record_validation_result(
                        "required_validation",
                        Some(&record.command),
                        record.success,
                        &record.dialog_text,
                    );
                }
                for text in required_application.post_edit_evidence.iter().cloned() {
                    append_system_text(context.tool_results_text, context.messages, text);
                }
                if let Some(source_context) = required_source_context {
                    append_system_text(context.tool_results_text, context.messages, source_context);
                }
                NoEffectiveDiffRepairController::apply(NoEffectiveDiffRepairContext {
                    route: context.route,
                    trace: context.trace,
                    turn_state: context.turn_state,
                    failed_commands: &required_application.failed_commands,
                    post_edit_evidence: &required_application.post_edit_evidence,
                    tool_results_text: context.tool_results_text,
                    messages: context.messages,
                });
            }
            return PostChangeWorkflowOutcome {
                should_closeout_after_verified_change: context
                    .should_closeout_after_verified_change,
                break_loop: false,
            };
        }

        FirstCodeChangeController::record(FirstCodeChangeContext {
            trace: context.trace,
            code_workflow: context.code_workflow,
            evidence_ledger: &mut context.turn_state.evidence_ledger,
            changed_files: context.changed_files,
        });

        let working_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let verification = PostEditVerificationController::run(PostEditVerificationContext {
            working_dir: &working_dir,
            changed_files: context.changed_files,
            lsp_manager: context.conversation.lsp_manager.as_deref(),
            required_validation_commands: context.required_validation_commands,
            successful_validation_commands: context.successful_validation_commands,
            successful_required_validation_commands: context
                .successful_required_validation_commands,
            trace: context.trace,
            evidence_ledger: &mut context.turn_state.evidence_ledger,
            tool_results_text: context.tool_results_text,
            messages: context.messages,
        })
        .await;

        let verification_trace = PostEditVerificationController::record_trace(
            context.trace,
            context.changed_files,
            &verification,
        );
        let should_closeout_after_verified_change =
            verification_trace.should_closeout_after_verified_change;
        let repair_tool_record_evidence = context
            .turn_state
            .evidence_ledger
            .repair_tool_record_evidence(&verification.failed_commands);

        let post_edit_repair_outcome = PostEditRepairController::run(
            context.conversation,
            PostEditRepairContext {
                trace: context.trace,
                route: context.route,
                code_workflow: context.code_workflow,
                task_bundle: context.task_bundle,
                changed_files: context.changed_files,
                verification: &verification,
                required_validation_commands: context.required_validation_commands,
                repair_tool_record_evidence,
                runtime: PostEditRepairRuntimeContext::from_turn_state(context.turn_state),
                max_iterations: context.conversation.max_iterations,
                should_closeout_after_verified_change,
                final_content: context.final_content,
                tool_results_text: context.tool_results_text,
                messages: context.messages,
                last_user_preview: context.last_user_preview,
            },
        )
        .await;

        PostChangeWorkflowOutcome {
            should_closeout_after_verified_change: post_edit_repair_outcome
                .should_closeout_after_verified_change,
            break_loop: post_edit_repair_outcome.break_loop,
        }
    }
}

fn append_system_text(tool_results_text: &mut String, messages: &mut Vec<Message>, text: String) {
    if !text.trim().is_empty() {
        if !tool_results_text.is_empty() {
            tool_results_text.push_str("\n\n");
        }
        tool_results_text.push_str(&text);
        messages.push(super::request_preparation_controller::recent_observation_message(&text));
    }
}

struct NoEffectiveDiffRepairContext<'a> {
    route: &'a IntentRoute,
    trace: &'a TraceCollector,
    turn_state: &'a mut TurnRuntimeState,
    failed_commands: &'a [String],
    post_edit_evidence: &'a [String],
    tool_results_text: &'a mut String,
    messages: &'a mut Vec<Message>,
}

struct NoEffectiveDiffRepairController;

impl NoEffectiveDiffRepairController {
    fn apply(context: NoEffectiveDiffRepairContext<'_>) {
        if !Self::should_apply(context.route, context.failed_commands) {
            return;
        }

        let focused_repair = &mut context.turn_state.focused_repair;
        focused_repair.no_effective_diff_repair_rounds += 1;
        focused_repair.action_checkpoint_active = true;
        focused_repair.action_checkpoint_requires_patch_before_validation = true;
        if focused_repair.no_effective_diff_repair_rounds >= NO_EFFECTIVE_DIFF_REPAIR_ROUND_LIMIT {
            focused_repair.action_checkpoint_no_change_rounds =
                focused_repair.action_checkpoint_no_change_rounds.max(1);
        }
        let round = focused_repair.no_effective_diff_repair_rounds;
        context.trace.record(TraceEvent::WorkflowFallback {
            error: format!(
                "no effective diff repair observation emitted after required validation failure round={round}"
            ),
        });
        append_system_text(
            context.tool_results_text,
            context.messages,
            Self::format_observation(round, context.failed_commands, context.post_edit_evidence),
        );
    }

    fn should_apply(route: &IntentRoute, failed_commands: &[String]) -> bool {
        is_programming_workflow(route.workflow) && !failed_commands.is_empty()
    }

    fn format_observation(round: usize, failed_commands: &[String], evidence: &[String]) -> String {
        let failed = failed_commands
            .iter()
            .take(6)
            .map(|command| format!("- `{}`", safe_prefix_by_bytes(command, 220)))
            .collect::<Vec<_>>()
            .join("\n");
        let evidence_preview = evidence
            .iter()
            .filter(|text| !text.trim().is_empty())
            .take(3)
            .map(|text| safe_prefix_by_bytes(text.trim(), 900).to_string())
            .collect::<Vec<_>>()
            .join("\n\n");
        let bounded_note = if round >= NO_EFFECTIVE_DIFF_REPAIR_ROUND_LIMIT {
            "\nrepair_escalation=focused_patch_required\nThe same no-effective-diff pattern has repeated; use the focused repair tools to patch from the gathered evidence before any further validation."
        } else {
            ""
        };

        format!(
            "[No-effective-diff repair observation]\nstatus=no_effective_diff\nround={round}\nreason=required validation or behavior assertions failed, but no changed files were recorded\nfailed_commands:\n{failed}\nevidence_preview:\n{evidence_preview}{bounded_note}\nrequired_next_action:\n- Inspect only the exact target if the evidence is insufficient.\n- Make an actual file_edit/file_write/file_patch change that addresses the failed assertion.\n- Rerun the failed required validation only after a file change.\n- Do not close out while the diff is empty or required validation is failing."
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::intent_router::{
        IntentKind, IntentRouter, ReasoningPolicy, RetrievalPolicy, RiskLevel, WorkflowKind,
    };
    use crate::engine::trace::TurnTrace;
    use crate::services::api::{ChatRequest, ChatResponse, LlmProvider};
    use crate::tools::ToolRegistry;
    use async_openai::types::ChatCompletionResponseStream;
    use std::sync::Arc;
    use tokio::sync::Mutex;

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

    fn code_change_route() -> IntentRoute {
        IntentRoute {
            intent: IntentKind::CodeChange,
            confidence: 0.9,
            workflow: WorkflowKind::CodeChange,
            retrieval: RetrievalPolicy::Project,
            reasoning: ReasoningPolicy::High,
            risk: RiskLevel::Medium,
            recommended_tools: Vec::new(),
            dependency_install_intent: false,
            mcp_auth_intent: false,
            reason: "test code-change route".to_string(),
        }
    }

    #[tokio::test]
    async fn skips_when_no_changed_files() {
        let conversation = conversation();
        let route = IntentRouter::new().route("say hello");
        let mut task_bundle = TaskContextBundle::new("say hello", ".", route.clone(), None);
        let mut code_workflow = CodeChangeWorkflowRunner::new(&task_bundle);
        let trace = TraceCollector::new(TurnTrace::new("session", 1, "say hello"));
        let mut turn_state = TurnRuntimeState::new(true);
        let mut successful_required_validation_commands = HashSet::new();
        let mut final_content = String::new();
        let mut tool_results_text = String::new();
        let mut messages = Vec::new();
        let changed_files = Vec::new();
        let required_validation_commands = Vec::new();
        let successful_validation_commands = Vec::new();

        let outcome = PostChangeWorkflowController::run(PostChangeWorkflowContext {
            conversation: &conversation,
            trace: &trace,
            route: &route,
            code_workflow: &mut code_workflow,
            task_bundle: &mut task_bundle,
            changed_files: &changed_files,
            required_validation_commands: &required_validation_commands,
            successful_validation_commands: &successful_validation_commands,
            successful_required_validation_commands: &mut successful_required_validation_commands,
            turn_state: &mut turn_state,
            should_closeout_after_verified_change: true,
            final_content: &mut final_content,
            tool_results_text: &mut tool_results_text,
            messages: &mut messages,
            last_user_preview: "say hello",
        })
        .await;

        assert!(outcome.should_closeout_after_verified_change);
        assert!(!outcome.break_loop);
        assert!(messages.is_empty());
        assert!(tool_results_text.is_empty());
        assert!(final_content.is_empty());
        assert!(successful_required_validation_commands.is_empty());
    }

    #[tokio::test]
    async fn runs_required_validation_even_without_changed_files() {
        let conversation = conversation();
        let route = IntentRouter::new().route("audit only");
        let mut task_bundle = TaskContextBundle::new("audit only", ".", route.clone(), None);
        let mut code_workflow = CodeChangeWorkflowRunner::new(&task_bundle);
        let trace = TraceCollector::new(TurnTrace::new("session", 1, "audit only"));
        let mut turn_state = TurnRuntimeState::new(true);
        let mut successful_required_validation_commands = HashSet::new();
        let mut final_content = String::new();
        let mut tool_results_text = String::new();
        let mut messages = Vec::new();
        let changed_files = Vec::new();
        let required_validation_commands = vec!["true".to_string()];
        let successful_validation_commands = Vec::new();

        let outcome = PostChangeWorkflowController::run(PostChangeWorkflowContext {
            conversation: &conversation,
            trace: &trace,
            route: &route,
            code_workflow: &mut code_workflow,
            task_bundle: &mut task_bundle,
            changed_files: &changed_files,
            required_validation_commands: &required_validation_commands,
            successful_validation_commands: &successful_validation_commands,
            successful_required_validation_commands: &mut successful_required_validation_commands,
            turn_state: &mut turn_state,
            should_closeout_after_verified_change: false,
            final_content: &mut final_content,
            tool_results_text: &mut tool_results_text,
            messages: &mut messages,
            last_user_preview: "audit only",
        })
        .await;

        assert!(!outcome.break_loop);
        assert!(successful_required_validation_commands.contains("true"));
        assert!(turn_state
            .evidence_ledger
            .runtime_required_validation_label(&required_validation_commands)
            .is_some_and(|label| label.contains("passed:1/1")));
    }

    #[tokio::test]
    async fn failed_required_validation_without_diff_enters_focused_repair() {
        let conversation = conversation();
        let route = code_change_route();
        let mut task_bundle = TaskContextBundle::new("fix behavior", ".", route.clone(), None);
        let mut code_workflow = CodeChangeWorkflowRunner::new(&task_bundle);
        let trace = TraceCollector::new(TurnTrace::new("session", 1, "fix behavior"));
        let mut turn_state = TurnRuntimeState::new(true);
        let mut successful_required_validation_commands = HashSet::new();
        let mut final_content = String::new();
        let mut tool_results_text = String::new();
        let mut messages = Vec::new();
        let changed_files = Vec::new();
        let required_validation_commands = vec!["false".to_string()];
        let successful_validation_commands = Vec::new();

        let outcome = PostChangeWorkflowController::run(PostChangeWorkflowContext {
            conversation: &conversation,
            trace: &trace,
            route: &route,
            code_workflow: &mut code_workflow,
            task_bundle: &mut task_bundle,
            changed_files: &changed_files,
            required_validation_commands: &required_validation_commands,
            successful_validation_commands: &successful_validation_commands,
            successful_required_validation_commands: &mut successful_required_validation_commands,
            turn_state: &mut turn_state,
            should_closeout_after_verified_change: false,
            final_content: &mut final_content,
            tool_results_text: &mut tool_results_text,
            messages: &mut messages,
            last_user_preview: "fix behavior",
        })
        .await;

        assert!(!outcome.break_loop);
        assert_eq!(turn_state.focused_repair.no_effective_diff_repair_rounds, 1);
        assert!(turn_state.focused_repair.action_checkpoint_active);
        assert!(
            turn_state
                .focused_repair
                .action_checkpoint_requires_patch_before_validation
        );
        assert_eq!(turn_state.reserved_repair_rounds, 0);
        assert!(tool_results_text.contains("status=no_effective_diff"));
        assert!(tool_results_text.contains("failed_commands:\n- `false`"));
        assert!(messages.iter().any(|message| matches!(
            message,
            Message::System { content } if content.contains("No-effective-diff repair observation")
        )));
    }

    #[test]
    fn repeated_no_effective_diff_repair_escalates_to_patch_required() {
        let route = code_change_route();
        let trace = TraceCollector::new(TurnTrace::new("session", 1, "fix behavior"));
        let mut turn_state = TurnRuntimeState::new(true);
        let failed_commands = vec!["cargo test -q behavior_contract".to_string()];
        let evidence = vec!["required command failed: cargo test -q behavior_contract".to_string()];
        let mut tool_results_text = String::new();
        let mut messages = Vec::new();

        for _ in 0..2 {
            NoEffectiveDiffRepairController::apply(NoEffectiveDiffRepairContext {
                route: &route,
                trace: &trace,
                turn_state: &mut turn_state,
                failed_commands: &failed_commands,
                post_edit_evidence: &evidence,
                tool_results_text: &mut tool_results_text,
                messages: &mut messages,
            });
        }

        assert_eq!(turn_state.focused_repair.no_effective_diff_repair_rounds, 2);
        assert_eq!(
            turn_state.focused_repair.action_checkpoint_no_change_rounds,
            1
        );
        assert!(tool_results_text.contains("repair_escalation=focused_patch_required"));
    }
}
