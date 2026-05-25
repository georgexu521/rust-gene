# Live Eval Report: minimum-agent-verification-repair

- Run id: `ab-20260525-155452-weighted`
- Sample: `evalsets/live_tasks/minimum-agent-verification-repair.yaml`
- Worktree: `target/live-evals/ab-20260525-155452-weighted/minimum-agent-verification-repair/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/ab-20260525-155452-weighted/minimum-agent-verification-repair/env`
- Test status: `failed`
- Generated: `2026-05-25 16:03:08 +0800`

## Git Status

```text
?? fixtures/mva_verification_repair/__pycache__/
```

## Diff Stat

```text
```

## Required Commands

```text
$ python3 fixtures/mva_verification_repair/test_slugify.py
F
======================================================================
FAIL: test_slugify_lowercase_hyphen (__main__.SlugifyTest.test_slugify_lowercase_hyphen)
----------------------------------------------------------------------
Traceback (most recent call last):
  File "/Users/georgexu/Desktop/rust-agent/target/live-evals/ab-20260525-155452-weighted/minimum-agent-verification-repair/worktree/fixtures/mva_verification_repair/test_slugify.py", line 7, in test_slugify_lowercase_hyphen
    self.assertEqual(slugify.slugify(" Hello World "), "hello-world")
AssertionError: 'Hello_World' != 'hello-world'
- Hello_World
? ^    ^^
+ hello-world
? ^    ^^


----------------------------------------------------------------------
Ran 1 test in 0.000s

FAILED (failures=1)
[exit status: 1]

$ rg -F 'return value.strip().lower().replace(" ", "-")' fixtures/mva_verification_repair/slugify.py
[exit status: 1]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-ab-20260525-155452-weighted/minimum-agent-verification-repair/agent-output.md`
- Events: `docs/benchmarks/live-ab-20260525-155452-weighted/minimum-agent-verification-repair/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
runtime_diagnostic: 1
start: 1
text_chunk: 2
tool_execution_complete: 11
tool_execution_progress: 2
tool_execution_start: 11
trace_summary: 1
```

Quality signals:

```text
output_chars: 1470
diff_chars: 0
diff_files_changed: 0
tool_executions: 11
first_write_tool_index: 11
forbidden_tool_uses: none
tool_errors: 2
tool_failures: 2
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 190
test_status: failed
verification_passed: false
stage_validation_passed: false
acceptance_accepted: None
closeout_status: not_verified
closeout_tool_records: 53
closeout_tool_evidence: tool evidence: records=53 completed=9 failed=44 denied=0 validation=1 closeout=1 repair=44 changed=0 workflows=code_change commands=python3 fixtures/mva_verification_repair/test_slugify.py
runtime_diet: prompt=8006 tool_schema=3950 tools=19 workflow=guarded closeout=full validation=not_run
adaptive_triggers: risk_signal_high,required_validation
risk_signal: entry=high runtime=high
risk_signal_reasons: required validation commands present
trace_event_types: tool.done,stop.check,agent.loop,stop.check,agent.loop,risk.signal,guided.debug,closeout,runtime.diet,completion.contract,memory.boundary,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: seeded_code_change
behavior_assertions: none
behavior_assertion_status: none
output_assertions: contains=2
output_assertion_status: failed
output_assertion_missing: contains:失败;contains:验证
trajectory_assertions: evidence_before_edit,requires_observer_outcome,requires_stop_check,max_scope_drift_count,max_premature_edit_count,requires_runtime_spine_passed
trajectory_assertion_status: failed
trajectory_assertion_missing: max_scope_drift_count:1>0;requires_runtime_spine_passed
runtime_spine: coverage=6/7, status=failed, missing=completion_status:completed,terminal_status:completed,verification_proof_status:verified
runtime_profile: minimum_viable_agent
mva_profile_active: true
runtime_spine_detail: context=46 latest=memory_boundary_evaluated decision=42 latest=risk_signal_assessed permission=0 latest=none tool_execution=32 latest=tool_completed state_update=64 latest=agent_loop_step_evaluated verification=3 latest=guided_debugging_completed closeout=3 latest=assistant_responded risky_tool_runs=1 risky_tool_reviewed=1 risky_tool_missing_action_review=none stop_reason=repeated_tool_failure stop_terminal_status=failed stop_action=recover stop_failure_type=unknown rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none action_scores=11 latest_action_score=23 low_action_score_count=0 phase_misaligned_actions=1 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true memory_boundary_recorded=true agent_loop_steps=20 context_zones=10 completion_contract=partial
runtime_spine_trace_present: true
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: phase:context,phase:decision,phase:tool_execution,phase:state_update,phase:verification,phase:closeout,event:context_zones_materialized,event:action_decision_evaluated,event:action_reviewed,event:agent_loop_step_evaluated,event:stop_check_evaluated,event:completion_contract_evaluated,completion_status:completed,terminal_status:completed,verification_proof_status:verified,special:verification_proof
runtime_spine_status: failed
runtime_spine_missing: completion_status:completed,terminal_status:completed,verification_proof_status:verified
risky_tool_runs: 1
risky_tool_reviewed: 1
risky_tool_missing_action_review: none
agent_loop_steps: 20
context_zones_materialized: true
context_zone_task_state_empty: false
context_zone_current_decision_request_empty: false
state_transition_recorded: false
completion_contract_status: partial
completion_contract_proof_status: not_run
candidate_score_calibrated: false
candidate_score_disagreement: false
observer_outcome_recorded: true
memory_boundary_recorded: true
verification_proof_status: not_run
verification_proof_summary: required validation missing 2/2 commands
premature_edit_count: 0
evidence_before_first_edit: true
scope_drift_count: 1
invalid_action_count: 10
repeated_action_count: 8
failed_action_count: 4
user_question_count: 0
unnecessary_question_count: 0
verification_attempted: true
verification_passed: false
tool_call_count: 11
llm_call_count: 10
warning: no_code_diff
warning: tool_errors_seen
warning: required_commands_not_passing
warning: output_assertions_not_passing
warning: trajectory_assertions_not_passing
warning: runtime_spine_assertions_not_passing
warning: closeout_not_successful
failure_owner: agent_flow
outcome_score: 0
process_score: 30
efficiency_score: 43
agent_score: 18
score_penalties: run_failed,required_commands_failed,verification_failed,closeout_not_successful,runtime_spine_failed,output_assertions_failed,trajectory_assertions_failed,expected_code_diff_missing,scope_drift,repeated_action,invalid_action,runtime_spine_not_passing,tool_budget_exceeded,failed_actions,repeated_actions,llm_call_budget_pressure
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
runtime_spine: coverage=6/7, status=failed, missing=completion_status:completed,terminal_status:completed,verification_proof_status:verified
runtime_profile: minimum_viable_agent
mva_profile_active: true
runtime_spine_detail: context=46 latest=memory_boundary_evaluated decision=42 latest=risk_signal_assessed permission=0 latest=none tool_execution=32 latest=tool_completed state_update=64 latest=agent_loop_step_evaluated verification=3 latest=guided_debugging_completed closeout=3 latest=assistant_responded risky_tool_runs=1 risky_tool_reviewed=1 risky_tool_missing_action_review=none stop_reason=repeated_tool_failure stop_terminal_status=failed stop_action=recover stop_failure_type=unknown rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none action_scores=11 latest_action_score=23 low_action_score_count=0 phase_misaligned_actions=1 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true memory_boundary_recorded=true agent_loop_steps=20 context_zones=10 completion_contract=partial
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: phase:context,phase:decision,phase:tool_execution,phase:state_update,phase:verification,phase:closeout,event:context_zones_materialized,event:action_decision_evaluated,event:action_reviewed,event:agent_loop_step_evaluated,event:stop_check_evaluated,event:completion_contract_evaluated,completion_status:completed,terminal_status:completed,verification_proof_status:verified,special:verification_proof
runtime_spine_status: failed
runtime_spine_missing: completion_status:completed,terminal_status:completed,verification_proof_status:verified
risky_tool_runs: 1
risky_tool_reviewed: 1
risky_tool_missing_action_review: none
agent_loop_steps: 20
context_zones_materialized: true
context_zone_task_state_empty: false
context_zone_current_decision_request_empty: false
state_transition_recorded: false
completion_contract_status: partial
completion_contract_proof_status: not_run
candidate_score_calibrated: false
candidate_score_disagreement: false
observer_outcome_recorded: true
memory_boundary_recorded: true
verification_proof_status: not_run
risk_signal: entry=high runtime=high
risk_signal_reasons: required validation commands present
memory_sync_events: 9
memory_tool_calls: 0
retrieval_sources: Project
memory_candidate_typed: false
memory_candidate_has_evidence: false
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
validation_events: 0
stage_validation_events: 0
tool_progress_events: 2
guided_debugging_events: 2
guided_reasoning_events: 0
workflow_plan_events: 1
weighted_plan_events: 1
reweighted_plan_events: 0
adaptive_trigger_events: 2
adaptive_triggers: risk_signal_high,required_validation
latest_top_priority: P3
latest_top_importance_score: 0.05000000074505806
latest_top_weight_share: 0.25
acceptance_accepted: missing
closeout_status: not_verified
closeout_tool_records: 53
closeout_tool_evidence: tool evidence: records=53 completed=9 failed=44 denied=0 validation=1 closeout=1 repair=44 changed=0 workflows=code_change commands=python3 fixtures/mva_verification_repair/test_slugify.py
runtime_diet: prompt=8006 tool_schema=3950 tools=19 workflow=guarded
attention: required commands did not pass in the harness
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

- Bundle: `docs/benchmarks/live-ab-20260525-155452-weighted/minimum-agent-verification-repair/run-bundle`
- Task: `docs/benchmarks/live-ab-20260525-155452-weighted/minimum-agent-verification-repair/run-bundle/task.json`
- Steps: `docs/benchmarks/live-ab-20260525-155452-weighted/minimum-agent-verification-repair/run-bundle/steps.jsonl`
- Events: `docs/benchmarks/live-ab-20260525-155452-weighted/minimum-agent-verification-repair/run-bundle/events.jsonl`
- Final report: `docs/benchmarks/live-ab-20260525-155452-weighted/minimum-agent-verification-repair/run-bundle/final_report.md`
