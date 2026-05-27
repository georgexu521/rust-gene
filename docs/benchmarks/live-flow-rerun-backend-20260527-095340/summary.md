# Live Eval Summary: flow-rerun-backend-20260527-095340

- Run directory: `docs/benchmarks/live-flow-rerun-backend-20260527-095340`
- Tasks found: `1`
- Pass rate: `1/1` (100.0%)
- Failure rate: `0/1` (0.0%)
- Skipped/unscored tasks: `0`
- Real code-change passes: `1`
- Plan-only passes: `0`
- Seeded no-diff failures: `0`
- Memory active tasks: `1`
- Memory changed-plan tasks: `0`
- Memory recalled items: `0`
- Memory conflicts: `0`
- Memory typed-candidate tasks: `1`
- Memory evidence-backed candidate tasks: `1`
- Memory proposal tasks: `1`
- Memory proposal candidates: `1`
- Memory proposal evidence items: `7`
- Memory proposal review-required tasks: `1`
- Skill active tasks: `0`
- Skill promotion-evidence tasks: `0`
- Behavior assertion tasks: `0`
- Behavior assertions passed: `0`
- Runtime-spine assertion tasks: `0`
- Runtime-spine assertions passed: `0`
- Runtime-spine assertions failed: `0`
- Runtime-spine full coverage tasks: `0`
- Runtime-spine trace-present tasks: `1`
- Runtime-spine risky tool runs: `8`
- Runtime-spine risky tool reviewed: `8`
- Runtime-spine risky missing-review tasks: `0`
- Route recovery tasks: `1`
- Route recovery events: `2`
- Route recovery read/search expansions: `0`
- Route recovery mutation blocks: `0`
- Route recovery safety-monotonic tasks: `1`
- Route recovery unsafe mutation-expansion tasks: `0`
- Context-zone envelope tasks: `1`
- Context-zone envelope messages: `1`
- Context-zone source messages: `4`
- Context-zone duplicate blocks removed: `0`
- Context-zone provenance markers: `3`
- Gate outcome tasks: `1`
- Gate outcome records: `22`
- Gate outcome protective blocks: `0`
- Gate outcome recoverable friction: `0`
- Gate outcome unrecovered blocks: `0`
- Gate outcome harmless passes: `22`
- Proof support verified tasks: `1`
- Proof support partial tasks: `0`
- Proof support not-verified tasks: `0`
- Proof support residual-risk tasks: `0`
- Average outcome score: `100.0`
- Average process score: `60.0`
- Average efficiency score: `80.0`
- Average agent score: `84.0`
- Invalid actions total: `14`
- Premature edits total: `0`
- Scope drifts total: `0`
- Repeated actions total: `14`
- Failed actions total: `0`
- Coding gauntlet agent-run tasks: `1`
- Coding gauntlet passes: `1`
- Coding gauntlet failures: `0`
- Coding gauntlet likely clean passes: `1`
- Coding gauntlet repaired passes: `0`
- Coding gauntlet required-validation passes: `1/1`
- Coding gauntlet first-write observed: `1/1`
- Coding gauntlet repair signals: `0`
- Coding gauntlet changed files: `1`
- Status counts: passed=1
- Failure owners: none=1
- Eval intents: seeded_code_change=1

## Failure Modes

- `earlier_stage_validation_failed_before_repair`: `1`
- `earlier_verification_failed_before_repair`: `1`

## Release Dogfood Failure Classes

| class | count | meaning |
|-------|-------|---------|
| desktop_evidence | 1 | Desktop UI, screenshot, native smoke, or visual evidence failures. |
| file_state | 1 | Read-before-edit, stale file, checkpoint, rollback, or diff-state failures. |
| permission_recovery | 1 | Permission denial, approval, or recovery-loop failures. |
| tool_contract | 1 | Tool schema, exposure, result-pair, or contract boundary failures. |

## Memory And Skill Evidence

| dimension | count | meaning |
|-----------|-------|---------|
| memory_active_tasks | 1 | Tasks where retrieval, sync, or memory tools were active. |
| memory_changed_plan_tasks | 0 | Tasks where memory or learning signals reweighted planning. |
| memory_recalled_items | 0 | Retrieved memory-backed context items across tasks. |
| memory_conflicts | 0 | Retrieval-context conflict count from memory-backed context. |
| memory_candidate_typed_tasks | 1 | Tasks with typed memory candidates, including review-only MemoryProposal candidates. |
| memory_candidate_evidence_tasks | 1 | Tasks with evidence-backed memory candidates, including review-only MemoryProposal evidence. |
| memory_proposal_tasks | 1 | Tasks that emitted a review-only MemoryProposal trace event. |
| memory_proposal_candidates | 1 | Review-only MemoryProposal candidates proposed across tasks. |
| memory_proposal_evidence_items | 7 | Evidence items attached to review-only MemoryProposal candidates. |
| memory_proposal_review_required_tasks | 1 | MemoryProposal tasks that require review before persistence. |
| skill_active_tasks | 0 | Tasks where skill tools or skill-specific signals were active. |
| skill_promotion_evidence_tasks | 0 | Tasks with promotion-related skill evidence. |
| behavior_assertion_tasks | 0 | Tasks with explicit behavior assertions in the live-eval sample. |
| behavior_assertions_passed | 0 | Explicit behavior-assertion tasks whose required checks passed. |
| memory_behavior_assertion_tasks | 0 | Behavior assertions covering memory semantics rather than only memory activity signals. |
| skill_behavior_assertion_tasks | 0 | Behavior assertions covering skill semantics rather than only skill activity signals. |

## Runtime Spine Evidence

| dimension | count | meaning |
|-----------|-------|---------|
| runtime_spine_assertion_tasks | 0 | Tasks with explicit runtime-spine assertions in the live-eval sample or report. |
| runtime_spine_assertions_passed | 0 | Runtime-spine assertion tasks whose required trace/control-loop signals were present. |
| runtime_spine_assertions_failed | 0 | Runtime-spine assertion tasks missing required trace/control-loop signals. |
| runtime_spine_full_coverage_tasks | 0 | Tasks whose trace touched all runtime-spine phases. |
| runtime_spine_trace_present_tasks | 1 | Tasks with a trace summary available to the report parser. |
| runtime_spine_risky_tool_runs | 8 | Risky tool executions observed from trace or agent events. |
| runtime_spine_risky_tool_reviewed | 8 | Risky tool executions with matching action.review trace evidence. |
| runtime_spine_risky_missing_review_tasks | 0 | Tasks with risky tool executions missing matching action.review evidence. |
| route_recovery_tasks | 1 | Tasks with route-recovery plans emitted by the runtime. |
| route_recovery_events | 2 | Route-recovery plans observed across task traces. |
| route_recovery_read_search_expansions | 0 | Tasks where route recovery expanded only read/search understanding tools. |
| route_recovery_mutation_blocks | 0 | Tasks where route recovery explicitly blocked silent mutation expansion. |
| route_recovery_safety_monotonic_tasks | 1 | Tasks where route recovery preserved destructive-tool authority. |
| route_recovery_unsafe_mutation_expansion_tasks | 0 | Tasks where route recovery exposed mutation alternatives and should be investigated. |
| context_zone_envelope_tasks | 1 | Tasks where dynamic context was consolidated into a primary zone-first envelope. |
| context_zone_envelope_messages | 1 | Consolidated context-zone envelope messages observed across tasks. |
| context_zone_source_messages | 4 | Dynamic source messages consumed into context-zone envelopes. |
| context_zone_duplicate_blocks_removed | 0 | Duplicate dynamic zone blocks removed during request assembly. |
| context_zone_provenance_markers | 3 | Provenance markers preserved inside context-zone envelopes. |
| gate_outcome_tasks | 1 | Tasks with derived gate-outcome records from trace or report fields. |
| gate_outcome_records | 22 | Total gate-outcome records derived across action review, permission, and closeout gates. |
| gate_outcome_protective_blocks | 0 | Gate blocks that protected policy, scope, budget, checkpoint, or closeout invariants. |
| gate_outcome_recoverable_friction | 0 | Gate friction followed by a completed or passed runtime outcome. |
| gate_outcome_unrecovered_blocks | 0 | Gate blocks without later runtime recovery evidence. |
| gate_outcome_suspected_false_positives | 0 | Scenario-oracle suspected gate false positives. |
| gate_outcome_policy_correct_but_ux_costly | 0 | Policy-correct gate decisions that still created measurable UX cost. |
| gate_outcome_harmless_passes | 22 | Gate decisions that passed without measurable friction. |
| proof_support_verified_tasks | 1 | Tasks whose proof-kind policy supports verified closeout. |
| proof_support_partial_tasks | 0 | Tasks with useful proof evidence that cannot support verified closeout. |
| proof_support_not_verified_tasks | 0 | Tasks whose proof policy blocks verified closeout. |
| proof_support_residual_risk_tasks | 0 | Tasks whose proof support carries residual risk. |

### Gate Outcome Matrix

| task | total | protective | recoverable | unrecovered | suspected_false_positive | policy_correct_but_ux_costly | harmless | records | owners |
|------|-------|------------|-------------|-------------|--------------------------|------------------------------|----------|---------|--------|
| backend-todo-api-crud | 22 | 0 | 0 | 0 | 0 | 0 | 22 | action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,action_review:allow:harmless_pass,+10 | none |

### Proof Support Matrix

| task | proof_status | support_status | supports_verified | residual_risk | proof_kinds | support_summary |
|------|--------------|----------------|-------------------|---------------|-------------|-----------------|
| backend-todo-api-crud | verified | verified | true | false | command_passed,required_validation_passed | verified by command_passed,required_validation_passed |

### Context Zone Matrix

| task | materialized | envelopes | sources | dedupe_removed | provenance | task_state_empty | current_request_empty |
|------|--------------|-----------|---------|----------------|------------|------------------|-----------------------|
| backend-todo-api-crud | true | 1 | 4 | 0 | 3 | false | false |

### Route Recovery Matrix

| task | events | kinds | failure_types | read_search | mutation_blocked | safety_monotonic | unsafe_mutation_expansion | summary |
|------|--------|-------|---------------|-------------|------------------|------------------|---------------------------|---------|
| backend-todo-api-crud | 2 | code_change_no_diff_replan | code_change_no_diff_after_repeated_progress | false | false | true | false | events=2, read_search=false, mutation_blocked=false, safety=true |

## Evaluation Scores

| dimension | value | meaning |
|-----------|-------|---------|
| outcome_score_avg | 100.0 | Average deterministic outcome score across task reports. |
| process_score_avg | 60.0 | Average deterministic process score across task reports. |
| efficiency_score_avg | 80.0 | Average deterministic efficiency score across task reports. |
| agent_score_avg | 84.0 | Weighted score: outcome 50%, process 30%, efficiency 20%. |
| invalid_actions_total | 14 | Premature edits, scope drift, repeated actions, risky review gaps, and phase-misaligned actions. |
| premature_edits_total | 0 | Edits attempted before enough evidence or explicitly demoted as early/low-value. |
| scope_drifts_total | 0 | Action decisions with very low scope fit or medium/high goal drift. |
| repeated_actions_total | 14 | Repeated tool actions or repeated-action stop signals. |
| failed_actions_total | 0 | Failed tool/action observations from trace and event logs. |

### Score Matrix

| task | outcome | process | efficiency | agent | invalid | premature_edit | scope_drift | repeated | failed_actions | penalties |
|------|---------|---------|------------|-------|---------|----------------|-------------|----------|----------------|-----------|
| backend-todo-api-crud | 100 | 60 | 80 | 84 | 14 | 0 | 0 | 14 | 0 | repeated_action,invalid_action,repeated_actions |

## Outcome Classes

| class | count | meaning |
|-------|-------|---------|
| real_code_change_passed | 1 | Agent-run tasks with passing status and a real diff. |
| plan_only_passed | 0 | Planning/API-only artifacts that passed their available checks. |
| seeded_no_diff_failed | 0 | Seeded code-change tasks where the agent did not produce a diff. |

## Coding Gauntlet Evidence

| task | gauntlet_status | first_pass_signal | failure_class | coding | required | closeout | spine | contract | risk | first_write | diff | warnings |
|------|-----------------|-------------------|---------------|--------|----------|----------|-------|----------|------|-------------|------|----------|
| backend-todo-api-crud | passed | likely_clean | tool_contract,file_state,permission_recovery,desktop_evidence | tools=21, tool_records=118, validations=8, repair=0, files=1 | ok | passed | coverage=6/7, status=none, missing=none | entry=active:force repair=active_after_failure | entry=high runtime=high | 11 | yes | none |

## Task Matrix

| task | status | intent | owner | failure_class | required | plan_quality | tool_boundary | verification_status | closeout | runtime_spine | runtime_diet | contract | risk | behavior_assertions | behavior_status | triggers | first_write | diff | memory | skill | warnings |
|------|--------|--------|-------|---------------|----------|--------------|---------------|---------------------|----------|---------------|--------------|----------|------|---------------------|-----------------|----------|-------------|------|--------|-------|----------|
| backend-todo-api-crud | passed | seeded_code_change | none | tool_contract,file_state,permission_recovery,desktop_evidence | ok | none | agent-run | passed | passed | coverage=6/7, status=none, missing=none | prompt=21579 tool_schema=3950 tools=19 workflow=strict closeout=full validation=passed:2/2 recovered_failed:1 | entry=active:force repair=active_after_failure | entry=high runtime=high | none | none | risk_signal_high,required_validation,repeated_no_code_progress,first_code_change,verification_failed | 11 | yes | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | none |

## Notes

- `plan_quality` describes plan-only/API artifacts when present.
- `tool_boundary` separates plan-only, collect-only, and real agent-run reports.
- `verification_status` combines closeout and required-command evidence; it is not a human-quality score.
- `skipped` reports are excluded from pass/fail rate denominators; collect-only reports need passing required commands to be scored.
- `real_code_change_passed` requires an agent-run report with a non-empty diff; plan-only success is tracked separately.
- `memory` and `skill` summarize evidence signals; they do not by themselves mean the task succeeded.
- `behavior_assertions` are explicit sample-level checks; memory/skill behavior assertions are stronger evidence than activity signals alone.
- `runtime_spine` summarizes trace/control-loop coverage and explicit runtime-spine assertions.
