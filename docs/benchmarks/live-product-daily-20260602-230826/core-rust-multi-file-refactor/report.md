# Live Eval Report: core-rust-multi-file-refactor

- Run id: `product-daily-20260602-230826`
- Sample: `evalsets/live_tasks/core-rust-multi-file-refactor.yaml`
- Worktree: `target/live-evals/product-daily-20260602-230826/core-rust-multi-file-refactor/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/product-daily-20260602-230826/core-rust-multi-file-refactor/env`
- Test status: `ok`
- Generated: `2026-06-02 23:19:46 +0800`

## Git Status

```text
 M fixtures/core_quality/rust_refactor/Cargo.toml
 M fixtures/core_quality/rust_refactor/src/report.rs
 M fixtures/core_quality/rust_refactor/src/stats.rs
?? fixtures/core_quality/rust_refactor/Cargo.lock
```

## Diff Stat

```text
 fixtures/core_quality/rust_refactor/Cargo.toml    | 3 +++
 fixtures/core_quality/rust_refactor/src/report.rs | 5 ++++-
 fixtures/core_quality/rust_refactor/src/stats.rs  | 8 ++++++--
 3 files changed, 13 insertions(+), 3 deletions(-)
 /dev/null => fixtures/core_quality/rust_refactor/Cargo.lock | 7 +++++++
 1 file changed, 7 insertions(+)
```

## Required Commands

```text
$ cargo test -q --manifest-path fixtures/core_quality/rust_refactor/Cargo.toml

running 2 tests
..
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

[exit status: 0]

$ rg 'avg=' fixtures/core_quality/rust_refactor/src/report.rs
    format!("{label} total={total} avg={avg}")
[exit status: 0]

$ rg 'stats::average' fixtures/core_quality/rust_refactor/src/report.rs
    let avg = stats::average(values);
[exit status: 0]

$ rg 'pub fn average' fixtures/core_quality/rust_refactor/src/stats.rs
pub fn average(values: &[u32]) -> u32 {
[exit status: 0]

$ rg 'format_total' fixtures/core_quality/rust_refactor/src/lib.rs
pub fn format_total(label: &str, values: &[u32]) -> String {
        assert_eq!(format_total("latency", &[3, 5, 7]), "latency total=15 avg=5");
        assert_eq!(format_total("latency", &[]), "latency total=0 avg=0");
[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-product-daily-20260602-230826/core-rust-multi-file-refactor/agent-output.md`
- Events: `docs/benchmarks/live-product-daily-20260602-230826/core-rust-multi-file-refactor/agent-events.jsonl`
- Monitor: `docs/benchmarks/live-product-daily-20260602-230826/core-rust-multi-file-refactor/agent-monitor.log`

Event counts:

```text
complete: 1
eval_started: 1
runtime_diagnostic: 13
start: 1
text_chunk: 1
tool_execution_complete: 10
tool_execution_progress: 3
tool_execution_start: 10
trace_summary: 1
```

Quality signals:

```text
output_chars: 2277
diff_chars: 1781
diff_files_changed: 4
diff_files_changed_raw: 4
generated_dependency_files_ignored: 0
tool_executions: 10
first_write_tool_index: 8
forbidden_tool_uses: none
tool_errors: 1
tool_failures: 2
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 239
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
closeout_tool_records: 11
closeout_tool_evidence: tool evidence: records=11 completed=9 failed=2 denied=0 validation=1 closeout=3 repair=4 changed=2 workflows=code_change commands=head -50 Cargo.toml 2>/dev/null || echo "no parent Cargo.toml" | cargo test -q --manifest-path fixtures/core_q...
runtime_diet: prompt=22500 tool_schema=4272 tools=19 workflow=strict closeout=full validation=passed:5/5 recovered_failed:3
adaptive_triggers: risk_signal_high,required_validation,first_code_change,verification_failed,acceptance_rejected
risk_signal: entry=high runtime=high
risk_signal_reasons: required validation commands present; complex required-validation surface; broad validation command requested
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
runtime_spine: coverage=6/7, status=none, missing=none
runtime_profile: none
mva_profile_active: false
runtime_spine_detail: context=46 latest=runtime_diet_report decision=45 latest=workflow_plan_progress permission=0 latest=none tool_execution=30 latest=tool_completed state_update=68 latest=workflow_fallback verification=17 latest=acceptance_review_completed closeout=3 latest=assistant_responded risky_tool_runs=3 risky_tool_reviewed=3 risky_tool_missing_action_review=none gate_outcomes=total=12, protective_block=0, recoverable_friction=1, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=11 stop_reason=no_issue stop_terminal_status=completed stop_action=continue stop_failure_type=execution_failed rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none route_recovery=events=0, read_search=false, mutation_blocked=false, safety=missing action_scores=10 latest_action_score=22 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=none runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=8 provider_protocol_repairs=0 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=44 context_zone_duplicate_blocks_removed=16 context_zone_provenance_markers=0 agent_loop_steps=16 context_zones=8 completion_contract=completed
runtime_spine_trace_present: true
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: none
runtime_spine_status: none
runtime_spine_missing: none
risky_tool_runs: 3
risky_tool_reviewed: 3
risky_tool_missing_action_review: none
gate_outcomes: total=12, protective_block=0, recoverable_friction=1, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=11
gate_outcome_records: action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:revise:recoverable_friction,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,closeout:passed:harmless_pass
gate_outcome_total: 12
gate_outcome_protective_blocks: 0
gate_outcome_recoverable_friction: 1
gate_outcome_unrecovered_blocks: 0
gate_outcome_suspected_false_positives: 0
gate_outcome_policy_correct_but_ux_costly: 0
gate_outcome_harmless_passes: 11
gate_outcome_failure_owners: none
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
context_zone_source_messages: 44
context_zone_duplicate_blocks_removed: 16
context_zone_provenance_markers: 0
state_transition_recorded: false
completion_contract_status: completed
completion_contract_proof_status: verified
candidate_score_calibrated: false
candidate_score_disagreement: false
observer_outcome_recorded: true
memory_boundary_recorded: true
verification_proof_status: verified
verification_proof_summary: required validation passed 5/5 commands
verification_proof_kinds: command_passed,required_validation_passed
verification_proof_support_status: verified
verification_proof_support_summary: verified by command_passed,required_validation_passed
verification_proof_supports_verified: true
verification_proof_residual_risk: false
premature_edit_count: 0
evidence_before_first_edit: true
scope_drift_count: 0
invalid_action_count: 5
repeated_action_count: 5
failed_action_count: 3
user_question_count: 0
unnecessary_question_count: 0
verification_attempted: true
verification_passed: true
tool_call_count: 10
llm_call_count: 8
warning: tool_errors_seen
warning: earlier_verification_failed_before_repair
warning: earlier_stage_validation_failed_before_repair
failure_owner: none
outcome_score: 100
process_score: 60
efficiency_score: 56
agent_score: 79
score_penalties: repeated_action,invalid_action,failed_actions,repeated_actions
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
runtime_spine: coverage=6/7, status=none, missing=none
runtime_profile: none
mva_profile_active: false
runtime_spine_detail: context=46 latest=runtime_diet_report decision=45 latest=workflow_plan_progress permission=0 latest=none tool_execution=30 latest=tool_completed state_update=68 latest=workflow_fallback verification=17 latest=acceptance_review_completed closeout=3 latest=assistant_responded risky_tool_runs=3 risky_tool_reviewed=3 risky_tool_missing_action_review=none gate_outcomes=total=12, protective_block=0, recoverable_friction=1, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=11 stop_reason=no_issue stop_terminal_status=completed stop_action=continue stop_failure_type=execution_failed rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none route_recovery=events=0, read_search=false, mutation_blocked=false, safety=missing action_scores=10 latest_action_score=22 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=none runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=8 provider_protocol_repairs=0 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=44 context_zone_duplicate_blocks_removed=16 context_zone_provenance_markers=0 agent_loop_steps=16 context_zones=8 completion_contract=completed
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: none
runtime_spine_status: none
runtime_spine_missing: none
risky_tool_runs: 3
risky_tool_reviewed: 3
risky_tool_missing_action_review: none
gate_outcomes: total=12, protective_block=0, recoverable_friction=1, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=11
gate_outcome_records: action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:revise:recoverable_friction,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,closeout:passed:harmless_pass
gate_outcome_total: 12
gate_outcome_protective_blocks: 0
gate_outcome_recoverable_friction: 1
gate_outcome_unrecovered_blocks: 0
gate_outcome_suspected_false_positives: 0
gate_outcome_policy_correct_but_ux_costly: 0
gate_outcome_harmless_passes: 11
gate_outcome_failure_owners: none
agent_loop_steps: 16
context_zones_materialized: true
context_zone_task_state_empty: false
context_zone_current_decision_request_empty: false
context_zone_envelope_messages: 1
context_zone_source_messages: 44
context_zone_duplicate_blocks_removed: 16
context_zone_provenance_markers: 0
state_transition_recorded: false
completion_contract_status: completed
completion_contract_proof_status: verified
candidate_score_calibrated: false
candidate_score_disagreement: false
observer_outcome_recorded: true
memory_boundary_recorded: true
verification_proof_status: verified
verification_proof_summary: required validation passed 5/5 commands
verification_proof_kinds: command_passed,required_validation_passed
verification_proof_support_status: verified
verification_proof_support_summary: verified by command_passed,required_validation_passed
verification_proof_supports_verified: true
verification_proof_residual_risk: false
risk_signal: entry=high runtime=high
risk_signal_reasons: required validation commands present; complex required-validation surface; broad validation command requested
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
required_commands: 5
agent_required_commands: 5
harness_commands: 0
required_command_status: ok
validation_events: 3
stage_validation_events: 3
tool_progress_events: 3
guided_debugging_events: 4
guided_reasoning_events: 0
workflow_plan_events: 2
weighted_plan_events: 1
reweighted_plan_events: 1
adaptive_trigger_events: 5
adaptive_triggers: risk_signal_high,required_validation,first_code_change,verification_failed,acceptance_rejected
latest_top_priority: P0
latest_top_importance_score: 0.9550000429153442
latest_top_weight_share: 0.24967321753501892
acceptance_accepted: True
closeout_status: passed
closeout_tool_records: 11
closeout_tool_evidence: tool evidence: records=11 completed=9 failed=2 denied=0 validation=1 closeout=3 repair=4 changed=2 workflows=code_change commands=head -50 Cargo.toml 2>/dev/null || echo "no parent Cargo.toml" | cargo test -q --manifest-path fixtures/core_q...
runtime_diet: prompt=22500 tool_schema=4272 tools=19 workflow=strict
```

Agent monitor tail:

```text
[2026-06-02T23:12:56+0800] agent-run still running elapsed=30s idle_for=25s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=787
[2026-06-02T23:13:26+0800] agent-run still running elapsed=60s idle_for=0s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=1038
[2026-06-02T23:13:56+0800] agent-run still running elapsed=90s idle_for=10s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=25042
[2026-06-02T23:14:26+0800] agent-run still running elapsed=120s idle_for=40s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=25042
[2026-06-02T23:14:56+0800] agent-run still running elapsed=150s idle_for=10s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=30429
[2026-06-02T23:15:26+0800] agent-run still running elapsed=180s idle_for=40s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=30429
[2026-06-02T23:15:56+0800] agent-run still running elapsed=210s idle_for=15s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=37213
[2026-06-02T23:16:26+0800] agent-run still running elapsed=240s idle_for=45s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=37213
[2026-06-02T23:16:56+0800] agent-run still running elapsed=270s idle_for=75s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=37213
[2026-06-02T23:17:26+0800] agent-run still running elapsed=300s idle_for=105s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=37213
[2026-06-02T23:17:56+0800] agent-run still running elapsed=330s idle_for=5s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=45150
[2026-06-02T23:18:27+0800] agent-run still running elapsed=360s idle_for=35s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=45150
[2026-06-02T23:18:57+0800] agent-run still running elapsed=390s idle_for=65s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=45150
[2026-06-02T23:19:27+0800] agent-run still running elapsed=420s idle_for=0s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=45383
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

- Bundle: `docs/benchmarks/live-product-daily-20260602-230826/core-rust-multi-file-refactor/run-bundle`
- Task: `docs/benchmarks/live-product-daily-20260602-230826/core-rust-multi-file-refactor/run-bundle/task.json`
- Steps: `docs/benchmarks/live-product-daily-20260602-230826/core-rust-multi-file-refactor/run-bundle/steps.jsonl`
- Events: `docs/benchmarks/live-product-daily-20260602-230826/core-rust-multi-file-refactor/run-bundle/events.jsonl`
- Final report: `docs/benchmarks/live-product-daily-20260602-230826/core-rust-multi-file-refactor/run-bundle/final_report.md`
