# Live Eval Report: minimum-agent-low-value-replan

- Run id: `mva-followup-20260525-162804`
- Sample: `evalsets/live_tasks/minimum-agent-low-value-replan.yaml`
- Worktree: `target/live-evals/mva-followup-20260525-162804/minimum-agent-low-value-replan/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/mva-followup-20260525-162804/minimum-agent-low-value-replan/env`
- Test status: `ok`
- Generated: `2026-05-25 16:34:02 +0800`

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
- Output: `docs/benchmarks/live-mva-followup-20260525-162804/minimum-agent-low-value-replan/agent-output.md`
- Events: `docs/benchmarks/live-mva-followup-20260525-162804/minimum-agent-low-value-replan/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
runtime_diagnostic: 1
start: 1
text_chunk: 2
tool_execution_complete: 3
tool_execution_start: 3
trace_summary: 1
```

Quality signals:

```text
output_chars: 886
diff_chars: 0
diff_files_changed: 0
tool_executions: 3
first_write_tool_index: none
forbidden_tool_uses: none
tool_errors: 0
tool_failures: 0
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 56
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
closeout_tool_records: 5
closeout_tool_evidence: tool evidence: records=5 completed=3 failed=2 denied=0 validation=0 closeout=0 repair=2 changed=0 workflows=direct commands=none
runtime_diet: prompt=2761 tool_schema=1069 tools=6 workflow=none closeout=full validation=not_applicable
adaptive_triggers: none
risk_signal: entry=ordinary runtime=none
risk_signal_reasons: ordinary change surface
trace_event_types: agent.loop,memory.sync,context.zones,api.start,workflow.fallback,api.done,action.candidates,closeout,runtime.diet,completion.contract,memory.boundary,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: read_only_audit
behavior_assertions: none
behavior_assertion_status: none
output_assertions: contains=1,contains_any=1,not_contains=1
output_assertion_status: failed
output_assertion_missing: contains:missing-target-token-7391;contains_any:没有找到|未找到|not found|missing
trajectory_assertions: requires_stop_check,max_repeated_action_count,max_scope_drift_count,max_premature_edit_count,max_invalid_action_count,requires_runtime_spine_passed
trajectory_assertion_status: failed
trajectory_assertion_missing: max_scope_drift_count:2>0
runtime_spine: coverage=7/7, status=passed, missing=none
runtime_profile: minimum_viable_agent
mva_profile_active: true
runtime_spine_detail: context=14 latest=memory_boundary_evaluated decision=12 latest=candidate_actions_evaluated permission=2 latest=goal_drift_detected tool_execution=9 latest=api_request_completed state_update=15 latest=workflow_fallback verification=1 latest=reflection_pass_completed closeout=3 latest=assistant_responded risky_tool_runs=0 risky_tool_reviewed=0 risky_tool_missing_action_review=none stop_reason=no_issue stop_terminal_status=completed stop_action=continue stop_failure_type=unknown rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none action_scores=3 latest_action_score=29 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true memory_boundary_recorded=true agent_loop_steps=4 context_zones=3 completion_contract=completed
runtime_spine_trace_present: true
runtime_spine_phase_coverage: 7/7
runtime_spine_observed_phases: context,decision,permission,tool_execution,state_update,verification,closeout
runtime_spine_assertions: phase:context,phase:decision,phase:tool_execution,phase:state_update,phase:closeout,event:context_zones_materialized,event:action_decision_evaluated,event:action_reviewed,event:agent_loop_step_evaluated,event:stop_check_evaluated,event:completion_contract_evaluated,completion_status:completed
runtime_spine_status: passed
runtime_spine_missing: none
risky_tool_runs: 0
risky_tool_reviewed: 0
risky_tool_missing_action_review: none
agent_loop_steps: 4
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
scope_drift_count: 2
invalid_action_count: 2
repeated_action_count: 0
failed_action_count: 0
user_question_count: 0
unnecessary_question_count: 0
verification_attempted: true
verification_passed: true
tool_call_count: 3
llm_call_count: 3
warning: no_code_diff
warning: output_assertions_not_passing
warning: trajectory_assertions_not_passing
failure_owner: agent_flow
outcome_score: 55
process_score: 60
efficiency_score: 100
agent_score: 66
score_penalties: run_failed,output_assertions_failed,trajectory_assertions_failed,scope_drift,invalid_action
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
runtime_spine: coverage=7/7, status=passed, missing=none
runtime_profile: minimum_viable_agent
mva_profile_active: true
runtime_spine_detail: context=14 latest=memory_boundary_evaluated decision=12 latest=candidate_actions_evaluated permission=2 latest=goal_drift_detected tool_execution=9 latest=api_request_completed state_update=15 latest=workflow_fallback verification=1 latest=reflection_pass_completed closeout=3 latest=assistant_responded risky_tool_runs=0 risky_tool_reviewed=0 risky_tool_missing_action_review=none stop_reason=no_issue stop_terminal_status=completed stop_action=continue stop_failure_type=unknown rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none action_scores=3 latest_action_score=29 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true memory_boundary_recorded=true agent_loop_steps=4 context_zones=3 completion_contract=completed
runtime_spine_phase_coverage: 7/7
runtime_spine_observed_phases: context,decision,permission,tool_execution,state_update,verification,closeout
runtime_spine_assertions: phase:context,phase:decision,phase:tool_execution,phase:state_update,phase:closeout,event:context_zones_materialized,event:action_decision_evaluated,event:action_reviewed,event:agent_loop_step_evaluated,event:stop_check_evaluated,event:completion_contract_evaluated,completion_status:completed
runtime_spine_status: passed
runtime_spine_missing: none
risky_tool_runs: 0
risky_tool_reviewed: 0
risky_tool_missing_action_review: none
agent_loop_steps: 4
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
acceptance_accepted: True
closeout_status: passed
closeout_tool_records: 5
closeout_tool_evidence: tool evidence: records=5 completed=3 failed=2 denied=0 validation=0 closeout=0 repair=2 changed=0 workflows=direct commands=none
runtime_diet: prompt=2761 tool_schema=1069 tools=6 workflow=none
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

- Bundle: `docs/benchmarks/live-mva-followup-20260525-162804/minimum-agent-low-value-replan/run-bundle`
- Task: `docs/benchmarks/live-mva-followup-20260525-162804/minimum-agent-low-value-replan/run-bundle/task.json`
- Steps: `docs/benchmarks/live-mva-followup-20260525-162804/minimum-agent-low-value-replan/run-bundle/steps.jsonl`
- Events: `docs/benchmarks/live-mva-followup-20260525-162804/minimum-agent-low-value-replan/run-bundle/events.jsonl`
- Final report: `docs/benchmarks/live-mva-followup-20260525-162804/minimum-agent-low-value-replan/run-bundle/final_report.md`
