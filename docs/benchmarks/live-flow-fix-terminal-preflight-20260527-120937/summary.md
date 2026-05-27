# Live Eval Summary: flow-fix-terminal-preflight-20260527-120937

- Run directory: `docs/benchmarks/live-flow-fix-terminal-preflight-20260527-120937`
- Tasks found: `1`
- Pass rate: `0/1` (0.0%)
- Failure rate: `1/1` (100.0%)
- Skipped/unscored tasks: `0`
- Real code-change passes: `0`
- Plan-only passes: `0`
- Seeded no-diff failures: `0`
- Memory active tasks: `0`
- Memory changed-plan tasks: `0`
- Memory recalled items: `0`
- Memory conflicts: `0`
- Memory typed-candidate tasks: `0`
- Memory evidence-backed candidate tasks: `0`
- Memory proposal tasks: `0`
- Memory proposal candidates: `0`
- Memory proposal evidence items: `0`
- Memory proposal review-required tasks: `0`
- Skill active tasks: `0`
- Skill promotion-evidence tasks: `0`
- Behavior assertion tasks: `0`
- Behavior assertions passed: `0`
- Runtime-spine assertion tasks: `0`
- Runtime-spine assertions passed: `0`
- Runtime-spine assertions failed: `0`
- Runtime-spine full coverage tasks: `0`
- Runtime-spine trace-present tasks: `0`
- Runtime-spine risky tool runs: `2`
- Runtime-spine risky tool reviewed: `0`
- Runtime-spine risky missing-review tasks: `1`
- Route recovery tasks: `0`
- Route recovery events: `0`
- Route recovery read/search expansions: `0`
- Route recovery mutation blocks: `0`
- Route recovery safety-monotonic tasks: `0`
- Route recovery unsafe mutation-expansion tasks: `0`
- Context-zone envelope tasks: `0`
- Context-zone envelope messages: `0`
- Context-zone source messages: `0`
- Context-zone duplicate blocks removed: `0`
- Context-zone provenance markers: `0`
- Gate outcome tasks: `0`
- Gate outcome records: `0`
- Gate outcome protective blocks: `0`
- Gate outcome recoverable friction: `0`
- Gate outcome unrecovered blocks: `0`
- Gate outcome harmless passes: `0`
- Proof support verified tasks: `0`
- Proof support partial tasks: `0`
- Proof support not-verified tasks: `0`
- Proof support residual-risk tasks: `0`
- Average outcome score: `15.0`
- Average process score: `53.0`
- Average efficiency score: `87.0`
- Average agent score: `41.0`
- Invalid actions total: `2`
- Premature edits total: `0`
- Scope drifts total: `0`
- Repeated actions total: `0`
- Failed actions total: `1`
- Coding gauntlet agent-run tasks: `1`
- Coding gauntlet passes: `0`
- Coding gauntlet failures: `1`
- Coding gauntlet likely clean passes: `0`
- Coding gauntlet repaired passes: `0`
- Coding gauntlet required-validation passes: `0/1`
- Coding gauntlet first-write observed: `0/1`
- Coding gauntlet repair signals: `0`
- Coding gauntlet changed files: `537`
- Status counts: failed=1
- Failure owners: agent_flow=1
- Eval intents: audit_or_regression_check=1

## Failure Modes

- `closeout_not_successful`: `1`
- `empty_agent_output`: `1`
- `max_files_changed_exceeded`: `1`
- `missing_trace_summary`: `1`
- `required_commands_not_passing`: `1`
- `tool_run_without_closeout`: `1`
- `warning:tool_errors_seen`: `1`

## Release Dogfood Failure Classes

| class | count | meaning |
|-------|-------|---------|
| desktop_evidence | 1 | Desktop UI, screenshot, native smoke, or visual evidence failures. |
| file_state | 1 | Read-before-edit, stale file, checkpoint, rollback, or diff-state failures. |
| llm_reasoning | 1 | Model failed to plan, edit, validate, or close out despite available tools. |
| tool_contract | 1 | Tool schema, exposure, result-pair, or contract boundary failures. |

## Memory And Skill Evidence

| dimension | count | meaning |
|-----------|-------|---------|
| memory_active_tasks | 0 | Tasks where retrieval, sync, or memory tools were active. |
| memory_changed_plan_tasks | 0 | Tasks where memory or learning signals reweighted planning. |
| memory_recalled_items | 0 | Retrieved memory-backed context items across tasks. |
| memory_conflicts | 0 | Retrieval-context conflict count from memory-backed context. |
| memory_candidate_typed_tasks | 0 | Tasks with typed memory candidates, including review-only MemoryProposal candidates. |
| memory_candidate_evidence_tasks | 0 | Tasks with evidence-backed memory candidates, including review-only MemoryProposal evidence. |
| memory_proposal_tasks | 0 | Tasks that emitted a review-only MemoryProposal trace event. |
| memory_proposal_candidates | 0 | Review-only MemoryProposal candidates proposed across tasks. |
| memory_proposal_evidence_items | 0 | Evidence items attached to review-only MemoryProposal candidates. |
| memory_proposal_review_required_tasks | 0 | MemoryProposal tasks that require review before persistence. |
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
| runtime_spine_trace_present_tasks | 0 | Tasks with a trace summary available to the report parser. |
| runtime_spine_risky_tool_runs | 2 | Risky tool executions observed from trace or agent events. |
| runtime_spine_risky_tool_reviewed | 0 | Risky tool executions with matching action.review trace evidence. |
| runtime_spine_risky_missing_review_tasks | 1 | Tasks with risky tool executions missing matching action.review evidence. |
| route_recovery_tasks | 0 | Tasks with route-recovery plans emitted by the runtime. |
| route_recovery_events | 0 | Route-recovery plans observed across task traces. |
| route_recovery_read_search_expansions | 0 | Tasks where route recovery expanded only read/search understanding tools. |
| route_recovery_mutation_blocks | 0 | Tasks where route recovery explicitly blocked silent mutation expansion. |
| route_recovery_safety_monotonic_tasks | 0 | Tasks where route recovery preserved destructive-tool authority. |
| route_recovery_unsafe_mutation_expansion_tasks | 0 | Tasks where route recovery exposed mutation alternatives and should be investigated. |
| context_zone_envelope_tasks | 0 | Tasks where dynamic context was consolidated into a primary zone-first envelope. |
| context_zone_envelope_messages | 0 | Consolidated context-zone envelope messages observed across tasks. |
| context_zone_source_messages | 0 | Dynamic source messages consumed into context-zone envelopes. |
| context_zone_duplicate_blocks_removed | 0 | Duplicate dynamic zone blocks removed during request assembly. |
| context_zone_provenance_markers | 0 | Provenance markers preserved inside context-zone envelopes. |
| gate_outcome_tasks | 0 | Tasks with derived gate-outcome records from trace or report fields. |
| gate_outcome_records | 0 | Total gate-outcome records derived across action review, permission, and closeout gates. |
| gate_outcome_protective_blocks | 0 | Gate blocks that protected policy, scope, budget, checkpoint, or closeout invariants. |
| gate_outcome_recoverable_friction | 0 | Gate friction followed by a completed or passed runtime outcome. |
| gate_outcome_unrecovered_blocks | 0 | Gate blocks without later runtime recovery evidence. |
| gate_outcome_suspected_false_positives | 0 | Scenario-oracle suspected gate false positives. |
| gate_outcome_policy_correct_but_ux_costly | 0 | Policy-correct gate decisions that still created measurable UX cost. |
| gate_outcome_harmless_passes | 0 | Gate decisions that passed without measurable friction. |
| proof_support_verified_tasks | 0 | Tasks whose proof-kind policy supports verified closeout. |
| proof_support_partial_tasks | 0 | Tasks with useful proof evidence that cannot support verified closeout. |
| proof_support_not_verified_tasks | 0 | Tasks whose proof policy blocks verified closeout. |
| proof_support_residual_risk_tasks | 0 | Tasks whose proof support carries residual risk. |

### Gate Outcome Matrix

| task | total | protective | recoverable | unrecovered | suspected_false_positive | policy_correct_but_ux_costly | harmless | records | owners |
|------|-------|------------|-------------|-------------|--------------------------|------------------------------|----------|---------|--------|
| core-terminal-install-run | 0 | 0 | 0 | 0 | 0 | 0 | 0 | none | none |

### Proof Support Matrix

| task | proof_status | support_status | supports_verified | residual_risk | proof_kinds | support_summary |
|------|--------------|----------------|-------------------|---------------|-------------|-----------------|
| core-terminal-install-run | missing | missing | false | false | none | missing |

### Context Zone Matrix

| task | materialized | envelopes | sources | dedupe_removed | provenance | task_state_empty | current_request_empty |
|------|--------------|-----------|---------|----------------|------------|------------------|-----------------------|
| core-terminal-install-run | false | 0 | 0 | 0 | 0 | false | false |

### Route Recovery Matrix

| task | events | kinds | failure_types | read_search | mutation_blocked | safety_monotonic | unsafe_mutation_expansion | summary |
|------|--------|-------|---------------|-------------|------------------|------------------|---------------------------|---------|
| core-terminal-install-run | 0 | none | none | false | false | missing | false | events=0, read_search=false, mutation_blocked=false, safety=missing |

## Evaluation Scores

| dimension | value | meaning |
|-----------|-------|---------|
| outcome_score_avg | 15.0 | Average deterministic outcome score across task reports. |
| process_score_avg | 53.0 | Average deterministic process score across task reports. |
| efficiency_score_avg | 87.0 | Average deterministic efficiency score across task reports. |
| agent_score_avg | 41.0 | Weighted score: outcome 50%, process 30%, efficiency 20%. |
| invalid_actions_total | 2 | Premature edits, scope drift, repeated actions, risky review gaps, and phase-misaligned actions. |
| premature_edits_total | 0 | Edits attempted before enough evidence or explicitly demoted as early/low-value. |
| scope_drifts_total | 0 | Action decisions with very low scope fit or medium/high goal drift. |
| repeated_actions_total | 0 | Repeated tool actions or repeated-action stop signals. |
| failed_actions_total | 1 | Failed tool/action observations from trace and event logs. |

### Score Matrix

| task | outcome | process | efficiency | agent | invalid | premature_edit | scope_drift | repeated | failed_actions | penalties |
|------|---------|---------|------------|-------|---------|----------------|-------------|----------|----------------|-----------|
| core-terminal-install-run | 15 | 53 | 87 | 41 | 2 | 0 | 0 | 0 | 1 | run_failed,required_commands_failed,verification_failed,closeout_not_successful,invalid_action,risky_tool_missing_review,observer_outcome_missing,stop_check_missing,failed_actions,user_questions |

## Outcome Classes

| class | count | meaning |
|-------|-------|---------|
| real_code_change_passed | 0 | Agent-run tasks with passing status and a real diff. |
| plan_only_passed | 0 | Planning/API-only artifacts that passed their available checks. |
| seeded_no_diff_failed | 0 | Seeded code-change tasks where the agent did not produce a diff. |

## Coding Gauntlet Evidence

| task | gauntlet_status | first_pass_signal | failure_class | coding | required | closeout | spine | contract | risk | first_write | diff | warnings |
|------|-----------------|-------------------|---------------|--------|----------|----------|-------|----------|------|-------------|------|----------|
| core-terminal-install-run | failed | failed | tool_contract,file_state,llm_reasoning,desktop_evidence | tools=11, tool_records=0, validations=0, repair=0, files=537 | failed | missing | coverage=0/7, status=none, missing=none | entry=missing repair=none | entry=missing runtime=none | none | yes | tool_errors_seen |

## Task Matrix

| task | status | intent | owner | failure_class | required | plan_quality | tool_boundary | verification_status | closeout | runtime_spine | runtime_diet | contract | risk | behavior_assertions | behavior_status | triggers | first_write | diff | memory | skill | warnings |
|------|--------|--------|-------|---------------|----------|--------------|---------------|---------------------|----------|---------------|--------------|----------|------|---------------------|-----------------|----------|-------------|------|--------|-------|----------|
| core-terminal-install-run | failed | audit_or_regression_check | agent_flow | tool_contract,file_state,llm_reasoning,desktop_evidence | failed | none | agent-run | failed | missing | coverage=0/7, status=none, missing=none | missing | entry=missing repair=none | entry=missing runtime=none | none | none | none | none | yes | active=false, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | tool_errors_seen |

## Notes

- `plan_quality` describes plan-only/API artifacts when present.
- `tool_boundary` separates plan-only, collect-only, and real agent-run reports.
- `verification_status` combines closeout and required-command evidence; it is not a human-quality score.
- `skipped` reports are excluded from pass/fail rate denominators; collect-only reports need passing required commands to be scored.
- `real_code_change_passed` requires an agent-run report with a non-empty diff; plan-only success is tracked separately.
- `memory` and `skill` summarize evidence signals; they do not by themselves mean the task succeeded.
- `behavior_assertions` are explicit sample-level checks; memory/skill behavior assertions are stronger evidence than activity signals alone.
- `runtime_spine` summarizes trace/control-loop coverage and explicit runtime-spine assertions.
