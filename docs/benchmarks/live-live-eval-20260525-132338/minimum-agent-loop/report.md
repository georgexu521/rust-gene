# Live Eval Report: minimum-agent-loop

- Run id: `live-eval-20260525-132338`
- Sample: `evalsets/live_tasks/minimum-agent-loop.yaml`
- Worktree: `target/live-evals/live-eval-20260525-132338/minimum-agent-loop/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/live-eval-20260525-132338/minimum-agent-loop/env`
- Test status: `failed`
- Generated: `2026-05-25 13:24:33 +0800`

## Git Status

```text
```

## Diff Stat

```text
```

## Required Commands

```text
$ python3 fixtures/mva_loop/test_calculator.py
F
======================================================================
FAIL: test_add (__main__.CalculatorTest.test_add)
----------------------------------------------------------------------
Traceback (most recent call last):
  File "/Users/georgexu/Desktop/rust-agent/target/live-evals/live-eval-20260525-132338/minimum-agent-loop/worktree/fixtures/mva_loop/test_calculator.py", line 7, in test_add
    self.assertEqual(calculator.add(2, 3), 5)
AssertionError: -1 != 5

----------------------------------------------------------------------
Ran 1 test in 0.001s

FAILED (failures=1)
[exit status: 1]

$ rg 'return a \+ b' fixtures/mva_loop/calculator.py
[exit status: 1]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-live-eval-20260525-132338/minimum-agent-loop/agent-output.md`
- Events: `docs/benchmarks/live-live-eval-20260525-132338/minimum-agent-loop/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
permission_request: 1
runtime_diagnostic: 1
start: 1
text_chunk: 2
tool_execution_complete: 3
tool_execution_start: 3
trace_summary: 1
```

Quality signals:

```text
output_chars: 1178
diff_chars: 0
diff_files_changed: 0
tool_executions: 3
first_write_tool_index: 3
forbidden_tool_uses: none
tool_errors: 1
tool_failures: 1
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 63
test_status: failed
verification_passed: false
stage_validation_passed: false
acceptance_accepted: None
closeout_status: not_verified
closeout_tool_records: 5
closeout_tool_evidence: tool evidence: records=5 completed=2 failed=2 denied=1 validation=0 closeout=1 repair=3 changed=0 workflows=code_change commands=none
runtime_diet: prompt=3095 tool_schema=3950 tools=19 workflow=guarded closeout=full validation=blocked
adaptive_triggers: risk_signal_high,required_validation
risk_signal: entry=high runtime=high
risk_signal_reasons: required validation commands present
trace_event_types: recovery,stop.check,agent.loop,stop.check,agent.loop,risk.signal,guided.debug,closeout,runtime.diet,completion.contract,memory.boundary,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: seeded_code_change
behavior_assertions: none
behavior_assertion_status: none
runtime_spine: coverage=7/7, status=failed, missing=completion_status:completed,terminal_status:completed,verification_proof_status:verified
runtime_profile: minimum_viable_agent
mva_profile_active: true
runtime_spine_detail: context=14 latest=memory_boundary_evaluated decision=17 latest=risk_signal_assessed permission=2 latest=permission_resolved tool_execution=8 latest=tool_completed state_update=17 latest=agent_loop_step_evaluated verification=2 latest=guided_debugging_completed closeout=3 latest=assistant_responded risky_tool_runs=1 risky_tool_reviewed=1 risky_tool_missing_action_review=none stop_reason=repeated_tool_failure stop_terminal_status=failed stop_action=recover stop_failure_type=unknown rollback_recommended=false rollback_completed=false recovery_failure_types=permission_block recovery_kinds=ask_user_or_choose_safer_path action_scores=3 latest_action_score=20 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true memory_boundary_recorded=true agent_loop_steps=4 context_zones=2 completion_contract=partial
runtime_spine_trace_present: true
runtime_spine_phase_coverage: 7/7
runtime_spine_observed_phases: context,decision,permission,tool_execution,state_update,verification,closeout
runtime_spine_assertions: phase:context,phase:decision,phase:tool_execution,phase:state_update,phase:verification,phase:closeout,event:context_zones_materialized,event:action_decision_evaluated,event:action_reviewed,event:agent_loop_step_evaluated,event:stop_check_evaluated,event:completion_contract_evaluated,completion_status:completed,terminal_status:completed,verification_proof_status:verified,special:verification_proof,special:context_task_state_non_empty,special:current_decision_request_non_empty
runtime_spine_status: failed
runtime_spine_missing: completion_status:completed,terminal_status:completed,verification_proof_status:verified
risky_tool_runs: 1
risky_tool_reviewed: 1
risky_tool_missing_action_review: none
agent_loop_steps: 4
context_zones_materialized: true
context_zone_task_state_empty: false
context_zone_current_decision_request_empty: false
state_transition_recorded: false
completion_contract_status: partial
completion_contract_proof_status: blocked
candidate_score_calibrated: false
candidate_score_disagreement: false
observer_outcome_recorded: true
memory_boundary_recorded: true
verification_proof_status: blocked
verification_proof_summary: verification is blocked
warning: no_code_diff
warning: tool_errors_seen
warning: required_commands_not_passing
warning: runtime_spine_assertions_not_passing
warning: closeout_not_successful
failure_owner: agent_flow
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
runtime_spine: coverage=7/7, status=failed, missing=completion_status:completed,terminal_status:completed,verification_proof_status:verified
runtime_profile: minimum_viable_agent
mva_profile_active: true
runtime_spine_detail: context=14 latest=memory_boundary_evaluated decision=17 latest=risk_signal_assessed permission=2 latest=permission_resolved tool_execution=8 latest=tool_completed state_update=17 latest=agent_loop_step_evaluated verification=2 latest=guided_debugging_completed closeout=3 latest=assistant_responded risky_tool_runs=1 risky_tool_reviewed=1 risky_tool_missing_action_review=none stop_reason=repeated_tool_failure stop_terminal_status=failed stop_action=recover stop_failure_type=unknown rollback_recommended=false rollback_completed=false recovery_failure_types=permission_block recovery_kinds=ask_user_or_choose_safer_path action_scores=3 latest_action_score=20 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true memory_boundary_recorded=true agent_loop_steps=4 context_zones=2 completion_contract=partial
runtime_spine_phase_coverage: 7/7
runtime_spine_observed_phases: context,decision,permission,tool_execution,state_update,verification,closeout
runtime_spine_assertions: phase:context,phase:decision,phase:tool_execution,phase:state_update,phase:verification,phase:closeout,event:context_zones_materialized,event:action_decision_evaluated,event:action_reviewed,event:agent_loop_step_evaluated,event:stop_check_evaluated,event:completion_contract_evaluated,completion_status:completed,terminal_status:completed,verification_proof_status:verified,special:verification_proof,special:context_task_state_non_empty,special:current_decision_request_non_empty
runtime_spine_status: failed
runtime_spine_missing: completion_status:completed,terminal_status:completed,verification_proof_status:verified
risky_tool_runs: 1
risky_tool_reviewed: 1
risky_tool_missing_action_review: none
agent_loop_steps: 4
context_zones_materialized: true
context_zone_task_state_empty: false
context_zone_current_decision_request_empty: false
state_transition_recorded: false
completion_contract_status: partial
completion_contract_proof_status: blocked
candidate_score_calibrated: false
candidate_score_disagreement: false
observer_outcome_recorded: true
memory_boundary_recorded: true
verification_proof_status: blocked
risk_signal: entry=high runtime=high
risk_signal_reasons: required validation commands present
memory_sync_events: 1
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
tool_progress_events: 0
guided_debugging_events: 1
guided_reasoning_events: 0
workflow_plan_events: 1
weighted_plan_events: 1
reweighted_plan_events: 0
adaptive_trigger_events: 2
adaptive_triggers: risk_signal_high,required_validation
latest_top_priority: P3
latest_top_importance_score: 0.05000000074505806
latest_top_weight_share: 0.3333333134651184
acceptance_accepted: missing
closeout_status: not_verified
closeout_tool_records: 5
closeout_tool_evidence: tool evidence: records=5 completed=2 failed=2 denied=1 validation=0 closeout=1 repair=3 changed=0 workflows=code_change commands=none
runtime_diet: prompt=3095 tool_schema=3950 tools=19 workflow=guarded
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
