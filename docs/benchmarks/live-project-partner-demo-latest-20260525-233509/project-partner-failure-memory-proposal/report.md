# Live Eval Report: project-partner-failure-memory-proposal

- Run id: `project-partner-demo-latest-20260525-233509`
- Sample: `evalsets/live_tasks/project-partner-failure-memory-proposal.yaml`
- Worktree: `target/live-evals/project-partner-demo-latest-20260525-233509/project-partner-failure-memory-proposal/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/project-partner-demo-latest-20260525-233509/project-partner-failure-memory-proposal/env`
- Test status: `ok`
- Generated: `2026-05-26 00:13:14 +0800`

## Git Status

```text
 M fixtures/project_partner_failure/slugify.py
```

## Diff Stat

```text
 fixtures/project_partner_failure/slugify.py | 2 +-
 1 file changed, 1 insertion(+), 1 deletion(-)
```

## Required Commands

```text
$ python3 fixtures/project_partner_failure/test_slugify.py
.
----------------------------------------------------------------------
Ran 1 test in 0.000s

OK
[exit status: 0]

$ rg 'lower\(\)' fixtures/project_partner_failure/slugify.py
    return text.strip().lower().replace(" ", "-")
[exit status: 0]

$ rg 'replace\(" ", "-"' fixtures/project_partner_failure/slugify.py
    return text.strip().lower().replace(" ", "-")
[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-project-partner-demo-latest-20260525-233509/project-partner-failure-memory-proposal/agent-output.md`
- Events: `docs/benchmarks/live-project-partner-demo-latest-20260525-233509/project-partner-failure-memory-proposal/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
runtime_diagnostic: 1
start: 1
text_chunk: 1
tool_execution_complete: 3
tool_execution_progress: 1
tool_execution_start: 3
trace_summary: 1
```

Quality signals:

```text
output_chars: 1805
diff_chars: 365
diff_files_changed: 1
tool_executions: 3
first_write_tool_index: 3
forbidden_tool_uses: none
tool_errors: 0
tool_failures: 0
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 67
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
closeout_tool_records: 5
closeout_tool_evidence: tool evidence: records=5 completed=3 failed=2 denied=0 validation=0 closeout=1 repair=3 changed=1 workflows=code_change commands=none
runtime_diet: prompt=4983 tool_schema=3950 tools=19 workflow=guarded closeout=full validation=passed:3/3
adaptive_triggers: risk_signal_high,required_validation,first_code_change
risk_signal: entry=high runtime=none
risk_signal_reasons: required validation commands present; runtime risk keyword in request: memory
trace_event_types: stage.validation,acceptance.review,workflow.plan,memory.boundary,memory.sync,workflow.fallback,closeout,execution.report,memory.proposal,runtime.diet,completion.contract,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: seeded_code_change
behavior_assertions: none
behavior_assertion_status: none
output_assertions: contains=1,contains_any=1
output_assertion_status: passed
output_assertion_missing: none
trajectory_assertions: evidence_before_edit,requires_observer_outcome,requires_stop_check,max_scope_drift_count,max_premature_edit_count,requires_runtime_spine_passed
trajectory_assertion_status: passed
trajectory_assertion_missing: none
runtime_spine: coverage=6/7, status=passed, missing=none
runtime_profile: project_partner_alignment
mva_profile_active: false
runtime_spine_detail: context=16 latest=runtime_diet_report decision=14 latest=workflow_plan_progress permission=0 latest=none tool_execution=8 latest=tool_completed state_update=17 latest=workflow_fallback verification=5 latest=acceptance_review_completed closeout=3 latest=assistant_responded risky_tool_runs=1 risky_tool_reviewed=1 risky_tool_missing_action_review=none stop_reason=no_issue stop_terminal_status=completed stop_action=continue stop_failure_type=unknown rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none action_scores=1 latest_action_score=20 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=none runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=2 provider_protocol_repairs=11 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true agent_loop_steps=4 context_zones=2 completion_contract=completed
runtime_spine_trace_present: true
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: phase:context,phase:decision,phase:tool_execution,phase:state_update,phase:verification,phase:closeout,event:task_contract_materialized,event:context_pack_materialized,event:action_reviewed,event:tool_observation_recorded,event:completion_contract_evaluated,event:execution_report_prepared,event:memory_proposal_prepared,completion_status:completed,verification_proof_status:verified,special:verification_proof
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
completion_contract_proof_status: verified
candidate_score_calibrated: false
candidate_score_disagreement: false
observer_outcome_recorded: true
memory_boundary_recorded: true
verification_proof_status: verified
verification_proof_summary: required validation passed 3/3 commands
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
tool_call_count: 3
llm_call_count: 2
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
weighted_planning_active: true
closeout_active: true
adaptive_workflow_active: true
active_specialty_signals: 5/7
workflow_contract_activation: entry=active:force repair=not_needed
workflow_contract_events: 1
runtime_spine: coverage=6/7, status=passed, missing=none
runtime_profile: project_partner_alignment
mva_profile_active: false
runtime_spine_detail: context=16 latest=runtime_diet_report decision=14 latest=workflow_plan_progress permission=0 latest=none tool_execution=8 latest=tool_completed state_update=17 latest=workflow_fallback verification=5 latest=acceptance_review_completed closeout=3 latest=assistant_responded risky_tool_runs=1 risky_tool_reviewed=1 risky_tool_missing_action_review=none stop_reason=no_issue stop_terminal_status=completed stop_action=continue stop_failure_type=unknown rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none action_scores=1 latest_action_score=20 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=none runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=2 provider_protocol_repairs=11 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true agent_loop_steps=4 context_zones=2 completion_contract=completed
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: phase:context,phase:decision,phase:tool_execution,phase:state_update,phase:verification,phase:closeout,event:task_contract_materialized,event:context_pack_materialized,event:action_reviewed,event:tool_observation_recorded,event:completion_contract_evaluated,event:execution_report_prepared,event:memory_proposal_prepared,completion_status:completed,verification_proof_status:verified,special:verification_proof
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
completion_contract_proof_status: verified
candidate_score_calibrated: false
candidate_score_disagreement: false
observer_outcome_recorded: true
memory_boundary_recorded: true
verification_proof_status: verified
risk_signal: entry=high runtime=none
risk_signal_reasons: required validation commands present; runtime risk keyword in request: memory
memory_sync_events: 2
memory_tool_calls: 0
retrieval_sources: Project
memory_candidate_typed: true
memory_candidate_has_evidence: true
memory_proposal_recorded: true
memory_proposal_status: proposed
memory_proposal_candidates: 1
memory_proposal_kinds: successful_fix
memory_proposal_evidence_items: 9
memory_proposal_write_policy: review_required
memory_proposal_write_performed: false
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
validation_events: 1
stage_validation_events: 1
tool_progress_events: 1
guided_debugging_events: 0
guided_reasoning_events: 0
workflow_plan_events: 2
weighted_plan_events: 1
reweighted_plan_events: 1
adaptive_trigger_events: 3
adaptive_triggers: risk_signal_high,required_validation,first_code_change
latest_top_priority: P0
latest_top_importance_score: 0.9500000476837158
latest_top_weight_share: 0.2567567527294159
acceptance_accepted: True
closeout_status: passed
closeout_tool_records: 5
closeout_tool_evidence: tool evidence: records=5 completed=3 failed=2 denied=0 validation=0 closeout=1 repair=3 changed=1 workflows=code_change commands=none
runtime_diet: prompt=4983 tool_schema=3950 tools=19 workflow=guarded
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

- Bundle: `docs/benchmarks/live-project-partner-demo-latest-20260525-233509/project-partner-failure-memory-proposal/run-bundle`
- Task: `docs/benchmarks/live-project-partner-demo-latest-20260525-233509/project-partner-failure-memory-proposal/run-bundle/task.json`
- Steps: `docs/benchmarks/live-project-partner-demo-latest-20260525-233509/project-partner-failure-memory-proposal/run-bundle/steps.jsonl`
- Events: `docs/benchmarks/live-project-partner-demo-latest-20260525-233509/project-partner-failure-memory-proposal/run-bundle/events.jsonl`
- Final report: `docs/benchmarks/live-project-partner-demo-latest-20260525-233509/project-partner-failure-memory-proposal/run-bundle/final_report.md`
