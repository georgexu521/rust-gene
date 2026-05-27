# Live Eval Report: core-inspection-grounding

- Run id: `flow-real-postb-20260527-110105`
- Sample: `evalsets/live_tasks/core-inspection-grounding.yaml`
- Worktree: `target/live-evals/flow-real-postb-20260527-110105/core-inspection-grounding/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/flow-real-postb-20260527-110105/core-inspection-grounding/env`
- Test status: `ok`
- Generated: `2026-05-27 11:17:56 +0800`

## Git Status

```text
```

## Diff Stat

```text
```

## Required Commands

```text
$ test -d fixtures/core_quality/inspection_target/gex
[exit status: 0]

$ test -f fixtures/core_quality/inspection_target/gex/a.txt
[exit status: 0]

$ test -f fixtures/core_quality/inspection_target/gex/.hidden
[exit status: 0]

$ test -d fixtures/core_quality/inspection_target/gex/notes
[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-flow-real-postb-20260527-110105/core-inspection-grounding/agent-output.md`
- Events: `docs/benchmarks/live-flow-real-postb-20260527-110105/core-inspection-grounding/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
runtime_diagnostic: 1
start: 1
text_chunk: 2
tool_execution_complete: 1
tool_execution_start: 1
trace_summary: 1
```

Quality signals:

```text
output_chars: 1089
diff_chars: 0
diff_files_changed: 0
tool_executions: 1
first_write_tool_index: none
forbidden_tool_uses: none
tool_errors: 0
tool_failures: 4
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 75
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
closeout_tool_records: 5
closeout_tool_evidence: tool evidence: records=5 completed=1 failed=4 denied=0 validation=4 closeout=4 repair=4 changed=0 workflows=code_change commands=ls -la fixtures/core_quality/inspection_target/gex/ | test -d fixtures/core_quality/inspection_target/gex && ec...
runtime_diet: prompt=5010 tool_schema=3950 tools=19 workflow=guarded closeout=full validation=passed:4/4 recovered_failed:4
adaptive_triggers: risk_signal_high,required_validation
risk_signal: entry=high runtime=high
risk_signal_reasons: required validation commands present; complex required-validation surface
trace_event_types: stop.check,agent.loop,stop.check,agent.loop,risk.signal,guided.debug,closeout,execution.report,memory.proposal,runtime.diet,completion.contract,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: audit_or_regression_check
behavior_assertions: none
behavior_assertion_status: none
output_assertions: none
output_assertion_status: none
output_assertion_missing: none
trajectory_assertions: none
trajectory_assertion_status: none
trajectory_assertion_missing: none
runtime_spine: coverage=6/7, status=passed, missing=none
runtime_profile: none
mva_profile_active: false
runtime_spine_detail: context=14 latest=runtime_diet_report decision=18 latest=risk_signal_assessed permission=0 latest=none tool_execution=12 latest=tool_completed state_update=21 latest=agent_loop_step_evaluated verification=2 latest=guided_debugging_completed closeout=3 latest=assistant_responded risky_tool_runs=5 risky_tool_reviewed=5 risky_tool_missing_action_review=none gate_outcomes=total=6, protective_block=0, recoverable_friction=4, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=2 stop_reason=consecutive_validation_failures stop_terminal_status=failed stop_action=recover stop_failure_type=validation_failure rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none route_recovery=events=0, read_search=false, mutation_blocked=false, safety=missing action_scores=4 latest_action_score=6 low_action_score_count=0 phase_misaligned_actions=4 observer_modifier_applied=false memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=none runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=2 provider_protocol_repairs=4 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=4 context_zone_duplicate_blocks_removed=0 context_zone_provenance_markers=4 agent_loop_steps=4 context_zones=2 completion_contract=completed
runtime_spine_trace_present: true
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: phase:context,phase:decision,phase:tool_execution,phase:state_update,phase:closeout,event:action_decision_evaluated,event:stop_check_evaluated,special:verification_proof
runtime_spine_status: passed
runtime_spine_missing: none
risky_tool_runs: 5
risky_tool_reviewed: 5
risky_tool_missing_action_review: none
gate_outcomes: total=6, protective_block=0, recoverable_friction=4, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=2
gate_outcome_records: action_review:allow:harmless_pass,action_review:revise:recoverable_friction,action_review:revise:recoverable_friction,action_review:revise:recoverable_friction,action_review:revise:recoverable_friction,closeout:passed:harmless_pass
gate_outcome_total: 6
gate_outcome_protective_blocks: 0
gate_outcome_recoverable_friction: 4
gate_outcome_unrecovered_blocks: 0
gate_outcome_suspected_false_positives: 0
gate_outcome_policy_correct_but_ux_costly: 0
gate_outcome_harmless_passes: 2
gate_outcome_failure_owners: none
route_recovery: events=0, read_search=false, mutation_blocked=false, safety=missing
route_recovery_events: 0
route_recovery_failure_types: none
route_recovery_kinds: none
route_recovery_read_search_expanded: false
route_recovery_mutation_blocked: false
route_recovery_safety_monotonic: missing
route_recovery_unsafe_mutation_expansion: false
agent_loop_steps: 4
context_zones_materialized: true
context_zone_task_state_empty: false
context_zone_current_decision_request_empty: false
context_zone_envelope_messages: 1
context_zone_source_messages: 4
context_zone_duplicate_blocks_removed: 0
context_zone_provenance_markers: 4
state_transition_recorded: false
completion_contract_status: completed
completion_contract_proof_status: verified
candidate_score_calibrated: false
candidate_score_disagreement: false
observer_outcome_recorded: true
memory_boundary_recorded: true
verification_proof_status: verified
verification_proof_summary: required validation passed 4/4 commands
verification_proof_kinds: command_passed,required_validation_passed
verification_proof_support_status: verified
verification_proof_support_summary: verified by command_passed,required_validation_passed
verification_proof_supports_verified: true
verification_proof_residual_risk: false
premature_edit_count: 0
evidence_before_first_edit: true
scope_drift_count: 0
invalid_action_count: 4
repeated_action_count: 0
failed_action_count: 4
user_question_count: 0
unnecessary_question_count: 0
verification_attempted: true
verification_passed: true
tool_call_count: 1
llm_call_count: 2
warning: no_code_diff
failure_owner: none
outcome_score: 100
process_score: 80
efficiency_score: 75
agent_score: 89
score_penalties: invalid_action,failed_actions
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
runtime_spine: coverage=6/7, status=passed, missing=none
runtime_profile: none
mva_profile_active: false
runtime_spine_detail: context=14 latest=runtime_diet_report decision=18 latest=risk_signal_assessed permission=0 latest=none tool_execution=12 latest=tool_completed state_update=21 latest=agent_loop_step_evaluated verification=2 latest=guided_debugging_completed closeout=3 latest=assistant_responded risky_tool_runs=5 risky_tool_reviewed=5 risky_tool_missing_action_review=none gate_outcomes=total=6, protective_block=0, recoverable_friction=4, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=2 stop_reason=consecutive_validation_failures stop_terminal_status=failed stop_action=recover stop_failure_type=validation_failure rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none route_recovery=events=0, read_search=false, mutation_blocked=false, safety=missing action_scores=4 latest_action_score=6 low_action_score_count=0 phase_misaligned_actions=4 observer_modifier_applied=false memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=none runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=2 provider_protocol_repairs=4 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=4 context_zone_duplicate_blocks_removed=0 context_zone_provenance_markers=4 agent_loop_steps=4 context_zones=2 completion_contract=completed
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: phase:context,phase:decision,phase:tool_execution,phase:state_update,phase:closeout,event:action_decision_evaluated,event:stop_check_evaluated,special:verification_proof
runtime_spine_status: passed
runtime_spine_missing: none
risky_tool_runs: 5
risky_tool_reviewed: 5
risky_tool_missing_action_review: none
gate_outcomes: total=6, protective_block=0, recoverable_friction=4, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=2
gate_outcome_records: action_review:allow:harmless_pass,action_review:revise:recoverable_friction,action_review:revise:recoverable_friction,action_review:revise:recoverable_friction,action_review:revise:recoverable_friction,closeout:passed:harmless_pass
gate_outcome_total: 6
gate_outcome_protective_blocks: 0
gate_outcome_recoverable_friction: 4
gate_outcome_unrecovered_blocks: 0
gate_outcome_suspected_false_positives: 0
gate_outcome_policy_correct_but_ux_costly: 0
gate_outcome_harmless_passes: 2
gate_outcome_failure_owners: none
agent_loop_steps: 4
context_zones_materialized: true
context_zone_task_state_empty: false
context_zone_current_decision_request_empty: false
context_zone_envelope_messages: 1
context_zone_source_messages: 4
context_zone_duplicate_blocks_removed: 0
context_zone_provenance_markers: 4
state_transition_recorded: false
completion_contract_status: completed
completion_contract_proof_status: verified
candidate_score_calibrated: false
candidate_score_disagreement: false
observer_outcome_recorded: true
memory_boundary_recorded: true
verification_proof_status: verified
verification_proof_summary: required validation passed 4/4 commands
verification_proof_kinds: command_passed,required_validation_passed
verification_proof_support_status: verified
verification_proof_support_summary: verified by command_passed,required_validation_passed
verification_proof_supports_verified: true
verification_proof_residual_risk: false
risk_signal: entry=high runtime=high
risk_signal_reasons: required validation commands present; complex required-validation surface
memory_sync_events: 1
memory_tool_calls: 0
retrieval_sources: Project,Session
memory_candidate_typed: false
memory_candidate_has_evidence: false
memory_proposal_recorded: true
memory_proposal_status: not_applicable
memory_proposal_candidates: 0
memory_proposal_kinds: none
memory_proposal_evidence_items: 0
memory_proposal_write_policy: review_required
memory_proposal_write_performed: false
memory_record_used: false
memory_use_count_updated: false
memory_failure_lesson_promoted: false
memory_action_weight_changed: false
memory_stale_demoted: false
memory_scope_correct: false
required_commands: 4
agent_required_commands: 4
harness_commands: 0
required_command_status: ok
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
latest_top_weight_share: 1.0
acceptance_accepted: True
closeout_status: passed
closeout_tool_records: 5
closeout_tool_evidence: tool evidence: records=5 completed=1 failed=4 denied=0 validation=4 closeout=4 repair=4 changed=0 workflows=code_change commands=ls -la fixtures/core_quality/inspection_target/gex/ | test -d fixtures/core_quality/inspection_target/gex && ec...
runtime_diet: prompt=5010 tool_schema=3950 tools=19 workflow=guarded
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

- Bundle: `docs/benchmarks/live-flow-real-postb-20260527-110105/core-inspection-grounding/run-bundle`
- Task: `docs/benchmarks/live-flow-real-postb-20260527-110105/core-inspection-grounding/run-bundle/task.json`
- Steps: `docs/benchmarks/live-flow-real-postb-20260527-110105/core-inspection-grounding/run-bundle/steps.jsonl`
- Events: `docs/benchmarks/live-flow-real-postb-20260527-110105/core-inspection-grounding/run-bundle/events.jsonl`
- Final report: `docs/benchmarks/live-flow-real-postb-20260527-110105/core-inspection-grounding/run-bundle/final_report.md`
