# Live Eval Report: memory-failure-lesson-promotion

- Run id: `coding-polish-real-20260528-123600`
- Sample: `evalsets/live_tasks/memory-failure-lesson-promotion.yaml`
- Worktree: `target/live-evals/coding-polish-real-20260528-123600/memory-failure-lesson-promotion/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/coding-polish-real-20260528-123600/memory-failure-lesson-promotion/env`
- Test status: `failed`
- Generated: `2026-05-28 14:07:07 +0800`

## Git Status

```text
```

## Diff Stat

```text
```

## Required Commands

```text
$ cargo test -q turn_recording -- --test-threads=1
warning: function `promote_trace_candidate_memories` is never used
  --> src/engine/conversation_loop/turn_recording.rs:58:21
   |
58 | pub(super) async fn promote_trace_candidate_memories(
   |                     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default


running 4 tests
....
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 2069 filtered out; finished in 0.00s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.00s

[exit status: 0]

$ cargo test -q memory -- --test-threads=1
warning: function `promote_trace_candidate_memories` is never used
  --> src/engine/conversation_loop/turn_recording.rs:58:21
   |
58 | pub(super) async fn promote_trace_candidate_memories(
   |                     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default


running 216 tests
....................................................................................... 87/216
....................................................................................... 174/216
..........................................
test result: ok. 216 passed; 0 failed; 0 ignored; 0 measured; 1857 filtered out; finished in 155.75s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.00s

[exit status: 0]

$ python3 -c "p='src/engine/conversation_loop/session_processor.rs'; s=open(p).read(); assert 'promote_trace_candidate_memories' in s and 'finish_trace' in s"
Traceback (most recent call last):
  File "<string>", line 1, in <module>
AssertionError
[exit status: 1]

$ python3 -c "p='src/engine/conversation_loop/turn_recording.rs'; s=open(p).read(); assert 'MemoryWriteTarget::Topic(\"strategy-failures\"' in s and 'MemoryEvidenceKind::RuntimeObservation' in s and 'source_experience_ids.push' in s"
[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-coding-polish-real-20260528-123600/memory-failure-lesson-promotion/agent-output.md`
- Events: `docs/benchmarks/live-coding-polish-real-20260528-123600/memory-failure-lesson-promotion/agent-events.jsonl`
- Monitor: `docs/benchmarks/live-coding-polish-real-20260528-123600/memory-failure-lesson-promotion/agent-monitor.log`

Event counts:

```text
complete: 1
eval_started: 1
runtime_diagnostic: 1
start: 1
text_chunk: 2
tool_execution_complete: 8
tool_execution_progress: 1
tool_execution_start: 8
trace_summary: 1
```

Quality signals:

```text
output_chars: 2298
diff_chars: 0
diff_files_changed: 0
diff_files_changed_raw: 0
generated_dependency_files_ignored: 0
tool_executions: 8
first_write_tool_index: 8
forbidden_tool_uses: none
tool_errors: 1
tool_failures: 1
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 141
test_status: failed
verification_passed: false
stage_validation_passed: false
acceptance_accepted: None
closeout_status: failed
closeout_tool_records: 33
closeout_tool_evidence: tool evidence: records=33 completed=7 failed=26 denied=0 validation=0 closeout=0 repair=26 changed=0 workflows=code_change commands=none
runtime_diet: prompt=16178 tool_schema=3950 tools=19 workflow=strict closeout=full validation=failed:1/4
adaptive_triggers: risk_signal_high,required_validation,repeated_no_code_progress
risk_signal: entry=high runtime=none
risk_signal_reasons: route risk is high; required validation commands present; complex required-validation surface; broad validation command requested; runtime risk keyword in request: memory
trace_event_types: tool.start,tool.observation,tool.done,stop.check,agent.loop,closeout,execution.report,memory.proposal,memory.proposal,runtime.diet,completion.contract,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: true
eval_intent: seeded_code_change
behavior_assertions: memory_candidate_typed,memory_candidate_has_evidence,memory_failure_lesson_promoted,memory_scope_correct
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
runtime_spine_detail: context=33 latest=runtime_diet_report decision=25 latest=action_reviewed permission=0 latest=none tool_execution=21 latest=tool_completed state_update=44 latest=agent_loop_step_evaluated verification=1 latest=reflection_pass_completed closeout=3 latest=assistant_responded risky_tool_runs=1 risky_tool_reviewed=1 risky_tool_missing_action_review=none gate_outcomes=total=9, protective_block=1, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=8 stop_reason=focused_repair_stalled stop_terminal_status=failed stop_action=recover stop_failure_type=focused_repair_stalled rollback_recommended=false rollback_completed=false recovery_failure_types=code_change_no_diff_after_repeated_progress recovery_kinds=code_change_no_diff_replan route_recovery=events=1, read_search=false, mutation_blocked=false, safety=true action_scores=8 latest_action_score=17 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=none runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=5 provider_protocol_repairs=74 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=6 context_zone_duplicate_blocks_removed=0 context_zone_provenance_markers=9 agent_loop_steps=10 context_zones=5 completion_contract=failed
runtime_spine_trace_present: true
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: none
runtime_spine_status: none
runtime_spine_missing: none
risky_tool_runs: 1
risky_tool_reviewed: 1
risky_tool_missing_action_review: none
gate_outcomes: total=9, protective_block=1, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=8
gate_outcome_records: action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,closeout:failed:protective_block
gate_outcome_total: 9
gate_outcome_protective_blocks: 1
gate_outcome_recoverable_friction: 0
gate_outcome_unrecovered_blocks: 0
gate_outcome_suspected_false_positives: 0
gate_outcome_policy_correct_but_ux_costly: 0
gate_outcome_harmless_passes: 8
gate_outcome_failure_owners: none
route_recovery: events=1, read_search=false, mutation_blocked=false, safety=true
route_recovery_events: 1
route_recovery_failure_types: code_change_no_diff_after_repeated_progress
route_recovery_kinds: code_change_no_diff_replan
route_recovery_read_search_expanded: false
route_recovery_mutation_blocked: false
route_recovery_safety_monotonic: true
route_recovery_unsafe_mutation_expansion: false
agent_loop_steps: 10
context_zones_materialized: true
context_zone_task_state_empty: false
context_zone_current_decision_request_empty: false
context_zone_envelope_messages: 1
context_zone_source_messages: 6
context_zone_duplicate_blocks_removed: 0
context_zone_provenance_markers: 9
state_transition_recorded: false
completion_contract_status: failed
completion_contract_proof_status: failed
candidate_score_calibrated: false
candidate_score_disagreement: false
observer_outcome_recorded: true
memory_boundary_recorded: true
verification_proof_status: failed
verification_proof_summary: required validation failed 1/4 commands
verification_proof_kinds: none
verification_proof_support_status: failed
verification_proof_support_summary: verification proof status failed blocks verified closeout before proof-kind policy
verification_proof_supports_verified: false
verification_proof_residual_risk: true
premature_edit_count: 0
evidence_before_first_edit: true
scope_drift_count: 0
invalid_action_count: 3
repeated_action_count: 3
failed_action_count: 2
user_question_count: 0
unnecessary_question_count: 0
verification_attempted: true
verification_passed: false
tool_call_count: 8
llm_call_count: 5
warning: no_code_diff
warning: tool_errors_seen
warning: patch_synthesis_no_change
warning: required_commands_not_passing
warning: behavior_assertions_not_passing
warning: closeout_not_successful
failure_owner: llm_reasoning
outcome_score: 0
process_score: 65
efficiency_score: 64
agent_score: 32
score_penalties: run_failed,required_commands_failed,verification_failed,closeout_not_successful,behavior_assertions_failed,expected_code_diff_missing,repeated_action,invalid_action,failed_actions,repeated_actions
```

Specialty signals:

```text
memory_active: true
automation_active: true
guided_debugging_active: false
guided_reasoning_active: true
weighted_planning_active: true
closeout_active: true
adaptive_workflow_active: true
active_specialty_signals: 6/7
workflow_contract_activation: entry=active:force repair=not_needed
workflow_contract_events: 1
runtime_spine: coverage=6/7, status=none, missing=none
runtime_profile: none
mva_profile_active: false
runtime_spine_detail: context=33 latest=runtime_diet_report decision=25 latest=action_reviewed permission=0 latest=none tool_execution=21 latest=tool_completed state_update=44 latest=agent_loop_step_evaluated verification=1 latest=reflection_pass_completed closeout=3 latest=assistant_responded risky_tool_runs=1 risky_tool_reviewed=1 risky_tool_missing_action_review=none gate_outcomes=total=9, protective_block=1, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=8 stop_reason=focused_repair_stalled stop_terminal_status=failed stop_action=recover stop_failure_type=focused_repair_stalled rollback_recommended=false rollback_completed=false recovery_failure_types=code_change_no_diff_after_repeated_progress recovery_kinds=code_change_no_diff_replan route_recovery=events=1, read_search=false, mutation_blocked=false, safety=true action_scores=8 latest_action_score=17 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=none runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=5 provider_protocol_repairs=74 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=6 context_zone_duplicate_blocks_removed=0 context_zone_provenance_markers=9 agent_loop_steps=10 context_zones=5 completion_contract=failed
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: none
runtime_spine_status: none
runtime_spine_missing: none
risky_tool_runs: 1
risky_tool_reviewed: 1
risky_tool_missing_action_review: none
gate_outcomes: total=9, protective_block=1, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=8
gate_outcome_records: action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,closeout:failed:protective_block
gate_outcome_total: 9
gate_outcome_protective_blocks: 1
gate_outcome_recoverable_friction: 0
gate_outcome_unrecovered_blocks: 0
gate_outcome_suspected_false_positives: 0
gate_outcome_policy_correct_but_ux_costly: 0
gate_outcome_harmless_passes: 8
gate_outcome_failure_owners: none
agent_loop_steps: 10
context_zones_materialized: true
context_zone_task_state_empty: false
context_zone_current_decision_request_empty: false
context_zone_envelope_messages: 1
context_zone_source_messages: 6
context_zone_duplicate_blocks_removed: 0
context_zone_provenance_markers: 9
state_transition_recorded: false
completion_contract_status: failed
completion_contract_proof_status: failed
candidate_score_calibrated: false
candidate_score_disagreement: false
observer_outcome_recorded: true
memory_boundary_recorded: true
verification_proof_status: failed
verification_proof_summary: required validation failed 1/4 commands
verification_proof_kinds: none
verification_proof_support_status: failed
verification_proof_support_summary: verification proof status failed blocks verified closeout before proof-kind policy
verification_proof_supports_verified: false
verification_proof_residual_risk: true
risk_signal: entry=high runtime=none
risk_signal_reasons: route risk is high; required validation commands present; complex required-validation surface; broad validation command requested; runtime risk keyword in request: memory
memory_sync_events: 0
memory_tool_calls: 0
retrieval_sources: Project,Memory
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
memory_failure_lesson_promoted: true
memory_action_weight_changed: false
memory_stale_demoted: false
memory_scope_correct: false
required_commands: 4
agent_required_commands: 4
harness_commands: 0
required_command_status: failed
validation_events: 0
stage_validation_events: 0
tool_progress_events: 1
guided_debugging_events: 0
guided_reasoning_events: 1
workflow_plan_events: 1
weighted_plan_events: 1
reweighted_plan_events: 0
adaptive_trigger_events: 3
adaptive_triggers: risk_signal_high,required_validation,repeated_no_code_progress
latest_top_priority: P2
latest_top_importance_score: 0.40625
latest_top_weight_share: 0.3396029472351074
acceptance_accepted: missing
closeout_status: failed
closeout_tool_records: 33
closeout_tool_evidence: tool evidence: records=33 completed=7 failed=26 denied=0 validation=0 closeout=0 repair=26 changed=0 workflows=code_change commands=none
runtime_diet: prompt=16178 tool_schema=3950 tools=19 workflow=strict
attention: required commands did not pass in the harness
note: guided debugging is expected only after a blocker or failed validation
```

Agent stderr tail:

```text
[required validation still running after 30s] cargo test -q turn_recording -- --test-threads=1
[required validation still running after 60s] cargo test -q turn_recording -- --test-threads=1
[required validation still running after 90s] cargo test -q turn_recording -- --test-threads=1
[required validation still running after 120s] cargo test -q turn_recording -- --test-threads=1
[required validation still running after 150s] cargo test -q turn_recording -- --test-threads=1
[required validation still running after 180s] cargo test -q turn_recording -- --test-threads=1
[required validation still running after 210s] cargo test -q turn_recording -- --test-threads=1
[required validation still running after 240s] cargo test -q turn_recording -- --test-threads=1
```

Agent monitor tail:

```text
[2026-05-28T13:59:08+0800] agent-run still running elapsed=30s idle_for=5s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=13230
[2026-05-28T13:59:38+0800] agent-run still running elapsed=60s idle_for=5s stdout_bytes=0 stderr_bytes=95 output_bytes=0 events_bytes=13230
[2026-05-28T14:00:08+0800] agent-run still running elapsed=90s idle_for=5s stdout_bytes=0 stderr_bytes=190 output_bytes=0 events_bytes=13230
[2026-05-28T14:00:38+0800] agent-run still running elapsed=120s idle_for=5s stdout_bytes=0 stderr_bytes=285 output_bytes=0 events_bytes=13230
[2026-05-28T14:01:08+0800] agent-run still running elapsed=150s idle_for=5s stdout_bytes=0 stderr_bytes=381 output_bytes=0 events_bytes=13230
[2026-05-28T14:01:38+0800] agent-run still running elapsed=180s idle_for=5s stdout_bytes=0 stderr_bytes=477 output_bytes=0 events_bytes=13230
[2026-05-28T14:02:08+0800] agent-run still running elapsed=210s idle_for=5s stdout_bytes=0 stderr_bytes=573 output_bytes=0 events_bytes=13230
[2026-05-28T14:02:39+0800] agent-run still running elapsed=240s idle_for=5s stdout_bytes=0 stderr_bytes=669 output_bytes=0 events_bytes=13230
[2026-05-28T14:03:09+0800] agent-run still running elapsed=270s idle_for=5s stdout_bytes=0 stderr_bytes=765 output_bytes=0 events_bytes=13230
[2026-05-28T14:03:39+0800] agent-run still running elapsed=300s idle_for=0s stdout_bytes=0 stderr_bytes=765 output_bytes=0 events_bytes=23484
[2026-05-28T14:04:09+0800] agent-run still running elapsed=330s idle_for=10s stdout_bytes=0 stderr_bytes=765 output_bytes=0 events_bytes=27276
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

- Bundle: `docs/benchmarks/live-coding-polish-real-20260528-123600/memory-failure-lesson-promotion/run-bundle`
- Task: `docs/benchmarks/live-coding-polish-real-20260528-123600/memory-failure-lesson-promotion/run-bundle/task.json`
- Steps: `docs/benchmarks/live-coding-polish-real-20260528-123600/memory-failure-lesson-promotion/run-bundle/steps.jsonl`
- Events: `docs/benchmarks/live-coding-polish-real-20260528-123600/memory-failure-lesson-promotion/run-bundle/events.jsonl`
- Final report: `docs/benchmarks/live-coding-polish-real-20260528-123600/memory-failure-lesson-promotion/run-bundle/final_report.md`
