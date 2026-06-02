# Live Eval Report: project-partner-resume-with-memory

- Run id: `product-daily-20260602-230826`
- Sample: `evalsets/live_tasks/project-partner-resume-with-memory.yaml`
- Worktree: `target/live-evals/product-daily-20260602-230826/project-partner-resume-with-memory/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/product-daily-20260602-230826/project-partner-resume-with-memory/env`
- Test status: `ok`
- Generated: `2026-06-02 23:35:05 +0800`

## Git Status

```text
```

## Diff Stat

```text
```

## Required Commands

```text
$ test -f fixtures/project_partner_resume/memory/project.md
[exit status: 0]

$ test -f fixtures/project_partner_resume/reports/previous_execution_report.json
[exit status: 0]

$ rg 'CSV export' fixtures/project_partner_resume
fixtures/project_partner_resume/reports/previous_execution_report.json:  "risks": ["CSV export is not implemented yet"],
fixtures/project_partner_resume/reports/previous_execution_report.json:  "next_steps": ["Implement CSV export before adding login or cloud sync"]
fixtures/project_partner_resume/memory/project.md:- Next product goal: add CSV export for recorded strain rows.
[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-product-daily-20260602-230826/project-partner-resume-with-memory/agent-output.md`
- Events: `docs/benchmarks/live-product-daily-20260602-230826/project-partner-resume-with-memory/agent-events.jsonl`
- Monitor: `docs/benchmarks/live-product-daily-20260602-230826/project-partner-resume-with-memory/agent-monitor.log`

Event counts:

```text
complete: 1
eval_started: 1
runtime_diagnostic: 16
start: 1
text_chunk: 128
tool_execution_complete: 11
tool_execution_progress: 1
tool_execution_start: 11
trace_summary: 1
```

Quality signals:

```text
output_chars: 5440
diff_chars: 0
diff_files_changed: 0
diff_files_changed_raw: 0
generated_dependency_files_ignored: 0
tool_executions: 11
first_write_tool_index: none
forbidden_tool_uses: none
tool_errors: 0
tool_failures: 2
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 232
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
closeout_tool_records: 12
closeout_tool_evidence: tool evidence: records=12 completed=10 failed=2 denied=0 validation=0 closeout=0 repair=2 changed=0 workflows=direct commands=none
runtime_diet: prompt=8696 tool_schema=2297 tools=10 workflow=none closeout=full validation=not_applicable
adaptive_triggers: none
risk_signal: entry=ordinary runtime=high
risk_signal_reasons: ordinary change surface
trace_event_types: api.start,provider.protocol,workflow.fallback,cache.usage,api.done,closeout,execution.report,memory.proposal,memory.proposal,runtime.diet,completion.contract,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: read_only_audit
behavior_assertions: none
behavior_assertion_status: none
output_assertions: contains=2,not_contains=1
output_assertion_status: passed
output_assertion_missing: none
trajectory_assertions: requires_observer_outcome,requires_stop_check,max_repeated_action_count,max_scope_drift_count,max_premature_edit_count,requires_runtime_spine_passed
trajectory_assertion_status: failed
trajectory_assertion_missing: max_repeated_action_count:5>1;max_scope_drift_count:1>0
runtime_spine: coverage=6/7, status=passed, missing=none
runtime_profile: project_partner_alignment
mva_profile_active: false
runtime_spine_detail: context=60 latest=runtime_diet_report decision=34 latest=action_reviewed permission=0 latest=none tool_execution=37 latest=api_request_completed state_update=68 latest=workflow_fallback verification=2 latest=guided_debugging_completed closeout=3 latest=assistant_responded risky_tool_runs=0 risky_tool_reviewed=0 risky_tool_missing_action_review=none gate_outcomes=total=14, protective_block=0, recoverable_friction=2, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=12 stop_reason=no_issue stop_terminal_status=completed stop_action=continue stop_failure_type=missing rollback_recommended=false rollback_completed=false recovery_failure_types=hidden_read_search_tool_requested recovery_kinds=expand_read_search_only route_recovery=events=1, read_search=true, mutation_blocked=false, safety=true action_scores=7 latest_action_score=36 low_action_score_count=1 phase_misaligned_actions=1 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=1 observer_quality_warning_labels=missing_permission_source permission_sources=none runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=11 provider_protocol_repairs=0 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=2 context_zone_duplicate_blocks_removed=0 context_zone_provenance_markers=0 agent_loop_steps=20 context_zones=11 completion_contract=completed
runtime_spine_trace_present: true
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: phase:context,phase:decision,phase:tool_execution,phase:state_update,phase:closeout,event:task_contract_materialized,event:context_pack_materialized,event:action_reviewed,event:tool_observation_recorded,event:completion_contract_evaluated,event:execution_report_prepared,completion_status:completed
runtime_spine_status: passed
runtime_spine_missing: none
risky_tool_runs: 0
risky_tool_reviewed: 0
risky_tool_missing_action_review: none
gate_outcomes: total=14, protective_block=0, recoverable_friction=2, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=12
gate_outcome_records: action_review:allow:harmless_pass,action_review:revise:recoverable_friction,action_review:revise:recoverable_friction,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,+2
gate_outcome_total: 14
gate_outcome_protective_blocks: 0
gate_outcome_recoverable_friction: 2
gate_outcome_unrecovered_blocks: 0
gate_outcome_suspected_false_positives: 0
gate_outcome_policy_correct_but_ux_costly: 0
gate_outcome_harmless_passes: 12
gate_outcome_failure_owners: none
route_recovery: events=1, read_search=true, mutation_blocked=false, safety=true
route_recovery_events: 1
route_recovery_failure_types: hidden_read_search_tool_requested
route_recovery_kinds: expand_read_search_only
route_recovery_read_search_expanded: true
route_recovery_mutation_blocked: false
route_recovery_safety_monotonic: true
route_recovery_unsafe_mutation_expansion: false
agent_loop_steps: 20
context_zones_materialized: true
context_zone_task_state_empty: false
context_zone_current_decision_request_empty: false
context_zone_envelope_messages: 1
context_zone_source_messages: 2
context_zone_duplicate_blocks_removed: 0
context_zone_provenance_markers: 0
state_transition_recorded: false
completion_contract_status: completed
completion_contract_proof_status: not_applicable
candidate_score_calibrated: false
candidate_score_disagreement: false
observer_outcome_recorded: true
memory_boundary_recorded: true
verification_proof_status: not_applicable
verification_proof_summary: validation not required for read-only direct answer
verification_proof_kinds: none
verification_proof_support_status: not_applicable
verification_proof_support_summary: no proof kind required for this task scope
verification_proof_supports_verified: false
verification_proof_residual_risk: false
premature_edit_count: 0
evidence_before_first_edit: true
scope_drift_count: 1
invalid_action_count: 7
repeated_action_count: 5
failed_action_count: 2
user_question_count: 0
unnecessary_question_count: 0
verification_attempted: true
verification_passed: true
tool_call_count: 11
llm_call_count: 11
warning: no_code_diff
warning: trajectory_assertions_not_passing
failure_owner: agent_flow
outcome_score: 65
process_score: 45
efficiency_score: 54
agent_score: 57
score_penalties: run_failed,trajectory_assertions_failed,scope_drift,repeated_action,invalid_action,failed_actions,repeated_actions,llm_call_budget_pressure
```

Specialty signals:

```text
memory_active: true
automation_active: true
guided_debugging_active: true
guided_reasoning_active: false
weighted_planning_active: false
closeout_active: true
adaptive_workflow_active: false
active_specialty_signals: 4/7
workflow_contract_activation: entry=skipped:force repair=active_after_failure
workflow_contract_events: 1
runtime_spine: coverage=6/7, status=passed, missing=none
runtime_profile: project_partner_alignment
mva_profile_active: false
runtime_spine_detail: context=60 latest=runtime_diet_report decision=34 latest=action_reviewed permission=0 latest=none tool_execution=37 latest=api_request_completed state_update=68 latest=workflow_fallback verification=2 latest=guided_debugging_completed closeout=3 latest=assistant_responded risky_tool_runs=0 risky_tool_reviewed=0 risky_tool_missing_action_review=none gate_outcomes=total=14, protective_block=0, recoverable_friction=2, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=12 stop_reason=no_issue stop_terminal_status=completed stop_action=continue stop_failure_type=missing rollback_recommended=false rollback_completed=false recovery_failure_types=hidden_read_search_tool_requested recovery_kinds=expand_read_search_only route_recovery=events=1, read_search=true, mutation_blocked=false, safety=true action_scores=7 latest_action_score=36 low_action_score_count=1 phase_misaligned_actions=1 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=1 observer_quality_warning_labels=missing_permission_source permission_sources=none runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=11 provider_protocol_repairs=0 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=2 context_zone_duplicate_blocks_removed=0 context_zone_provenance_markers=0 agent_loop_steps=20 context_zones=11 completion_contract=completed
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: phase:context,phase:decision,phase:tool_execution,phase:state_update,phase:closeout,event:task_contract_materialized,event:context_pack_materialized,event:action_reviewed,event:tool_observation_recorded,event:completion_contract_evaluated,event:execution_report_prepared,completion_status:completed
runtime_spine_status: passed
runtime_spine_missing: none
risky_tool_runs: 0
risky_tool_reviewed: 0
risky_tool_missing_action_review: none
gate_outcomes: total=14, protective_block=0, recoverable_friction=2, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=12
gate_outcome_records: action_review:allow:harmless_pass,action_review:revise:recoverable_friction,action_review:revise:recoverable_friction,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,+2
gate_outcome_total: 14
gate_outcome_protective_blocks: 0
gate_outcome_recoverable_friction: 2
gate_outcome_unrecovered_blocks: 0
gate_outcome_suspected_false_positives: 0
gate_outcome_policy_correct_but_ux_costly: 0
gate_outcome_harmless_passes: 12
gate_outcome_failure_owners: none
agent_loop_steps: 20
context_zones_materialized: true
context_zone_task_state_empty: false
context_zone_current_decision_request_empty: false
context_zone_envelope_messages: 1
context_zone_source_messages: 2
context_zone_duplicate_blocks_removed: 0
context_zone_provenance_markers: 0
state_transition_recorded: false
completion_contract_status: completed
completion_contract_proof_status: not_applicable
candidate_score_calibrated: false
candidate_score_disagreement: false
observer_outcome_recorded: true
memory_boundary_recorded: true
verification_proof_status: not_applicable
verification_proof_summary: validation not required for read-only direct answer
verification_proof_kinds: none
verification_proof_support_status: not_applicable
verification_proof_support_summary: no proof kind required for this task scope
verification_proof_supports_verified: false
verification_proof_residual_risk: false
risk_signal: entry=ordinary runtime=high
risk_signal_reasons: ordinary change surface
memory_sync_events: 0
memory_tool_calls: 1
retrieval_sources: Session,ProjectMap
memory_candidate_typed: true
memory_candidate_has_evidence: true
memory_proposal_recorded: true
memory_proposal_status: proposed
memory_proposal_candidates: 1
memory_proposal_kinds: validation_baseline
memory_proposal_evidence_items: 4
memory_proposal_write_policy: review_required
memory_proposal_write_performed: false
memory_record_used: false
memory_use_count_updated: false
memory_failure_lesson_promoted: false
memory_action_weight_changed: false
memory_stale_demoted: false
memory_scope_correct: false
required_commands: 3
agent_required_commands: 0
harness_commands: 3
required_command_status: ok
validation_events: 0
stage_validation_events: 0
tool_progress_events: 1
guided_debugging_events: 1
guided_reasoning_events: 0
workflow_plan_events: 0
weighted_plan_events: 0
reweighted_plan_events: 0
adaptive_trigger_events: 0
adaptive_triggers: none
latest_top_priority: none
latest_top_importance_score: none
latest_top_weight_share: none
acceptance_accepted: True
closeout_status: passed
closeout_tool_records: 12
closeout_tool_evidence: tool evidence: records=12 completed=10 failed=2 denied=0 validation=0 closeout=0 repair=2 changed=0 workflows=direct commands=none
runtime_diet: prompt=8696 tool_schema=2297 tools=10 workflow=none
```

Agent monitor tail:

```text
[2026-06-02T23:33:48+0800] agent-run still running elapsed=30s idle_for=10s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=3338
[2026-06-02T23:34:18+0800] agent-run still running elapsed=60s idle_for=0s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=26447
[2026-06-02T23:34:48+0800] agent-run still running elapsed=90s idle_for=10s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=49062
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

- Bundle: `docs/benchmarks/live-product-daily-20260602-230826/project-partner-resume-with-memory/run-bundle`
- Task: `docs/benchmarks/live-product-daily-20260602-230826/project-partner-resume-with-memory/run-bundle/task.json`
- Steps: `docs/benchmarks/live-product-daily-20260602-230826/project-partner-resume-with-memory/run-bundle/steps.jsonl`
- Events: `docs/benchmarks/live-product-daily-20260602-230826/project-partner-resume-with-memory/run-bundle/events.jsonl`
- Final report: `docs/benchmarks/live-product-daily-20260602-230826/project-partner-resume-with-memory/run-bundle/final_report.md`
