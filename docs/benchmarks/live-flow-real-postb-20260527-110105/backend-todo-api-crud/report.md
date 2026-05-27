# Live Eval Report: backend-todo-api-crud

- Run id: `flow-real-postb-20260527-110105`
- Sample: `evalsets/live_tasks/backend-todo-api-crud.yaml`
- Worktree: `target/live-evals/flow-real-postb-20260527-110105/backend-todo-api-crud/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/flow-real-postb-20260527-110105/backend-todo-api-crud/env`
- Test status: `failed`
- Generated: `2026-05-27 11:08:42 +0800`

## Git Status

```text
 M fixtures/live_backend/todo_api/todo_api.py
```

## Diff Stat

```text
 fixtures/live_backend/todo_api/todo_api.py | 170 ++++++++++++++++++++++++++---
 1 file changed, 154 insertions(+), 16 deletions(-)
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
  File "/Users/georgexu/Desktop/rust-agent/target/live-evals/flow-real-postb-20260527-110105/backend-todo-api-crud/worktree/fixtures/live_backend/todo_api/test_todo_api.py", line 8, in <module>
    import todo_api
  File "/Users/georgexu/Desktop/rust-agent/target/live-evals/flow-real-postb-20260527-110105/backend-todo-api-crud/worktree/fixtures/live_backend/todo_api/todo_api.py", line 51, in <module>
    class TodoHandler(BaseHTTPRequestHandler):
                      ^^^^^^^^^^^^^^^^^^^^^^
NameError: name 'BaseHTTPRequestHandler' is not defined


----------------------------------------------------------------------
Ran 1 test in 0.000s

FAILED (errors=1)
[exit status: 1]

$ ! rg 'TODO' fixtures/live_backend/todo_api/todo_api.py
[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-flow-real-postb-20260527-110105/backend-todo-api-crud/agent-output.md`
- Events: `docs/benchmarks/live-flow-real-postb-20260527-110105/backend-todo-api-crud/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
runtime_diagnostic: 1
start: 1
text_chunk: 1
tool_execution_complete: 17
tool_execution_progress: 9
tool_execution_start: 17
trace_summary: 1
```

Quality signals:

```text
output_chars: 2055
diff_chars: 7408
diff_files_changed: 1
tool_executions: 17
first_write_tool_index: 8
forbidden_tool_uses: none
tool_errors: 1
tool_failures: 7
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 325
test_status: failed
verification_passed: false
stage_validation_passed: false
acceptance_accepted: False
closeout_status: failed
closeout_tool_records: 200
closeout_tool_evidence: tool evidence: records=200 completed=16 failed=184 denied=0 validation=0 closeout=8 repair=192 changed=8 workflows=code_change commands=none
runtime_diet: prompt=39419 tool_schema=3950 tools=19 workflow=strict closeout=full validation=failed:1/2
adaptive_triggers: risk_signal_high,required_validation,repeated_no_code_progress,first_code_change,verification_failed,acceptance_rejected
risk_signal: entry=high runtime=high
risk_signal_reasons: required validation commands present
trace_event_types: tool.done,stop.check,stop.check,agent.loop,stop.check,agent.loop,closeout,execution.report,memory.proposal,runtime.diet,completion.contract,assistant
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
runtime_spine_detail: context=59 latest=runtime_diet_report decision=58 latest=action_reviewed permission=0 latest=none tool_execution=59 latest=tool_completed state_update=125 latest=agent_loop_step_evaluated verification=16 latest=acceptance_review_completed closeout=3 latest=assistant_responded risky_tool_runs=10 risky_tool_reviewed=10 risky_tool_missing_action_review=none gate_outcomes=total=24, protective_block=2, recoverable_friction=0, unrecovered_block=5, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=17 stop_reason=repeated_tool_failure stop_terminal_status=failed stop_action=recover stop_failure_type=unknown rollback_recommended=false rollback_completed=false recovery_failure_types=code_change_no_diff_after_repeated_progress,hidden_read_search_tool_requested recovery_kinds=code_change_no_diff_replan,expand_read_search_only route_recovery=events=3, read_search=true, mutation_blocked=false, safety=true action_scores=20 latest_action_score=36 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=none runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=13 provider_protocol_repairs=1220 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=5 context_zone_duplicate_blocks_removed=0 context_zone_provenance_markers=3 agent_loop_steps=26 context_zones=13 completion_contract=failed
runtime_spine_trace_present: true
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: none
runtime_spine_status: none
runtime_spine_missing: none
risky_tool_runs: 10
risky_tool_reviewed: 10
risky_tool_missing_action_review: none
gate_outcomes: total=24, protective_block=2, recoverable_friction=0, unrecovered_block=5, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=17
gate_outcome_records: action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:revise:unrecovered_block,action_review:revise:protective_block,action_review:allow:harmless_pass,+12
gate_outcome_total: 24
gate_outcome_protective_blocks: 2
gate_outcome_recoverable_friction: 0
gate_outcome_unrecovered_blocks: 5
gate_outcome_suspected_false_positives: 0
gate_outcome_policy_correct_but_ux_costly: 0
gate_outcome_harmless_passes: 17
gate_outcome_failure_owners: action_review
route_recovery: events=3, read_search=true, mutation_blocked=false, safety=true
route_recovery_events: 3
route_recovery_failure_types: code_change_no_diff_after_repeated_progress,hidden_read_search_tool_requested
route_recovery_kinds: code_change_no_diff_replan,expand_read_search_only
route_recovery_read_search_expanded: true
route_recovery_mutation_blocked: false
route_recovery_safety_monotonic: true
route_recovery_unsafe_mutation_expansion: false
agent_loop_steps: 26
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
verification_proof_summary: required validation failed 1/2 commands
verification_proof_kinds: none
verification_proof_support_status: failed
verification_proof_support_summary: verification proof status failed blocks verified closeout before proof-kind policy
verification_proof_supports_verified: false
verification_proof_residual_risk: true
premature_edit_count: 0
evidence_before_first_edit: true
scope_drift_count: 0
invalid_action_count: 7
repeated_action_count: 7
failed_action_count: 8
user_question_count: 3
unnecessary_question_count: 0
verification_attempted: true
verification_passed: false
tool_call_count: 17
llm_call_count: 13
warning: tool_errors_seen
warning: earlier_verification_failed_before_repair
warning: earlier_stage_validation_failed_before_repair
warning: required_commands_not_passing
warning: closeout_not_successful
warning: acceptance_review_rejected
warning: stage_validation_failed
warning: verification_failed
failure_owner: eval_harness
outcome_score: 15
process_score: 60
efficiency_score: 30
agent_score: 32
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
runtime_spine_detail: context=59 latest=runtime_diet_report decision=58 latest=action_reviewed permission=0 latest=none tool_execution=59 latest=tool_completed state_update=125 latest=agent_loop_step_evaluated verification=16 latest=acceptance_review_completed closeout=3 latest=assistant_responded risky_tool_runs=10 risky_tool_reviewed=10 risky_tool_missing_action_review=none gate_outcomes=total=24, protective_block=2, recoverable_friction=0, unrecovered_block=5, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=17 stop_reason=repeated_tool_failure stop_terminal_status=failed stop_action=recover stop_failure_type=unknown rollback_recommended=false rollback_completed=false recovery_failure_types=code_change_no_diff_after_repeated_progress,hidden_read_search_tool_requested recovery_kinds=code_change_no_diff_replan,expand_read_search_only route_recovery=events=3, read_search=true, mutation_blocked=false, safety=true action_scores=20 latest_action_score=36 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=none runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=13 provider_protocol_repairs=1220 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=5 context_zone_duplicate_blocks_removed=0 context_zone_provenance_markers=3 agent_loop_steps=26 context_zones=13 completion_contract=failed
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: none
runtime_spine_status: none
runtime_spine_missing: none
risky_tool_runs: 10
risky_tool_reviewed: 10
risky_tool_missing_action_review: none
gate_outcomes: total=24, protective_block=2, recoverable_friction=0, unrecovered_block=5, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=17
gate_outcome_records: action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:revise:unrecovered_block,action_review:revise:protective_block,action_review:allow:harmless_pass,+12
gate_outcome_total: 24
gate_outcome_protective_blocks: 2
gate_outcome_recoverable_friction: 0
gate_outcome_unrecovered_blocks: 5
gate_outcome_suspected_false_positives: 0
gate_outcome_policy_correct_but_ux_costly: 0
gate_outcome_harmless_passes: 17
gate_outcome_failure_owners: action_review
agent_loop_steps: 26
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
verification_proof_summary: required validation failed 1/2 commands
verification_proof_kinds: none
verification_proof_support_status: failed
verification_proof_support_summary: verification proof status failed blocks verified closeout before proof-kind policy
verification_proof_supports_verified: false
verification_proof_residual_risk: true
risk_signal: entry=high runtime=high
risk_signal_reasons: required validation commands present
memory_sync_events: 7
memory_tool_calls: 0
retrieval_sources: Project
memory_candidate_typed: true
memory_candidate_has_evidence: true
memory_proposal_recorded: true
memory_proposal_status: proposed
memory_proposal_candidates: 1
memory_proposal_kinds: failure_pattern
memory_proposal_evidence_items: 15
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
tool_progress_events: 9
guided_debugging_events: 3
guided_reasoning_events: 0
workflow_plan_events: 1
weighted_plan_events: 1
reweighted_plan_events: 0
adaptive_trigger_events: 6
adaptive_triggers: risk_signal_high,required_validation,repeated_no_code_progress,first_code_change,verification_failed,acceptance_rejected
latest_top_priority: P3
latest_top_importance_score: 0.2750000059604645
latest_top_weight_share: 0.38461536169052124
acceptance_accepted: False
closeout_status: failed
closeout_tool_records: 200
closeout_tool_evidence: tool evidence: records=200 completed=16 failed=184 denied=0 validation=0 closeout=8 repair=192 changed=8 workflows=code_change commands=none
runtime_diet: prompt=39419 tool_schema=3950 tools=19 workflow=strict
attention: required commands did not pass in the harness
```

Agent stderr tail:

```text
2026-05-27T03:07:42.459039Z  WARN priority_agent::engine::conversation_loop::patch_recovery: Patch synthesis JSON actions were not directly applicable: synthesized patch old_string was not found exactly in /Users/georgexu/Desktop/rust-agent/target/live-evals/flow-real-postb-20260527-110105/backend-todo-api-crud/worktree/fixtures/live_backend/todo_api/todo_api.py; refusing inexact multi-line replacement; synthesized patch old_string was not found exactly in /Users/georgexu/Desktop/rust-agent/target/live-evals/flow-real-postb-20260527-110105/backend-todo-api-crud/worktree/fixtures/live_backend/todo_api/todo_api.py; refusing inexact multi-line replacement; synthesized patch old_string was not found exactly in /Users/georgexu/Desktop/rust-agent/target/live-evals/flow-real-postb-20260527-110105/backend-todo-api-crud/worktree/fixtures/live_backend/todo_api/todo_api.py; refusing inexact multi-line replacement; synthesized patch old_string was not found exactly in /Users/georgexu/Desktop/rust-agent/target/live-evals/flow-real-postb-20260527-110105/backend-todo-api-crud/worktree/fixtures/live_backend/todo_api/todo_api.py; refusing inexact multi-line replacement; synthesized patch old_string was not found exactly in /Users/georgexu/Desktop/rust-agent/target/live-evals/flow-real-postb-20260527-110105/backend-todo-api-crud/worktree/fixtures/live_backend/todo_api/todo_api.py; refusing inexact multi-line replacement; synthesized patch old_string was not found exactly in /Users/georgexu/Desktop/rust-agent/target/live-evals/flow-real-postb-20260527-110105/backend-todo-api-crud/worktree/fixtures/live_backend/todo_api/todo_api.py; refusing inexact multi-line replacement; patch synthesis declined without a reason
2026-05-27T03:08:04.167774Z  WARN priority_agent::engine::conversation_loop::patch_recovery: Patch synthesis JSON actions were not directly applicable: patch synthesis declined without a reason; patch synthesis declined without a reason
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

- Bundle: `docs/benchmarks/live-flow-real-postb-20260527-110105/backend-todo-api-crud/run-bundle`
- Task: `docs/benchmarks/live-flow-real-postb-20260527-110105/backend-todo-api-crud/run-bundle/task.json`
- Steps: `docs/benchmarks/live-flow-real-postb-20260527-110105/backend-todo-api-crud/run-bundle/steps.jsonl`
- Events: `docs/benchmarks/live-flow-real-postb-20260527-110105/backend-todo-api-crud/run-bundle/events.jsonl`
- Final report: `docs/benchmarks/live-flow-real-postb-20260527-110105/backend-todo-api-crud/run-bundle/final_report.md`
