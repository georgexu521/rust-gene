# Live Eval Summary: flow-real-postc-20260527-134900

- Run directory: `docs/benchmarks/live-flow-real-postc-20260527-134900`
- Tasks found: `14`
- Pass rate: `9/14` (64.3%)
- Failure rate: `5/14` (35.7%)
- Skipped/unscored tasks: `0`
- Real code-change passes: `6`
- Plan-only passes: `0`
- Seeded no-diff failures: `4`
- Memory active tasks: `14`
- Memory changed-plan tasks: `4`
- Memory recalled items: `0`
- Memory conflicts: `0`
- Memory typed-candidate tasks: `10`
- Memory evidence-backed candidate tasks: `10`
- Memory proposal tasks: `14`
- Memory proposal candidates: `10`
- Memory proposal evidence items: `114`
- Memory proposal review-required tasks: `14`
- Skill active tasks: `1`
- Skill promotion-evidence tasks: `1`
- Behavior assertion tasks: `2`
- Behavior assertions passed: `0`
- Runtime-spine assertion tasks: `4`
- Runtime-spine assertions passed: `4`
- Runtime-spine assertions failed: `0`
- Runtime-spine full coverage tasks: `2`
- Runtime-spine trace-present tasks: `14`
- Runtime-spine risky tool runs: `41`
- Runtime-spine risky tool reviewed: `41`
- Runtime-spine risky missing-review tasks: `0`
- Route recovery tasks: `5`
- Route recovery events: `5`
- Route recovery read/search expansions: `0`
- Route recovery mutation blocks: `2`
- Route recovery safety-monotonic tasks: `5`
- Route recovery unsafe mutation-expansion tasks: `0`
- Context-zone envelope tasks: `14`
- Context-zone envelope messages: `14`
- Context-zone source messages: `59`
- Context-zone duplicate blocks removed: `0`
- Context-zone provenance markers: `47`
- Gate outcome tasks: `14`
- Gate outcome records: `151`
- Gate outcome protective blocks: `15`
- Gate outcome recoverable friction: `14`
- Gate outcome unrecovered blocks: `0`
- Gate outcome harmless passes: `122`
- Proof support verified tasks: `9`
- Proof support partial tasks: `0`
- Proof support not-verified tasks: `5`
- Proof support residual-risk tasks: `5`
- Average outcome score: `65.4`
- Average process score: `82.2`
- Average efficiency score: `70.8`
- Average agent score: `71.6`
- Invalid actions total: `29`
- Premature edits total: `0`
- Scope drifts total: `0`
- Repeated actions total: `23`
- Failed actions total: `36`
- Coding gauntlet agent-run tasks: `14`
- Coding gauntlet passes: `9`
- Coding gauntlet failures: `5`
- Coding gauntlet likely clean passes: `4`
- Coding gauntlet repaired passes: `4`
- Coding gauntlet required-validation passes: `9/14`
- Coding gauntlet first-write observed: `8/14`
- Coding gauntlet repair signals: `31`
- Coding gauntlet changed files: `546`
- Status counts: failed=5, passed=9
- Failure owners: llm_reasoning=5, none=9
- Eval intents: audit_or_regression_check=5, seeded_code_change=9

## Failure Modes

- `warning:no_code_diff`: `7`
- `closeout_not_successful`: `5`
- `required_commands_not_passing`: `5`
- `warning:tool_errors_seen`: `5`
- `expected_code_diff_missing`: `4`
- `patch_synthesis_no_change`: `3`
- `warning:audit_no_code_diff`: `3`
- `warning:patch_synthesis_no_change`: `3`
- `behavior_assertions_not_passing`: `2`
- `acceptance_review_rejected`: `1`
- `earlier_stage_validation_failed_before_repair`: `1`
- `earlier_verification_failed_before_repair`: `1`
- `fields `total`, `passed`, and `failed` are never read`: `1`
- `function `estimate_skill_semantic_drift` is never used`: `1`
- `function `format_memory_write_outcome` is never used`: `1`
- `function `skill_fitness_from_bound_eval` is never used`: `1`
- `function `validate_skill_promotion_for_apply` is never used`: `1`
- `stage_validation_failed`: `1`
- `verification_failed`: `1`

## Release Dogfood Failure Classes

| class | count | meaning |
|-------|-------|---------|
| desktop_evidence | 14 | Desktop UI, screenshot, native smoke, or visual evidence failures. |
| file_state | 14 | Read-before-edit, stale file, checkpoint, rollback, or diff-state failures. |
| permission_recovery | 14 | Permission denial, approval, or recovery-loop failures. |
| tool_contract | 14 | Tool schema, exposure, result-pair, or contract boundary failures. |
| llm_reasoning | 8 | Model failed to plan, edit, validate, or close out despite available tools. |

## Memory And Skill Evidence

| dimension | count | meaning |
|-----------|-------|---------|
| memory_active_tasks | 14 | Tasks where retrieval, sync, or memory tools were active. |
| memory_changed_plan_tasks | 4 | Tasks where memory or learning signals reweighted planning. |
| memory_recalled_items | 0 | Retrieved memory-backed context items across tasks. |
| memory_conflicts | 0 | Retrieval-context conflict count from memory-backed context. |
| memory_candidate_typed_tasks | 10 | Tasks with typed memory candidates, including review-only MemoryProposal candidates. |
| memory_candidate_evidence_tasks | 10 | Tasks with evidence-backed memory candidates, including review-only MemoryProposal evidence. |
| memory_proposal_tasks | 14 | Tasks that emitted a review-only MemoryProposal trace event. |
| memory_proposal_candidates | 10 | Review-only MemoryProposal candidates proposed across tasks. |
| memory_proposal_evidence_items | 114 | Evidence items attached to review-only MemoryProposal candidates. |
| memory_proposal_review_required_tasks | 14 | MemoryProposal tasks that require review before persistence. |
| skill_active_tasks | 1 | Tasks where skill tools or skill-specific signals were active. |
| skill_promotion_evidence_tasks | 1 | Tasks with promotion-related skill evidence. |
| behavior_assertion_tasks | 2 | Tasks with explicit behavior assertions in the live-eval sample. |
| behavior_assertions_passed | 0 | Explicit behavior-assertion tasks whose required checks passed. |
| memory_behavior_assertion_tasks | 1 | Behavior assertions covering memory semantics rather than only memory activity signals. |
| skill_behavior_assertion_tasks | 1 | Behavior assertions covering skill semantics rather than only skill activity signals. |

## Runtime Spine Evidence

| dimension | count | meaning |
|-----------|-------|---------|
| runtime_spine_assertion_tasks | 4 | Tasks with explicit runtime-spine assertions in the live-eval sample or report. |
| runtime_spine_assertions_passed | 4 | Runtime-spine assertion tasks whose required trace/control-loop signals were present. |
| runtime_spine_assertions_failed | 0 | Runtime-spine assertion tasks missing required trace/control-loop signals. |
| runtime_spine_full_coverage_tasks | 2 | Tasks whose trace touched all runtime-spine phases. |
| runtime_spine_trace_present_tasks | 14 | Tasks with a trace summary available to the report parser. |
| runtime_spine_risky_tool_runs | 41 | Risky tool executions observed from trace or agent events. |
| runtime_spine_risky_tool_reviewed | 41 | Risky tool executions with matching action.review trace evidence. |
| runtime_spine_risky_missing_review_tasks | 0 | Tasks with risky tool executions missing matching action.review evidence. |
| route_recovery_tasks | 5 | Tasks with route-recovery plans emitted by the runtime. |
| route_recovery_events | 5 | Route-recovery plans observed across task traces. |
| route_recovery_read_search_expansions | 0 | Tasks where route recovery expanded only read/search understanding tools. |
| route_recovery_mutation_blocks | 2 | Tasks where route recovery explicitly blocked silent mutation expansion. |
| route_recovery_safety_monotonic_tasks | 5 | Tasks where route recovery preserved destructive-tool authority. |
| route_recovery_unsafe_mutation_expansion_tasks | 0 | Tasks where route recovery exposed mutation alternatives and should be investigated. |
| context_zone_envelope_tasks | 14 | Tasks where dynamic context was consolidated into a primary zone-first envelope. |
| context_zone_envelope_messages | 14 | Consolidated context-zone envelope messages observed across tasks. |
| context_zone_source_messages | 59 | Dynamic source messages consumed into context-zone envelopes. |
| context_zone_duplicate_blocks_removed | 0 | Duplicate dynamic zone blocks removed during request assembly. |
| context_zone_provenance_markers | 47 | Provenance markers preserved inside context-zone envelopes. |
| gate_outcome_tasks | 14 | Tasks with derived gate-outcome records from trace or report fields. |
| gate_outcome_records | 151 | Total gate-outcome records derived across action review, permission, and closeout gates. |
| gate_outcome_protective_blocks | 15 | Gate blocks that protected policy, scope, budget, checkpoint, or closeout invariants. |
| gate_outcome_recoverable_friction | 14 | Gate friction followed by a completed or passed runtime outcome. |
| gate_outcome_unrecovered_blocks | 0 | Gate blocks without later runtime recovery evidence. |
| gate_outcome_suspected_false_positives | 0 | Scenario-oracle suspected gate false positives. |
| gate_outcome_policy_correct_but_ux_costly | 0 | Policy-correct gate decisions that still created measurable UX cost. |
| gate_outcome_harmless_passes | 122 | Gate decisions that passed without measurable friction. |
| proof_support_verified_tasks | 9 | Tasks whose proof-kind policy supports verified closeout. |
| proof_support_partial_tasks | 0 | Tasks with useful proof evidence that cannot support verified closeout. |
| proof_support_not_verified_tasks | 5 | Tasks whose proof policy blocks verified closeout. |
| proof_support_residual_risk_tasks | 5 | Tasks whose proof support carries residual risk. |

### Gate Outcome Matrix

| task | total | protective | recoverable | unrecovered | suspected_false_positive | policy_correct_but_ux_costly | harmless | records | owners |
|------|-------|------------|-------------|-------------|--------------------------|------------------------------|----------|---------|--------|
| backend-todo-api-crud | 15 | 9 | 0 | 0 | 0 | 0 | 6 | action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:revise:protective_block,action_review:revise:protective_block,action_review:revise:protective_block,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:revise:protective_block,action_review:revise:protective_block,action_review:revise:protective_block,action_review:allow:harmless_pass,+3 | none |
| code-change-verification-repair-loop | 5 | 1 | 0 | 0 | 0 | 0 | 4 | action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,closeout:failed:protective_block | none |
| core-inspection-grounding | 6 | 0 | 4 | 0 | 0 | 0 | 2 | action_review:allow:harmless_pass,action_review:revise:recoverable_friction,action_review:revise:recoverable_friction,action_review:revise:recoverable_friction,action_review:revise:recoverable_friction,closeout:passed:harmless_pass | none |
| core-long-output-artifact | 4 | 0 | 2 | 0 | 0 | 0 | 2 | action_review:allow:harmless_pass,action_review:ask_user:recoverable_friction,permission:reject_once:recoverable_friction,closeout:passed:harmless_pass | none |
| core-multi-file-edit | 15 | 0 | 0 | 0 | 0 | 0 | 15 | action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,+3 | none |
| core-permission-rejection-recovery | 4 | 3 | 0 | 0 | 0 | 0 | 1 | action_review:allow:harmless_pass,action_review:revise:protective_block,action_review:revise:protective_block,closeout:failed:protective_block | none |
| core-provider-roundtrip | 19 | 0 | 0 | 0 | 0 | 0 | 19 | action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,+7 | none |
| core-rollback-product-path | 28 | 0 | 4 | 0 | 0 | 0 | 24 | action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:ask_user:recoverable_friction,permission:reject_once:recoverable_friction,action_review:ask_user:recoverable_friction,+16 | none |
| core-simple-stale-edit | 3 | 0 | 0 | 0 | 0 | 0 | 3 | action_review:allow:harmless_pass,action_review:allow:harmless_pass,closeout:passed:harmless_pass | none |
| core-terminal-install-run | 19 | 0 | 4 | 0 | 0 | 0 | 15 | action_review:allow:harmless_pass,action_review:revise:recoverable_friction,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:revise:recoverable_friction,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,+7 | none |
| frontend-book-notes-localstorage | 8 | 0 | 0 | 0 | 0 | 0 | 8 | action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,closeout:passed:harmless_pass | none |
| live-eval-dashboard-summary | 6 | 0 | 0 | 0 | 0 | 0 | 6 | action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,closeout:passed:harmless_pass | none |
| memory-save-quality-gate | 10 | 1 | 0 | 0 | 0 | 0 | 9 | action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,closeout:failed:protective_block | none |
| skill-promotion-gate | 9 | 1 | 0 | 0 | 0 | 0 | 8 | action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,closeout:failed:protective_block | none |

### Proof Support Matrix

| task | proof_status | support_status | supports_verified | residual_risk | proof_kinds | support_summary |
|------|--------------|----------------|-------------------|---------------|-------------|-----------------|
| backend-todo-api-crud | failed | failed | false | true | none | verification proof status failed blocks verified closeout before proof-kind policy |
| code-change-verification-repair-loop | failed | failed | false | true | none | verification proof status failed blocks verified closeout before proof-kind policy |
| core-inspection-grounding | verified | verified | true | false | command_passed,required_validation_passed | verified by command_passed,required_validation_passed |
| core-long-output-artifact | verified | verified | true | false | command_passed,required_validation_passed | verified by command_passed,required_validation_passed |
| core-multi-file-edit | verified | verified | true | false | command_passed,required_validation_passed | verified by command_passed,required_validation_passed |
| core-permission-rejection-recovery | failed | failed | false | true | none | verification proof status failed blocks verified closeout before proof-kind policy |
| core-provider-roundtrip | verified | verified | true | false | command_passed,required_validation_passed | verified by command_passed,required_validation_passed |
| core-rollback-product-path | verified | verified | true | false | command_passed,required_validation_passed | verified by command_passed,required_validation_passed |
| core-simple-stale-edit | verified | verified | true | false | command_passed,required_validation_passed | verified by command_passed,required_validation_passed |
| core-terminal-install-run | verified | verified | true | false | command_passed,required_validation_passed | verified by command_passed,required_validation_passed |
| frontend-book-notes-localstorage | verified | verified | true | false | command_passed,required_validation_passed | verified by command_passed,required_validation_passed |
| live-eval-dashboard-summary | verified | verified | true | false | command_passed,required_validation_passed | verified by command_passed,required_validation_passed |
| memory-save-quality-gate | failed | failed | false | true | none | verification proof status failed blocks verified closeout before proof-kind policy |
| skill-promotion-gate | failed | failed | false | true | none | verification proof status failed blocks verified closeout before proof-kind policy |

### Context Zone Matrix

| task | materialized | envelopes | sources | dedupe_removed | provenance | task_state_empty | current_request_empty |
|------|--------------|-----------|---------|----------------|------------|------------------|-----------------------|
| backend-todo-api-crud | true | 1 | 5 | 0 | 3 | false | false |
| code-change-verification-repair-loop | true | 1 | 4 | 0 | 4 | false | false |
| core-inspection-grounding | true | 1 | 4 | 0 | 4 | false | false |
| core-long-output-artifact | true | 1 | 4 | 0 | 3 | false | false |
| core-multi-file-edit | true | 1 | 5 | 0 | 3 | false | false |
| core-permission-rejection-recovery | true | 1 | 4 | 0 | 3 | false | false |
| core-provider-roundtrip | true | 1 | 4 | 0 | 3 | false | false |
| core-rollback-product-path | true | 1 | 4 | 0 | 3 | false | false |
| core-simple-stale-edit | true | 1 | 4 | 0 | 3 | false | false |
| core-terminal-install-run | true | 1 | 4 | 0 | 3 | false | false |
| frontend-book-notes-localstorage | true | 1 | 5 | 0 | 4 | false | false |
| live-eval-dashboard-summary | true | 1 | 4 | 0 | 3 | false | false |
| memory-save-quality-gate | true | 1 | 4 | 0 | 4 | false | false |
| skill-promotion-gate | true | 1 | 4 | 0 | 4 | false | false |

### Route Recovery Matrix

| task | events | kinds | failure_types | read_search | mutation_blocked | safety_monotonic | unsafe_mutation_expansion | summary |
|------|--------|-------|---------------|-------------|------------------|------------------|---------------------------|---------|
| backend-todo-api-crud | 1 | no_silent_mutation_expansion | hidden_mutation_tool_requested | false | true | true | false | events=1, read_search=false, mutation_blocked=true, safety=true |
| code-change-verification-repair-loop | 0 | none | none | false | false | missing | false | events=0, read_search=false, mutation_blocked=false, safety=missing |
| core-inspection-grounding | 0 | none | none | false | false | missing | false | events=0, read_search=false, mutation_blocked=false, safety=missing |
| core-long-output-artifact | 0 | none | none | false | false | missing | false | events=0, read_search=false, mutation_blocked=false, safety=missing |
| core-multi-file-edit | 1 | code_change_no_diff_replan | code_change_no_diff_after_repeated_progress | false | false | true | false | events=1, read_search=false, mutation_blocked=false, safety=true |
| core-permission-rejection-recovery | 1 | no_silent_mutation_expansion | hidden_mutation_tool_requested | false | true | true | false | events=1, read_search=false, mutation_blocked=true, safety=true |
| core-provider-roundtrip | 0 | none | none | false | false | missing | false | events=0, read_search=false, mutation_blocked=false, safety=missing |
| core-rollback-product-path | 0 | none | none | false | false | missing | false | events=0, read_search=false, mutation_blocked=false, safety=missing |
| core-simple-stale-edit | 0 | none | none | false | false | missing | false | events=0, read_search=false, mutation_blocked=false, safety=missing |
| core-terminal-install-run | 0 | none | none | false | false | missing | false | events=0, read_search=false, mutation_blocked=false, safety=missing |
| frontend-book-notes-localstorage | 1 | code_change_no_diff_replan | code_change_no_diff_after_repeated_progress | false | false | true | false | events=1, read_search=false, mutation_blocked=false, safety=true |
| live-eval-dashboard-summary | 1 | code_change_no_diff_replan | code_change_no_diff_after_repeated_progress | false | false | true | false | events=1, read_search=false, mutation_blocked=false, safety=true |
| memory-save-quality-gate | 0 | none | none | false | false | missing | false | events=0, read_search=false, mutation_blocked=false, safety=missing |
| skill-promotion-gate | 0 | none | none | false | false | missing | false | events=0, read_search=false, mutation_blocked=false, safety=missing |

## Evaluation Scores

| dimension | value | meaning |
|-----------|-------|---------|
| outcome_score_avg | 65.4 | Average deterministic outcome score across task reports. |
| process_score_avg | 82.2 | Average deterministic process score across task reports. |
| efficiency_score_avg | 70.8 | Average deterministic efficiency score across task reports. |
| agent_score_avg | 71.6 | Weighted score: outcome 50%, process 30%, efficiency 20%. |
| invalid_actions_total | 29 | Premature edits, scope drift, repeated actions, risky review gaps, and phase-misaligned actions. |
| premature_edits_total | 0 | Edits attempted before enough evidence or explicitly demoted as early/low-value. |
| scope_drifts_total | 0 | Action decisions with very low scope fit or medium/high goal drift. |
| repeated_actions_total | 23 | Repeated tool actions or repeated-action stop signals. |
| failed_actions_total | 36 | Failed tool/action observations from trace and event logs. |

### Score Matrix

| task | outcome | process | efficiency | agent | invalid | premature_edit | scope_drift | repeated | failed_actions | penalties |
|------|---------|---------|------------|-------|---------|----------------|-------------|----------|----------------|-----------|
| backend-todo-api-crud | 15 | 65 | 35 | 34 | 3 | 0 | 0 | 3 | 8 | run_failed,required_commands_failed,verification_failed,closeout_not_successful,repeated_action,invalid_action,failed_actions,repeated_actions,user_questions,llm_call_budget_pressure |
| code-change-verification-repair-loop | 0 | 100 | 84 | 47 | 0 | 0 | 0 | 0 | 2 | run_failed,required_commands_failed,verification_failed,closeout_not_successful,expected_code_diff_missing,failed_actions |
| core-inspection-grounding | 100 | 80 | 75 | 89 | 4 | 0 | 0 | 0 | 4 | invalid_action,failed_actions |
| core-long-output-artifact | 100 | 100 | 74 | 95 | 0 | 0 | 0 | 0 | 2 | failed_actions,user_questions |
| core-multi-file-edit | 100 | 100 | 100 | 100 | 0 | 0 | 0 | 0 | 0 | none |
| core-permission-rejection-recovery | 0 | 69 | 70 | 35 | 3 | 0 | 0 | 2 | 2 | run_failed,required_commands_failed,verification_failed,closeout_not_successful,expected_code_diff_missing,repeated_action,invalid_action,failed_actions,repeated_actions |
| core-provider-roundtrip | 100 | 60 | 70 | 82 | 4 | 0 | 0 | 4 | 0 | repeated_action,invalid_action,repeated_actions,llm_call_budget_pressure |
| core-rollback-product-path | 100 | 100 | 50 | 90 | 0 | 0 | 0 | 0 | 4 | failed_actions,user_questions,llm_call_budget_pressure |
| core-simple-stale-edit | 100 | 100 | 100 | 100 | 0 | 0 | 0 | 0 | 0 | none |
| core-terminal-install-run | 100 | 60 | 30 | 74 | 8 | 0 | 0 | 7 | 4 | repeated_action,invalid_action,failed_actions,repeated_actions,user_questions,llm_call_budget_pressure |
| frontend-book-notes-localstorage | 100 | 65 | 80 | 86 | 3 | 0 | 0 | 3 | 0 | repeated_action,invalid_action,repeated_actions |
| live-eval-dashboard-summary | 100 | 100 | 100 | 100 | 0 | 0 | 0 | 0 | 0 | none |
| memory-save-quality-gate | 0 | 87 | 68 | 40 | 1 | 0 | 0 | 1 | 6 | run_failed,required_commands_failed,verification_failed,closeout_not_successful,behavior_assertions_failed,expected_code_diff_missing,repeated_action,invalid_action,failed_actions,repeated_actions |
| skill-promotion-gate | 0 | 65 | 55 | 30 | 3 | 0 | 0 | 3 | 4 | run_failed,required_commands_failed,verification_failed,closeout_not_successful,behavior_assertions_failed,expected_code_diff_missing,repeated_action,invalid_action,failed_actions,repeated_actions |

## Outcome Classes

| class | count | meaning |
|-------|-------|---------|
| real_code_change_passed | 6 | Agent-run tasks with passing status and a real diff. |
| plan_only_passed | 0 | Planning/API-only artifacts that passed their available checks. |
| seeded_no_diff_failed | 4 | Seeded code-change tasks where the agent did not produce a diff. |

## Coding Gauntlet Evidence

| task | gauntlet_status | first_pass_signal | failure_class | coding | required | closeout | spine | contract | risk | first_write | diff | warnings |
|------|-----------------|-------------------|---------------|--------|----------|----------|-------|----------|------|-------------|------|----------|
| backend-todo-api-crud | failed | failed | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | tools=6, tool_records=97, validations=6, repair=8, files=1 | failed | failed | coverage=6/7, status=none, missing=none | entry=active:force repair=active_after_failure | entry=high runtime=high | 3 | yes | none |
| code-change-verification-repair-loop | failed | failed | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | tools=4, tool_records=9, validations=0, repair=1, files=0 | failed | failed | coverage=6/7, status=passed, missing=none | entry=active:force repair=not_needed | entry=high runtime=none | 4 | no | no_code_diff,tool_errors_seen,patch_synthesis_no_change |
| core-inspection-grounding | passed | repaired | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | tools=1, tool_records=5, validations=0, repair=5, files=0 | ok | passed | coverage=6/7, status=passed, missing=none | entry=active:force repair=not_needed | entry=high runtime=none | none | no | no_code_diff,audit_no_code_diff |
| core-long-output-artifact | passed | repaired | tool_contract,file_state,permission_recovery,desktop_evidence | tools=2, tool_records=3, validations=2, repair=2, files=1 | ok | passed | coverage=7/7, status=none, missing=none | entry=active:force repair=active_after_failure | entry=high runtime=high | none | yes | tool_errors_seen |
| core-multi-file-edit | passed | likely_clean | tool_contract,file_state,permission_recovery,desktop_evidence | tools=14, tool_records=42, validations=2, repair=0, files=2 | ok | passed | coverage=6/7, status=passed, missing=none | entry=active:force repair=not_needed | entry=high runtime=none | 13 | yes | none |
| core-permission-rejection-recovery | failed | failed | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | tools=1, tool_records=4, validations=0, repair=2, files=0 | failed | failed | coverage=6/7, status=none, missing=none | entry=active:force repair=active_after_failure | entry=high runtime=high | none | no | no_code_diff |
| core-provider-roundtrip | passed | no_write | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | tools=18, tool_records=128, validations=0, repair=0, files=0 | ok | passed | coverage=6/7, status=none, missing=none | entry=active:force repair=not_needed | entry=high runtime=none | none | no | no_code_diff,audit_no_code_diff |
| core-rollback-product-path | passed | repaired | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | tools=25, tool_records=161, validations=0, repair=3, files=0 | ok | passed | coverage=7/7, status=none, missing=none | entry=active:force repair=not_needed | entry=high runtime=none | none | no | no_code_diff,audit_no_code_diff,tool_errors_seen |
| core-simple-stale-edit | passed | likely_clean | tool_contract,file_state,permission_recovery,desktop_evidence | tools=2, tool_records=3, validations=2, repair=0, files=1 | ok | passed | coverage=6/7, status=passed, missing=none | entry=active:force repair=not_needed | entry=high runtime=none | 2 | yes | none |
| core-terminal-install-run | passed | repaired | tool_contract,file_state,permission_recovery,desktop_evidence | tools=14, tool_records=73, validations=0, repair=5, files=538 | ok | passed | coverage=6/7, status=none, missing=none | entry=active:force repair=active_after_failure | entry=high runtime=high | none | yes | none |
| frontend-book-notes-localstorage | passed | likely_clean | tool_contract,file_state,permission_recovery,desktop_evidence | tools=7, tool_records=27, validations=2, repair=0, files=1 | ok | passed | coverage=6/7, status=none, missing=none | entry=active:force repair=not_needed | entry=high runtime=none | 7 | yes | none |
| live-eval-dashboard-summary | passed | likely_clean | tool_contract,file_state,permission_recovery,desktop_evidence | tools=5, tool_records=10, validations=2, repair=0, files=2 | ok | passed | coverage=6/7, status=none, missing=none | entry=active:force repair=not_needed | entry=high runtime=none | 5 | yes | none |
| memory-save-quality-gate | failed | failed | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | tools=9, tool_records=18, validations=0, repair=3, files=0 | failed | failed | coverage=6/7, status=none, missing=none | entry=active:force repair=not_needed | entry=high runtime=none | 7 | no | no_code_diff,tool_errors_seen,patch_synthesis_no_change |
| skill-promotion-gate | failed | failed | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | tools=8, tool_records=17, validations=0, repair=2, files=0 | failed | failed | coverage=6/7, status=none, missing=none | entry=active:force repair=not_needed | entry=high runtime=none | 7 | no | no_code_diff,tool_errors_seen,patch_synthesis_no_change |

## Task Matrix

| task | status | intent | owner | failure_class | required | plan_quality | tool_boundary | verification_status | closeout | runtime_spine | runtime_diet | contract | risk | behavior_assertions | behavior_status | triggers | first_write | diff | memory | skill | warnings |
|------|--------|--------|-------|---------------|----------|--------------|---------------|---------------------|----------|---------------|--------------|----------|------|---------------------|-----------------|----------|-------------|------|--------|-------|----------|
| backend-todo-api-crud | failed | seeded_code_change | llm_reasoning | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | failed | none | agent-run | failed | failed | coverage=6/7, status=none, missing=none | prompt=38034 tool_schema=3950 tools=19 workflow=strict closeout=full validation=failed:2/2 | entry=active:force repair=active_after_failure | entry=high runtime=high | none | none | risk_signal_high,required_validation,first_code_change,verification_failed,acceptance_rejected | 3 | yes | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | none |
| code-change-verification-repair-loop | failed | seeded_code_change | llm_reasoning | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | failed | none | agent-run | failed | failed | coverage=6/7, status=passed, missing=none | prompt=8454 tool_schema=3950 tools=19 workflow=strict closeout=full validation=failed:4/5 | entry=active:force repair=not_needed | entry=high runtime=none | none | none | risk_signal_high,required_validation | 4 | no | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff,tool_errors_seen,patch_synthesis_no_change |
| core-inspection-grounding | passed | audit_or_regression_check | none | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | ok | none | agent-run | passed | passed | coverage=6/7, status=passed, missing=none | prompt=7070 tool_schema=3950 tools=19 workflow=guarded closeout=full validation=passed:4/4 recovered_failed:4 | entry=active:force repair=not_needed | entry=high runtime=none | none | none | risk_signal_high,required_validation | none | no | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff,audit_no_code_diff |
| core-long-output-artifact | passed | audit_or_regression_check | none | tool_contract,file_state,permission_recovery,desktop_evidence | ok | none | agent-run | passed | passed | coverage=7/7, status=none, missing=none | prompt=4615 tool_schema=3950 tools=19 workflow=guarded closeout=full validation=passed:3/3 | entry=active:force repair=active_after_failure | entry=high runtime=high | none | none | risk_signal_high,required_validation,first_code_change | none | yes | active=true, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | tool_errors_seen |
| core-multi-file-edit | passed | seeded_code_change | none | tool_contract,file_state,permission_recovery,desktop_evidence | ok | none | agent-run | passed | passed | coverage=6/7, status=passed, missing=none | prompt=10369 tool_schema=3950 tools=19 workflow=guarded closeout=full validation=passed:3/3 recovered_failed:3 | entry=active:force repair=not_needed | entry=high runtime=none | none | none | risk_signal_high,required_validation,repeated_no_code_progress,first_code_change | 13 | yes | active=true, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | none |
| core-permission-rejection-recovery | failed | seeded_code_change | llm_reasoning | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | failed | none | agent-run | failed | failed | coverage=6/7, status=none, missing=none | prompt=5316 tool_schema=3950 tools=19 workflow=strict closeout=full validation=failed:2/3 | entry=active:force repair=active_after_failure | entry=high runtime=high | none | none | risk_signal_high,required_validation | none | no | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff |
| core-provider-roundtrip | passed | audit_or_regression_check | none | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | ok | none | agent-run | passed | passed | coverage=6/7, status=none, missing=none | prompt=17696 tool_schema=3950 tools=19 workflow=guarded closeout=full validation=passed:1/1 | entry=active:force repair=not_needed | entry=high runtime=none | none | none | risk_signal_high,required_validation | none | no | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff,audit_no_code_diff |
| core-rollback-product-path | passed | audit_or_regression_check | none | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | ok | none | agent-run | passed | passed | coverage=7/7, status=none, missing=none | prompt=29704 tool_schema=3950 tools=19 workflow=guarded closeout=full validation=passed:2/2 | entry=active:force repair=not_needed | entry=high runtime=none | none | none | risk_signal_high,required_validation | none | no | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff,audit_no_code_diff,tool_errors_seen |
| core-simple-stale-edit | passed | seeded_code_change | none | tool_contract,file_state,permission_recovery,desktop_evidence | ok | none | agent-run | passed | passed | coverage=6/7, status=passed, missing=none | prompt=4697 tool_schema=3950 tools=19 workflow=guarded closeout=full validation=passed:2/2 recovered_failed:2 | entry=active:force repair=not_needed | entry=high runtime=none | none | none | risk_signal_high,required_validation,first_code_change | 2 | yes | active=true, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | none |
| core-terminal-install-run | passed | audit_or_regression_check | none | tool_contract,file_state,permission_recovery,desktop_evidence | ok | none | agent-run | passed | passed | coverage=6/7, status=none, missing=none | prompt=19151 tool_schema=4300 tools=20 workflow=guarded closeout=full validation=passed:2/2 | entry=active:force repair=active_after_failure | entry=high runtime=high | none | none | risk_signal_high,required_validation | none | yes | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | none |
| frontend-book-notes-localstorage | passed | seeded_code_change | none | tool_contract,file_state,permission_recovery,desktop_evidence | ok | none | agent-run | passed | passed | coverage=6/7, status=none, missing=none | prompt=11579 tool_schema=3950 tools=19 workflow=guarded closeout=full validation=passed:2/2 recovered_failed:2 | entry=active:force repair=not_needed | entry=high runtime=none | none | none | risk_signal_high,required_validation,repeated_no_code_progress,first_code_change | 7 | yes | active=true, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | none |
| live-eval-dashboard-summary | passed | seeded_code_change | none | tool_contract,file_state,permission_recovery,desktop_evidence | ok | none | agent-run | passed | passed | coverage=6/7, status=none, missing=none | prompt=4956 tool_schema=3950 tools=19 workflow=guarded closeout=full validation=passed:5/5 recovered_failed:1 | entry=active:force repair=not_needed | entry=high runtime=none | none | none | risk_signal_high,required_validation,repeated_no_code_progress,first_code_change | 5 | yes | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | none |
| memory-save-quality-gate | failed | seeded_code_change | llm_reasoning | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | failed | none | agent-run | failed | failed | coverage=6/7, status=none, missing=none | prompt=6415 tool_schema=3950 tools=19 workflow=strict closeout=full validation=failed:3/4 | entry=active:force repair=not_needed | entry=high runtime=none | memory_quality_gate,memory_save_outcome_visibility | failed | risk_signal_high,required_validation | 7 | no | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff,tool_errors_seen,patch_synthesis_no_change |
| skill-promotion-gate | failed | seeded_code_change | llm_reasoning | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | failed | none | agent-run | failed | failed | coverage=6/7, status=none, missing=none | prompt=8088 tool_schema=3950 tools=19 workflow=guarded closeout=full validation=failed:2/5 | entry=active:force repair=not_needed | entry=high runtime=none | skill_promotion_gate,skill_evolution_cooldown | failed | risk_signal_high,required_validation | 7 | no | active=true, recalled=0, conflicts=0, changed_plan=false | active=true, tool_calls=0, usage_events=0, promotion=true | no_code_diff,tool_errors_seen,patch_synthesis_no_change |

## Notes

- `plan_quality` describes plan-only/API artifacts when present.
- `tool_boundary` separates plan-only, collect-only, and real agent-run reports.
- `verification_status` combines closeout and required-command evidence; it is not a human-quality score.
- `skipped` reports are excluded from pass/fail rate denominators; collect-only reports need passing required commands to be scored.
- `real_code_change_passed` requires an agent-run report with a non-empty diff; plan-only success is tracked separately.
- `memory` and `skill` summarize evidence signals; they do not by themselves mean the task succeeded.
- `behavior_assertions` are explicit sample-level checks; memory/skill behavior assertions are stronger evidence than activity signals alone.
- `runtime_spine` summarizes trace/control-loop coverage and explicit runtime-spine assertions.
