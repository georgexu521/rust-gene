use super::tool_batch_result_processor::ToolBatchProcessingOutcome;
use super::tool_round_controller::{ToolRoundContext, ToolRoundController};
use super::turn_state::{TurnLoopState, TurnRuntimeContext, TurnRuntimeState};
use super::ConversationLoop;
use crate::engine::task_context::TaskContextBundle;
use crate::services::api::{Message, ToolCall};
use crate::tools::ToolResult;
use std::collections::HashMap;
use std::path::PathBuf;

pub(super) struct TurnToolRoundState {
    pub(super) tool_results_text: String,
    pub(super) changed_files: Vec<PathBuf>,
    pub(super) batch_has_unsuccessful_tools: bool,
    pub(super) used_write_tool: bool,
    pub(super) successful_write_tool: bool,
    pub(super) used_action_checkpoint_lookup: bool,
    pub(super) any_tool_success: bool,
    pub(super) repeated_failed_tools: Vec<String>,
    pub(super) failed_tool_names_this_round: Vec<String>,
    pub(super) failed_tool_evidence: Vec<String>,
    pub(super) file_edit_failure_correction_added: bool,
    pub(super) successful_validation_commands: Vec<String>,
    pub(super) duplicate_successful_read_only_tools: Vec<String>,
    pub(super) should_closeout_after_verified_change: bool,
}

impl TurnToolRoundState {
    pub(super) fn has_worktree_changes(&self) -> bool {
        !self.changed_files.is_empty()
    }

    pub(super) fn has_successful_validation_commands(&self) -> bool {
        !self.successful_validation_commands.is_empty()
    }

    pub(super) fn failed_tool_evidence_present(&self) -> bool {
        !self.failed_tool_evidence.is_empty()
    }
}

struct TurnToolRoundOutcomeController;

impl TurnToolRoundOutcomeController {
    fn from_batch(outcome: ToolBatchProcessingOutcome) -> TurnToolRoundState {
        TurnToolRoundState {
            tool_results_text: outcome.tool_results_text,
            changed_files: outcome.changed_files,
            batch_has_unsuccessful_tools: outcome.batch_has_unsuccessful_tools,
            used_write_tool: outcome.used_write_tool,
            successful_write_tool: outcome.successful_write_tool,
            used_action_checkpoint_lookup: outcome.used_action_checkpoint_lookup,
            any_tool_success: outcome.any_tool_success,
            repeated_failed_tools: outcome.repeated_failed_tools,
            failed_tool_names_this_round: outcome.failed_tool_names_this_round,
            failed_tool_evidence: outcome.failed_tool_evidence,
            file_edit_failure_correction_added: outcome.file_edit_failure_correction_added,
            successful_validation_commands: outcome.successful_validation_commands,
            duplicate_successful_read_only_tools: outcome.duplicate_successful_read_only_tools,
            should_closeout_after_verified_change: false,
        }
    }
}

pub(super) struct TurnToolRoundStepContext<'a> {
    pub(super) conversation: &'a ConversationLoop,
    pub(super) content: &'a str,
    pub(super) tool_calls: &'a [ToolCall],
    pub(super) pre_executed: HashMap<usize, ToolResult>,
    pub(super) runtime: TurnRuntimeContext<'a>,
    pub(super) turn_state: &'a mut TurnRuntimeState,
    pub(super) task_bundle: &'a mut TaskContextBundle,
    pub(super) messages: &'a mut Vec<Message>,
    pub(super) is_programming_workflow: bool,
    pub(super) loop_state: &'a mut TurnLoopState,
}

pub(super) struct TurnToolRoundStepController;

impl TurnToolRoundStepController {
    pub(super) async fn run(context: TurnToolRoundStepContext<'_>) -> TurnToolRoundState {
        let batch_processing = ToolRoundController::execute(ToolRoundContext {
            conversation: context.conversation,
            content: context.content,
            tool_calls: context.tool_calls,
            pre_executed: context.pre_executed,
            runtime: context.runtime,
            turn_state: context.turn_state,
            task_bundle: context.task_bundle,
            messages: context.messages,
            is_programming_workflow: context.is_programming_workflow,
            companion_context_keys: &mut context.loop_state.companion_context_keys,
            failed_tool_fingerprints: &mut context.loop_state.failed_tool_fingerprints,
            failed_tool_names: &mut context.loop_state.failed_tool_names,
            successful_required_validation_commands: &mut context
                .loop_state
                .successful_required_validation_commands,
        })
        .await;
        context
            .loop_state
            .record_executed_tool_calls(context.tool_calls);

        TurnToolRoundOutcomeController::from_batch(batch_processing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::destructive_scope::DestructiveScopeContract;
    use crate::engine::intent_router::IntentRouter;
    use crate::engine::resource_policy::ResourcePolicy;
    use crate::engine::trace::{TraceCollector, TurnTrace};
    use crate::services::api::{ChatRequest, ChatResponse, LlmProvider};
    use crate::tools::ToolRegistry;
    use async_openai::types::ChatCompletionResponseStream;
    use std::collections::HashSet;
    use std::path::Path;
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

    #[tokio::test]
    async fn empty_round_returns_empty_round_state() {
        let conversation = conversation();
        let route = IntentRouter::new().route("hello");
        let resource_policy = ResourcePolicy::from_route(&route);
        let destructive_scope =
            DestructiveScopeContract::from_user_request("hello", Path::new("."));
        let trace = TraceCollector::new(TurnTrace::new("session", 1, "hello"));
        let mut turn_state = TurnRuntimeState::new(true);
        let mut task_bundle = TaskContextBundle::new("hello", Path::new("."), route.clone(), None);
        let mut loop_state = TurnLoopState::default();
        let mut messages = vec![Message::user("hello")];
        let exposed_tool_names = HashSet::new();
        let baseline_git_status_files = HashSet::new();
        let retained_context = crate::tools::ToolContextRetainedContext::default();

        let round_state = TurnToolRoundStepController::run(TurnToolRoundStepContext {
            conversation: &conversation,
            content: "done",
            tool_calls: &[],
            pre_executed: HashMap::new(),
            runtime: TurnRuntimeContext {
                tx: None,
                trace: &trace,
                route: &route,
                resource_policy: &resource_policy,
                task_stage: crate::engine::task_context::AgentTaskStage::Understand,
                exposed_tool_names: &exposed_tool_names,
                working_dir: Path::new("."),
                last_user_preview: "hello",
                required_validation_commands: &[],
                destructive_scope: &destructive_scope,
                baseline_git_status_files: &baseline_git_status_files,
                retained_context: &retained_context,
            },
            turn_state: &mut turn_state,
            task_bundle: &mut task_bundle,
            messages: &mut messages,
            is_programming_workflow: false,
            loop_state: &mut loop_state,
        })
        .await;

        assert!(round_state.tool_results_text.is_empty());
        assert!(!round_state.any_tool_success);
        assert!(!round_state.batch_has_unsuccessful_tools);
        assert!(!round_state.should_closeout_after_verified_change);
        assert_eq!(messages.len(), 2);
        assert!(matches!(
            messages.last(),
            Some(Message::Assistant {
                content,
                tool_calls: Some(tool_calls),
            }) if content == "done" && tool_calls.is_empty()
        ));
    }

    #[test]
    fn from_batch_preserves_round_outcome_and_starts_closeout_false() {
        let state = TurnToolRoundOutcomeController::from_batch(ToolBatchProcessingOutcome {
            tool_results_text: "result text".to_string(),
            changed_files: vec![PathBuf::from("src/lib.rs")],
            batch_has_unsuccessful_tools: true,
            used_write_tool: true,
            successful_write_tool: true,
            used_action_checkpoint_lookup: false,
            any_tool_success: true,
            repeated_failed_tools: vec!["bash".to_string()],
            failed_tool_names_this_round: vec!["bash".to_string()],
            failed_tool_evidence: vec!["bash failed".to_string()],
            file_edit_failure_correction_added: false,
            successful_validation_commands: vec!["cargo test -q".to_string()],
            duplicate_successful_read_only_tools: vec!["file_read".to_string()],
        });

        assert_eq!(state.tool_results_text, "result text");
        assert!(state.has_worktree_changes());
        assert!(state.has_successful_validation_commands());
        assert!(state.failed_tool_evidence_present());
        assert!(state.batch_has_unsuccessful_tools);
        assert!(state.used_write_tool);
        assert!(state.successful_write_tool);
        assert!(state.any_tool_success);
        assert!(!state.used_action_checkpoint_lookup);
        assert_eq!(state.repeated_failed_tools, vec!["bash".to_string()]);
        assert_eq!(state.failed_tool_names_this_round, vec!["bash".to_string()]);
        assert_eq!(
            state.successful_validation_commands,
            vec!["cargo test -q".to_string()]
        );
        assert_eq!(
            state.duplicate_successful_read_only_tools,
            vec!["file_read".to_string()]
        );
        assert!(!state.should_closeout_after_verified_change);
    }
}
