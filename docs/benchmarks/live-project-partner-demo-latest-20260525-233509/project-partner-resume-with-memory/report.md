# Live Eval Report: project-partner-resume-with-memory

- Run id: `project-partner-demo-latest-20260525-233509`
- Sample: `evalsets/live_tasks/project-partner-resume-with-memory.yaml`
- Worktree: `target/live-evals/project-partner-demo-latest-20260525-233509/project-partner-resume-with-memory/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/project-partner-demo-latest-20260525-233509/project-partner-resume-with-memory/env`
- Test status: `ok`
- Generated: `2026-05-25 23:36:51 +0800`

## Git Status

```text
```

## Diff Stat

```text
```

## Required Commands

```text
$ test -f fixtures/project_partner_resume/memory/project.md
[exit status: 0]

$ test -f fixtures/project_partner_resume/reports/previous_execution_report.json
[exit status: 0]

$ rg 'CSV export' fixtures/project_partner_resume
fixtures/project_partner_resume/reports/previous_execution_report.json:  "risks": ["CSV export is not implemented yet"],
fixtures/project_partner_resume/reports/previous_execution_report.json:  "next_steps": ["Implement CSV export before adding login or cloud sync"]
fixtures/project_partner_resume/memory/project.md:- Next product goal: add CSV export for recorded strain rows.
[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-project-partner-demo-latest-20260525-233509/project-partner-resume-with-memory/agent-output.md`
- Events: `docs/benchmarks/live-project-partner-demo-latest-20260525-233509/project-partner-resume-with-memory/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
runtime_diagnostic: 1
start: 1
text_chunk: 2
tool_execution_complete: 5
tool_execution_start: 5
trace_summary: 1
```

Quality signals:

```text
output_chars: 1537
diff_chars: 0
diff_files_changed: 0
tool_executions: 5
first_write_tool_index: none
forbidden_tool_uses: none
tool_errors: 0
tool_failures: 0
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 80
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
closeout_tool_records: 9
closeout_tool_evidence: tool evidence: records=9 completed=5 failed=4 denied=0 validation=0 closeout=0 repair=4 changed=0 workflows=direct commands=none
runtime_diet: prompt=4097 tool_schema=1069 tools=6 workflow=none closeout=full validation=not_applicable
adaptive_triggers: none
risk_signal: entry=ordinary runtime=none
risk_signal_reasons: ordinary change surface
trace_event_types: memory.sync,context.zones,api.start,provider.protocol,workflow.fallback,api.done,closeout,execution.report,memory.proposal,runtime.diet,completion.contract,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: read_only_audit
behavior_assertions: none
behavior_assertion_status: none
output_assertions: contains=2,not_contains=1
output_assertion_status: passed
output_assertion_missing: none
trajectory_assertions: requires_observer_outcome,requires_stop_check,max_repeated_action_count,max_scope_drift_count,max_premature_edit_count,requires_runtime_spine_passed
trajectory_assertion_status: passed
trajectory_assertion_missing: none
runtime_spine: coverage=6/7, status=passed, missing=none
runtime_profile: project_partner_alignment
mva_profile_active: false
runtime_spine_detail: context=23 latest=runtime_diet_report decision=12 latest=action_reviewed permission=0 latest=none tool_execution=14 latest=api_request_completed state_update=23 latest=workflow_fallback verification=1 latest=reflection_pass_completed closeout=3 latest=assistant_responded risky_tool_runs=0 risky_tool_reviewed=0 risky_tool_missing_action_review=none stop_reason=no_issue stop_terminal_status=completed stop_action=continue stop_failure_type=unknown rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none action_scores=4 latest_action_score=36 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=none runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=4 provider_protocol_repairs=24 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true agent_loop_steps=6 context_zones=4 completion_contract=completed
runtime_spine_trace_present: true
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: phase:context,phase:decision,phase:tool_execution,phase:state_update,phase:closeout,event:task_contract_materialized,event:context_pack_materialized,event:action_reviewed,event:tool_observation_recorded,event:completion_contract_evaluated,event:execution_report_prepared,completion_status:completed
runtime_spine_status: passed
runtime_spine_missing: none
risky_tool_runs: 0
risky_tool_reviewed: 0
risky_tool_missing_action_review: none
agent_loop_steps: 6
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
tool_call_count: 5
llm_call_count: 4
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
runtime_profile: project_partner_alignment
mva_profile_active: false
runtime_spine_detail: context=23 latest=runtime_diet_report decision=12 latest=action_reviewed permission=0 latest=none tool_execution=14 latest=api_request_completed state_update=23 latest=workflow_fallback verification=1 latest=reflection_pass_completed closeout=3 latest=assistant_responded risky_tool_runs=0 risky_tool_reviewed=0 risky_tool_missing_action_review=none stop_reason=no_issue stop_terminal_status=completed stop_action=continue stop_failure_type=unknown rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none action_scores=4 latest_action_score=36 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=none runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=4 provider_protocol_repairs=24 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true agent_loop_steps=6 context_zones=4 completion_contract=completed
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: phase:context,phase:decision,phase:tool_execution,phase:state_update,phase:closeout,event:task_contract_materialized,event:context_pack_materialized,event:action_reviewed,event:tool_observation_recorded,event:completion_contract_evaluated,event:execution_report_prepared,completion_status:completed
runtime_spine_status: passed
runtime_spine_missing: none
risky_tool_runs: 0
risky_tool_reviewed: 0
risky_tool_missing_action_review: none
agent_loop_steps: 6
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
memory_sync_events: 3
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
closeout_tool_records: 9
closeout_tool_evidence: tool evidence: records=9 completed=5 failed=4 denied=0 validation=0 closeout=0 repair=4 changed=0 workflows=direct commands=none
runtime_diet: prompt=4097 tool_schema=1069 tools=6 workflow=none
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

- Bundle: `docs/benchmarks/live-project-partner-demo-latest-20260525-233509/project-partner-resume-with-memory/run-bundle`
- Task: `docs/benchmarks/live-project-partner-demo-latest-20260525-233509/project-partner-resume-with-memory/run-bundle/task.json`
- Steps: `docs/benchmarks/live-project-partner-demo-latest-20260525-233509/project-partner-resume-with-memory/run-bundle/steps.jsonl`
- Events: `docs/benchmarks/live-project-partner-demo-latest-20260525-233509/project-partner-resume-with-memory/run-bundle/events.jsonl`
- Final report: `docs/benchmarks/live-project-partner-demo-latest-20260525-233509/project-partner-resume-with-memory/run-bundle/final_report.md`
