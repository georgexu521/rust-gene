# Live Eval Report: desktop-ui-smoke-polish

- Run id: `product-daily-20260602-230826`
- Sample: `evalsets/live_tasks/desktop-ui-smoke-polish.yaml`
- Worktree: `target/live-evals/product-daily-20260602-230826/desktop-ui-smoke-polish/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/product-daily-20260602-230826/desktop-ui-smoke-polish/env`
- Test status: `failed`
- Generated: `2026-06-02 23:51:38 +0800`

## Git Status

```text
```

## Diff Stat

```text
```

## Required Commands

```text
$ corepack pnpm --dir apps/desktop build
[ENOENT] Command failed with ENOENT: pnpm install
spawnSync pnpm ENOENT

pnpm: Command failed with ENOENT: pnpm install
spawnSync pnpm ENOENT
    at getFinalError (file:///Users/georgexu/.cache/node/corepack/v1/pnpm/11.2.2/dist/pnpm.mjs:29448:14)
    at makeError (file:///Users/georgexu/.cache/node/corepack/v1/pnpm/11.2.2/dist/pnpm.mjs:31755:21)
    at getSyncResult (file:///Users/georgexu/.cache/node/corepack/v1/pnpm/11.2.2/dist/pnpm.mjs:33599:10)
    at spawnSubprocessSync (file:///Users/georgexu/.cache/node/corepack/v1/pnpm/11.2.2/dist/pnpm.mjs:33559:14)
    at execaCoreSync (file:///Users/georgexu/.cache/node/corepack/v1/pnpm/11.2.2/dist/pnpm.mjs:33489:23)
    at callBoundExeca (file:///Users/georgexu/.cache/node/corepack/v1/pnpm/11.2.2/dist/pnpm.mjs:36017:23)
    at boundExeca (file:///Users/georgexu/.cache/node/corepack/v1/pnpm/11.2.2/dist/pnpm.mjs:35994:49)
    at sync (file:///Users/georgexu/.cache/node/corepack/v1/pnpm/11.2.2/dist/pnpm.mjs:36149:14)
    at runPnpmCli (file:///Users/georgexu/.cache/node/corepack/v1/pnpm/11.2.2/dist/pnpm.mjs:213263:5)
    at runDepsStatusCheck (file:///Users/georgexu/.cache/node/corepack/v1/pnpm/11.2.2/dist/pnpm.mjs:214967:7)
[exit status: 1]

$ corepack pnpm --dir apps/desktop exec playwright test tests/run-event-state.spec.ts
[ENOENT] Command failed with ENOENT: pnpm install
spawnSync pnpm ENOENT

pnpm: Command failed with ENOENT: pnpm install
spawnSync pnpm ENOENT
    at getFinalError (file:///Users/georgexu/.cache/node/corepack/v1/pnpm/11.2.2/dist/pnpm.mjs:29448:14)
    at makeError (file:///Users/georgexu/.cache/node/corepack/v1/pnpm/11.2.2/dist/pnpm.mjs:31755:21)
    at getSyncResult (file:///Users/georgexu/.cache/node/corepack/v1/pnpm/11.2.2/dist/pnpm.mjs:33599:10)
    at spawnSubprocessSync (file:///Users/georgexu/.cache/node/corepack/v1/pnpm/11.2.2/dist/pnpm.mjs:33559:14)
    at execaCoreSync (file:///Users/georgexu/.cache/node/corepack/v1/pnpm/11.2.2/dist/pnpm.mjs:33489:23)
    at callBoundExeca (file:///Users/georgexu/.cache/node/corepack/v1/pnpm/11.2.2/dist/pnpm.mjs:36017:23)
    at boundExeca (file:///Users/georgexu/.cache/node/corepack/v1/pnpm/11.2.2/dist/pnpm.mjs:35994:49)
    at sync (file:///Users/georgexu/.cache/node/corepack/v1/pnpm/11.2.2/dist/pnpm.mjs:36149:14)
    at runPnpmCli (file:///Users/georgexu/.cache/node/corepack/v1/pnpm/11.2.2/dist/pnpm.mjs:213263:5)
    at runDepsStatusCheck (file:///Users/georgexu/.cache/node/corepack/v1/pnpm/11.2.2/dist/pnpm.mjs:214967:7)
[exit status: 1]

$ corepack pnpm --dir apps/desktop test:ui-smoke
[ENOENT] Command failed with ENOENT: pnpm install
spawnSync pnpm ENOENT

pnpm: Command failed with ENOENT: pnpm install
spawnSync pnpm ENOENT
    at getFinalError (file:///Users/georgexu/.cache/node/corepack/v1/pnpm/11.2.2/dist/pnpm.mjs:29448:14)
    at makeError (file:///Users/georgexu/.cache/node/corepack/v1/pnpm/11.2.2/dist/pnpm.mjs:31755:21)
    at getSyncResult (file:///Users/georgexu/.cache/node/corepack/v1/pnpm/11.2.2/dist/pnpm.mjs:33599:10)
    at spawnSubprocessSync (file:///Users/georgexu/.cache/node/corepack/v1/pnpm/11.2.2/dist/pnpm.mjs:33559:14)
    at execaCoreSync (file:///Users/georgexu/.cache/node/corepack/v1/pnpm/11.2.2/dist/pnpm.mjs:33489:23)
    at callBoundExeca (file:///Users/georgexu/.cache/node/corepack/v1/pnpm/11.2.2/dist/pnpm.mjs:36017:23)
    at boundExeca (file:///Users/georgexu/.cache/node/corepack/v1/pnpm/11.2.2/dist/pnpm.mjs:35994:49)
    at sync (file:///Users/georgexu/.cache/node/corepack/v1/pnpm/11.2.2/dist/pnpm.mjs:36149:14)
    at runPnpmCli (file:///Users/georgexu/.cache/node/corepack/v1/pnpm/11.2.2/dist/pnpm.mjs:213263:5)
    at runDepsStatusCheck (file:///Users/georgexu/.cache/node/corepack/v1/pnpm/11.2.2/dist/pnpm.mjs:214967:7)
[exit status: 1]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-product-daily-20260602-230826/desktop-ui-smoke-polish/agent-output.md`
- Events: `docs/benchmarks/live-product-daily-20260602-230826/desktop-ui-smoke-polish/agent-events.jsonl`
- Monitor: `docs/benchmarks/live-product-daily-20260602-230826/desktop-ui-smoke-polish/agent-monitor.log`

Event counts:

```text
complete: 1
eval_started: 1
runtime_diagnostic: 16
start: 1
text_chunk: 208
tool_execution_complete: 18
tool_execution_progress: 3
tool_execution_start: 18
trace_summary: 1
```

Quality signals:

```text
output_chars: 9692
diff_chars: 0
diff_files_changed: 0
diff_files_changed_raw: 0
generated_dependency_files_ignored: 0
tool_executions: 18
first_write_tool_index: none
forbidden_tool_uses: none
tool_errors: 3
tool_failures: 5
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 290
test_status: failed
verification_passed: false
stage_validation_passed: false
acceptance_accepted: None
closeout_status: failed
closeout_tool_records: 20
closeout_tool_evidence: tool evidence: records=20 completed=15 failed=5 denied=0 validation=0 closeout=0 repair=5 changed=0 workflows=code_change commands=ls /Users/georgexu/Desktop/rust-agent/target/live-evals/product-daily-20260602-230826/desktop-ui-smoke-polish...
runtime_diet: prompt=24383 tool_schema=4272 tools=19 workflow=minimal closeout=full validation=failed
adaptive_triggers: none
risk_signal: entry=ordinary runtime=high
risk_signal_reasons: ordinary change surface
trace_event_types: provider.protocol,provider.tool_repair,workflow.fallback,cache.usage,api.done,closeout,execution.report,memory.proposal,memory.proposal,runtime.diet,completion.contract,assistant
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
runtime_spine: coverage=7/7, status=none, missing=none
runtime_profile: none
mva_profile_active: false
runtime_spine_detail: context=60 latest=runtime_diet_report decision=53 latest=action_reviewed permission=2 latest=goal_drift_detected tool_execution=51 latest=api_request_completed state_update=77 latest=workflow_fallback verification=5 latest=guided_debugging_completed closeout=3 latest=assistant_responded risky_tool_runs=4 risky_tool_reviewed=4 risky_tool_missing_action_review=none gate_outcomes=total=21, protective_block=3, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=18 stop_reason=no_issue stop_terminal_status=failed stop_action=continue stop_failure_type=invalid_params rollback_recommended=false rollback_completed=false recovery_failure_types=permission_block recovery_kinds=ask_user_or_choose_safer_path route_recovery=events=0, read_search=false, mutation_blocked=false, safety=missing action_scores=13 latest_action_score=34 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=2 observer_quality_warning_labels=missing_validation_result permission_sources=none runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=11 provider_protocol_repairs=0 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=12 context_zone_duplicate_blocks_removed=0 context_zone_provenance_markers=0 agent_loop_steps=20 context_zones=11 completion_contract=failed
runtime_spine_trace_present: true
runtime_spine_phase_coverage: 7/7
runtime_spine_observed_phases: context,decision,permission,tool_execution,state_update,verification,closeout
runtime_spine_assertions: none
runtime_spine_status: none
runtime_spine_missing: none
risky_tool_runs: 4
risky_tool_reviewed: 4
risky_tool_missing_action_review: none
gate_outcomes: total=21, protective_block=3, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=18
gate_outcome_records: action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:revise:protective_block,+9
gate_outcome_total: 21
gate_outcome_protective_blocks: 3
gate_outcome_recoverable_friction: 0
gate_outcome_unrecovered_blocks: 0
gate_outcome_suspected_false_positives: 0
gate_outcome_policy_correct_but_ux_costly: 0
gate_outcome_harmless_passes: 18
gate_outcome_failure_owners: none
route_recovery: events=0, read_search=false, mutation_blocked=false, safety=missing
route_recovery_events: 0
route_recovery_failure_types: none
route_recovery_kinds: none
route_recovery_read_search_expanded: false
route_recovery_mutation_blocked: false
route_recovery_safety_monotonic: missing
route_recovery_unsafe_mutation_expansion: false
agent_loop_steps: 20
context_zones_materialized: true
context_zone_task_state_empty: false
context_zone_current_decision_request_empty: false
context_zone_envelope_messages: 1
context_zone_source_messages: 12
context_zone_duplicate_blocks_removed: 0
context_zone_provenance_markers: 0
state_transition_recorded: false
completion_contract_status: failed
completion_contract_proof_status: failed
candidate_score_calibrated: false
candidate_score_disagreement: true
observer_outcome_recorded: true
memory_boundary_recorded: true
verification_proof_status: failed
verification_proof_summary: task state reports failed verification without ledger evidence
verification_proof_kinds: none
verification_proof_support_status: failed
verification_proof_support_summary: verification proof status failed blocks verified closeout before proof-kind policy
verification_proof_supports_verified: false
verification_proof_residual_risk: true
premature_edit_count: 0
evidence_before_first_edit: true
scope_drift_count: 2
invalid_action_count: 2
repeated_action_count: 0
failed_action_count: 8
user_question_count: 0
unnecessary_question_count: 0
verification_attempted: true
verification_passed: false
tool_call_count: 18
llm_call_count: 11
warning: no_code_diff
warning: tool_errors_seen
warning: required_commands_not_passing
warning: closeout_not_successful
failure_owner: mixed
outcome_score: 15
process_score: 60
efficiency_score: 65
agent_score: 38
score_penalties: run_failed,required_commands_failed,verification_failed,closeout_not_successful,scope_drift,invalid_action,failed_actions,llm_call_budget_pressure
```

Specialty signals:

```text
memory_active: false
automation_active: true
guided_debugging_active: true
guided_reasoning_active: false
weighted_planning_active: true
closeout_active: true
adaptive_workflow_active: false
active_specialty_signals: 4/7
workflow_contract_activation: entry=active:force repair=active_after_failure
workflow_contract_events: 1
runtime_spine: coverage=7/7, status=none, missing=none
runtime_profile: none
mva_profile_active: false
runtime_spine_detail: context=60 latest=runtime_diet_report decision=53 latest=action_reviewed permission=2 latest=goal_drift_detected tool_execution=51 latest=api_request_completed state_update=77 latest=workflow_fallback verification=5 latest=guided_debugging_completed closeout=3 latest=assistant_responded risky_tool_runs=4 risky_tool_reviewed=4 risky_tool_missing_action_review=none gate_outcomes=total=21, protective_block=3, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=18 stop_reason=no_issue stop_terminal_status=failed stop_action=continue stop_failure_type=invalid_params rollback_recommended=false rollback_completed=false recovery_failure_types=permission_block recovery_kinds=ask_user_or_choose_safer_path route_recovery=events=0, read_search=false, mutation_blocked=false, safety=missing action_scores=13 latest_action_score=34 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=2 observer_quality_warning_labels=missing_validation_result permission_sources=none runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=11 provider_protocol_repairs=0 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=12 context_zone_duplicate_blocks_removed=0 context_zone_provenance_markers=0 agent_loop_steps=20 context_zones=11 completion_contract=failed
runtime_spine_phase_coverage: 7/7
runtime_spine_observed_phases: context,decision,permission,tool_execution,state_update,verification,closeout
runtime_spine_assertions: none
runtime_spine_status: none
runtime_spine_missing: none
risky_tool_runs: 4
risky_tool_reviewed: 4
risky_tool_missing_action_review: none
gate_outcomes: total=21, protective_block=3, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=18
gate_outcome_records: action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:revise:protective_block,+9
gate_outcome_total: 21
gate_outcome_protective_blocks: 3
gate_outcome_recoverable_friction: 0
gate_outcome_unrecovered_blocks: 0
gate_outcome_suspected_false_positives: 0
gate_outcome_policy_correct_but_ux_costly: 0
gate_outcome_harmless_passes: 18
gate_outcome_failure_owners: none
agent_loop_steps: 20
context_zones_materialized: true
context_zone_task_state_empty: false
context_zone_current_decision_request_empty: false
context_zone_envelope_messages: 1
context_zone_source_messages: 12
context_zone_duplicate_blocks_removed: 0
context_zone_provenance_markers: 0
state_transition_recorded: false
completion_contract_status: failed
completion_contract_proof_status: failed
candidate_score_calibrated: false
candidate_score_disagreement: true
observer_outcome_recorded: true
memory_boundary_recorded: true
verification_proof_status: failed
verification_proof_summary: task state reports failed verification without ledger evidence
verification_proof_kinds: none
verification_proof_support_status: failed
verification_proof_support_summary: verification proof status failed blocks verified closeout before proof-kind policy
verification_proof_supports_verified: false
verification_proof_residual_risk: true
risk_signal: entry=ordinary runtime=high
risk_signal_reasons: ordinary change surface
memory_sync_events: 0
memory_tool_calls: 0
retrieval_sources: Project,ProjectMap
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
required_commands: 3
agent_required_commands: 3
harness_commands: 0
required_command_status: failed
validation_events: 0
stage_validation_events: 0
tool_progress_events: 3
guided_debugging_events: 4
guided_reasoning_events: 0
workflow_plan_events: 1
weighted_plan_events: 1
reweighted_plan_events: 0
adaptive_trigger_events: 0
adaptive_triggers: none
latest_top_priority: P0
latest_top_importance_score: 0.8025000095367432
latest_top_weight_share: 0.3203592896461487
acceptance_accepted: missing
closeout_status: failed
closeout_tool_records: 20
closeout_tool_evidence: tool evidence: records=20 completed=15 failed=5 denied=0 validation=0 closeout=0 repair=5 changed=0 workflows=code_change commands=ls /Users/georgexu/Desktop/rust-agent/target/live-evals/product-daily-20260602-230826/desktop-ui-smoke-polish...
runtime_diet: prompt=24383 tool_schema=4272 tools=19 workflow=minimal
attention: required commands did not pass in the harness
```

Agent monitor tail:

```text
[2026-06-02T23:46:55+0800] agent-run still running elapsed=30s idle_for=25s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=775
[2026-06-02T23:47:25+0800] agent-run still running elapsed=60s idle_for=5s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=1026
[2026-06-02T23:47:55+0800] agent-run still running elapsed=90s idle_for=20s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=15753
[2026-06-02T23:48:25+0800] agent-run still running elapsed=120s idle_for=0s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=48419
[2026-06-02T23:48:55+0800] agent-run still running elapsed=150s idle_for=30s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=48419
[2026-06-02T23:49:25+0800] agent-run still running elapsed=180s idle_for=60s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=48419
[2026-06-02T23:49:55+0800] agent-run still running elapsed=210s idle_for=15s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=54120
[2026-06-02T23:50:25+0800] agent-run still running elapsed=240s idle_for=10s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=60249
[2026-06-02T23:50:55+0800] agent-run still running elapsed=270s idle_for=0s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=71783
[2026-06-02T23:51:25+0800] agent-run still running elapsed=300s idle_for=30s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=71783
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

- Bundle: `docs/benchmarks/live-product-daily-20260602-230826/desktop-ui-smoke-polish/run-bundle`
- Task: `docs/benchmarks/live-product-daily-20260602-230826/desktop-ui-smoke-polish/run-bundle/task.json`
- Steps: `docs/benchmarks/live-product-daily-20260602-230826/desktop-ui-smoke-polish/run-bundle/steps.jsonl`
- Events: `docs/benchmarks/live-product-daily-20260602-230826/desktop-ui-smoke-polish/run-bundle/events.jsonl`
- Final report: `docs/benchmarks/live-product-daily-20260602-230826/desktop-ui-smoke-polish/run-bundle/final_report.md`
