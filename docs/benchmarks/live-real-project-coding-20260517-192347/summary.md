# Live Eval Summary: real-project-coding-20260517-192347

- Run directory: `docs/benchmarks/live-real-project-coding-20260517-192347`
- Tasks found: `15`
- Pass rate: `15/15` (100.0%)
- Failure rate: `0/15` (0.0%)
- Skipped/unscored tasks: `0`
- Real code-change passes: `10`
- Plan-only passes: `0`
- Seeded no-diff failures: `0`
- Memory active tasks: `15`
- Memory changed-plan tasks: `10`
- Memory recalled items: `0`
- Memory conflicts: `0`
- Skill active tasks: `1`
- Skill promotion-evidence tasks: `1`
- Behavior assertion tasks: `3`
- Behavior assertions passed: `3`
- Coding gauntlet agent-run tasks: `15`
- Coding gauntlet passes: `15`
- Coding gauntlet failures: `0`
- Coding gauntlet likely clean passes: `7`
- Coding gauntlet repaired passes: `4`
- Coding gauntlet required-validation passes: `15/15`
- Coding gauntlet first-write observed: `10/15`
- Coding gauntlet repair signals: `8`
- Coding gauntlet changed files: `13`
- Status counts: passed=15
- Failure owners: none=15
- Eval intents: audit_or_regression_check=5, seeded_code_change=10

## Failure Modes

- `warning:audit_no_code_diff`: `5`
- `warning:no_code_diff`: `5`
- `warning:tool_errors_seen`: `3`
- `earlier_stage_validation_failed_before_repair`: `2`
- `earlier_verification_failed_before_repair`: `2`

## Memory And Skill Evidence

| dimension | count | meaning |
|-----------|-------|---------|
| memory_active_tasks | 15 | Tasks where retrieval, sync, or memory tools were active. |
| memory_changed_plan_tasks | 10 | Tasks where memory or learning signals reweighted planning. |
| memory_recalled_items | 0 | Retrieved memory-backed context items across tasks. |
| memory_conflicts | 0 | Retrieval-context conflict count from memory-backed context. |
| skill_active_tasks | 1 | Tasks where skill tools or skill-specific signals were active. |
| skill_promotion_evidence_tasks | 1 | Tasks with promotion-related skill evidence. |
| behavior_assertion_tasks | 3 | Tasks with explicit behavior assertions in the live-eval sample. |
| behavior_assertions_passed | 3 | Explicit behavior-assertion tasks whose required checks passed. |
| memory_behavior_assertion_tasks | 2 | Behavior assertions covering memory semantics rather than only memory activity signals. |
| skill_behavior_assertion_tasks | 1 | Behavior assertions covering skill semantics rather than only skill activity signals. |

## Outcome Classes

| class | count | meaning |
|-------|-------|---------|
| real_code_change_passed | 10 | Agent-run tasks with passing status and a real diff. |
| plan_only_passed | 0 | Planning/API-only artifacts that passed their available checks. |
| seeded_no_diff_failed | 0 | Seeded code-change tasks where the agent did not produce a diff. |

## Coding Gauntlet Evidence

| task | gauntlet_status | first_pass_signal | coding | required | closeout | first_write | diff | warnings |
|------|-----------------|-------------------|--------|----------|----------|-------------|------|----------|
| backend-todo-api-crud | passed | likely_clean | tools=12, validations=20, repair=0, files=1 | ok | passed | 3 | yes | none |
| code-change-verification-repair-loop | passed | likely_clean | tools=7, validations=2, repair=0, files=1 | ok | passed | 7 | yes | none |
| core-inspection-grounding | passed | no_write | tools=5, validations=0, repair=0, files=0 | ok | passed | none | no | no_code_diff,audit_no_code_diff |
| core-long-output-artifact | passed | no_write | tools=1, validations=2, repair=0, files=0 | ok | passed | none | no | no_code_diff,audit_no_code_diff |
| core-multi-file-edit | passed | likely_clean | tools=3, validations=2, repair=0, files=2 | ok | passed | 3 | yes | none |
| core-permission-rejection-recovery | passed | likely_clean | tools=3, validations=2, repair=0, files=1 | ok | passed | 3 | yes | none |
| core-provider-roundtrip | passed | no_write | tools=9, validations=0, repair=0, files=0 | ok | passed | none | no | no_code_diff,audit_no_code_diff |
| core-rollback-product-path | passed | no_write | tools=9, validations=0, repair=0, files=0 | ok | passed | none | no | no_code_diff,audit_no_code_diff |
| core-simple-stale-edit | passed | likely_clean | tools=2, validations=2, repair=0, files=1 | ok | passed | 2 | yes | none |
| core-terminal-install-run | passed | repaired | tools=12, validations=0, repair=2, files=0 | ok | passed | none | no | no_code_diff,audit_no_code_diff,tool_errors_seen |
| frontend-book-notes-localstorage | passed | likely_clean | tools=6, validations=6, repair=0, files=1 | ok | passed | 4 | yes | none |
| live-eval-dashboard-summary | passed | repaired | tools=5, validations=2, repair=2, files=1 | ok | passed | 5 | yes | tool_errors_seen |
| memory-save-quality-gate | passed | likely_clean | tools=12, validations=2, repair=0, files=3 | ok | passed | 9 | yes | none |
| persistent-memory-planning-context | passed | repaired | tools=9, validations=2, repair=2, files=1 | ok | passed | 9 | yes | tool_errors_seen |
| skill-promotion-gate | passed | repaired | tools=11, validations=2, repair=2, files=1 | ok | passed | 10 | yes | none |

## Task Matrix

| task | status | intent | owner | required | plan_quality | tool_boundary | verification_status | closeout | runtime_diet | behavior_assertions | behavior_status | triggers | first_write | diff | memory | skill | warnings |
|------|--------|--------|-------|----------|--------------|---------------|---------------------|----------|--------------|---------------------|-----------------|----------|-------------|------|--------|-------|----------|
| backend-todo-api-crud | passed | seeded_code_change | none | ok | none | agent-run | passed | passed | prompt=28952 tool_schema=3186 tools=15 workflow=strict closeout=full validation=passed:2/2 recovered_failed:2 | none | none | required_validation,first_code_change,verification_failed | 3 | yes | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | none |
| code-change-verification-repair-loop | passed | seeded_code_change | none | ok | none | agent-run | passed | passed | prompt=14702 tool_schema=3186 tools=15 workflow=strict closeout=full validation=passed:5/5 | none | none | required_validation,repeated_no_code_progress,first_code_change | 7 | yes | active=true, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | none |
| core-inspection-grounding | passed | audit_or_regression_check | none | ok | none | agent-run | passed | passed | prompt=2457 tool_schema=3186 tools=15 workflow=guarded closeout=full validation=passed:4/4 | none | none | required_validation | none | no | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff,audit_no_code_diff |
| core-long-output-artifact | passed | audit_or_regression_check | none | ok | none | agent-run | passed | passed | prompt=2101 tool_schema=3186 tools=15 workflow=guarded closeout=full validation=passed:3/3 | none | none | required_validation,first_code_change | none | no | active=true, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff,audit_no_code_diff |
| core-multi-file-edit | passed | seeded_code_change | none | ok | none | agent-run | passed | passed | prompt=2325 tool_schema=3186 tools=15 workflow=guarded closeout=full validation=passed:2/2 | none | none | required_validation,first_code_change | 3 | yes | active=true, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | none |
| core-permission-rejection-recovery | passed | seeded_code_change | none | ok | none | agent-run | passed | passed | prompt=2023 tool_schema=3186 tools=15 workflow=strict closeout=full validation=passed:3/3 | none | none | required_validation,first_code_change | 3 | yes | active=true, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | none |
| core-provider-roundtrip | passed | audit_or_regression_check | none | ok | none | agent-run | passed | passed | prompt=12330 tool_schema=3186 tools=15 workflow=guarded closeout=full validation=passed:2/2 | none | none | required_validation | none | no | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff,audit_no_code_diff |
| core-rollback-product-path | passed | audit_or_regression_check | none | ok | none | agent-run | passed | passed | prompt=11931 tool_schema=3186 tools=15 workflow=guarded closeout=full validation=passed:2/2 | none | none | required_validation | none | no | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff,audit_no_code_diff |
| core-simple-stale-edit | passed | seeded_code_change | none | ok | none | agent-run | passed | passed | prompt=2104 tool_schema=3186 tools=15 workflow=guarded closeout=full validation=passed:1/1 | none | none | required_validation,first_code_change | 2 | yes | active=true, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | none |
| core-terminal-install-run | passed | audit_or_regression_check | none | ok | none | agent-run | passed | passed | prompt=3825 tool_schema=3186 tools=15 workflow=guarded closeout=full validation=passed:1/1 | none | none | required_validation | none | no | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff,audit_no_code_diff,tool_errors_seen |
| frontend-book-notes-localstorage | passed | seeded_code_change | none | ok | none | agent-run | passed | passed | prompt=12188 tool_schema=3186 tools=15 workflow=strict closeout=full validation=passed:2/2 recovered_failed:2 | none | none | required_validation,first_code_change,verification_failed,acceptance_rejected | 4 | yes | active=true, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | none |
| live-eval-dashboard-summary | passed | seeded_code_change | none | ok | none | agent-run | passed | passed | prompt=6050 tool_schema=3186 tools=15 workflow=guarded closeout=full validation=passed:5/5 | none | none | required_validation,repeated_no_code_progress,first_code_change | 5 | yes | active=true, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | tool_errors_seen |
| memory-save-quality-gate | passed | seeded_code_change | none | ok | none | agent-run | passed | passed | prompt=13147 tool_schema=3186 tools=15 workflow=strict closeout=full validation=passed:4/4 | memory_quality_gate,memory_save_outcome_visibility | passed | required_validation,repeated_no_code_progress,first_code_change | 9 | yes | active=true, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | none |
| persistent-memory-planning-context | passed | seeded_code_change | none | ok | none | agent-run | passed | passed | prompt=12480 tool_schema=3186 tools=15 workflow=strict closeout=full validation=passed:6/6 | memory_planning_context,memory_retrieval_before_workflow_judgment | passed | required_validation,first_code_change | 9 | yes | active=true, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | tool_errors_seen |
| skill-promotion-gate | passed | seeded_code_change | none | ok | none | agent-run | passed | passed | prompt=22502 tool_schema=3186 tools=15 workflow=guarded closeout=full validation=passed:5/5 | skill_promotion_gate,skill_evolution_cooldown | passed | required_validation,repeated_no_code_progress,first_code_change | 10 | yes | active=true, recalled=0, conflicts=0, changed_plan=true | active=true, tool_calls=0, usage_events=0, promotion=true | none |

## Notes

- `plan_quality` describes plan-only/API artifacts when present.
- `tool_boundary` separates plan-only, collect-only, and real agent-run reports.
- `verification_status` combines closeout and required-command evidence; it is not a human-quality score.
- `skipped` reports are excluded from pass/fail rate denominators; collect-only reports need passing required commands to be scored.
- `real_code_change_passed` requires an agent-run report with a non-empty diff; plan-only success is tracked separately.
- `memory` and `skill` summarize evidence signals; they do not by themselves mean the task succeeded.
- `behavior_assertions` are explicit sample-level checks; memory/skill behavior assertions are stronger evidence than activity signals alone.
