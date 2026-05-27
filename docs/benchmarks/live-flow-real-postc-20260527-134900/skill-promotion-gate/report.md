# Live Eval Report: skill-promotion-gate

- Run id: `flow-real-postc-20260527-134900`
- Sample: `evalsets/live_tasks/skill-promotion-gate.yaml`
- Worktree: `target/live-evals/flow-real-postc-20260527-134900/skill-promotion-gate/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/flow-real-postc-20260527-134900/skill-promotion-gate/env`
- Test status: `failed`
- Generated: `2026-05-27 14:00:02 +0800`

## Git Status

```text
```

## Diff Stat

```text
```

## Required Commands

```text
$ cargo test -q skill_evolution -- --test-threads=1
warning: fields `total`, `passed`, and `failed` are never read
    --> src/tui/slash_handler/learning.rs:1498:5
     |
1495 | struct BoundSkillEvalReport {
     |        -------------------- fields in this struct
...
1498 |     total: usize,
     |     ^^^^^
1499 |     passed: usize,
     |     ^^^^^^
1500 |     failed: usize,
     |     ^^^^^^
     |
     = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: function `validate_skill_promotion_for_apply` is never used
    --> src/tui/slash_handler/learning.rs:1546:4
     |
1546 | fn validate_skill_promotion_for_apply(
     |    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: function `skill_fitness_from_bound_eval` is never used
    --> src/tui/slash_handler/learning.rs:1619:4
     |
1619 | fn skill_fitness_from_bound_eval(
     |    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: function `estimate_skill_semantic_drift` is never used
    --> src/tui/slash_handler/learning.rs:1649:4
     |
1649 | fn estimate_skill_semantic_drift(proposal: &crate::engine::skill_evolution::SkillProposal) -> f32 {
     |    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^


running 9 tests
.........
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 1964 filtered out; finished in 0.00s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.00s

[exit status: 0]

$ cargo test -q slash_handler -- --test-threads=1
warning: fields `total`, `passed`, and `failed` are never read
    --> src/tui/slash_handler/learning.rs:1498:5
     |
1495 | struct BoundSkillEvalReport {
     |        -------------------- fields in this struct
...
1498 |     total: usize,
     |     ^^^^^
1499 |     passed: usize,
     |     ^^^^^^
1500 |     failed: usize,
     |     ^^^^^^
     |
     = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: function `validate_skill_promotion_for_apply` is never used
    --> src/tui/slash_handler/learning.rs:1546:4
     |
1546 | fn validate_skill_promotion_for_apply(
     |    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: function `skill_fitness_from_bound_eval` is never used
    --> src/tui/slash_handler/learning.rs:1619:4
     |
1619 | fn skill_fitness_from_bound_eval(
     |    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: function `estimate_skill_semantic_drift` is never used
    --> src/tui/slash_handler/learning.rs:1649:4
     |
1649 | fn estimate_skill_semantic_drift(proposal: &crate::engine::skill_evolution::SkillProposal) -> f32 {
     |    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^


running 57 tests
.........................................................
test result: ok. 57 passed; 0 failed; 0 ignored; 0 measured; 1916 filtered out; finished in 0.11s


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
warning: fields `total`, `passed`, and `failed` are never read
    --> src/tui/slash_handler/learning.rs:1498:5
     |
1495 | struct BoundSkillEvalReport {
     |        -------------------- fields in this struct
...
1498 |     total: usize,
     |     ^^^^^
1499 |     passed: usize,
     |     ^^^^^^
1500 |     failed: usize,
     |     ^^^^^^
     |
     = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: function `validate_skill_promotion_for_apply` is never used
    --> src/tui/slash_handler/learning.rs:1546:4
     |
1546 | fn validate_skill_promotion_for_apply(
     |    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: function `skill_fitness_from_bound_eval` is never used
    --> src/tui/slash_handler/learning.rs:1619:4
     |
1619 | fn skill_fitness_from_bound_eval(
     |    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: function `estimate_skill_semantic_drift` is never used
    --> src/tui/slash_handler/learning.rs:1649:4
     |
1649 | fn estimate_skill_semantic_drift(proposal: &crate::engine::skill_evolution::SkillProposal) -> f32 {
     |    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^


running 1973 tests
....................................................................................... 87/1973
....................................................................................... 174/1973
....................................................................................... 261/1973
....................................................................................... 348/1973
....................................................................................... 435/1973
....................................................................................... 522/1973
....................................................................................... 609/1973
....................................................................................... 696/1973
....................................................................................... 783/1973
....................................................................................... 870/1973
....................................................................................... 957/1973
....................................................................................... 1044/1973
....................................................................................... 1131/1973
....................................................................................... 1218/1973
....................................................................................... 1305/1973
....................................................................................... 1392/1973
....................................................................................... 1479/1973
....................................................................................... 1566/1973
....................................................................................... 1653/1973
....................................................................................... 1740/1973
....................................................................................... 1827/1973
....................................................................................... 1914/1973
...........................................................
test result: ok. 1973 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.83s


running 3 tests
...
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-flow-real-postc-20260527-134900/skill-promotion-gate/agent-output.md`
- Events: `docs/benchmarks/live-flow-real-postc-20260527-134900/skill-promotion-gate/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
runtime_diagnostic: 1
start: 1
text_chunk: 2
tool_execution_complete: 8
tool_execution_progress: 2
tool_execution_start: 8
trace_summary: 1
```

Quality signals:

```text
output_chars: 3149
diff_chars: 0
diff_files_changed: 0
tool_executions: 8
first_write_tool_index: 7
forbidden_tool_uses: none
tool_errors: 2
tool_failures: 2
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 86
test_status: failed
verification_passed: false
stage_validation_passed: false
acceptance_accepted: None
closeout_status: failed
closeout_tool_records: 17
closeout_tool_evidence: tool evidence: records=17 completed=6 failed=11 denied=0 validation=0 closeout=0 repair=11 changed=0 workflows=code_change commands=none
runtime_diet: prompt=8088 tool_schema=3950 tools=19 workflow=guarded closeout=full validation=failed:2/5
adaptive_triggers: risk_signal_high,required_validation
risk_signal: entry=high runtime=none
risk_signal_reasons: required validation commands present; complex required-validation surface; broad validation command requested
trace_event_types: action.review,tool.start,tool.observation,tool.done,stop.check,agent.loop,closeout,execution.report,memory.proposal,runtime.diet,completion.contract,assistant
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
runtime_spine_detail: context=14 latest=runtime_diet_report decision=21 latest=action_reviewed permission=0 latest=none tool_execution=18 latest=tool_completed state_update=24 latest=agent_loop_step_evaluated verification=1 latest=reflection_pass_completed closeout=3 latest=assistant_responded risky_tool_runs=2 risky_tool_reviewed=2 risky_tool_missing_action_review=none gate_outcomes=total=9, protective_block=1, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=8 stop_reason=focused_repair_stalled stop_terminal_status=failed stop_action=recover stop_failure_type=focused_repair_stalled rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none route_recovery=events=0, read_search=false, mutation_blocked=false, safety=missing action_scores=5 latest_action_score=17 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=none runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=2 provider_protocol_repairs=8 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=4 context_zone_duplicate_blocks_removed=0 context_zone_provenance_markers=4 agent_loop_steps=4 context_zones=2 completion_contract=failed
runtime_spine_trace_present: true
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: none
runtime_spine_status: none
runtime_spine_missing: none
risky_tool_runs: 2
risky_tool_reviewed: 2
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
context_zone_source_messages: 4
context_zone_duplicate_blocks_removed: 0
context_zone_provenance_markers: 4
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
invalid_action_count: 3
repeated_action_count: 3
failed_action_count: 4
user_question_count: 0
unnecessary_question_count: 0
verification_attempted: true
verification_passed: false
tool_call_count: 8
llm_call_count: 2
warning: no_code_diff
warning: tool_errors_seen
warning: patch_synthesis_no_change
warning: required_commands_not_passing
warning: behavior_assertions_not_passing
warning: closeout_not_successful
failure_owner: llm_reasoning
outcome_score: 0
process_score: 65
efficiency_score: 55
agent_score: 30
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
runtime_spine_detail: context=14 latest=runtime_diet_report decision=21 latest=action_reviewed permission=0 latest=none tool_execution=18 latest=tool_completed state_update=24 latest=agent_loop_step_evaluated verification=1 latest=reflection_pass_completed closeout=3 latest=assistant_responded risky_tool_runs=2 risky_tool_reviewed=2 risky_tool_missing_action_review=none gate_outcomes=total=9, protective_block=1, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=8 stop_reason=focused_repair_stalled stop_terminal_status=failed stop_action=recover stop_failure_type=focused_repair_stalled rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none route_recovery=events=0, read_search=false, mutation_blocked=false, safety=missing action_scores=5 latest_action_score=17 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=none runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=2 provider_protocol_repairs=8 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=4 context_zone_duplicate_blocks_removed=0 context_zone_provenance_markers=4 agent_loop_steps=4 context_zones=2 completion_contract=failed
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: none
runtime_spine_status: none
runtime_spine_missing: none
risky_tool_runs: 2
risky_tool_reviewed: 2
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
agent_loop_steps: 4
context_zones_materialized: true
context_zone_task_state_empty: false
context_zone_current_decision_request_empty: false
context_zone_envelope_messages: 1
context_zone_source_messages: 4
context_zone_duplicate_blocks_removed: 0
context_zone_provenance_markers: 4
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
memory_sync_events: 1
memory_tool_calls: 0
retrieval_sources: Project,Session
memory_candidate_typed: true
memory_candidate_has_evidence: true
memory_proposal_recorded: true
memory_proposal_status: proposed
memory_proposal_candidates: 1
memory_proposal_kinds: failure_pattern
memory_proposal_evidence_items: 11
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
guided_reasoning_events: 0
workflow_plan_events: 1
weighted_plan_events: 1
reweighted_plan_events: 0
adaptive_trigger_events: 2
adaptive_triggers: risk_signal_high,required_validation
latest_top_priority: P1
latest_top_importance_score: 0.7150000333786011
latest_top_weight_share: 0.19324323534965515
acceptance_accepted: missing
closeout_status: failed
closeout_tool_records: 17
closeout_tool_evidence: tool evidence: records=17 completed=6 failed=11 denied=0 validation=0 closeout=0 repair=11 changed=0 workflows=code_change commands=none
runtime_diet: prompt=8088 tool_schema=3950 tools=19 workflow=guarded
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

- Bundle: `docs/benchmarks/live-flow-real-postc-20260527-134900/skill-promotion-gate/run-bundle`
- Task: `docs/benchmarks/live-flow-real-postc-20260527-134900/skill-promotion-gate/run-bundle/task.json`
- Steps: `docs/benchmarks/live-flow-real-postc-20260527-134900/skill-promotion-gate/run-bundle/steps.jsonl`
- Events: `docs/benchmarks/live-flow-real-postc-20260527-134900/skill-promotion-gate/run-bundle/events.jsonl`
- Final report: `docs/benchmarks/live-flow-real-postc-20260527-134900/skill-promotion-gate/run-bundle/final_report.md`
