# Live Eval Summary: product-maturity-memory-skill-rerun-20260517-124722

- Run directory: `docs/benchmarks/live-product-maturity-memory-skill-rerun-20260517-124722`
- Tasks found: `6`
- Pass rate: `3/6` (50.0%)
- Failure rate: `3/6` (50.0%)
- Skipped/unscored tasks: `0`
- Real code-change passes: `0`
- Plan-only passes: `0`
- Seeded no-diff failures: `0`
- Memory active tasks: `6`
- Memory changed-plan tasks: `2`
- Memory recalled items: `0`
- Memory conflicts: `0`
- Skill active tasks: `1`
- Skill promotion-evidence tasks: `1`
- Behavior assertion tasks: `6`
- Behavior assertions passed: `4`
- Status counts: failed=3, passed=3
- Failure owners: agent_flow=2, llm_reasoning=1, none=3
- Eval intents: audit_or_regression_check=3, seeded_code_change=3

## Failure Modes

- `warning:audit_no_code_diff`: `3`
- `warning:current_head_no_fixture_already_satisfied`: `3`
- `warning:no_code_diff`: `3`
- `behavior_assertions_not_passing`: `2`
- `closeout_not_successful`: `2`
- `earlier_stage_validation_failed_before_repair`: `2`
- `earlier_verification_failed_before_repair`: `2`
- `required_commands_not_passing`: `2`
- `acceptance_review_rejected`: `1`
- `stage_validation_failed`: `1`
- `unused imports: `build_project_retrieval_context` and `build_session_retrieval_context``: `1`
- `verification_failed`: `1`
- `warning:tool_errors_seen`: `1`

## Memory And Skill Evidence

| dimension | count | meaning |
|-----------|-------|---------|
| memory_active_tasks | 6 | Tasks where retrieval, sync, or memory tools were active. |
| memory_changed_plan_tasks | 2 | Tasks where memory or learning signals reweighted planning. |
| memory_recalled_items | 0 | Retrieved memory-backed context items across tasks. |
| memory_conflicts | 0 | Retrieval-context conflict count from memory-backed context. |
| skill_active_tasks | 1 | Tasks where skill tools or skill-specific signals were active. |
| skill_promotion_evidence_tasks | 1 | Tasks with promotion-related skill evidence. |
| behavior_assertion_tasks | 6 | Tasks with explicit behavior assertions in the live-eval sample. |
| behavior_assertions_passed | 4 | Explicit behavior-assertion tasks whose required checks passed. |
| memory_behavior_assertion_tasks | 5 | Behavior assertions covering memory semantics rather than only memory activity signals. |
| skill_behavior_assertion_tasks | 1 | Behavior assertions covering skill semantics rather than only skill activity signals. |

## Outcome Classes

| class | count | meaning |
|-------|-------|---------|
| real_code_change_passed | 0 | Agent-run tasks with passing status and a real diff. |
| plan_only_passed | 0 | Planning/API-only artifacts that passed their available checks. |
| seeded_no_diff_failed | 0 | Seeded code-change tasks where the agent did not produce a diff. |

## Task Matrix

| task | status | intent | owner | required | plan_quality | tool_boundary | verification_status | closeout | runtime_diet | behavior_assertions | behavior_status | triggers | first_write | diff | memory | skill | warnings |
|------|--------|--------|-------|----------|--------------|---------------|---------------------|----------|--------------|---------------------|-----------------|----------|-------------|------|--------|-------|----------|
| memory-recall-conflict-precision | passed | audit_or_regression_check | none | ok | none | agent-run | passed | passed | prompt=14411 tool_schema=3186 tools=15 workflow=strict closeout=full validation=passed:3/3 | memory_conflict_precision,memory_recall_demotion | passed | required_validation | none | no | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff,audit_no_code_diff,current_head_no_fixture_already_satisfied |
| memory-save-duplicate-demotion | passed | audit_or_regression_check | none | ok | none | agent-run | passed | passed | prompt=29638 tool_schema=3186 tools=15 workflow=guarded closeout=full validation=passed:2/2 | memory_duplicate_demotion,memory_namespace_precision | passed | required_validation | none | no | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff,audit_no_code_diff,current_head_no_fixture_already_satisfied |
| memory-save-quality-gate | failed | seeded_code_change | agent_flow | ok | none | agent-run | failed | not_verified | prompt=8512 tool_schema=3186 tools=15 workflow=strict closeout=full validation=passed:7/7 | memory_quality_gate,memory_save_outcome_visibility | passed | required_validation,repeated_no_code_progress,first_code_change | 14 | yes | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | none |
| memory-save-sensitive-hard-block | passed | audit_or_regression_check | none | ok | none | agent-run | passed | passed | prompt=23987 tool_schema=3186 tools=15 workflow=strict closeout=full validation=passed:3/3 | memory_sensitive_hard_block,memory_save_outcome_visibility | passed | required_validation | none | no | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff,audit_no_code_diff,current_head_no_fixture_already_satisfied |
| persistent-memory-planning-context | failed | seeded_code_change | llm_reasoning | failed | none | agent-run | failed | failed | prompt=33386 tool_schema=3186 tools=15 workflow=strict closeout=full validation=failed:5/10 | memory_planning_context,memory_retrieval_before_workflow_judgment | failed | required_validation,repeated_no_code_progress,first_code_change,verification_failed,acceptance_rejected | 9 | yes | active=true, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | tool_errors_seen |
| skill-promotion-gate | failed | seeded_code_change | agent_flow | failed | none | agent-run | failed | passed | prompt=29759 tool_schema=3186 tools=15 workflow=strict closeout=full validation=failed:3/10 | skill_promotion_gate,skill_evolution_cooldown | failed | required_validation,repeated_no_code_progress,first_code_change,verification_failed,acceptance_rejected | 9 | yes | active=true, recalled=0, conflicts=0, changed_plan=true | active=true, tool_calls=0, usage_events=0, promotion=true | none |

## Notes

- `plan_quality` describes plan-only/API artifacts when present.
- `tool_boundary` separates plan-only, collect-only, and real agent-run reports.
- `verification_status` combines closeout and required-command evidence; it is not a human-quality score.
- `skipped` reports are excluded from pass/fail rate denominators; collect-only reports need passing required commands to be scored.
- `real_code_change_passed` requires an agent-run report with a non-empty diff; plan-only success is tracked separately.
- `memory` and `skill` summarize evidence signals; they do not by themselves mean the task succeeded.
- `behavior_assertions` are explicit sample-level checks; memory/skill behavior assertions are stronger evidence than activity signals alone.
