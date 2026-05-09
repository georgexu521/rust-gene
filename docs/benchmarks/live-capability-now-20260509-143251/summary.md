# Live Eval Summary: capability-now-20260509-143251

- Run directory: `docs/benchmarks/live-capability-now-20260509-143251`
- Tasks found: `1`
- Pass rate: `0/1` (0.0%)
- Failure rate: `1/1` (100.0%)
- Real code-change passes: `0`
- Plan-only passes: `0`
- Seeded no-diff failures: `1`
- Status counts: failed=1
- Failure owners: agent_flow=1
- Eval intents: seeded_code_change=1

## Failure Modes

- `action_checkpoint_invalid_tools`: `1`
- `closeout_not_successful`: `1`
- `expected_code_diff_missing`: `1`
- `required_commands_not_passing`: `1`
- `warning:action_checkpoint_invalid_tools`: `1`
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
| live-eval-dashboard-summary | failed | seeded_code_change | agent_flow | failed | none | agent-run | failed | not_verified | prompt=5334 tool_schema=2641 tools=12 workflow=guarded closeout=full validation=not_verified | required_validation,repeated_no_code_progress | none | no | no_code_diff,action_checkpoint_invalid_tools |

## Notes

- `plan_quality` describes plan-only/API artifacts when present.
- `tool_boundary` separates plan-only, collect-only, and real agent-run reports.
- `verification_status` combines closeout and required-command evidence; it is not a human-quality score.
- `real_code_change_passed` requires an agent-run report with a non-empty diff; plan-only success is tracked separately.
