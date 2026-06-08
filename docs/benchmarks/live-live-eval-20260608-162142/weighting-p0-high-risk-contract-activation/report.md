# Live Eval Report: weighting-p0-high-risk-contract-activation

- Run id: `live-eval-20260608-162142`
- Sample: `evalsets/live_tasks/weighting-p0-high-risk-contract-activation.yaml`
- Worktree: `target/live-evals/live-eval-20260608-162142/weighting-p0-high-risk-contract-activation/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/live-eval-20260608-162142/weighting-p0-high-risk-contract-activation/env`
- Test status: `ok`
- Generated: `2026-06-08 16:23:56 +0800`

## Git Status

```text
 M fixtures/weighting_p0_contract/src/a.rs
 M fixtures/weighting_p0_contract/src/b.rs
```

## Diff Stat

```text
 fixtures/weighting_p0_contract/src/a.rs | 2 +-
 fixtures/weighting_p0_contract/src/b.rs | 2 +-
 2 files changed, 2 insertions(+), 2 deletions(-)
```

## Required Commands

```text
$ rg -F 'priority_value' fixtures/weighting_p0_contract/src
fixtures/weighting_p0_contract/src/b.rs:pub fn priority_value() -> i32 { 1 }
fixtures/weighting_p0_contract/src/a.rs:pub fn priority_value() -> i32 { 1 }
[exit status: 0]

$ ! rg -F 'fn value' fixtures/weighting_p0_contract/src
[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-live-eval-20260608-162142/weighting-p0-high-risk-contract-activation/agent-output.md`
- Events: `docs/benchmarks/live-live-eval-20260608-162142/weighting-p0-high-risk-contract-activation/agent-events.jsonl`
- Monitor: `docs/benchmarks/live-live-eval-20260608-162142/weighting-p0-high-risk-contract-activation/agent-monitor.log`
- Metrics: `docs/benchmarks/live-live-eval-20260608-162142/weighting-p0-high-risk-contract-activation/agent-run-metrics.json`

Event counts:

```text
closeout: 1
complete: 1
eval_started: 1
runtime_diagnostic: 11
start: 1
text_chunk: 34
thinking_complete: 3
thinking_start: 3
tool_call_args: 213
tool_call_complete: 6
tool_call_start: 6
tool_execution_complete: 6
tool_execution_progress: 2
tool_execution_start: 2
trace_summary: 1
usage: 3
```

Quality signals:

```text
output_chars: 1815
diff_chars: 596
diff_files_changed: 2
diff_files_changed_raw: 2
generated_dependency_files_ignored: 0
tool_executions: 6
first_write_tool_index: 1
forbidden_tool_uses: none
tool_errors: 0
tool_failures: 0
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 117
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
closeout_tool_records: 6
closeout_tool_evidence: tool evidence: records=6 completed=6 failed=0 denied=0 validation=0 closeout=2 repair=2 changed=2 workflows=code_change commands=none
runtime_diet: prompt=7282 tool_schema=3186 tools=15 workflow=strict closeout=full validation=passed:2/2 recovered_failed:2
adaptive_triggers: risk_signal_high,required_validation,first_code_change
risk_signal: entry=high runtime=none
risk_signal_reasons: route risk is high; required validation commands present
trace_event_types: workflow.plan,memory.boundary,workflow.fallback,closeout,execution.report,closeout.background,closeout.background,memory.proposal,runtime.diet,completion.contract,memory.boundary,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: seeded_code_change
behavior_assertions: high_risk_contract_activation,workflow_scoring_summary
behavior_assertion_status: passed
output_assertions: none
output_assertion_status: none
output_assertion_missing: none
trajectory_assertions: none
trajectory_assertion_status: none
trajectory_assertion_missing: none
runtime_spine: coverage=6/7, status=failed, missing=event:risk_signal,event:workflow_contract,event:workflow_plan
runtime_profile: minimum_viable_agent
mva_profile_active: true
runtime_spine_detail: context=31 latest=memory_boundary_evaluated decision=25 latest=workflow_plan_progress permission=0 latest=none tool_execution=15 latest=tool_completed state_update=25 latest=workflow_fallback verification=5 latest=acceptance_review_completed closeout=3 latest=assistant_responded risky_tool_runs=2 risky_tool_reviewed=2 risky_tool_missing_action_review=none gate_outcomes=total=7, protective_block=0, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=7 stop_reason=no_issue stop_terminal_status=completed stop_action=continue stop_failure_type=missing rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none route_recovery=events=0, read_search=false, mutation_blocked=false, safety=missing action_scores=6 latest_action_score=17 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=none runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=3 provider_protocol_repairs=0 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 provider_request_started=3 provider_request_completed=3 provider_request_timeout=0 provider_request_retrying=0 provider_request_slow_warning=0 provider_request_cancelled=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=7 context_zone_duplicate_blocks_removed=2 context_zone_provenance_markers=0 agent_loop_steps=6 context_zones=3 completion_contract=completed
runtime_spine_trace_present: true
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: event:risk_signal,event:workflow_contract,event:workflow_plan,event:action_decision_evaluated,event:action_reviewed,event:completion_contract_evaluated,completion_status:completed
runtime_spine_status: failed
runtime_spine_missing: event:risk_signal,event:workflow_contract,event:workflow_plan
risky_tool_runs: 2
risky_tool_reviewed: 2
risky_tool_missing_action_review: none
gate_outcomes: total=7, protective_block=0, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=7
gate_outcome_records: action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,closeout:passed:harmless_pass
gate_outcome_total: 7
gate_outcome_protective_blocks: 0
gate_outcome_recoverable_friction: 0
gate_outcome_unrecovered_blocks: 0
gate_outcome_suspected_false_positives: 0
gate_outcome_policy_correct_but_ux_costly: 0
gate_outcome_harmless_passes: 7
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
context_zone_source_messages: 7
context_zone_duplicate_blocks_removed: 2
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
failed_action_count: 0
user_question_count: 0
unnecessary_question_count: 0
verification_attempted: true
verification_passed: true
tool_call_count: 2
llm_call_count: 3
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
guided_reasoning_active: true
weighted_planning_active: true
closeout_active: true
adaptive_workflow_active: true
active_specialty_signals: 5/7
workflow_contract_activation: entry=active:force repair=not_needed
workflow_contract_events: 1
runtime_spine: coverage=6/7, status=failed, missing=event:risk_signal,event:workflow_contract,event:workflow_plan
runtime_profile: minimum_viable_agent
mva_profile_active: true
runtime_spine_detail: context=31 latest=memory_boundary_evaluated decision=25 latest=workflow_plan_progress permission=0 latest=none tool_execution=15 latest=tool_completed state_update=25 latest=workflow_fallback verification=5 latest=acceptance_review_completed closeout=3 latest=assistant_responded risky_tool_runs=2 risky_tool_reviewed=2 risky_tool_missing_action_review=none gate_outcomes=total=7, protective_block=0, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=7 stop_reason=no_issue stop_terminal_status=completed stop_action=continue stop_failure_type=missing rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none route_recovery=events=0, read_search=false, mutation_blocked=false, safety=missing action_scores=6 latest_action_score=17 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=none runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=3 provider_protocol_repairs=0 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 provider_request_started=3 provider_request_completed=3 provider_request_timeout=0 provider_request_retrying=0 provider_request_slow_warning=0 provider_request_cancelled=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=7 context_zone_duplicate_blocks_removed=2 context_zone_provenance_markers=0 agent_loop_steps=6 context_zones=3 completion_contract=completed
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: event:risk_signal,event:workflow_contract,event:workflow_plan,event:action_decision_evaluated,event:action_reviewed,event:completion_contract_evaluated,completion_status:completed
runtime_spine_status: failed
runtime_spine_missing: event:risk_signal,event:workflow_contract,event:workflow_plan
risky_tool_runs: 2
risky_tool_reviewed: 2
risky_tool_missing_action_review: none
gate_outcomes: total=7, protective_block=0, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=7
gate_outcome_records: action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,closeout:passed:harmless_pass
gate_outcome_total: 7
gate_outcome_protective_blocks: 0
gate_outcome_recoverable_friction: 0
gate_outcome_unrecovered_blocks: 0
gate_outcome_suspected_false_positives: 0
gate_outcome_policy_correct_but_ux_costly: 0
gate_outcome_harmless_passes: 7
gate_outcome_failure_owners: none
agent_loop_steps: 6
context_zones_materialized: true
context_zone_task_state_empty: false
context_zone_current_decision_request_empty: false
context_zone_envelope_messages: 1
context_zone_source_messages: 7
context_zone_duplicate_blocks_removed: 2
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
risk_signal: entry=high runtime=none
risk_signal_reasons: route risk is high; required validation commands present
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
latest_top_priority: P1
latest_top_importance_score: 0.7899999618530273
latest_top_weight_share: 0.3788968622684479
acceptance_accepted: True
closeout_status: passed
closeout_tool_records: 6
closeout_tool_evidence: tool evidence: records=6 completed=6 failed=0 denied=0 validation=0 closeout=2 repair=2 changed=2 workflows=code_change commands=none
runtime_diet: prompt=7282 tool_schema=3186 tools=15 workflow=strict
note: guided debugging is expected only after a blocker or failed validation
```

Agent stderr tail:

```text
[2m2026-06-08T08:23:50.468696Z[0m [33m WARN[0m [2mpriority_agent::engine::streaming[0m[2m:[0m session end memory flush join failed: task 75 was cancelled
```

Agent monitor tail:

```text
[2026-06-08T16:22:24+0800] agent-run still running elapsed=30s idle_for=0s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=21152
[2026-06-08T16:22:54+0800] agent-run still running elapsed=60s idle_for=25s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=41637
[2026-06-08T16:23:24+0800] agent-run still running elapsed=90s idle_for=55s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=41637
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

- Bundle: `docs/benchmarks/live-live-eval-20260608-162142/weighting-p0-high-risk-contract-activation/run-bundle`
- Task: `docs/benchmarks/live-live-eval-20260608-162142/weighting-p0-high-risk-contract-activation/run-bundle/task.json`
- Steps: `docs/benchmarks/live-live-eval-20260608-162142/weighting-p0-high-risk-contract-activation/run-bundle/steps.jsonl`
- Events: `docs/benchmarks/live-live-eval-20260608-162142/weighting-p0-high-risk-contract-activation/run-bundle/events.jsonl`
- Final report: `docs/benchmarks/live-live-eval-20260608-162142/weighting-p0-high-risk-contract-activation/run-bundle/final_report.md`
