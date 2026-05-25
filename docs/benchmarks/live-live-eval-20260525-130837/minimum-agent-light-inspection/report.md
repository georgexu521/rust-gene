# Live Eval Report: minimum-agent-light-inspection

- Run id: `live-eval-20260525-130837`
- Sample: `evalsets/live_tasks/minimum-agent-light-inspection.yaml`
- Worktree: `target/live-evals/live-eval-20260525-130837/minimum-agent-light-inspection/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/live-eval-20260525-130837/minimum-agent-light-inspection/env`
- Test status: `ok`
- Generated: `2026-05-25 13:10:24 +0800`

## Git Status

```text
```

## Diff Stat

```text
```

## Required Commands

```text
$ test -f fixtures/mva_light_inspection/a.txt
[exit status: 0]

$ test -f fixtures/mva_light_inspection/.hidden
[exit status: 0]

$ test -d fixtures/mva_light_inspection/notes
[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-live-eval-20260525-130837/minimum-agent-light-inspection/agent-output.md`
- Events: `docs/benchmarks/live-live-eval-20260525-130837/minimum-agent-light-inspection/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
permission_request: 3
runtime_diagnostic: 1
start: 1
text_chunk: 2
tool_execution_complete: 7
tool_execution_start: 7
trace_summary: 1
```

Quality signals:

```text
output_chars: 1312
diff_chars: 0
diff_files_changed: 0
tool_executions: 7
first_write_tool_index: none
forbidden_tool_uses: none
tool_errors: 3
tool_failures: 3
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 101
test_status: ok
verification_passed: false
stage_validation_passed: false
acceptance_accepted: None
closeout_status: not_verified
closeout_tool_records: 12
closeout_tool_evidence: tool evidence: records=12 completed=4 failed=5 denied=3 validation=3 closeout=3 repair=8 changed=0 workflows=code_change commands=test -f fixtures/mva_light_inspection/a.txt && echo "PASS" || echo "FAIL" | test -f fixtures/mva_light_inspect...
runtime_diet: prompt=3075 tool_schema=3950 tools=19 workflow=guarded closeout=full validation=failed:3/3
adaptive_triggers: risk_signal_high,required_validation
risk_signal: entry=high runtime=high
risk_signal_reasons: required validation commands present
trace_event_types: recovery,stop.check,agent.loop,stop.check,agent.loop,risk.signal,guided.debug,closeout,runtime.diet,completion.contract,memory.boundary,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: audit_or_regression_check
behavior_assertions: none
behavior_assertion_status: none
runtime_spine: coverage=7/7, status=failed, missing=completion_status:completed
runtime_profile: minimum_viable_agent
mva_profile_active: true
runtime_spine_detail: context=18 latest=memory_boundary_evaluated decision=23 latest=risk_signal_assessed permission=6 latest=permission_resolved tool_execution=17 latest=tool_completed state_update=32 latest=agent_loop_step_evaluated verification=2 latest=guided_debugging_completed closeout=3 latest=assistant_responded risky_tool_runs=3 risky_tool_reviewed=3 risky_tool_missing_action_review=none stop_reason=repeated_tool_failure stop_terminal_status=failed stop_action=recover stop_failure_type=unknown rollback_recommended=false rollback_completed=false recovery_failure_types=permission_block recovery_kinds=ask_user_or_choose_safer_path action_scores=6 latest_action_score=29 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true memory_boundary_recorded=true agent_loop_steps=6 context_zones=3 completion_contract=partial
runtime_spine_trace_present: true
runtime_spine_phase_coverage: 7/7
runtime_spine_observed_phases: context,decision,permission,tool_execution,state_update,verification,closeout
runtime_spine_assertions: phase:context,phase:decision,phase:tool_execution,phase:state_update,phase:closeout,event:context_zones_materialized,event:action_decision_evaluated,event:action_reviewed,event:agent_loop_step_evaluated,event:stop_check_evaluated,event:completion_contract_evaluated,completion_status:completed
runtime_spine_status: failed
runtime_spine_missing: completion_status:completed
risky_tool_runs: 3
risky_tool_reviewed: 3
risky_tool_missing_action_review: none
agent_loop_steps: 6
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
weighted_planning_active: false
closeout_active: true
adaptive_workflow_active: true
active_specialty_signals: 5/7
workflow_contract_activation: entry=active:force repair=active_after_failure
workflow_contract_events: 1
runtime_spine: coverage=7/7, status=failed, missing=completion_status:completed
runtime_profile: minimum_viable_agent
mva_profile_active: true
runtime_spine_detail: context=18 latest=memory_boundary_evaluated decision=23 latest=risk_signal_assessed permission=6 latest=permission_resolved tool_execution=17 latest=tool_completed state_update=32 latest=agent_loop_step_evaluated verification=2 latest=guided_debugging_completed closeout=3 latest=assistant_responded risky_tool_runs=3 risky_tool_reviewed=3 risky_tool_missing_action_review=none stop_reason=repeated_tool_failure stop_terminal_status=failed stop_action=recover stop_failure_type=unknown rollback_recommended=false rollback_completed=false recovery_failure_types=permission_block recovery_kinds=ask_user_or_choose_safer_path action_scores=6 latest_action_score=29 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true memory_boundary_recorded=true agent_loop_steps=6 context_zones=3 completion_contract=partial
runtime_spine_phase_coverage: 7/7
runtime_spine_observed_phases: context,decision,permission,tool_execution,state_update,verification,closeout
runtime_spine_assertions: phase:context,phase:decision,phase:tool_execution,phase:state_update,phase:closeout,event:context_zones_materialized,event:action_decision_evaluated,event:action_reviewed,event:agent_loop_step_evaluated,event:stop_check_evaluated,event:completion_contract_evaluated,completion_status:completed
runtime_spine_status: failed
runtime_spine_missing: completion_status:completed
risky_tool_runs: 3
risky_tool_reviewed: 3
risky_tool_missing_action_review: none
agent_loop_steps: 6
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
memory_sync_events: 2
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
required_commands: 3
agent_required_commands: 3
harness_commands: 0
required_command_status: ok
validation_events: 0
stage_validation_events: 0
tool_progress_events: 0
guided_debugging_events: 1
guided_reasoning_events: 0
workflow_plan_events: 0
weighted_plan_events: 0
reweighted_plan_events: 0
adaptive_trigger_events: 2
adaptive_triggers: risk_signal_high,required_validation
latest_top_priority: none
latest_top_importance_score: none
latest_top_weight_share: none
acceptance_accepted: missing
closeout_status: not_verified
closeout_tool_records: 12
closeout_tool_evidence: tool evidence: records=12 completed=4 failed=5 denied=3 validation=3 closeout=3 repair=8 changed=0 workflows=code_change commands=test -f fixtures/mva_light_inspection/a.txt && echo "PASS" || echo "FAIL" | test -f fixtures/mva_light_inspect...
runtime_diet: prompt=3075 tool_schema=3950 tools=19 workflow=guarded
```

Agent stderr tail:

```text
2026-05-25T05:09:08.572069Z  WARN priority_agent::engine::conversation_loop::workflow_contract_controller: Workflow judgment analysis failed: unknown variant `pass`, expected one of `pending`, `passed`, `failed`, `not_verified`
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
