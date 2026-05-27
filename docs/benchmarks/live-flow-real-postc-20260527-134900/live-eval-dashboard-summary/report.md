# Live Eval Report: live-eval-dashboard-summary

- Run id: `flow-real-postc-20260527-134900`
- Sample: `evalsets/live_tasks/live-eval-dashboard-summary.yaml`
- Worktree: `target/live-evals/flow-real-postc-20260527-134900/live-eval-dashboard-summary/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/flow-real-postc-20260527-134900/live-eval-dashboard-summary/env`
- Test status: `ok`
- Generated: `2026-05-27 13:51:02 +0800`

## Git Status

```text
 M scripts/run_live_eval.sh
?? docs/benchmarks/live-live-summary-smoke/
```

## Diff Stat

```text
 scripts/run_live_eval.sh | 94 +++++++++++++++++++++++++++++++++++++++++++++---
 1 file changed, 89 insertions(+), 5 deletions(-)
 .../benchmarks/live-live-summary-smoke/summary.md      | 18 ++++++++++++++++++
 1 file changed, 18 insertions(+)
```

## Required Commands

```text
$ bash -n scripts/run_live_eval.sh
[exit status: 0]

$ scripts/run_live_eval.sh --list
id                                   type         eval_intent                risk       title
--                                   ----         -----------                ----       -----
backend-todo-api-crud                feature      seeded_code_change         medium     implement a tiny stdlib todo API backend
cli-scrollback-polish                ux           audit_or_regression_check  medium     interactive CLI should feel smooth and readable
code-change-verification-repair-loop feature      seeded_code_change         high       failed verification should trigger repair before closeout
core-inspection-grounding            audit        audit_or_regression_check  low        inspect filesystem facts without hallucinating metadata
core-long-output-artifact            runtime      audit_or_regression_check  medium     preserve and inspect long command output
core-multi-file-edit                 feature      seeded_code_change         medium     coordinate a two-file code and docs edit
core-permission-rejection-recovery   bug_fix      seeded_code_change         high       recover after user rejects a destructive cleanup
core-provider-roundtrip              protocol     audit_or_regression_check  medium     verify provider tool-call and tool-result protocol support
core-rollback-product-path           audit        audit_or_regression_check  medium     verify file history rollback is a product path
core-rust-multi-file-refactor        refactor     seeded_code_change         medium     multi-file Rust refactor with focused tests
core-simple-stale-edit               bug_fix      seeded_code_change         medium     read before a focused single-file edit
core-terminal-install-run            runtime      audit_or_regression_check  medium     install a local Python package and run it through the terminal
desktop-ui-smoke-polish              ux           audit_or_regression_check  medium     desktop UI smoke validation for runtime evidence
frontend-book-notes-localstorage     feature      seeded_code_change         medium     build a small book notes frontend with search, tags, and persistence
live-eval-dashboard-summary          feature      seeded_code_change         medium     live eval reports should summarize pass rates and failure modes
memory-failure-lesson-promotion      bug_fix      seeded_code_change         high       stop recovery failure lessons should become typed memory
memory-recall-conflict-precision     bug_fix      audit_or_regression_check  high       memory recall should demote only relevant conflicts
memory-save-duplicate-demotion       bug_fix      audit_or_regression_check  medium     duplicate memory candidates should not pollute long-term memory
memory-save-quality-gate             bug_fix      seeded_code_change         high       memory_save should respect quality gates
memory-save-sensitive-hard-block     bug_fix      audit_or_regression_check  high       explicit memory saves must not persist sensitive data
memory-stale-project-fact-demotion   bug_fix      seeded_code_change         high       stale project facts should be demoted before retrieval injection
minimum-agent-direct-answer          audit        direct_answer              low        minimum agent direct answer closes without tool use
minimum-agent-high-risk-block        audit        audit_or_regression_check  high       minimum agent blocks unsupported destructive request
minimum-agent-light-inspection       audit        read_only_audit            low        minimum agent light inspection answers from grounded local evidence
minimum-agent-loop                   feature      seeded_code_change         medium     minimum viable agent loop records route state action observation stop and completion
minimum-agent-low-value-replan       audit        read_only_audit            low        minimum agent stops or replans after repeated low-value search
minimum-agent-memory-boundary        audit        read_only_audit            medium     minimum agent memory boundary reads preference and closes with summary candidate
minimum-agent-verification-repair    bug_fix      seeded_code_change         medium     minimum agent repairs after observing failing validation
permission-default-open-dangerous-guard bug_fix      audit_or_regression_check  high       default-open permissions should still guard destructive operations
persistent-memory-planning-context   bug_fix      seeded_code_change         high       persistent memory should affect workflow planning
project-partner-failure-memory-proposal feature      seeded_code_change         medium     project partner turns a failed validation lesson into a review-only memory proposal
project-partner-resume-with-memory   audit        read_only_audit            low        project partner resumes from project memory and prior execution evidence
project-partner-vague-local-tool     feature      seeded_code_change         medium     project partner narrows a vague local tool idea into a scoped MVP
resume-session-picker                feature      seeded_code_change         medium     interactive CLI should support Claude-style resume
runtime-spine-p0b-isolated-worktree-implementer bug_fix      seeded_code_change         high       P0b implementer subagent changes require parent verified proof
runtime-spine-p0b-memory-retrieval-conflict audit        read_only_audit            medium     P0b conflicting memory is demoted below current workspace evidence
runtime-spine-p0b-permission-required bug_fix      seeded_code_change         high       P0b permission-required action is explicit and recoverable
runtime-spine-p0b-route-mistake-recovery bug_fix      seeded_code_change         medium     P0b route recovery expands understanding without silent mutation expansion
runtime-spine-p0b-skill-guidance     bug_fix      seeded_code_change         medium     P0b skill guidance stays background and validation still owns closeout
runtime-spine-p0b-subagent-verifier  audit        read_only_audit            medium     P0b subagent verifier claim remains non-authoritative without parent proof
runtime-spine-p0b-test-failure-repair bug_fix      seeded_code_change         medium     P0b failed validation re-enters context and triggers bounded repair
skill-promotion-gate                 bug_fix      seeded_code_change         medium     skill apply should require promotion evidence
[exit status: 0]

$ scripts/run_live_eval.sh --mode summary --run-id live-summary-smoke
Summary written to docs/benchmarks/live-live-summary-smoke/summary.md
[exit status: 0]

$ cargo test -q -- --test-threads=1

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
- Output: `docs/benchmarks/live-flow-real-postc-20260527-134900/live-eval-dashboard-summary/agent-output.md`
- Events: `docs/benchmarks/live-flow-real-postc-20260527-134900/live-eval-dashboard-summary/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
runtime_diagnostic: 1
start: 1
text_chunk: 1
tool_execution_complete: 5
tool_execution_progress: 1
tool_execution_start: 5
trace_summary: 1
```

Quality signals:

```text
output_chars: 1143
diff_chars: 4626
diff_files_changed: 2
tool_executions: 5
first_write_tool_index: 5
forbidden_tool_uses: none
tool_errors: 0
tool_failures: 0
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 83
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
closeout_tool_records: 10
closeout_tool_evidence: tool evidence: records=10 completed=5 failed=5 denied=0 validation=0 closeout=1 repair=6 changed=1 workflows=code_change commands=cat /Users/georgexu/Desktop/rust-agent/target/live-evals/flow-real-postc-20260527-134900/live-eval-dashboard-s...
runtime_diet: prompt=4956 tool_schema=3950 tools=19 workflow=guarded closeout=full validation=passed:5/5 recovered_failed:1
adaptive_triggers: risk_signal_high,required_validation,repeated_no_code_progress,first_code_change
risk_signal: entry=high runtime=none
risk_signal_reasons: required validation commands present; complex required-validation surface; broad validation command requested
trace_event_types: reflection.pass,stage.validation,acceptance.review,memory.boundary,memory.sync,workflow.fallback,closeout,execution.report,memory.proposal,runtime.diet,completion.contract,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: seeded_code_change
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
runtime_spine_detail: context=16 latest=runtime_diet_report decision=16 latest=adaptive_workflow_triggered permission=0 latest=none tool_execution=12 latest=tool_completed state_update=26 latest=workflow_fallback verification=5 latest=acceptance_review_completed closeout=3 latest=assistant_responded risky_tool_runs=2 risky_tool_reviewed=2 risky_tool_missing_action_review=none gate_outcomes=total=6, protective_block=0, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=6 stop_reason=no_issue stop_terminal_status=completed stop_action=continue stop_failure_type=unknown rollback_recommended=false rollback_completed=false recovery_failure_types=code_change_no_diff_after_repeated_progress recovery_kinds=code_change_no_diff_replan route_recovery=events=1, read_search=false, mutation_blocked=false, safety=true action_scores=3 latest_action_score=17 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=none runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=2 provider_protocol_repairs=4 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=4 context_zone_duplicate_blocks_removed=0 context_zone_provenance_markers=3 agent_loop_steps=4 context_zones=2 completion_contract=completed
runtime_spine_trace_present: true
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: none
runtime_spine_status: none
runtime_spine_missing: none
risky_tool_runs: 2
risky_tool_reviewed: 2
risky_tool_missing_action_review: none
gate_outcomes: total=6, protective_block=0, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=6
gate_outcome_records: action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,closeout:passed:harmless_pass
gate_outcome_total: 6
gate_outcome_protective_blocks: 0
gate_outcome_recoverable_friction: 0
gate_outcome_unrecovered_blocks: 0
gate_outcome_suspected_false_positives: 0
gate_outcome_policy_correct_but_ux_costly: 0
gate_outcome_harmless_passes: 6
gate_outcome_failure_owners: none
route_recovery: events=1, read_search=false, mutation_blocked=false, safety=true
route_recovery_events: 1
route_recovery_failure_types: code_change_no_diff_after_repeated_progress
route_recovery_kinds: code_change_no_diff_replan
route_recovery_read_search_expanded: false
route_recovery_mutation_blocked: false
route_recovery_safety_monotonic: true
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
completion_contract_status: completed
completion_contract_proof_status: verified
candidate_score_calibrated: false
candidate_score_disagreement: false
observer_outcome_recorded: true
memory_boundary_recorded: true
verification_proof_status: verified
verification_proof_summary: required validation passed 5/5 commands
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
failed_action_count: 0
user_question_count: 0
unnecessary_question_count: 0
verification_attempted: true
verification_passed: true
tool_call_count: 5
llm_call_count: 2
failure_owner: none
outcome_score: 100
process_score: 100
efficiency_score: 100
agent_score: 100
score_penalties: none
```

Specialty signals:

```text
memory_active: true
automation_active: true
guided_debugging_active: false
guided_reasoning_active: false
weighted_planning_active: false
closeout_active: true
adaptive_workflow_active: true
active_specialty_signals: 4/7
workflow_contract_activation: entry=active:force repair=not_needed
workflow_contract_events: 1
runtime_spine: coverage=6/7, status=none, missing=none
runtime_profile: none
mva_profile_active: false
runtime_spine_detail: context=16 latest=runtime_diet_report decision=16 latest=adaptive_workflow_triggered permission=0 latest=none tool_execution=12 latest=tool_completed state_update=26 latest=workflow_fallback verification=5 latest=acceptance_review_completed closeout=3 latest=assistant_responded risky_tool_runs=2 risky_tool_reviewed=2 risky_tool_missing_action_review=none gate_outcomes=total=6, protective_block=0, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=6 stop_reason=no_issue stop_terminal_status=completed stop_action=continue stop_failure_type=unknown rollback_recommended=false rollback_completed=false recovery_failure_types=code_change_no_diff_after_repeated_progress recovery_kinds=code_change_no_diff_replan route_recovery=events=1, read_search=false, mutation_blocked=false, safety=true action_scores=3 latest_action_score=17 low_action_score_count=0 phase_misaligned_actions=0 observer_modifier_applied=true memory_modifier_applied=false observer_outcome_recorded=true observer_quality_warnings=0 observer_quality_warning_labels=none permission_sources=none runtime_diet_warnings=prompt_budget_heavy provider_protocol_events=2 provider_protocol_repairs=4 streaming_tool_shadow_events=0 streaming_tool_shadow_eligible=0 memory_boundary_recorded=true task_contract_recorded=true context_pack_recorded=true execution_report_recorded=true memory_proposal_recorded=true context_zone_envelope_messages=1 context_zone_source_messages=4 context_zone_duplicate_blocks_removed=0 context_zone_provenance_markers=3 agent_loop_steps=4 context_zones=2 completion_contract=completed
runtime_spine_phase_coverage: 6/7
runtime_spine_observed_phases: context,decision,tool_execution,state_update,verification,closeout
runtime_spine_assertions: none
runtime_spine_status: none
runtime_spine_missing: none
risky_tool_runs: 2
risky_tool_reviewed: 2
risky_tool_missing_action_review: none
gate_outcomes: total=6, protective_block=0, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=6
gate_outcome_records: action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,closeout:passed:harmless_pass
gate_outcome_total: 6
gate_outcome_protective_blocks: 0
gate_outcome_recoverable_friction: 0
gate_outcome_unrecovered_blocks: 0
gate_outcome_suspected_false_positives: 0
gate_outcome_policy_correct_but_ux_costly: 0
gate_outcome_harmless_passes: 6
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
completion_contract_status: completed
completion_contract_proof_status: verified
candidate_score_calibrated: false
candidate_score_disagreement: false
observer_outcome_recorded: true
memory_boundary_recorded: true
verification_proof_status: verified
verification_proof_summary: required validation passed 5/5 commands
verification_proof_kinds: command_passed,required_validation_passed
verification_proof_support_status: verified
verification_proof_support_summary: verified by command_passed,required_validation_passed
verification_proof_supports_verified: true
verification_proof_residual_risk: false
risk_signal: entry=high runtime=none
risk_signal_reasons: required validation commands present; complex required-validation surface; broad validation command requested
memory_sync_events: 2
memory_tool_calls: 0
retrieval_sources: Project
memory_candidate_typed: true
memory_candidate_has_evidence: true
memory_proposal_recorded: true
memory_proposal_status: proposed
memory_proposal_candidates: 1
memory_proposal_kinds: successful_fix
memory_proposal_evidence_items: 7
memory_proposal_write_policy: review_required
memory_proposal_write_performed: false
memory_record_used: false
memory_use_count_updated: false
memory_failure_lesson_promoted: false
memory_action_weight_changed: false
memory_stale_demoted: false
memory_scope_correct: false
required_commands: 4
agent_required_commands: 4
harness_commands: 0
required_command_status: ok
validation_events: 1
stage_validation_events: 1
tool_progress_events: 1
guided_debugging_events: 0
guided_reasoning_events: 0
workflow_plan_events: 0
weighted_plan_events: 0
reweighted_plan_events: 0
adaptive_trigger_events: 4
adaptive_triggers: risk_signal_high,required_validation,repeated_no_code_progress,first_code_change
latest_top_priority: none
latest_top_importance_score: none
latest_top_weight_share: none
acceptance_accepted: True
closeout_status: passed
closeout_tool_records: 10
closeout_tool_evidence: tool evidence: records=10 completed=5 failed=5 denied=0 validation=0 closeout=1 repair=6 changed=1 workflows=code_change commands=cat /Users/georgexu/Desktop/rust-agent/target/live-evals/flow-real-postc-20260527-134900/live-eval-dashboard-s...
runtime_diet: prompt=4956 tool_schema=3950 tools=19 workflow=guarded
note: guided debugging is expected only after a blocker or failed validation
```

Agent stderr tail:

```text
[required validation still running after 30s] cargo test -q -- --test-threads=1
[required validation still running after 60s] cargo test -q -- --test-threads=1
[required validation still running after 90s] cargo test -q -- --test-threads=1
[required validation still running after 120s] cargo test -q -- --test-threads=1
[required validation still running after 150s] cargo test -q -- --test-threads=1
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

- Bundle: `docs/benchmarks/live-flow-real-postc-20260527-134900/live-eval-dashboard-summary/run-bundle`
- Task: `docs/benchmarks/live-flow-real-postc-20260527-134900/live-eval-dashboard-summary/run-bundle/task.json`
- Steps: `docs/benchmarks/live-flow-real-postc-20260527-134900/live-eval-dashboard-summary/run-bundle/steps.jsonl`
- Events: `docs/benchmarks/live-flow-real-postc-20260527-134900/live-eval-dashboard-summary/run-bundle/events.jsonl`
- Final report: `docs/benchmarks/live-flow-real-postc-20260527-134900/live-eval-dashboard-summary/run-bundle/final_report.md`
