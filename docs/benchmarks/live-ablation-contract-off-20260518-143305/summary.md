# Live Eval Summary: ablation-contract-off-20260518-143305

- Run directory: `docs/benchmarks/live-ablation-contract-off-20260518-143305`
- Tasks found: `4`
- Pass rate: `3/4` (75.0%)
- Failure rate: `1/4` (25.0%)
- Skipped/unscored tasks: `0`
- Real code-change passes: `3`
- Plan-only passes: `0`
- Seeded no-diff failures: `0`
- Memory active tasks: `4`
- Memory changed-plan tasks: `0`
- Memory recalled items: `0`
- Memory conflicts: `0`
- Skill active tasks: `0`
- Skill promotion-evidence tasks: `0`
- Behavior assertion tasks: `0`
- Behavior assertions passed: `0`
- Coding gauntlet agent-run tasks: `4`
- Coding gauntlet passes: `3`
- Coding gauntlet failures: `1`
- Coding gauntlet likely clean passes: `3`
- Coding gauntlet repaired passes: `0`
- Coding gauntlet required-validation passes: `3/4`
- Coding gauntlet first-write observed: `4/4`
- Coding gauntlet repair signals: `1`
- Coding gauntlet changed files: `4`
- Status counts: failed=1, passed=3
- Failure owners: llm_reasoning=1, none=3
- Eval intents: seeded_code_change=4

## Failure Modes

- `earlier_stage_validation_failed_before_repair`: `2`
- `earlier_verification_failed_before_repair`: `2`
- `closeout_not_successful`: `1`
- `required_commands_not_passing`: `1`
- `stage_validation_failed`: `1`
- `verification_failed`: `1`
- `warning:tool_errors_seen`: `1`

## Memory And Skill Evidence

| dimension | count | meaning |
|-----------|-------|---------|
| memory_active_tasks | 4 | Tasks where retrieval, sync, or memory tools were active. |
| memory_changed_plan_tasks | 0 | Tasks where memory or learning signals reweighted planning. |
| memory_recalled_items | 0 | Retrieved memory-backed context items across tasks. |
| memory_conflicts | 0 | Retrieval-context conflict count from memory-backed context. |
| skill_active_tasks | 0 | Tasks where skill tools or skill-specific signals were active. |
| skill_promotion_evidence_tasks | 0 | Tasks with promotion-related skill evidence. |
| behavior_assertion_tasks | 0 | Tasks with explicit behavior assertions in the live-eval sample. |
| behavior_assertions_passed | 0 | Explicit behavior-assertion tasks whose required checks passed. |
| memory_behavior_assertion_tasks | 0 | Behavior assertions covering memory semantics rather than only memory activity signals. |
| skill_behavior_assertion_tasks | 0 | Behavior assertions covering skill semantics rather than only skill activity signals. |

## Outcome Classes

| class | count | meaning |
|-------|-------|---------|
| real_code_change_passed | 3 | Agent-run tasks with passing status and a real diff. |
| plan_only_passed | 0 | Planning/API-only artifacts that passed their available checks. |
| seeded_no_diff_failed | 0 | Seeded code-change tasks where the agent did not produce a diff. |

## Coding Gauntlet Evidence

| task | gauntlet_status | first_pass_signal | coding | required | closeout | contract | first_write | diff | warnings |
|------|-----------------|-------------------|--------|----------|----------|----------|-------------|------|----------|
| backend-todo-api-crud | passed | likely_clean | tools=10, tool_records=10, validations=16, repair=0, files=1 | ok | passed | missing | 3 | yes | none |
| code-change-verification-repair-loop | failed | failed | tools=14, tool_records=13, validations=14, repair=1, files=1 | failed | failed | missing | 7 | yes | tool_errors_seen |
| core-permission-rejection-recovery | passed | likely_clean | tools=2, tool_records=2, validations=2, repair=0, files=1 | ok | passed | missing | 2 | yes | none |
| frontend-book-notes-localstorage | passed | likely_clean | tools=6, tool_records=6, validations=2, repair=0, files=1 | ok | passed | missing | 6 | yes | none |

## Task Matrix

| task | status | intent | owner | required | plan_quality | tool_boundary | verification_status | closeout | runtime_diet | contract | behavior_assertions | behavior_status | triggers | first_write | diff | memory | skill | warnings |
|------|--------|--------|-------|----------|--------------|---------------|---------------------|----------|--------------|----------|---------------------|-----------------|----------|-------------|------|--------|-------|----------|
| backend-todo-api-crud | passed | seeded_code_change | none | ok | none | agent-run | passed | passed | prompt=24726 tool_schema=3186 tools=15 workflow=strict closeout=full validation=passed:2/2 recovered_failed:2 | missing | none | none | required_validation,first_code_change,verification_failed | 3 | yes | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | none |
| code-change-verification-repair-loop | failed | seeded_code_change | llm_reasoning | failed | none | agent-run | failed | failed | prompt=26049 tool_schema=3186 tools=15 workflow=strict closeout=full validation=failed:1/5 | missing | none | none | required_validation,repeated_no_code_progress,first_code_change,verification_failed | 7 | yes | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | tool_errors_seen |
| core-permission-rejection-recovery | passed | seeded_code_change | none | ok | none | agent-run | passed | passed | prompt=1683 tool_schema=3186 tools=15 workflow=strict closeout=full validation=passed:3/3 | missing | none | none | required_validation,first_code_change | 2 | yes | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | none |
| frontend-book-notes-localstorage | passed | seeded_code_change | none | ok | none | agent-run | passed | passed | prompt=4705 tool_schema=3186 tools=15 workflow=guarded closeout=full validation=passed:2/2 | missing | none | none | required_validation,first_code_change | 6 | yes | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | none |

## Notes

- `plan_quality` describes plan-only/API artifacts when present.
- `tool_boundary` separates plan-only, collect-only, and real agent-run reports.
- `verification_status` combines closeout and required-command evidence; it is not a human-quality score.
- `skipped` reports are excluded from pass/fail rate denominators; collect-only reports need passing required commands to be scored.
- `real_code_change_passed` requires an agent-run report with a non-empty diff; plan-only success is tracked separately.
- `memory` and `skill` summarize evidence signals; they do not by themselves mean the task succeeded.
- `behavior_assertions` are explicit sample-level checks; memory/skill behavior assertions are stronger evidence than activity signals alone.
