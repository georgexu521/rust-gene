# Live Eval Summary: coding-polish-real-20260528-123600

- Run directory: `docs/benchmarks/live-coding-polish-real-20260528-123600`
- Tasks found: `17`
- Pass rate: `12/17` (70.6%)
- Failure rate: `5/17` (29.4%)
- Skipped/unscored tasks: `0`
- Real code-change passes: `9`
- Plan-only passes: `0`
- Seeded no-diff failures: `4`
- Memory active tasks: `6`
- Memory changed-plan tasks: `9`
- Memory recalled items: `42`
- Memory conflicts: `0`
- Memory typed-candidate tasks: `17`
- Memory evidence-backed candidate tasks: `17`
- Memory proposal tasks: `17`
- Memory proposal candidates: `27`
- Memory proposal evidence items: `153`
- Memory proposal review-required tasks: `17`
- Skill active tasks: `1`
- Skill promotion-evidence tasks: `1`
- Behavior assertion tasks: `5`
- Behavior assertions passed: `1`
- Runtime-spine assertion tasks: `4`
- Runtime-spine assertions passed: `4`
- Runtime-spine assertions failed: `0`
- Runtime-spine full coverage tasks: `3`
- Runtime-spine trace-present tasks: `17`
- Runtime-spine risky tool runs: `44`
- Runtime-spine risky tool reviewed: `44`
- Runtime-spine risky missing-review tasks: `0`
- Route recovery tasks: `5`
- Route recovery events: `6`
- Route recovery read/search expansions: `0`
- Route recovery mutation blocks: `1`
- Route recovery safety-monotonic tasks: `5`
- Route recovery unsafe mutation-expansion tasks: `0`
- Context-zone envelope tasks: `17`
- Context-zone envelope messages: `17`
- Context-zone source messages: `76`
- Context-zone duplicate blocks removed: `0`
- Context-zone provenance markers: `94`
- Gate outcome tasks: `17`
- Gate outcome records: `173`
- Gate outcome protective blocks: `7`
- Gate outcome recoverable friction: `12`
- Gate outcome unrecovered blocks: `0`
- Gate outcome harmless passes: `154`
- Proof support verified tasks: `12`
- Proof support partial tasks: `0`
- Proof support not-verified tasks: `5`
- Proof support residual-risk tasks: `5`
- Average outcome score: `70.9`
- Average process score: `78.5`
- Average efficiency score: `73.0`
- Average agent score: `73.6`
- Invalid actions total: `40`
- Premature edits total: `0`
- Scope drifts total: `3`
- Repeated actions total: `31`
- Failed actions total: `36`
- Coding gauntlet agent-run tasks: `17`
- Coding gauntlet passes: `12`
- Coding gauntlet failures: `5`
- Coding gauntlet likely clean passes: `4`
- Coding gauntlet repaired passes: `8`
- Coding gauntlet required-validation passes: `12/17`
- Coding gauntlet first-write observed: `11/17`
- Coding gauntlet repair signals: `32`
- Coding gauntlet changed files: `11`
- Status counts: failed=5, passed=12
- Failure owners: llm_reasoning=5, none=12
- Eval intents: audit_or_regression_check=5, seeded_code_change=12

## Failure Modes

- `warning:tool_errors_seen`: `8`
- `warning:no_code_diff`: `7`
- `closeout_not_successful`: `5`
- `required_commands_not_passing`: `5`
- `behavior_assertions_not_passing`: `4`
- `expected_code_diff_missing`: `4`
- `patch_synthesis_no_change`: `3`
- `warning:audit_no_code_diff`: `3`
- `warning:patch_synthesis_no_change`: `3`
- `earlier_stage_validation_failed_before_repair`: `2`
- `earlier_verification_failed_before_repair`: `2`
- `associated function `build_active_memory_context` is never used`: `1`
- `field `session_id` is never read`: `1`
- `function `estimate_skill_semantic_drift` is never used`: `1`
- `function `format_memory_write_outcome` is never used`: `1`
- `function `promote_trace_candidate_memories` is never used`: `1`
- `function `skill_fitness_from_bound_eval` is never used`: `1`
- `function `validate_skill_promotion_for_apply` is never used`: `1`
- `stage_validation_failed`: `1`
- `verification_failed`: `1`

## Release Dogfood Failure Classes

| class | count | meaning |
|-------|-------|---------|
| desktop_evidence | 17 | Desktop UI, screenshot, native smoke, or visual evidence failures. |
| file_state | 17 | Read-before-edit, stale file, checkpoint, rollback, or diff-state failures. |
| permission_recovery | 17 | Permission denial, approval, or recovery-loop failures. |
| tool_contract | 17 | Tool schema, exposure, result-pair, or contract boundary failures. |
| llm_reasoning | 8 | Model failed to plan, edit, validate, or close out despite available tools. |

## Memory And Skill Evidence

| dimension | count | meaning |
|-----------|-------|---------|
| memory_active_tasks | 6 | Tasks where retrieval, sync, or memory tools were active. |
| memory_changed_plan_tasks | 9 | Tasks where memory or learning signals reweighted planning. |
| memory_recalled_items | 42 | Retrieved memory-backed context items across tasks. |
| memory_conflicts | 0 | Retrieval-context conflict count from memory-backed context. |
| memory_candidate_typed_tasks | 17 | Tasks with typed memory candidates, including review-only MemoryProposal candidates. |
| memory_candidate_evidence_tasks | 17 | Tasks with evidence-backed memory candidates, including review-only MemoryProposal evidence. |
| memory_proposal_tasks | 17 | Tasks that emitted a review-only MemoryProposal trace event. |
| memory_proposal_candidates | 27 | Review-only MemoryProposal candidates proposed across tasks. |
| memory_proposal_evidence_items | 153 | Evidence items attached to review-only MemoryProposal candidates. |
| memory_proposal_review_required_tasks | 17 | MemoryProposal tasks that require review before persistence. |
| skill_active_tasks | 1 | Tasks where skill tools or skill-specific signals were active. |
| skill_promotion_evidence_tasks | 1 | Tasks with promotion-related skill evidence. |
| behavior_assertion_tasks | 5 | Tasks with explicit behavior assertions in the live-eval sample. |
| behavior_assertions_passed | 1 | Explicit behavior-assertion tasks whose required checks passed. |
| memory_behavior_assertion_tasks | 4 | Behavior assertions covering memory semantics rather than only memory activity signals. |
| skill_behavior_assertion_tasks | 1 | Behavior assertions covering skill semantics rather than only skill activity signals. |

## Runtime Spine Evidence

| dimension | count | meaning |
|-----------|-------|---------|
| runtime_spine_assertion_tasks | 4 | Tasks with explicit runtime-spine assertions in the live-eval sample or report. |
| runtime_spine_assertions_passed | 4 | Runtime-spine assertion tasks whose required trace/control-loop signals were present. |
| runtime_spine_assertions_failed | 0 | Runtime-spine assertion tasks missing required trace/control-loop signals. |
| runtime_spine_full_coverage_tasks | 3 | Tasks whose trace touched all runtime-spine phases. |
| runtime_spine_trace_present_tasks | 17 | Tasks with a trace summary available to the report parser. |
| runtime_spine_risky_tool_runs | 44 | Risky tool executions observed from trace or agent events. |
| runtime_spine_risky_tool_reviewed | 44 | Risky tool executions with matching action.review trace evidence. |
| runtime_spine_risky_missing_review_tasks | 0 | Tasks with risky tool executions missing matching action.review evidence. |
| route_recovery_tasks | 5 | Tasks with route-recovery plans emitted by the runtime. |
| route_recovery_events | 6 | Route-recovery plans observed across task traces. |
| route_recovery_read_search_expansions | 0 | Tasks where route recovery expanded only read/search understanding tools. |
| route_recovery_mutation_blocks | 1 | Tasks where route recovery explicitly blocked silent mutation expansion. |
| route_recovery_safety_monotonic_tasks | 5 | Tasks where route recovery preserved destructive-tool authority. |
| route_recovery_unsafe_mutation_expansion_tasks | 0 | Tasks where route recovery exposed mutation alternatives and should be investigated. |
| context_zone_envelope_tasks | 17 | Tasks where dynamic context was consolidated into a primary zone-first envelope. |
| context_zone_envelope_messages | 17 | Consolidated context-zone envelope messages observed across tasks. |
| context_zone_source_messages | 76 | Dynamic source messages consumed into context-zone envelopes. |
| context_zone_duplicate_blocks_removed | 0 | Duplicate dynamic zone blocks removed during request assembly. |
| context_zone_provenance_markers | 94 | Provenance markers preserved inside context-zone envelopes. |
| gate_outcome_tasks | 17 | Tasks with derived gate-outcome records from trace or report fields. |
| gate_outcome_records | 173 | Total gate-outcome records derived across action review, permission, and closeout gates. |
| gate_outcome_protective_blocks | 7 | Gate blocks that protected policy, scope, budget, checkpoint, or closeout invariants. |
| gate_outcome_recoverable_friction | 12 | Gate friction followed by a completed or passed runtime outcome. |
| gate_outcome_unrecovered_blocks | 0 | Gate blocks without later runtime recovery evidence. |
| gate_outcome_suspected_false_positives | 0 | Scenario-oracle suspected gate false positives. |
| gate_outcome_policy_correct_but_ux_costly | 0 | Policy-correct gate decisions that still created measurable UX cost. |
| gate_outcome_harmless_passes | 154 | Gate decisions that passed without measurable friction. |
| proof_support_verified_tasks | 12 | Tasks whose proof-kind policy supports verified closeout. |
| proof_support_partial_tasks | 0 | Tasks with useful proof evidence that cannot support verified closeout. |
| proof_support_not_verified_tasks | 5 | Tasks whose proof policy blocks verified closeout. |
| proof_support_residual_risk_tasks | 5 | Tasks whose proof support carries residual risk. |

### Gate Outcome Matrix

| task | total | protective | recoverable | unrecovered | suspected_false_positive | policy_correct_but_ux_costly | harmless | records | owners |
|------|-------|------------|-------------|-------------|--------------------------|------------------------------|----------|---------|--------|
| backend-todo-api-crud | 12 | 0 | 3 | 0 | 0 | 0 | 9 | action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:revise:recoverable_friction,action_review:revise:recoverable_friction,action_review:revise:recoverable_friction,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,closeout:passed:harmless_pass | none |
| code-change-verification-repair-loop | 4 | 0 | 0 | 0 | 0 | 0 | 4 | action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,closeout:passed:harmless_pass | none |
| core-inspection-grounding | 6 | 0 | 4 | 0 | 0 | 0 | 2 | action_review:allow:harmless_pass,action_review:revise:recoverable_friction,action_review:revise:recoverable_friction,action_review:revise:recoverable_friction,action_review:revise:recoverable_friction,closeout:passed:harmless_pass | none |
| core-long-output-artifact | 4 | 0 | 2 | 0 | 0 | 0 | 2 | action_review:allow:harmless_pass,action_review:ask_user:recoverable_friction,permission:reject_once:recoverable_friction,closeout:passed:harmless_pass | none |
| core-multi-file-edit | 4 | 0 | 0 | 0 | 0 | 0 | 4 | action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,closeout:passed:harmless_pass | none |
| core-permission-rejection-recovery | 4 | 3 | 0 | 0 | 0 | 0 | 1 | action_review:allow:harmless_pass,action_review:revise:protective_block,action_review:revise:protective_block,closeout:failed:protective_block | none |
| core-provider-roundtrip | 14 | 0 | 0 | 0 | 0 | 0 | 14 | action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,+2 | none |
| core-rollback-product-path | 36 | 0 | 2 | 0 | 0 | 0 | 34 | action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:revise:recoverable_friction,action_review:revise:recoverable_friction,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,+24 | none |
| core-simple-stale-edit | 4 | 0 | 0 | 0 | 0 | 0 | 4 | action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,closeout:passed:harmless_pass | none |
| core-terminal-install-run | 12 | 0 | 1 | 0 | 0 | 0 | 11 | action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:revise:recoverable_friction,closeout:passed:harmless_pass | none |
| frontend-book-notes-localstorage | 15 | 0 | 0 | 0 | 0 | 0 | 15 | action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,+3 | none |
| live-eval-dashboard-summary | 7 | 0 | 0 | 0 | 0 | 0 | 7 | action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,closeout:passed:harmless_pass | none |
| memory-failure-lesson-promotion | 9 | 1 | 0 | 0 | 0 | 0 | 8 | action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,closeout:failed:protective_block | none |
| memory-save-quality-gate | 9 | 1 | 0 | 0 | 0 | 0 | 8 | action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,closeout:failed:protective_block | none |
| memory-stale-project-fact-demotion | 18 | 1 | 0 | 0 | 0 | 0 | 17 | action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,+6 | none |
| persistent-memory-planning-context | 7 | 0 | 0 | 0 | 0 | 0 | 7 | action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,closeout:passed:harmless_pass | none |
| skill-promotion-gate | 8 | 1 | 0 | 0 | 0 | 0 | 7 | action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,closeout:failed:protective_block | none |

### Proof Support Matrix

| task | proof_status | support_status | supports_verified | residual_risk | proof_kinds | support_summary |
|------|--------------|----------------|-------------------|---------------|-------------|-----------------|
| backend-todo-api-crud | verified | verified | true | false | command_passed,required_validation_passed | verified by command_passed,required_validation_passed |
| code-change-verification-repair-loop | verified | verified | true | false | command_passed,required_validation_passed | verified by command_passed,required_validation_passed |
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
| memory-failure-lesson-promotion | failed | failed | false | true | none | verification proof status failed blocks verified closeout before proof-kind policy |
| memory-save-quality-gate | failed | failed | false | true | none | verification proof status failed blocks verified closeout before proof-kind policy |
| memory-stale-project-fact-demotion | failed | failed | false | true | none | verification proof status failed blocks verified closeout before proof-kind policy |
| persistent-memory-planning-context | verified | verified | true | false | command_passed,required_validation_passed | verified by command_passed,required_validation_passed |
| skill-promotion-gate | failed | failed | false | true | none | verification proof status failed blocks verified closeout before proof-kind policy |

### Context Zone Matrix

| task | materialized | envelopes | sources | dedupe_removed | provenance | task_state_empty | current_request_empty |
|------|--------------|-----------|---------|----------------|------------|------------------|-----------------------|
| backend-todo-api-crud | true | 1 | 5 | 0 | 3 | false | false |
| code-change-verification-repair-loop | true | 1 | 3 | 0 | 4 | false | false |
| core-inspection-grounding | true | 1 | 4 | 0 | 4 | false | false |
| core-long-output-artifact | true | 1 | 4 | 0 | 3 | false | false |
| core-multi-file-edit | true | 1 | 4 | 0 | 3 | false | false |
| core-permission-rejection-recovery | true | 1 | 4 | 0 | 3 | false | false |
| core-provider-roundtrip | true | 1 | 4 | 0 | 3 | false | false |
| core-rollback-product-path | true | 1 | 4 | 0 | 3 | false | false |
| core-simple-stale-edit | true | 1 | 4 | 0 | 3 | false | false |
| core-terminal-install-run | true | 1 | 4 | 0 | 3 | false | false |
| frontend-book-notes-localstorage | true | 1 | 5 | 0 | 4 | false | false |
| live-eval-dashboard-summary | true | 1 | 5 | 0 | 9 | false | false |
| memory-failure-lesson-promotion | true | 1 | 6 | 0 | 9 | false | false |
| memory-save-quality-gate | true | 1 | 5 | 0 | 10 | false | false |
| memory-stale-project-fact-demotion | true | 1 | 5 | 0 | 10 | false | false |
| persistent-memory-planning-context | true | 1 | 5 | 0 | 10 | false | false |
| skill-promotion-gate | true | 1 | 5 | 0 | 10 | false | false |

### Route Recovery Matrix

| task | events | kinds | failure_types | read_search | mutation_blocked | safety_monotonic | unsafe_mutation_expansion | summary |
|------|--------|-------|---------------|-------------|------------------|------------------|---------------------------|---------|
| backend-todo-api-crud | 0 | none | none | false | false | missing | false | events=0, read_search=false, mutation_blocked=false, safety=missing |
| code-change-verification-repair-loop | 0 | none | none | false | false | missing | false | events=0, read_search=false, mutation_blocked=false, safety=missing |
| core-inspection-grounding | 0 | none | none | false | false | missing | false | events=0, read_search=false, mutation_blocked=false, safety=missing |
| core-long-output-artifact | 0 | none | none | false | false | missing | false | events=0, read_search=false, mutation_blocked=false, safety=missing |
| core-multi-file-edit | 0 | none | none | false | false | missing | false | events=0, read_search=false, mutation_blocked=false, safety=missing |
| core-permission-rejection-recovery | 2 | no_silent_mutation_expansion | hidden_mutation_tool_requested | false | true | true | false | events=2, read_search=false, mutation_blocked=true, safety=true |
| core-provider-roundtrip | 0 | none | none | false | false | missing | false | events=0, read_search=false, mutation_blocked=false, safety=missing |
| core-rollback-product-path | 0 | none | none | false | false | missing | false | events=0, read_search=false, mutation_blocked=false, safety=missing |
| core-simple-stale-edit | 0 | none | none | false | false | missing | false | events=0, read_search=false, mutation_blocked=false, safety=missing |
| core-terminal-install-run | 0 | none | none | false | false | missing | false | events=0, read_search=false, mutation_blocked=false, safety=missing |
| frontend-book-notes-localstorage | 1 | code_change_no_diff_replan | code_change_no_diff_after_repeated_progress | false | false | true | false | events=1, read_search=false, mutation_blocked=false, safety=true |
| live-eval-dashboard-summary | 1 | code_change_no_diff_replan | code_change_no_diff_after_repeated_progress | false | false | true | false | events=1, read_search=false, mutation_blocked=false, safety=true |
| memory-failure-lesson-promotion | 1 | code_change_no_diff_replan | code_change_no_diff_after_repeated_progress | false | false | true | false | events=1, read_search=false, mutation_blocked=false, safety=true |
| memory-save-quality-gate | 0 | none | none | false | false | missing | false | events=0, read_search=false, mutation_blocked=false, safety=missing |
| memory-stale-project-fact-demotion | 1 | code_change_no_diff_replan | code_change_no_diff_after_repeated_progress | false | false | true | false | events=1, read_search=false, mutation_blocked=false, safety=true |
| persistent-memory-planning-context | 0 | none | none | false | false | missing | false | events=0, read_search=false, mutation_blocked=false, safety=missing |
| skill-promotion-gate | 0 | none | none | false | false | missing | false | events=0, read_search=false, mutation_blocked=false, safety=missing |

## Evaluation Scores

| dimension | value | meaning |
|-----------|-------|---------|
| outcome_score_avg | 70.9 | Average deterministic outcome score across task reports. |
| process_score_avg | 78.5 | Average deterministic process score across task reports. |
| efficiency_score_avg | 73.0 | Average deterministic efficiency score across task reports. |
| agent_score_avg | 73.6 | Weighted score: outcome 50%, process 30%, efficiency 20%. |
| invalid_actions_total | 40 | Premature edits, scope drift, repeated actions, risky review gaps, and phase-misaligned actions. |
| premature_edits_total | 0 | Edits attempted before enough evidence or explicitly demoted as early/low-value. |
| scope_drifts_total | 3 | Action decisions with very low scope fit or medium/high goal drift. |
| repeated_actions_total | 31 | Repeated tool actions or repeated-action stop signals. |
| failed_actions_total | 36 | Failed tool/action observations from trace and event logs. |

### Score Matrix

| task | outcome | process | efficiency | agent | invalid | premature_edit | scope_drift | repeated | failed_actions | penalties |
|------|---------|---------|------------|-------|---------|----------------|-------------|----------|----------------|-----------|
| backend-todo-api-crud | 100 | 60 | 51 | 78 | 5 | 0 | 0 | 5 | 3 | repeated_action,invalid_action,failed_actions,repeated_actions,user_questions |
| code-change-verification-repair-loop | 100 | 100 | 84 | 97 | 0 | 0 | 0 | 0 | 2 | failed_actions |
| core-inspection-grounding | 100 | 80 | 75 | 89 | 4 | 0 | 0 | 0 | 4 | invalid_action,failed_actions |
| core-long-output-artifact | 100 | 100 | 74 | 95 | 0 | 0 | 0 | 0 | 2 | failed_actions,user_questions |
| core-multi-file-edit | 100 | 100 | 100 | 100 | 0 | 0 | 0 | 0 | 0 | none |
| core-permission-rejection-recovery | 0 | 100 | 84 | 47 | 0 | 0 | 0 | 0 | 2 | run_failed,required_commands_failed,verification_failed,closeout_not_successful,expected_code_diff_missing,failed_actions |
| core-provider-roundtrip | 100 | 60 | 54 | 79 | 7 | 0 | 0 | 7 | 2 | repeated_action,invalid_action,failed_actions,repeated_actions,llm_call_budget_pressure |
| core-rollback-product-path | 100 | 64 | 29 | 75 | 4 | 0 | 0 | 2 | 2 | repeated_action,invalid_action,tool_budget_exceeded,failed_actions,repeated_actions,user_questions,llm_call_budget_pressure |
| core-simple-stale-edit | 100 | 100 | 100 | 100 | 0 | 0 | 0 | 0 | 0 | none |
| core-terminal-install-run | 100 | 74 | 78 | 88 | 2 | 0 | 0 | 2 | 1 | repeated_action,invalid_action,failed_actions,repeated_actions |
| frontend-book-notes-localstorage | 100 | 60 | 80 | 84 | 5 | 0 | 0 | 5 | 0 | repeated_action,invalid_action,repeated_actions |
| live-eval-dashboard-summary | 100 | 80 | 84 | 91 | 1 | 0 | 1 | 0 | 2 | scope_drift,invalid_action,failed_actions |
| memory-failure-lesson-promotion | 0 | 65 | 64 | 32 | 3 | 0 | 0 | 3 | 2 | run_failed,required_commands_failed,verification_failed,closeout_not_successful,behavior_assertions_failed,expected_code_diff_missing,repeated_action,invalid_action,failed_actions,repeated_actions |
| memory-save-quality-gate | 0 | 87 | 68 | 40 | 1 | 0 | 0 | 1 | 6 | run_failed,required_commands_failed,verification_failed,closeout_not_successful,behavior_assertions_failed,expected_code_diff_missing,repeated_action,invalid_action,failed_actions,repeated_actions |
| memory-stale-project-fact-demotion | 5 | 30 | 55 | 22 | 6 | 0 | 2 | 4 | 4 | run_failed,required_commands_failed,verification_failed,closeout_not_successful,behavior_assertions_failed,scope_drift,repeated_action,invalid_action,failed_actions,repeated_actions |
| persistent-memory-planning-context | 100 | 100 | 100 | 100 | 0 | 0 | 0 | 0 | 0 | none |
| skill-promotion-gate | 0 | 74 | 61 | 34 | 2 | 0 | 0 | 2 | 4 | run_failed,required_commands_failed,verification_failed,closeout_not_successful,behavior_assertions_failed,expected_code_diff_missing,repeated_action,invalid_action,failed_actions,repeated_actions |

## Outcome Classes

| class | count | meaning |
|-------|-------|---------|
| real_code_change_passed | 9 | Agent-run tasks with passing status and a real diff. |
| plan_only_passed | 0 | Planning/API-only artifacts that passed their available checks. |
| seeded_no_diff_failed | 4 | Seeded code-change tasks where the agent did not produce a diff. |

## Coding Gauntlet Evidence

| task | gauntlet_status | first_pass_signal | failure_class | coding | required | closeout | spine | contract | risk | first_write | diff | warnings |
|------|-----------------|-------------------|---------------|--------|----------|----------|-------|----------|------|-------------|------|----------|
| backend-todo-api-crud | passed | repaired | tool_contract,file_state,permission_recovery,desktop_evidence | tools=8, tool_records=31, validations=4, repair=4, files=1 | ok | passed | coverage=6/7, status=none, missing=none | entry=active:force repair=active_after_failure | entry=high runtime=high | 3 | yes | none |
| code-change-verification-repair-loop | passed | repaired | tool_contract,file_state,permission_recovery,desktop_evidence | tools=3, tool_records=5, validations=2, repair=2, files=1 | ok | passed | coverage=6/7, status=passed, missing=none | entry=active:force repair=not_needed | entry=high runtime=none | 3 | yes | tool_errors_seen |
| core-inspection-grounding | passed | repaired | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | tools=1, tool_records=6, validations=0, repair=5, files=0 | ok | passed | coverage=6/7, status=passed, missing=none | entry=active:force repair=active_after_failure | entry=high runtime=high | none | no | no_code_diff,audit_no_code_diff |
| core-long-output-artifact | passed | repaired | tool_contract,file_state,permission_recovery,desktop_evidence | tools=2, tool_records=3, validations=2, repair=2, files=1 | ok | passed | coverage=7/7, status=none, missing=none | entry=active:force repair=active_after_failure | entry=high runtime=high | none | yes | tool_errors_seen |
| core-multi-file-edit | passed | likely_clean | tool_contract,file_state,permission_recovery,desktop_evidence | tools=3, tool_records=5, validations=2, repair=0, files=2 | ok | passed | coverage=6/7, status=passed, missing=none | entry=active:force repair=not_needed | entry=high runtime=none | 3 | yes | none |
| core-permission-rejection-recovery | failed | failed | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | tools=1, tool_records=6, validations=0, repair=2, files=0 | failed | failed | coverage=6/7, status=none, missing=none | entry=active:force repair=active_after_failure | entry=high runtime=high | none | no | no_code_diff |
| core-provider-roundtrip | passed | repaired | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | tools=13, tool_records=39, validations=0, repair=2, files=0 | ok | passed | coverage=6/7, status=none, missing=none | entry=active:force repair=not_needed | entry=high runtime=none | none | no | no_code_diff,audit_no_code_diff,tool_errors_seen |
| core-rollback-product-path | passed | repaired | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | tools=33, tool_records=243, validations=0, repair=3, files=0 | ok | passed | coverage=6/7, status=none, missing=none | entry=active:force repair=not_needed | entry=high runtime=none | none | no | no_code_diff,audit_no_code_diff |
| core-simple-stale-edit | passed | likely_clean | tool_contract,file_state,permission_recovery,desktop_evidence | tools=3, tool_records=5, validations=2, repair=0, files=1 | ok | passed | coverage=6/7, status=passed, missing=none | entry=active:force repair=not_needed | entry=high runtime=none | 3 | yes | none |
| core-terminal-install-run | passed | repaired | tool_contract,file_state,permission_recovery,desktop_evidence | tools=10, tool_records=27, validations=0, repair=2, files=0 | ok | passed | coverage=6/7, status=none, missing=none | entry=active:force repair=active_after_failure | entry=high runtime=high | none | yes | none |
| frontend-book-notes-localstorage | passed | likely_clean | tool_contract,file_state,permission_recovery,desktop_evidence | tools=14, tool_records=38, validations=2, repair=0, files=1 | ok | passed | coverage=6/7, status=none, missing=none | entry=active:force repair=not_needed | entry=high runtime=none | 9 | yes | none |
| live-eval-dashboard-summary | passed | repaired | tool_contract,file_state,permission_recovery,desktop_evidence | tools=6, tool_records=14, validations=2, repair=2, files=2 | ok | passed | coverage=7/7, status=none, missing=none | entry=active:force repair=not_needed | entry=high runtime=none | 6 | yes | tool_errors_seen |
| memory-failure-lesson-promotion | failed | failed | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | tools=8, tool_records=33, validations=0, repair=1, files=0 | failed | failed | coverage=6/7, status=none, missing=none | entry=active:force repair=not_needed | entry=high runtime=none | 8 | no | no_code_diff,tool_errors_seen,patch_synthesis_no_change |
| memory-save-quality-gate | failed | failed | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | tools=8, tool_records=16, validations=0, repair=3, files=0 | failed | failed | coverage=6/7, status=none, missing=none | entry=active:force repair=not_needed | entry=high runtime=none | 6 | no | no_code_diff,tool_errors_seen,patch_synthesis_no_change |
| memory-stale-project-fact-demotion | failed | failed | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | tools=17, tool_records=83, validations=4, repair=2, files=1 | failed | failed | coverage=7/7, status=none, missing=none | entry=active:force repair=active_after_failure | entry=high runtime=high | 13 | yes | tool_errors_seen |
| persistent-memory-planning-context | passed | likely_clean | tool_contract,file_state,permission_recovery,desktop_evidence | tools=6, tool_records=14, validations=2, repair=0, files=1 | ok | passed | coverage=6/7, status=none, missing=none | entry=active:force repair=not_needed | entry=high runtime=none | 6 | yes | none |
| skill-promotion-gate | failed | failed | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | tools=7, tool_records=15, validations=0, repair=2, files=0 | failed | failed | coverage=6/7, status=none, missing=none | entry=active:force repair=not_needed | entry=high runtime=none | 6 | no | no_code_diff,tool_errors_seen,patch_synthesis_no_change |

## Task Matrix

| task | status | intent | owner | failure_class | required | plan_quality | tool_boundary | verification_status | closeout | runtime_spine | runtime_diet | contract | risk | behavior_assertions | behavior_status | triggers | first_write | diff | memory | skill | warnings |
|------|--------|--------|-------|---------------|----------|--------------|---------------|---------------------|----------|---------------|--------------|----------|------|---------------------|-----------------|----------|-------------|------|--------|-------|----------|
| backend-todo-api-crud | passed | seeded_code_change | none | tool_contract,file_state,permission_recovery,desktop_evidence | ok | none | agent-run | passed | passed | coverage=6/7, status=none, missing=none | prompt=15803 tool_schema=3950 tools=19 workflow=strict closeout=full validation=passed:2/2 recovered_failed:2 | entry=active:force repair=active_after_failure | entry=high runtime=high | none | none | risk_signal_high,required_validation,first_code_change,verification_failed,acceptance_rejected | 3 | yes | active=false, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | none |
| code-change-verification-repair-loop | passed | seeded_code_change | none | tool_contract,file_state,permission_recovery,desktop_evidence | ok | none | agent-run | passed | passed | coverage=6/7, status=passed, missing=none | prompt=5045 tool_schema=3950 tools=19 workflow=strict closeout=full validation=passed:5/5 | entry=active:force repair=not_needed | entry=high runtime=none | none | none | risk_signal_high,required_validation,first_code_change | 3 | yes | active=false, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | tool_errors_seen |
| core-inspection-grounding | passed | audit_or_regression_check | none | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | ok | none | agent-run | passed | passed | coverage=6/7, status=passed, missing=none | prompt=5008 tool_schema=3950 tools=19 workflow=guarded closeout=full validation=passed:4/4 recovered_failed:4 | entry=active:force repair=active_after_failure | entry=high runtime=high | none | none | risk_signal_high,required_validation | none | no | active=false, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff,audit_no_code_diff |
| core-long-output-artifact | passed | audit_or_regression_check | none | tool_contract,file_state,permission_recovery,desktop_evidence | ok | none | agent-run | passed | passed | coverage=7/7, status=none, missing=none | prompt=4571 tool_schema=3950 tools=19 workflow=guarded closeout=full validation=passed:3/3 | entry=active:force repair=active_after_failure | entry=high runtime=high | none | none | risk_signal_high,required_validation,first_code_change | none | yes | active=false, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | tool_errors_seen |
| core-multi-file-edit | passed | seeded_code_change | none | tool_contract,file_state,permission_recovery,desktop_evidence | ok | none | agent-run | passed | passed | coverage=6/7, status=passed, missing=none | prompt=5335 tool_schema=3950 tools=19 workflow=guarded closeout=full validation=passed:3/3 recovered_failed:3 | entry=active:force repair=not_needed | entry=high runtime=none | none | none | risk_signal_high,required_validation,first_code_change | 3 | yes | active=false, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | none |
| core-permission-rejection-recovery | failed | seeded_code_change | llm_reasoning | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | failed | none | agent-run | failed | failed | coverage=6/7, status=none, missing=none | prompt=5636 tool_schema=3950 tools=19 workflow=strict closeout=full validation=failed:2/3 | entry=active:force repair=active_after_failure | entry=high runtime=high | none | none | risk_signal_high,required_validation | none | no | active=false, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff |
| core-provider-roundtrip | passed | audit_or_regression_check | none | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | ok | none | agent-run | passed | passed | coverage=6/7, status=none, missing=none | prompt=19367 tool_schema=3950 tools=19 workflow=guarded closeout=full validation=passed:1/1 | entry=active:force repair=not_needed | entry=high runtime=none | none | none | risk_signal_high,required_validation | none | no | active=false, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff,audit_no_code_diff,tool_errors_seen |
| core-rollback-product-path | passed | audit_or_regression_check | none | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | ok | none | agent-run | passed | passed | coverage=6/7, status=none, missing=none | prompt=22198 tool_schema=3950 tools=19 workflow=guarded closeout=full validation=passed:2/2 | entry=active:force repair=not_needed | entry=high runtime=none | none | none | risk_signal_high,required_validation | none | no | active=false, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff,audit_no_code_diff |
| core-simple-stale-edit | passed | seeded_code_change | none | tool_contract,file_state,permission_recovery,desktop_evidence | ok | none | agent-run | passed | passed | coverage=6/7, status=passed, missing=none | prompt=5176 tool_schema=3950 tools=19 workflow=guarded closeout=full validation=passed:2/2 recovered_failed:2 | entry=active:force repair=not_needed | entry=high runtime=none | none | none | risk_signal_high,required_validation,first_code_change | 3 | yes | active=false, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | none |
| core-terminal-install-run | passed | audit_or_regression_check | none | tool_contract,file_state,permission_recovery,desktop_evidence | ok | none | agent-run | passed | passed | coverage=6/7, status=none, missing=none | prompt=11491 tool_schema=4300 tools=20 workflow=guarded closeout=full validation=passed:2/2 | entry=active:force repair=active_after_failure | entry=high runtime=high | none | none | risk_signal_high,required_validation | none | yes | active=false, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | none |
| frontend-book-notes-localstorage | passed | seeded_code_change | none | tool_contract,file_state,permission_recovery,desktop_evidence | ok | none | agent-run | passed | passed | coverage=6/7, status=none, missing=none | prompt=12374 tool_schema=3950 tools=19 workflow=guarded closeout=full validation=passed:2/2 recovered_failed:2 | entry=active:force repair=not_needed | entry=high runtime=none | none | none | risk_signal_high,required_validation,repeated_no_code_progress,first_code_change | 9 | yes | active=false, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | none |
| live-eval-dashboard-summary | passed | seeded_code_change | none | tool_contract,file_state,permission_recovery,desktop_evidence | ok | none | agent-run | passed | passed | coverage=7/7, status=none, missing=none | prompt=5933 tool_schema=3950 tools=19 workflow=guarded closeout=full validation=passed:5/5 recovered_failed:1 | entry=active:force repair=not_needed | entry=high runtime=none | none | none | risk_signal_high,required_validation,repeated_no_code_progress,first_code_change | 6 | yes | active=true, recalled=3, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | tool_errors_seen |
| memory-failure-lesson-promotion | failed | seeded_code_change | llm_reasoning | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | failed | none | agent-run | failed | failed | coverage=6/7, status=none, missing=none | prompt=16178 tool_schema=3950 tools=19 workflow=strict closeout=full validation=failed:1/4 | entry=active:force repair=not_needed | entry=high runtime=none | memory_candidate_typed,memory_candidate_has_evidence,memory_failure_lesson_promoted,memory_scope_correct | failed | risk_signal_high,required_validation,repeated_no_code_progress | 8 | no | active=true, recalled=12, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff,tool_errors_seen,patch_synthesis_no_change |
| memory-save-quality-gate | failed | seeded_code_change | llm_reasoning | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | failed | none | agent-run | failed | failed | coverage=6/7, status=none, missing=none | prompt=8321 tool_schema=3950 tools=19 workflow=strict closeout=full validation=failed:3/4 | entry=active:force repair=not_needed | entry=high runtime=none | memory_quality_gate,memory_save_outcome_visibility | failed | risk_signal_high,required_validation | 6 | no | active=true, recalled=3, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff,tool_errors_seen,patch_synthesis_no_change |
| memory-stale-project-fact-demotion | failed | seeded_code_change | llm_reasoning | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | failed | none | agent-run | failed | failed | coverage=7/7, status=none, missing=none | prompt=26659 tool_schema=3950 tools=19 workflow=strict closeout=full validation=failed:2/4 | entry=active:force repair=active_after_failure | entry=high runtime=high | memory_record_used,memory_stale_demoted,memory_candidate_has_evidence,memory_scope_correct | failed | risk_signal_high,required_validation,repeated_no_code_progress,first_code_change,verification_failed | 13 | yes | active=true, recalled=18, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | tool_errors_seen |
| persistent-memory-planning-context | passed | seeded_code_change | none | tool_contract,file_state,permission_recovery,desktop_evidence | ok | none | agent-run | passed | passed | coverage=6/7, status=none, missing=none | prompt=15111 tool_schema=3950 tools=19 workflow=strict closeout=full validation=passed:6/6 recovered_failed:2 | entry=active:force repair=not_needed | entry=high runtime=none | memory_planning_context,memory_retrieval_before_workflow_judgment | passed | risk_signal_high,required_validation,first_code_change | 6 | yes | active=true, recalled=3, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | none |
| skill-promotion-gate | failed | seeded_code_change | llm_reasoning | tool_contract,file_state,permission_recovery,llm_reasoning,desktop_evidence | failed | none | agent-run | failed | failed | coverage=6/7, status=none, missing=none | prompt=8458 tool_schema=3950 tools=19 workflow=guarded closeout=full validation=failed:2/5 | entry=active:force repair=not_needed | entry=high runtime=none | skill_promotion_gate,skill_evolution_cooldown | failed | risk_signal_high,required_validation | 6 | no | active=true, recalled=3, conflicts=0, changed_plan=false | active=true, tool_calls=0, usage_events=0, promotion=true | no_code_diff,tool_errors_seen,patch_synthesis_no_change |

## Notes

- `plan_quality` describes plan-only/API artifacts when present.
- `tool_boundary` separates plan-only, collect-only, and real agent-run reports.
- `verification_status` combines closeout and required-command evidence; it is not a human-quality score.
- `skipped` reports are excluded from pass/fail rate denominators; collect-only reports need passing required commands to be scored.
- `real_code_change_passed` requires an agent-run report with a non-empty diff; plan-only success is tracked separately.
- `memory` and `skill` summarize evidence signals; they do not by themselves mean the task succeeded.
- `behavior_assertions` are explicit sample-level checks; memory/skill behavior assertions are stronger evidence than activity signals alone.
- `runtime_spine` summarizes trace/control-loop coverage and explicit runtime-spine assertions.
