# Live Eval Summary: capability-now-20260509-135556

- Run directory: `docs/benchmarks/live-capability-now-20260509-135556`
- Tasks found: `1`
- Pass rate: `1/1` (100.0%)
- Failure rate: `0/1` (0.0%)
- Real code-change passes: `1`
- Plan-only passes: `0`
- Seeded no-diff failures: `0`
- Status counts: passed=1
- Failure owners: none=1
- Eval intents: seeded_code_change=1

## Failure Modes

- none

## Outcome Classes

| class | count | meaning |
|-------|-------|---------|
| real_code_change_passed | 1 | Agent-run tasks with passing status and a real diff. |
| plan_only_passed | 0 | Planning/API-only artifacts that passed their available checks. |
| seeded_no_diff_failed | 0 | Seeded code-change tasks where the agent did not produce a diff. |

## Task Matrix

| task | status | intent | owner | required | plan_quality | tool_boundary | verification_status | closeout | runtime_diet | triggers | first_write | diff | warnings |
|------|--------|--------|-------|----------|--------------|---------------|---------------------|----------|--------------|----------|-------------|------|----------|
| code-change-verification-repair-loop | passed | seeded_code_change | none | ok | none | agent-run | passed | passed | prompt=3792 tool_schema=2641 tools=12 workflow=strict closeout=full validation=passed | required_validation,repeated_no_code_progress,first_code_change | 6 | yes | none |

## Notes

- `plan_quality` describes plan-only/API artifacts when present.
- `tool_boundary` separates plan-only, collect-only, and real agent-run reports.
- `verification_status` combines closeout and required-command evidence; it is not a human-quality score.
- `real_code_change_passed` requires an agent-run report with a non-empty diff; plan-only success is tracked separately.
