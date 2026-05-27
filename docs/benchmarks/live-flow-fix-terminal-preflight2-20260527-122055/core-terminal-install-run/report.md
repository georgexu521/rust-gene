# Live Eval Report: core-terminal-install-run

- Run id: `flow-fix-terminal-preflight2-20260527-122055`
- Sample: `evalsets/live_tasks/core-terminal-install-run.yaml`
- Worktree: `target/live-evals/flow-fix-terminal-preflight2-20260527-122055/core-terminal-install-run/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/flow-fix-terminal-preflight2-20260527-122055/core-terminal-install-run/env`
- Test status: `failed`
- Generated: `2026-05-27 12:24:37 +0800`

## Git Status

```text
?? fixtures/core_quality/terminal_app/build/
?? fixtures/core_quality/terminal_app/core_terminal_demo.egg-info/
```

## Diff Stat

```text
 .../core_quality/terminal_app/build/lib/core_terminal_demo/__init__.py   | 1 +
 1 file changed, 1 insertion(+)
 .../terminal_app/build/lib/core_terminal_demo/__main__.py | 15 +++++++++++++++
 1 file changed, 15 insertions(+)
 .../core_quality/terminal_app/core_terminal_demo.egg-info/PKG-INFO    | 4 ++++
 1 file changed, 4 insertions(+)
 .../terminal_app/core_terminal_demo.egg-info/SOURCES.txt           | 7 +++++++
 1 file changed, 7 insertions(+)
 .../terminal_app/core_terminal_demo.egg-info/dependency_links.txt        | 1 +
 1 file changed, 1 insertion(+)
 .../core_quality/terminal_app/core_terminal_demo.egg-info/top_level.txt  | 1 +
 1 file changed, 1 insertion(+)
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
- Output: `docs/benchmarks/live-flow-fix-terminal-preflight2-20260527-122055/core-terminal-install-run/agent-output.md`
- Events: `docs/benchmarks/live-flow-fix-terminal-preflight2-20260527-122055/core-terminal-install-run/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
runtime_diagnostic: 1
start: 1
text_chunk: 2
tool_execution_complete: 4
tool_execution_progress: 1
tool_execution_start: 4
trace_summary: 1
```

Quality signals:

```text
output_chars: 1081
diff_chars: 2694
diff_files_changed: 6
tool_executions: 4
first_write_tool_index: none
forbidden_tool_uses: none
tool_errors: 0
tool_failures: 2
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 91
test_status: failed
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
closeout_tool_records: 7
closeout_tool_evidence: tool evidence: records=7 completed=4 failed=3 denied=0 validation=0 closeout=0 repair=3 changed=0 workflows=code_change commands=pwd && ls -la | which python3 && python3 --version | ls -la fixtures/core_quality/terminal_app/ 2>/dev/null || ...
runtime_diet: prompt=7158 tool_schema=4300 tools=20 workflow=guarded closeout=full validation=passed:2/2 recovered_failed:1
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
runtime_spine_detail: context=19 latest=runtime_diet_report decision=20 latest=risk_signal_assessed permission=0 latest=none tool_execution=15 latest=tool_completed state_update=27 latest=agent_loop_step_evaluated verification=2 latest=guided_debugging_completed closeout=3 latest=assistant_responded risky_tool_runs=5 risky_tool_reviewed=5 risky_tool_missing_action_review=none gate_outcomes=total=7, protective_block=0, recoverable_friction=2, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=5 stop_reason=repeated_tool_failure stop_terminal_status=failed stop_action=recover stop_failure_type=unknown rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none route_recovery=events=0, read_search=false, mutation_blocked=false, safety=missing action_scores=5 latest_action_score=12 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=none runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=3 provider_protocol_repairs=16 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=4 context_zone_duplicate_blocks_removed=0 context_zone_provenance_markers=3 agent_loop_steps=6 context_zones=3 completion_contract=completed
runtime_spine_trace_present: true
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: none
runtime_spine_status: none
runtime_spine_missing: none
risky_tool_runs: 5
risky_tool_reviewed: 5
risky_tool_missing_action_review: none
gate_outcomes: total=7, protective_block=0, recoverable_friction=2, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=5
gate_outcome_records: action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:revise:recoverable_friction,action_review:revise:recoverable_friction,closeout:passed:harmless_pass
gate_outcome_total: 7
gate_outcome_protective_blocks: 0
gate_outcome_recoverable_friction: 2
gate_outcome_unrecovered_blocks: 0
gate_outcome_suspected_false_positives: 0
gate_outcome_policy_correct_but_ux_costly: 0
gate_outcome_harmless_passes: 5
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
context_zone_source_messages: 4
context_zone_duplicate_blocks_removed: 0
context_zone_provenance_markers: 3
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
failed_action_count: 2
user_question_count: 0
unnecessary_question_count: 0
verification_attempted: true
verification_passed: false
tool_call_count: 4
llm_call_count: 3
warning: max_files_changed_exceeded
warning: required_commands_not_passing
failure_owner: agent_flow
outcome_score: 30
process_score: 100
efficiency_score: 84
agent_score: 62
score_penalties: run_failed,required_commands_failed,verification_failed,failed_actions
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
runtime_spine_detail: context=19 latest=runtime_diet_report decision=20 latest=risk_signal_assessed permission=0 latest=none tool_execution=15 latest=tool_completed state_update=27 latest=agent_loop_step_evaluated verification=2 latest=guided_debugging_completed closeout=3 latest=assistant_responded risky_tool_runs=5 risky_tool_reviewed=5 risky_tool_missing_action_review=none gate_outcomes=total=7, protective_block=0, recoverable_friction=2, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=5 stop_reason=repeated_tool_failure stop_terminal_status=failed stop_action=recover stop_failure_type=unknown rollback_recommended=false rollback_completed=false recovery_failure_types=none recovery_kinds=none route_recovery=events=0, read_search=false, mutation_blocked=false, safety=missing action_scores=5 latest_action_score=12 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=none runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=3 provider_protocol_repairs=16 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=4 context_zone_duplicate_blocks_removed=0 context_zone_provenance_markers=3 agent_loop_steps=6 context_zones=3 completion_contract=completed
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: none
runtime_spine_status: none
runtime_spine_missing: none
risky_tool_runs: 5
risky_tool_reviewed: 5
risky_tool_missing_action_review: none
gate_outcomes: total=7, protective_block=0, recoverable_friction=2, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=5
gate_outcome_records: action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:revise:recoverable_friction,action_review:revise:recoverable_friction,closeout:passed:harmless_pass
gate_outcome_total: 7
gate_outcome_protective_blocks: 0
gate_outcome_recoverable_friction: 2
gate_outcome_unrecovered_blocks: 0
gate_outcome_suspected_false_positives: 0
gate_outcome_policy_correct_but_ux_costly: 0
gate_outcome_harmless_passes: 5
gate_outcome_failure_owners: none
agent_loop_steps: 6
context_zones_materialized: true
context_zone_task_state_empty: false
context_zone_current_decision_request_empty: false
context_zone_envelope_messages: 1
context_zone_source_messages: 4
context_zone_duplicate_blocks_removed: 0
context_zone_provenance_markers: 3
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
risk_signal: entry=high runtime=high
risk_signal_reasons: required validation commands present; runtime risk keyword in request: runtime
memory_sync_events: 2
memory_tool_calls: 0
retrieval_sources: Project
memory_candidate_typed: false
memory_candidate_has_evidence: false
memory_proposal_recorded: true
memory_proposal_status: not_applicable
memory_proposal_candidates: 0
memory_proposal_kinds: none
memory_proposal_evidence_items: 0
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
latest_top_priority: P0
latest_top_importance_score: 0.8350000381469727
latest_top_weight_share: 0.372767835855484
acceptance_accepted: True
closeout_status: passed
closeout_tool_records: 7
closeout_tool_evidence: tool evidence: records=7 completed=4 failed=3 denied=0 validation=0 closeout=0 repair=3 changed=0 workflows=code_change commands=pwd && ls -la | which python3 && python3 --version | ls -la fixtures/core_quality/terminal_app/ 2>/dev/null || ...
runtime_diet: prompt=7158 tool_schema=4300 tools=20 workflow=guarded
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

- Bundle: `docs/benchmarks/live-flow-fix-terminal-preflight2-20260527-122055/core-terminal-install-run/run-bundle`
- Task: `docs/benchmarks/live-flow-fix-terminal-preflight2-20260527-122055/core-terminal-install-run/run-bundle/task.json`
- Steps: `docs/benchmarks/live-flow-fix-terminal-preflight2-20260527-122055/core-terminal-install-run/run-bundle/steps.jsonl`
- Events: `docs/benchmarks/live-flow-fix-terminal-preflight2-20260527-122055/core-terminal-install-run/run-bundle/events.jsonl`
- Final report: `docs/benchmarks/live-flow-fix-terminal-preflight2-20260527-122055/core-terminal-install-run/run-bundle/final_report.md`
