# Live Eval Report: backend-todo-api-crud

- Run id: `flow-rerun-backend-20260527-095340`
- Sample: `evalsets/live_tasks/backend-todo-api-crud.yaml`
- Worktree: `target/live-evals/flow-rerun-backend-20260527-095340/backend-todo-api-crud/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/flow-rerun-backend-20260527-095340/backend-todo-api-crud/env`
- Test status: `ok`
- Generated: `2026-05-27 09:56:07 +0800`

## Git Status

```text
 M fixtures/live_backend/todo_api/todo_api.py
```

## Diff Stat

```text
 fixtures/live_backend/todo_api/todo_api.py | 84 +++++++++++++++++++++++++-----
 1 file changed, 70 insertions(+), 14 deletions(-)
```

## Required Commands

```text
$ python3 -m unittest discover -s fixtures/live_backend/todo_api -p 'test_*.py'
..
----------------------------------------------------------------------
Ran 2 tests in 0.510s

OK
[exit status: 0]

$ ! rg 'TODO' fixtures/live_backend/todo_api/todo_api.py
[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-flow-rerun-backend-20260527-095340/backend-todo-api-crud/agent-output.md`
- Events: `docs/benchmarks/live-flow-rerun-backend-20260527-095340/backend-todo-api-crud/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
runtime_diagnostic: 1
start: 1
text_chunk: 1
tool_execution_complete: 21
tool_execution_progress: 8
tool_execution_start: 21
trace_summary: 1
```

Quality signals:

```text
output_chars: 1128
diff_chars: 4577
diff_files_changed: 1
tool_executions: 21
first_write_tool_index: 11
forbidden_tool_uses: none
tool_errors: 0
tool_failures: 0
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 254
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
closeout_tool_records: 118
closeout_tool_evidence: tool evidence: records=118 completed=21 failed=97 denied=0 validation=0 closeout=8 repair=105 changed=8 workflows=code_change commands=none
runtime_diet: prompt=21579 tool_schema=3950 tools=19 workflow=strict closeout=full validation=passed:2/2 recovered_failed:1
adaptive_triggers: risk_signal_high,required_validation,repeated_no_code_progress,first_code_change,verification_failed
risk_signal: entry=high runtime=high
risk_signal_reasons: required validation commands present
trace_event_types: reflection.pass,stage.validation,acceptance.review,memory.boundary,memory.sync,workflow.fallback,closeout,execution.report,memory.proposal,runtime.diet,completion.contract,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: seeded_code_change
behavior_assertions: none
behavior_assertion_status: none
output_assertions: none
output_assertion_status: none
output_assertion_missing: none
trajectory_assertions: none
trajectory_assertion_status: none
trajectory_assertion_missing: none
runtime_spine: coverage=6/7, status=none, missing=none
runtime_profile: none
mva_profile_active: false
runtime_spine_detail: context=46 latest=runtime_diet_report decision=52 latest=action_reviewed permission=0 latest=none tool_execution=50 latest=tool_completed state_update=82 latest=workflow_fallback verification=16 latest=acceptance_review_completed closeout=3 latest=assistant_responded risky_tool_runs=8 risky_tool_reviewed=8 risky_tool_missing_action_review=none gate_outcomes=total=22, protective_block=0, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=22 stop_reason=consecutive_edit_failures stop_terminal_status=blocked stop_action=recover stop_failure_type=edit_failure rollback_recommended=false rollback_completed=false recovery_failure_types=code_change_no_diff_after_repeated_progress recovery_kinds=code_change_no_diff_replan route_recovery=events=2, read_search=false, mutation_blocked=false, safety=true action_scores=19 latest_action_score=17 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=none runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=8 provider_protocol_repairs=338 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=4 context_zone_duplicate_blocks_removed=0 context_zone_provenance_markers=3 agent_loop_steps=16 context_zones=8 completion_contract=completed
runtime_spine_trace_present: true
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: none
runtime_spine_status: none
runtime_spine_missing: none
risky_tool_runs: 8
risky_tool_reviewed: 8
risky_tool_missing_action_review: none
gate_outcomes: total=22, protective_block=0, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=22
gate_outcome_records: action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,+10
gate_outcome_total: 22
gate_outcome_protective_blocks: 0
gate_outcome_recoverable_friction: 0
gate_outcome_unrecovered_blocks: 0
gate_outcome_suspected_false_positives: 0
gate_outcome_policy_correct_but_ux_costly: 0
gate_outcome_harmless_passes: 22
gate_outcome_failure_owners: none
route_recovery: events=2, read_search=false, mutation_blocked=false, safety=true
route_recovery_events: 2
route_recovery_failure_types: code_change_no_diff_after_repeated_progress
route_recovery_kinds: code_change_no_diff_replan
route_recovery_read_search_expanded: false
route_recovery_mutation_blocked: false
route_recovery_safety_monotonic: true
route_recovery_unsafe_mutation_expansion: false
agent_loop_steps: 16
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
verification_proof_summary: required validation passed 2/2 commands
verification_proof_kinds: command_passed,required_validation_passed
verification_proof_support_status: verified
verification_proof_support_summary: verified by command_passed,required_validation_passed
verification_proof_supports_verified: true
verification_proof_residual_risk: false
premature_edit_count: 0
evidence_before_first_edit: true
scope_drift_count: 0
invalid_action_count: 14
repeated_action_count: 14
failed_action_count: 0
user_question_count: 0
unnecessary_question_count: 0
verification_attempted: true
verification_passed: true
tool_call_count: 21
llm_call_count: 8
warning: earlier_verification_failed_before_repair
warning: earlier_stage_validation_failed_before_repair
failure_owner: none
outcome_score: 100
process_score: 60
efficiency_score: 80
agent_score: 84
score_penalties: repeated_action,invalid_action,repeated_actions
```

Specialty signals:

```text
memory_active: true
automation_active: true
guided_debugging_active: true
guided_reasoning_active: false
weighted_planning_active: false
closeout_active: true
adaptive_workflow_active: true
active_specialty_signals: 5/7
workflow_contract_activation: entry=active:force repair=active_after_failure
workflow_contract_events: 1
runtime_spine: coverage=6/7, status=none, missing=none
runtime_profile: none
mva_profile_active: false
runtime_spine_detail: context=46 latest=runtime_diet_report decision=52 latest=action_reviewed permission=0 latest=none tool_execution=50 latest=tool_completed state_update=82 latest=workflow_fallback verification=16 latest=acceptance_review_completed closeout=3 latest=assistant_responded risky_tool_runs=8 risky_tool_reviewed=8 risky_tool_missing_action_review=none gate_outcomes=total=22, protective_block=0, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=22 stop_reason=consecutive_edit_failures stop_terminal_status=blocked stop_action=recover stop_failure_type=edit_failure rollback_recommended=false rollback_completed=false recovery_failure_types=code_change_no_diff_after_repeated_progress recovery_kinds=code_change_no_diff_replan route_recovery=events=2, read_search=false, mutation_blocked=false, safety=true action_scores=19 latest_action_score=17 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=none runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=8 provider_protocol_repairs=338 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=4 context_zone_duplicate_blocks_removed=0 context_zone_provenance_markers=3 agent_loop_steps=16 context_zones=8 completion_contract=completed
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: none
runtime_spine_status: none
runtime_spine_missing: none
risky_tool_runs: 8
risky_tool_reviewed: 8
risky_tool_missing_action_review: none
gate_outcomes: total=22, protective_block=0, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=22
gate_outcome_records: action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,+10
gate_outcome_total: 22
gate_outcome_protective_blocks: 0
gate_outcome_recoverable_friction: 0
gate_outcome_unrecovered_blocks: 0
gate_outcome_suspected_false_positives: 0
gate_outcome_policy_correct_but_ux_costly: 0
gate_outcome_harmless_passes: 22
gate_outcome_failure_owners: none
agent_loop_steps: 16
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
verification_proof_summary: required validation passed 2/2 commands
verification_proof_kinds: command_passed,required_validation_passed
verification_proof_support_status: verified
verification_proof_support_summary: verified by command_passed,required_validation_passed
verification_proof_supports_verified: true
verification_proof_residual_risk: false
risk_signal: entry=high runtime=high
risk_signal_reasons: required validation commands present
memory_sync_events: 8
memory_tool_calls: 0
retrieval_sources: Project
memory_candidate_typed: true
memory_candidate_has_evidence: true
memory_proposal_recorded: true
memory_proposal_status: proposed
memory_proposal_candidates: 1
memory_proposal_kinds: successful_fix
memory_proposal_evidence_items: 7
memory_proposal_write_policy: review_required
memory_proposal_write_performed: false
memory_record_used: false
memory_use_count_updated: false
memory_failure_lesson_promoted: false
memory_action_weight_changed: false
memory_stale_demoted: false
memory_scope_correct: false
required_commands: 2
agent_required_commands: 2
harness_commands: 0
required_command_status: ok
validation_events: 4
stage_validation_events: 4
tool_progress_events: 8
guided_debugging_events: 2
guided_reasoning_events: 0
workflow_plan_events: 0
weighted_plan_events: 0
reweighted_plan_events: 0
adaptive_trigger_events: 5
adaptive_triggers: risk_signal_high,required_validation,repeated_no_code_progress,first_code_change,verification_failed
latest_top_priority: none
latest_top_importance_score: none
latest_top_weight_share: none
acceptance_accepted: True
closeout_status: passed
closeout_tool_records: 118
closeout_tool_evidence: tool evidence: records=118 completed=21 failed=97 denied=0 validation=0 closeout=8 repair=105 changed=8 workflows=code_change commands=none
runtime_diet: prompt=21579 tool_schema=3950 tools=19 workflow=strict
```

Agent stderr tail:

```text
2026-05-27T01:55:05.446583Z  WARN priority_agent::engine::conversation_loop::repair_controller: Guided validation debugging failed: missing field `symptom`
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

- Bundle: `docs/benchmarks/live-flow-rerun-backend-20260527-095340/backend-todo-api-crud/run-bundle`
- Task: `docs/benchmarks/live-flow-rerun-backend-20260527-095340/backend-todo-api-crud/run-bundle/task.json`
- Steps: `docs/benchmarks/live-flow-rerun-backend-20260527-095340/backend-todo-api-crud/run-bundle/steps.jsonl`
- Events: `docs/benchmarks/live-flow-rerun-backend-20260527-095340/backend-todo-api-crud/run-bundle/events.jsonl`
- Final report: `docs/benchmarks/live-flow-rerun-backend-20260527-095340/backend-todo-api-crud/run-bundle/final_report.md`
