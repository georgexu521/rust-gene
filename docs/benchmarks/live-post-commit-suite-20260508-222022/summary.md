# Live Eval Summary: post-commit-suite-20260508-222022

- Run directory: `docs/benchmarks/live-post-commit-suite-20260508-222022`
- Tasks found: `1`
- Pass rate: `0/1` (0.0%)
- Failure rate: `1/1` (100.0%)
- Real code-change passes: `0`
- Plan-only passes: `0`
- Seeded no-diff failures: `1`
- Status counts: failed=1
- Failure owners: llm_reasoning=1
- Eval intents: seeded_code_change=1

## Failure Modes

- `closeout_not_successful`: `1`
- `expected_code_diff_missing`: `1`
- `required_commands_not_passing`: `1`
- `warning:no_code_diff`: `1`

## Outcome Classes

| class | count | meaning |
|-------|-------|---------|
| real_code_change_passed | 0 | Agent-run tasks with passing status and a real diff. |
| plan_only_passed | 0 | Planning/API-only artifacts that passed their available checks. |
| seeded_no_diff_failed | 1 | Seeded code-change tasks where the agent did not produce a diff. |

## Task Matrix

| task | status | intent | owner | required | plan_quality | tool_boundary | verification_status | closeout | runtime_diet | triggers | first_write | diff | warnings |
|------|--------|--------|-------|----------|--------------|---------------|---------------------|----------|--------------|----------|-------------|------|----------|
| live-eval-dashboard-summary | failed | seeded_code_change | llm_reasoning | failed | none | agent-run | failed | missing | missing | required_validation | none | no | no_code_diff |

## Notes

- `plan_quality` describes plan-only/API artifacts when present.
- `tool_boundary` separates plan-only, collect-only, and real agent-run reports.
- `verification_status` combines closeout and required-command evidence; it is not a human-quality score.
- `real_code_change_passed` requires an agent-run report with a non-empty diff; plan-only success is tracked separately.
