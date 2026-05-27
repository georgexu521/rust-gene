# Live Eval Report: backend-todo-api-crud

- Run id: `flow-real-postc-20260527-134900`
- Sample: `evalsets/live_tasks/backend-todo-api-crud.yaml`
- Worktree: `target/live-evals/flow-real-postc-20260527-134900/backend-todo-api-crud/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/flow-real-postc-20260527-134900/backend-todo-api-crud/env`
- Test status: `failed`
- Generated: `2026-05-27 13:26:01 +0800`

## Git Status

```text
 M fixtures/live_backend/todo_api/todo_api.py
```

## Diff Stat

```text
 fixtures/live_backend/todo_api/todo_api.py | 68 +++++++++++++++++++++++++-----
 1 file changed, 58 insertions(+), 10 deletions(-)
```

## Required Commands

```text
$ python3 -m unittest discover -s fixtures/live_backend/todo_api -p 'test_*.py'
E
======================================================================
ERROR: test_todo_api (unittest.loader._FailedTest.test_todo_api)
----------------------------------------------------------------------
ImportError: Failed to import test module: test_todo_api
Traceback (most recent call last):
  File "/Library/Frameworks/Python.framework/Versions/3.12/lib/python3.12/unittest/loader.py", line 394, in _find_test_path
    module = self._get_module_from_name(name)
             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  File "/Library/Frameworks/Python.framework/Versions/3.12/lib/python3.12/unittest/loader.py", line 337, in _get_module_from_name
    __import__(name)
  File "/Users/georgexu/Desktop/rust-agent/target/live-evals/flow-real-postc-20260527-134900/backend-todo-api-crud/worktree/fixtures/live_backend/todo_api/test_todo_api.py", line 8, in <module>
    import todo_api
  File "/Users/georgexu/Desktop/rust-agent/target/live-evals/flow-real-postc-20260527-134900/backend-todo-api-crud/worktree/fixtures/live_backend/todo_api/todo_api.py", line 41
    if item["id"] == item_id:
IndentationError: unexpected indent


----------------------------------------------------------------------
Ran 1 test in 0.000s

FAILED (errors=1)
[exit status: 1]

$ ! rg 'TODO' fixtures/live_backend/todo_api/todo_api.py
        # TODO: implement PATCH /todos/<id>.
        # TODO: implement DELETE /todos/<id>.
[exit status: 1]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-flow-real-postc-20260527-134900/backend-todo-api-crud/agent-output.md`
- Events: `docs/benchmarks/live-flow-real-postc-20260527-134900/backend-todo-api-crud/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
runtime_diagnostic: 1
start: 1
text_chunk: 1
tool_execution_complete: 6
tool_execution_progress: 4
tool_execution_start: 6
trace_summary: 1
```

Quality signals:

```text
output_chars: 2701
diff_chars: 3600
diff_files_changed: 1
tool_executions: 6
first_write_tool_index: 3
forbidden_tool_uses: none
tool_errors: 0
tool_failures: 8
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 247
test_status: failed
verification_passed: false
stage_validation_passed: false
acceptance_accepted: False
closeout_status: failed
closeout_tool_records: 97
closeout_tool_evidence: tool evidence: records=97 completed=6 failed=91 denied=0 validation=0 closeout=4 repair=95 changed=4 workflows=code_change commands=none
runtime_diet: prompt=38034 tool_schema=3950 tools=19 workflow=strict closeout=full validation=failed:2/2
adaptive_triggers: risk_signal_high,required_validation,first_code_change,verification_failed,acceptance_rejected
risk_signal: entry=high runtime=high
risk_signal_reasons: required validation commands present
trace_event_types: agent.loop,context.zones,api.start,provider.protocol,workflow.fallback,api.done,closeout,execution.report,memory.proposal,runtime.diet,completion.contract,assistant
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
runtime_spine_detail: context=47 latest=runtime_diet_report decision=40 latest=action_reviewed permission=0 latest=none tool_execution=39 latest=api_request_completed state_update=97 latest=workflow_fallback verification=16 latest=acceptance_review_completed closeout=3 latest=assistant_responded risky_tool_runs=12 risky_tool_reviewed=12 risky_tool_missing_action_review=none gate_outcomes=total=15, protective_block=9, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=6 stop_reason=repeated_tool_failure stop_terminal_status=failed stop_action=recover stop_failure_type=unknown rollback_recommended=false rollback_completed=false recovery_failure_types=hidden_mutation_tool_requested recovery_kinds=no_silent_mutation_expansion route_recovery=events=1, read_search=false, mutation_blocked=true, safety=true action_scores=12 latest_action_score=21 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=none runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=11 provider_protocol_repairs=735 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=5 context_zone_duplicate_blocks_removed=0 context_zone_provenance_markers=3 agent_loop_steps=20 context_zones=11 completion_contract=failed
runtime_spine_trace_present: true
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: none
runtime_spine_status: none
runtime_spine_missing: none
risky_tool_runs: 12
risky_tool_reviewed: 12
risky_tool_missing_action_review: none
gate_outcomes: total=15, protective_block=9, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=6
gate_outcome_records: action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:revise:protective_block,action_review:revise:protective_block,action_review:revise:protective_block,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:revise:protective_block,action_review:revise:protective_block,action_review:revise:protective_block,action_review:allow:harmless_pass,+3
gate_outcome_total: 15
gate_outcome_protective_blocks: 9
gate_outcome_recoverable_friction: 0
gate_outcome_unrecovered_blocks: 0
gate_outcome_suspected_false_positives: 0
gate_outcome_policy_correct_but_ux_costly: 0
gate_outcome_harmless_passes: 6
gate_outcome_failure_owners: none
route_recovery: events=1, read_search=false, mutation_blocked=true, safety=true
route_recovery_events: 1
route_recovery_failure_types: hidden_mutation_tool_requested
route_recovery_kinds: no_silent_mutation_expansion
route_recovery_read_search_expanded: false
route_recovery_mutation_blocked: true
route_recovery_safety_monotonic: true
route_recovery_unsafe_mutation_expansion: false
agent_loop_steps: 20
context_zones_materialized: true
context_zone_task_state_empty: false
context_zone_current_decision_request_empty: false
context_zone_envelope_messages: 1
context_zone_source_messages: 5
context_zone_duplicate_blocks_removed: 0
context_zone_provenance_markers: 3
state_transition_recorded: false
completion_contract_status: failed
completion_contract_proof_status: failed
candidate_score_calibrated: false
candidate_score_disagreement: false
observer_outcome_recorded: true
memory_boundary_recorded: true
verification_proof_status: failed
verification_proof_summary: required validation failed 2/2 commands
verification_proof_kinds: none
verification_proof_support_status: failed
verification_proof_support_summary: verification proof status failed blocks verified closeout before proof-kind policy
verification_proof_supports_verified: false
verification_proof_residual_risk: true
premature_edit_count: 0
evidence_before_first_edit: true
scope_drift_count: 0
invalid_action_count: 3
repeated_action_count: 3
failed_action_count: 8
user_question_count: 2
unnecessary_question_count: 0
verification_attempted: true
verification_passed: false
tool_call_count: 6
llm_call_count: 11
warning: earlier_verification_failed_before_repair
warning: earlier_stage_validation_failed_before_repair
warning: required_commands_not_passing
warning: closeout_not_successful
warning: acceptance_review_rejected
warning: stage_validation_failed
warning: verification_failed
failure_owner: llm_reasoning
outcome_score: 15
process_score: 65
efficiency_score: 35
agent_score: 34
score_penalties: run_failed,required_commands_failed,verification_failed,closeout_not_successful,repeated_action,invalid_action,failed_actions,repeated_actions,user_questions,llm_call_budget_pressure
```

Specialty signals:

```text
memory_active: true
automation_active: true
guided_debugging_active: true
guided_reasoning_active: false
weighted_planning_active: true
closeout_active: true
adaptive_workflow_active: true
active_specialty_signals: 6/7
workflow_contract_activation: entry=active:force repair=active_after_failure
workflow_contract_events: 1
runtime_spine: coverage=6/7, status=none, missing=none
runtime_profile: none
mva_profile_active: false
runtime_spine_detail: context=47 latest=runtime_diet_report decision=40 latest=action_reviewed permission=0 latest=none tool_execution=39 latest=api_request_completed state_update=97 latest=workflow_fallback verification=16 latest=acceptance_review_completed closeout=3 latest=assistant_responded risky_tool_runs=12 risky_tool_reviewed=12 risky_tool_missing_action_review=none gate_outcomes=total=15, protective_block=9, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=6 stop_reason=repeated_tool_failure stop_terminal_status=failed stop_action=recover stop_failure_type=unknown rollback_recommended=false rollback_completed=false recovery_failure_types=hidden_mutation_tool_requested recovery_kinds=no_silent_mutation_expansion route_recovery=events=1, read_search=false, mutation_blocked=true, safety=true action_scores=12 latest_action_score=21 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=none runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=11 provider_protocol_repairs=735 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=5 context_zone_duplicate_blocks_removed=0 context_zone_provenance_markers=3 agent_loop_steps=20 context_zones=11 completion_contract=failed
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: none
runtime_spine_status: none
runtime_spine_missing: none
risky_tool_runs: 12
risky_tool_reviewed: 12
risky_tool_missing_action_review: none
gate_outcomes: total=15, protective_block=9, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=6
gate_outcome_records: action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:revise:protective_block,action_review:revise:protective_block,action_review:revise:protective_block,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:revise:protective_block,action_review:revise:protective_block,action_review:revise:protective_block,action_review:allow:harmless_pass,+3
gate_outcome_total: 15
gate_outcome_protective_blocks: 9
gate_outcome_recoverable_friction: 0
gate_outcome_unrecovered_blocks: 0
gate_outcome_suspected_false_positives: 0
gate_outcome_policy_correct_but_ux_costly: 0
gate_outcome_harmless_passes: 6
gate_outcome_failure_owners: none
agent_loop_steps: 20
context_zones_materialized: true
context_zone_task_state_empty: false
context_zone_current_decision_request_empty: false
context_zone_envelope_messages: 1
context_zone_source_messages: 5
context_zone_duplicate_blocks_removed: 0
context_zone_provenance_markers: 3
state_transition_recorded: false
completion_contract_status: failed
completion_contract_proof_status: failed
candidate_score_calibrated: false
candidate_score_disagreement: false
observer_outcome_recorded: true
memory_boundary_recorded: true
verification_proof_status: failed
verification_proof_summary: required validation failed 2/2 commands
verification_proof_kinds: none
verification_proof_support_status: failed
verification_proof_support_summary: verification proof status failed blocks verified closeout before proof-kind policy
verification_proof_supports_verified: false
verification_proof_residual_risk: true
risk_signal: entry=high runtime=high
risk_signal_reasons: required validation commands present
memory_sync_events: 4
memory_tool_calls: 0
retrieval_sources: Project
memory_candidate_typed: true
memory_candidate_has_evidence: true
memory_proposal_recorded: true
memory_proposal_status: proposed
memory_proposal_candidates: 1
memory_proposal_kinds: failure_pattern
memory_proposal_evidence_items: 28
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
required_command_status: failed
validation_events: 3
stage_validation_events: 3
tool_progress_events: 4
guided_debugging_events: 3
guided_reasoning_events: 0
workflow_plan_events: 1
weighted_plan_events: 1
reweighted_plan_events: 0
adaptive_trigger_events: 5
adaptive_triggers: risk_signal_high,required_validation,first_code_change,verification_failed,acceptance_rejected
latest_top_priority: P3
latest_top_importance_score: 0.17000000178813934
latest_top_weight_share: 0.2518518567085266
acceptance_accepted: False
closeout_status: failed
closeout_tool_records: 97
closeout_tool_evidence: tool evidence: records=97 completed=6 failed=91 denied=0 validation=0 closeout=4 repair=95 changed=4 workflows=code_change commands=none
runtime_diet: prompt=38034 tool_schema=3950 tools=19 workflow=strict
attention: required commands did not pass in the harness
```

Agent stderr tail:

```text
2026-05-27T05:24:57.662989Z  WARN priority_agent::engine::conversation_loop::patch_recovery: Patch synthesis JSON actions were not directly applicable: patch plan did not include a valid file_edit action
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

- Bundle: `docs/benchmarks/live-flow-real-postc-20260527-134900/backend-todo-api-crud/run-bundle`
- Task: `docs/benchmarks/live-flow-real-postc-20260527-134900/backend-todo-api-crud/run-bundle/task.json`
- Steps: `docs/benchmarks/live-flow-real-postc-20260527-134900/backend-todo-api-crud/run-bundle/steps.jsonl`
- Events: `docs/benchmarks/live-flow-real-postc-20260527-134900/backend-todo-api-crud/run-bundle/events.jsonl`
- Final report: `docs/benchmarks/live-flow-real-postc-20260527-134900/backend-todo-api-crud/run-bundle/final_report.md`
