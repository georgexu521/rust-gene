# Live Eval Summary: flow-real-20260527-084801

- Run directory: `docs/benchmarks/live-flow-real-20260527-084801`
- Tasks found: `11`
- Pass rate: `4/11` (36.4%)
- Failure rate: `7/11` (63.6%)
- Skipped/unscored tasks: `0`
- Real code-change passes: `4`
- Plan-only passes: `0`
- Seeded no-diff failures: `1`
- Memory active tasks: `11`
- Memory changed-plan tasks: `2`
- Memory recalled items: `0`
- Memory conflicts: `0`
- Memory typed-candidate tasks: `11`
- Memory evidence-backed candidate tasks: `11`
- Memory proposal tasks: `11`
- Memory proposal candidates: `11`
- Memory proposal evidence items: `107`
- Memory proposal review-required tasks: `11`
- Skill active tasks: `0`
- Skill promotion-evidence tasks: `0`
- Behavior assertion tasks: `0`
- Behavior assertions passed: `0`
- Runtime-spine assertion tasks: `4`
- Runtime-spine assertions passed: `4`
- Runtime-spine assertions failed: `0`
- Runtime-spine full coverage tasks: `0`
- Runtime-spine trace-present tasks: `11`
- Runtime-spine risky tool runs: `19`
- Runtime-spine risky tool reviewed: `19`
- Runtime-spine risky missing-review tasks: `0`
- Route recovery tasks: `3`
- Route recovery events: `5`
- Route recovery read/search expansions: `0`
- Route recovery mutation blocks: `1`
- Route recovery safety-monotonic tasks: `3`
- Route recovery unsafe mutation-expansion tasks: `0`
- Context-zone envelope tasks: `11`
- Context-zone envelope messages: `11`
- Context-zone source messages: `45`
- Context-zone duplicate blocks removed: `0`
- Context-zone provenance markers: `36`
- Gate outcome tasks: `11`
- Gate outcome records: `86`
- Gate outcome protective blocks: `14`
- Gate outcome recoverable friction: `0`
- Gate outcome unrecovered blocks: `0`
- Gate outcome harmless passes: `72`
- Proof support verified tasks: `4`
- Proof support partial tasks: `0`
- Proof support not-verified tasks: `7`
- Proof support residual-risk tasks: `7`
- Average outcome score: `51.4`
- Average process score: `90.2`
- Average efficiency score: `85.2`
- Average agent score: `69.9`
- Invalid actions total: `15`
- Premature edits total: `0`
- Scope drifts total: `0`
- Repeated actions total: `11`
- Failed actions total: `15`
- Coding gauntlet agent-run tasks: `11`
- Coding gauntlet passes: `4`
- Coding gauntlet failures: `7`
- Coding gauntlet likely clean passes: `3`
- Coding gauntlet repaired passes: `1`
- Coding gauntlet required-validation passes: `7/11`
- Coding gauntlet first-write observed: `5/11`
- Coding gauntlet repair signals: `15`
- Coding gauntlet changed files: `6`
- Status counts: failed=7, passed=4
- Failure owners: agent_flow=4, llm_reasoning=1, mixed=2, none=4
- Eval intents: audit_or_regression_check=5, seeded_code_change=6

## Failure Modes

- `closeout_not_successful`: `7`
- `warning:no_code_diff`: `6`
- `warning:audit_no_code_diff`: `5`
- `required_commands_not_passing`: `4`
- `warning:tool_errors_seen`: `3`
- `action_checkpoint_invalid_tools`: `1`
- `earlier_stage_validation_failed_before_repair`: `1`
- `earlier_verification_failed_before_repair`: `1`
- `expected_code_diff_missing`: `1`
- `stage_validation_failed`: `1`
- `verification_failed`: `1`
- `warning:action_checkpoint_invalid_tools`: `1`

## Release Dogfood Failure Classes

| class | count | meaning |
|-------|-------|---------|
| desktop_evidence | 11 | Desktop UI, screenshot, native smoke, or visual evidence failures. |
| file_state | 11 | Read-before-edit, stale file, checkpoint, rollback, or diff-state failures. |
| permission_recovery | 11 | Permission denial, approval, or recovery-loop failures. |
| tool_contract | 11 | Tool schema, exposure, result-pair, or contract boundary failures. |
| llm_reasoning | 7 | Model failed to plan, edit, validate, or close out despite available tools. |

## Memory And Skill Evidence

| dimension | count | meaning |
|-----------|-------|---------|
| memory_active_tasks | 11 | Tasks where retrieval, sync, or memory tools were active. |
| memory_changed_plan_tasks | 2 | Tasks where memory or learning signals reweighted planning. |
| memory_recalled_items | 0 | Retrieved memory-backed context items across tasks. |
| memory_conflicts | 0 | Retrieval-context conflict count from memory-backed context. |
| memory_candidate_typed_tasks | 11 | Tasks with typed memory candidates, including review-only MemoryProposal candidates. |
| memory_candidate_evidence_tasks | 11 | Tasks with evidence-backed memory candidates, including review-only MemoryProposal evidence. |
| memory_proposal_tasks | 11 | Tasks that emitted a review-only MemoryProposal trace event. |
| memory_proposal_candidates | 11 | Review-only MemoryProposal candidates proposed across tasks. |
| memory_proposal_evidence_items | 107 | Evidence items attached to review-only MemoryProposal candidates. |
| memory_proposal_review_required_tasks | 11 | MemoryProposal tasks that require review before persistence. |
| skill_active_tasks | 0 | Tasks where skill tools or skill-specific signals were active. |
| skill_promotion_evidence_tasks | 0 | Tasks with promotion-related skill evidence. |
| behavior_assertion_tasks | 0 | Tasks with explicit behavior assertions in the live-eval sample. |
| behavior_assertions_passed | 0 | Explicit behavior-assertion tasks whose required checks passed. |
| memory_behavior_assertion_tasks | 0 | Behavior assertions covering memory semantics rather than only memory activity signals. |
| skill_behavior_assertion_tasks | 0 | Behavior assertions covering skill semantics rather than only skill activity signals. |

## Runtime Spine Evidence

| dimension | count | meaning |
|-----------|-------|---------|
| runtime_spine_assertion_tasks | 4 | Tasks with explicit runtime-spine assertions in the live-eval sample or report. |
| runtime_spine_assertions_passed | 4 | Runtime-spine assertion tasks whose required trace/control-loop signals were present. |
| runtime_spine_assertions_failed | 0 | Runtime-spine assertion tasks missing required trace/control-loop signals. |
| runtime_spine_full_coverage_tasks | 0 | Tasks whose trace touched all runtime-spine phases. |
| runtime_spine_trace_present_tasks | 11 | Tasks with a trace summary available to the report parser. |
| runtime_spine_risky_tool_runs | 19 | Risky tool executions observed from trace or agent events. |
| runtime_spine_risky_tool_reviewed | 19 | Risky tool executions with matching action.review trace evidence. |
| runtime_spine_risky_missing_review_tasks | 0 | Tasks with risky tool executions missing matching action.review evidence. |
| route_recovery_tasks | 3 | Tasks with route-recovery plans emitted by the runtime. |
| route_recovery_events | 5 | Route-recovery plans observed across task traces. |
| route_recovery_read_search_expansions | 0 | Tasks where route recovery expanded only read/search understanding tools. |
| route_recovery_mutation_blocks | 1 | Tasks where route recovery explicitly blocked silent mutation expansion. |
| route_recovery_safety_monotonic_tasks | 3 | Tasks where route recovery preserved destructive-tool authority. |
| route_recovery_unsafe_mutation_expansion_tasks | 0 | Tasks where route recovery exposed mutation alternatives and should be investigated. |
| context_zone_envelope_tasks | 11 | Tasks where dynamic context was consolidated into a primary zone-first envelope. |
| context_zone_envelope_messages | 11 | Consolidated context-zone envelope messages observed across tasks. |
| context_zone_source_messages | 45 | Dynamic source messages consumed into context-zone envelopes. |
| context_zone_duplicate_blocks_removed | 0 | Duplicate dynamic zone blocks removed during request assembly. |
| context_zone_provenance_markers | 36 | Provenance markers preserved inside context-zone envelopes. |
| gate_outcome_tasks | 11 | Tasks with derived gate-outcome records from trace or report fields. |
| gate_outcome_records | 86 | Total gate-outcome records derived across action review, permission, and closeout gates. |
| gate_outcome_protective_blocks | 14 | Gate blocks that protected policy, scope, budget, checkpoint, or closeout invariants. |
| gate_outcome_recoverable_friction | 0 | Gate friction followed by a completed or passed runtime outcome. |
| gate_outcome_unrecovered_blocks | 0 | Gate blocks without later runtime recovery evidence. |
| gate_outcome_suspected_false_positives | 0 | Scenario-oracle suspected gate false positives. |
| gate_outcome_policy_correct_but_ux_costly | 0 | Policy-correct gate decisions that still created measurable UX cost. |
| gate_outcome_harmless_passes | 72 | Gate decisions that passed without measurable friction. |
| proof_support_verified_tasks | 4 | Tasks whose proof-kind policy supports verified closeout. |
| proof_support_partial_tasks | 0 | Tasks with useful proof evidence that cannot support verified closeout. |
| proof_support_not_verified_tasks | 7 | Tasks whose proof policy blocks verified closeout. |
| proof_support_residual_risk_tasks | 7 | Tasks whose proof support carries residual risk. |

### Gate Outcome Matrix

| task | total | protective | recoverable | unrecovered | suspected_false_positive | policy_correct_but_ux_costly | harmless | records | owners |
|------|-------|------------|-------------|-------------|--------------------------|------------------------------|----------|---------|--------|
| backend-todo-api-crud | 13 | 1 | 0 | 0 | 0 | 0 | 12 | action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,+1 | none |
| code-change-verification-repair-loop | 6 | 0 | 0 | 0 | 0 | 0 | 6 | action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,closeout:passed:harmless_pass | none |
| core-inspection-grounding | 6 | 5 | 0 | 0 | 0 | 0 | 1 | action_review:allow:harmless_pass,action_review:revise:protective_block,action_review:revise:protective_block,action_review:revise:protective_block,action_review:revise:protective_block,closeout:failed:protective_block | none |
| core-long-output-artifact | 4 | 2 | 0 | 0 | 0 | 0 | 2 | action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:revise:protective_block,closeout:not_verified:protective_block | none |
| core-multi-file-edit | 5 | 0 | 0 | 0 | 0 | 0 | 5 | action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,closeout:passed:harmless_pass | none |
| core-permission-rejection-recovery | 4 | 2 | 0 | 0 | 0 | 0 | 2 | action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:revise:protective_block,closeout:not_verified:protective_block | none |
| core-provider-roundtrip | 6 | 1 | 0 | 0 | 0 | 0 | 5 | action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,closeout:not_verified:protective_block | none |
| core-rollback-product-path | 24 | 1 | 0 | 0 | 0 | 0 | 23 | action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,+12 | none |
| core-simple-stale-edit | 4 | 0 | 0 | 0 | 0 | 0 | 4 | action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,closeout:passed:harmless_pass | none |
| core-terminal-install-run | 6 | 2 | 0 | 0 | 0 | 0 | 4 | action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:revise:protective_block,closeout:not_verified:protective_block | none |
| frontend-book-notes-localstorage | 8 | 0 | 0 | 0 | 0 | 0 | 8 | action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,closeout:passed:harmless_pass | none |

### Proof Support Matrix

| task | proof_status | support_status | supports_verified | residual_risk | proof_kinds | support_summary |
|------|--------------|----------------|-------------------|---------------|-------------|-----------------|
| backend-todo-api-crud | failed | failed | false | true | none | verification proof status failed blocks verified closeout before proof-kind policy |
| code-change-verification-repair-loop | verified | verified | true | false | command_passed,required_validation_passed | verified by command_passed,required_validation_passed |
| core-inspection-grounding | failed | failed | false | true | none | verification proof status failed blocks verified closeout before proof-kind policy |
| core-long-output-artifact | not_run | not_run | false | true | none | verification proof status not_run blocks verified closeout before proof-kind policy |
| core-multi-file-edit | verified | verified | true | false | command_passed,required_validation_passed | verified by command_passed,required_validation_passed |
| core-permission-rejection-recovery | not_run | not_run | false | true | none | verification proof status not_run blocks verified closeout before proof-kind policy |
| core-provider-roundtrip | not_run | not_run | false | true | none | verification proof status not_run blocks verified closeout before proof-kind policy |
| core-rollback-product-path | not_run | not_run | false | true | none | verification proof status not_run blocks verified closeout before proof-kind policy |
| core-simple-stale-edit | verified | verified | true | false | command_passed,required_validation_passed | verified by command_passed,required_validation_passed |
| core-terminal-install-run | not_run | not_run | false | true | none | verification proof status not_run blocks verified closeout before proof-kind policy |
| frontend-book-notes-localstorage | verified | verified | true | false | command_passed,required_validation_passed | verified by command_passed,required_validation_passed |

### Context Zone Matrix

| task | materialized | envelopes | sources | dedupe_removed | provenance | task_state_empty | current_request_empty |
|------|--------------|-----------|---------|----------------|------------|------------------|-----------------------|
| backend-todo-api-crud | true | 1 | 4 | 0 | 3 | false | false |
| code-change-verification-repair-loop | true | 1 | 4 | 0 | 4 | false | false |
| core-inspection-grounding | true | 1 | 4 | 0 | 4 | false | false |
| core-long-output-artifact | true | 1 | 4 | 0 | 3 | false | false |
| core-multi-file-edit | true | 1 | 4 | 0 | 3 | false | false |
| core-permission-rejection-recovery | true | 1 | 4 | 0 | 3 | false | false |
| core-provider-roundtrip | true | 1 | 4 | 0 | 3 | false | false |
| core-rollback-product-path | true | 1 | 4 | 0 | 3 | false | false |
| core-simple-stale-edit | true | 1 | 4 | 0 | 3 | false | false |
| core-terminal-install-run | true | 1 | 4 | 0 | 3 | false | false |
| frontend-book-notes-localstorage | true | 1 | 5 | 0 | 4 | false | false |

### Route Recovery Matrix

| task | events | kinds | failure_types | read_search | mutation_blocked | safety_monotonic | unsafe_mutation_expansion | summary |
|------|--------|-------|---------------|-------------|------------------|------------------|---------------------------|---------|
| backend-todo-api-crud | 3 | code_change_no_diff_replan | code_change_no_diff_after_repeated_progress | false | false | true | false | events=3, read_search=false, mutation_blocked=false, safety=true |
| code-change-verification-repair-loop | 0 | none | none | false | false | missing | false | events=0, read_search=false, mutation_blocked=false, safety=missing |
| core-inspection-grounding | 0 | none | none | false | false | missing | false | events=0, read_search=false, mutation_blocked=false, safety=missing |
| core-long-output-artifact | 0 | none | none | false | false | missing | false | events=0, read_search=false, mutation_blocked=false, safety=missing |
| core-multi-file-edit | 0 | none | none | false | false | missing | false | events=0, read_search=false, mutation_blocked=false, safety=missing |
| core-permission-rejection-recovery | 1 | no_silent_mutation_expansion | hidden_mutation_tool_requested | false | true | true | false | events=1, read_search=false, mutation_blocked=true, safety=true |
| core-provider-roundtrip | 0 | none | none | false | false | missing | false | events=0, read_search=false, mutation_blocked=false, safety=missing |
| core-rollback-product-path | 0 | none | none | false | false | missing | false | events=0, read_search=false, mutation_blocked=false, safety=missing |
| core-simple-stale-edit | 0 | none | none | false | false | missing | false | events=0, read_search=false, mutation_blocked=false, safety=missing |
| core-terminal-install-run | 0 | none | none | false | false | missing | false | events=0, read_search=false, mutation_blocked=false, safety=missing |
| frontend-book-notes-localstorage | 1 | code_change_no_diff_replan | code_change_no_diff_after_repeated_progress | false | false | true | false | events=1, read_search=false, mutation_blocked=false, safety=true |

## Evaluation Scores

| dimension | value | meaning |
|-----------|-------|---------|
| outcome_score_avg | 51.4 | Average deterministic outcome score across task reports. |
| process_score_avg | 90.2 | Average deterministic process score across task reports. |
| efficiency_score_avg | 85.2 | Average deterministic efficiency score across task reports. |
| agent_score_avg | 69.9 | Weighted score: outcome 50%, process 30%, efficiency 20%. |
| invalid_actions_total | 15 | Premature edits, scope drift, repeated actions, risky review gaps, and phase-misaligned actions. |
| premature_edits_total | 0 | Edits attempted before enough evidence or explicitly demoted as early/low-value. |
| scope_drifts_total | 0 | Action decisions with very low scope fit or medium/high goal drift. |
| repeated_actions_total | 11 | Repeated tool actions or repeated-action stop signals. |
| failed_actions_total | 15 | Failed tool/action observations from trace and event logs. |

### Score Matrix

| task | outcome | process | efficiency | agent | invalid | premature_edit | scope_drift | repeated | failed_actions | penalties |
|------|---------|---------|------------|-------|---------|----------------|-------------|----------|----------------|-----------|
| backend-todo-api-crud | 15 | 60 | 80 | 42 | 7 | 0 | 0 | 7 | 0 | run_failed,required_commands_failed,verification_failed,closeout_not_successful,repeated_action,invalid_action,repeated_actions |
| code-change-verification-repair-loop | 100 | 100 | 100 | 100 | 0 | 0 | 0 | 0 | 0 | none |
| core-inspection-grounding | 40 | 80 | 75 | 59 | 4 | 0 | 0 | 0 | 4 | run_failed,verification_failed,closeout_not_successful,invalid_action,failed_actions |
| core-long-output-artifact | 15 | 100 | 92 | 56 | 0 | 0 | 0 | 0 | 1 | run_failed,required_commands_failed,verification_failed,closeout_not_successful,failed_actions |
| core-multi-file-edit | 100 | 100 | 100 | 100 | 0 | 0 | 0 | 0 | 0 | none |
| core-permission-rejection-recovery | 0 | 100 | 92 | 48 | 0 | 0 | 0 | 0 | 1 | run_failed,required_commands_failed,verification_failed,closeout_not_successful,expected_code_diff_missing,failed_actions |
| core-provider-roundtrip | 40 | 100 | 84 | 67 | 0 | 0 | 0 | 0 | 2 | run_failed,verification_failed,closeout_not_successful,failed_actions |
| core-rollback-product-path | 40 | 87 | 58 | 58 | 1 | 0 | 0 | 1 | 4 | run_failed,verification_failed,closeout_not_successful,repeated_action,invalid_action,failed_actions,repeated_actions,llm_call_budget_pressure |
| core-simple-stale-edit | 100 | 100 | 84 | 97 | 0 | 0 | 0 | 0 | 2 | failed_actions |
| core-terminal-install-run | 15 | 100 | 92 | 56 | 0 | 0 | 0 | 0 | 1 | run_failed,required_commands_failed,verification_failed,closeout_not_successful,failed_actions |
| frontend-book-notes-localstorage | 100 | 65 | 80 | 86 | 3 | 0 | 0 | 3 | 0 | repeated_action,invalid_action,repeated_actions |

## Outcome Classes

| class | count | meaning |
|-------|-------|---------|
| real_code_change_passed | 4 | Agent-run tasks with passing status and a real diff. |
| plan_only_passed | 0 | Planning/API-only artifacts that passed their available checks. |
| seeded_no_diff_failed | 1 | Seeded code-change tasks where the agent did not produce a diff. |

## Coding Gauntlet Evidence

| task | gauntlet_status | first_pass_signal | failure_class | coding | required | closeout | spine | contract | risk | first_write | diff | warnings |
|------|-----------------|-------------------|---------------|--------|----------|----------|-------|----------|------|-------------|------|----------|
| backend-todo-api-crud | failed | failed | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | tools=12, tool_records=62, validations=4, repair=0, files=1 | failed | failed | coverage=6/7, status=none, missing=none | entry=active:force repair=active_after_failure | entry=high runtime=high | 7 | yes | action_checkpoint_invalid_tools |
| code-change-verification-repair-loop | passed | likely_clean | tool_contract,file_state,permission_recovery,desktop_evidence | tools=5, tool_records=11, validations=2, repair=0, files=1 | ok | passed | coverage=6/7, status=passed, missing=none | entry=active:force repair=not_needed | entry=high runtime=none | 5 | yes | none |
| core-inspection-grounding | failed | failed | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | tools=1, tool_records=5, validations=0, repair=5, files=0 | ok | failed | coverage=6/7, status=passed, missing=none | entry=active:force repair=not_needed | entry=high runtime=none | none | no | no_code_diff,audit_no_code_diff |
| core-long-output-artifact | failed | failed | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | tools=2, tool_records=4, validations=0, repair=1, files=0 | failed | not_verified | coverage=6/7, status=none, missing=none | entry=active:force repair=active_after_failure | entry=high runtime=high | none | no | no_code_diff,audit_no_code_diff |
| core-multi-file-edit | passed | likely_clean | tool_contract,file_state,permission_recovery,desktop_evidence | tools=4, tool_records=7, validations=2, repair=0, files=2 | ok | passed | coverage=6/7, status=passed, missing=none | entry=active:force repair=not_needed | entry=high runtime=none | 4 | yes | none |
| core-permission-rejection-recovery | failed | failed | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | tools=2, tool_records=5, validations=0, repair=1, files=0 | failed | not_verified | coverage=6/7, status=none, missing=none | entry=active:force repair=active_after_failure | entry=high runtime=high | none | no | no_code_diff |
| core-provider-roundtrip | failed | failed | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | tools=5, tool_records=11, validations=0, repair=2, files=0 | ok | not_verified | coverage=6/7, status=none, missing=none | entry=active:force repair=active_after_failure | entry=high runtime=high | none | no | no_code_diff,audit_no_code_diff,tool_errors_seen |
| core-rollback-product-path | failed | failed | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | tools=23, tool_records=144, validations=0, repair=3, files=0 | ok | not_verified | coverage=6/7, status=none, missing=none | entry=active:force repair=not_needed | entry=high runtime=none | none | no | no_code_diff,audit_no_code_diff,tool_errors_seen |
| core-simple-stale-edit | passed | repaired | tool_contract,file_state,permission_recovery,desktop_evidence | tools=3, tool_records=4, validations=2, repair=2, files=1 | ok | passed | coverage=6/7, status=passed, missing=none | entry=active:force repair=not_needed | entry=high runtime=none | 3 | yes | tool_errors_seen |
| core-terminal-install-run | failed | failed | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | tools=4, tool_records=8, validations=0, repair=1, files=0 | failed | not_verified | coverage=6/7, status=none, missing=none | entry=active:force repair=active_after_failure | entry=high runtime=high | none | no | no_code_diff,audit_no_code_diff |
| frontend-book-notes-localstorage | passed | likely_clean | tool_contract,file_state,permission_recovery,desktop_evidence | tools=7, tool_records=27, validations=2, repair=0, files=1 | ok | passed | coverage=6/7, status=none, missing=none | entry=active:force repair=not_needed | entry=high runtime=none | 7 | yes | none |

## Task Matrix

| task | status | intent | owner | failure_class | required | plan_quality | tool_boundary | verification_status | closeout | runtime_spine | runtime_diet | contract | risk | behavior_assertions | behavior_status | triggers | first_write | diff | memory | skill | warnings |
|------|--------|--------|-------|---------------|----------|--------------|---------------|---------------------|----------|---------------|--------------|----------|------|---------------------|-----------------|----------|-------------|------|--------|-------|----------|
| backend-todo-api-crud | failed | seeded_code_change | agent_flow | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | failed | none | agent-run | failed | failed | coverage=6/7, status=none, missing=none | prompt=15894 tool_schema=3950 tools=19 workflow=strict closeout=full validation=failed:1/2 | entry=active:force repair=active_after_failure | entry=high runtime=high | none | none | risk_signal_high,required_validation,repeated_no_code_progress,first_code_change,verification_failed | 7 | yes | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | action_checkpoint_invalid_tools |
| code-change-verification-repair-loop | passed | seeded_code_change | none | tool_contract,file_state,permission_recovery,desktop_evidence | ok | none | agent-run | passed | passed | coverage=6/7, status=passed, missing=none | prompt=14154 tool_schema=3950 tools=19 workflow=strict closeout=full validation=passed:5/5 | entry=active:force repair=not_needed | entry=high runtime=none | none | none | risk_signal_high,required_validation,first_code_change | 5 | yes | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | none |
| core-inspection-grounding | failed | audit_or_regression_check | agent_flow | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | ok | none | agent-run | failed | failed | coverage=6/7, status=passed, missing=none | prompt=7581 tool_schema=3950 tools=19 workflow=guarded closeout=full validation=failed:4/4 | entry=active:force repair=not_needed | entry=high runtime=none | none | none | risk_signal_high,required_validation | none | no | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff,audit_no_code_diff |
| core-long-output-artifact | failed | audit_or_regression_check | mixed | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | failed | none | agent-run | failed | not_verified | coverage=6/7, status=none, missing=none | prompt=4972 tool_schema=3950 tools=19 workflow=guarded closeout=full validation=not_run | entry=active:force repair=active_after_failure | entry=high runtime=high | none | none | risk_signal_high,required_validation | none | no | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff,audit_no_code_diff |
| core-multi-file-edit | passed | seeded_code_change | none | tool_contract,file_state,permission_recovery,desktop_evidence | ok | none | agent-run | passed | passed | coverage=6/7, status=passed, missing=none | prompt=5455 tool_schema=3950 tools=19 workflow=guarded closeout=full validation=passed:3/3 | entry=active:force repair=not_needed | entry=high runtime=none | none | none | risk_signal_high,required_validation,first_code_change | 4 | yes | active=true, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | none |
| core-permission-rejection-recovery | failed | seeded_code_change | llm_reasoning | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | failed | none | agent-run | failed | not_verified | coverage=6/7, status=none, missing=none | prompt=4744 tool_schema=3950 tools=19 workflow=strict closeout=full validation=not_run | entry=active:force repair=active_after_failure | entry=high runtime=high | none | none | risk_signal_high,required_validation | none | no | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff |
| core-provider-roundtrip | failed | audit_or_regression_check | agent_flow | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | ok | none | agent-run | failed | not_verified | coverage=6/7, status=none, missing=none | prompt=16697 tool_schema=3950 tools=19 workflow=guarded closeout=full validation=not_run | entry=active:force repair=active_after_failure | entry=high runtime=high | none | none | risk_signal_high,required_validation | none | no | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff,audit_no_code_diff,tool_errors_seen |
| core-rollback-product-path | failed | audit_or_regression_check | agent_flow | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | ok | none | agent-run | failed | not_verified | coverage=6/7, status=none, missing=none | prompt=27045 tool_schema=3950 tools=19 workflow=guarded closeout=full validation=not_run | entry=active:force repair=not_needed | entry=high runtime=none | none | none | risk_signal_high,required_validation | none | no | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff,audit_no_code_diff,tool_errors_seen |
| core-simple-stale-edit | passed | seeded_code_change | none | tool_contract,file_state,permission_recovery,desktop_evidence | ok | none | agent-run | passed | passed | coverage=6/7, status=passed, missing=none | prompt=5190 tool_schema=3950 tools=19 workflow=guarded closeout=full validation=passed:2/2 recovered_failed:1 | entry=active:force repair=not_needed | entry=high runtime=none | none | none | risk_signal_high,required_validation,first_code_change | 3 | yes | active=true, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | tool_errors_seen |
| core-terminal-install-run | failed | audit_or_regression_check | mixed | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | failed | none | agent-run | failed | not_verified | coverage=6/7, status=none, missing=none | prompt=6176 tool_schema=4300 tools=20 workflow=guarded closeout=full validation=not_run | entry=active:force repair=active_after_failure | entry=high runtime=high | none | none | risk_signal_high,required_validation | none | no | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff,audit_no_code_diff |
| frontend-book-notes-localstorage | passed | seeded_code_change | none | tool_contract,file_state,permission_recovery,desktop_evidence | ok | none | agent-run | passed | passed | coverage=6/7, status=none, missing=none | prompt=8534 tool_schema=3950 tools=19 workflow=guarded closeout=full validation=passed:2/2 | entry=active:force repair=not_needed | entry=high runtime=none | none | none | risk_signal_high,required_validation,repeated_no_code_progress,first_code_change | 7 | yes | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | none |

## Notes

- `plan_quality` describes plan-only/API artifacts when present.
- `tool_boundary` separates plan-only, collect-only, and real agent-run reports.
- `verification_status` combines closeout and required-command evidence; it is not a human-quality score.
- `skipped` reports are excluded from pass/fail rate denominators; collect-only reports need passing required commands to be scored.
- `real_code_change_passed` requires an agent-run report with a non-empty diff; plan-only success is tracked separately.
- `memory` and `skill` summarize evidence signals; they do not by themselves mean the task succeeded.
- `behavior_assertions` are explicit sample-level checks; memory/skill behavior assertions are stronger evidence than activity signals alone.
- `runtime_spine` summarizes trace/control-loop coverage and explicit runtime-spine assertions.
