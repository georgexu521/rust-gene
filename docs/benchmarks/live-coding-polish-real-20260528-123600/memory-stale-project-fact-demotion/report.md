# Live Eval Report: memory-stale-project-fact-demotion

- Run id: `coding-polish-real-20260528-123600`
- Sample: `evalsets/live_tasks/memory-stale-project-fact-demotion.yaml`
- Worktree: `target/live-evals/coding-polish-real-20260528-123600/memory-stale-project-fact-demotion/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/coding-polish-real-20260528-123600/memory-stale-project-fact-demotion/env`
- Test status: `failed`
- Generated: `2026-05-28 14:14:39 +0800`

## Git Status

```text
 M src/memory/manager.rs
```

## Diff Stat

```text
 src/memory/manager.rs | 28 ++++++++++++++++++++++++----
 1 file changed, 24 insertions(+), 4 deletions(-)
```

## Required Commands

```text
$ cargo test -q memory -- --test-threads=1
error: unexpected closing delimiter: `}`
   --> src/memory/manager.rs:683:1
    |
672 |     match record.kind {
    |                       - this opening brace...
...
682 |     }
    |     - ...matches this closing brace
683 | }
    | ^ unexpected closing delimiter

error: could not compile `priority-agent` (lib) due to 1 previous error
error: could not compile `priority-agent` (lib test) due to 1 previous error
[exit status: 101]

$ cargo test -q retrieval_context -- --test-threads=1
error: unexpected closing delimiter: `}`
   --> src/memory/manager.rs:683:1
    |
672 |     match record.kind {
    |                       - this opening brace...
...
682 |     }
    |     - ...matches this closing brace
683 | }
    | ^ unexpected closing delimiter

error: could not compile `priority-agent` (lib) due to 1 previous error
error: could not compile `priority-agent` (lib test) due to 1 previous error
[exit status: 101]

$ python3 -c "p='src/memory/manager.rs'; s=open(p).read(); assert 'record_needs_revalidation' in s and ':stale' in s and 'superseded_by' in s"
[exit status: 0]

$ python3 -c "p='src/engine/retrieval_context.rs'; s=open(p).read(); assert 'needs revalidation' in s and 'TrustLevel::Low' in s and ':stale:' in s"
[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-coding-polish-real-20260528-123600/memory-stale-project-fact-demotion/agent-output.md`
- Events: `docs/benchmarks/live-coding-polish-real-20260528-123600/memory-stale-project-fact-demotion/agent-events.jsonl`
- Monitor: `docs/benchmarks/live-coding-polish-real-20260528-123600/memory-stale-project-fact-demotion/agent-monitor.log`

Event counts:

```text
complete: 1
eval_started: 1
runtime_diagnostic: 1
start: 1
text_chunk: 2
tool_execution_complete: 17
tool_execution_progress: 1
tool_execution_start: 17
trace_summary: 1
```

Quality signals:

```text
output_chars: 1900
diff_chars: 1321
diff_files_changed: 1
diff_files_changed_raw: 1
generated_dependency_files_ignored: 0
tool_executions: 17
first_write_tool_index: 13
forbidden_tool_uses: none
tool_errors: 2
tool_failures: 2
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 229
test_status: failed
verification_passed: false
stage_validation_passed: false
acceptance_accepted: None
closeout_status: failed
closeout_tool_records: 83
closeout_tool_evidence: tool evidence: records=83 completed=15 failed=68 denied=0 validation=0 closeout=1 repair=69 changed=1 workflows=code_change commands=none
runtime_diet: prompt=26659 tool_schema=3950 tools=19 workflow=strict closeout=full validation=failed:2/4
adaptive_triggers: risk_signal_high,required_validation,repeated_no_code_progress,first_code_change,verification_failed
risk_signal: entry=high runtime=high
risk_signal_reasons: route risk is high; required validation commands present; complex required-validation surface; broad validation command requested; runtime risk keyword in request: memory
trace_event_types: agent.loop,stop.check,agent.loop,risk.signal,guided.debug,closeout,execution.report,memory.proposal,memory.proposal,runtime.diet,completion.contract,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: seeded_code_change
behavior_assertions: memory_record_used,memory_stale_demoted,memory_candidate_has_evidence,memory_scope_correct
behavior_assertion_status: failed
output_assertions: none
output_assertion_status: none
output_assertion_missing: none
trajectory_assertions: none
trajectory_assertion_status: none
trajectory_assertion_missing: none
runtime_spine: coverage=7/7, status=none, missing=none
runtime_profile: none
mva_profile_active: false
runtime_spine_detail: context=45 latest=runtime_diet_report decision=46 latest=risk_signal_assessed permission=2 latest=goal_drift_detected tool_execution=41 latest=tool_completed state_update=71 latest=agent_loop_step_evaluated verification=10 latest=guided_debugging_completed closeout=3 latest=assistant_responded risky_tool_runs=1 risky_tool_reviewed=1 risky_tool_missing_action_review=none gate_outcomes=total=18, protective_block=1, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=17 stop_reason=repeated_tool_failure stop_terminal_status=failed stop_action=recover stop_failure_type=unknown rollback_recommended=false rollback_completed=false recovery_failure_types=code_change_no_diff_after_repeated_progress,stale_read_conflict recovery_kinds=code_change_no_diff_replan,refresh_read_before_edit route_recovery=events=1, read_search=false, mutation_blocked=false, safety=true action_scores=17 latest_action_score=31 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=none runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=7 provider_protocol_repairs=301 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=5 context_zone_duplicate_blocks_removed=0 context_zone_provenance_markers=10 agent_loop_steps=14 context_zones=7 completion_contract=failed
runtime_spine_trace_present: true
runtime_spine_phase_coverage: 7/7
runtime_spine_observed_phases: context,decision,permission,tool_execution,state_update,verification,closeout
runtime_spine_assertions: none
runtime_spine_status: none
runtime_spine_missing: none
risky_tool_runs: 1
risky_tool_reviewed: 1
risky_tool_missing_action_review: none
gate_outcomes: total=18, protective_block=1, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=17
gate_outcome_records: action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,+6
gate_outcome_total: 18
gate_outcome_protective_blocks: 1
gate_outcome_recoverable_friction: 0
gate_outcome_unrecovered_blocks: 0
gate_outcome_suspected_false_positives: 0
gate_outcome_policy_correct_but_ux_costly: 0
gate_outcome_harmless_passes: 17
gate_outcome_failure_owners: none
route_recovery: events=1, read_search=false, mutation_blocked=false, safety=true
route_recovery_events: 1
route_recovery_failure_types: code_change_no_diff_after_repeated_progress
route_recovery_kinds: code_change_no_diff_replan
route_recovery_read_search_expanded: false
route_recovery_mutation_blocked: false
route_recovery_safety_monotonic: true
route_recovery_unsafe_mutation_expansion: false
agent_loop_steps: 14
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
verification_proof_summary: required validation failed 2/4 commands
verification_proof_kinds: none
verification_proof_support_status: failed
verification_proof_support_summary: verification proof status failed blocks verified closeout before proof-kind policy
verification_proof_supports_verified: false
verification_proof_residual_risk: true
premature_edit_count: 0
evidence_before_first_edit: true
scope_drift_count: 2
invalid_action_count: 6
repeated_action_count: 4
failed_action_count: 4
user_question_count: 0
unnecessary_question_count: 0
verification_attempted: true
verification_passed: false
tool_call_count: 17
llm_call_count: 7
warning: tool_errors_seen
warning: earlier_verification_failed_before_repair
warning: earlier_stage_validation_failed_before_repair
warning: required_commands_not_passing
warning: behavior_assertions_not_passing
warning: closeout_not_successful
warning: stage_validation_failed
warning: verification_failed
failure_owner: llm_reasoning
outcome_score: 5
process_score: 30
efficiency_score: 55
agent_score: 22
score_penalties: run_failed,required_commands_failed,verification_failed,closeout_not_successful,behavior_assertions_failed,scope_drift,repeated_action,invalid_action,failed_actions,repeated_actions
```

Specialty signals:

```text
memory_active: true
automation_active: true
guided_debugging_active: true
guided_reasoning_active: false
weighted_planning_active: false
closeout_active: true
adaptive_workflow_active: true
active_specialty_signals: 5/7
workflow_contract_activation: entry=active:force repair=active_after_failure
workflow_contract_events: 1
runtime_spine: coverage=7/7, status=none, missing=none
runtime_profile: none
mva_profile_active: false
runtime_spine_detail: context=45 latest=runtime_diet_report decision=46 latest=risk_signal_assessed permission=2 latest=goal_drift_detected tool_execution=41 latest=tool_completed state_update=71 latest=agent_loop_step_evaluated verification=10 latest=guided_debugging_completed closeout=3 latest=assistant_responded risky_tool_runs=1 risky_tool_reviewed=1 risky_tool_missing_action_review=none gate_outcomes=total=18, protective_block=1, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=17 stop_reason=repeated_tool_failure stop_terminal_status=failed stop_action=recover stop_failure_type=unknown rollback_recommended=false rollback_completed=false recovery_failure_types=code_change_no_diff_after_repeated_progress,stale_read_conflict recovery_kinds=code_change_no_diff_replan,refresh_read_before_edit route_recovery=events=1, read_search=false, mutation_blocked=false, safety=true action_scores=17 latest_action_score=31 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=none runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=7 provider_protocol_repairs=301 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=5 context_zone_duplicate_blocks_removed=0 context_zone_provenance_markers=10 agent_loop_steps=14 context_zones=7 completion_contract=failed
runtime_spine_phase_coverage: 7/7
runtime_spine_observed_phases: context,decision,permission,tool_execution,state_update,verification,closeout
runtime_spine_assertions: none
runtime_spine_status: none
runtime_spine_missing: none
risky_tool_runs: 1
risky_tool_reviewed: 1
risky_tool_missing_action_review: none
gate_outcomes: total=18, protective_block=1, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=17
gate_outcome_records: action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,+6
gate_outcome_total: 18
gate_outcome_protective_blocks: 1
gate_outcome_recoverable_friction: 0
gate_outcome_unrecovered_blocks: 0
gate_outcome_suspected_false_positives: 0
gate_outcome_policy_correct_but_ux_costly: 0
gate_outcome_harmless_passes: 17
gate_outcome_failure_owners: none
agent_loop_steps: 14
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
verification_proof_summary: required validation failed 2/4 commands
verification_proof_kinds: none
verification_proof_support_status: failed
verification_proof_support_summary: verification proof status failed blocks verified closeout before proof-kind policy
verification_proof_supports_verified: false
verification_proof_residual_risk: true
risk_signal: entry=high runtime=high
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
memory_stale_demoted: true
memory_scope_correct: false
required_commands: 4
agent_required_commands: 4
harness_commands: 0
required_command_status: failed
validation_events: 2
stage_validation_events: 2
tool_progress_events: 1
guided_debugging_events: 3
guided_reasoning_events: 0
workflow_plan_events: 0
weighted_plan_events: 0
reweighted_plan_events: 0
adaptive_trigger_events: 5
adaptive_triggers: risk_signal_high,required_validation,repeated_no_code_progress,first_code_change,verification_failed
latest_top_priority: none
latest_top_importance_score: none
latest_top_weight_share: none
acceptance_accepted: missing
closeout_status: failed
closeout_tool_records: 83
closeout_tool_evidence: tool evidence: records=83 completed=15 failed=68 denied=0 validation=0 closeout=1 repair=69 changed=1 workflows=code_change commands=none
runtime_diet: prompt=26659 tool_schema=3950 tools=19 workflow=strict
attention: required commands did not pass in the harness
```

Agent stderr tail:

```text
[required validation still running after 30s] cargo test -q memory -- --test-threads=1
[required validation still running after 60s] cargo test -q memory -- --test-threads=1
[required validation still running after 90s] cargo test -q memory -- --test-threads=1
[required validation still running after 120s] cargo test -q memory -- --test-threads=1
[required validation still running after 150s] cargo test -q memory -- --test-threads=1
```

Agent monitor tail:

```text
[2026-05-28T14:07:40+0800] agent-run still running elapsed=30s idle_for=15s stdout_bytes=0 stderr_bytes=0 output_bytes=0 events_bytes=12946
[2026-05-28T14:08:10+0800] agent-run still running elapsed=60s idle_for=15s stdout_bytes=0 stderr_bytes=87 output_bytes=0 events_bytes=12946
[2026-05-28T14:08:40+0800] agent-run still running elapsed=90s idle_for=15s stdout_bytes=0 stderr_bytes=174 output_bytes=0 events_bytes=12946
[2026-05-28T14:09:10+0800] agent-run still running elapsed=120s idle_for=15s stdout_bytes=0 stderr_bytes=261 output_bytes=0 events_bytes=12946
[2026-05-28T14:09:40+0800] agent-run still running elapsed=150s idle_for=15s stdout_bytes=0 stderr_bytes=349 output_bytes=0 events_bytes=12946
[2026-05-28T14:10:11+0800] agent-run still running elapsed=180s idle_for=15s stdout_bytes=0 stderr_bytes=437 output_bytes=0 events_bytes=12946
[2026-05-28T14:10:41+0800] agent-run still running elapsed=210s idle_for=0s stdout_bytes=0 stderr_bytes=437 output_bytes=0 events_bytes=32064
[2026-05-28T14:11:11+0800] agent-run still running elapsed=240s idle_for=20s stdout_bytes=0 stderr_bytes=437 output_bytes=0 events_bytes=39494
[2026-05-28T14:11:41+0800] agent-run still running elapsed=270s idle_for=50s stdout_bytes=0 stderr_bytes=437 output_bytes=0 events_bytes=39494
[2026-05-28T14:12:11+0800] agent-run still running elapsed=300s idle_for=15s stdout_bytes=0 stderr_bytes=437 output_bytes=0 events_bytes=48038
[2026-05-28T14:12:41+0800] agent-run still running elapsed=330s idle_for=45s stdout_bytes=0 stderr_bytes=437 output_bytes=0 events_bytes=48038
[2026-05-28T14:13:11+0800] agent-run still running elapsed=360s idle_for=75s stdout_bytes=0 stderr_bytes=437 output_bytes=0 events_bytes=48038
[2026-05-28T14:13:41+0800] agent-run still running elapsed=390s idle_for=105s stdout_bytes=0 stderr_bytes=437 output_bytes=0 events_bytes=48038
[2026-05-28T14:14:11+0800] agent-run still running elapsed=420s idle_for=0s stdout_bytes=0 stderr_bytes=437 output_bytes=0 events_bytes=61750
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

- Bundle: `docs/benchmarks/live-coding-polish-real-20260528-123600/memory-stale-project-fact-demotion/run-bundle`
- Task: `docs/benchmarks/live-coding-polish-real-20260528-123600/memory-stale-project-fact-demotion/run-bundle/task.json`
- Steps: `docs/benchmarks/live-coding-polish-real-20260528-123600/memory-stale-project-fact-demotion/run-bundle/steps.jsonl`
- Events: `docs/benchmarks/live-coding-polish-real-20260528-123600/memory-stale-project-fact-demotion/run-bundle/events.jsonl`
- Final report: `docs/benchmarks/live-coding-polish-real-20260528-123600/memory-stale-project-fact-demotion/run-bundle/final_report.md`
