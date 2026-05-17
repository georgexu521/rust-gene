# Live Eval Summary: product-maturity-seeded-fixes-20260517-143047

- Run directory: `docs/benchmarks/live-product-maturity-seeded-fixes-20260517-143047`
- Tasks found: `3`
- Pass rate: `3/3` (100.0%)
- Failure rate: `0/3` (0.0%)
- Skipped/unscored tasks: `0`
- Real code-change passes: `3`
- Plan-only passes: `0`
- Seeded no-diff failures: `0`
- Memory active tasks: `3`
- Memory changed-plan tasks: `2`
- Memory recalled items: `0`
- Memory conflicts: `0`
- Skill active tasks: `1`
- Skill promotion-evidence tasks: `1`
- Behavior assertion tasks: `3`
- Behavior assertions passed: `3`
- Status counts: passed=3
- Failure owners: none=3
- Eval intents: seeded_code_change=3

## Failure Modes

- none

## Memory And Skill Evidence

| dimension | count | meaning |
|-----------|-------|---------|
| memory_active_tasks | 3 | Tasks where retrieval, sync, or memory tools were active. |
| memory_changed_plan_tasks | 2 | Tasks where memory or learning signals reweighted planning. |
| memory_recalled_items | 0 | Retrieved memory-backed context items across tasks. |
| memory_conflicts | 0 | Retrieval-context conflict count from memory-backed context. |
| skill_active_tasks | 1 | Tasks where skill tools or skill-specific signals were active. |
| skill_promotion_evidence_tasks | 1 | Tasks with promotion-related skill evidence. |
| behavior_assertion_tasks | 3 | Tasks with explicit behavior assertions in the live-eval sample. |
| behavior_assertions_passed | 3 | Explicit behavior-assertion tasks whose required checks passed. |
| memory_behavior_assertion_tasks | 2 | Behavior assertions covering memory semantics rather than only memory activity signals. |
| skill_behavior_assertion_tasks | 1 | Behavior assertions covering skill semantics rather than only skill activity signals. |

## Outcome Classes

| class | count | meaning |
|-------|-------|---------|
| real_code_change_passed | 3 | Agent-run tasks with passing status and a real diff. |
| plan_only_passed | 0 | Planning/API-only artifacts that passed their available checks. |
| seeded_no_diff_failed | 0 | Seeded code-change tasks where the agent did not produce a diff. |

## Task Matrix

| task | status | intent | owner | required | plan_quality | tool_boundary | verification_status | closeout | runtime_diet | behavior_assertions | behavior_status | triggers | first_write | diff | memory | skill | warnings |
|------|--------|--------|-------|----------|--------------|---------------|---------------------|----------|--------------|---------------------|-----------------|----------|-------------|------|--------|-------|----------|
| memory-save-quality-gate | passed | seeded_code_change | none | ok | none | agent-run | passed | passed | prompt=6521 tool_schema=3186 tools=15 workflow=strict closeout=full validation=passed:7/7 | memory_quality_gate,memory_save_outcome_visibility | passed | required_validation,repeated_no_code_progress,first_code_change | 10 | yes | active=true, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | none |
| persistent-memory-planning-context | passed | seeded_code_change | none | ok | none | agent-run | passed | passed | prompt=11969 tool_schema=3186 tools=15 workflow=strict closeout=full validation=passed:9/9 | memory_planning_context,memory_retrieval_before_workflow_judgment | passed | required_validation,first_code_change | 5 | yes | active=true, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | none |
| skill-promotion-gate | passed | seeded_code_change | none | ok | none | agent-run | passed | passed | prompt=7758 tool_schema=3186 tools=15 workflow=guarded closeout=full validation=passed:8/8 | skill_promotion_gate,skill_evolution_cooldown | passed | required_validation,repeated_no_code_progress,first_code_change | 9 | yes | active=true, recalled=0, conflicts=0, changed_plan=false | active=true, tool_calls=0, usage_events=0, promotion=true | none |

## Notes

- `plan_quality` describes plan-only/API artifacts when present.
- `tool_boundary` separates plan-only, collect-only, and real agent-run reports.
- `verification_status` combines closeout and required-command evidence; it is not a human-quality score.
- `skipped` reports are excluded from pass/fail rate denominators; collect-only reports need passing required commands to be scored.
- `real_code_change_passed` requires an agent-run report with a non-empty diff; plan-only success is tracked separately.
- `memory` and `skill` summarize evidence signals; they do not by themselves mean the task succeeded.
- `behavior_assertions` are explicit sample-level checks; memory/skill behavior assertions are stronger evidence than activity signals alone.
