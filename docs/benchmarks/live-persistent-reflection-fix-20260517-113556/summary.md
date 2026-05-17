# Live Eval Summary: persistent-reflection-fix-20260517-113556

- Run directory: `docs/benchmarks/live-persistent-reflection-fix-20260517-113556`
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
- Behavior assertion tasks: `1`
- Behavior assertions passed: `1`
- Status counts: passed=1
- Failure owners: none=1
- Eval intents: seeded_code_change=1

## Failure Modes

- `earlier_stage_validation_failed_before_repair`: `1`
- `earlier_verification_failed_before_repair`: `1`

## Memory And Skill Evidence

| dimension | count | meaning |
|-----------|-------|---------|
| memory_active_tasks | 1 | Tasks where retrieval, sync, or memory tools were active. |
| memory_changed_plan_tasks | 1 | Tasks where memory or learning signals reweighted planning. |
| memory_recalled_items | 0 | Retrieved memory-backed context items across tasks. |
| memory_conflicts | 0 | Retrieval-context conflict count from memory-backed context. |
| skill_active_tasks | 0 | Tasks where skill tools or skill-specific signals were active. |
| skill_promotion_evidence_tasks | 0 | Tasks with promotion-related skill evidence. |
| behavior_assertion_tasks | 1 | Tasks with explicit behavior assertions in the live-eval sample. |
| behavior_assertions_passed | 1 | Explicit behavior-assertion tasks whose required checks passed. |
| memory_behavior_assertion_tasks | 1 | Behavior assertions covering memory semantics rather than only memory activity signals. |
| skill_behavior_assertion_tasks | 0 | Behavior assertions covering skill semantics rather than only skill activity signals. |

## Outcome Classes

| class | count | meaning |
|-------|-------|---------|
| real_code_change_passed | 1 | Agent-run tasks with passing status and a real diff. |
| plan_only_passed | 0 | Planning/API-only artifacts that passed their available checks. |
| seeded_no_diff_failed | 0 | Seeded code-change tasks where the agent did not produce a diff. |

## Task Matrix

| task | status | intent | owner | required | plan_quality | tool_boundary | verification_status | closeout | runtime_diet | behavior_assertions | behavior_status | triggers | first_write | diff | memory | skill | warnings |
|------|--------|--------|-------|----------|--------------|---------------|---------------------|----------|--------------|---------------------|-----------------|----------|-------------|------|--------|-------|----------|
| persistent-memory-planning-context | passed | seeded_code_change | none | ok | none | agent-run | passed | passed | prompt=26814 tool_schema=3186 tools=15 workflow=strict closeout=full validation=failed:1/10 | memory_planning_context,memory_retrieval_before_workflow_judgment | passed | required_validation,repeated_no_code_progress,first_code_change,verification_failed,acceptance_rejected | 7 | yes | active=true, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | none |

## Notes

- `plan_quality` describes plan-only/API artifacts when present.
- `tool_boundary` separates plan-only, collect-only, and real agent-run reports.
- `verification_status` combines closeout and required-command evidence; it is not a human-quality score.
- `real_code_change_passed` requires an agent-run report with a non-empty diff; plan-only success is tracked separately.
- `memory` and `skill` summarize evidence signals; they do not by themselves mean the task succeeded.
- `behavior_assertions` are explicit sample-level checks; memory/skill behavior assertions are stronger evidence than activity signals alone.
