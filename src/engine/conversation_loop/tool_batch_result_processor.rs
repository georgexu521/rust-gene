use super::companion_context;
use super::tool_context_helpers::{tool_call_fingerprint, tool_result_dialog_content};
use super::tool_execution::is_read_only;
use super::tool_execution_controller::ToolExecutionBatch;
use super::tool_turn_controller::{ToolTurnAppendContext, ToolTurnController};
use super::turn_runtime_state::TurnRuntimeState;
use super::validation_runner::RequiredValidationController;
use super::workflow_change_tracker::WorkflowChangeTracker;
use super::ConversationLoop;
use crate::engine::destructive_scope::DestructiveScopeContract;
use crate::engine::task_context::TaskContextBundle;
use crate::engine::trace::{TraceCollector, TraceEvent};
use crate::services::api::{Message, ToolCall};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

const DUPLICATE_READ_ONLY_RESULT_CHAR_LIMIT: usize = 6_000;

pub(super) struct ToolBatchProcessingContext<'a> {
    pub(super) tool_calls: &'a [ToolCall],
    pub(super) tool_batch: &'a mut ToolExecutionBatch,
    pub(super) turn_state: &'a mut TurnRuntimeState,
    pub(super) task_bundle: &'a mut TaskContextBundle,
    pub(super) messages: &'a mut Vec<Message>,
    pub(super) trace: &'a TraceCollector,
    pub(super) is_programming_workflow: bool,
    pub(super) working_dir: &'a Path,
    pub(super) last_user_preview: &'a str,
    pub(super) companion_context_keys: &'a mut HashSet<String>,
    pub(super) failed_tool_fingerprints: &'a mut HashMap<String, usize>,
    pub(super) failed_tool_names: &'a mut HashMap<String, usize>,
    pub(super) required_validation_commands: &'a [String],
    pub(super) successful_required_validation_commands: &'a mut HashSet<String>,
    pub(super) action_checkpoint_active: bool,
    pub(super) destructive_scope: &'a DestructiveScopeContract,
    pub(super) baseline_git_status_files: &'a HashSet<PathBuf>,
}

pub(super) struct ToolBatchProcessingOutcome {
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
    pub(super) duplicate_successful_read_only_results: Vec<DuplicateSuccessfulReadOnlyToolResult>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DuplicateSuccessfulReadOnlyToolResult {
    pub(super) tool_name: String,
    pub(super) result_text: String,
    pub(super) ledger_summary: Option<String>,
}

pub(super) struct ToolBatchResultProcessor;

impl ToolBatchResultProcessor {
    pub(super) async fn process(
        context: ToolBatchProcessingContext<'_>,
    ) -> ToolBatchProcessingOutcome {
        let ToolBatchProcessingContext {
            tool_calls,
            tool_batch,
            turn_state,
            task_bundle,
            messages,
            trace,
            is_programming_workflow,
            working_dir,
            last_user_preview,
            companion_context_keys,
            failed_tool_fingerprints,
            failed_tool_names,
            required_validation_commands,
            successful_required_validation_commands,
            action_checkpoint_active,
            destructive_scope,
            baseline_git_status_files,
        } = context;

        let mut outcome = ToolBatchProcessingOutcome {
            tool_results_text: String::new(),
            changed_files: Vec::new(),
            batch_has_unsuccessful_tools: tool_batch.unsuccessful_count() > 0,
            used_write_tool: tool_calls
                .iter()
                .any(|tc| ConversationLoop::is_code_write_tool_name(&tc.name)),
            successful_write_tool: false,
            used_action_checkpoint_lookup: action_checkpoint_active
                && tool_calls
                    .iter()
                    .any(|tc| matches!(tc.name.as_str(), "file_read" | "grep")),
            any_tool_success: tool_batch.any_success(),
            repeated_failed_tools: Vec::new(),
            failed_tool_names_this_round: Vec::new(),
            failed_tool_evidence: Vec::new(),
            file_edit_failure_correction_added: false,
            successful_validation_commands: Vec::new(),
            duplicate_successful_read_only_tools: Vec::new(),
            duplicate_successful_read_only_results: Vec::new(),
        };

        if outcome.used_write_tool && !required_validation_commands.is_empty() {
            successful_required_validation_commands.clear();
        }

        for (tc, result) in tool_batch.results_mut().iter_mut() {
            ToolTurnController::append_tool_result(
                tc,
                result,
                ToolTurnAppendContext {
                    evidence_ledger: &mut turn_state.evidence_ledger,
                    runtime_diet: &mut turn_state.runtime_diet,
                    tool_results_text: &mut outcome.tool_results_text,
                    messages: &mut *messages,
                },
            )
            .await;

            if is_programming_workflow {
                Self::append_companion_context_note(
                    tc,
                    result,
                    &mut outcome,
                    working_dir,
                    last_user_preview,
                    companion_context_keys,
                    messages,
                );
            }

            Self::record_tool_success_or_failure(
                tc,
                result,
                &mut outcome,
                turn_state,
                failed_tool_fingerprints,
                failed_tool_names,
            );
            task_bundle
                .agent_state
                .observe_tool_context_evidence(tc, result);

            if result.success && matches!(tc.name.as_str(), "file_edit" | "file_write") {
                outcome.successful_write_tool = true;
                turn_state
                    .focused_repair
                    .action_checkpoint_requires_patch_before_validation = false;
                if let Some(path) = tc.arguments["path"].as_str() {
                    outcome.changed_files.push(PathBuf::from(path));
                }
            }

            if let Some(command) =
                RequiredValidationController::successful_validation_command(tc, result.success)
            {
                if RequiredValidationController::command_matches_required(
                    required_validation_commands,
                    &command,
                ) {
                    successful_required_validation_commands.insert(command.clone());
                }
                outcome.successful_validation_commands.push(command);
            }
        }

        if !outcome.duplicate_successful_read_only_tools.is_empty() {
            let mut tools = outcome.duplicate_successful_read_only_tools.clone();
            tools.sort();
            tools.dedup();
            messages.push(Message::system(format!(
                "The last successful read-only tool call duplicated an earlier result: {}. Stop calling the same read-only tool with the same arguments. Answer the user from the tool output already present in this conversation.",
                tools.join(", ")
            )));
        }

        Self::append_destructive_scope_guard(
            &mut outcome,
            tool_batch,
            destructive_scope,
            working_dir,
            trace,
            messages,
        );
        if is_programming_workflow {
            Self::append_file_edit_failure_correction(&mut outcome, trace, messages);
            WorkflowChangeTracker::append_changed_files_since(
                &mut outcome.changed_files,
                baseline_git_status_files,
            );
        }

        outcome
    }

    fn append_companion_context_note(
        tool_call: &ToolCall,
        result: &crate::tools::ToolResult,
        outcome: &mut ToolBatchProcessingOutcome,
        working_dir: &Path,
        last_user_preview: &str,
        companion_context_keys: &mut HashSet<String>,
        messages: &mut Vec<Message>,
    ) {
        if let Some(note) = companion_context::companion_context_note(
            working_dir,
            last_user_preview,
            tool_call,
            result,
        ) {
            if companion_context_keys.insert(note.key) {
                outcome.tool_results_text.push('\n');
                outcome.tool_results_text.push_str(&note.text);
                outcome.tool_results_text.push('\n');
                messages.push(Message::system(note.text));
            }
        }
    }

    fn record_tool_success_or_failure(
        tool_call: &ToolCall,
        result: &crate::tools::ToolResult,
        outcome: &mut ToolBatchProcessingOutcome,
        turn_state: &mut TurnRuntimeState,
        failed_tool_fingerprints: &mut HashMap<String, usize>,
        failed_tool_names: &mut HashMap<String, usize>,
    ) {
        let fingerprint = tool_call_fingerprint(tool_call);
        if result.success {
            if is_read_only(&tool_call.name) {
                let result_text = tool_result_dialog_content(result);
                let cached_result_text = turn_state
                    .successful_read_only_tool_results
                    .entry(fingerprint.clone())
                    .and_modify(|cached| {
                        if is_read_cache_notice(cached) && !is_read_cache_notice(&result_text) {
                            *cached = bounded_duplicate_read_only_result(&result_text);
                        }
                    })
                    .or_insert_with(|| bounded_duplicate_read_only_result(&result_text))
                    .clone();
                let success_count = turn_state
                    .successful_read_only_tool_fingerprints
                    .entry(fingerprint.clone())
                    .or_insert(0);
                *success_count += 1;
                if *success_count >= 2 {
                    let message = format!(
                        "Repeated successful read-only tool call detected: {}. You already have this result; answer from existing tool output now and do not call the same read-only tool with the same arguments again.",
                        tool_call.name
                    );
                    if *success_count == 2 {
                        outcome.tool_results_text.push('\n');
                        outcome.tool_results_text.push_str(&message);
                        outcome.tool_results_text.push('\n');
                    }
                    outcome
                        .duplicate_successful_read_only_tools
                        .push(tool_call.name.clone());
                    outcome.duplicate_successful_read_only_results.push(
                        DuplicateSuccessfulReadOnlyToolResult {
                            tool_name: tool_call.name.clone(),
                            result_text: cached_result_text,
                            ledger_summary: ledger_summary_from_result(result),
                        },
                    );
                }
            }
            failed_tool_fingerprints.remove(&fingerprint);
            failed_tool_names.remove(&tool_call.name);
            return;
        }

        let count = failed_tool_fingerprints.entry(fingerprint).or_insert(0);
        *count += 1;
        if *count >= 2 {
            outcome.repeated_failed_tools.push(tool_call.name.clone());
        }
        let name_count = failed_tool_names.entry(tool_call.name.clone()).or_insert(0);
        *name_count += 1;
        outcome
            .failed_tool_names_this_round
            .push(tool_call.name.clone());
        outcome.failed_tool_evidence.push(format!(
            "{} {} failed:\n{}",
            tool_call.name,
            tool_call.id,
            tool_result_dialog_content(result)
        ));
    }

    fn append_destructive_scope_guard(
        outcome: &mut ToolBatchProcessingOutcome,
        tool_batch: &ToolExecutionBatch,
        destructive_scope: &DestructiveScopeContract,
        working_dir: &Path,
        trace: &TraceCollector,
        messages: &mut Vec<Message>,
    ) {
        if let Some(guard) = destructive_scope
            .completion_guard_for_results(tool_batch.result_successes(), working_dir)
        {
            trace.record(TraceEvent::DestructiveScopeChecked {
                tool: "assistant_response".to_string(),
                call_id: "post_action_guard".to_string(),
                operation: "post_action_guard".to_string(),
                target: None,
                allowed: false,
                reason: guard.clone(),
            });
            messages.push(Message::system(guard.clone()));
            outcome.tool_results_text.push('\n');
            outcome.tool_results_text.push_str(&guard);
            outcome.tool_results_text.push('\n');
        }
    }

    fn append_file_edit_failure_correction(
        outcome: &mut ToolBatchProcessingOutcome,
        trace: &TraceCollector,
        messages: &mut Vec<Message>,
    ) {
        if let Some(correction) =
            ConversationLoop::file_edit_failure_repair_correction(&outcome.failed_tool_evidence)
        {
            trace.record(TraceEvent::WorkflowFallback {
                error: "file_edit failure converted to line-range repair correction".to_string(),
            });
            outcome.file_edit_failure_correction_added = true;
            outcome.tool_results_text.push('\n');
            outcome.tool_results_text.push_str(&correction);
            messages.push(Message::system(correction));
        }
    }
}

fn ledger_summary_from_result(result: &crate::tools::ToolResult) -> Option<String> {
    let data = result.data.as_ref()?;
    if let Some(path) = data.get("path").and_then(serde_json::Value::as_str) {
        let total_lines = data
            .get("total_lines")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        let content_hash = data
            .get("content_hash")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown");
        let coverage = data
            .get("read_coverage")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("read");
        return Some(format!(
            "ledger: file `{path}` is unchanged in this session ({coverage}, {total_lines} lines, hash {content_hash})"
        ));
    }
    if let Some(shell_result) = data.get("shell_result") {
        let command = shell_result
            .get("command")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("bash");
        let exit_code = shell_result
            .get("exit_code")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0);
        return Some(format!(
            "ledger: read-only command `{command}` already ran in this session with exit {exit_code}"
        ));
    }
    None
}

fn bounded_duplicate_read_only_result(text: &str) -> String {
    let trimmed = text.trim();
    let mut preview = String::new();
    let mut truncated = false;
    for (idx, ch) in trimmed.chars().enumerate() {
        if idx >= DUPLICATE_READ_ONLY_RESULT_CHAR_LIMIT {
            truncated = true;
            break;
        }
        preview.push(ch);
    }
    if truncated {
        preview.push_str("\n\n[stored read-only result truncated]");
    }
    preview
}

fn is_read_cache_notice(text: &str) -> bool {
    text.trim_start()
        .starts_with("[File unchanged since last read:")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::trace::TurnTrace;
    use crate::tools::ToolResult;

    fn tool_call(id: &str, name: &str, arguments: serde_json::Value) -> ToolCall {
        ToolCall {
            id: id.to_string(),
            name: name.to_string(),
            arguments,
        }
    }

    fn trace() -> TraceCollector {
        TraceCollector::new(TurnTrace::new("session", 1, "test"))
    }

    fn destructive_scope() -> DestructiveScopeContract {
        DestructiveScopeContract::from_user_request("test", Path::new("."))
    }

    fn task_bundle() -> TaskContextBundle {
        let route = crate::engine::intent_router::IntentRouter::new().route("modify src/lib.rs");
        TaskContextBundle::new("modify src/lib.rs", ".", route, None)
    }

    #[tokio::test]
    async fn records_successful_write_and_clears_required_validation_progress() {
        let call = tool_call(
            "call_1",
            "file_write",
            serde_json::json!({"path": "src/lib.rs"}),
        );
        let mut batch = ToolExecutionBatch::new(
            vec![(call.clone(), ToolResult::success("wrote file"))],
            Vec::new(),
        );
        let mut turn_state = TurnRuntimeState::new(true);
        let mut messages = Vec::new();
        let trace = trace();
        let mut companion_keys = HashSet::new();
        let mut failed_fingerprints = HashMap::new();
        let mut failed_names = HashMap::new();
        let required = vec!["cargo test -q".to_string()];
        let mut successful_required = HashSet::from(["cargo test -q".to_string()]);
        let mut task_bundle = task_bundle();
        turn_state
            .focused_repair
            .action_checkpoint_requires_patch_before_validation = true;
        let destructive_scope = destructive_scope();
        let baseline = HashSet::new();

        let outcome = ToolBatchResultProcessor::process(ToolBatchProcessingContext {
            tool_calls: std::slice::from_ref(&call),
            tool_batch: &mut batch,
            turn_state: &mut turn_state,
            task_bundle: &mut task_bundle,
            messages: &mut messages,
            trace: &trace,
            is_programming_workflow: false,
            working_dir: Path::new("."),
            last_user_preview: "write file",
            companion_context_keys: &mut companion_keys,
            failed_tool_fingerprints: &mut failed_fingerprints,
            failed_tool_names: &mut failed_names,
            required_validation_commands: &required,
            successful_required_validation_commands: &mut successful_required,
            action_checkpoint_active: false,
            destructive_scope: &destructive_scope,
            baseline_git_status_files: &baseline,
        })
        .await;

        assert!(outcome.used_write_tool);
        assert!(outcome.successful_write_tool);
        assert_eq!(outcome.changed_files, vec![PathBuf::from("src/lib.rs")]);
        assert!(successful_required.is_empty());
        assert!(
            !turn_state
                .focused_repair
                .action_checkpoint_requires_patch_before_validation
        );
        assert!(task_bundle.agent_state.completed_steps.iter().any(|step| {
            step.stage == crate::engine::task_context::AgentTaskStage::Edit
                && step.summary.contains("src/lib.rs")
        }));
        assert_eq!(messages.len(), 1);
    }

    #[tokio::test]
    async fn tracks_repeated_failure_and_adds_file_edit_repair_correction() {
        let call = tool_call(
            "call_1",
            "file_edit",
            serde_json::json!({"path": "src/lib.rs", "old_string": "foo", "new_string": "bar"}),
        );
        let fingerprint = tool_call_fingerprint(&call);
        let mut batch = ToolExecutionBatch::new(
            vec![(call.clone(), ToolResult::error("Expected 1 occurrence"))],
            Vec::new(),
        );
        let mut turn_state = TurnRuntimeState::new(true);
        let mut messages = Vec::new();
        let trace = trace();
        let mut companion_keys = HashSet::new();
        let mut failed_fingerprints = HashMap::from([(fingerprint, 1usize)]);
        let mut failed_names = HashMap::new();
        let mut successful_required = HashSet::new();
        let mut task_bundle = task_bundle();
        let destructive_scope = destructive_scope();
        let baseline = WorkflowChangeTracker::git_status_files();

        let outcome = ToolBatchResultProcessor::process(ToolBatchProcessingContext {
            tool_calls: std::slice::from_ref(&call),
            tool_batch: &mut batch,
            turn_state: &mut turn_state,
            task_bundle: &mut task_bundle,
            messages: &mut messages,
            trace: &trace,
            is_programming_workflow: true,
            working_dir: Path::new("."),
            last_user_preview: "edit file",
            companion_context_keys: &mut companion_keys,
            failed_tool_fingerprints: &mut failed_fingerprints,
            failed_tool_names: &mut failed_names,
            required_validation_commands: &[],
            successful_required_validation_commands: &mut successful_required,
            action_checkpoint_active: true,
            destructive_scope: &destructive_scope,
            baseline_git_status_files: &baseline,
        })
        .await;

        assert_eq!(outcome.repeated_failed_tools, vec!["file_edit"]);
        assert_eq!(outcome.failed_tool_names_this_round, vec!["file_edit"]);
        assert!(outcome.file_edit_failure_correction_added);
        assert!(outcome
            .tool_results_text
            .contains("File edit repair correction"));
        assert!(messages.iter().any(|message| {
            matches!(message, Message::System { content } if content.contains("File edit repair correction"))
        }));
    }

    #[tokio::test]
    async fn repeated_successful_read_only_uses_first_full_result_for_closeout() {
        let call = tool_call(
            "call_1",
            "file_read",
            serde_json::json!({"path": "README.md"}),
        );
        let mut turn_state = TurnRuntimeState::new(true);
        let trace = trace();
        let mut companion_keys = HashSet::new();
        let mut failed_fingerprints = HashMap::new();
        let mut failed_names = HashMap::new();
        let mut successful_required = HashSet::new();
        let mut task_bundle = task_bundle();
        let destructive_scope = destructive_scope();
        let baseline = HashSet::new();

        let mut first_batch = ToolExecutionBatch::new(
            vec![(
                call.clone(),
                ToolResult::success("   1 | # PhageMatch\n   2 | first full result"),
            )],
            Vec::new(),
        );
        let mut first_messages = Vec::new();
        let first_outcome = ToolBatchResultProcessor::process(ToolBatchProcessingContext {
            tool_calls: std::slice::from_ref(&call),
            tool_batch: &mut first_batch,
            turn_state: &mut turn_state,
            task_bundle: &mut task_bundle,
            messages: &mut first_messages,
            trace: &trace,
            is_programming_workflow: false,
            working_dir: Path::new("."),
            last_user_preview: "read readme",
            companion_context_keys: &mut companion_keys,
            failed_tool_fingerprints: &mut failed_fingerprints,
            failed_tool_names: &mut failed_names,
            required_validation_commands: &[],
            successful_required_validation_commands: &mut successful_required,
            action_checkpoint_active: false,
            destructive_scope: &destructive_scope,
            baseline_git_status_files: &baseline,
        })
        .await;
        assert!(first_outcome
            .duplicate_successful_read_only_results
            .is_empty());

        let mut second_batch = ToolExecutionBatch::new(
            vec![(
                call.clone(),
                ToolResult::success(
                    "[File unchanged since last read: README.md] (2 lines)\nIf you need the full content, it was provided in a previous message.",
                ),
            )],
            Vec::new(),
        );
        let mut second_messages = Vec::new();
        let second_outcome = ToolBatchResultProcessor::process(ToolBatchProcessingContext {
            tool_calls: std::slice::from_ref(&call),
            tool_batch: &mut second_batch,
            turn_state: &mut turn_state,
            task_bundle: &mut task_bundle,
            messages: &mut second_messages,
            trace: &trace,
            is_programming_workflow: false,
            working_dir: Path::new("."),
            last_user_preview: "read readme",
            companion_context_keys: &mut companion_keys,
            failed_tool_fingerprints: &mut failed_fingerprints,
            failed_tool_names: &mut failed_names,
            required_validation_commands: &[],
            successful_required_validation_commands: &mut successful_required,
            action_checkpoint_active: false,
            destructive_scope: &destructive_scope,
            baseline_git_status_files: &baseline,
        })
        .await;

        assert_eq!(
            second_outcome.duplicate_successful_read_only_tools,
            vec!["file_read".to_string()]
        );
        assert_eq!(
            second_outcome.duplicate_successful_read_only_results[0].result_text,
            "1 | # PhageMatch\n   2 | first full result"
        );
    }
}
