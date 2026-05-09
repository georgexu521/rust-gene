# Live Eval Summary: capability-now-20260509-140733

- Run directory: `docs/benchmarks/live-capability-now-20260509-140733`
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

- `earlier_stage_validation_failed_before_repair`: `1`
- `earlier_verification_failed_before_repair`: `1`
- `warning:tool_errors_seen`: `1`

## Outcome Classes

| class | count | meaning |
|-------|-------|---------|
| real_code_change_passed | 1 | Agent-run tasks with passing status and a real diff. |
| plan_only_passed | 0 | Planning/API-only artifacts that passed their available checks. |
| seeded_no_diff_failed | 0 | Seeded code-change tasks where the agent did not produce a diff. |

## Task Matrix

| task | status | intent | owner | required | plan_quality | tool_boundary | verification_status | closeout | runtime_diet | triggers | first_write | diff | warnings |
|------|--------|--------|-------|----------|--------------|---------------|---------------------|----------|--------------|----------|-------------|------|----------|
| backend-todo-api-crud | passed | seeded_code_change | none | ok | none | agent-run | passed | passed | prompt=14304 tool_schema=2641 tools=12 workflow=strict closeout=full validation=passed | required_validation,first_code_change,verification_failed,acceptance_rejected,repeated_no_code_progress | 3 | yes | tool_errors_seen |

## Notes

- `plan_quality` describes plan-only/API artifacts when present.
- `tool_boundary` separates plan-only, collect-only, and real agent-run reports.
- `verification_status` combines closeout and required-command evidence; it is not a human-quality score.
- `real_code_change_passed` requires an agent-run report with a non-empty diff; plan-only success is tracked separately.
