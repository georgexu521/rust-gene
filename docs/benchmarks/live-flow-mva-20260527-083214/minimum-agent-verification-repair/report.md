# Live Eval Report: minimum-agent-verification-repair

- Run id: `flow-mva-20260527-083214`
- Sample: `evalsets/live_tasks/minimum-agent-verification-repair.yaml`
- Worktree: `target/live-evals/flow-mva-20260527-083214/minimum-agent-verification-repair/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/flow-mva-20260527-083214/minimum-agent-verification-repair/env`
- Test status: `failed`
- Generated: `2026-05-27 08:42:59 +0800`

## Git Status

```text
```

## Diff Stat

```text
```

## Required Commands

```text
$ python3 fixtures/mva_verification_repair/test_slugify.py
F
======================================================================
FAIL: test_slugify_lowercase_hyphen (__main__.SlugifyTest.test_slugify_lowercase_hyphen)
----------------------------------------------------------------------
Traceback (most recent call last):
  File "/Users/georgexu/Desktop/rust-agent/target/live-evals/flow-mva-20260527-083214/minimum-agent-verification-repair/worktree/fixtures/mva_verification_repair/test_slugify.py", line 7, in test_slugify_lowercase_hyphen
    self.assertEqual(slugify.slugify(" Hello World "), "hello-world")
AssertionError: 'Hello_World' != 'hello-world'
- Hello_World
? ^    ^^
+ hello-world
? ^    ^^


----------------------------------------------------------------------
Ran 1 test in 0.000s

FAILED (failures=1)
[exit status: 1]

$ rg -F 'return value.strip().lower().replace(" ", "-")' fixtures/mva_verification_repair/slugify.py
[exit status: 1]

```

## Agent Run

- Exit status: `-15`
- Events: `docs/benchmarks/live-flow-mva-20260527-083214/minimum-agent-verification-repair/agent-events.jsonl`

Event counts:

```text
eval_started: 1
permission_request: 1
start: 1
tool_execution_complete: 1
tool_execution_progress: 1
tool_execution_start: 2
```

Quality signals:

```text
output_chars: 0
diff_chars: 0
diff_files_changed: 0
tool_executions: 1
first_write_tool_index: none
forbidden_tool_uses: none
tool_errors: 1
tool_failures: 0
has_closeout: false
has_validation_claim: false
trace_status: missing
trace_events: 0
test_status: failed
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
eval_intent: seeded_code_change
behavior_assertions: none
behavior_assertion_status: none
output_assertions: contains_any=2
output_assertion_status: failed
output_assertion_missing: contains_any:失败|failed|failing validation|initial failure;contains_any:验证|verified|verification proof|required command
trajectory_assertions: evidence_before_edit,requires_observer_outcome,requires_stop_check,max_scope_drift_count,max_premature_edit_count,requires_runtime_spine_passed
trajectory_assertion_status: failed
trajectory_assertion_missing: requires_observer_outcome;requires_stop_check;requires_runtime_spine_passed
runtime_spine: coverage=0/7, status=missing, missing=phase:context,phase:decision,phase:tool_execution,phase:state_update,phase:verification,phase:closeout,event:context_zones_materialized,event:action_decision_evaluated,event:action_reviewed,event:agent_loop_step_evaluated,event:stop_check_evaluated,event:completion_contract_evaluated,completion_status:completed,terminal_status:completed,verification_proof_status:verified,special:verification_proof
runtime_profile: minimum_viable_agent
mva_profile_active: true
runtime_spine_detail: context=0 latest=none decision=0 latest=none permission=0 latest=none tool_execution=0 latest=none state_update=0 latest=none verification=0 latest=none closeout=0 latest=none risky_tool_runs=1 risky_tool_reviewed=0 risky_tool_missing_action_review=bash:call_functio gate_outcomes=total=0, protective_block=0, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=0 stop_reason=missing stop_terminal_status=missing stop_action=missing stop_failure_type=missing rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none route_recovery=events=0, read_search=false, mutation_blocked=false, safety=missing action_scores=0 latest_action_score=none low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=false memory_modifier_applied=false observer_outcome_recorded=false observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=none runtime_diet_warnings=none provider_protocol_events=0 provider_protocol_repairs=0 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 memory_boundary_recorded=false task_contract_recorded=false context_pack_recorded=false execution_report_recorded=false memory_proposal_recorded=false context_zone_envelope_messages=0 context_zone_source_messages=0 context_zone_duplicate_blocks_removed=0 context_zone_provenance_markers=0 agent_loop_steps=0 context_zones=0 completion_contract=missing
runtime_spine_trace_present: false
runtime_spine_phase_coverage: 0/7
runtime_spine_observed_phases: none
runtime_spine_assertions: phase:context,phase:decision,phase:tool_execution,phase:state_update,phase:verification,phase:closeout,event:context_zones_materialized,event:action_decision_evaluated,event:action_reviewed,event:agent_loop_step_evaluated,event:stop_check_evaluated,event:completion_contract_evaluated,completion_status:completed,terminal_status:completed,verification_proof_status:verified,special:verification_proof
runtime_spine_status: missing
runtime_spine_missing: phase:context,phase:decision,phase:tool_execution,phase:state_update,phase:verification,phase:closeout,event:context_zones_materialized,event:action_decision_evaluated,event:action_reviewed,event:agent_loop_step_evaluated,event:stop_check_evaluated,event:completion_contract_evaluated,completion_status:completed,terminal_status:completed,verification_proof_status:verified,special:verification_proof
risky_tool_runs: 1
risky_tool_reviewed: 0
risky_tool_missing_action_review: bash:call_functio
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
invalid_action_count: 1
repeated_action_count: 0
failed_action_count: 1
user_question_count: 1
unnecessary_question_count: 0
verification_attempted: true
verification_passed: false
tool_call_count: 2
llm_call_count: 1
warning: empty_agent_output
warning: tool_run_without_closeout
warning: no_code_diff
warning: tool_errors_seen
warning: missing_trace_summary
warning: required_commands_not_passing
warning: output_assertions_not_passing
warning: trajectory_assertions_not_passing
warning: runtime_spine_assertions_not_passing
warning: closeout_not_successful
failure_owner: agent_flow
outcome_score: 0
process_score: 55
efficiency_score: 87
agent_score: 34
score_penalties: run_failed,required_commands_failed,verification_failed,closeout_not_successful,runtime_spine_failed,output_assertions_failed,trajectory_assertions_failed,expected_code_diff_missing,invalid_action,risky_tool_missing_review,runtime_spine_not_passing,observer_outcome_missing,stop_check_missing,failed_actions,user_questions
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
runtime_spine: coverage=0/7, status=missing, missing=phase:context,phase:decision,phase:tool_execution,phase:state_update,phase:verification,phase:closeout,event:context_zones_materialized,event:action_decision_evaluated,event:action_reviewed,event:agent_loop_step_evaluated,event:stop_check_evaluated,event:completion_contract_evaluated,completion_status:completed,terminal_status:completed,verification_proof_status:verified,special:verification_proof
runtime_profile: minimum_viable_agent
mva_profile_active: true
runtime_spine_detail: context=0 latest=none decision=0 latest=none permission=0 latest=none tool_execution=0 latest=none state_update=0 latest=none verification=0 latest=none closeout=0 latest=none risky_tool_runs=1 risky_tool_reviewed=0 risky_tool_missing_action_review=bash:call_functio gate_outcomes=total=0, protective_block=0, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=0 stop_reason=missing stop_terminal_status=missing stop_action=missing stop_failure_type=missing rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none route_recovery=events=0, read_search=false, mutation_blocked=false, safety=missing action_scores=0 latest_action_score=none low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=false memory_modifier_applied=false observer_outcome_recorded=false observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=none runtime_diet_warnings=none provider_protocol_events=0 provider_protocol_repairs=0 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 memory_boundary_recorded=false task_contract_recorded=false context_pack_recorded=false execution_report_recorded=false memory_proposal_recorded=false context_zone_envelope_messages=0 context_zone_source_messages=0 context_zone_duplicate_blocks_removed=0 context_zone_provenance_markers=0 agent_loop_steps=0 context_zones=0 completion_contract=missing
runtime_spine_phase_coverage: 0/7
runtime_spine_observed_phases: none
runtime_spine_assertions: phase:context,phase:decision,phase:tool_execution,phase:state_update,phase:verification,phase:closeout,event:context_zones_materialized,event:action_decision_evaluated,event:action_reviewed,event:agent_loop_step_evaluated,event:stop_check_evaluated,event:completion_contract_evaluated,completion_status:completed,terminal_status:completed,verification_proof_status:verified,special:verification_proof
runtime_spine_status: missing
runtime_spine_missing: phase:context,phase:decision,phase:tool_execution,phase:state_update,phase:verification,phase:closeout,event:context_zones_materialized,event:action_decision_evaluated,event:action_reviewed,event:agent_loop_step_evaluated,event:stop_check_evaluated,event:completion_contract_evaluated,completion_status:completed,terminal_status:completed,verification_proof_status:verified,special:verification_proof
risky_tool_runs: 1
risky_tool_reviewed: 0
risky_tool_missing_action_review: bash:call_functio
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
required_commands: 2
agent_required_commands: 2
harness_commands: 0
required_command_status: failed
validation_events: 0
stage_validation_events: 0
tool_progress_events: 1
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
attention: required commands did not pass in the harness
note: guided debugging is expected only after a blocker or failed validation
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

- Bundle: `docs/benchmarks/live-flow-mva-20260527-083214/minimum-agent-verification-repair/run-bundle`
- Task: `docs/benchmarks/live-flow-mva-20260527-083214/minimum-agent-verification-repair/run-bundle/task.json`
- Steps: `docs/benchmarks/live-flow-mva-20260527-083214/minimum-agent-verification-repair/run-bundle/steps.jsonl`
- Events: `docs/benchmarks/live-flow-mva-20260527-083214/minimum-agent-verification-repair/run-bundle/events.jsonl`
- Final report: `docs/benchmarks/live-flow-mva-20260527-083214/minimum-agent-verification-repair/run-bundle/final_report.md`
