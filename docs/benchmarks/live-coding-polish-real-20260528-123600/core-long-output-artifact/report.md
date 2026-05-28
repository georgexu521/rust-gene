# Live Eval Report: core-long-output-artifact

- Run id: `coding-polish-real-20260528-123600`
- Sample: `evalsets/live_tasks/core-long-output-artifact.yaml`
- Worktree: `target/live-evals/coding-polish-real-20260528-123600/core-long-output-artifact/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/coding-polish-real-20260528-123600/core-long-output-artifact/env`
- Test status: `ok`
- Generated: `2026-05-28 12:59:15 +0800`

## Git Status

```text
?? fixtures/core_quality/long_output/output.log
```

## Diff Stat

```text
 .../core_quality/long_output/output.log            | 800 +++++++++++++++++++++
 1 file changed, 800 insertions(+)
```

## Required Commands

```text
$ test -s fixtures/core_quality/long_output/output.log
[exit status: 0]

$ rg 'line 0537 ERROR_ANCHOR payment retry budget exceeded' fixtures/core_quality/long_output/output.log
line 0537 ERROR_ANCHOR payment retry budget exceeded
[exit status: 0]

$ python3 -c 'from pathlib import Path; assert len(Path("fixtures/core_quality/long_output/output.log").read_text().splitlines()) == 800'
[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-coding-polish-real-20260528-123600/core-long-output-artifact/agent-output.md`
- Events: `docs/benchmarks/live-coding-polish-real-20260528-123600/core-long-output-artifact/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
permission_request: 1
runtime_diagnostic: 1
start: 1
text_chunk: 1
tool_execution_complete: 2
tool_execution_start: 2
trace_summary: 1
```

Quality signals:

```text
output_chars: 1319
diff_chars: 11474
diff_files_changed: 1
diff_files_changed_raw: 1
generated_dependency_files_ignored: 0
tool_executions: 2
first_write_tool_index: none
forbidden_tool_uses: none
tool_errors: 1
tool_failures: 1
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 69
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
closeout_tool_records: 3
closeout_tool_evidence: tool evidence: records=3 completed=1 failed=1 denied=1 validation=0 closeout=1 repair=2 changed=0 workflows=code_change commands=python3 fixtures/core_quality/long_output/generate_log.py > fixtures/core_quality/long_output/output.log 2>&1
runtime_diet: prompt=4571 tool_schema=3950 tools=19 workflow=guarded closeout=full validation=passed:3/3
adaptive_triggers: risk_signal_high,required_validation,first_code_change
risk_signal: entry=high runtime=high
risk_signal_reasons: required validation commands present; runtime risk keyword in request: runtime
trace_event_types: stage.validation,acceptance.review,workflow.plan,memory.boundary,workflow.fallback,closeout,execution.report,memory.proposal,memory.proposal,runtime.diet,completion.contract,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: audit_or_regression_check
behavior_assertions: none
behavior_assertion_status: none
output_assertions: none
output_assertion_status: none
output_assertion_missing: none
trajectory_assertions: none
trajectory_assertion_status: none
trajectory_assertion_missing: none
runtime_spine: coverage=7/7, status=none, missing=none
runtime_profile: none
mva_profile_active: false
runtime_spine_detail: context=14 latest=runtime_diet_report decision=14 latest=workflow_plan_progress permission=2 latest=permission_resolved tool_execution=6 latest=tool_completed state_update=18 latest=workflow_fallback verification=6 latest=acceptance_review_completed closeout=3 latest=assistant_responded risky_tool_runs=1 risky_tool_reviewed=1 risky_tool_missing_action_review=none gate_outcomes=total=4, protective_block=0, recoverable_friction=2, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=2 stop_reason=consecutive_permission_blocks stop_terminal_status=needs_user stop_action=ask_user stop_failure_type=permission_block rollback_recommended=false rollback_completed=false recovery_failure_types=permission_block recovery_kinds=ask_user_or_choose_safer_path route_recovery=events=0, read_search=false, mutation_blocked=false, safety=missing action_scores=1 latest_action_score=16 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=user_once_reject runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=2 provider_protocol_repairs=4 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=4 context_zone_duplicate_blocks_removed=0 context_zone_provenance_markers=3 agent_loop_steps=4 context_zones=2 completion_contract=completed
runtime_spine_trace_present: true
runtime_spine_phase_coverage: 7/7
runtime_spine_observed_phases: context,decision,permission,tool_execution,state_update,verification,closeout
runtime_spine_assertions: none
runtime_spine_status: none
runtime_spine_missing: none
risky_tool_runs: 1
risky_tool_reviewed: 1
risky_tool_missing_action_review: none
gate_outcomes: total=4, protective_block=0, recoverable_friction=2, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=2
gate_outcome_records: action_review:allow:harmless_pass,action_review:ask_user:recoverable_friction,permission:reject_once:recoverable_friction,closeout:passed:harmless_pass
gate_outcome_total: 4
gate_outcome_protective_blocks: 0
gate_outcome_recoverable_friction: 2
gate_outcome_unrecovered_blocks: 0
gate_outcome_suspected_false_positives: 0
gate_outcome_policy_correct_but_ux_costly: 0
gate_outcome_harmless_passes: 2
gate_outcome_failure_owners: none
route_recovery: events=0, read_search=false, mutation_blocked=false, safety=missing
route_recovery_events: 0
route_recovery_failure_types: none
route_recovery_kinds: none
route_recovery_read_search_expanded: false
route_recovery_mutation_blocked: false
route_recovery_safety_monotonic: missing
route_recovery_unsafe_mutation_expansion: false
agent_loop_steps: 4
context_zones_materialized: true
context_zone_task_state_empty: false
context_zone_current_decision_request_empty: false
context_zone_envelope_messages: 1
context_zone_source_messages: 4
context_zone_duplicate_blocks_removed: 0
context_zone_provenance_markers: 3
state_transition_recorded: false
completion_contract_status: completed
completion_contract_proof_status: verified
candidate_score_calibrated: false
candidate_score_disagreement: false
observer_outcome_recorded: true
memory_boundary_recorded: true
verification_proof_status: verified
verification_proof_summary: required validation passed 3/3 commands
verification_proof_kinds: command_passed,required_validation_passed
verification_proof_support_status: verified
verification_proof_support_summary: verified by command_passed,required_validation_passed
verification_proof_supports_verified: true
verification_proof_residual_risk: false
premature_edit_count: 0
evidence_before_first_edit: true
scope_drift_count: 0
invalid_action_count: 0
repeated_action_count: 0
failed_action_count: 2
user_question_count: 2
unnecessary_question_count: 0
verification_attempted: true
verification_passed: true
tool_call_count: 2
llm_call_count: 2
warning: tool_errors_seen
failure_owner: none
outcome_score: 100
process_score: 100
efficiency_score: 74
agent_score: 95
score_penalties: failed_actions,user_questions
```

Specialty signals:

```text
memory_active: false
automation_active: true
guided_debugging_active: true
guided_reasoning_active: false
weighted_planning_active: true
closeout_active: true
adaptive_workflow_active: true
active_specialty_signals: 5/7
workflow_contract_activation: entry=active:force repair=active_after_failure
workflow_contract_events: 1
runtime_spine: coverage=7/7, status=none, missing=none
runtime_profile: none
mva_profile_active: false
runtime_spine_detail: context=14 latest=runtime_diet_report decision=14 latest=workflow_plan_progress permission=2 latest=permission_resolved tool_execution=6 latest=tool_completed state_update=18 latest=workflow_fallback verification=6 latest=acceptance_review_completed closeout=3 latest=assistant_responded risky_tool_runs=1 risky_tool_reviewed=1 risky_tool_missing_action_review=none gate_outcomes=total=4, protective_block=0, recoverable_friction=2, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=2 stop_reason=consecutive_permission_blocks stop_terminal_status=needs_user stop_action=ask_user stop_failure_type=permission_block rollback_recommended=false rollback_completed=false recovery_failure_types=permission_block recovery_kinds=ask_user_or_choose_safer_path route_recovery=events=0, read_search=false, mutation_blocked=false, safety=missing action_scores=1 latest_action_score=16 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=user_once_reject runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=2 provider_protocol_repairs=4 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=4 context_zone_duplicate_blocks_removed=0 context_zone_provenance_markers=3 agent_loop_steps=4 context_zones=2 completion_contract=completed
runtime_spine_phase_coverage: 7/7
runtime_spine_observed_phases: context,decision,permission,tool_execution,state_update,verification,closeout
runtime_spine_assertions: none
runtime_spine_status: none
runtime_spine_missing: none
risky_tool_runs: 1
risky_tool_reviewed: 1
risky_tool_missing_action_review: none
gate_outcomes: total=4, protective_block=0, recoverable_friction=2, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=2
gate_outcome_records: action_review:allow:harmless_pass,action_review:ask_user:recoverable_friction,permission:reject_once:recoverable_friction,closeout:passed:harmless_pass
gate_outcome_total: 4
gate_outcome_protective_blocks: 0
gate_outcome_recoverable_friction: 2
gate_outcome_unrecovered_blocks: 0
gate_outcome_suspected_false_positives: 0
gate_outcome_policy_correct_but_ux_costly: 0
gate_outcome_harmless_passes: 2
gate_outcome_failure_owners: none
agent_loop_steps: 4
context_zones_materialized: true
context_zone_task_state_empty: false
context_zone_current_decision_request_empty: false
context_zone_envelope_messages: 1
context_zone_source_messages: 4
context_zone_duplicate_blocks_removed: 0
context_zone_provenance_markers: 3
state_transition_recorded: false
completion_contract_status: completed
completion_contract_proof_status: verified
candidate_score_calibrated: false
candidate_score_disagreement: false
observer_outcome_recorded: true
memory_boundary_recorded: true
verification_proof_status: verified
verification_proof_summary: required validation passed 3/3 commands
verification_proof_kinds: command_passed,required_validation_passed
verification_proof_support_status: verified
verification_proof_support_summary: verified by command_passed,required_validation_passed
verification_proof_supports_verified: true
verification_proof_residual_risk: false
risk_signal: entry=high runtime=high
risk_signal_reasons: required validation commands present; runtime risk keyword in request: runtime
memory_sync_events: 0
memory_tool_calls: 0
retrieval_sources: Project
memory_candidate_typed: true
memory_candidate_has_evidence: true
memory_proposal_recorded: true
memory_proposal_status: proposed
memory_proposal_candidates: 1
memory_proposal_kinds: validation_baseline
memory_proposal_evidence_items: 10
memory_proposal_write_policy: review_required
memory_proposal_write_performed: false
memory_record_used: false
memory_use_count_updated: false
memory_failure_lesson_promoted: false
memory_action_weight_changed: false
memory_stale_demoted: false
memory_scope_correct: false
required_commands: 3
agent_required_commands: 3
harness_commands: 0
required_command_status: ok
validation_events: 1
stage_validation_events: 1
tool_progress_events: 0
guided_debugging_events: 1
guided_reasoning_events: 0
workflow_plan_events: 2
weighted_plan_events: 1
reweighted_plan_events: 1
adaptive_trigger_events: 3
adaptive_triggers: risk_signal_high,required_validation,first_code_change
latest_top_priority: P3
latest_top_importance_score: 0.20499999821186066
latest_top_weight_share: 0.2578616440296173
acceptance_accepted: True
closeout_status: passed
closeout_tool_records: 3
closeout_tool_evidence: tool evidence: records=3 completed=1 failed=1 denied=1 validation=0 closeout=1 repair=2 changed=0 workflows=code_change commands=python3 fixtures/core_quality/long_output/generate_log.py > fixtures/core_quality/long_output/output.log 2>&1
runtime_diet: prompt=4571 tool_schema=3950 tools=19 workflow=guarded
```

## Human Review

- accepted: TODO
- task_success: TODO
- mainline_hit: TODO
- plan_coverage: TODO
- rework_count: TODO
- tool_efficiency: TODO
- diff_discipline: TODO
- closeout_accuracy: TODO
- notes: TODO

## Run Bundle

- Bundle: `docs/benchmarks/live-coding-polish-real-20260528-123600/core-long-output-artifact/run-bundle`
- Task: `docs/benchmarks/live-coding-polish-real-20260528-123600/core-long-output-artifact/run-bundle/task.json`
- Steps: `docs/benchmarks/live-coding-polish-real-20260528-123600/core-long-output-artifact/run-bundle/steps.jsonl`
- Events: `docs/benchmarks/live-coding-polish-real-20260528-123600/core-long-output-artifact/run-bundle/events.jsonl`
- Final report: `docs/benchmarks/live-coding-polish-real-20260528-123600/core-long-output-artifact/run-bundle/final_report.md`
