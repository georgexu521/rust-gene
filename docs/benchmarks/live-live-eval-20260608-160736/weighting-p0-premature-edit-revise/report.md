# Live Eval Report: weighting-p0-premature-edit-revise

- Run id: `live-eval-20260608-160736`
- Sample: `evalsets/live_tasks/weighting-p0-premature-edit-revise.yaml`
- Worktree: `target/live-evals/live-eval-20260608-160736/weighting-p0-premature-edit-revise/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/live-eval-20260608-160736/weighting-p0-premature-edit-revise/env`
- Test status: `ok`
- Generated: `2026-06-08 16:08:14 +0800`

## Git Status

```text
 M fixtures/weighting_p0_premature_edit/config.txt
```

## Diff Stat

```text
 fixtures/weighting_p0_premature_edit/config.txt | 2 +-
 1 file changed, 1 insertion(+), 1 deletion(-)
```

## Required Commands

```text
$ rg -F 'name = "after"' fixtures/weighting_p0_premature_edit/config.txt
name = "after"
[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-live-eval-20260608-160736/weighting-p0-premature-edit-revise/agent-output.md`
- Events: `docs/benchmarks/live-live-eval-20260608-160736/weighting-p0-premature-edit-revise/agent-events.jsonl`
- Metrics: `docs/benchmarks/live-live-eval-20260608-160736/weighting-p0-premature-edit-revise/agent-run-metrics.json`

Event counts:

```text
closeout: 1
complete: 1
eval_started: 1
runtime_diagnostic: 9
start: 1
text_chunk: 32
thinking_complete: 2
thinking_start: 2
tool_call_args: 68
tool_call_complete: 2
tool_call_start: 2
tool_execution_complete: 2
tool_execution_progress: 1
tool_execution_start: 1
trace_summary: 1
usage: 2
```

Quality signals:

```text
output_chars: 1342
diff_chars: 296
diff_files_changed: 1
diff_files_changed_raw: 1
generated_dependency_files_ignored: 0
tool_executions: 2
first_write_tool_index: 1
forbidden_tool_uses: none
tool_errors: 0
tool_failures: 0
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 79
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
closeout_tool_records: 2
closeout_tool_evidence: tool evidence: records=2 completed=2 failed=0 denied=0 validation=0 closeout=1 repair=1 changed=1 workflows=code_change commands=none
runtime_diet: prompt=5754 tool_schema=3186 tools=15 workflow=guarded closeout=full validation=passed:1/1 recovered_failed:1
adaptive_triggers: risk_signal_high,required_validation,first_code_change
risk_signal: entry=high runtime=none
risk_signal_reasons: required validation commands present
trace_event_types: workflow.plan,memory.boundary,workflow.fallback,closeout,execution.report,closeout.background,closeout.background,memory.proposal,runtime.diet,completion.contract,memory.boundary,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: seeded_code_change
behavior_assertions: none
behavior_assertion_status: none
output_assertions: none
output_assertion_status: none
output_assertion_missing: none
trajectory_assertions: none
trajectory_assertion_status: none
trajectory_assertion_missing: none
runtime_spine: coverage=6/7, status=failed, missing=event:tool_observation
runtime_profile: minimum_viable_agent
mva_profile_active: true
runtime_spine_detail: context=23 latest=memory_boundary_evaluated decision=16 latest=workflow_plan_progress permission=0 latest=none tool_execution=6 latest=tool_completed state_update=15 latest=workflow_fallback verification=5 latest=acceptance_review_completed closeout=3 latest=assistant_responded risky_tool_runs=1 risky_tool_reviewed=1 risky_tool_missing_action_review=none gate_outcomes=total=3, protective_block=0, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=3 stop_reason=no_issue stop_terminal_status=completed stop_action=continue stop_failure_type=missing rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none route_recovery=events=0, read_search=false, mutation_blocked=false, safety=missing action_scores=2 latest_action_score=20 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=none runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=2 provider_protocol_repairs=0 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 provider_request_started=2 provider_request_completed=2 provider_request_timeout=0 provider_request_retrying=0 provider_request_slow_warning=0 provider_request_cancelled=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=3 context_zone_duplicate_blocks_removed=0 context_zone_provenance_markers=0 agent_loop_steps=4 context_zones=2 completion_contract=completed
runtime_spine_trace_present: true
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: phase:context,phase:tool_execution,phase:closeout,event:action_decision_evaluated,event:action_reviewed,event:tool_observation,event:completion_contract_evaluated
runtime_spine_status: failed
runtime_spine_missing: event:tool_observation
risky_tool_runs: 1
risky_tool_reviewed: 1
risky_tool_missing_action_review: none
gate_outcomes: total=3, protective_block=0, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=3
gate_outcome_records: action_review:allow:harmless_pass,action_review:allow:harmless_pass,closeout:passed:harmless_pass
gate_outcome_total: 3
gate_outcome_protective_blocks: 0
gate_outcome_recoverable_friction: 0
gate_outcome_unrecovered_blocks: 0
gate_outcome_suspected_false_positives: 0
gate_outcome_policy_correct_but_ux_costly: 0
gate_outcome_harmless_passes: 3
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
context_zone_source_messages: 3
context_zone_duplicate_blocks_removed: 0
context_zone_provenance_markers: 0
state_transition_recorded: false
completion_contract_status: completed
completion_contract_proof_status: verified
candidate_score_calibrated: false
candidate_score_disagreement: false
observer_outcome_recorded: true
memory_boundary_recorded: true
verification_proof_status: verified
verification_proof_summary: required validation passed 1/1 commands
verification_proof_kinds: command_passed,required_validation_passed
verification_proof_support_status: verified
verification_proof_support_summary: verified by command_passed,required_validation_passed
verification_proof_supports_verified: true
verification_proof_residual_risk: false
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
warning: runtime_spine_assertions_not_passing
failure_owner: agent_flow
outcome_score: 65
process_score: 85
efficiency_score: 100
agent_score: 78
score_penalties: run_failed,runtime_spine_failed,runtime_spine_not_passing
```

Specialty signals:

```text
memory_active: false
automation_active: true
guided_debugging_active: false
guided_reasoning_active: false
weighted_planning_active: true
closeout_active: true
adaptive_workflow_active: true
active_specialty_signals: 4/7
workflow_contract_activation: entry=active:force repair=not_needed
workflow_contract_events: 1
runtime_spine: coverage=6/7, status=failed, missing=event:tool_observation
runtime_profile: minimum_viable_agent
mva_profile_active: true
runtime_spine_detail: context=23 latest=memory_boundary_evaluated decision=16 latest=workflow_plan_progress permission=0 latest=none tool_execution=6 latest=tool_completed state_update=15 latest=workflow_fallback verification=5 latest=acceptance_review_completed closeout=3 latest=assistant_responded risky_tool_runs=1 risky_tool_reviewed=1 risky_tool_missing_action_review=none gate_outcomes=total=3, protective_block=0, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=3 stop_reason=no_issue stop_terminal_status=completed stop_action=continue stop_failure_type=missing rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none route_recovery=events=0, read_search=false, mutation_blocked=false, safety=missing action_scores=2 latest_action_score=20 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=none runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=2 provider_protocol_repairs=0 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 provider_request_started=2 provider_request_completed=2 provider_request_timeout=0 provider_request_retrying=0 provider_request_slow_warning=0 provider_request_cancelled=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=3 context_zone_duplicate_blocks_removed=0 context_zone_provenance_markers=0 agent_loop_steps=4 context_zones=2 completion_contract=completed
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: phase:context,phase:tool_execution,phase:closeout,event:action_decision_evaluated,event:action_reviewed,event:tool_observation,event:completion_contract_evaluated
runtime_spine_status: failed
runtime_spine_missing: event:tool_observation
risky_tool_runs: 1
risky_tool_reviewed: 1
risky_tool_missing_action_review: none
gate_outcomes: total=3, protective_block=0, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=3
gate_outcome_records: action_review:allow:harmless_pass,action_review:allow:harmless_pass,closeout:passed:harmless_pass
gate_outcome_total: 3
gate_outcome_protective_blocks: 0
gate_outcome_recoverable_friction: 0
gate_outcome_unrecovered_blocks: 0
gate_outcome_suspected_false_positives: 0
gate_outcome_policy_correct_but_ux_costly: 0
gate_outcome_harmless_passes: 3
gate_outcome_failure_owners: none
agent_loop_steps: 4
context_zones_materialized: true
context_zone_task_state_empty: false
context_zone_current_decision_request_empty: false
context_zone_envelope_messages: 1
context_zone_source_messages: 3
context_zone_duplicate_blocks_removed: 0
context_zone_provenance_markers: 0
state_transition_recorded: false
completion_contract_status: completed
completion_contract_proof_status: verified
candidate_score_calibrated: false
candidate_score_disagreement: false
observer_outcome_recorded: true
memory_boundary_recorded: true
verification_proof_status: verified
verification_proof_summary: required validation passed 1/1 commands
verification_proof_kinds: command_passed,required_validation_passed
verification_proof_support_status: verified
verification_proof_support_summary: verified by command_passed,required_validation_passed
verification_proof_supports_verified: true
verification_proof_residual_risk: false
risk_signal: entry=high runtime=none
risk_signal_reasons: required validation commands present
memory_sync_events: 0
memory_tool_calls: 0
retrieval_sources: Project,ProjectMap
memory_candidate_typed: false
memory_candidate_has_evidence: false
memory_proposal_recorded: true
memory_proposal_status: skipped
memory_proposal_candidates: 0
memory_proposal_kinds: none
memory_proposal_evidence_items: 0
memory_proposal_write_policy: generation_disabled
memory_proposal_write_performed: false
memory_record_used: false
memory_use_count_updated: false
memory_failure_lesson_promoted: false
memory_action_weight_changed: false
memory_stale_demoted: false
memory_scope_correct: false
required_commands: 1
agent_required_commands: 1
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
latest_top_priority: P3
latest_top_importance_score: 0.05000000074505806
latest_top_weight_share: 1.0
acceptance_accepted: True
closeout_status: passed
closeout_tool_records: 2
closeout_tool_evidence: tool evidence: records=2 completed=2 failed=0 denied=0 validation=0 closeout=1 repair=1 changed=1 workflows=code_change commands=none
runtime_diet: prompt=5754 tool_schema=3186 tools=15 workflow=guarded
note: guided debugging is expected only after a blocker or failed validation
```

Agent stderr tail:

```text
[2m2026-06-08T08:08:11.183349Z[0m [33m WARN[0m [2mpriority_agent::engine::streaming[0m[2m:[0m session end memory flush join failed: task 41 was cancelled
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

- Bundle: `docs/benchmarks/live-live-eval-20260608-160736/weighting-p0-premature-edit-revise/run-bundle`
- Task: `docs/benchmarks/live-live-eval-20260608-160736/weighting-p0-premature-edit-revise/run-bundle/task.json`
- Steps: `docs/benchmarks/live-live-eval-20260608-160736/weighting-p0-premature-edit-revise/run-bundle/steps.jsonl`
- Events: `docs/benchmarks/live-live-eval-20260608-160736/weighting-p0-premature-edit-revise/run-bundle/events.jsonl`
- Final report: `docs/benchmarks/live-live-eval-20260608-160736/weighting-p0-premature-edit-revise/run-bundle/final_report.md`
