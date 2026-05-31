use super::tool_execution_controller::{
    ToolExecutionBatch, ToolExecutionContext, ToolExecutionController, ToolExecutionRequest,
};
use super::tool_turn_controller::{ToolTurnAppendContext, ToolTurnController};
use super::turn_runtime_state::TurnRuntimeState;
use super::workflow_change_tracker::WorkflowChangeTracker;
use super::ConversationLoop;
use crate::engine::destructive_scope::DestructiveScopeContract;
use crate::engine::resource_policy::ResourcePolicy;
use crate::engine::streaming::StreamEvent;
use crate::engine::trace::TraceCollector;
use crate::services::api::{Message, ToolCall};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

pub(super) struct PatchSynthesisExecutionContext<'a> {
    pub(super) conversation: &'a ConversationLoop,
    pub(super) tool_calls: &'a [ToolCall],
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
}

pub(super) struct PatchSynthesisExecutionOutcome {
    pub(super) any_tool_success: bool,
}

struct PatchSynthesisCollectionContext<'a> {
    tool_batch: &'a mut ToolExecutionBatch,
    turn_state: &'a mut TurnRuntimeState,
    tool_results_text: &'a mut String,
    messages: &'a mut Vec<Message>,
    changed_files: &'a mut Vec<PathBuf>,
    baseline_git_status_files: &'a HashSet<PathBuf>,
    is_programming_workflow: bool,
    mark_patch_requirement_on_success: bool,
}

pub(super) struct PatchSynthesisExecutor;

impl PatchSynthesisExecutor {
    pub(super) async fn execute(
        context: PatchSynthesisExecutionContext<'_>,
    ) -> PatchSynthesisExecutionOutcome {
        let mut context = context;
        let mut synthesized_batch = Self::execute_batch(&mut context).await;
        Self::collect_batch_results(PatchSynthesisCollectionContext {
            tool_batch: &mut synthesized_batch,
            turn_state: context.turn_state,
            tool_results_text: context.tool_results_text,
            messages: context.messages,
            changed_files: context.changed_files,
            baseline_git_status_files: context.baseline_git_status_files,
            is_programming_workflow: context.is_programming_workflow,
            mark_patch_requirement_on_success: context.mark_patch_requirement_on_success,
        })
        .await
    }

    async fn execute_batch(context: &mut PatchSynthesisExecutionContext<'_>) -> ToolExecutionBatch {
        let exposed_synth_tools =
            HashSet::from(["file_edit".to_string(), "file_write".to_string()]);
        let working_dir = context.conversation.create_tool_context().working_dir;
        Self::mark_synthesized_edit_targets_read(
            &context.conversation.session_id,
            &working_dir,
            context.tool_calls,
        );
        ToolExecutionController::new(ToolExecutionContext::from_conversation(
            context.conversation,
        ))
        .execute_tools_parallel(ToolExecutionRequest {
            tool_calls: context.tool_calls,
            parent_assistant_content: "patch synthesis",
            tx: context.tx,
            pre_executed: HashMap::new(),
            trace: Some(context.trace.clone()),
            route: None,
            resource_policy: context.resource_policy,
            exposed_tool_names: &exposed_synth_tools,
            retained_context: &crate::tools::ToolContextRetainedContext::default(),
            task_stage: crate::engine::task_context::AgentTaskStage::Repair,
            task_state: None,
            // Synthesized edits have already passed patch-synthesis
            // validation. Avoid applying the direct action-checkpoint guard
            // again, or safe recovered patches can be rejected without giving
            // the model a way to inspect and repair the arguments.
            action_checkpoint_active: false,
            action_checkpoint_lookup_count: 0,
            no_progress_rounds: context.turn_state.focused_repair.no_code_progress_rounds,
            has_changes_before_tools: false,
            destructive_scope: context.destructive_scope,
            storm_state: &mut context.turn_state.storm_state,
            lifecycle: &mut context.turn_state.tool_lifecycle,
        })
        .await
    }

    fn mark_synthesized_edit_targets_read(
        session_id: &str,
        working_dir: &Path,
        tool_calls: &[ToolCall],
    ) {
        for tool_call in tool_calls {
            if tool_call.name != "file_edit" {
                continue;
            }
            let Some(path) = tool_call.arguments["path"].as_str() else {
                continue;
            };
            let trimmed = path.trim();
            if trimmed.is_empty() {
                continue;
            }
            let raw_path = Path::new(trimmed);
            let candidate = if raw_path.is_absolute() {
                raw_path.to_path_buf()
            } else {
                working_dir.join(raw_path)
            };
            let Ok(canonical) = candidate.canonicalize() else {
                continue;
            };
            if !canonical.is_file() {
                continue;
            }
            let canonical_working_dir = working_dir
                .canonicalize()
                .unwrap_or_else(|_| working_dir.to_path_buf());
            if !canonical.starts_with(&canonical_working_dir) {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(&canonical) else {
                continue;
            };
            let modified = std::fs::metadata(&canonical)
                .and_then(|metadata| metadata.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            crate::tools::file_tool::mark_file_read_with_state(
                session_id,
                &canonical.to_string_lossy(),
                &content,
                modified,
            );
        }
    }

    async fn collect_batch_results(
        context: PatchSynthesisCollectionContext<'_>,
    ) -> PatchSynthesisExecutionOutcome {
        let mut any_tool_success = false;
        for (tc, result) in context.tool_batch.results_mut().iter_mut() {
            ToolTurnController::append_tool_result(
                tc,
                result,
                ToolTurnAppendContext {
                    evidence_ledger: &mut context.turn_state.evidence_ledger,
                    runtime_diet: &mut context.turn_state.runtime_diet,
                    tool_results_text: context.tool_results_text,
                    messages: context.messages,
                },
            )
            .await;
            if result.success {
                any_tool_success = true;
            }
            if result.success && ConversationLoop::is_code_write_tool_name(&tc.name) {
                if context.mark_patch_requirement_on_success {
                    context
                        .turn_state
                        .focused_repair
                        .action_checkpoint_requires_patch_before_validation = false;
                }
                if let Some(path) = tc.arguments["path"].as_str() {
                    context.changed_files.push(PathBuf::from(path));
                }
            }
        }

        if context.is_programming_workflow {
            WorkflowChangeTracker::append_changed_files_since(
                context.changed_files,
                context.baseline_git_status_files,
            );
        }

        PatchSynthesisExecutionOutcome { any_tool_success }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::ToolResult;

    fn file_write_call(path: &str) -> ToolCall {
        ToolCall {
            id: "call_1".to_string(),
            name: "file_write".to_string(),
            arguments: serde_json::json!({"path": path, "content": "updated"}),
        }
    }

    fn file_edit_call(path: &str) -> ToolCall {
        ToolCall {
            id: "call_1".to_string(),
            name: "file_edit".to_string(),
            arguments: serde_json::json!({
                "path": path,
                "old_string": "old",
                "new_string": "new",
                "expected_replacements": 1
            }),
        }
    }

    #[test]
    fn marks_synthesized_file_edit_targets_as_read() {
        let tmp = tempfile::tempdir().expect("create tempdir");
        let session_id = format!("patch-synthesis-read-{}", uuid::Uuid::new_v4().simple());
        let file_path = tmp.path().join("src/lib.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).expect("create src dir");
        std::fs::write(&file_path, "old\n").expect("write file");
        let call = file_edit_call("src/lib.rs");

        PatchSynthesisExecutor::mark_synthesized_edit_targets_read(
            &session_id,
            tmp.path(),
            &[call],
        );

        let canonical = file_path.canonicalize().expect("canonical file");
        assert!(crate::tools::file_tool::is_file_read(
            &session_id,
            &canonical.to_string_lossy()
        ));
    }

    #[tokio::test]
    async fn collection_records_successful_synthesized_write() {
        let call = file_write_call("src/lib.rs");
        let mut batch =
            ToolExecutionBatch::new(vec![(call, ToolResult::success("wrote file"))], Vec::new());
        let mut turn_state = TurnRuntimeState::new(true);
        let mut tool_results_text = String::new();
        let mut messages = Vec::new();
        let mut changed_files = Vec::new();
        let baseline = HashSet::new();
        turn_state
            .focused_repair
            .action_checkpoint_requires_patch_before_validation = true;

        let outcome =
            PatchSynthesisExecutor::collect_batch_results(PatchSynthesisCollectionContext {
                tool_batch: &mut batch,
                turn_state: &mut turn_state,
                tool_results_text: &mut tool_results_text,
                messages: &mut messages,
                changed_files: &mut changed_files,
                baseline_git_status_files: &baseline,
                is_programming_workflow: false,
                mark_patch_requirement_on_success: true,
            })
            .await;

        assert!(outcome.any_tool_success);
        assert_eq!(changed_files, vec![PathBuf::from("src/lib.rs")]);
        assert!(
            !turn_state
                .focused_repair
                .action_checkpoint_requires_patch_before_validation
        );
        assert_eq!(messages.len(), 1);
        assert!(tool_results_text.contains("wrote file"));
    }

    #[tokio::test]
    async fn collection_can_preserve_patch_requirement_flag() {
        let call = file_write_call("src/lib.rs");
        let mut batch =
            ToolExecutionBatch::new(vec![(call, ToolResult::success("wrote file"))], Vec::new());
        let mut turn_state = TurnRuntimeState::new(true);
        let mut tool_results_text = String::new();
        let mut messages = Vec::new();
        let mut changed_files = Vec::new();
        let baseline = HashSet::new();
        turn_state
            .focused_repair
            .action_checkpoint_requires_patch_before_validation = true;

        PatchSynthesisExecutor::collect_batch_results(PatchSynthesisCollectionContext {
            tool_batch: &mut batch,
            turn_state: &mut turn_state,
            tool_results_text: &mut tool_results_text,
            messages: &mut messages,
            changed_files: &mut changed_files,
            baseline_git_status_files: &baseline,
            is_programming_workflow: false,
            mark_patch_requirement_on_success: false,
        })
        .await;

        assert!(
            turn_state
                .focused_repair
                .action_checkpoint_requires_patch_before_validation
        );
        assert_eq!(changed_files, vec![PathBuf::from("src/lib.rs")]);
    }
}
