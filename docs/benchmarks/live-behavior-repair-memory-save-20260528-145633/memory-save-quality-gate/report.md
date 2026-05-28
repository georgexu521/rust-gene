# Live Eval Report: memory-save-quality-gate

- Run id: `behavior-repair-memory-save-20260528-145633`
- Sample: `evalsets/live_tasks/memory-save-quality-gate.yaml`
- Worktree: `target/live-evals/behavior-repair-memory-save-20260528-145633/memory-save-quality-gate/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/behavior-repair-memory-save-20260528-145633/memory-save-quality-gate/env`
- Test status: `failed`
- Generated: `2026-05-28 15:15:46 +0800`

## Git Status

```text
```

## Diff Stat

```text
```

## Required Commands

```text
$ cargo test -q memory -- --test-threads=1
warning: function `format_memory_write_outcome` is never used
   --> src/tui/app.rs:315:4
    |
315 | fn format_memory_write_outcome(
    |    ^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |
    = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default


running 216 tests
............................................................................. 77/216
memory::calibration::tests::built_in_calibration_samples_pass --- FAILED
....................................................................................... 165/216
......... 174/216
memory::quality::tests::explicit_does_not_accept_duplicate_memory --- FAILED
memory::quality::tests::explicit_does_not_accept_low_quality_note --- FAILED
........................................
failures:

---- memory::calibration::tests::built_in_calibration_samples_pass stdout ----

thread 'memory::calibration::tests::built_in_calibration_samples_pass' (766534) panicked at src/memory/calibration.rs:197:9:
failed calibration samples: [MemoryCalibrationResult { id: "explicit_low_value_note", expected: NotAccepted, actual: Accepted, score: Some(0.56542045), passed: false, reason: "write_score=0.57, status=Proposed, relevance=0.65, reuse=0.65, stability=0.33, trust=0.70, novelty=1.00, risk_reduction=0.25, token_cost=0.05, sensitivity_risk=0.00, kind=Note, stable=0.65, utility=0.65, specificity=0.55, volatility=0.70, duplication=0.00", rationale: "Explicit save can lower friction but must not bypass quality gates." }, MemoryCalibrationResult { id: "duplicate_project_fact", expected: Rejected, actual: Accepted, score: Some(0.65165913), passed: false, reason: "write_score=0.65, status=Rejected, relevance=0.85, reuse=0.80, stability=0.76, trust=0.72, novelty=0.00, risk_reduction=0.65, token_cost=0.05, sensitivity_risk=0.00, kind=WorkflowConvention, stable=0.85, utility=0.80, specificity=0.80, volatility=0.20, duplication=1.00", rationale: "Duplicate memories should be rejected even when explicit." }]
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- memory::quality::tests::explicit_does_not_accept_duplicate_memory stdout ----

thread 'memory::quality::tests::explicit_does_not_accept_duplicate_memory' (766632) panicked at src/memory/quality.rs:276:9:
assertion `left != right` failed
  left: Accepted
 right: Accepted

---- memory::quality::tests::explicit_does_not_accept_low_quality_note stdout ----

thread 'memory::quality::tests::explicit_does_not_accept_low_quality_note' (766633) panicked at src/memory/quality.rs:267:9:
assertion `left != right` failed
  left: Accepted
 right: Accepted


failures:
    memory::calibration::tests::built_in_calibration_samples_pass
    memory::quality::tests::explicit_does_not_accept_duplicate_memory
    memory::quality::tests::explicit_does_not_accept_low_quality_note

test result: FAILED. 213 passed; 3 failed; 0 ignored; 0 measured; 1859 filtered out; finished in 155.57s

error: test failed, to rerun pass `--lib`
[exit status: 101]

$ ! rg 'assess_memory_candidate\(content, category, &existing, true\)' src/tools/memory_tool/mod.rs
[exit status: 0]

$ ! rg 'let status = if explicit \|\| score >= 0\.65' src/memory/quality.rs
    let status = if explicit || score >= 0.65 { MemoryStatus::Accepted } else { write_decision.status };
[exit status: 1]

$ ! rg 'format!\("Saved: \{\}' src/tui/app.rs
                            format!("Saved: {}", save_content)
                            format!("Saved: {}", save_content)
[exit status: 1]

$ cargo test -q -- --test-threads=1
warning: function `format_memory_write_outcome` is never used
   --> src/tui/app.rs:315:4
    |
315 | fn format_memory_write_outcome(
    |    ^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |
    = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default


running 2075 tests
....................................................................................... 87/2075
....................................................................................... 174/2075
....................................................................................... 261/2075
....................................................................................... 348/2075
....................................................................................... 435/2075
....................................................................................... 522/2075
....................................................................................... 609/2075
....................................................................................... 696/2075
....................................................................................... 783/2075
....................................................................................... 870/2075
....................................................................................... 957/2075
....................................................................................... 1044/2075
....................................................................................... 1131/2075
.......................................................................... 1205/2075
memory::calibration::tests::built_in_calibration_samples_pass --- FAILED
....................................................................................... 1293/2075
......... 1302/2075
memory::quality::tests::explicit_does_not_accept_duplicate_memory --- FAILED
memory::quality::tests::explicit_does_not_accept_low_quality_note --- FAILED
....................................................................................... 1391/2075
....................................................................................... 1478/2075
....................................................................................... 1565/2075
....................................................................................... 1652/2075
....................................................................................... 1739/2075
....................................................................................... 1826/2075
....................................................................................... 1913/2075
....................................................................................... 2000/2075
...........................................................................
failures:

---- memory::calibration::tests::built_in_calibration_samples_pass stdout ----

thread 'memory::calibration::tests::built_in_calibration_samples_pass' (774091) panicked at src/memory/calibration.rs:197:9:
failed calibration samples: [MemoryCalibrationResult { id: "explicit_low_value_note", expected: NotAccepted, actual: Accepted, score: Some(0.56542045), passed: false, reason: "write_score=0.57, status=Proposed, relevance=0.65, reuse=0.65, stability=0.33, trust=0.70, novelty=1.00, risk_reduction=0.25, token_cost=0.05, sensitivity_risk=0.00, kind=Note, stable=0.65, utility=0.65, specificity=0.55, volatility=0.70, duplication=0.00", rationale: "Explicit save can lower friction but must not bypass quality gates." }, MemoryCalibrationResult { id: "duplicate_project_fact", expected: Rejected, actual: Accepted, score: Some(0.65165913), passed: false, reason: "write_score=0.65, status=Rejected, relevance=0.85, reuse=0.80, stability=0.76, trust=0.72, novelty=0.00, risk_reduction=0.65, token_cost=0.05, sensitivity_risk=0.00, kind=WorkflowConvention, stable=0.85, utility=0.80, specificity=0.80, volatility=0.20, duplication=1.00", rationale: "Duplicate memories should be rejected even when explicit." }]
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- memory::quality::tests::explicit_does_not_accept_duplicate_memory stdout ----

thread 'memory::quality::tests::explicit_does_not_accept_duplicate_memory' (774191) panicked at src/memory/quality.rs:276:9:
assertion `left != right` failed
  left: Accepted
 right: Accepted

---- memory::quality::tests::explicit_does_not_accept_low_quality_note stdout ----

thread 'memory::quality::tests::explicit_does_not_accept_low_quality_note' (774192) panicked at src/memory/quality.rs:267:9:
assertion `left != right` failed
  left: Accepted
 right: Accepted


failures:
    memory::calibration::tests::built_in_calibration_samples_pass
    memory::quality::tests::explicit_does_not_accept_duplicate_memory
    memory::quality::tests::explicit_does_not_accept_low_quality_note

test result: FAILED. 2072 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 541.00s

error: test failed, to rerun pass `--lib`
[exit status: 101]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-behavior-repair-memory-save-20260528-145633/memory-save-quality-gate/agent-output.md`
- Events: `docs/benchmarks/live-behavior-repair-memory-save-20260528-145633/memory-save-quality-gate/agent-events.jsonl`
- Monitor: `docs/benchmarks/live-behavior-repair-memory-save-20260528-145633/memory-save-quality-gate/agent-monitor.log`

Event counts:

```text
complete: 1
eval_started: 1
runtime_diagnostic: 1
start: 1
text_chunk: 2
tool_execution_complete: 9
tool_execution_progress: 3
tool_execution_start: 9
trace_summary: 1
```

Quality signals:

```text
output_chars: 2049
diff_chars: 0
diff_files_changed: 0
diff_files_changed_raw: 0
generated_dependency_files_ignored: 0
tool_executions: 9
first_write_tool_index: 7
forbidden_tool_uses: none
tool_errors: 3
tool_failures: 3
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 103
test_status: failed
verification_passed: false
stage_validation_passed: false
acceptance_accepted: None
closeout_status: failed
closeout_tool_records: 18
closeout_tool_evidence: tool evidence: records=18 completed=6 failed=12 denied=0 validation=0 closeout=0 repair=12 changed=0 workflows=code_change commands=none
runtime_diet: prompt=9511 tool_schema=3950 tools=19 workflow=strict closeout=full validation=failed:3/4
adaptive_triggers: risk_signal_high,required_validation
risk_signal: entry=high runtime=none
risk_signal_reasons: route risk is high; required validation commands present; complex required-validation surface; broad validation command requested; runtime risk keyword in request: memory
trace_event_types: tool.start,tool.observation,tool.done,stop.check,agent.loop,closeout,execution.report,memory.proposal,memory.proposal,runtime.diet,completion.contract,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: true
eval_intent: seeded_code_change
behavior_assertions: memory_quality_gate,memory_save_outcome_visibility
behavior_assertion_status: failed
output_assertions: none
output_assertion_status: none
output_assertion_missing: none
trajectory_assertions: none
trajectory_assertion_status: none
trajectory_assertion_missing: none
runtime_spine: coverage=6/7, status=none, missing=none
runtime_profile: none
mva_profile_active: false
runtime_spine_detail: context=15 latest=runtime_diet_report decision=26 latest=action_reviewed permission=0 latest=none tool_execution=20 latest=tool_completed state_update=26 latest=agent_loop_step_evaluated verification=1 latest=reflection_pass_completed closeout=3 latest=assistant_responded risky_tool_runs=3 risky_tool_reviewed=3 risky_tool_missing_action_review=none gate_outcomes=total=10, protective_block=1, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=9 stop_reason=focused_repair_stalled stop_terminal_status=failed stop_action=recover stop_failure_type=focused_repair_stalled rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none route_recovery=events=0, read_search=false, mutation_blocked=false, safety=missing action_scores=9 latest_action_score=17 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=none runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=2 provider_protocol_repairs=11 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=6 context_zone_duplicate_blocks_removed=0 context_zone_provenance_markers=10 agent_loop_steps=4 context_zones=2 completion_contract=failed
runtime_spine_trace_present: true
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: none
runtime_spine_status: none
runtime_spine_missing: none
risky_tool_runs: 3
risky_tool_reviewed: 3
risky_tool_missing_action_review: none
gate_outcomes: total=10, protective_block=1, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=9
gate_outcome_records: action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,closeout:failed:protective_block
gate_outcome_total: 10
gate_outcome_protective_blocks: 1
gate_outcome_recoverable_friction: 0
gate_outcome_unrecovered_blocks: 0
gate_outcome_suspected_false_positives: 0
gate_outcome_policy_correct_but_ux_costly: 0
gate_outcome_harmless_passes: 9
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
context_zone_source_messages: 6
context_zone_duplicate_blocks_removed: 0
context_zone_provenance_markers: 10
state_transition_recorded: false
completion_contract_status: failed
completion_contract_proof_status: failed
candidate_score_calibrated: false
candidate_score_disagreement: false
observer_outcome_recorded: true
memory_boundary_recorded: true
verification_proof_status: failed
verification_proof_summary: required validation failed 3/4 commands
verification_proof_kinds: none
verification_proof_support_status: failed
verification_proof_support_summary: verification proof status failed blocks verified closeout before proof-kind policy
verification_proof_supports_verified: false
verification_proof_residual_risk: true
premature_edit_count: 0
evidence_before_first_edit: true
scope_drift_count: 0
invalid_action_count: 1
repeated_action_count: 1
failed_action_count: 6
user_question_count: 0
unnecessary_question_count: 0
verification_attempted: true
verification_passed: false
tool_call_count: 9
llm_call_count: 2
warning: no_code_diff
warning: tool_errors_seen
warning: patch_synthesis_no_change
warning: required_commands_not_passing
warning: behavior_assertions_not_passing
warning: closeout_not_successful
failure_owner: llm_reasoning
outcome_score: 0
process_score: 87
efficiency_score: 68
agent_score: 40
score_penalties: run_failed,required_commands_failed,verification_failed,closeout_not_successful,behavior_assertions_failed,expected_code_diff_missing,repeated_action,invalid_action,failed_actions,repeated_actions
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
runtime_spine: coverage=6/7, status=none, missing=none
runtime_profile: none
mva_profile_active: false
runtime_spine_detail: context=15 latest=runtime_diet_report decision=26 latest=action_reviewed permission=0 latest=none tool_execution=20 latest=tool_completed state_update=26 latest=agent_loop_step_evaluated verification=1 latest=reflection_pass_completed closeout=3 latest=assistant_responded risky_tool_runs=3 risky_tool_reviewed=3 risky_tool_missing_action_review=none gate_outcomes=total=10, protective_block=1, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=9 stop_reason=focused_repair_stalled stop_terminal_status=failed stop_action=recover stop_failure_type=focused_repair_stalled rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none route_recovery=events=0, read_search=false, mutation_blocked=false, safety=missing action_scores=9 latest_action_score=17 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=none runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=2 provider_protocol_repairs=11 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=6 context_zone_duplicate_blocks_removed=0 context_zone_provenance_markers=10 agent_loop_steps=4 context_zones=2 completion_contract=failed
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: none
runtime_spine_status: none
runtime_spine_missing: none
risky_tool_runs: 3
risky_tool_reviewed: 3
risky_tool_missing_action_review: none
gate_outcomes: total=10, protective_block=1, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=9
gate_outcome_records: action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,closeout:failed:protective_block
gate_outcome_total: 10
gate_outcome_protective_blocks: 1
gate_outcome_recoverable_friction: 0
gate_outcome_unrecovered_blocks: 0
gate_outcome_suspected_false_positives: 0
gate_outcome_policy_correct_but_ux_costly: 0
gate_outcome_harmless_passes: 9
gate_outcome_failure_owners: none
agent_loop_steps: 4
context_zones_materialized: true
context_zone_task_state_empty: false
context_zone_current_decision_request_empty: false
context_zone_envelope_messages: 1
context_zone_source_messages: 6
context_zone_duplicate_blocks_removed: 0
context_zone_provenance_markers: 10
state_transition_recorded: false
completion_contract_status: failed
completion_contract_proof_status: failed
candidate_score_calibrated: false
candidate_score_disagreement: false
observer_outcome_recorded: true
memory_boundary_recorded: true
verification_proof_status: failed
verification_proof_summary: required validation failed 3/4 commands
verification_proof_kinds: none
verification_proof_support_status: failed
verification_proof_support_summary: verification proof status failed blocks verified closeout before proof-kind policy
verification_proof_supports_verified: false
verification_proof_residual_risk: true
risk_signal: entry=high runtime=none
risk_signal_reasons: route risk is high; required validation commands present; complex required-validation surface; broad validation command requested; runtime risk keyword in request: memory
memory_sync_events: 0
memory_tool_calls: 0
retrieval_sources: Project,Session,Memory
memory_candidate_typed: true
memory_candidate_has_evidence: true
memory_proposal_recorded: true
memory_proposal_status: proposed
memory_proposal_candidates: 3
memory_proposal_kinds: next_step
memory_proposal_evidence_items: 9
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
required_command_status: failed
validation_events: 0
stage_validation_events: 0
tool_progress_events: 3
guided_debugging_events: 0
guided_reasoning_events: 0
workflow_plan_events: 1
weighted_plan_events: 1
reweighted_plan_events: 0
adaptive_trigger_events: 2
adaptive_triggers: risk_signal_high,required_validation
latest_top_priority: P2
latest_top_importance_score: 0.40625
latest_top_weight_share: 0.24904213845729828
acceptance_accepted: missing
closeout_status: failed
closeout_tool_records: 18
closeout_tool_evidence: tool evidence: records=18 completed=6 failed=12 denied=0 validation=0 closeout=0 repair=12 changed=0 workflows=code_change commands=none
runtime_diet: prompt=9511 tool_schema=3950 tools=19 workflow=strict
attention: required commands did not pass in the harness
note: guided debugging is expected only after a blocker or failed validation
```

Agent stderr tail:

```text
[required validation still running after 30s] cargo test -q memory -- --test-threads=1
[required validation still running after 60s] cargo test -q memory -- --test-threads=1
[required validation still running after 90s] cargo test -q memory -- --test-threads=1
[required validation still running after 120s] cargo test -q memory -- --test-threads=1
[required validation still running after 150s] cargo test -q memory -- --test-threads=1
[required validation still running after 180s] cargo test -q memory -- --test-threads=1
```

Agent monitor tail:

```text
[2026-05-28T15:00:02+0800] agent-run still running elapsed=30s idle_for=25s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=336
[2026-05-28T15:00:32+0800] agent-run still running elapsed=60s idle_for=20s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=15505
[2026-05-28T15:01:02+0800] agent-run still running elapsed=90s idle_for=20s stdout_bytes=0 stderr_bytes=87 output_bytes=0 events_bytes=15505
[2026-05-28T15:01:32+0800] agent-run still running elapsed=120s idle_for=20s stdout_bytes=0 stderr_bytes=174 output_bytes=0 events_bytes=15505
[2026-05-28T15:02:02+0800] agent-run still running elapsed=150s idle_for=20s stdout_bytes=0 stderr_bytes=261 output_bytes=0 events_bytes=15505
[2026-05-28T15:02:32+0800] agent-run still running elapsed=180s idle_for=20s stdout_bytes=0 stderr_bytes=349 output_bytes=0 events_bytes=15505
[2026-05-28T15:03:02+0800] agent-run still running elapsed=210s idle_for=20s stdout_bytes=0 stderr_bytes=437 output_bytes=0 events_bytes=15505
[2026-05-28T15:03:32+0800] agent-run still running elapsed=240s idle_for=20s stdout_bytes=0 stderr_bytes=525 output_bytes=0 events_bytes=15505
[2026-05-28T15:04:02+0800] agent-run still running elapsed=270s idle_for=50s stdout_bytes=0 stderr_bytes=525 output_bytes=0 events_bytes=15505
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

- Bundle: `docs/benchmarks/live-behavior-repair-memory-save-20260528-145633/memory-save-quality-gate/run-bundle`
- Task: `docs/benchmarks/live-behavior-repair-memory-save-20260528-145633/memory-save-quality-gate/run-bundle/task.json`
- Steps: `docs/benchmarks/live-behavior-repair-memory-save-20260528-145633/memory-save-quality-gate/run-bundle/steps.jsonl`
- Events: `docs/benchmarks/live-behavior-repair-memory-save-20260528-145633/memory-save-quality-gate/run-bundle/events.jsonl`
- Final report: `docs/benchmarks/live-behavior-repair-memory-save-20260528-145633/memory-save-quality-gate/run-bundle/final_report.md`
