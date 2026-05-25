# Live Eval Report: minimum-agent-low-value-replan

- Run id: `live-eval-20260525-143755`
- Sample: `evalsets/live_tasks/minimum-agent-low-value-replan.yaml`
- Worktree: `target/live-evals/live-eval-20260525-143755/minimum-agent-low-value-replan/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/live-eval-20260525-143755/minimum-agent-low-value-replan/env`
- Test status: `ok`
- Generated: `2026-05-25 14:38:50 +0800`

## Git Status

```text
```

## Diff Stat

```text
```

## Required Commands

```text
$ test -f fixtures/mva_low_value_replan/known.txt
[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-live-eval-20260525-143755/minimum-agent-low-value-replan/agent-output.md`
- Events: `docs/benchmarks/live-live-eval-20260525-143755/minimum-agent-low-value-replan/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
runtime_diagnostic: 1
start: 1
text_chunk: 2
tool_execution_complete: 2
tool_execution_start: 2
trace_summary: 1
```

Quality signals:

```text
output_chars: 723
diff_chars: 0
diff_files_changed: 0
tool_executions: 2
first_write_tool_index: none
forbidden_tool_uses: none
tool_errors: 0
tool_failures: 1
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 61
test_status: ok
verification_passed: false
stage_validation_passed: false
acceptance_accepted: None
closeout_status: not_verified
closeout_tool_records: 4
closeout_tool_evidence: tool evidence: records=4 completed=2 failed=2 denied=0 validation=0 closeout=0 repair=2 changed=0 workflows=direct commands=ls -la fixtures/mva_low_value_replan 2>/dev/null || echo "Directory not found"
runtime_diet: prompt=2787 tool_schema=809 tools=4 workflow=none closeout=full validation=blocked
adaptive_triggers: none
risk_signal: entry=ordinary runtime=high
risk_signal_reasons: ordinary change surface
trace_event_types: stop.check,stop.check,agent.loop,stop.check,agent.loop,risk.signal,guided.debug,closeout,runtime.diet,completion.contract,memory.boundary,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: read_only_audit
behavior_assertions: none
behavior_assertion_status: none
runtime_spine: coverage=6/7, status=passed, missing=none
runtime_profile: minimum_viable_agent
mva_profile_active: true
runtime_spine_detail: context=14 latest=memory_boundary_evaluated decision=13 latest=risk_signal_assessed permission=0 latest=none tool_execution=9 latest=tool_completed state_update=20 latest=agent_loop_step_evaluated verification=2 latest=guided_debugging_completed closeout=3 latest=assistant_responded risky_tool_runs=1 risky_tool_reviewed=1 risky_tool_missing_action_review=none stop_reason=repeated_tool_failure stop_terminal_status=failed stop_action=recover stop_failure_type=unknown rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none action_scores=3 latest_action_score=32 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=false memory_modifier_applied=false observer_outcome_recorded=true memory_boundary_recorded=true agent_loop_steps=6 context_zones=3 completion_contract=completed
runtime_spine_trace_present: true
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: phase:context,phase:decision,phase:tool_execution,phase:state_update,phase:closeout,event:context_zones_materialized,event:action_decision_evaluated,event:action_reviewed,event:agent_loop_step_evaluated,event:stop_check_evaluated,event:completion_contract_evaluated,completion_status:completed
runtime_spine_status: passed
runtime_spine_missing: none
risky_tool_runs: 1
risky_tool_reviewed: 1
risky_tool_missing_action_review: none
agent_loop_steps: 6
context_zones_materialized: true
context_zone_task_state_empty: false
context_zone_current_decision_request_empty: false
state_transition_recorded: false
completion_contract_status: completed
completion_contract_proof_status: blocked
candidate_score_calibrated: false
candidate_score_disagreement: false
observer_outcome_recorded: true
memory_boundary_recorded: true
verification_proof_status: blocked
verification_proof_summary: verification is blocked
warning: no_code_diff
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
adaptive_workflow_active: false
active_specialty_signals: 4/7
workflow_contract_activation: entry=skipped:force repair=active_after_failure
workflow_contract_events: 1
runtime_spine: coverage=6/7, status=passed, missing=none
runtime_profile: minimum_viable_agent
mva_profile_active: true
runtime_spine_detail: context=14 latest=memory_boundary_evaluated decision=13 latest=risk_signal_assessed permission=0 latest=none tool_execution=9 latest=tool_completed state_update=20 latest=agent_loop_step_evaluated verification=2 latest=guided_debugging_completed closeout=3 latest=assistant_responded risky_tool_runs=1 risky_tool_reviewed=1 risky_tool_missing_action_review=none stop_reason=repeated_tool_failure stop_terminal_status=failed stop_action=recover stop_failure_type=unknown rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none action_scores=3 latest_action_score=32 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=false memory_modifier_applied=false observer_outcome_recorded=true memory_boundary_recorded=true agent_loop_steps=6 context_zones=3 completion_contract=completed
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: phase:context,phase:decision,phase:tool_execution,phase:state_update,phase:closeout,event:context_zones_materialized,event:action_decision_evaluated,event:action_reviewed,event:agent_loop_step_evaluated,event:stop_check_evaluated,event:completion_contract_evaluated,completion_status:completed
runtime_spine_status: passed
runtime_spine_missing: none
risky_tool_runs: 1
risky_tool_reviewed: 1
risky_tool_missing_action_review: none
agent_loop_steps: 6
context_zones_materialized: true
context_zone_task_state_empty: false
context_zone_current_decision_request_empty: false
state_transition_recorded: false
completion_contract_status: completed
completion_contract_proof_status: blocked
candidate_score_calibrated: false
candidate_score_disagreement: false
observer_outcome_recorded: true
memory_boundary_recorded: true
verification_proof_status: blocked
risk_signal: entry=ordinary runtime=high
risk_signal_reasons: ordinary change surface
memory_sync_events: 2
memory_tool_calls: 0
retrieval_sources: none
memory_candidate_typed: false
memory_candidate_has_evidence: false
memory_record_used: false
memory_use_count_updated: false
memory_failure_lesson_promoted: false
memory_action_weight_changed: false
memory_stale_demoted: false
memory_scope_correct: false
required_commands: 1
agent_required_commands: 0
harness_commands: 1
required_command_status: ok
validation_events: 0
stage_validation_events: 0
tool_progress_events: 0
guided_debugging_events: 1
guided_reasoning_events: 0
workflow_plan_events: 0
weighted_plan_events: 0
reweighted_plan_events: 0
adaptive_trigger_events: 0
adaptive_triggers: none
latest_top_priority: none
latest_top_importance_score: none
latest_top_weight_share: none
acceptance_accepted: missing
closeout_status: not_verified
closeout_tool_records: 4
closeout_tool_evidence: tool evidence: records=4 completed=2 failed=2 denied=0 validation=0 closeout=0 repair=2 changed=0 workflows=direct commands=ls -la fixtures/mva_low_value_replan 2>/dev/null || echo "Directory not found"
runtime_diet: prompt=2787 tool_schema=809 tools=4 workflow=none
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
