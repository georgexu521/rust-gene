# Live Eval Report: minimum-agent-verification-repair

- Run id: `mva-followup-20260525-162804`
- Sample: `evalsets/live_tasks/minimum-agent-verification-repair.yaml`
- Worktree: `target/live-evals/mva-followup-20260525-162804/minimum-agent-verification-repair/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/mva-followup-20260525-162804/minimum-agent-verification-repair/env`
- Test status: `ok`
- Generated: `2026-05-25 16:32:21 +0800`

## Git Status

```text
 M fixtures/mva_verification_repair/slugify.py
?? fixtures/mva_verification_repair/__pycache__/
```

## Diff Stat

```text
 fixtures/mva_verification_repair/slugify.py | 2 +-
 1 file changed, 1 insertion(+), 1 deletion(-)
```

## Required Commands

```text
$ python3 fixtures/mva_verification_repair/test_slugify.py
.
----------------------------------------------------------------------
Ran 1 test in 0.000s

OK
[exit status: 0]

$ rg -F 'return value.strip().lower().replace(" ", "-")' fixtures/mva_verification_repair/slugify.py
    return value.strip().lower().replace(" ", "-")
[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-mva-followup-20260525-162804/minimum-agent-verification-repair/agent-output.md`
- Events: `docs/benchmarks/live-mva-followup-20260525-162804/minimum-agent-verification-repair/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
runtime_diagnostic: 1
start: 1
text_chunk: 1
tool_execution_complete: 3
tool_execution_progress: 2
tool_execution_start: 3
trace_summary: 1
```

Quality signals:

```text
output_chars: 1144
diff_chars: 368
diff_files_changed: 1
tool_executions: 3
first_write_tool_index: 3
forbidden_tool_uses: none
tool_errors: 1
tool_failures: 1
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 66
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
closeout_tool_records: 4
closeout_tool_evidence: tool evidence: records=4 completed=2 failed=2 denied=0 validation=1 closeout=2 repair=3 changed=1 workflows=code_change commands=python3 fixtures/mva_verification_repair/test_slugify.py
runtime_diet: prompt=3690 tool_schema=3950 tools=19 workflow=guarded closeout=full validation=passed:2/2 recovered_failed:1
adaptive_triggers: risk_signal_high,required_validation,first_code_change
risk_signal: entry=high runtime=none
risk_signal_reasons: required validation commands present
trace_event_types: verify.done,reflection.pass,stage.validation,acceptance.review,workflow.plan,memory.sync,workflow.fallback,closeout,runtime.diet,completion.contract,memory.boundary,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: seeded_code_change
behavior_assertions: none
behavior_assertion_status: none
output_assertions: contains_any=2
output_assertion_status: passed
output_assertion_missing: none
trajectory_assertions: evidence_before_edit,requires_observer_outcome,requires_stop_check,max_scope_drift_count,max_premature_edit_count,requires_runtime_spine_passed
trajectory_assertion_status: failed
trajectory_assertion_missing: max_scope_drift_count:1>0
runtime_spine: coverage=6/7, status=passed, missing=none
runtime_profile: minimum_viable_agent
mva_profile_active: true
runtime_spine_detail: context=15 latest=memory_boundary_evaluated decision=18 latest=workflow_plan_progress permission=0 latest=none tool_execution=8 latest=tool_completed state_update=17 latest=workflow_fallback verification=5 latest=acceptance_review_completed closeout=3 latest=assistant_responded risky_tool_runs=2 risky_tool_reviewed=2 risky_tool_missing_action_review=none stop_reason=no_issue stop_terminal_status=completed stop_action=continue stop_failure_type=unknown rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none action_scores=3 latest_action_score=24 low_action_score_count=0 phase_misaligned_actions=1 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true memory_boundary_recorded=true agent_loop_steps=4 context_zones=2 completion_contract=completed
runtime_spine_trace_present: true
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: phase:context,phase:decision,phase:tool_execution,phase:state_update,phase:verification,phase:closeout,event:context_zones_materialized,event:action_decision_evaluated,event:action_reviewed,event:agent_loop_step_evaluated,event:stop_check_evaluated,event:completion_contract_evaluated,completion_status:completed,terminal_status:completed,verification_proof_status:verified,special:verification_proof
runtime_spine_status: passed
runtime_spine_missing: none
risky_tool_runs: 2
risky_tool_reviewed: 2
risky_tool_missing_action_review: none
agent_loop_steps: 4
context_zones_materialized: true
context_zone_task_state_empty: false
context_zone_current_decision_request_empty: false
state_transition_recorded: false
completion_contract_status: completed
completion_contract_proof_status: verified
candidate_score_calibrated: false
candidate_score_disagreement: false
observer_outcome_recorded: true
memory_boundary_recorded: true
verification_proof_status: verified
verification_proof_summary: required validation passed 2/2 commands
premature_edit_count: 0
evidence_before_first_edit: true
scope_drift_count: 1
invalid_action_count: 2
repeated_action_count: 0
failed_action_count: 2
user_question_count: 0
unnecessary_question_count: 0
verification_attempted: true
verification_passed: true
tool_call_count: 3
llm_call_count: 2
warning: tool_errors_seen
warning: trajectory_assertions_not_passing
failure_owner: agent_flow
outcome_score: 65
process_score: 75
efficiency_score: 84
agent_score: 72
score_penalties: run_failed,trajectory_assertions_failed,scope_drift,invalid_action,failed_actions
```

Specialty signals:

```text
memory_active: true
automation_active: true
guided_debugging_active: false
guided_reasoning_active: true
weighted_planning_active: true
closeout_active: true
adaptive_workflow_active: true
active_specialty_signals: 6/7
workflow_contract_activation: entry=active:force repair=not_needed
workflow_contract_events: 1
runtime_spine: coverage=6/7, status=passed, missing=none
runtime_profile: minimum_viable_agent
mva_profile_active: true
runtime_spine_detail: context=15 latest=memory_boundary_evaluated decision=18 latest=workflow_plan_progress permission=0 latest=none tool_execution=8 latest=tool_completed state_update=17 latest=workflow_fallback verification=5 latest=acceptance_review_completed closeout=3 latest=assistant_responded risky_tool_runs=2 risky_tool_reviewed=2 risky_tool_missing_action_review=none stop_reason=no_issue stop_terminal_status=completed stop_action=continue stop_failure_type=unknown rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none action_scores=3 latest_action_score=24 low_action_score_count=0 phase_misaligned_actions=1 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true memory_boundary_recorded=true agent_loop_steps=4 context_zones=2 completion_contract=completed
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: phase:context,phase:decision,phase:tool_execution,phase:state_update,phase:verification,phase:closeout,event:context_zones_materialized,event:action_decision_evaluated,event:action_reviewed,event:agent_loop_step_evaluated,event:stop_check_evaluated,event:completion_contract_evaluated,completion_status:completed,terminal_status:completed,verification_proof_status:verified,special:verification_proof
runtime_spine_status: passed
runtime_spine_missing: none
risky_tool_runs: 2
risky_tool_reviewed: 2
risky_tool_missing_action_review: none
agent_loop_steps: 4
context_zones_materialized: true
context_zone_task_state_empty: false
context_zone_current_decision_request_empty: false
state_transition_recorded: false
completion_contract_status: completed
completion_contract_proof_status: verified
candidate_score_calibrated: false
candidate_score_disagreement: false
observer_outcome_recorded: true
memory_boundary_recorded: true
verification_proof_status: verified
risk_signal: entry=high runtime=none
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
required_commands: 2
agent_required_commands: 2
harness_commands: 0
required_command_status: ok
validation_events: 1
stage_validation_events: 1
tool_progress_events: 2
guided_debugging_events: 0
guided_reasoning_events: 1
workflow_plan_events: 2
weighted_plan_events: 1
reweighted_plan_events: 1
adaptive_trigger_events: 3
adaptive_triggers: risk_signal_high,required_validation,first_code_change
latest_top_priority: P3
latest_top_importance_score: 0.23000001907348633
latest_top_weight_share: 0.2705882489681244
acceptance_accepted: True
closeout_status: passed
closeout_tool_records: 4
closeout_tool_evidence: tool evidence: records=4 completed=2 failed=2 denied=0 validation=1 closeout=2 repair=3 changed=1 workflows=code_change commands=python3 fixtures/mva_verification_repair/test_slugify.py
runtime_diet: prompt=3690 tool_schema=3950 tools=19 workflow=guarded
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

- Bundle: `docs/benchmarks/live-mva-followup-20260525-162804/minimum-agent-verification-repair/run-bundle`
- Task: `docs/benchmarks/live-mva-followup-20260525-162804/minimum-agent-verification-repair/run-bundle/task.json`
- Steps: `docs/benchmarks/live-mva-followup-20260525-162804/minimum-agent-verification-repair/run-bundle/steps.jsonl`
- Events: `docs/benchmarks/live-mva-followup-20260525-162804/minimum-agent-verification-repair/run-bundle/events.jsonl`
- Final report: `docs/benchmarks/live-mva-followup-20260525-162804/minimum-agent-verification-repair/run-bundle/final_report.md`
