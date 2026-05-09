# Live Eval Summary: capability-evidence-20260509-173239

- Run directory: `docs/benchmarks/live-capability-evidence-20260509-173239`
- Tasks found: `6`
- Pass rate: `6/6` (100.0%)
- Failure rate: `0/6` (0.0%)
- Real code-change passes: `6`
- Plan-only passes: `0`
- Seeded no-diff failures: `0`
- Memory active tasks: `6`
- Memory changed-plan tasks: `5`
- Memory recalled items: `0`
- Memory conflicts: `0`
- Skill active tasks: `1`
- Skill promotion-evidence tasks: `1`
- Status counts: passed=6
- Failure owners: none=6
- Eval intents: seeded_code_change=6

## Failure Modes

- `acceptance_review_rejected`: `1`
- `closeout_not_successful`: `1`
- `earlier_stage_validation_failed_before_repair`: `1`
- `earlier_verification_failed_before_repair`: `1`
- `recovered_acceptance_review_rejected`: `1`
- `recovered_action_checkpoint_invalid_tools`: `1`
- `recovered_closeout_not_successful`: `1`
- `recovered_stage_validation_failed`: `1`
- `recovered_verification_failed`: `1`
- `stage_validation_failed`: `1`
- `verification_failed`: `1`
- `warning:action_checkpoint_invalid_tools`: `1`

## Memory And Skill Evidence

| dimension | count | meaning |
|-----------|-------|---------|
| memory_active_tasks | 6 | Tasks where retrieval, sync, or memory tools were active. |
| memory_changed_plan_tasks | 5 | Tasks where memory or learning signals reweighted planning. |
| memory_recalled_items | 0 | Retrieved memory-backed context items across tasks. |
| memory_conflicts | 0 | Retrieval-context conflict count from memory-backed context. |
| skill_active_tasks | 1 | Tasks where skill tools or skill-specific signals were active. |
| skill_promotion_evidence_tasks | 1 | Tasks with promotion-related skill evidence. |

## Outcome Classes

| class | count | meaning |
|-------|-------|---------|
| real_code_change_passed | 6 | Agent-run tasks with passing status and a real diff. |
| plan_only_passed | 0 | Planning/API-only artifacts that passed their available checks. |
| seeded_no_diff_failed | 0 | Seeded code-change tasks where the agent did not produce a diff. |

## Task Matrix

| task | status | intent | owner | required | plan_quality | tool_boundary | verification_status | closeout | runtime_diet | triggers | first_write | diff | memory | skill | warnings |
|------|--------|--------|-------|----------|--------------|---------------|---------------------|----------|--------------|----------|-------------|------|--------|-------|----------|
| backend-todo-api-crud | passed | seeded_code_change | none | ok | none | agent-run | passed | passed | prompt=8092 tool_schema=2641 tools=12 workflow=strict closeout=full validation=passed | required_validation,repeated_no_code_progress,first_code_change,verification_failed,acceptance_rejected | 5 | yes | active=true, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | none |
| code-change-verification-repair-loop | passed | seeded_code_change | none | ok | none | agent-run | passed | passed | prompt=9903 tool_schema=2641 tools=12 workflow=strict closeout=full validation=passed | required_validation,repeated_no_code_progress,first_code_change | 6 | yes | active=true, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | none |
| frontend-book-notes-localstorage | passed | seeded_code_change | none | ok | none | agent-run | passed | passed | prompt=4821 tool_schema=2641 tools=12 workflow=guarded closeout=full validation=passed | required_validation,first_code_change | 5 | yes | active=true, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | none |
| live-eval-dashboard-summary | passed | seeded_code_change | none | ok | none | agent-run | unknown | failed | prompt=7686 tool_schema=2641 tools=12 workflow=strict closeout=full validation=failed | required_validation,repeated_no_code_progress,first_code_change,verification_failed,acceptance_rejected | 4 | yes | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | action_checkpoint_invalid_tools |
| memory-save-quality-gate | passed | seeded_code_change | none | ok | none | agent-run | passed | passed | prompt=9647 tool_schema=2641 tools=12 workflow=strict closeout=full validation=passed | required_validation,repeated_no_code_progress,first_code_change | 7 | yes | active=true, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | none |
| skill-promotion-gate | passed | seeded_code_change | none | ok | none | agent-run | passed | passed | prompt=5318 tool_schema=2641 tools=12 workflow=guarded closeout=full validation=passed | required_validation,repeated_no_code_progress,first_code_change | 8 | yes | active=true, recalled=0, conflicts=0, changed_plan=true | active=true, tool_calls=0, usage_events=0, promotion=true | none |

## Notes

- `plan_quality` describes plan-only/API artifacts when present.
- `tool_boundary` separates plan-only, collect-only, and real agent-run reports.
- `verification_status` combines closeout and required-command evidence; it is not a human-quality score.
- `real_code_change_passed` requires an agent-run report with a non-empty diff; plan-only success is tracked separately.
- `memory` and `skill` summarize evidence signals; they do not by themselves mean the task succeeded.
