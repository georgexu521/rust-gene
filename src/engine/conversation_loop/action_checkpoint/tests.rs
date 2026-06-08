use super::*;
use std::collections::HashSet;
use tempfile::tempdir;

fn progress_request() -> ProgressCheckpointRequest {
    ProgressCheckpointRequest {
        no_diff_audit_closeout_allowed: false,
        has_worktree_changes: false,
        has_successful_validation_commands: false,
        no_code_progress_rounds: 0,
        action_checkpoint_active: false,
        action_checkpoint_lookup_count: 0,
        action_checkpoint_no_change_rounds: 0,
        no_diff_audit_validation_checkpoint_sent: false,
        code_write_tools_forbidden: false,
        code_write_forbidden_checkpoint_sent: false,
        used_action_checkpoint_lookup: false,
    }
}

#[test]
fn audit_no_diff_validation_resets_rounds_and_marks_sent() {
    let decision =
        ProgressCheckpointController::evaluate_read_only_success(ProgressCheckpointRequest {
            no_diff_audit_closeout_allowed: true,
            no_code_progress_rounds: 1,
            ..progress_request()
        });

    assert_eq!(
        decision.action,
        ProgressCheckpointAction::AuditNoDiffValidation
    );
    assert_eq!(decision.no_code_progress_rounds, 0);
    assert!(decision.no_diff_audit_validation_checkpoint_sent);
    assert!(!decision.force_patch_synthesis_after_no_change);
}

#[test]
fn existing_diff_repair_enters_patch_synthesis() {
    let decision =
        ProgressCheckpointController::evaluate_read_only_success(ProgressCheckpointRequest {
            has_worktree_changes: true,
            no_code_progress_rounds: 1,
            ..progress_request()
        });

    assert_eq!(
        decision.action,
        ProgressCheckpointAction::ExistingDiffNeedsRepair {
            no_code_progress_rounds: 2
        }
    );
    assert!(decision.action_checkpoint_active);
    assert_eq!(
        decision.action_checkpoint_lookup_count,
        ConversationLoop::ACTION_CHECKPOINT_TARGETED_LOOKUP_BUDGET
    );
    assert_eq!(decision.action_checkpoint_no_change_rounds, 2);
    assert!(decision.reset_file_edit_failure_retry);
    assert!(decision.force_patch_synthesis_after_no_change);
    assert_eq!(
        decision.force_patch_synthesis_reason,
        Some("existing diff still needs repair after repeated read-only rounds")
    );
}

#[test]
fn focused_lookup_exhaustion_forces_patch_synthesis() {
    let decision =
        ProgressCheckpointController::evaluate_read_only_success(ProgressCheckpointRequest {
            action_checkpoint_active: true,
            action_checkpoint_lookup_count: 1,
            used_action_checkpoint_lookup: true,
            ..progress_request()
        });

    assert_eq!(
        decision.action,
        ProgressCheckpointAction::FocusedLookupNotice { exhausted: true }
    );
    assert_eq!(
        decision.action_checkpoint_lookup_count,
        ConversationLoop::ACTION_CHECKPOINT_TARGETED_LOOKUP_BUDGET
    );
    assert_eq!(decision.action_checkpoint_no_change_rounds, 1);
    assert!(decision.force_patch_synthesis_after_no_change);
    assert_eq!(
        decision.force_patch_synthesis_reason,
        Some("focused repair lookup budget exhausted")
    );
}

#[test]
fn focused_repair_stalled_forces_patch_synthesis() {
    let decision =
        ProgressCheckpointController::evaluate_read_only_success(ProgressCheckpointRequest {
            action_checkpoint_active: true,
            action_checkpoint_no_change_rounds: 2,
            ..progress_request()
        });

    assert_eq!(
        decision.action,
        ProgressCheckpointAction::FocusedRepairStalled
    );
    assert_eq!(decision.action_checkpoint_no_change_rounds, 3);
    assert!(decision.force_patch_synthesis_after_no_change);
    assert_eq!(
        decision.force_patch_synthesis_reason,
        Some("focused repair lookup did not produce a patch")
    );
}

fn code_workflow(prompt: &str) -> crate::engine::code_change_workflow::CodeChangeWorkflowRunner {
    let route = crate::engine::intent_router::IntentRouter::new().route(prompt);
    let bundle = crate::engine::task_context::TaskContextBundle::new(prompt, ".", route, None);
    crate::engine::code_change_workflow::CodeChangeWorkflowRunner::new(&bundle)
}

fn trace() -> TraceCollector {
    TraceCollector::new(crate::engine::trace::TurnTrace::new("session", 1, "test"))
}

#[test]
fn progress_action_appender_adds_system_checkpoint() {
    let trace = trace();
    let mut code_workflow = code_workflow("modify CLI status");
    let mut messages = Vec::new();
    let mut tool_results_text = String::new();

    ProgressCheckpointActionApplier::apply(ProgressCheckpointActionContext {
        action: ProgressCheckpointAction::ProgressReminder {
            no_code_progress_rounds: 2,
        },
        workflow: WorkflowKind::CodeChange,
        trace: &trace,
        code_workflow: &mut code_workflow,
        messages: &mut messages,
        tool_results_text: &mut tool_results_text,
    });

    assert_eq!(messages.len(), 1);
    assert!(matches!(
        &messages[0],
        Message::System { content } if content.contains("Workflow progress checkpoint")
    ));
    assert!(tool_results_text.contains("Workflow progress checkpoint"));
    let finished = trace.finish(crate::engine::trace::TurnStatus::Completed);
    assert!(finished.events.iter().any(|event| matches!(
        event,
        TraceEvent::WorkflowFallback { error }
            if error == "code-change task needs an edit after repeated inspection"
    )));
}

#[test]
fn existing_diff_action_activates_repeated_no_code_progress_trigger() {
    let trace = trace();
    let mut code_workflow = code_workflow("fix bug in parser");
    let mut messages = Vec::new();
    let mut tool_results_text = String::new();

    ProgressCheckpointActionApplier::apply(ProgressCheckpointActionContext {
        action: ProgressCheckpointAction::ExistingDiffNeedsRepair {
            no_code_progress_rounds: 2,
        },
        workflow: WorkflowKind::BugFix,
        trace: &trace,
        code_workflow: &mut code_workflow,
        messages: &mut messages,
        tool_results_text: &mut tool_results_text,
    });

    assert!(code_workflow
        .adaptive_trigger_labels()
        .contains(&"repeated_no_code_progress"));
    assert!(tool_results_text.contains("Workflow acceptance repair checkpoint"));
    let finished = trace.finish(crate::engine::trace::TurnStatus::Completed);
    assert!(finished.events.iter().any(|event| matches!(
        event,
        TraceEvent::AdaptiveWorkflowTriggered { trigger, .. }
            if trigger == "repeated_no_code_progress"
    )));
    assert!(finished.events.iter().any(|event| matches!(
        event,
        TraceEvent::RecoveryPlan {
            source,
            failure_type,
            recovery_kind,
            allowed_alternatives,
            ..
        } if source == "route_recovery"
            && failure_type == "code_change_no_diff_after_repeated_progress"
            && recovery_kind == "code_change_no_diff_replan"
            && allowed_alternatives
                .iter()
                .all(|tool| !crate::engine::route_recovery::is_mutation_tool(tool))
    )));
}

#[test]
fn no_diff_action_checkpoint_records_route_recovery_without_mutation_expansion() {
    let trace = trace();
    let mut code_workflow = code_workflow("modify CLI status");
    let mut messages = Vec::new();
    let mut tool_results_text = String::new();

    ProgressCheckpointActionApplier::apply(ProgressCheckpointActionContext {
        action: ProgressCheckpointAction::EnterActionCheckpoint {
            no_code_progress_rounds: 3,
        },
        workflow: WorkflowKind::CodeChange,
        trace: &trace,
        code_workflow: &mut code_workflow,
        messages: &mut messages,
        tool_results_text: &mut tool_results_text,
    });

    assert!(tool_results_text.contains("Workflow action checkpoint"));
    let finished = trace.finish(crate::engine::trace::TurnStatus::Completed);
    assert!(finished.events.iter().any(|event| matches!(
        event,
        TraceEvent::RecoveryPlan {
            source,
            category,
            failure_type,
            recovery_kind,
            safe_retry,
            requires_user_decision,
            allowed_alternatives,
            ..
        } if source == "route_recovery"
            && category == "route_drift"
            && failure_type == "code_change_no_diff_after_repeated_progress"
            && recovery_kind == "code_change_no_diff_replan"
            && *safe_retry
            && !*requires_user_decision
            && allowed_alternatives
                .iter()
                .all(|tool| !crate::engine::route_recovery::is_mutation_tool(tool))
    )));
}

#[test]
fn action_checkpoint_blocks_patch_bash_and_allows_validation_after_changes() {
    assert!(!ConversationLoop::bash_allowed_at_action_checkpoint(
        &serde_json::json!({"command": "python3 - <<'PY'\nfrom pathlib import Path\nPath('x').write_text('y')\nPY"}),
        false,
        &HashSet::new(),
    ));
    assert!(!ConversationLoop::bash_allowed_at_action_checkpoint(
        &serde_json::json!({"command": "apply_patch <<'PATCH'\n*** Begin Patch\n*** End Patch\nPATCH"}),
        false,
        &HashSet::new(),
    ));
    assert!(!ConversationLoop::bash_allowed_at_action_checkpoint(
        &serde_json::json!({"command": "cat > src/main.rs <<'EOF'\nfn main() {}\nEOF"}),
        false,
        &HashSet::new(),
    ));
    assert!(!ConversationLoop::bash_allowed_at_action_checkpoint(
        &serde_json::json!({"command": "sed -n '1,20p' src/main.rs"}),
        false,
        &HashSet::new(),
    ));
    assert!(!ConversationLoop::bash_allowed_at_action_checkpoint(
        &serde_json::json!({"command": "cargo test -q"}),
        false,
        &HashSet::new(),
    ));
    assert!(ConversationLoop::bash_allowed_at_action_checkpoint(
        &serde_json::json!({"command": "cargo test -q"}),
        true,
        &HashSet::new(),
    ));
    assert!(ConversationLoop::bash_allowed_at_action_checkpoint(
        &serde_json::json!({"command": "scripts/run_live_eval.sh --mode summary --run-id live-summary-smoke"}),
        true,
        &HashSet::new(),
    ));
    assert!(ConversationLoop::bash_allowed_at_action_checkpoint(
        &serde_json::json!({"command": "bash -n scripts/run_live_eval.sh"}),
        true,
        &HashSet::new(),
    ));
}

#[test]
fn focused_repair_blocks_bash_patch_bypass() {
    for command in [
        "apply_patch <<'PATCH'\n*** Begin Patch\n*** End Patch\nPATCH",
        "python3 - <<'PY'\nopen('x', 'w').write('y')\nPY",
        "sed -i '' 's/a/b/' src/main.rs",
        "cat > src/main.rs <<'EOF'\nfn main() {}\nEOF",
        "tee src/main.rs <<'EOF'\nfn main() {}\nEOF",
    ] {
        assert!(
            !ConversationLoop::bash_allowed_at_action_checkpoint(
                &serde_json::json!({ "command": command }),
                true,
                &HashSet::new(),
            ),
            "mutating bash command should not bypass file tools: {command}"
        );
    }

    assert!(ConversationLoop::bash_allowed_at_action_checkpoint(
        &serde_json::json!({"command": "cargo test -q"}),
        true,
        &HashSet::new(),
    ));
}

#[test]
fn action_checkpoint_allows_bounded_artifact_prep_without_file_edit_tools() {
    assert!(ConversationLoop::bash_allowed_at_action_checkpoint(
        &serde_json::json!({"command": "python3 -m venv .venv"}),
        false,
        &HashSet::from(["bash".to_string(), "file_read".to_string()]),
    ));
    assert!(ConversationLoop::bash_allowed_at_action_checkpoint(
        &serde_json::json!({"command": "python3 fixtures/core_quality/long_output/generate_log.py > fixtures/core_quality/long_output/output.log"}),
        false,
        &HashSet::from(["bash".to_string(), "file_read".to_string()]),
    ));
    assert!(ConversationLoop::bash_allowed_at_action_checkpoint(
        &serde_json::json!({"command": "python3 -m venv .venv"}),
        false,
        &HashSet::from([
            "bash".to_string(),
            "file_read".to_string(),
            "file_edit".to_string()
        ]),
    ));
}

#[test]
fn focused_repair_prompt_allows_one_targeted_read_without_broad_tools() {
    let exposed = vec![
        "file_edit".to_string(),
        "file_read".to_string(),
        "grep".to_string(),
    ];

    let prompt = ConversationLoop::focused_repair_mode_prompt(&exposed, 0);

    assert!(prompt.contains("Up to 2 targeted file_read/grep lookups remain"));
    assert!(prompt.contains("Do not call glob/project_list"));
    assert!(prompt.contains("using file_edit/file_write/file_patch so permission"));
    assert!(prompt.contains("Do not use bash for patching"));
    assert!(!prompt.contains("Do not call grep/glob/file_read/project_list"));

    let prompt_after_one_lookup = ConversationLoop::focused_repair_mode_prompt(&exposed, 1);
    assert!(prompt_after_one_lookup.contains("One targeted file_read/grep lookup remains"));

    let prompt_after_budget = ConversationLoop::focused_repair_mode_prompt(&exposed, 2);
    assert!(prompt_after_budget.contains("targeted lookup budget has already been used"));
    assert!(prompt_after_budget.contains("do not call file_read/grep again"));
}

#[test]
fn file_edit_failure_correction_prefers_line_range_retry() {
    let correction = ConversationLoop::file_edit_failure_repair_correction(&[r#"
file_edit call_1 failed:
Expected 1 occurrence(s) of old_string, but found 1487.
  ... showing first 12 of 1487 matches. The old_string is too broad.
"#
    .to_string()])
    .expect("ambiguous file_edit should produce a correction");

    assert!(correction.contains("line_start, line_end"));
    assert!(correction.contains("Do not retry the same broad or stale old_string"));
    assert!(correction.contains("not close out"));
}

#[test]
fn file_edit_failure_correction_handles_old_string_not_found() {
    let correction = ConversationLoop::file_edit_failure_repair_correction(&[r#"
file_edit call_1 failed:
Could not find old_string in file. Make sure it matches exactly (including whitespace).
"#
    .to_string()])
    .expect("missing old_string should produce a correction");

    assert!(correction.contains("old_string was not found"));
    assert!(correction.contains("re-read the target at most once"));
    assert!(correction.contains("latest line numbers"));
    assert!(correction.contains("not close out"));
}

#[test]
fn file_edit_failure_correction_gets_one_model_retry_before_synthesis() {
    assert!(
        ConversationLoop::should_retry_after_file_edit_failure_correction(true, true, false, false,)
    );
    assert!(
        !ConversationLoop::should_retry_after_file_edit_failure_correction(true, true, true, false,)
    );
    assert!(
        !ConversationLoop::should_retry_after_file_edit_failure_correction(true, true, false, true,)
    );
    assert!(
        !ConversationLoop::should_retry_after_file_edit_failure_correction(
            false, true, false, false,
        )
    );
}

#[test]
fn action_checkpoint_unexposed_tool_message_lists_allowed_tools() {
    let exposed = HashSet::from([
        "file_edit".to_string(),
        "file_read".to_string(),
        "grep".to_string(),
    ]);

    let message =
        ConversationLoop::action_checkpoint_unexposed_tool_message("project_list", &exposed, 0);

    assert!(message.contains("project_list"));
    assert!(message.contains("Exposed tools: file_edit, file_read, grep"));
    assert!(message.contains("Use file_edit/file_write/file_patch for patches"));
    assert!(message.contains("lookup budget still has room"));
    assert!(message.contains("Up to 2 targeted file_read/grep lookups remain"));

    let exhausted =
        ConversationLoop::action_checkpoint_unexposed_tool_message("file_read", &exposed, 2);
    assert!(exhausted.contains("targeted lookup budget has already been used"));
}

#[test]
fn focused_repair_action_proposal_records_budget_and_fallback_reason() {
    let exposed = HashSet::from([
        "grep".to_string(),
        "file_edit".to_string(),
        "file_read".to_string(),
    ]);

    let proposal = ConversationLoop::focused_repair_action_proposal(FocusedRepairActionRequest {
        action_checkpoint_active: true,
        any_tool_success: false,
        batch_has_unsuccessful_tools: true,
        failed_tool_evidence_present: true,
        force_patch_synthesis_after_no_change: false,
        force_patch_synthesis_reason: None,
        action_checkpoint_no_change_rounds: 0,
        action_checkpoint_lookup_count: 1,
        exposed_tool_names: &exposed,
    })
    .expect("focused repair failure should propose a recovery action");

    assert!(!proposal.enter_patch_synthesis);
    assert_eq!(proposal.next_no_change_rounds, 1);
    assert_eq!(proposal.fallback_owner, "action_checkpoint");
    assert_eq!(
        proposal.fallback_reason,
        "repeated invalid tools in focused repair"
    );
    assert!(proposal.reminder.contains("file_edit, file_read, grep"));
    assert!(proposal
        .reminder
        .contains("One targeted file_read/grep lookup remains"));
}

#[test]
fn focused_repair_action_proposal_enters_patch_synthesis_after_budget() {
    let exposed = HashSet::from(["file_edit".to_string()]);

    let proposal = ConversationLoop::focused_repair_action_proposal(FocusedRepairActionRequest {
        action_checkpoint_active: true,
        any_tool_success: true,
        batch_has_unsuccessful_tools: false,
        failed_tool_evidence_present: false,
        force_patch_synthesis_after_no_change: true,
        force_patch_synthesis_reason: Some("focused repair lookup budget exhausted"),
        action_checkpoint_no_change_rounds: 1,
        action_checkpoint_lookup_count: 2,
        exposed_tool_names: &exposed,
    })
    .expect("forced no-change repair should propose patch synthesis");

    assert!(proposal.enter_patch_synthesis);
    assert_eq!(proposal.next_no_change_rounds, 2);
    assert_eq!(
        proposal.fallback_reason,
        "focused repair lookup budget exhausted"
    );
    assert!(proposal
        .trace_error
        .contains("focused repair lookup budget exhausted"));
    assert!(proposal
        .reminder
        .contains("targeted lookup budget has already been used"));
}

#[test]
fn action_checkpoint_rejects_multi_replacement_file_edit() {
    let tmp = tempdir().expect("create temp dir");
    let src = tmp.path().join("src");
    std::fs::create_dir_all(&src).expect("create src");
    std::fs::write(
        src.join("lib.rs"),
        "let status = true;\nlet status = false;\n",
    )
    .expect("write file");

    let rejection = ConversationLoop::action_checkpoint_file_edit_rejection(
        &serde_json::json!({
            "path": "src/lib.rs",
            "old_string": "let status",
            "new_string": "let checked_status",
            "expected_replacements": 2
        }),
        tmp.path(),
    )
    .expect("multi replacement edit should be rejected");

    assert!(rejection.contains("only permits one replacement"));
}

#[test]
fn action_checkpoint_rejects_non_unique_anchor() {
    let tmp = tempdir().expect("create temp dir");
    let src = tmp.path().join("src");
    std::fs::create_dir_all(&src).expect("create src");
    std::fs::write(
        src.join("lib.rs"),
        "let status = true;\nlet status = false;\n",
    )
    .expect("write file");

    let rejection = ConversationLoop::action_checkpoint_file_edit_rejection(
        &serde_json::json!({
            "path": "src/lib.rs",
            "old_string": "let status",
            "new_string": "let checked_status"
        }),
        tmp.path(),
    )
    .expect("non-unique anchor should be rejected");

    assert!(rejection.contains("unique edit anchor"));
}

#[test]
fn action_checkpoint_rejects_multi_line_range_edit() {
    let tmp = tempdir().expect("create temp dir");
    let src = tmp.path().join("src");
    std::fs::create_dir_all(&src).expect("create src");
    std::fs::write(
            src.join("lib.rs"),
            "let write_decision = score();\nlet score = write_decision.score;\nlet status = write_decision.status;\n",
        )
        .expect("write file");

    let rejection = ConversationLoop::action_checkpoint_file_edit_rejection(
        &serde_json::json!({
            "path": "src/lib.rs",
            "line_start": 1,
            "line_end": 3,
            "new_string": "let status = write_decision.status;"
        }),
        tmp.path(),
    )
    .expect("multi-line action checkpoint edit should be rejected");

    assert!(rejection.contains("exactly one line"));
}

#[test]
fn action_checkpoint_accepts_unique_anchor() {
    let tmp = tempdir().expect("create temp dir");
    let src = tmp.path().join("src");
    std::fs::create_dir_all(&src).expect("create src");
    std::fs::write(
        src.join("lib.rs"),
        "let status = true;\nlet other = false;\n",
    )
    .expect("write file");

    let rejection = ConversationLoop::action_checkpoint_file_edit_rejection(
        &serde_json::json!({
            "path": "src/lib.rs",
            "old_string": "let status = true;",
            "new_string": "let status = false;"
        }),
        tmp.path(),
    );

    assert!(rejection.is_none(), "{rejection:?}");
}
