# Live Eval Report: routing-topic-switch-readonly

- Run id: `live-eval-20260608-184336`
- Sample: `evalsets/live_tasks/routing-topic-switch-readonly.yaml`
- Worktree: `target/live-evals/live-eval-20260608-184336/routing-topic-switch-readonly/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/live-eval-20260608-184336/routing-topic-switch-readonly/env`
- Test status: `ok`
- Generated: `2026-06-08 18:47:17 +0800`

## Git Status

```text
```

## Diff Stat

```text
```

## Required Commands

```text
$ rg -F 'ready' fixtures/routing_switch/status.txt
data: ready
[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-live-eval-20260608-184336/routing-topic-switch-readonly/agent-output.md`
- Events: `docs/benchmarks/live-live-eval-20260608-184336/routing-topic-switch-readonly/agent-events.jsonl`
- Metrics: `docs/benchmarks/live-live-eval-20260608-184336/routing-topic-switch-readonly/agent-run-metrics.json`

Event counts:

```text
closeout: 1
eval_started: 1
runtime_diagnostic: 10
start: 1
text_chunk: 183
thinking_complete: 3
thinking_start: 3
tool_call_args: 185
tool_call_complete: 3
tool_call_start: 3
tool_execution_complete: 3
usage: 3
```

Quality signals:

```text
output_chars: 833
diff_chars: 0
diff_files_changed: 0
diff_files_changed_raw: 0
generated_dependency_files_ignored: 0
tool_executions: 3
first_write_tool_index: none
forbidden_tool_uses: none
tool_errors: 0
tool_failures: 0
has_closeout: true
has_validation_claim: true
trace_status: missing
trace_events: 0
test_status: ok
verification_passed: false
stage_validation_passed: false
acceptance_accepted: None
closeout_status: missing
closeout_tool_records: 0
closeout_tool_evidence: missing
runtime_diet: missing
adaptive_triggers: none
risk_signal: entry=missing runtime=none
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
runtime_spine: coverage=0/7, status=missing, missing=phase:context,phase:tool_execution,phase:closeout,event:action_decision_evaluated,event:stop_check_evaluated
runtime_profile: minimum_viable_agent
mva_profile_active: true
runtime_spine_detail: context=0 latest=none decision=0 latest=none permission=0 latest=none tool_execution=0 latest=none state_update=0 latest=none verification=0 latest=none closeout=0 latest=none risky_tool_runs=0 risky_tool_reviewed=0 risky_tool_missing_action_review=none gate_outcomes=total=0, protective_block=0, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=0 stop_reason=missing stop_terminal_status=missing stop_action=missing stop_failure_type=missing rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none route_recovery=events=0, read_search=false, mutation_blocked=false, safety=missing action_scores=0 latest_action_score=none low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=false memory_modifier_applied=false observer_outcome_recorded=false observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=none runtime_diet_warnings=none provider_protocol_events=0 provider_protocol_repairs=0 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 provider_request_started=0 provider_request_completed=0 provider_request_timeout=0 provider_request_retrying=0 provider_request_slow_warning=0 provider_request_cancelled=0 memory_boundary_recorded=false task_contract_recorded=false context_pack_recorded=false execution_report_recorded=false memory_proposal_recorded=false context_zone_envelope_messages=0 context_zone_source_messages=0 context_zone_duplicate_blocks_removed=0 context_zone_provenance_markers=0 agent_loop_steps=0 context_zones=0 completion_contract=missing
runtime_spine_trace_present: false
runtime_spine_phase_coverage: 0/7
runtime_spine_observed_phases: none
runtime_spine_assertions: phase:context,phase:tool_execution,phase:closeout,event:action_decision_evaluated,event:stop_check_evaluated
runtime_spine_status: missing
runtime_spine_missing: phase:context,phase:tool_execution,phase:closeout,event:action_decision_evaluated,event:stop_check_evaluated
risky_tool_runs: 0
risky_tool_reviewed: 0
risky_tool_missing_action_review: none
gate_outcomes: total=0, protective_block=0, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=0
gate_outcome_records: none
gate_outcome_total: 0
gate_outcome_protective_blocks: 0
gate_outcome_recoverable_friction: 0
gate_outcome_unrecovered_blocks: 0
gate_outcome_suspected_false_positives: 0
gate_outcome_policy_correct_but_ux_costly: 0
gate_outcome_harmless_passes: 0
gate_outcome_failure_owners: none
route_recovery: events=0, read_search=false, mutation_blocked=false, safety=missing
route_recovery_events: 0
route_recovery_failure_types: none
route_recovery_kinds: none
route_recovery_read_search_expanded: false
route_recovery_mutation_blocked: false
route_recovery_safety_monotonic: missing
route_recovery_unsafe_mutation_expansion: false
agent_loop_steps: 0
context_zones_materialized: false
context_zone_task_state_empty: false
context_zone_current_decision_request_empty: false
context_zone_envelope_messages: 0
context_zone_source_messages: 0
context_zone_duplicate_blocks_removed: 0
context_zone_provenance_markers: 0
state_transition_recorded: false
completion_contract_status: missing
completion_contract_proof_status: missing
candidate_score_calibrated: false
candidate_score_disagreement: false
observer_outcome_recorded: false
memory_boundary_recorded: false
verification_proof_status: missing
verification_proof_summary: missing
verification_proof_kinds: none
verification_proof_support_status: missing
verification_proof_support_summary: missing
verification_proof_supports_verified: false
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
tool_call_count: 0
llm_call_count: 3
warning: no_code_diff
warning: missing_trace_summary
warning: runtime_spine_assertions_not_passing
warning: closeout_not_successful
failure_owner: agent_flow
outcome_score: 30
process_score: 80
efficiency_score: 100
agent_score: 59
score_penalties: run_failed,verification_failed,closeout_not_successful,runtime_spine_failed,runtime_spine_not_passing,stop_check_missing
```

Specialty signals:

```text
memory_active: false
automation_active: true
guided_debugging_active: false
guided_reasoning_active: false
weighted_planning_active: false
closeout_active: false
adaptive_workflow_active: false
active_specialty_signals: 1/7
workflow_contract_activation: entry=missing repair=none
workflow_contract_events: 0
runtime_spine: coverage=0/7, status=missing, missing=phase:context,phase:tool_execution,phase:closeout,event:action_decision_evaluated,event:stop_check_evaluated
runtime_profile: minimum_viable_agent
mva_profile_active: true
runtime_spine_detail: context=0 latest=none decision=0 latest=none permission=0 latest=none tool_execution=0 latest=none state_update=0 latest=none verification=0 latest=none closeout=0 latest=none risky_tool_runs=0 risky_tool_reviewed=0 risky_tool_missing_action_review=none gate_outcomes=total=0, protective_block=0, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=0 stop_reason=missing stop_terminal_status=missing stop_action=missing stop_failure_type=missing rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none route_recovery=events=0, read_search=false, mutation_blocked=false, safety=missing action_scores=0 latest_action_score=none low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=false memory_modifier_applied=false observer_outcome_recorded=false observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=none runtime_diet_warnings=none provider_protocol_events=0 provider_protocol_repairs=0 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 provider_request_started=0 provider_request_completed=0 provider_request_timeout=0 provider_request_retrying=0 provider_request_slow_warning=0 provider_request_cancelled=0 memory_boundary_recorded=false task_contract_recorded=false context_pack_recorded=false execution_report_recorded=false memory_proposal_recorded=false context_zone_envelope_messages=0 context_zone_source_messages=0 context_zone_duplicate_blocks_removed=0 context_zone_provenance_markers=0 agent_loop_steps=0 context_zones=0 completion_contract=missing
runtime_spine_phase_coverage: 0/7
runtime_spine_observed_phases: none
runtime_spine_assertions: phase:context,phase:tool_execution,phase:closeout,event:action_decision_evaluated,event:stop_check_evaluated
runtime_spine_status: missing
runtime_spine_missing: phase:context,phase:tool_execution,phase:closeout,event:action_decision_evaluated,event:stop_check_evaluated
risky_tool_runs: 0
risky_tool_reviewed: 0
risky_tool_missing_action_review: none
gate_outcomes: total=0, protective_block=0, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=0
gate_outcome_records: none
gate_outcome_total: 0
gate_outcome_protective_blocks: 0
gate_outcome_recoverable_friction: 0
gate_outcome_unrecovered_blocks: 0
gate_outcome_suspected_false_positives: 0
gate_outcome_policy_correct_but_ux_costly: 0
gate_outcome_harmless_passes: 0
gate_outcome_failure_owners: none
agent_loop_steps: 0
context_zones_materialized: false
context_zone_task_state_empty: false
context_zone_current_decision_request_empty: false
context_zone_envelope_messages: 0
context_zone_source_messages: 0
context_zone_duplicate_blocks_removed: 0
context_zone_provenance_markers: 0
state_transition_recorded: false
completion_contract_status: missing
completion_contract_proof_status: missing
candidate_score_calibrated: false
candidate_score_disagreement: false
observer_outcome_recorded: false
memory_boundary_recorded: false
verification_proof_status: missing
verification_proof_summary: missing
verification_proof_kinds: none
verification_proof_support_status: missing
verification_proof_support_summary: missing
verification_proof_supports_verified: false
verification_proof_residual_risk: false
risk_signal: entry=missing runtime=none
memory_sync_events: 0
memory_tool_calls: 0
retrieval_sources: none
memory_candidate_typed: false
memory_candidate_has_evidence: false
memory_proposal_recorded: false
memory_proposal_status: missing
memory_proposal_candidates: 0
memory_proposal_kinds: none
memory_proposal_evidence_items: 0
memory_proposal_write_policy: missing
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
runtime_diet: missing
note: guided debugging is expected only after a blocker or failed validation
```

Agent stderr tail:

```text

thread 'tokio-rt-worker' (4932782) panicked at src/engine/trace/event_summary_workflow.rs:384:14:
internal error: entered unreachable code: workflow trace summary called for non-workflow event
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
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

- Bundle: `docs/benchmarks/live-live-eval-20260608-184336/routing-topic-switch-readonly/run-bundle`
- Task: `docs/benchmarks/live-live-eval-20260608-184336/routing-topic-switch-readonly/run-bundle/task.json`
- Steps: `docs/benchmarks/live-live-eval-20260608-184336/routing-topic-switch-readonly/run-bundle/steps.jsonl`
- Events: `docs/benchmarks/live-live-eval-20260608-184336/routing-topic-switch-readonly/run-bundle/events.jsonl`
- Final report: `docs/benchmarks/live-live-eval-20260608-184336/routing-topic-switch-readonly/run-bundle/final_report.md`
