# Live Eval Summary: dashboard-patch-retry-20260509-200245

- Run directory: `docs/benchmarks/live-dashboard-patch-retry-20260509-200245`
- Tasks found: `1`
- Pass rate: `1/1` (100.0%)
- Failure rate: `0/1` (0.0%)
- Real code-change passes: `1`
- Plan-only passes: `0`
- Seeded no-diff failures: `0`
- Memory active tasks: `1`
- Memory changed-plan tasks: `1`
- Memory recalled items: `0`
- Memory conflicts: `0`
- Skill active tasks: `0`
- Skill promotion-evidence tasks: `0`
- Status counts: passed=1
- Failure owners: none=1
- Eval intents: seeded_code_change=1

## Failure Modes

- `earlier_stage_validation_failed_before_repair`: `1`
- `earlier_verification_failed_before_repair`: `1`
- `warning:tool_errors_seen`: `1`

## Memory And Skill Evidence

| dimension | count | meaning |
|-----------|-------|---------|
| memory_active_tasks | 1 | Tasks where retrieval, sync, or memory tools were active. |
| memory_changed_plan_tasks | 1 | Tasks where memory or learning signals reweighted planning. |
| memory_recalled_items | 0 | Retrieved memory-backed context items across tasks. |
| memory_conflicts | 0 | Retrieval-context conflict count from memory-backed context. |
| skill_active_tasks | 0 | Tasks where skill tools or skill-specific signals were active. |
| skill_promotion_evidence_tasks | 0 | Tasks with promotion-related skill evidence. |

## Outcome Classes

| class | count | meaning |
|-------|-------|---------|
| real_code_change_passed | 1 | Agent-run tasks with passing status and a real diff. |
| plan_only_passed | 0 | Planning/API-only artifacts that passed their available checks. |
| seeded_no_diff_failed | 0 | Seeded code-change tasks where the agent did not produce a diff. |

## Task Matrix

| task | status | intent | owner | required | plan_quality | tool_boundary | verification_status | closeout | runtime_diet | triggers | first_write | diff | memory | skill | warnings |
|------|--------|--------|-------|----------|--------------|---------------|---------------------|----------|--------------|----------|-------------|------|--------|-------|----------|
| live-eval-dashboard-summary | passed | seeded_code_change | none | ok | none | agent-run | passed | passed | prompt=17416 tool_schema=2641 tools=12 workflow=strict closeout=full validation=passed | required_validation,repeated_no_code_progress,first_code_change,verification_failed,acceptance_rejected | 5 | yes | active=true, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | tool_errors_seen |

## Notes

- `plan_quality` describes plan-only/API artifacts when present.
- `tool_boundary` separates plan-only, collect-only, and real agent-run reports.
- `verification_status` combines closeout and required-command evidence; it is not a human-quality score.
- `real_code_change_passed` requires an agent-run report with a non-empty diff; plan-only success is tracked separately.
- `memory` and `skill` summarize evidence signals; they do not by themselves mean the task succeeded.
