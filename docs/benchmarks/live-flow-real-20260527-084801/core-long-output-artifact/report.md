# Live Eval Report: core-long-output-artifact

- Run id: `flow-real-20260527-084801`
- Sample: `evalsets/live_tasks/core-long-output-artifact.yaml`
- Worktree: `target/live-evals/flow-real-20260527-084801/core-long-output-artifact/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/flow-real-20260527-084801/core-long-output-artifact/env`
- Test status: `failed`
- Generated: `2026-05-27 08:58:57 +0800`

## Git Status

```text
```

## Diff Stat

```text
```

## Required Commands

```text
$ test -s fixtures/core_quality/long_output/output.log
[exit status: 1]

$ rg 'line 0537 ERROR_ANCHOR payment retry budget exceeded' fixtures/core_quality/long_output/output.log
rg: fixtures/core_quality/long_output/output.log: IO error for operation on fixtures/core_quality/long_output/output.log: No such file or directory (os error 2)
[exit status: 2]

$ python3 -c 'from pathlib import Path; assert len(Path("fixtures/core_quality/long_output/output.log").read_text().splitlines()) == 800'
Traceback (most recent call last):
  File "<string>", line 1, in <module>
  File "/Library/Frameworks/Python.framework/Versions/3.12/lib/python3.12/pathlib.py", line 1027, in read_text
    with self.open(mode='r', encoding=encoding, errors=errors) as f:
         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  File "/Library/Frameworks/Python.framework/Versions/3.12/lib/python3.12/pathlib.py", line 1013, in open
    return io.open(self, mode, buffering, encoding, errors, newline)
           ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
FileNotFoundError: [Errno 2] No such file or directory: 'fixtures/core_quality/long_output/output.log'
[exit status: 1]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-flow-real-20260527-084801/core-long-output-artifact/agent-output.md`
- Events: `docs/benchmarks/live-flow-real-20260527-084801/core-long-output-artifact/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
runtime_diagnostic: 1
start: 1
text_chunk: 2
tool_execution_complete: 2
tool_execution_start: 2
trace_summary: 1
```

Quality signals:

```text
output_chars: 2005
diff_chars: 0
diff_files_changed: 0
tool_executions: 2
first_write_tool_index: none
forbidden_tool_uses: none
tool_errors: 0
tool_failures: 1
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 61
test_status: failed
verification_passed: false
stage_validation_passed: false
acceptance_accepted: None
closeout_status: not_verified
closeout_tool_records: 4
closeout_tool_evidence: tool evidence: records=4 completed=2 failed=2 denied=0 validation=0 closeout=0 repair=2 changed=0 workflows=code_change commands=ls -la fixtures/core_quality/long_output/ | python3 fixtures/core_quality/long_output/generate_log.py > fixture...
runtime_diet: prompt=4972 tool_schema=3950 tools=19 workflow=guarded closeout=full validation=not_run
adaptive_triggers: risk_signal_high,required_validation
risk_signal: entry=high runtime=high
risk_signal_reasons: required validation commands present; runtime risk keyword in request: runtime
trace_event_types: stop.check,agent.loop,stop.check,agent.loop,risk.signal,guided.debug,closeout,execution.report,memory.proposal,runtime.diet,completion.contract,assistant
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
runtime_spine: coverage=6/7, status=none, missing=none
runtime_profile: none
mva_profile_active: false
runtime_spine_detail: context=14 latest=runtime_diet_report decision=13 latest=risk_signal_assessed permission=0 latest=none tool_execution=8 latest=tool_completed state_update=16 latest=agent_loop_step_evaluated verification=2 latest=guided_debugging_completed closeout=3 latest=assistant_responded risky_tool_runs=2 risky_tool_reviewed=2 risky_tool_missing_action_review=none gate_outcomes=total=4, protective_block=2, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=2 stop_reason=no_issue stop_terminal_status=partial stop_action=continue stop_failure_type=unknown rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none route_recovery=events=0, read_search=false, mutation_blocked=false, safety=missing action_scores=1 latest_action_score=16 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=none runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=2 provider_protocol_repairs=4 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=4 context_zone_duplicate_blocks_removed=0 context_zone_provenance_markers=3 agent_loop_steps=4 context_zones=2 completion_contract=partial
runtime_spine_trace_present: true
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: none
runtime_spine_status: none
runtime_spine_missing: none
risky_tool_runs: 2
risky_tool_reviewed: 2
risky_tool_missing_action_review: none
gate_outcomes: total=4, protective_block=2, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=2
gate_outcome_records: action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:revise:protective_block,closeout:not_verified:protective_block
gate_outcome_total: 4
gate_outcome_protective_blocks: 2
gate_outcome_recoverable_friction: 0
gate_outcome_unrecovered_blocks: 0
gate_outcome_suspected_false_positives: 0
gate_outcome_policy_correct_but_ux_costly: 0
gate_outcome_harmless_passes: 2
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
context_zone_provenance_markers: 3
state_transition_recorded: false
completion_contract_status: partial
completion_contract_proof_status: not_run
candidate_score_calibrated: false
candidate_score_disagreement: false
observer_outcome_recorded: true
memory_boundary_recorded: true
verification_proof_status: not_run
verification_proof_summary: required validation missing 3/3 commands
verification_proof_kinds: none
verification_proof_support_status: not_run
verification_proof_support_summary: verification proof status not_run blocks verified closeout before proof-kind policy
verification_proof_supports_verified: false
verification_proof_residual_risk: true
premature_edit_count: 0
evidence_before_first_edit: true
scope_drift_count: 0
invalid_action_count: 0
repeated_action_count: 0
failed_action_count: 1
user_question_count: 0
unnecessary_question_count: 0
verification_attempted: true
verification_passed: false
tool_call_count: 2
llm_call_count: 2
warning: no_code_diff
warning: required_commands_not_passing
warning: closeout_not_successful
failure_owner: mixed
outcome_score: 15
process_score: 100
efficiency_score: 92
agent_score: 56
score_penalties: run_failed,required_commands_failed,verification_failed,closeout_not_successful,failed_actions
```

Specialty signals:

```text
memory_active: true
automation_active: true
guided_debugging_active: true
guided_reasoning_active: false
weighted_planning_active: true
closeout_active: true
adaptive_workflow_active: true
active_specialty_signals: 6/7
workflow_contract_activation: entry=active:force repair=active_after_failure
workflow_contract_events: 1
runtime_spine: coverage=6/7, status=none, missing=none
runtime_profile: none
mva_profile_active: false
runtime_spine_detail: context=14 latest=runtime_diet_report decision=13 latest=risk_signal_assessed permission=0 latest=none tool_execution=8 latest=tool_completed state_update=16 latest=agent_loop_step_evaluated verification=2 latest=guided_debugging_completed closeout=3 latest=assistant_responded risky_tool_runs=2 risky_tool_reviewed=2 risky_tool_missing_action_review=none gate_outcomes=total=4, protective_block=2, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=2 stop_reason=no_issue stop_terminal_status=partial stop_action=continue stop_failure_type=unknown rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none route_recovery=events=0, read_search=false, mutation_blocked=false, safety=missing action_scores=1 latest_action_score=16 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=none runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=2 provider_protocol_repairs=4 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=4 context_zone_duplicate_blocks_removed=0 context_zone_provenance_markers=3 agent_loop_steps=4 context_zones=2 completion_contract=partial
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: none
runtime_spine_status: none
runtime_spine_missing: none
risky_tool_runs: 2
risky_tool_reviewed: 2
risky_tool_missing_action_review: none
gate_outcomes: total=4, protective_block=2, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=2
gate_outcome_records: action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:revise:protective_block,closeout:not_verified:protective_block
gate_outcome_total: 4
gate_outcome_protective_blocks: 2
gate_outcome_recoverable_friction: 0
gate_outcome_unrecovered_blocks: 0
gate_outcome_suspected_false_positives: 0
gate_outcome_policy_correct_but_ux_costly: 0
gate_outcome_harmless_passes: 2
gate_outcome_failure_owners: none
agent_loop_steps: 4
context_zones_materialized: true
context_zone_task_state_empty: false
context_zone_current_decision_request_empty: false
context_zone_envelope_messages: 1
context_zone_source_messages: 4
context_zone_duplicate_blocks_removed: 0
context_zone_provenance_markers: 3
state_transition_recorded: false
completion_contract_status: partial
completion_contract_proof_status: not_run
candidate_score_calibrated: false
candidate_score_disagreement: false
observer_outcome_recorded: true
memory_boundary_recorded: true
verification_proof_status: not_run
verification_proof_summary: required validation missing 3/3 commands
verification_proof_kinds: none
verification_proof_support_status: not_run
verification_proof_support_summary: verification proof status not_run blocks verified closeout before proof-kind policy
verification_proof_supports_verified: false
verification_proof_residual_risk: true
risk_signal: entry=high runtime=high
risk_signal_reasons: required validation commands present; runtime risk keyword in request: runtime
memory_sync_events: 1
memory_tool_calls: 0
retrieval_sources: Project
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
required_commands: 3
agent_required_commands: 3
harness_commands: 0
required_command_status: failed
validation_events: 0
stage_validation_events: 0
tool_progress_events: 0
guided_debugging_events: 1
guided_reasoning_events: 0
workflow_plan_events: 1
weighted_plan_events: 1
reweighted_plan_events: 0
adaptive_trigger_events: 2
adaptive_triggers: risk_signal_high,required_validation
latest_top_priority: P3
latest_top_importance_score: 0.2750000059604645
latest_top_weight_share: 0.3333333134651184
acceptance_accepted: missing
closeout_status: not_verified
closeout_tool_records: 4
closeout_tool_evidence: tool evidence: records=4 completed=2 failed=2 denied=0 validation=0 closeout=0 repair=2 changed=0 workflows=code_change commands=ls -la fixtures/core_quality/long_output/ | python3 fixtures/core_quality/long_output/generate_log.py > fixture...
runtime_diet: prompt=4972 tool_schema=3950 tools=19 workflow=guarded
attention: required commands did not pass in the harness
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

- Bundle: `docs/benchmarks/live-flow-real-20260527-084801/core-long-output-artifact/run-bundle`
- Task: `docs/benchmarks/live-flow-real-20260527-084801/core-long-output-artifact/run-bundle/task.json`
- Steps: `docs/benchmarks/live-flow-real-20260527-084801/core-long-output-artifact/run-bundle/steps.jsonl`
- Events: `docs/benchmarks/live-flow-real-20260527-084801/core-long-output-artifact/run-bundle/events.jsonl`
- Final report: `docs/benchmarks/live-flow-real-20260527-084801/core-long-output-artifact/run-bundle/final_report.md`
