# Live Eval Summary: core-quality-real-rerun-20260517-091952

- Run directory: `docs/benchmarks/live-core-quality-real-rerun-20260517-091952`
- Tasks found: `8`
- Pass rate: `8/8` (100.0%)
- Failure rate: `0/8` (0.0%)
- Real code-change passes: `3`
- Plan-only passes: `0`
- Seeded no-diff failures: `0`
- Memory active tasks: `8`
- Memory changed-plan tasks: `4`
- Memory recalled items: `0`
- Memory conflicts: `0`
- Skill active tasks: `0`
- Skill promotion-evidence tasks: `0`
- Status counts: passed=8
- Failure owners: none=8
- Eval intents: audit_or_regression_check=5, seeded_code_change=3

## Failure Modes

- `warning:audit_no_code_diff`: `5`
- `warning:no_code_diff`: `5`
- `earlier_stage_validation_failed_before_repair`: `1`
- `earlier_verification_failed_before_repair`: `1`

## Memory And Skill Evidence

| dimension | count | meaning |
|-----------|-------|---------|
| memory_active_tasks | 8 | Tasks where retrieval, sync, or memory tools were active. |
| memory_changed_plan_tasks | 4 | Tasks where memory or learning signals reweighted planning. |
| memory_recalled_items | 0 | Retrieved memory-backed context items across tasks. |
| memory_conflicts | 0 | Retrieval-context conflict count from memory-backed context. |
| skill_active_tasks | 0 | Tasks where skill tools or skill-specific signals were active. |
| skill_promotion_evidence_tasks | 0 | Tasks with promotion-related skill evidence. |

## Outcome Classes

| class | count | meaning |
|-------|-------|---------|
| real_code_change_passed | 3 | Agent-run tasks with passing status and a real diff. |
| plan_only_passed | 0 | Planning/API-only artifacts that passed their available checks. |
| seeded_no_diff_failed | 0 | Seeded code-change tasks where the agent did not produce a diff. |

## Task Matrix

| task | status | intent | owner | required | plan_quality | tool_boundary | verification_status | closeout | runtime_diet | triggers | first_write | diff | memory | skill | warnings |
|------|--------|--------|-------|----------|--------------|---------------|---------------------|----------|--------------|----------|-------------|------|--------|-------|----------|
| core-inspection-grounding | passed | audit_or_regression_check | none | ok | none | agent-run | passed | passed | prompt=2494 tool_schema=3186 tools=15 workflow=guarded closeout=full validation=passed:4/4 | required_validation | none | no | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff,audit_no_code_diff |
| core-long-output-artifact | passed | audit_or_regression_check | none | ok | none | agent-run | passed | passed | prompt=2073 tool_schema=3186 tools=15 workflow=guarded closeout=full validation=passed:4/4 | required_validation,first_code_change | none | no | active=true, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff,audit_no_code_diff |
| core-multi-file-edit | passed | seeded_code_change | none | ok | none | agent-run | passed | passed | prompt=2335 tool_schema=3186 tools=15 workflow=guarded closeout=full validation=passed:5/5 | required_validation,first_code_change | 3 | yes | active=true, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | none |
| core-permission-rejection-recovery | passed | seeded_code_change | none | ok | none | agent-run | passed | passed | prompt=4255 tool_schema=3186 tools=15 workflow=strict closeout=full validation=passed:6/6 recovered_failed:1 | required_validation,first_code_change,verification_failed,acceptance_rejected | 3 | yes | active=true, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | none |
| core-provider-roundtrip | passed | audit_or_regression_check | none | ok | none | agent-run | passed | passed | prompt=9469 tool_schema=3186 tools=15 workflow=guarded closeout=full validation=passed:1/1 | required_validation | none | no | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff,audit_no_code_diff |
| core-rollback-product-path | passed | audit_or_regression_check | none | ok | none | agent-run | passed | passed | prompt=21072 tool_schema=3186 tools=15 workflow=guarded closeout=full validation=passed:2/2 | required_validation | none | no | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff,audit_no_code_diff |
| core-simple-stale-edit | passed | seeded_code_change | none | ok | none | agent-run | passed | passed | prompt=2075 tool_schema=3186 tools=15 workflow=guarded closeout=full validation=passed:4/4 | required_validation,first_code_change | 2 | yes | active=true, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | none |
| core-terminal-install-run | passed | audit_or_regression_check | none | ok | none | agent-run | passed | passed | prompt=2401 tool_schema=3186 tools=15 workflow=guarded closeout=full validation=passed:2/2 | required_validation | none | no | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff,audit_no_code_diff |

## Notes

- `plan_quality` describes plan-only/API artifacts when present.
- `tool_boundary` separates plan-only, collect-only, and real agent-run reports.
- `verification_status` combines closeout and required-command evidence; it is not a human-quality score.
- `real_code_change_passed` requires an agent-run report with a non-empty diff; plan-only success is tracked separately.
- `memory` and `skill` summarize evidence signals; they do not by themselves mean the task succeeded.
