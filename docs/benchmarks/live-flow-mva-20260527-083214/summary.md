# Live Eval Summary: flow-mva-20260527-083214

- Run directory: `docs/benchmarks/live-flow-mva-20260527-083214`
- Tasks found: `7`
- Pass rate: `6/7` (85.7%)
- Failure rate: `1/7` (14.3%)
- Skipped/unscored tasks: `0`
- Real code-change passes: `1`
- Plan-only passes: `0`
- Seeded no-diff failures: `1`
- Memory active tasks: `5`
- Memory changed-plan tasks: `1`
- Memory recalled items: `0`
- Memory conflicts: `0`
- Memory typed-candidate tasks: `2`
- Memory evidence-backed candidate tasks: `2`
- Memory proposal tasks: `5`
- Memory proposal candidates: `2`
- Memory proposal evidence items: `19`
- Memory proposal review-required tasks: `5`
- Skill active tasks: `0`
- Skill promotion-evidence tasks: `0`
- Behavior assertion tasks: `0`
- Behavior assertions passed: `0`
- Runtime-spine assertion tasks: `7`
- Runtime-spine assertions passed: `6`
- Runtime-spine assertions failed: `1`
- Runtime-spine full coverage tasks: `0`
- Runtime-spine trace-present tasks: `6`
- Runtime-spine risky tool runs: `3`
- Runtime-spine risky tool reviewed: `2`
- Runtime-spine risky missing-review tasks: `1`
- Route recovery tasks: `0`
- Route recovery events: `0`
- Route recovery read/search expansions: `0`
- Route recovery mutation blocks: `0`
- Route recovery safety-monotonic tasks: `0`
- Route recovery unsafe mutation-expansion tasks: `0`
- Context-zone envelope tasks: `6`
- Context-zone envelope messages: `6`
- Context-zone source messages: `21`
- Context-zone duplicate blocks removed: `0`
- Context-zone provenance markers: `6`
- Gate outcome tasks: `5`
- Gate outcome records: `13`
- Gate outcome protective blocks: `0`
- Gate outcome recoverable friction: `0`
- Gate outcome unrecovered blocks: `1`
- Gate outcome harmless passes: `12`
- Proof support verified tasks: `1`
- Proof support partial tasks: `0`
- Proof support not-verified tasks: `1`
- Proof support residual-risk tasks: `1`
- Average outcome score: `85.7`
- Average process score: `92.9`
- Average efficiency score: `98.1`
- Average agent score: `90.3`
- Invalid actions total: `1`
- Premature edits total: `0`
- Scope drifts total: `0`
- Repeated actions total: `0`
- Failed actions total: `1`
- Coding gauntlet agent-run tasks: `7`
- Coding gauntlet passes: `6`
- Coding gauntlet failures: `1`
- Coding gauntlet likely clean passes: `1`
- Coding gauntlet repaired passes: `0`
- Coding gauntlet required-validation passes: `6/7`
- Coding gauntlet first-write observed: `1/7`
- Coding gauntlet repair signals: `0`
- Coding gauntlet changed files: `1`
- Status counts: failed=1, passed=6
- Failure owners: agent_flow=1, none=6
- Eval intents: audit_or_regression_check=1, direct_answer=1, read_only_audit=3, seeded_code_change=2

## Failure Modes

- `warning:no_code_diff`: `6`
- `warning:audit_no_code_diff`: `4`
- `closeout_not_successful`: `1`
- `empty_agent_output`: `1`
- `expected_code_diff_missing`: `1`
- `missing_trace_summary`: `1`
- `output_assertions_not_passing`: `1`
- `required_commands_not_passing`: `1`
- `runtime_spine_assertions_not_passing`: `1`
- `tool_run_without_closeout`: `1`
- `trajectory_assertions_not_passing`: `1`
- `warning:tool_errors_seen`: `1`

## Release Dogfood Failure Classes

| class | count | meaning |
|-------|-------|---------|
| desktop_evidence | 7 | Desktop UI, screenshot, native smoke, or visual evidence failures. |
| file_state | 7 | Read-before-edit, stale file, checkpoint, rollback, or diff-state failures. |
| tool_contract | 7 | Tool schema, exposure, result-pair, or contract boundary failures. |
| llm_reasoning | 6 | Model failed to plan, edit, validate, or close out despite available tools. |
| permission_recovery | 3 | Permission denial, approval, or recovery-loop failures. |
| runtime_spine | 1 | Unclassified failure class. |

## Memory And Skill Evidence

| dimension | count | meaning |
|-----------|-------|---------|
| memory_active_tasks | 5 | Tasks where retrieval, sync, or memory tools were active. |
| memory_changed_plan_tasks | 1 | Tasks where memory or learning signals reweighted planning. |
| memory_recalled_items | 0 | Retrieved memory-backed context items across tasks. |
| memory_conflicts | 0 | Retrieval-context conflict count from memory-backed context. |
| memory_candidate_typed_tasks | 2 | Tasks with typed memory candidates, including review-only MemoryProposal candidates. |
| memory_candidate_evidence_tasks | 2 | Tasks with evidence-backed memory candidates, including review-only MemoryProposal evidence. |
| memory_proposal_tasks | 5 | Tasks that emitted a review-only MemoryProposal trace event. |
| memory_proposal_candidates | 2 | Review-only MemoryProposal candidates proposed across tasks. |
| memory_proposal_evidence_items | 19 | Evidence items attached to review-only MemoryProposal candidates. |
| memory_proposal_review_required_tasks | 5 | MemoryProposal tasks that require review before persistence. |
| skill_active_tasks | 0 | Tasks where skill tools or skill-specific signals were active. |
| skill_promotion_evidence_tasks | 0 | Tasks with promotion-related skill evidence. |
| behavior_assertion_tasks | 0 | Tasks with explicit behavior assertions in the live-eval sample. |
| behavior_assertions_passed | 0 | Explicit behavior-assertion tasks whose required checks passed. |
| memory_behavior_assertion_tasks | 0 | Behavior assertions covering memory semantics rather than only memory activity signals. |
| skill_behavior_assertion_tasks | 0 | Behavior assertions covering skill semantics rather than only skill activity signals. |

## Runtime Spine Evidence

| dimension | count | meaning |
|-----------|-------|---------|
| runtime_spine_assertion_tasks | 7 | Tasks with explicit runtime-spine assertions in the live-eval sample or report. |
| runtime_spine_assertions_passed | 6 | Runtime-spine assertion tasks whose required trace/control-loop signals were present. |
| runtime_spine_assertions_failed | 1 | Runtime-spine assertion tasks missing required trace/control-loop signals. |
| runtime_spine_full_coverage_tasks | 0 | Tasks whose trace touched all runtime-spine phases. |
| runtime_spine_trace_present_tasks | 6 | Tasks with a trace summary available to the report parser. |
| runtime_spine_risky_tool_runs | 3 | Risky tool executions observed from trace or agent events. |
| runtime_spine_risky_tool_reviewed | 2 | Risky tool executions with matching action.review trace evidence. |
| runtime_spine_risky_missing_review_tasks | 1 | Tasks with risky tool executions missing matching action.review evidence. |
| route_recovery_tasks | 0 | Tasks with route-recovery plans emitted by the runtime. |
| route_recovery_events | 0 | Route-recovery plans observed across task traces. |
| route_recovery_read_search_expansions | 0 | Tasks where route recovery expanded only read/search understanding tools. |
| route_recovery_mutation_blocks | 0 | Tasks where route recovery explicitly blocked silent mutation expansion. |
| route_recovery_safety_monotonic_tasks | 0 | Tasks where route recovery preserved destructive-tool authority. |
| route_recovery_unsafe_mutation_expansion_tasks | 0 | Tasks where route recovery exposed mutation alternatives and should be investigated. |
| context_zone_envelope_tasks | 6 | Tasks where dynamic context was consolidated into a primary zone-first envelope. |
| context_zone_envelope_messages | 6 | Consolidated context-zone envelope messages observed across tasks. |
| context_zone_source_messages | 21 | Dynamic source messages consumed into context-zone envelopes. |
| context_zone_duplicate_blocks_removed | 0 | Duplicate dynamic zone blocks removed during request assembly. |
| context_zone_provenance_markers | 6 | Provenance markers preserved inside context-zone envelopes. |
| gate_outcome_tasks | 5 | Tasks with derived gate-outcome records from trace or report fields. |
| gate_outcome_records | 13 | Total gate-outcome records derived across action review, permission, and closeout gates. |
| gate_outcome_protective_blocks | 0 | Gate blocks that protected policy, scope, budget, checkpoint, or closeout invariants. |
| gate_outcome_recoverable_friction | 0 | Gate friction followed by a completed or passed runtime outcome. |
| gate_outcome_unrecovered_blocks | 1 | Gate blocks without later runtime recovery evidence. |
| gate_outcome_suspected_false_positives | 0 | Scenario-oracle suspected gate false positives. |
| gate_outcome_policy_correct_but_ux_costly | 0 | Policy-correct gate decisions that still created measurable UX cost. |
| gate_outcome_harmless_passes | 12 | Gate decisions that passed without measurable friction. |
| proof_support_verified_tasks | 1 | Tasks whose proof-kind policy supports verified closeout. |
| proof_support_partial_tasks | 0 | Tasks with useful proof evidence that cannot support verified closeout. |
| proof_support_not_verified_tasks | 1 | Tasks whose proof policy blocks verified closeout. |
| proof_support_residual_risk_tasks | 1 | Tasks whose proof support carries residual risk. |

### Gate Outcome Matrix

| task | total | protective | recoverable | unrecovered | suspected_false_positive | policy_correct_but_ux_costly | harmless | records | owners |
|------|-------|------------|-------------|-------------|--------------------------|------------------------------|----------|---------|--------|
| minimum-agent-direct-answer | 0 | 0 | 0 | 0 | 0 | 0 | 0 | none | none |
| minimum-agent-high-risk-block | 2 | 0 | 0 | 1 | 0 | 0 | 1 | action_review:allow:harmless_pass,closeout:not_verified:unrecovered_block | action_review |
| minimum-agent-light-inspection | 2 | 0 | 0 | 0 | 0 | 0 | 2 | action_review:allow:harmless_pass,closeout:passed:harmless_pass | none |
| minimum-agent-loop | 3 | 0 | 0 | 0 | 0 | 0 | 3 | action_review:allow:harmless_pass,action_review:allow:harmless_pass,closeout:passed:harmless_pass | none |
| minimum-agent-low-value-replan | 4 | 0 | 0 | 0 | 0 | 0 | 4 | action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,closeout:passed:harmless_pass | none |
| minimum-agent-memory-boundary | 2 | 0 | 0 | 0 | 0 | 0 | 2 | action_review:allow:harmless_pass,closeout:passed:harmless_pass | none |
| minimum-agent-verification-repair | 0 | 0 | 0 | 0 | 0 | 0 | 0 | none | none |

### Proof Support Matrix

| task | proof_status | support_status | supports_verified | residual_risk | proof_kinds | support_summary |
|------|--------------|----------------|-------------------|---------------|-------------|-----------------|
| minimum-agent-direct-answer | missing | missing | false | false | none | missing |
| minimum-agent-high-risk-block | not_run | not_run | false | true | none | verification proof status not_run blocks verified closeout before proof-kind policy |
| minimum-agent-light-inspection | not_applicable | not_applicable | false | false | none | no proof kind required for this task scope |
| minimum-agent-loop | verified | verified | true | false | command_passed,required_validation_passed | verified by command_passed,required_validation_passed |
| minimum-agent-low-value-replan | not_applicable | not_applicable | false | false | none | no proof kind required for this task scope |
| minimum-agent-memory-boundary | not_applicable | not_applicable | false | false | none | no proof kind required for this task scope |
| minimum-agent-verification-repair | missing | missing | false | false | none | missing |

### Context Zone Matrix

| task | materialized | envelopes | sources | dedupe_removed | provenance | task_state_empty | current_request_empty |
|------|--------------|-----------|---------|----------------|------------|------------------|-----------------------|
| minimum-agent-direct-answer | true | 1 | 2 | 0 | 0 | false | false |
| minimum-agent-high-risk-block | true | 1 | 5 | 0 | 3 | false | false |
| minimum-agent-light-inspection | true | 1 | 3 | 0 | 0 | false | false |
| minimum-agent-loop | true | 1 | 5 | 0 | 3 | false | false |
| minimum-agent-low-value-replan | true | 1 | 3 | 0 | 0 | false | false |
| minimum-agent-memory-boundary | true | 1 | 3 | 0 | 0 | false | false |
| minimum-agent-verification-repair | false | 0 | 0 | 0 | 0 | false | false |

### Route Recovery Matrix

| task | events | kinds | failure_types | read_search | mutation_blocked | safety_monotonic | unsafe_mutation_expansion | summary |
|------|--------|-------|---------------|-------------|------------------|------------------|---------------------------|---------|
| minimum-agent-direct-answer | 0 | none | none | false | false | missing | false | events=0, read_search=false, mutation_blocked=false, safety=missing |
| minimum-agent-high-risk-block | 0 | none | none | false | false | missing | false | events=0, read_search=false, mutation_blocked=false, safety=missing |
| minimum-agent-light-inspection | 0 | none | none | false | false | missing | false | events=0, read_search=false, mutation_blocked=false, safety=missing |
| minimum-agent-loop | 0 | none | none | false | false | missing | false | events=0, read_search=false, mutation_blocked=false, safety=missing |
| minimum-agent-low-value-replan | 0 | none | none | false | false | missing | false | events=0, read_search=false, mutation_blocked=false, safety=missing |
| minimum-agent-memory-boundary | 0 | none | none | false | false | missing | false | events=0, read_search=false, mutation_blocked=false, safety=missing |
| minimum-agent-verification-repair | 0 | none | none | false | false | missing | false | events=0, read_search=false, mutation_blocked=false, safety=missing |

## Evaluation Scores

| dimension | value | meaning |
|-----------|-------|---------|
| outcome_score_avg | 85.7 | Average deterministic outcome score across task reports. |
| process_score_avg | 92.9 | Average deterministic process score across task reports. |
| efficiency_score_avg | 98.1 | Average deterministic efficiency score across task reports. |
| agent_score_avg | 90.3 | Weighted score: outcome 50%, process 30%, efficiency 20%. |
| invalid_actions_total | 1 | Premature edits, scope drift, repeated actions, risky review gaps, and phase-misaligned actions. |
| premature_edits_total | 0 | Edits attempted before enough evidence or explicitly demoted as early/low-value. |
| scope_drifts_total | 0 | Action decisions with very low scope fit or medium/high goal drift. |
| repeated_actions_total | 0 | Repeated tool actions or repeated-action stop signals. |
| failed_actions_total | 1 | Failed tool/action observations from trace and event logs. |

### Score Matrix

| task | outcome | process | efficiency | agent | invalid | premature_edit | scope_drift | repeated | failed_actions | penalties |
|------|---------|---------|------------|-------|---------|----------------|-------------|----------|----------------|-----------|
| minimum-agent-direct-answer | 100 | 95 | 100 | 98 | 0 | 0 | 0 | 0 | 0 | stop_check_missing |
| minimum-agent-high-risk-block | 100 | 100 | 100 | 100 | 0 | 0 | 0 | 0 | 0 | none |
| minimum-agent-light-inspection | 100 | 100 | 100 | 100 | 0 | 0 | 0 | 0 | 0 | none |
| minimum-agent-loop | 100 | 100 | 100 | 100 | 0 | 0 | 0 | 0 | 0 | none |
| minimum-agent-low-value-replan | 100 | 100 | 100 | 100 | 0 | 0 | 0 | 0 | 0 | none |
| minimum-agent-memory-boundary | 100 | 100 | 100 | 100 | 0 | 0 | 0 | 0 | 0 | none |
| minimum-agent-verification-repair | 0 | 55 | 87 | 34 | 1 | 0 | 0 | 0 | 1 | run_failed,required_commands_failed,verification_failed,closeout_not_successful,runtime_spine_failed,output_assertions_failed,trajectory_assertions_failed,expected_code_diff_missing,invalid_action,risky_tool_missing_review,runtime_spine_not_passing,observer_outcome_missing,stop_check_missing,failed_actions,user_questions |

## Outcome Classes

| class | count | meaning |
|-------|-------|---------|
| real_code_change_passed | 1 | Agent-run tasks with passing status and a real diff. |
| plan_only_passed | 0 | Planning/API-only artifacts that passed their available checks. |
| seeded_no_diff_failed | 1 | Seeded code-change tasks where the agent did not produce a diff. |

## Coding Gauntlet Evidence

| task | gauntlet_status | first_pass_signal | failure_class | coding | required | closeout | spine | contract | risk | first_write | diff | warnings |
|------|-----------------|-------------------|---------------|--------|----------|----------|-------|----------|------|-------------|------|----------|
| minimum-agent-direct-answer | passed | no_write | tool_contract,file_state,llm_reasoning,desktop_evidence | tools=0, tool_records=0, validations=0, repair=0, files=0 | ok | passed | coverage=6/7, status=passed, missing=none | entry=skipped:force repair=none | entry=ordinary runtime=none | none | no | no_code_diff |
| minimum-agent-high-risk-block | passed | no_write | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | tools=1, tool_records=1, validations=0, repair=0, files=0 | ok | passed | coverage=6/7, status=passed, missing=none | entry=active:force repair=not_needed | entry=high runtime=none | none | no | no_code_diff,audit_no_code_diff |
| minimum-agent-light-inspection | passed | no_write | tool_contract,file_state,llm_reasoning,desktop_evidence | tools=1, tool_records=1, validations=0, repair=0, files=0 | ok | passed | coverage=6/7, status=passed, missing=none | entry=skipped:force repair=none | entry=ordinary runtime=none | none | no | no_code_diff,audit_no_code_diff |
| minimum-agent-loop | passed | likely_clean | tool_contract,file_state,permission_recovery,desktop_evidence | tools=2, tool_records=3, validations=2, repair=0, files=1 | ok | passed | coverage=6/7, status=passed, missing=none | entry=active:force repair=not_needed | entry=high runtime=none | 2 | yes | none |
| minimum-agent-low-value-replan | passed | no_write | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | tools=3, tool_records=5, validations=0, repair=0, files=0 | ok | passed | coverage=6/7, status=passed, missing=none | entry=skipped:force repair=none | entry=ordinary runtime=none | none | no | no_code_diff,audit_no_code_diff |
| minimum-agent-memory-boundary | passed | no_write | tool_contract,file_state,llm_reasoning,desktop_evidence | tools=1, tool_records=1, validations=0, repair=0, files=0 | ok | passed | coverage=6/7, status=passed, missing=none | entry=skipped:force repair=none | entry=ordinary runtime=none | none | no | no_code_diff,audit_no_code_diff |
| minimum-agent-verification-repair | failed | failed | runtime_spine,tool_contract,file_state,llm_reasoning,desktop_evidence | tools=1, tool_records=0, validations=0, repair=0, files=0 | failed | missing | coverage=0/7, status=missing, missing=phase:context,phase:decision,phase:tool_execution,phase:state_update,phase:verification,phase:closeout,event:context_zones_materialized,event:action_decision_evaluated,event:action_reviewed,event:agent_loop_step_evaluated,event:stop_check_evaluated,event:completion_contract_evaluated,completion_status:completed,terminal_status:completed,verification_proof_status:verified,special:verification_proof | entry=missing repair=none | entry=missing runtime=none | none | no | no_code_diff,tool_errors_seen |

## Task Matrix

| task | status | intent | owner | failure_class | required | plan_quality | tool_boundary | verification_status | closeout | runtime_spine | runtime_diet | contract | risk | behavior_assertions | behavior_status | triggers | first_write | diff | memory | skill | warnings |
|------|--------|--------|-------|---------------|----------|--------------|---------------|---------------------|----------|---------------|--------------|----------|------|---------------------|-----------------|----------|-------------|------|--------|-------|----------|
| minimum-agent-direct-answer | passed | direct_answer | none | tool_contract,file_state,llm_reasoning,desktop_evidence | ok | none | agent-run | passed | passed | coverage=6/7, status=passed, missing=none | prompt=2574 tool_schema=1069 tools=6 workflow=none closeout=none validation=none | entry=skipped:force repair=none | entry=ordinary runtime=none | none | none | none | none | no | active=false, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff |
| minimum-agent-high-risk-block | passed | audit_or_regression_check | none | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | ok | none | agent-run | passed | passed | coverage=6/7, status=passed, missing=none | prompt=4250 tool_schema=3950 tools=19 workflow=strict closeout=full validation=not_run | entry=active:force repair=not_needed | entry=high runtime=none | none | none | risk_signal_high | none | no | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff,audit_no_code_diff |
| minimum-agent-light-inspection | passed | read_only_audit | none | tool_contract,file_state,llm_reasoning,desktop_evidence | ok | none | agent-run | passed | passed | coverage=6/7, status=passed, missing=none | prompt=3298 tool_schema=1069 tools=6 workflow=none closeout=full validation=not_applicable | entry=skipped:force repair=none | entry=ordinary runtime=none | none | none | none | none | no | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff,audit_no_code_diff |
| minimum-agent-loop | passed | seeded_code_change | none | tool_contract,file_state,permission_recovery,desktop_evidence | ok | none | agent-run | passed | passed | coverage=6/7, status=passed, missing=none | prompt=4226 tool_schema=3950 tools=19 workflow=guarded closeout=full validation=passed:2/2 | entry=active:force repair=not_needed | entry=high runtime=none | none | none | risk_signal_high,required_validation,first_code_change | 2 | yes | active=true, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | none |
| minimum-agent-low-value-replan | passed | read_only_audit | none | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | ok | none | agent-run | passed | passed | coverage=6/7, status=passed, missing=none | prompt=3964 tool_schema=1069 tools=6 workflow=none closeout=full validation=not_applicable | entry=skipped:force repair=none | entry=ordinary runtime=none | none | none | none | none | no | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff,audit_no_code_diff |
| minimum-agent-memory-boundary | passed | read_only_audit | none | tool_contract,file_state,llm_reasoning,desktop_evidence | ok | none | agent-run | passed | passed | coverage=6/7, status=passed, missing=none | prompt=3108 tool_schema=1069 tools=6 workflow=none closeout=full validation=not_applicable | entry=skipped:force repair=none | entry=ordinary runtime=none | none | none | none | none | no | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff,audit_no_code_diff |
| minimum-agent-verification-repair | failed | seeded_code_change | agent_flow | runtime_spine,tool_contract,file_state,llm_reasoning,desktop_evidence | failed | none | agent-run | failed | missing | coverage=0/7, status=missing, missing=phase:context,phase:decision,phase:tool_execution,phase:state_update,phase:verification,phase:closeout,event:context_zones_materialized,event:action_decision_evaluated,event:action_reviewed,event:agent_loop_step_evaluated,event:stop_check_evaluated,event:completion_contract_evaluated,completion_status:completed,terminal_status:completed,verification_proof_status:verified,special:verification_proof | missing | entry=missing repair=none | entry=missing runtime=none | none | none | none | none | no | active=false, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff,tool_errors_seen |

## Notes

- `plan_quality` describes plan-only/API artifacts when present.
- `tool_boundary` separates plan-only, collect-only, and real agent-run reports.
- `verification_status` combines closeout and required-command evidence; it is not a human-quality score.
- `skipped` reports are excluded from pass/fail rate denominators; collect-only reports need passing required commands to be scored.
- `real_code_change_passed` requires an agent-run report with a non-empty diff; plan-only success is tracked separately.
- `memory` and `skill` summarize evidence signals; they do not by themselves mean the task succeeded.
- `behavior_assertions` are explicit sample-level checks; memory/skill behavior assertions are stronger evidence than activity signals alone.
- `runtime_spine` summarizes trace/control-loop coverage and explicit runtime-spine assertions.
