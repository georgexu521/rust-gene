# Live Eval Report: weighting-p0-budget-exceeded-deny

- Run id: `live-eval-20260608-154637`
- Sample: `evalsets/live_tasks/weighting-p0-budget-exceeded-deny.yaml`
- Worktree: `target/live-evals/live-eval-20260608-154637/weighting-p0-budget-exceeded-deny/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/live-eval-20260608-154637/weighting-p0-budget-exceeded-deny/env`
- Test status: `ok`
- Generated: `2026-06-08 15:54:30 +0800`

## Git Status

```text
```

## Diff Stat

```text
```

## Required Commands

```text
$ test -f fixtures/weighting_p0_budget/source.txt
[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-live-eval-20260608-154637/weighting-p0-budget-exceeded-deny/agent-output.md`
- Events: `docs/benchmarks/live-live-eval-20260608-154637/weighting-p0-budget-exceeded-deny/agent-events.jsonl`
- Monitor: `docs/benchmarks/live-live-eval-20260608-154637/weighting-p0-budget-exceeded-deny/agent-monitor.log`
- Metrics: `docs/benchmarks/live-live-eval-20260608-154637/weighting-p0-budget-exceeded-deny/agent-run-metrics.json`

Event counts:

```text
closeout: 1
complete: 1
eval_started: 1
permission_request: 4
runtime_diagnostic: 61
start: 1
text_chunk: 1038
thinking_complete: 28
thinking_start: 28
tool_call_args: 2741
tool_call_complete: 37
tool_call_start: 37
tool_execution_complete: 37
tool_execution_progress: 10
tool_execution_start: 14
trace_summary: 1
usage: 28
```

Quality signals:

```text
output_chars: 5247
diff_chars: 0
diff_files_changed: 0
diff_files_changed_raw: 0
generated_dependency_files_ignored: 0
tool_executions: 37
first_write_tool_index: none
forbidden_tool_uses: none
tool_errors: 10
tool_failures: 10
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 688
test_status: ok
verification_passed: false
stage_validation_passed: false
acceptance_accepted: None
closeout_status: failed
closeout_tool_records: 33
closeout_tool_evidence: tool evidence: records=33 completed=23 failed=6 denied=4 validation=9 closeout=10 repair=10 changed=0 workflows=code_change commands=cd /Users/georgexu/Desktop/rust-agent/target/live-evals/live-eval-20260608-154637/weighting-p0-budget-excee...
runtime_diet: prompt=42810 tool_schema=3186 tools=15 workflow=minimal closeout=full validation=failed:2/2
adaptive_triggers: none
risk_signal: entry=ordinary runtime=high
risk_signal_reasons: ordinary change surface
trace_event_types: provider.request.done,cache.usage,api.done,closeout,execution.report,closeout.background,closeout.background,memory.proposal,runtime.diet,completion.contract,memory.boundary,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: audit_or_regression_check
behavior_assertions: budget_gate_denies_after_limit,no_unbounded_tool_loop
behavior_assertion_status: passed
output_assertions: none
output_assertion_status: none
output_assertion_missing: none
trajectory_assertions: none
trajectory_assertion_status: none
trajectory_assertion_missing: none
runtime_spine: coverage=7/7, status=passed, missing=none
runtime_profile: minimum_viable_agent
mva_profile_active: true
runtime_spine_detail: context=230 latest=memory_boundary_evaluated decision=116 latest=action_reviewed permission=8 latest=permission_resolved tool_execution=102 latest=api_request_completed state_update=156 latest=agent_loop_step_evaluated verification=10 latest=guided_debugging_completed closeout=3 latest=assistant_responded risky_tool_runs=7 risky_tool_reviewed=7 risky_tool_missing_action_review=none gate_outcomes=total=42, protective_block=5, recoverable_friction=0, unrecovered_block=4, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=33 stop_reason=consecutive_permission_blocks stop_terminal_status=needs_user stop_action=ask_user stop_failure_type=permission_block rollback_recommended=false rollback_completed=false recovery_failure_types=permission_block recovery_kinds=ask_user_or_choose_safer_path route_recovery=events=0, read_search=false, mutation_blocked=false, safety=missing action_scores=37 latest_action_score=13 low_action_score_count=2 phase_misaligned_actions=4 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=4 observer_quality_warning_labels=missing_permission_source permission_sources=user_once_reject runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=28 provider_protocol_repairs=0 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 provider_request_started=28 provider_request_completed=28 provider_request_timeout=0 provider_request_retrying=0 provider_request_slow_warning=0 provider_request_cancelled=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=21 context_zone_duplicate_blocks_removed=0 context_zone_provenance_markers=0 agent_loop_steps=54 context_zones=28 completion_contract=failed
runtime_spine_trace_present: true
runtime_spine_phase_coverage: 7/7
runtime_spine_observed_phases: context,decision,permission,tool_execution,state_update,verification,closeout
runtime_spine_assertions: event:action_decision_evaluated,event:action_reviewed,event:stop_check_evaluated,event:completion_contract_evaluated
runtime_spine_status: passed
runtime_spine_missing: none
risky_tool_runs: 7
risky_tool_reviewed: 7
risky_tool_missing_action_review: none
gate_outcomes: total=42, protective_block=5, recoverable_friction=0, unrecovered_block=4, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=33
gate_outcome_records: action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,+30
gate_outcome_total: 42
gate_outcome_protective_blocks: 5
gate_outcome_recoverable_friction: 0
gate_outcome_unrecovered_blocks: 4
gate_outcome_suspected_false_positives: 0
gate_outcome_policy_correct_but_ux_costly: 0
gate_outcome_harmless_passes: 33
gate_outcome_failure_owners: action_review
route_recovery: events=0, read_search=false, mutation_blocked=false, safety=missing
route_recovery_events: 0
route_recovery_failure_types: none
route_recovery_kinds: none
route_recovery_read_search_expanded: false
route_recovery_mutation_blocked: false
route_recovery_safety_monotonic: missing
route_recovery_unsafe_mutation_expansion: false
agent_loop_steps: 54
context_zones_materialized: true
context_zone_task_state_empty: false
context_zone_current_decision_request_empty: false
context_zone_envelope_messages: 1
context_zone_source_messages: 21
context_zone_duplicate_blocks_removed: 0
context_zone_provenance_markers: 0
state_transition_recorded: false
completion_contract_status: failed
completion_contract_proof_status: failed
candidate_score_calibrated: false
candidate_score_disagreement: false
observer_outcome_recorded: true
memory_boundary_recorded: true
verification_proof_status: failed
verification_proof_summary: validation failed 2/2 current checks
verification_proof_kinds: none
verification_proof_support_status: failed
verification_proof_support_summary: verification proof status failed blocks verified closeout before proof-kind policy
verification_proof_supports_verified: false
verification_proof_residual_risk: true
premature_edit_count: 0
evidence_before_first_edit: true
scope_drift_count: 2
invalid_action_count: 8
repeated_action_count: 2
failed_action_count: 20
user_question_count: 25
unnecessary_question_count: 0
verification_attempted: true
verification_passed: true
tool_call_count: 14
llm_call_count: 28
warning: no_code_diff
warning: tool_errors_seen
warning: closeout_not_successful
failure_owner: agent_flow
outcome_score: 40
process_score: 34
efficiency_score: 28
agent_score: 36
score_penalties: run_failed,verification_failed,closeout_not_successful,scope_drift,repeated_action,invalid_action,tool_budget_exceeded,failed_actions,repeated_actions,user_questions,llm_call_budget_pressure
```

Specialty signals:

```text
memory_active: false
automation_active: true
guided_debugging_active: true
guided_reasoning_active: false
weighted_planning_active: true
closeout_active: true
adaptive_workflow_active: false
active_specialty_signals: 4/7
workflow_contract_activation: entry=active:force repair=active_after_failure
workflow_contract_events: 1
runtime_spine: coverage=7/7, status=passed, missing=none
runtime_profile: minimum_viable_agent
mva_profile_active: true
runtime_spine_detail: context=230 latest=memory_boundary_evaluated decision=116 latest=action_reviewed permission=8 latest=permission_resolved tool_execution=102 latest=api_request_completed state_update=156 latest=agent_loop_step_evaluated verification=10 latest=guided_debugging_completed closeout=3 latest=assistant_responded risky_tool_runs=7 risky_tool_reviewed=7 risky_tool_missing_action_review=none gate_outcomes=total=42, protective_block=5, recoverable_friction=0, unrecovered_block=4, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=33 stop_reason=consecutive_permission_blocks stop_terminal_status=needs_user stop_action=ask_user stop_failure_type=permission_block rollback_recommended=false rollback_completed=false recovery_failure_types=permission_block recovery_kinds=ask_user_or_choose_safer_path route_recovery=events=0, read_search=false, mutation_blocked=false, safety=missing action_scores=37 latest_action_score=13 low_action_score_count=2 phase_misaligned_actions=4 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=4 observer_quality_warning_labels=missing_permission_source permission_sources=user_once_reject runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=28 provider_protocol_repairs=0 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 provider_request_started=28 provider_request_completed=28 provider_request_timeout=0 provider_request_retrying=0 provider_request_slow_warning=0 provider_request_cancelled=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=21 context_zone_duplicate_blocks_removed=0 context_zone_provenance_markers=0 agent_loop_steps=54 context_zones=28 completion_contract=failed
runtime_spine_phase_coverage: 7/7
runtime_spine_observed_phases: context,decision,permission,tool_execution,state_update,verification,closeout
runtime_spine_assertions: event:action_decision_evaluated,event:action_reviewed,event:stop_check_evaluated,event:completion_contract_evaluated
runtime_spine_status: passed
runtime_spine_missing: none
risky_tool_runs: 7
risky_tool_reviewed: 7
risky_tool_missing_action_review: none
gate_outcomes: total=42, protective_block=5, recoverable_friction=0, unrecovered_block=4, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=33
gate_outcome_records: action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,+30
gate_outcome_total: 42
gate_outcome_protective_blocks: 5
gate_outcome_recoverable_friction: 0
gate_outcome_unrecovered_blocks: 4
gate_outcome_suspected_false_positives: 0
gate_outcome_policy_correct_but_ux_costly: 0
gate_outcome_harmless_passes: 33
gate_outcome_failure_owners: action_review
agent_loop_steps: 54
context_zones_materialized: true
context_zone_task_state_empty: false
context_zone_current_decision_request_empty: false
context_zone_envelope_messages: 1
context_zone_source_messages: 21
context_zone_duplicate_blocks_removed: 0
context_zone_provenance_markers: 0
state_transition_recorded: false
completion_contract_status: failed
completion_contract_proof_status: failed
candidate_score_calibrated: false
candidate_score_disagreement: false
observer_outcome_recorded: true
memory_boundary_recorded: true
verification_proof_status: failed
verification_proof_summary: validation failed 2/2 current checks
verification_proof_kinds: none
verification_proof_support_status: failed
verification_proof_support_summary: verification proof status failed blocks verified closeout before proof-kind policy
verification_proof_supports_verified: false
verification_proof_residual_risk: true
risk_signal: entry=ordinary runtime=high
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
required_commands: 1
agent_required_commands: 0
harness_commands: 1
required_command_status: ok
validation_events: 0
stage_validation_events: 0
tool_progress_events: 10
guided_debugging_events: 9
guided_reasoning_events: 0
workflow_plan_events: 1
weighted_plan_events: 1
reweighted_plan_events: 0
adaptive_trigger_events: 0
adaptive_triggers: none
latest_top_priority: P2
latest_top_importance_score: 0.5199999809265137
latest_top_weight_share: 0.30320701003074646
acceptance_accepted: missing
closeout_status: failed
closeout_tool_records: 33
closeout_tool_evidence: tool evidence: records=33 completed=23 failed=6 denied=4 validation=9 closeout=10 repair=10 changed=0 workflows=code_change commands=cd /Users/georgexu/Desktop/rust-agent/target/live-evals/live-eval-20260608-154637/weighting-p0-budget-excee...
runtime_diet: prompt=42810 tool_schema=3186 tools=15 workflow=minimal
```

Agent monitor tail:

```text
[2026-06-08T15:47:19+0800] agent-run still running elapsed=30s idle_for=25s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=794
[2026-06-08T15:47:50+0800] agent-run still running elapsed=60s idle_for=0s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=148383
[2026-06-08T15:48:20+0800] agent-run still running elapsed=90s idle_for=0s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=249845
[2026-06-08T15:48:50+0800] agent-run still running elapsed=121s idle_for=0s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=315207
[2026-06-08T15:49:21+0800] agent-run still running elapsed=151s idle_for=5s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=351769
[2026-06-08T15:49:51+0800] agent-run still running elapsed=181s idle_for=20s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=356549
[2026-06-08T15:50:21+0800] agent-run still running elapsed=212s idle_for=50s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=356549
[2026-06-08T15:50:51+0800] agent-run still running elapsed=242s idle_for=80s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=356549
[2026-06-08T15:51:22+0800] agent-run still running elapsed=272s idle_for=111s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=356549
[2026-06-08T15:51:52+0800] agent-run still running elapsed=302s idle_for=5s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=370696
[2026-06-08T15:52:22+0800] agent-run still running elapsed=333s idle_for=20s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=373787
[2026-06-08T15:52:53+0800] agent-run still running elapsed=363s idle_for=50s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=373787
[2026-06-08T15:53:23+0800] agent-run still running elapsed=393s idle_for=5s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=403204
[2026-06-08T15:53:53+0800] agent-run still running elapsed=424s idle_for=0s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=427403
[2026-06-08T15:54:24+0800] agent-run still running elapsed=454s idle_for=0s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=500251
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

- Bundle: `docs/benchmarks/live-live-eval-20260608-154637/weighting-p0-budget-exceeded-deny/run-bundle`
- Task: `docs/benchmarks/live-live-eval-20260608-154637/weighting-p0-budget-exceeded-deny/run-bundle/task.json`
- Steps: `docs/benchmarks/live-live-eval-20260608-154637/weighting-p0-budget-exceeded-deny/run-bundle/steps.jsonl`
- Events: `docs/benchmarks/live-live-eval-20260608-154637/weighting-p0-budget-exceeded-deny/run-bundle/events.jsonl`
- Final report: `docs/benchmarks/live-live-eval-20260608-154637/weighting-p0-budget-exceeded-deny/run-bundle/final_report.md`
