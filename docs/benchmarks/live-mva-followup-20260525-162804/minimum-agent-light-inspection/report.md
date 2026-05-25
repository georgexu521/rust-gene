# Live Eval Report: minimum-agent-light-inspection

- Run id: `mva-followup-20260525-162804`
- Sample: `evalsets/live_tasks/minimum-agent-light-inspection.yaml`
- Worktree: `target/live-evals/mva-followup-20260525-162804/minimum-agent-light-inspection/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/mva-followup-20260525-162804/minimum-agent-light-inspection/env`
- Test status: `ok`
- Generated: `2026-05-25 16:31:04 +0800`

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
- Output: `docs/benchmarks/live-mva-followup-20260525-162804/minimum-agent-light-inspection/agent-output.md`
- Events: `docs/benchmarks/live-mva-followup-20260525-162804/minimum-agent-light-inspection/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
runtime_diagnostic: 1
start: 1
text_chunk: 10
tool_execution_complete: 1
tool_execution_start: 1
trace_summary: 1
```

Quality signals:

```text
output_chars: 1018
diff_chars: 0
diff_files_changed: 0
tool_executions: 1
first_write_tool_index: none
forbidden_tool_uses: none
tool_errors: 0
tool_failures: 0
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 33
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
closeout_tool_records: 1
closeout_tool_evidence: tool evidence: records=1 completed=1 failed=0 denied=0 validation=0 closeout=0 repair=0 changed=0 workflows=direct commands=none
runtime_diet: prompt=2100 tool_schema=1069 tools=6 workflow=none closeout=full validation=not_applicable
adaptive_triggers: none
risk_signal: entry=ordinary runtime=none
risk_signal_reasons: ordinary change surface
trace_event_types: stop.check,agent.loop,memory.sync,context.zones,api.start,workflow.fallback,api.done,closeout,runtime.diet,completion.contract,memory.boundary,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: read_only_audit
behavior_assertions: none
behavior_assertion_status: none
output_assertions: contains=3
output_assertion_status: passed
output_assertion_missing: none
trajectory_assertions: requires_observer_outcome,requires_stop_check,max_repeated_action_count,max_scope_drift_count,max_premature_edit_count,max_invalid_action_count,requires_runtime_spine_passed
trajectory_assertion_status: passed
trajectory_assertion_missing: none
runtime_spine: coverage=6/7, status=passed, missing=none
runtime_profile: minimum_viable_agent
mva_profile_active: true
runtime_spine_detail: context=11 latest=memory_boundary_evaluated decision=6 latest=action_reviewed permission=0 latest=none tool_execution=4 latest=api_request_completed state_update=8 latest=workflow_fallback verification=1 latest=reflection_pass_completed closeout=3 latest=assistant_responded risky_tool_runs=0 risky_tool_reviewed=0 risky_tool_missing_action_review=none stop_reason=no_issue stop_terminal_status=completed stop_action=continue stop_failure_type=missing rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none action_scores=1 latest_action_score=30 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=false memory_modifier_applied=false observer_outcome_recorded=true memory_boundary_recorded=true agent_loop_steps=2 context_zones=2 completion_contract=completed
runtime_spine_trace_present: true
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: phase:context,phase:decision,phase:tool_execution,phase:state_update,phase:closeout,event:context_zones_materialized,event:action_decision_evaluated,event:action_reviewed,event:agent_loop_step_evaluated,event:stop_check_evaluated,event:completion_contract_evaluated,completion_status:completed
runtime_spine_status: passed
runtime_spine_missing: none
risky_tool_runs: 0
risky_tool_reviewed: 0
risky_tool_missing_action_review: none
agent_loop_steps: 2
context_zones_materialized: true
context_zone_task_state_empty: false
context_zone_current_decision_request_empty: false
state_transition_recorded: false
completion_contract_status: completed
completion_contract_proof_status: not_applicable
candidate_score_calibrated: false
candidate_score_disagreement: false
observer_outcome_recorded: true
memory_boundary_recorded: true
verification_proof_status: not_applicable
verification_proof_summary: validation not required for this task
premature_edit_count: 0
evidence_before_first_edit: true
scope_drift_count: 0
invalid_action_count: 0
repeated_action_count: 0
failed_action_count: 0
user_question_count: 0
unnecessary_question_count: 0
verification_attempted: true
verification_passed: true
tool_call_count: 1
llm_call_count: 2
warning: no_code_diff
failure_owner: none
outcome_score: 100
process_score: 100
efficiency_score: 100
agent_score: 100
score_penalties: none
```

Specialty signals:

```text
memory_active: true
automation_active: true
guided_debugging_active: false
guided_reasoning_active: false
weighted_planning_active: false
closeout_active: true
adaptive_workflow_active: false
active_specialty_signals: 3/7
workflow_contract_activation: entry=skipped:force repair=none
workflow_contract_events: 1
runtime_spine: coverage=6/7, status=passed, missing=none
runtime_profile: minimum_viable_agent
mva_profile_active: true
runtime_spine_detail: context=11 latest=memory_boundary_evaluated decision=6 latest=action_reviewed permission=0 latest=none tool_execution=4 latest=api_request_completed state_update=8 latest=workflow_fallback verification=1 latest=reflection_pass_completed closeout=3 latest=assistant_responded risky_tool_runs=0 risky_tool_reviewed=0 risky_tool_missing_action_review=none stop_reason=no_issue stop_terminal_status=completed stop_action=continue stop_failure_type=missing rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none action_scores=1 latest_action_score=30 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=false memory_modifier_applied=false observer_outcome_recorded=true memory_boundary_recorded=true agent_loop_steps=2 context_zones=2 completion_contract=completed
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: phase:context,phase:decision,phase:tool_execution,phase:state_update,phase:closeout,event:context_zones_materialized,event:action_decision_evaluated,event:action_reviewed,event:agent_loop_step_evaluated,event:stop_check_evaluated,event:completion_contract_evaluated,completion_status:completed
runtime_spine_status: passed
runtime_spine_missing: none
risky_tool_runs: 0
risky_tool_reviewed: 0
risky_tool_missing_action_review: none
agent_loop_steps: 2
context_zones_materialized: true
context_zone_task_state_empty: false
context_zone_current_decision_request_empty: false
state_transition_recorded: false
completion_contract_status: completed
completion_contract_proof_status: not_applicable
candidate_score_calibrated: false
candidate_score_disagreement: false
observer_outcome_recorded: true
memory_boundary_recorded: true
verification_proof_status: not_applicable
risk_signal: entry=ordinary runtime=none
risk_signal_reasons: ordinary change surface
memory_sync_events: 1
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
required_commands: 3
agent_required_commands: 0
harness_commands: 3
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
acceptance_accepted: True
closeout_status: passed
closeout_tool_records: 1
closeout_tool_evidence: tool evidence: records=1 completed=1 failed=0 denied=0 validation=0 closeout=0 repair=0 changed=0 workflows=direct commands=none
runtime_diet: prompt=2100 tool_schema=1069 tools=6 workflow=none
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

## Run Bundle

- Bundle: `docs/benchmarks/live-mva-followup-20260525-162804/minimum-agent-light-inspection/run-bundle`
- Task: `docs/benchmarks/live-mva-followup-20260525-162804/minimum-agent-light-inspection/run-bundle/task.json`
- Steps: `docs/benchmarks/live-mva-followup-20260525-162804/minimum-agent-light-inspection/run-bundle/steps.jsonl`
- Events: `docs/benchmarks/live-mva-followup-20260525-162804/minimum-agent-light-inspection/run-bundle/events.jsonl`
- Final report: `docs/benchmarks/live-mva-followup-20260525-162804/minimum-agent-light-inspection/run-bundle/final_report.md`
