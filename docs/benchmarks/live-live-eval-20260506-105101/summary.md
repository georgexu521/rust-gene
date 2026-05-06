# Live Eval Summary: live-eval-20260506-105101

- Run directory: `docs/benchmarks/live-live-eval-20260506-105101`
- Tasks found: `1`
- Pass rate: `0/1` (0.0%)
- Failure rate: `1/1` (100.0%)
- Real code-change passes: `0`
- Plan-only passes: `0`
- Seeded no-diff failures: `0`
- Status counts: failed=1
- Failure owners: llm_reasoning=1
- Eval intents: seeded_code_change=1

## Failure Modes

- `acceptance_review_rejected`: `1`
- `closeout_not_successful`: `1`
- `earlier_stage_validation_failed_before_repair`: `1`
- `earlier_verification_failed_before_repair`: `1`
- `required_commands_not_passing`: `1`
- `stage_validation_failed`: `1`
- `verification_failed`: `1`

## Outcome Classes

| class | count | meaning |
|-------|-------|---------|
| real_code_change_passed | 0 | Agent-run tasks with passing status and a real diff. |
| plan_only_passed | 0 | Planning/API-only artifacts that passed their available checks. |
| seeded_no_diff_failed | 0 | Seeded code-change tasks where the agent did not produce a diff. |

## Task Matrix

| task | status | intent | owner | required | plan_quality | tool_boundary | verification_status | closeout | triggers | first_write | diff | warnings |
|------|--------|--------|-------|----------|--------------|---------------|---------------------|----------|----------|-------------|------|----------|
| code-change-verification-repair-loop | failed | seeded_code_change | llm_reasoning | failed | none | agent-run | failed | failed | required_validation,repeated_no_code_progress,first_code_change,verification_failed,acceptance_rejected | 7 | yes | none |

## Notes

- `plan_quality` describes plan-only/API artifacts when present.
- `tool_boundary` separates plan-only, collect-only, and real agent-run reports.
- `verification_status` combines closeout and required-command evidence; it is not a human-quality score.
- `real_code_change_passed` requires an agent-run report with a non-empty diff; plan-only success is tracked separately.
