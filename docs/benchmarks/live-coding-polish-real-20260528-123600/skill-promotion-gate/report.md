# Live Eval Report: skill-promotion-gate

- Run id: `coding-polish-real-20260528-123600`
- Sample: `evalsets/live_tasks/skill-promotion-gate.yaml`
- Worktree: `target/live-evals/coding-polish-real-20260528-123600/skill-promotion-gate/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/coding-polish-real-20260528-123600/skill-promotion-gate/env`
- Test status: `failed`
- Generated: `2026-05-28 13:43:19 +0800`

## Git Status

```text
```

## Diff Stat

```text
```

## Required Commands

```text
$ cargo test -q skill_evolution -- --test-threads=1
warning: function `validate_skill_promotion_for_apply` is never used
    --> src/tui/slash_handler/learning.rs:2804:4
     |
2804 | fn validate_skill_promotion_for_apply(
     |    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
     |
     = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: function `skill_fitness_from_bound_eval` is never used
    --> src/tui/slash_handler/learning.rs:2877:4
     |
2877 | fn skill_fitness_from_bound_eval(
     |    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: function `estimate_skill_semantic_drift` is never used
    --> src/tui/slash_handler/learning.rs:2907:4
     |
2907 | fn estimate_skill_semantic_drift(proposal: &crate::engine::skill_evolution::SkillProposal) -> f32 {
     |    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^


running 9 tests
.........
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 2064 filtered out; finished in 0.00s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.00s

[exit status: 0]

$ cargo test -q slash_handler -- --test-threads=1
warning: function `validate_skill_promotion_for_apply` is never used
    --> src/tui/slash_handler/learning.rs:2804:4
     |
2804 | fn validate_skill_promotion_for_apply(
     |    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
     |
     = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: function `skill_fitness_from_bound_eval` is never used
    --> src/tui/slash_handler/learning.rs:2877:4
     |
2877 | fn skill_fitness_from_bound_eval(
     |    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: function `estimate_skill_semantic_drift` is never used
    --> src/tui/slash_handler/learning.rs:2907:4
     |
2907 | fn estimate_skill_semantic_drift(proposal: &crate::engine::skill_evolution::SkillProposal) -> f32 {
     |    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^


running 65 tests
.................................................................
test result: ok. 65 passed; 0 failed; 0 ignored; 0 measured; 2008 filtered out; finished in 0.17s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.00s

[exit status: 0]

$ python3 -c "import re; p='src/tui/slash_handler/learning.rs'; s=open(p).read(); h=s.find('pub fn handle_skill_proposals'); a=s.find('\"apply\" =>', h); b=s.find('let root = user_skill_root()', a); m=re.search(r'validate_skill_promotion_for_apply\\(\\s*&store,\\s*&current,\\s*bound_report\\.as_ref\\(\\)\\s*\\)', s[a:b]); assert h >= 0 and a >= 0 and b >= 0 and m"
Traceback (most recent call last):
  File "<string>", line 1, in <module>
AssertionError
[exit status: 1]

$ python3 -c "p='src/tui/slash_handler/learning.rs'; s=open(p).read(); h=s.find('pub fn handle_skill_proposals'); a=s.find('\"apply\" =>', h); r=s.find('store.record_applied_version(id, &path)', a); b=s.find('let loaded = app.skill_runtime.reload()', r); c=s.find('record_evolution_update(', r); assert h >= 0 and a >= 0 and r >= 0 and b >= 0 and c >= 0 and c < b"
Traceback (most recent call last):
  File "<string>", line 1, in <module>
AssertionError
[exit status: 1]

$ cargo test -q -- --test-threads=1
warning: function `validate_skill_promotion_for_apply` is never used
    --> src/tui/slash_handler/learning.rs:2804:4
     |
2804 | fn validate_skill_promotion_for_apply(
     |    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
     |
     = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: function `skill_fitness_from_bound_eval` is never used
    --> src/tui/slash_handler/learning.rs:2877:4
     |
2877 | fn skill_fitness_from_bound_eval(
     |    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: function `estimate_skill_semantic_drift` is never used
    --> src/tui/slash_handler/learning.rs:2907:4
     |
2907 | fn estimate_skill_semantic_drift(proposal: &crate::engine::skill_evolution::SkillProposal) -> f32 {
     |    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^


running 2073 tests
....................................................................................... 87/2073
....................................................................................... 174/2073
....................................................................................... 261/2073
....................................................................................... 348/2073
....................................................................................... 435/2073
....................................................................................... 522/2073
....................................................................................... 609/2073
....................................................................................... 696/2073
....................................................................................... 783/2073
....................................................................................... 870/2073
....................................................................................... 957/2073
....................................................................................... 1044/2073
....................................................................................... 1131/2073
....................................................................................... 1218/2073
....................................................................................... 1305/2073
....................................................................................... 1392/2073
....................................................................................... 1479/2073
....................................................................................... 1566/2073
....................................................................................... 1653/2073
....................................................................................... 1740/2073
....................................................................................... 1827/2073
....................................................................................... 1914/2073
....................................................................................... 2001/2073
........................................................................
test result: ok. 2073 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 490.49s


running 3 tests
...
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-coding-polish-real-20260528-123600/skill-promotion-gate/agent-output.md`
- Events: `docs/benchmarks/live-coding-polish-real-20260528-123600/skill-promotion-gate/agent-events.jsonl`
- Monitor: `docs/benchmarks/live-coding-polish-real-20260528-123600/skill-promotion-gate/agent-monitor.log`

Event counts:

```text
complete: 1
eval_started: 1
runtime_diagnostic: 1
start: 1
text_chunk: 2
tool_execution_complete: 7
tool_execution_progress: 2
tool_execution_start: 7
trace_summary: 1
```

Quality signals:

```text
output_chars: 2879
diff_chars: 0
diff_files_changed: 0
diff_files_changed_raw: 0
generated_dependency_files_ignored: 0
tool_executions: 7
first_write_tool_index: 6
forbidden_tool_uses: none
tool_errors: 2
tool_failures: 2
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 88
test_status: failed
verification_passed: false
stage_validation_passed: false
acceptance_accepted: None
closeout_status: failed
closeout_tool_records: 15
closeout_tool_evidence: tool evidence: records=15 completed=5 failed=10 denied=0 validation=0 closeout=0 repair=10 changed=0 workflows=code_change commands=none
runtime_diet: prompt=8458 tool_schema=3950 tools=19 workflow=guarded closeout=full validation=failed:2/5
adaptive_triggers: risk_signal_high,required_validation
risk_signal: entry=high runtime=none
risk_signal_reasons: required validation commands present; complex required-validation surface; broad validation command requested
trace_event_types: tool.start,tool.observation,tool.done,stop.check,agent.loop,closeout,execution.report,memory.proposal,memory.proposal,runtime.diet,completion.contract,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: true
eval_intent: seeded_code_change
behavior_assertions: skill_promotion_gate,skill_evolution_cooldown
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
runtime_spine_detail: context=15 latest=runtime_diet_report decision=19 latest=action_reviewed permission=0 latest=none tool_execution=16 latest=tool_completed state_update=23 latest=agent_loop_step_evaluated verification=1 latest=reflection_pass_completed closeout=3 latest=assistant_responded risky_tool_runs=2 risky_tool_reviewed=2 risky_tool_missing_action_review=none gate_outcomes=total=8, protective_block=1, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=7 stop_reason=focused_repair_stalled stop_terminal_status=failed stop_action=recover stop_failure_type=focused_repair_stalled rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none route_recovery=events=0, read_search=false, mutation_blocked=false, safety=missing action_scores=4 latest_action_score=17 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=none runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=2 provider_protocol_repairs=8 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=5 context_zone_duplicate_blocks_removed=0 context_zone_provenance_markers=10 agent_loop_steps=4 context_zones=2 completion_contract=failed
runtime_spine_trace_present: true
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: none
runtime_spine_status: none
runtime_spine_missing: none
risky_tool_runs: 2
risky_tool_reviewed: 2
risky_tool_missing_action_review: none
gate_outcomes: total=8, protective_block=1, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=7
gate_outcome_records: action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,closeout:failed:protective_block
gate_outcome_total: 8
gate_outcome_protective_blocks: 1
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
agent_loop_steps: 4
context_zones_materialized: true
context_zone_task_state_empty: false
context_zone_current_decision_request_empty: false
context_zone_envelope_messages: 1
context_zone_source_messages: 5
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
verification_proof_summary: required validation failed 2/5 commands
verification_proof_kinds: none
verification_proof_support_status: failed
verification_proof_support_summary: verification proof status failed blocks verified closeout before proof-kind policy
verification_proof_supports_verified: false
verification_proof_residual_risk: true
premature_edit_count: 0
evidence_before_first_edit: true
scope_drift_count: 0
invalid_action_count: 2
repeated_action_count: 2
failed_action_count: 4
user_question_count: 0
unnecessary_question_count: 0
verification_attempted: true
verification_passed: false
tool_call_count: 7
llm_call_count: 2
warning: no_code_diff
warning: tool_errors_seen
warning: patch_synthesis_no_change
warning: required_commands_not_passing
warning: behavior_assertions_not_passing
warning: closeout_not_successful
failure_owner: llm_reasoning
outcome_score: 0
process_score: 74
efficiency_score: 61
agent_score: 34
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
runtime_spine_detail: context=15 latest=runtime_diet_report decision=19 latest=action_reviewed permission=0 latest=none tool_execution=16 latest=tool_completed state_update=23 latest=agent_loop_step_evaluated verification=1 latest=reflection_pass_completed closeout=3 latest=assistant_responded risky_tool_runs=2 risky_tool_reviewed=2 risky_tool_missing_action_review=none gate_outcomes=total=8, protective_block=1, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=7 stop_reason=focused_repair_stalled stop_terminal_status=failed stop_action=recover stop_failure_type=focused_repair_stalled rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none route_recovery=events=0, read_search=false, mutation_blocked=false, safety=missing action_scores=4 latest_action_score=17 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=none runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=2 provider_protocol_repairs=8 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=5 context_zone_duplicate_blocks_removed=0 context_zone_provenance_markers=10 agent_loop_steps=4 context_zones=2 completion_contract=failed
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: none
runtime_spine_status: none
runtime_spine_missing: none
risky_tool_runs: 2
risky_tool_reviewed: 2
risky_tool_missing_action_review: none
gate_outcomes: total=8, protective_block=1, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=7
gate_outcome_records: action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,closeout:failed:protective_block
gate_outcome_total: 8
gate_outcome_protective_blocks: 1
gate_outcome_recoverable_friction: 0
gate_outcome_unrecovered_blocks: 0
gate_outcome_suspected_false_positives: 0
gate_outcome_policy_correct_but_ux_costly: 0
gate_outcome_harmless_passes: 7
gate_outcome_failure_owners: none
agent_loop_steps: 4
context_zones_materialized: true
context_zone_task_state_empty: false
context_zone_current_decision_request_empty: false
context_zone_envelope_messages: 1
context_zone_source_messages: 5
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
verification_proof_summary: required validation failed 2/5 commands
verification_proof_kinds: none
verification_proof_support_status: failed
verification_proof_support_summary: verification proof status failed blocks verified closeout before proof-kind policy
verification_proof_supports_verified: false
verification_proof_residual_risk: true
risk_signal: entry=high runtime=none
risk_signal_reasons: required validation commands present; complex required-validation surface; broad validation command requested
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
tool_progress_events: 2
guided_debugging_events: 0
guided_reasoning_events: 1
workflow_plan_events: 1
weighted_plan_events: 1
reweighted_plan_events: 0
adaptive_trigger_events: 2
adaptive_triggers: risk_signal_high,required_validation
latest_top_priority: P3
latest_top_importance_score: 0.20500001311302185
latest_top_weight_share: 0.1626984179019928
acceptance_accepted: missing
closeout_status: failed
closeout_tool_records: 15
closeout_tool_evidence: tool evidence: records=15 completed=5 failed=10 denied=0 validation=0 closeout=0 repair=10 changed=0 workflows=code_change commands=none
runtime_diet: prompt=8458 tool_schema=3950 tools=19 workflow=guarded
attention: required commands did not pass in the harness
note: guided debugging is expected only after a blocker or failed validation
```

Agent stderr tail:

```text
[required validation still running after 30s] cargo test -q skill_evolution -- --test-threads=1
[required validation still running after 60s] cargo test -q skill_evolution -- --test-threads=1
[required validation still running after 90s] cargo test -q skill_evolution -- --test-threads=1
[required validation still running after 120s] cargo test -q skill_evolution -- --test-threads=1
[required validation still running after 150s] cargo test -q skill_evolution -- --test-threads=1
```

Agent monitor tail:

```text
[2026-05-28T13:31:47+0800] agent-run still running elapsed=30s idle_for=10s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=12880
[2026-05-28T13:32:17+0800] agent-run still running elapsed=60s idle_for=5s stdout_bytes=0 stderr_bytes=96 output_bytes=0 events_bytes=12880
[2026-05-28T13:32:47+0800] agent-run still running elapsed=90s idle_for=5s stdout_bytes=0 stderr_bytes=192 output_bytes=0 events_bytes=12880
[2026-05-28T13:33:17+0800] agent-run still running elapsed=120s idle_for=5s stdout_bytes=0 stderr_bytes=288 output_bytes=0 events_bytes=12880
[2026-05-28T13:33:47+0800] agent-run still running elapsed=150s idle_for=5s stdout_bytes=0 stderr_bytes=385 output_bytes=0 events_bytes=12880
[2026-05-28T13:34:17+0800] agent-run still running elapsed=180s idle_for=10s stdout_bytes=0 stderr_bytes=482 output_bytes=0 events_bytes=12880
[2026-05-28T13:34:47+0800] agent-run still running elapsed=210s idle_for=40s stdout_bytes=0 stderr_bytes=482 output_bytes=0 events_bytes=12880
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

- Bundle: `docs/benchmarks/live-coding-polish-real-20260528-123600/skill-promotion-gate/run-bundle`
- Task: `docs/benchmarks/live-coding-polish-real-20260528-123600/skill-promotion-gate/run-bundle/task.json`
- Steps: `docs/benchmarks/live-coding-polish-real-20260528-123600/skill-promotion-gate/run-bundle/steps.jsonl`
- Events: `docs/benchmarks/live-coding-polish-real-20260528-123600/skill-promotion-gate/run-bundle/events.jsonl`
- Final report: `docs/benchmarks/live-coding-polish-real-20260528-123600/skill-promotion-gate/run-bundle/final_report.md`
