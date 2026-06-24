# Live Eval Report: core-long-output-artifact

- Run id: `deepseek-v4-flash-coverage-20260624-114658`
- Sample: `evalsets/live_tasks/core-long-output-artifact.yaml`
- Worktree: `target/live-evals/deepseek-v4-flash-coverage-20260624-114658/core-long-output-artifact/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/deepseek-v4-flash-coverage-20260624-114658/core-long-output-artifact/env`
- Test status: `ok`
- Generated: `2026-06-24 11:47:46 +0800`

## Git Status

```text
?? fixtures/core_quality/long_output/output.log
```

## Diff Stat

```text
 .../core_quality/long_output/output.log            | 800 +++++++++++++++++++++
 1 file changed, 800 insertions(+)
```

## Required Commands

```text
$ test -s fixtures/core_quality/long_output/output.log
[exit status: 0]

$ rg 'line 0537 ERROR_ANCHOR payment retry budget exceeded' fixtures/core_quality/long_output/output.log
line 0537 ERROR_ANCHOR payment retry budget exceeded
[exit status: 0]

$ python3 -c 'from pathlib import Path; assert len(Path("fixtures/core_quality/long_output/output.log").read_text().splitlines()) == 800'
[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-deepseek-v4-flash-coverage-20260624-114658/core-long-output-artifact/agent-output.md`
- Events: `docs/benchmarks/live-deepseek-v4-flash-coverage-20260624-114658/core-long-output-artifact/agent-events.jsonl`
- Metrics: `docs/benchmarks/live-deepseek-v4-flash-coverage-20260624-114658/core-long-output-artifact/agent-run-metrics.json`

Event counts:

```text
closeout: 1
complete: 1
eval_started: 1
permission_request: 1
runtime_diagnostic: 11
start: 1
text_chunk: 5
thinking_complete: 6
thinking_start: 3
tool_call_args: 3
tool_call_complete: 3
tool_call_start: 3
tool_execution_complete: 3
tool_execution_progress: 1
tool_execution_start: 3
tool_results_ready_for_model: 3
trace_summary: 1
```

Quality signals:

```text
output_chars: 1302
diff_chars: 11474
diff_files_changed: 1
diff_files_changed_raw: 1
generated_dependency_files_ignored: 0
tool_executions: 3
first_write_tool_index: none
forbidden_tool_uses: none
tool_errors: 0
tool_failures: 0
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 103
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
closeout_tool_records: 3
closeout_tool_evidence: tool evidence: records=3 completed=3 failed=0 denied=0 validation=0 closeout=1 repair=1 changed=1 workflows=code_change commands=python3 fixtures/core_quality/long_output/generate_log.py > fixtures/core_quality/long_output/output.log
runtime_diet: prompt=7392 tool_schema=793 tools=2 workflow=guarded closeout=full validation=passed:3/3 recovered_failed:3
adaptive_triggers: risk_signal_high,required_validation,first_code_change
risk_signal: entry=high runtime=none
risk_signal_reasons: required validation commands present
trace_event_types: acceptance.review,workflow.plan,memory.boundary,workflow.fallback,closeout,execution.report,closeout.background,closeout.background,memory.proposal,runtime.diet,completion.contract,assistant
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
runtime_spine: coverage=7/7, status=none, missing=none
runtime_profile: none
mva_profile_active: false
runtime_spine_detail: context=27 latest=runtime_diet_report decision=15 latest=workflow_plan_progress permission=2 latest=permission_resolved tool_execution=9 latest=tool_completed state_update=26 latest=workflow_fallback verification=5 latest=acceptance_review_completed closeout=3 latest=assistant_responded risky_tool_runs=1 risky_tool_reviewed=1 risky_tool_missing_action_review=none gate_outcomes=total=5, protective_block=0, recoverable_friction=1, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=4 stop_reason=no_issue stop_terminal_status=completed stop_action=continue stop_failure_type=missing rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none route_recovery=events=0, read_search=false, mutation_blocked=false, safety=missing action_scores=2 latest_action_score=16 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=user_session_allow runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=3 provider_protocol_repairs=0 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 provider_request_started=3 provider_request_completed=3 provider_request_timeout=0 provider_request_retrying=0 provider_request_slow_warning=0 provider_request_cancelled=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=9 context_zone_duplicate_blocks_removed=3 context_zone_provenance_markers=0 agent_loop_steps=6 context_zones=3 completion_contract=completed
runtime_spine_trace_present: true
runtime_spine_phase_coverage: 7/7
runtime_spine_observed_phases: context,decision,permission,tool_execution,state_update,verification,closeout
runtime_spine_assertions: none
runtime_spine_status: none
runtime_spine_missing: none
risky_tool_runs: 1
risky_tool_reviewed: 1
risky_tool_missing_action_review: none
gate_outcomes: total=5, protective_block=0, recoverable_friction=1, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=4
gate_outcome_records: action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:ask_user:recoverable_friction,permission:approve_session:harmless_pass,closeout:passed:harmless_pass
gate_outcome_total: 5
gate_outcome_protective_blocks: 0
gate_outcome_recoverable_friction: 1
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
agent_loop_steps: 6
context_zones_materialized: true
context_zone_task_state_empty: false
context_zone_current_decision_request_empty: false
context_zone_envelope_messages: 1
context_zone_source_messages: 9
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
verification_proof_summary: required validation passed 3/3 commands
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
tool_call_count: 3
llm_call_count: 3
failure_owner: none
outcome_score: 100
process_score: 100
efficiency_score: 100
agent_score: 100
score_penalties: none
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
runtime_spine: coverage=7/7, status=none, missing=none
runtime_profile: none
mva_profile_active: false
runtime_spine_detail: context=27 latest=runtime_diet_report decision=15 latest=workflow_plan_progress permission=2 latest=permission_resolved tool_execution=9 latest=tool_completed state_update=26 latest=workflow_fallback verification=5 latest=acceptance_review_completed closeout=3 latest=assistant_responded risky_tool_runs=1 risky_tool_reviewed=1 risky_tool_missing_action_review=none gate_outcomes=total=5, protective_block=0, recoverable_friction=1, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=4 stop_reason=no_issue stop_terminal_status=completed stop_action=continue stop_failure_type=missing rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none route_recovery=events=0, read_search=false, mutation_blocked=false, safety=missing action_scores=2 latest_action_score=16 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=user_session_allow runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=3 provider_protocol_repairs=0 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 provider_request_started=3 provider_request_completed=3 provider_request_timeout=0 provider_request_retrying=0 provider_request_slow_warning=0 provider_request_cancelled=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=9 context_zone_duplicate_blocks_removed=3 context_zone_provenance_markers=0 agent_loop_steps=6 context_zones=3 completion_contract=completed
runtime_spine_phase_coverage: 7/7
runtime_spine_observed_phases: context,decision,permission,tool_execution,state_update,verification,closeout
runtime_spine_assertions: none
runtime_spine_status: none
runtime_spine_missing: none
risky_tool_runs: 1
risky_tool_reviewed: 1
risky_tool_missing_action_review: none
gate_outcomes: total=5, protective_block=0, recoverable_friction=1, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=4
gate_outcome_records: action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:ask_user:recoverable_friction,permission:approve_session:harmless_pass,closeout:passed:harmless_pass
gate_outcome_total: 5
gate_outcome_protective_blocks: 0
gate_outcome_recoverable_friction: 1
gate_outcome_unrecovered_blocks: 0
gate_outcome_suspected_false_positives: 0
gate_outcome_policy_correct_but_ux_costly: 0
gate_outcome_harmless_passes: 4
gate_outcome_failure_owners: none
agent_loop_steps: 6
context_zones_materialized: true
context_zone_task_state_empty: false
context_zone_current_decision_request_empty: false
context_zone_envelope_messages: 1
context_zone_source_messages: 9
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
verification_proof_summary: required validation passed 3/3 commands
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
latest_top_priority: P3
latest_top_importance_score: 0.05000000074505806
latest_top_weight_share: 1.0
acceptance_accepted: True
closeout_status: passed
closeout_tool_records: 3
closeout_tool_evidence: tool evidence: records=3 completed=3 failed=0 denied=0 validation=0 closeout=1 repair=1 changed=1 workflows=code_change commands=python3 fixtures/core_quality/long_output/generate_log.py > fixtures/core_quality/long_output/output.log
runtime_diet: prompt=7392 tool_schema=793 tools=2 workflow=guarded
note: guided debugging is expected only after a blocker or failed validation
```

Agent stderr tail:

```text
2026-06-24T03:47:16.034044Z  WARN priority_agent::services::config::runtime: Failed to load AppConfig, using defaults: missing field `temperature`
2026-06-24T03:47:38.969580Z  WARN priority_agent::engine::streaming::text_progress: session end memory flush join failed: task 49 was cancelled
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

- Bundle: `docs/benchmarks/live-deepseek-v4-flash-coverage-20260624-114658/core-long-output-artifact/run-bundle`
- Task: `docs/benchmarks/live-deepseek-v4-flash-coverage-20260624-114658/core-long-output-artifact/run-bundle/task.json`
- Steps: `docs/benchmarks/live-deepseek-v4-flash-coverage-20260624-114658/core-long-output-artifact/run-bundle/steps.jsonl`
- Events: `docs/benchmarks/live-deepseek-v4-flash-coverage-20260624-114658/core-long-output-artifact/run-bundle/events.jsonl`
- Final report: `docs/benchmarks/live-deepseek-v4-flash-coverage-20260624-114658/core-long-output-artifact/run-bundle/final_report.md`
