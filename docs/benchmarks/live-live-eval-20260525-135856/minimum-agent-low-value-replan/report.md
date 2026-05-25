# Live Eval Report: minimum-agent-low-value-replan

- Run id: `live-eval-20260525-135856`
- Sample: `evalsets/live_tasks/minimum-agent-low-value-replan.yaml`
- Worktree: `target/live-evals/live-eval-20260525-135856/minimum-agent-low-value-replan/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/live-eval-20260525-135856/minimum-agent-low-value-replan/env`
- Test status: `ok`
- Generated: `2026-05-25 13:59:29 +0800`

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
- Output: `docs/benchmarks/live-live-eval-20260525-135856/minimum-agent-low-value-replan/agent-output.md`
- Events: `docs/benchmarks/live-live-eval-20260525-135856/minimum-agent-low-value-replan/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
runtime_diagnostic: 1
start: 1
text_chunk: 1
tool_execution_complete: 3
tool_execution_start: 3
trace_summary: 1
```

Quality signals:

```text
output_chars: 214
diff_chars: 0
diff_files_changed: 0
tool_executions: 3
first_write_tool_index: none
forbidden_tool_uses: none
tool_errors: 0
tool_failures: 0
has_closeout: false
has_validation_claim: false
trace_status: Completed
trace_events: 53
test_status: ok
verification_passed: false
stage_validation_passed: false
acceptance_accepted: None
closeout_status: missing
closeout_tool_records: 0
closeout_tool_evidence: missing
runtime_diet: prompt=3068 tool_schema=809 tools=4 workflow=none closeout=none validation=none
adaptive_triggers: none
risk_signal: entry=ordinary runtime=none
risk_signal_reasons: ordinary change surface
trace_event_types: stop.check,agent.loop,memory.sync,context.zones,api.start,workflow.fallback,api.done,action.candidates,runtime.diet,completion.contract,memory.boundary,assistant
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
runtime_spine_detail: context=14 latest=memory_boundary_evaluated decision=12 latest=candidate_actions_evaluated permission=0 latest=none tool_execution=9 latest=api_request_completed state_update=15 latest=workflow_fallback verification=1 latest=reflection_pass_completed closeout=2 latest=assistant_responded risky_tool_runs=1 risky_tool_reviewed=1 risky_tool_missing_action_review=none stop_reason=no_issue stop_terminal_status=missing stop_action=continue stop_failure_type=unknown rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none action_scores=3 latest_action_score=29 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true memory_boundary_recorded=true agent_loop_steps=4 context_zones=3 completion_contract=completed
runtime_spine_trace_present: true
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: phase:context,phase:decision,phase:tool_execution,phase:state_update,phase:closeout,event:context_zones_materialized,event:action_decision_evaluated,event:action_reviewed,event:agent_loop_step_evaluated,event:stop_check_evaluated,event:completion_contract_evaluated,completion_status:completed
runtime_spine_status: passed
runtime_spine_missing: none
risky_tool_runs: 1
risky_tool_reviewed: 1
risky_tool_missing_action_review: none
agent_loop_steps: 4
context_zones_materialized: true
context_zone_task_state_empty: false
context_zone_current_decision_request_empty: false
state_transition_recorded: false
completion_contract_status: completed
completion_contract_proof_status: missing
candidate_score_calibrated: false
candidate_score_disagreement: false
observer_outcome_recorded: true
memory_boundary_recorded: true
verification_proof_status: missing
verification_proof_summary: missing
warning: tool_run_without_closeout
warning: no_code_diff
failure_owner: agent_flow
```

Specialty signals:

```text
memory_active: true
automation_active: true
guided_debugging_active: false
guided_reasoning_active: false
weighted_planning_active: false
closeout_active: false
adaptive_workflow_active: false
active_specialty_signals: 2/7
workflow_contract_activation: entry=skipped:force repair=none
workflow_contract_events: 1
runtime_spine: coverage=6/7, status=passed, missing=none
runtime_profile: minimum_viable_agent
mva_profile_active: true
runtime_spine_detail: context=14 latest=memory_boundary_evaluated decision=12 latest=candidate_actions_evaluated permission=0 latest=none tool_execution=9 latest=api_request_completed state_update=15 latest=workflow_fallback verification=1 latest=reflection_pass_completed closeout=2 latest=assistant_responded risky_tool_runs=1 risky_tool_reviewed=1 risky_tool_missing_action_review=none stop_reason=no_issue stop_terminal_status=missing stop_action=continue stop_failure_type=unknown rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none action_scores=3 latest_action_score=29 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true memory_boundary_recorded=true agent_loop_steps=4 context_zones=3 completion_contract=completed
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: phase:context,phase:decision,phase:tool_execution,phase:state_update,phase:closeout,event:context_zones_materialized,event:action_decision_evaluated,event:action_reviewed,event:agent_loop_step_evaluated,event:stop_check_evaluated,event:completion_contract_evaluated,completion_status:completed
runtime_spine_status: passed
runtime_spine_missing: none
risky_tool_runs: 1
risky_tool_reviewed: 1
risky_tool_missing_action_review: none
agent_loop_steps: 4
context_zones_materialized: true
context_zone_task_state_empty: false
context_zone_current_decision_request_empty: false
state_transition_recorded: false
completion_contract_status: completed
completion_contract_proof_status: missing
candidate_score_calibrated: false
candidate_score_disagreement: false
observer_outcome_recorded: true
memory_boundary_recorded: true
verification_proof_status: missing
risk_signal: entry=ordinary runtime=none
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
guided_debugging_events: 0
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
closeout_status: missing
closeout_tool_records: 0
closeout_tool_evidence: missing
runtime_diet: prompt=3068 tool_schema=809 tools=4 workflow=none
note: guided debugging is expected only after a blocker or failed validation
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
