# Live Eval Report: core-simple-stale-edit

- Run id: `product-daily-20260602-230826`
- Sample: `evalsets/live_tasks/core-simple-stale-edit.yaml`
- Worktree: `target/live-evals/product-daily-20260602-230826/core-simple-stale-edit/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/product-daily-20260602-230826/core-simple-stale-edit/env`
- Test status: `ok`
- Generated: `2026-06-02 23:11:14 +0800`

## Git Status

```text
 M fixtures/core_quality/simple_edit/settings.py
```

## Diff Stat

```text
 fixtures/core_quality/simple_edit/settings.py | 2 +-
 1 file changed, 1 insertion(+), 1 deletion(-)
```

## Required Commands

```text
$ python3 fixtures/core_quality/simple_edit/test_settings.py
.
----------------------------------------------------------------------
Ran 1 test in 0.000s

OK
[exit status: 0]

$ rg 'DEFAULT_TIMEOUT = 10' fixtures/core_quality/simple_edit/settings.py
DEFAULT_TIMEOUT = 10
[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-product-daily-20260602-230826/core-simple-stale-edit/agent-output.md`
- Events: `docs/benchmarks/live-product-daily-20260602-230826/core-simple-stale-edit/agent-events.jsonl`
- Monitor: `docs/benchmarks/live-product-daily-20260602-230826/core-simple-stale-edit/agent-monitor.log`

Event counts:

```text
complete: 1
eval_started: 1
runtime_diagnostic: 9
start: 1
text_chunk: 1
tool_execution_complete: 3
tool_execution_progress: 1
tool_execution_start: 3
trace_summary: 1
```

Quality signals:

```text
output_chars: 1843
diff_chars: 330
diff_files_changed: 1
diff_files_changed_raw: 1
generated_dependency_files_ignored: 0
tool_executions: 3
first_write_tool_index: 3
forbidden_tool_uses: file_patch
tool_errors: 0
tool_failures: 2
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 124
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
closeout_tool_records: 5
closeout_tool_evidence: tool evidence: records=5 completed=3 failed=2 denied=0 validation=0 closeout=1 repair=3 changed=1 workflows=code_change commands=cd /Users/georgexu/Desktop/rust-agent/target/live-evals/product-daily-20260602-230826/core-simple-stale-edit/wo...
runtime_diet: prompt=10917 tool_schema=4272 tools=19 workflow=guarded closeout=full validation=passed:2/2 recovered_failed:2
adaptive_triggers: risk_signal_high,required_validation,first_code_change
risk_signal: entry=high runtime=high
risk_signal_reasons: required validation commands present
trace_event_types: stage.validation,acceptance.review,workflow.plan,memory.boundary,workflow.fallback,closeout,execution.report,memory.proposal,memory.proposal,runtime.diet,completion.contract,assistant
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
runtime_spine: coverage=6/7, status=passed, missing=none
runtime_profile: none
mva_profile_active: false
runtime_spine_detail: context=25 latest=runtime_diet_report decision=23 latest=workflow_plan_progress permission=0 latest=none tool_execution=14 latest=tool_completed state_update=35 latest=workflow_fallback verification=6 latest=acceptance_review_completed closeout=3 latest=assistant_responded risky_tool_runs=3 risky_tool_reviewed=3 risky_tool_missing_action_review=none gate_outcomes=total=6, protective_block=0, recoverable_friction=2, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=4 stop_reason=no_issue stop_terminal_status=completed stop_action=continue stop_failure_type=edit rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none route_recovery=events=0, read_search=false, mutation_blocked=false, safety=missing action_scores=3 latest_action_score=21 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=none runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=4 provider_protocol_repairs=0 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=13 context_zone_duplicate_blocks_removed=3 context_zone_provenance_markers=0 agent_loop_steps=8 context_zones=4 completion_contract=completed
runtime_spine_trace_present: true
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: phase:context,phase:decision,phase:tool_execution,phase:state_update,phase:closeout,event:action_decision_evaluated,event:stop_check_evaluated,special:verification_proof
runtime_spine_status: passed
runtime_spine_missing: none
risky_tool_runs: 3
risky_tool_reviewed: 3
risky_tool_missing_action_review: none
gate_outcomes: total=6, protective_block=0, recoverable_friction=2, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=4
gate_outcome_records: action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:revise:recoverable_friction,action_review:revise:recoverable_friction,action_review:allow:harmless_pass,closeout:passed:harmless_pass
gate_outcome_total: 6
gate_outcome_protective_blocks: 0
gate_outcome_recoverable_friction: 2
gate_outcome_unrecovered_blocks: 0
gate_outcome_suspected_false_positives: 0
gate_outcome_policy_correct_but_ux_costly: 0
gate_outcome_harmless_passes: 4
gate_outcome_failure_owners: none
route_recovery: events=0, read_search=false, mutation_blocked=false, safety=missing
route_recovery_events: 0
route_recovery_failure_types: none
route_recovery_kinds: none
route_recovery_read_search_expanded: false
route_recovery_mutation_blocked: false
route_recovery_safety_monotonic: missing
route_recovery_unsafe_mutation_expansion: false
agent_loop_steps: 8
context_zones_materialized: true
context_zone_task_state_empty: false
context_zone_current_decision_request_empty: false
context_zone_envelope_messages: 1
context_zone_source_messages: 13
context_zone_duplicate_blocks_removed: 3
context_zone_provenance_markers: 0
state_transition_recorded: false
completion_contract_status: completed
completion_contract_proof_status: verified
candidate_score_calibrated: false
candidate_score_disagreement: false
observer_outcome_recorded: true
memory_boundary_recorded: true
verification_proof_status: verified
verification_proof_summary: required validation passed 2/2 commands
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
failed_action_count: 2
user_question_count: 0
unnecessary_question_count: 0
verification_attempted: true
verification_passed: true
tool_call_count: 3
llm_call_count: 4
warning: forbidden_tool_used
failure_owner: mixed
outcome_score: 45
process_score: 100
efficiency_score: 84
agent_score: 69
score_penalties: run_failed,forbidden_tool_used,failed_actions
```

Specialty signals:

```text
memory_active: false
automation_active: true
guided_debugging_active: true
guided_reasoning_active: false
weighted_planning_active: true
closeout_active: true
adaptive_workflow_active: true
active_specialty_signals: 5/7
workflow_contract_activation: entry=active:force repair=active_after_failure
workflow_contract_events: 1
runtime_spine: coverage=6/7, status=passed, missing=none
runtime_profile: none
mva_profile_active: false
runtime_spine_detail: context=25 latest=runtime_diet_report decision=23 latest=workflow_plan_progress permission=0 latest=none tool_execution=14 latest=tool_completed state_update=35 latest=workflow_fallback verification=6 latest=acceptance_review_completed closeout=3 latest=assistant_responded risky_tool_runs=3 risky_tool_reviewed=3 risky_tool_missing_action_review=none gate_outcomes=total=6, protective_block=0, recoverable_friction=2, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=4 stop_reason=no_issue stop_terminal_status=completed stop_action=continue stop_failure_type=edit rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none route_recovery=events=0, read_search=false, mutation_blocked=false, safety=missing action_scores=3 latest_action_score=21 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=none runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=4 provider_protocol_repairs=0 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=13 context_zone_duplicate_blocks_removed=3 context_zone_provenance_markers=0 agent_loop_steps=8 context_zones=4 completion_contract=completed
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: phase:context,phase:decision,phase:tool_execution,phase:state_update,phase:closeout,event:action_decision_evaluated,event:stop_check_evaluated,special:verification_proof
runtime_spine_status: passed
runtime_spine_missing: none
risky_tool_runs: 3
risky_tool_reviewed: 3
risky_tool_missing_action_review: none
gate_outcomes: total=6, protective_block=0, recoverable_friction=2, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=4
gate_outcome_records: action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:revise:recoverable_friction,action_review:revise:recoverable_friction,action_review:allow:harmless_pass,closeout:passed:harmless_pass
gate_outcome_total: 6
gate_outcome_protective_blocks: 0
gate_outcome_recoverable_friction: 2
gate_outcome_unrecovered_blocks: 0
gate_outcome_suspected_false_positives: 0
gate_outcome_policy_correct_but_ux_costly: 0
gate_outcome_harmless_passes: 4
gate_outcome_failure_owners: none
agent_loop_steps: 8
context_zones_materialized: true
context_zone_task_state_empty: false
context_zone_current_decision_request_empty: false
context_zone_envelope_messages: 1
context_zone_source_messages: 13
context_zone_duplicate_blocks_removed: 3
context_zone_provenance_markers: 0
state_transition_recorded: false
completion_contract_status: completed
completion_contract_proof_status: verified
candidate_score_calibrated: false
candidate_score_disagreement: false
observer_outcome_recorded: true
memory_boundary_recorded: true
verification_proof_status: verified
verification_proof_summary: required validation passed 2/2 commands
verification_proof_kinds: command_passed,required_validation_passed
verification_proof_support_status: verified
verification_proof_support_summary: verified by command_passed,required_validation_passed
verification_proof_supports_verified: true
verification_proof_residual_risk: false
risk_signal: entry=high runtime=high
risk_signal_reasons: required validation commands present
memory_sync_events: 0
memory_tool_calls: 0
retrieval_sources: Project,ProjectMap
memory_candidate_typed: true
memory_candidate_has_evidence: true
memory_proposal_recorded: true
memory_proposal_status: proposed
memory_proposal_candidates: 1
memory_proposal_kinds: validation_baseline
memory_proposal_evidence_items: 10
memory_proposal_write_policy: review_required
memory_proposal_write_performed: false
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
tool_progress_events: 1
guided_debugging_events: 1
guided_reasoning_events: 0
workflow_plan_events: 2
weighted_plan_events: 1
reweighted_plan_events: 1
adaptive_trigger_events: 3
adaptive_triggers: risk_signal_high,required_validation,first_code_change
latest_top_priority: P0
latest_top_importance_score: 0.9149999618530273
latest_top_weight_share: 0.22761192917823792
acceptance_accepted: True
closeout_status: passed
closeout_tool_records: 5
closeout_tool_evidence: tool evidence: records=5 completed=3 failed=2 denied=0 validation=0 closeout=1 repair=3 changed=1 workflows=code_change commands=cd /Users/georgexu/Desktop/rust-agent/target/live-evals/product-daily-20260602-230826/core-simple-stale-edit/wo...
runtime_diet: prompt=10917 tool_schema=4272 tools=19 workflow=guarded
```

Agent monitor tail:

```text
[2026-06-02T23:10:28+0800] agent-run still running elapsed=30s idle_for=0s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=1024
[2026-06-02T23:10:58+0800] agent-run still running elapsed=60s idle_for=15s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=9058
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

- Bundle: `docs/benchmarks/live-product-daily-20260602-230826/core-simple-stale-edit/run-bundle`
- Task: `docs/benchmarks/live-product-daily-20260602-230826/core-simple-stale-edit/run-bundle/task.json`
- Steps: `docs/benchmarks/live-product-daily-20260602-230826/core-simple-stale-edit/run-bundle/steps.jsonl`
- Events: `docs/benchmarks/live-product-daily-20260602-230826/core-simple-stale-edit/run-bundle/events.jsonl`
- Final report: `docs/benchmarks/live-product-daily-20260602-230826/core-simple-stale-edit/run-bundle/final_report.md`
