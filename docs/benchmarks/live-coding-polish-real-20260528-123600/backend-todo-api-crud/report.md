# Live Eval Report: backend-todo-api-crud

- Run id: `coding-polish-real-20260528-123600`
- Sample: `evalsets/live_tasks/backend-todo-api-crud.yaml`
- Worktree: `target/live-evals/coding-polish-real-20260528-123600/backend-todo-api-crud/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/coding-polish-real-20260528-123600/backend-todo-api-crud/env`
- Test status: `ok`
- Generated: `2026-05-28 12:43:02 +0800`

## Git Status

```text
 M fixtures/live_backend/todo_api/todo_api.py
```

## Diff Stat

```text
 fixtures/live_backend/todo_api/todo_api.py | 85 +++++++++++++++++++++++++-----
 1 file changed, 71 insertions(+), 14 deletions(-)
```

## Required Commands

```text
$ python3 -m unittest discover -s fixtures/live_backend/todo_api -p 'test_*.py'
..
----------------------------------------------------------------------
Ran 2 tests in 0.511s

OK
[exit status: 0]

$ ! rg 'TODO' fixtures/live_backend/todo_api/todo_api.py
[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-coding-polish-real-20260528-123600/backend-todo-api-crud/agent-output.md`
- Events: `docs/benchmarks/live-coding-polish-real-20260528-123600/backend-todo-api-crud/agent-events.jsonl`
- Monitor: `docs/benchmarks/live-coding-polish-real-20260528-123600/backend-todo-api-crud/agent-monitor.log`

Event counts:

```text
complete: 1
eval_started: 1
runtime_diagnostic: 1
start: 1
text_chunk: 1
tool_execution_complete: 8
tool_execution_progress: 6
tool_execution_start: 8
trace_summary: 1
```

Quality signals:

```text
output_chars: 1399
diff_chars: 4548
diff_files_changed: 1
diff_files_changed_raw: 1
generated_dependency_files_ignored: 0
tool_executions: 8
first_write_tool_index: 3
forbidden_tool_uses: none
tool_errors: 0
tool_failures: 3
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 155
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
closeout_tool_records: 31
closeout_tool_evidence: tool evidence: records=31 completed=8 failed=23 denied=0 validation=0 closeout=6 repair=29 changed=6 workflows=code_change commands=none
runtime_diet: prompt=15803 tool_schema=3950 tools=19 workflow=strict closeout=full validation=passed:2/2 recovered_failed:2
adaptive_triggers: risk_signal_high,required_validation,first_code_change,verification_failed,acceptance_rejected
risk_signal: entry=high runtime=high
risk_signal_reasons: required validation commands present
trace_event_types: stage.validation,acceptance.review,workflow.plan,memory.boundary,workflow.fallback,closeout,execution.report,memory.proposal,memory.proposal,runtime.diet,completion.contract,assistant
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
runtime_spine_detail: context=24 latest=runtime_diet_report decision=33 latest=workflow_plan_progress permission=0 latest=none tool_execution=27 latest=tool_completed state_update=52 latest=workflow_fallback verification=10 latest=acceptance_review_completed closeout=3 latest=assistant_responded risky_tool_runs=9 risky_tool_reviewed=9 risky_tool_missing_action_review=none gate_outcomes=total=12, protective_block=0, recoverable_friction=3, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=9 stop_reason=consecutive_permission_blocks stop_terminal_status=needs_user stop_action=ask_user stop_failure_type=permission_block rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none route_recovery=events=0, read_search=false, mutation_blocked=false, safety=missing action_scores=9 latest_action_score=16 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=none runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=5 provider_protocol_repairs=92 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=5 context_zone_duplicate_blocks_removed=0 context_zone_provenance_markers=3 agent_loop_steps=10 context_zones=5 completion_contract=completed
runtime_spine_trace_present: true
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: none
runtime_spine_status: none
runtime_spine_missing: none
risky_tool_runs: 9
risky_tool_reviewed: 9
risky_tool_missing_action_review: none
gate_outcomes: total=12, protective_block=0, recoverable_friction=3, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=9
gate_outcome_records: action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:revise:recoverable_friction,action_review:revise:recoverable_friction,action_review:revise:recoverable_friction,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,closeout:passed:harmless_pass
gate_outcome_total: 12
gate_outcome_protective_blocks: 0
gate_outcome_recoverable_friction: 3
gate_outcome_unrecovered_blocks: 0
gate_outcome_suspected_false_positives: 0
gate_outcome_policy_correct_but_ux_costly: 0
gate_outcome_harmless_passes: 9
gate_outcome_failure_owners: none
route_recovery: events=0, read_search=false, mutation_blocked=false, safety=missing
route_recovery_events: 0
route_recovery_failure_types: none
route_recovery_kinds: none
route_recovery_read_search_expanded: false
route_recovery_mutation_blocked: false
route_recovery_safety_monotonic: missing
route_recovery_unsafe_mutation_expansion: false
agent_loop_steps: 10
context_zones_materialized: true
context_zone_task_state_empty: false
context_zone_current_decision_request_empty: false
context_zone_envelope_messages: 1
context_zone_source_messages: 5
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
invalid_action_count: 5
repeated_action_count: 5
failed_action_count: 3
user_question_count: 1
unnecessary_question_count: 0
verification_attempted: true
verification_passed: true
tool_call_count: 8
llm_call_count: 5
warning: earlier_verification_failed_before_repair
warning: earlier_stage_validation_failed_before_repair
failure_owner: none
outcome_score: 100
process_score: 60
efficiency_score: 51
agent_score: 78
score_penalties: repeated_action,invalid_action,failed_actions,repeated_actions,user_questions
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
runtime_spine: coverage=6/7, status=none, missing=none
runtime_profile: none
mva_profile_active: false
runtime_spine_detail: context=24 latest=runtime_diet_report decision=33 latest=workflow_plan_progress permission=0 latest=none tool_execution=27 latest=tool_completed state_update=52 latest=workflow_fallback verification=10 latest=acceptance_review_completed closeout=3 latest=assistant_responded risky_tool_runs=9 risky_tool_reviewed=9 risky_tool_missing_action_review=none gate_outcomes=total=12, protective_block=0, recoverable_friction=3, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=9 stop_reason=consecutive_permission_blocks stop_terminal_status=needs_user stop_action=ask_user stop_failure_type=permission_block rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none route_recovery=events=0, read_search=false, mutation_blocked=false, safety=missing action_scores=9 latest_action_score=16 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=none runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=5 provider_protocol_repairs=92 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=5 context_zone_duplicate_blocks_removed=0 context_zone_provenance_markers=3 agent_loop_steps=10 context_zones=5 completion_contract=completed
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: none
runtime_spine_status: none
runtime_spine_missing: none
risky_tool_runs: 9
risky_tool_reviewed: 9
risky_tool_missing_action_review: none
gate_outcomes: total=12, protective_block=0, recoverable_friction=3, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=9
gate_outcome_records: action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:revise:recoverable_friction,action_review:revise:recoverable_friction,action_review:revise:recoverable_friction,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,closeout:passed:harmless_pass
gate_outcome_total: 12
gate_outcome_protective_blocks: 0
gate_outcome_recoverable_friction: 3
gate_outcome_unrecovered_blocks: 0
gate_outcome_suspected_false_positives: 0
gate_outcome_policy_correct_but_ux_costly: 0
gate_outcome_harmless_passes: 9
gate_outcome_failure_owners: none
agent_loop_steps: 10
context_zones_materialized: true
context_zone_task_state_empty: false
context_zone_current_decision_request_empty: false
context_zone_envelope_messages: 1
context_zone_source_messages: 5
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
required_commands: 2
agent_required_commands: 2
harness_commands: 0
required_command_status: ok
validation_events: 2
stage_validation_events: 2
tool_progress_events: 6
guided_debugging_events: 1
guided_reasoning_events: 0
workflow_plan_events: 2
weighted_plan_events: 1
reweighted_plan_events: 1
adaptive_trigger_events: 5
adaptive_triggers: risk_signal_high,required_validation,first_code_change,verification_failed,acceptance_rejected
latest_top_priority: P1
latest_top_importance_score: 0.7849999666213989
latest_top_weight_share: 0.26837605237960815
acceptance_accepted: True
closeout_status: passed
closeout_tool_records: 31
closeout_tool_evidence: tool evidence: records=31 completed=8 failed=23 denied=0 validation=0 closeout=6 repair=29 changed=6 workflows=code_change commands=none
runtime_diet: prompt=15803 tool_schema=3950 tools=19 workflow=strict
```

Agent monitor tail:

```text
[2026-05-28T12:41:55+0800] agent-run still running elapsed=30s idle_for=5s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=9291
[2026-05-28T12:42:25+0800] agent-run still running elapsed=60s idle_for=20s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=14741
[2026-05-28T12:42:55+0800] agent-run still running elapsed=90s idle_for=0s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=40004
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

- Bundle: `docs/benchmarks/live-coding-polish-real-20260528-123600/backend-todo-api-crud/run-bundle`
- Task: `docs/benchmarks/live-coding-polish-real-20260528-123600/backend-todo-api-crud/run-bundle/task.json`
- Steps: `docs/benchmarks/live-coding-polish-real-20260528-123600/backend-todo-api-crud/run-bundle/steps.jsonl`
- Events: `docs/benchmarks/live-coding-polish-real-20260528-123600/backend-todo-api-crud/run-bundle/events.jsonl`
- Final report: `docs/benchmarks/live-coding-polish-real-20260528-123600/backend-todo-api-crud/run-bundle/final_report.md`
