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
use crate::engine::task_context::{AgentTaskMode, TaskContextBundle};
use crate::engine::trace::{TraceCollector, TraceEvent};
use crate::services::api::{Message, ToolCall};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

pub(super) struct ToolBatchProcessingContext<'a> {
    pub(super) tool_calls: &'a [ToolCall],
    pub(super) tool_batch: &'a mut ToolExecutionBatch,
    pub(super) turn_state: &'a mut TurnRuntimeState,
    pub(super) task_bundle: &'a mut TaskContextBundle,
    pub(super) messages: &'a mut Vec<Message>,
    pub(super) trace: &'a TraceCollector,
    pub(super) session_id: &'a str,
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
            session_id,
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
                    session_id: Some(session_id),
                    working_dir,
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
            Self::apply_route_recovery_if_needed(
                tc,
                result,
                &mut outcome,
                turn_state,
                task_bundle,
                trace,
                messages,
            );
            if is_programming_workflow {
                Self::append_shell_file_mutation_correction(
                    tc,
                    result,
                    &mut outcome,
                    trace,
                    messages,
                );
            }

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
            // Bash write risk gate: if files changed but no file_write/file_edit/file_patch
            // was used successfully, bash is the sole mutation path — record a framework risk.
            if !outcome.changed_files.is_empty() && !outcome.successful_write_tool {
                trace.record(TraceEvent::WorkflowFallback {
                    error: format!(
                        "bash_only_mutation: {} file(s) changed without file_write/file_edit/file_patch. \
                         Prefer file tools for code changes.",
                        outcome.changed_files.len()
                    ),
                });
            }
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
                messages.push(Message::system(format!(
                    "<recent_observation>\n{}\n</recent_observation>",
                    note.text
                )));
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
                let success_count = turn_state
                    .successful_read_only_tool_fingerprints
                    .entry(fingerprint.clone())
                    .or_insert(0);
                *success_count += 1;
                if *success_count >= 2 {
                    outcome
                        .duplicate_successful_read_only_tools
                        .push(tool_call.name.clone());
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

    fn apply_route_recovery_if_needed(
        tool_call: &ToolCall,
        result: &crate::tools::ToolResult,
        outcome: &mut ToolBatchProcessingOutcome,
        turn_state: &mut TurnRuntimeState,
        task_bundle: &mut TaskContextBundle,
        trace: &TraceCollector,
        messages: &mut Vec<Message>,
    ) {
        if result.success {
            return;
        }
        let error = result
            .error
            .as_deref()
            .filter(|error| !error.trim().is_empty())
            .unwrap_or(result.content.as_str());
        if !crate::engine::route_recovery::is_unexposed_tool_error(error) {
            return;
        }

        let mode_before = task_bundle.agent_state.mode;
        let Some(decision) = turn_state.route_recovery.observe_unexposed_tool_request(
            task_bundle.route.workflow,
            task_mode_label(mode_before),
            &tool_call.name,
            error,
        ) else {
            return;
        };

        if decision.mode_escalates_to_light && mode_before == AgentTaskMode::Direct {
            task_bundle.agent_state.mode = AgentTaskMode::Light;
        }

        super::turn_recording::record_recovery_plan(trace, &decision.plan);
        messages.push(Message::system(format!(
            "<recent_observation>\n{}\n</recent_observation>",
            decision.correction
        )));
        outcome.tool_results_text.push('\n');
        outcome.tool_results_text.push_str(&decision.correction);
        outcome.tool_results_text.push('\n');
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
            messages.push(Message::system(format!(
                "<recent_observation>\n{}\n</recent_observation>",
                guard
            )));
            outcome.tool_results_text.push('\n');
            outcome.tool_results_text.push_str(&guard);
            outcome.tool_results_text.push('\n');
        }
    }

    fn append_shell_file_mutation_correction(
        tool_call: &ToolCall,
        result: &crate::tools::ToolResult,
        outcome: &mut ToolBatchProcessingOutcome,
        trace: &TraceCollector,
        messages: &mut Vec<Message>,
    ) {
        if result.success || tool_call.name != "bash" {
            return;
        }
        let command = tool_call.arguments["command"]
            .as_str()
            .or_else(|| tool_call.arguments["cmd"].as_str())
            .unwrap_or_default();
        let classification = crate::tools::bash_tool::command_classifier::classify_command(command);
        if !matches!(
            classification.category,
            crate::tools::bash_tool::command_classifier::ShellCommandCategory::FileMutation
        ) {
            return;
        }
        if outcome
            .tool_results_text
            .contains("Shell file mutation correction")
        {
            return;
        }

        let correction = "Shell file mutation correction: the last bash command tried to create, overwrite, or edit files. For code changes, use file_write/file_edit/file_patch so permission, stale-read, diff, checkpoint, and rollback checks stay active. Keep bash or run_tests for validation after a file tool succeeds. Do not retry the same shell redirection, heredoc, or script-based file write.".to_string();
        trace.record(TraceEvent::WorkflowFallback {
            error: "bash file mutation failure redirected to file edit tools".to_string(),
        });
        outcome.tool_results_text.push('\n');
        outcome.tool_results_text.push_str(&correction);
        messages.push(Message::system(format!(
            "<recent_observation>\n{}\n</recent_observation>",
            correction
        )));
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
            messages.push(Message::system(format!(
                "<recent_observation>\n{}\n</recent_observation>",
                correction
            )));
        }
    }
}

fn task_mode_label(mode: AgentTaskMode) -> &'static str {
    match mode {
        AgentTaskMode::Direct => "direct",
        AgentTaskMode::Light => "light",
        AgentTaskMode::Full => "full",
        AgentTaskMode::HighRisk => "high_risk",
    }
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

    fn direct_task_bundle() -> TaskContextBundle {
        let route = crate::engine::intent_router::IntentRouter::new().route("hello");
        TaskContextBundle::new("hello", ".", route, None)
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
            session_id: "session-test",
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
            session_id: "session-test",
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
    async fn failed_bash_file_mutation_adds_file_tool_repair_correction() {
        let call = tool_call(
            "call_1",
            "bash",
            serde_json::json!({
                "command": "cat > src/lib.rs <<'EOF'\nfn main() {}\nEOF",
                "description": "Write source file"
            }),
        );
        let mut batch = ToolExecutionBatch::new(
            vec![(
                call.clone(),
                ToolResult::error("Permission denied: 'bash' requires user confirmation."),
            )],
            Vec::new(),
        );
        let mut turn_state = TurnRuntimeState::new(true);
        let mut messages = Vec::new();
        let trace = trace();
        let mut companion_keys = HashSet::new();
        let mut failed_fingerprints = HashMap::new();
        let mut failed_names = HashMap::new();
        let mut successful_required = HashSet::new();
        let mut task_bundle = task_bundle();
        let destructive_scope = destructive_scope();
        let baseline = HashSet::new();

        let outcome = ToolBatchResultProcessor::process(ToolBatchProcessingContext {
            tool_calls: std::slice::from_ref(&call),
            tool_batch: &mut batch,
            turn_state: &mut turn_state,
            task_bundle: &mut task_bundle,
            messages: &mut messages,
            trace: &trace,
            session_id: "session-test",
            is_programming_workflow: true,
            working_dir: Path::new("."),
            last_user_preview: "edit file",
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

        assert!(outcome
            .tool_results_text
            .contains("Shell file mutation correction"));
        assert!(outcome
            .tool_results_text
            .contains("file_write/file_edit/file_patch"));
        assert!(messages.iter().any(|message| {
            matches!(message, Message::System { content } if content.contains("Shell file mutation correction"))
        }));
        assert!(trace.snapshot().events.iter().any(|event| {
            matches!(
                event,
                TraceEvent::WorkflowFallback { error }
                    if error.contains("bash file mutation failure")
            )
        }));
    }

    #[tokio::test]
    async fn repeated_successful_read_only_is_counted_without_model_prompt() {
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
            session_id: "session-test",
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
            .duplicate_successful_read_only_tools
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
            session_id: "session-test",
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
        assert!(!second_messages.iter().any(|message| matches!(
            message,
            Message::System { content } if content.contains("duplicated an earlier result")
        )));
    }

    #[tokio::test]
    async fn unexposed_read_tool_triggers_safe_route_recovery() {
        let call = tool_call(
            "call_1",
            "file_read",
            serde_json::json!({"path": "README.md"}),
        );
        let mut batch = ToolExecutionBatch::new(
            vec![(
                call.clone(),
                ToolResult::error(
                    "Tool 'file_read' was not exposed in the current request and cannot be executed.",
                ),
            )],
            Vec::new(),
        );
        let mut turn_state = TurnRuntimeState::new(true);
        let trace = trace();
        let mut companion_keys = HashSet::new();
        let mut failed_fingerprints = HashMap::new();
        let mut failed_names = HashMap::new();
        let mut successful_required = HashSet::new();
        let mut task_bundle = direct_task_bundle();
        let destructive_scope = destructive_scope();
        let baseline = HashSet::new();
        assert_eq!(
            task_bundle.agent_state.mode,
            crate::engine::task_context::AgentTaskMode::Direct
        );
        let mut messages = Vec::new();

        let outcome = ToolBatchResultProcessor::process(ToolBatchProcessingContext {
            tool_calls: std::slice::from_ref(&call),
            tool_batch: &mut batch,
            turn_state: &mut turn_state,
            task_bundle: &mut task_bundle,
            messages: &mut messages,
            trace: &trace,
            session_id: "session-test",
            is_programming_workflow: false,
            working_dir: Path::new("."),
            last_user_preview: "read README",
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

        assert!(turn_state.route_recovery.read_search_expanded);
        assert_eq!(turn_state.route_recovery.read_search_expansions, 1);
        assert_eq!(
            task_bundle.agent_state.mode,
            crate::engine::task_context::AgentTaskMode::Light
        );
        assert!(outcome.tool_results_text.contains("Route recovery"));
        assert!(messages.iter().any(|message| {
            matches!(message, Message::System { content } if content.contains("read/search tools only"))
        }));
        assert!(trace.snapshot().events.iter().any(|event| {
            matches!(
                event,
                TraceEvent::RecoveryPlan {
                    source,
                    recovery_kind,
                    safe_retry,
                    ..
                } if source == "route_recovery"
                    && recovery_kind == "expand_read_search_only"
                    && *safe_retry
            )
        }));
    }

    #[tokio::test]
    async fn unexposed_mutation_tool_does_not_expand_route_recovery_tools() {
        let call = tool_call(
            "call_1",
            "file_edit",
            serde_json::json!({"path": "README.md", "old_string": "a", "new_string": "b"}),
        );
        let mut batch = ToolExecutionBatch::new(
            vec![(
                call.clone(),
                ToolResult::error(
                    "Tool 'file_edit' was not exposed in the current request and cannot be executed.",
                ),
            )],
            Vec::new(),
        );
        let mut turn_state = TurnRuntimeState::new(true);
        let trace = trace();
        let mut companion_keys = HashSet::new();
        let mut failed_fingerprints = HashMap::new();
        let mut failed_names = HashMap::new();
        let mut successful_required = HashSet::new();
        let mut task_bundle = direct_task_bundle();
        let destructive_scope = destructive_scope();
        let baseline = HashSet::new();
        let mut messages = Vec::new();

        let outcome = ToolBatchResultProcessor::process(ToolBatchProcessingContext {
            tool_calls: std::slice::from_ref(&call),
            tool_batch: &mut batch,
            turn_state: &mut turn_state,
            task_bundle: &mut task_bundle,
            messages: &mut messages,
            trace: &trace,
            session_id: "session-test",
            is_programming_workflow: false,
            working_dir: Path::new("."),
            last_user_preview: "edit README",
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

        assert!(!turn_state.route_recovery.read_search_expanded);
        assert_eq!(turn_state.route_recovery.blocked_mutation_requests, 1);
        assert_eq!(
            task_bundle.agent_state.mode,
            crate::engine::task_context::AgentTaskMode::Direct
        );
        assert!(outcome
            .tool_results_text
            .contains("cannot silently expand mutation authority"));
        assert!(trace.snapshot().events.iter().any(|event| {
            matches!(
                event,
                TraceEvent::RecoveryPlan {
                    source,
                    recovery_kind,
                    safe_retry,
                    requires_user_decision,
                    ..
                } if source == "route_recovery"
                    && recovery_kind == "no_silent_mutation_expansion"
                    && !*safe_retry
                    && *requires_user_decision
            )
        }));
    }
}
