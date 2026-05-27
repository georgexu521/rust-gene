# Live Eval Report: core-terminal-install-run

- Run id: `flow-rerun-terminal-20260527-091311`
- Sample: `evalsets/live_tasks/core-terminal-install-run.yaml`
- Worktree: `target/live-evals/flow-rerun-terminal-20260527-091311/core-terminal-install-run/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/flow-rerun-terminal-20260527-091311/core-terminal-install-run/env`
- Test status: `failed`
- Generated: `2026-05-27 09:14:33 +0800`

## Git Status

```text
```

## Diff Stat

```text
```

## Required Commands

```text
$ test -x .venv/bin/python
[exit status: 1]

$ . .venv/bin/activate && python -m core_terminal_demo --self-test | rg '^core-terminal-demo-ok$'
bash: .venv/bin/activate: No such file or directory
[exit status: 1]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-flow-rerun-terminal-20260527-091311/core-terminal-install-run/agent-output.md`
- Events: `docs/benchmarks/live-flow-rerun-terminal-20260527-091311/core-terminal-install-run/agent-events.jsonl`

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
output_chars: 1284
diff_chars: 0
diff_files_changed: 0
tool_executions: 8
first_write_tool_index: none
forbidden_tool_uses: none
tool_errors: 0
tool_failures: 2
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 177
test_status: failed
verification_passed: false
stage_validation_passed: false
acceptance_accepted: None
closeout_status: not_verified
closeout_tool_records: 45
closeout_tool_evidence: tool evidence: records=45 completed=8 failed=37 denied=0 validation=1 closeout=1 repair=37 changed=0 workflows=code_change commands=python3 -c "import core_terminal_demo" 2>&1 || echo "IMPORT_FAILED" | test -x .venv/bin/python && echo "VENV...
runtime_diet: prompt=7603 tool_schema=4300 tools=20 workflow=guarded closeout=full validation=passed:1/1
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
runtime_spine_detail: context=49 latest=runtime_diet_report decision=28 latest=risk_signal_assessed permission=0 latest=none tool_execution=29 latest=tool_completed state_update=61 latest=agent_loop_step_evaluated verification=2 latest=guided_debugging_completed closeout=3 latest=assistant_responded risky_tool_runs=3 risky_tool_reviewed=3 risky_tool_missing_action_review=none gate_outcomes=total=11, protective_block=3, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=8 stop_reason=repeated_tool_failure stop_terminal_status=failed stop_action=recover stop_failure_type=unknown rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none route_recovery=events=0, read_search=false, mutation_blocked=false, safety=missing action_scores=9 latest_action_score=-1 low_action_score_count=1 phase_misaligned_actions=1 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=none runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=9 provider_protocol_repairs=181 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=4 context_zone_duplicate_blocks_removed=0 context_zone_provenance_markers=3 agent_loop_steps=18 context_zones=9 completion_contract=partial
runtime_spine_trace_present: true
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: none
runtime_spine_status: none
runtime_spine_missing: none
risky_tool_runs: 3
risky_tool_reviewed: 3
risky_tool_missing_action_review: none
gate_outcomes: total=11, protective_block=3, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=8
gate_outcome_records: action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:revise:protective_block,action_review:allow:harmless_pass,action_review:revise:protective_block,closeout:not_verified:protective_block
gate_outcome_total: 11
gate_outcome_protective_blocks: 3
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
agent_loop_steps: 18
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
verification_proof_summary: required validation missing 1/2 commands
verification_proof_kinds: command_passed
verification_proof_support_status: not_run
verification_proof_support_summary: verification proof status not_run blocks verified closeout before proof-kind policy
verification_proof_supports_verified: false
verification_proof_residual_risk: true
premature_edit_count: 0
evidence_before_first_edit: true
scope_drift_count: 0
invalid_action_count: 5
repeated_action_count: 4
failed_action_count: 2
user_question_count: 0
unnecessary_question_count: 0
verification_attempted: true
verification_passed: false
tool_call_count: 8
llm_call_count: 9
warning: no_code_diff
warning: required_commands_not_passing
warning: closeout_not_successful
failure_owner: mixed
outcome_score: 15
process_score: 60
efficiency_score: 54
agent_score: 36
score_penalties: run_failed,required_commands_failed,verification_failed,closeout_not_successful,repeated_action,invalid_action,failed_actions,repeated_actions,llm_call_budget_pressure
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
runtime_spine_detail: context=49 latest=runtime_diet_report decision=28 latest=risk_signal_assessed permission=0 latest=none tool_execution=29 latest=tool_completed state_update=61 latest=agent_loop_step_evaluated verification=2 latest=guided_debugging_completed closeout=3 latest=assistant_responded risky_tool_runs=3 risky_tool_reviewed=3 risky_tool_missing_action_review=none gate_outcomes=total=11, protective_block=3, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=8 stop_reason=repeated_tool_failure stop_terminal_status=failed stop_action=recover stop_failure_type=unknown rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none route_recovery=events=0, read_search=false, mutation_blocked=false, safety=missing action_scores=9 latest_action_score=-1 low_action_score_count=1 phase_misaligned_actions=1 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=none runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=9 provider_protocol_repairs=181 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=4 context_zone_duplicate_blocks_removed=0 context_zone_provenance_markers=3 agent_loop_steps=18 context_zones=9 completion_contract=partial
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: none
runtime_spine_status: none
runtime_spine_missing: none
risky_tool_runs: 3
risky_tool_reviewed: 3
risky_tool_missing_action_review: none
gate_outcomes: total=11, protective_block=3, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=8
gate_outcome_records: action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:revise:protective_block,action_review:allow:harmless_pass,action_review:revise:protective_block,closeout:not_verified:protective_block
gate_outcome_total: 11
gate_outcome_protective_blocks: 3
gate_outcome_recoverable_friction: 0
gate_outcome_unrecovered_blocks: 0
gate_outcome_suspected_false_positives: 0
gate_outcome_policy_correct_but_ux_costly: 0
gate_outcome_harmless_passes: 8
gate_outcome_failure_owners: none
agent_loop_steps: 18
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
verification_proof_summary: required validation missing 1/2 commands
verification_proof_kinds: command_passed
verification_proof_support_status: not_run
verification_proof_support_summary: verification proof status not_run blocks verified closeout before proof-kind policy
verification_proof_supports_verified: false
verification_proof_residual_risk: true
risk_signal: entry=high runtime=high
risk_signal_reasons: required validation commands present; runtime risk keyword in request: runtime
memory_sync_events: 8
memory_tool_calls: 0
retrieval_sources: Project
memory_candidate_typed: true
memory_candidate_has_evidence: true
memory_proposal_recorded: true
memory_proposal_status: proposed
memory_proposal_candidates: 1
memory_proposal_kinds: failure_pattern
memory_proposal_evidence_items: 7
memory_proposal_write_policy: review_required
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
guided_debugging_events: 1
guided_reasoning_events: 0
workflow_plan_events: 1
weighted_plan_events: 1
reweighted_plan_events: 0
adaptive_trigger_events: 2
adaptive_triggers: risk_signal_high,required_validation
latest_top_priority: P3
latest_top_importance_score: 0.2150000035762787
latest_top_weight_share: 0.2738853394985199
acceptance_accepted: missing
closeout_status: not_verified
closeout_tool_records: 45
closeout_tool_evidence: tool evidence: records=45 completed=8 failed=37 denied=0 validation=1 closeout=1 repair=37 changed=0 workflows=code_change commands=python3 -c "import core_terminal_demo" 2>&1 || echo "IMPORT_FAILED" | test -x .venv/bin/python && echo "VENV...
runtime_diet: prompt=7603 tool_schema=4300 tools=20 workflow=guarded
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

- Bundle: `docs/benchmarks/live-flow-rerun-terminal-20260527-091311/core-terminal-install-run/run-bundle`
- Task: `docs/benchmarks/live-flow-rerun-terminal-20260527-091311/core-terminal-install-run/run-bundle/task.json`
- Steps: `docs/benchmarks/live-flow-rerun-terminal-20260527-091311/core-terminal-install-run/run-bundle/steps.jsonl`
- Events: `docs/benchmarks/live-flow-rerun-terminal-20260527-091311/core-terminal-install-run/run-bundle/events.jsonl`
- Final report: `docs/benchmarks/live-flow-rerun-terminal-20260527-091311/core-terminal-install-run/run-bundle/final_report.md`
