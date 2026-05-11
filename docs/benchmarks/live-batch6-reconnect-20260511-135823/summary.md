# Live Eval Summary: batch6-reconnect-20260511-135823

- Run directory: `docs/benchmarks/live-batch6-reconnect-20260511-135823`
- Tasks found: `1`
- Pass rate: `1/1` (100.0%)
- Failure rate: `0/1` (0.0%)
- Real code-change passes: `0`
- Plan-only passes: `0`
- Seeded no-diff failures: `0`
- Memory active tasks: `1`
- Memory changed-plan tasks: `0`
- Memory recalled items: `0`
- Memory conflicts: `0`
- Skill active tasks: `0`
- Skill promotion-evidence tasks: `0`
- Status counts: passed=1
- Failure owners: none=1
- Eval intents: audit_or_regression_check=1

## Failure Modes

- `warning:audit_no_code_diff`: `1`
- `warning:current_head_no_fixture_already_satisfied`: `1`
- `warning:no_code_diff`: `1`

## Memory And Skill Evidence

| dimension | count | meaning |
|-----------|-------|---------|
| memory_active_tasks | 1 | Tasks where retrieval, sync, or memory tools were active. |
| memory_changed_plan_tasks | 0 | Tasks where memory or learning signals reweighted planning. |
| memory_recalled_items | 0 | Retrieved memory-backed context items across tasks. |
| memory_conflicts | 0 | Retrieval-context conflict count from memory-backed context. |
| skill_active_tasks | 0 | Tasks where skill tools or skill-specific signals were active. |
| skill_promotion_evidence_tasks | 0 | Tasks with promotion-related skill evidence. |

## Outcome Classes

| class | count | meaning |
|-------|-------|---------|
| real_code_change_passed | 0 | Agent-run tasks with passing status and a real diff. |
| plan_only_passed | 0 | Planning/API-only artifacts that passed their available checks. |
| seeded_no_diff_failed | 0 | Seeded code-change tasks where the agent did not produce a diff. |

## Task Matrix

| task | status | intent | owner | required | plan_quality | tool_boundary | verification_status | closeout | runtime_diet | triggers | first_write | diff | memory | skill | warnings |
|------|--------|--------|-------|----------|--------------|---------------|---------------------|----------|--------------|----------|-------------|------|--------|-------|----------|
| permission-default-open-dangerous-guard | passed | audit_or_regression_check | none | ok | none | agent-run | passed | passed | prompt=15577 tool_schema=2641 tools=12 workflow=strict closeout=full validation=passed:3/3 | required_validation | none | no | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff,audit_no_code_diff,current_head_no_fixture_already_satisfied |

## Notes

- `plan_quality` describes plan-only/API artifacts when present.
- `tool_boundary` separates plan-only, collect-only, and real agent-run reports.
- `verification_status` combines closeout and required-command evidence; it is not a human-quality score.
- `real_code_change_passed` requires an agent-run report with a non-empty diff; plan-only success is tracked separately.
- `memory` and `skill` summarize evidence signals; they do not by themselves mean the task succeeded.
