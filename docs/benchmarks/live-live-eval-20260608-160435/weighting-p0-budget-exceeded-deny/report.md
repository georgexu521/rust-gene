# Live Eval Report: weighting-p0-budget-exceeded-deny

- Run id: `live-eval-20260608-160435`
- Sample: `evalsets/live_tasks/weighting-p0-budget-exceeded-deny.yaml`
- Worktree: `target/live-evals/live-eval-20260608-160435/weighting-p0-budget-exceeded-deny/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/live-eval-20260608-160435/weighting-p0-budget-exceeded-deny/env`
- Test status: `ok`
- Generated: `2026-06-08 16:05:39 +0800`

## Git Status

```text
```

## Diff Stat

```text
```

## Required Commands

```text
$ test -f fixtures/weighting_p0_budget/status_1.txt
[exit status: 0]

$ for f in fixtures/weighting_p0_budget/status_*.txt; do test -f "$f"; done
[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-live-eval-20260608-160435/weighting-p0-budget-exceeded-deny/agent-output.md`
- Events: `docs/benchmarks/live-live-eval-20260608-160435/weighting-p0-budget-exceeded-deny/agent-events.jsonl`
- Monitor: `docs/benchmarks/live-live-eval-20260608-160435/weighting-p0-budget-exceeded-deny/agent-monitor.log`
- Metrics: `docs/benchmarks/live-live-eval-20260608-160435/weighting-p0-budget-exceeded-deny/agent-run-metrics.json`

Event counts:

```text
closeout: 1
complete: 1
eval_started: 1
runtime_diagnostic: 23
start: 1
text_chunk: 556
thinking_complete: 9
thinking_start: 9
tool_call_args: 1008
tool_call_complete: 12
tool_call_start: 12
tool_execution_complete: 12
tool_execution_progress: 6
tool_execution_start: 10
trace_summary: 1
usage: 9
```

Quality signals:

```text
output_chars: 3752
diff_chars: 0
diff_files_changed: 0
diff_files_changed_raw: 0
generated_dependency_files_ignored: 0
tool_executions: 12
first_write_tool_index: none
forbidden_tool_uses: none
tool_errors: 0
tool_failures: 0
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 224
test_status: ok
verification_passed: false
stage_validation_passed: false
acceptance_accepted: None
closeout_status: not_verified
closeout_tool_records: 5
closeout_tool_evidence: tool evidence: records=5 completed=5 failed=0 denied=0 validation=0 closeout=0 repair=0 changed=0 workflows=code_change commands=none
runtime_diet: prompt=8263 tool_schema=3186 tools=15 workflow=minimal closeout=full validation=not_run
adaptive_triggers: none
risk_signal: entry=ordinary runtime=none
risk_signal_reasons: ordinary change surface
trace_event_types: provider.request.done,cache.usage,api.done,closeout,execution.report,closeout.background,closeout.background,memory.proposal,runtime.diet,completion.contract,memory.boundary,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: audit_or_regression_check
behavior_assertions: budget_gate_denies_after_limit,no_file_mutation
behavior_assertion_status: passed
output_assertions: none
output_assertion_status: none
output_assertion_missing: none
trajectory_assertions: none
trajectory_assertion_status: none
trajectory_assertion_missing: none
runtime_spine: coverage=6/7, status=passed, missing=none
runtime_profile: minimum_viable_agent
mva_profile_active: true
runtime_spine_detail: context=78 latest=memory_boundary_evaluated decision=38 latest=action_reviewed permission=0 latest=none tool_execution=33 latest=api_request_completed state_update=46 latest=agent_loop_step_evaluated verification=1 latest=reflection_pass_completed closeout=3 latest=assistant_responded risky_tool_runs=6 risky_tool_reviewed=6 risky_tool_missing_action_review=none gate_outcomes=total=13, protective_block=0, recoverable_friction=0, unrecovered_block=1, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=12 stop_reason=no_issue stop_terminal_status=partial stop_action=continue stop_failure_type=missing rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none route_recovery=events=0, read_search=false, mutation_blocked=false, safety=missing action_scores=12 latest_action_score=1 low_action_score_count=6 phase_misaligned_actions=7 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=6 observer_quality_warning_labels=missing_permission_source permission_sources=none runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=9 provider_protocol_repairs=0 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 provider_request_started=9 provider_request_completed=9 provider_request_timeout=0 provider_request_retrying=0 provider_request_slow_warning=0 provider_request_cancelled=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=2 context_zone_duplicate_blocks_removed=0 context_zone_provenance_markers=0 agent_loop_steps=16 context_zones=9 completion_contract=partial
runtime_spine_trace_present: true
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: event:action_decision_evaluated,event:action_reviewed,event:stop_check_evaluated,event:completion_contract_evaluated
runtime_spine_status: passed
runtime_spine_missing: none
risky_tool_runs: 6
risky_tool_reviewed: 6
risky_tool_missing_action_review: none
gate_outcomes: total=13, protective_block=0, recoverable_friction=0, unrecovered_block=1, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=12
gate_outcome_records: action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,+1
gate_outcome_total: 13
gate_outcome_protective_blocks: 0
gate_outcome_recoverable_friction: 0
gate_outcome_unrecovered_blocks: 1
gate_outcome_suspected_false_positives: 0
gate_outcome_policy_correct_but_ux_costly: 0
gate_outcome_harmless_passes: 12
gate_outcome_failure_owners: action_review
route_recovery: events=0, read_search=false, mutation_blocked=false, safety=missing
route_recovery_events: 0
route_recovery_failure_types: none
route_recovery_kinds: none
route_recovery_read_search_expanded: false
route_recovery_mutation_blocked: false
route_recovery_safety_monotonic: missing
route_recovery_unsafe_mutation_expansion: false
agent_loop_steps: 16
context_zones_materialized: true
context_zone_task_state_empty: false
context_zone_current_decision_request_empty: false
context_zone_envelope_messages: 1
context_zone_source_messages: 2
context_zone_duplicate_blocks_removed: 0
context_zone_provenance_markers: 0
state_transition_recorded: false
completion_contract_status: partial
completion_contract_proof_status: not_run
candidate_score_calibrated: false
candidate_score_disagreement: true
observer_outcome_recorded: true
memory_boundary_recorded: true
verification_proof_status: not_run
verification_proof_summary: validation required but no evidence was recorded
verification_proof_kinds: none
verification_proof_support_status: not_run
verification_proof_support_summary: verification proof status not_run blocks verified closeout before proof-kind policy
verification_proof_supports_verified: false
verification_proof_residual_risk: true
premature_edit_count: 0
evidence_before_first_edit: true
scope_drift_count: 7
invalid_action_count: 15
repeated_action_count: 1
failed_action_count: 0
user_question_count: 0
unnecessary_question_count: 0
verification_attempted: true
verification_passed: true
tool_call_count: 10
llm_call_count: 9
warning: no_code_diff
warning: closeout_not_successful
failure_owner: agent_flow
outcome_score: 40
process_score: 42
efficiency_score: 83
agent_score: 49
score_penalties: run_failed,verification_failed,closeout_not_successful,scope_drift,repeated_action,invalid_action,repeated_actions,llm_call_budget_pressure
```

Specialty signals:

```text
memory_active: false
automation_active: true
guided_debugging_active: false
guided_reasoning_active: false
weighted_planning_active: true
closeout_active: true
adaptive_workflow_active: false
active_specialty_signals: 3/7
workflow_contract_activation: entry=active:force repair=not_needed
workflow_contract_events: 1
runtime_spine: coverage=6/7, status=passed, missing=none
runtime_profile: minimum_viable_agent
mva_profile_active: true
runtime_spine_detail: context=78 latest=memory_boundary_evaluated decision=38 latest=action_reviewed permission=0 latest=none tool_execution=33 latest=api_request_completed state_update=46 latest=agent_loop_step_evaluated verification=1 latest=reflection_pass_completed closeout=3 latest=assistant_responded risky_tool_runs=6 risky_tool_reviewed=6 risky_tool_missing_action_review=none gate_outcomes=total=13, protective_block=0, recoverable_friction=0, unrecovered_block=1, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=12 stop_reason=no_issue stop_terminal_status=partial stop_action=continue stop_failure_type=missing rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none route_recovery=events=0, read_search=false, mutation_blocked=false, safety=missing action_scores=12 latest_action_score=1 low_action_score_count=6 phase_misaligned_actions=7 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=6 observer_quality_warning_labels=missing_permission_source permission_sources=none runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=9 provider_protocol_repairs=0 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 provider_request_started=9 provider_request_completed=9 provider_request_timeout=0 provider_request_retrying=0 provider_request_slow_warning=0 provider_request_cancelled=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=2 context_zone_duplicate_blocks_removed=0 context_zone_provenance_markers=0 agent_loop_steps=16 context_zones=9 completion_contract=partial
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: event:action_decision_evaluated,event:action_reviewed,event:stop_check_evaluated,event:completion_contract_evaluated
runtime_spine_status: passed
runtime_spine_missing: none
risky_tool_runs: 6
risky_tool_reviewed: 6
risky_tool_missing_action_review: none
gate_outcomes: total=13, protective_block=0, recoverable_friction=0, unrecovered_block=1, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=12
gate_outcome_records: action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,+1
gate_outcome_total: 13
gate_outcome_protective_blocks: 0
gate_outcome_recoverable_friction: 0
gate_outcome_unrecovered_blocks: 1
gate_outcome_suspected_false_positives: 0
gate_outcome_policy_correct_but_ux_costly: 0
gate_outcome_harmless_passes: 12
gate_outcome_failure_owners: action_review
agent_loop_steps: 16
context_zones_materialized: true
context_zone_task_state_empty: false
context_zone_current_decision_request_empty: false
context_zone_envelope_messages: 1
context_zone_source_messages: 2
context_zone_duplicate_blocks_removed: 0
context_zone_provenance_markers: 0
state_transition_recorded: false
completion_contract_status: partial
completion_contract_proof_status: not_run
candidate_score_calibrated: false
candidate_score_disagreement: true
observer_outcome_recorded: true
memory_boundary_recorded: true
verification_proof_status: not_run
verification_proof_summary: validation required but no evidence was recorded
verification_proof_kinds: none
verification_proof_support_status: not_run
verification_proof_support_summary: verification proof status not_run blocks verified closeout before proof-kind policy
verification_proof_supports_verified: false
verification_proof_residual_risk: true
risk_signal: entry=ordinary runtime=none
risk_signal_reasons: ordinary change surface
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
required_commands: 2
agent_required_commands: 0
harness_commands: 2
required_command_status: ok
validation_events: 0
stage_validation_events: 0
tool_progress_events: 6
guided_debugging_events: 0
guided_reasoning_events: 0
workflow_plan_events: 1
weighted_plan_events: 1
reweighted_plan_events: 0
adaptive_trigger_events: 0
adaptive_triggers: none
latest_top_priority: P1
latest_top_importance_score: 0.7799999713897705
latest_top_weight_share: 0.20000000298023224
acceptance_accepted: missing
closeout_status: not_verified
closeout_tool_records: 5
closeout_tool_evidence: tool evidence: records=5 completed=5 failed=0 denied=0 validation=0 closeout=0 repair=0 changed=0 workflows=code_change commands=none
runtime_diet: prompt=8263 tool_schema=3186 tools=15 workflow=minimal
note: guided debugging is expected only after a blocker or failed validation
```

Agent monitor tail:

```text
[2026-06-08T16:05:18+0800] agent-run still running elapsed=30s idle_for=0s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=45972
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

- Bundle: `docs/benchmarks/live-live-eval-20260608-160435/weighting-p0-budget-exceeded-deny/run-bundle`
- Task: `docs/benchmarks/live-live-eval-20260608-160435/weighting-p0-budget-exceeded-deny/run-bundle/task.json`
- Steps: `docs/benchmarks/live-live-eval-20260608-160435/weighting-p0-budget-exceeded-deny/run-bundle/steps.jsonl`
- Events: `docs/benchmarks/live-live-eval-20260608-160435/weighting-p0-budget-exceeded-deny/run-bundle/events.jsonl`
- Final report: `docs/benchmarks/live-live-eval-20260608-160435/weighting-p0-budget-exceeded-deny/run-bundle/final_report.md`
