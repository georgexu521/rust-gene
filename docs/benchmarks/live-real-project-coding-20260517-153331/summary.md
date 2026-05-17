# Live Eval Summary: real-project-coding-20260517-153331

- Run directory: `docs/benchmarks/live-real-project-coding-20260517-153331`
- Tasks found: `15`
- Pass rate: `13/15` (86.7%)
- Failure rate: `2/15` (13.3%)
- Skipped/unscored tasks: `0`
- Real code-change passes: `8`
- Plan-only passes: `0`
- Seeded no-diff failures: `0`
- Memory active tasks: `15`
- Memory changed-plan tasks: `9`
- Memory recalled items: `0`
- Memory conflicts: `0`
- Skill active tasks: `1`
- Skill promotion-evidence tasks: `1`
- Behavior assertion tasks: `3`
- Behavior assertions passed: `1`
- Coding gauntlet agent-run tasks: `15`
- Coding gauntlet passes: `13`
- Coding gauntlet failures: `2`
- Coding gauntlet likely clean passes: `7`
- Coding gauntlet repaired passes: `2`
- Coding gauntlet required-validation passes: `13/15`
- Coding gauntlet first-write observed: `10/15`
- Coding gauntlet repair signals: `22`
- Coding gauntlet changed files: `13`
- Status counts: failed=2, passed=13
- Failure owners: agent_flow=1, llm_reasoning=1, none=13
- Eval intents: audit_or_regression_check=5, seeded_code_change=10

## Failure Modes

- `warning:audit_no_code_diff`: `5`
- `warning:no_code_diff`: `5`
- `earlier_stage_validation_failed_before_repair`: `4`
- `earlier_verification_failed_before_repair`: `4`
- `acceptance_review_rejected`: `2`
- `behavior_assertions_not_passing`: `2`
- `closeout_not_successful`: `2`
- `required_commands_not_passing`: `2`
- `stage_validation_failed`: `2`
- `verification_failed`: `2`
- `warning:tool_errors_seen`: `2`
- `action_checkpoint_invalid_tools`: `1`
- `unused variable: `destructive_scope``: `1`
- `unused variable: `resource_policy``: `1`
- `unused variable: `route``: `1`
- `unused variable: `working_dir``: `1`
- `warning:action_checkpoint_invalid_tools`: `1`

## Memory And Skill Evidence

| dimension | count | meaning |
|-----------|-------|---------|
| memory_active_tasks | 15 | Tasks where retrieval, sync, or memory tools were active. |
| memory_changed_plan_tasks | 9 | Tasks where memory or learning signals reweighted planning. |
| memory_recalled_items | 0 | Retrieved memory-backed context items across tasks. |
| memory_conflicts | 0 | Retrieval-context conflict count from memory-backed context. |
| skill_active_tasks | 1 | Tasks where skill tools or skill-specific signals were active. |
| skill_promotion_evidence_tasks | 1 | Tasks with promotion-related skill evidence. |
| behavior_assertion_tasks | 3 | Tasks with explicit behavior assertions in the live-eval sample. |
| behavior_assertions_passed | 1 | Explicit behavior-assertion tasks whose required checks passed. |
| memory_behavior_assertion_tasks | 2 | Behavior assertions covering memory semantics rather than only memory activity signals. |
| skill_behavior_assertion_tasks | 1 | Behavior assertions covering skill semantics rather than only skill activity signals. |

## Outcome Classes

| class | count | meaning |
|-------|-------|---------|
| real_code_change_passed | 8 | Agent-run tasks with passing status and a real diff. |
| plan_only_passed | 0 | Planning/API-only artifacts that passed their available checks. |
| seeded_no_diff_failed | 0 | Seeded code-change tasks where the agent did not produce a diff. |

## Coding Gauntlet Evidence

| task | gauntlet_status | first_pass_signal | coding | required | closeout | first_write | diff | warnings |
|------|-----------------|-------------------|--------|----------|----------|-------------|------|----------|
| backend-todo-api-crud | passed | likely_clean | tools=4, validations=4, repair=0, files=1 | ok | passed | 3 | yes | none |
| code-change-verification-repair-loop | passed | repaired | tools=6, validations=4, repair=5, files=1 | ok | passed | 5 | yes | none |
| core-inspection-grounding | passed | no_write | tools=5, validations=0, repair=0, files=0 | ok | passed | none | no | no_code_diff,audit_no_code_diff |
| core-long-output-artifact | passed | no_write | tools=2, validations=2, repair=0, files=0 | ok | passed | none | no | no_code_diff,audit_no_code_diff |
| core-multi-file-edit | passed | likely_clean | tools=3, validations=2, repair=0, files=2 | ok | passed | 3 | yes | none |
| core-permission-rejection-recovery | passed | likely_clean | tools=3, validations=2, repair=0, files=1 | ok | passed | 3 | yes | none |
| core-provider-roundtrip | passed | repaired | tools=13, validations=0, repair=2, files=0 | ok | passed | none | no | no_code_diff,audit_no_code_diff,tool_errors_seen |
| core-rollback-product-path | passed | no_write | tools=6, validations=0, repair=0, files=0 | ok | passed | none | no | no_code_diff,audit_no_code_diff |
| core-simple-stale-edit | passed | likely_clean | tools=2, validations=2, repair=0, files=1 | ok | passed | 2 | yes | none |
| core-terminal-install-run | passed | no_write | tools=6, validations=0, repair=0, files=0 | ok | passed | none | no | no_code_diff,audit_no_code_diff |
| frontend-book-notes-localstorage | passed | likely_clean | tools=6, validations=2, repair=0, files=1 | ok | passed | 6 | yes | none |
| live-eval-dashboard-summary | passed | likely_clean | tools=6, validations=2, repair=0, files=1 | ok | passed | 6 | yes | none |
| memory-save-quality-gate | failed | failed | tools=17, validations=8, repair=7, files=3 | failed | failed | 11 | yes | tool_errors_seen |
| persistent-memory-planning-context | failed | failed | tools=14, validations=6, repair=8, files=1 | failed | failed | 12 | yes | action_checkpoint_invalid_tools |
| skill-promotion-gate | passed | likely_clean | tools=12, validations=2, repair=0, files=1 | ok | passed | 11 | yes | none |

## Task Matrix

| task | status | intent | owner | required | plan_quality | tool_boundary | verification_status | closeout | runtime_diet | behavior_assertions | behavior_status | triggers | first_write | diff | memory | skill | warnings |
|------|--------|--------|-------|----------|--------------|---------------|---------------------|----------|--------------|---------------------|-----------------|----------|-------------|------|--------|-------|----------|
| backend-todo-api-crud | passed | seeded_code_change | none | ok | none | agent-run | passed | passed | prompt=8457 tool_schema=3186 tools=15 workflow=strict closeout=full validation=passed:5/5 recovered_failed:2 | none | none | required_validation,first_code_change,verification_failed,acceptance_rejected | 3 | yes | active=true, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | none |
| code-change-verification-repair-loop | passed | seeded_code_change | none | ok | none | agent-run | passed | passed | prompt=16541 tool_schema=3186 tools=15 workflow=strict closeout=full validation=failed:1/9 | none | none | required_validation,repeated_no_code_progress,first_code_change,verification_failed,acceptance_rejected | 5 | yes | active=true, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | none |
| core-inspection-grounding | passed | audit_or_regression_check | none | ok | none | agent-run | passed | passed | prompt=2543 tool_schema=3186 tools=15 workflow=guarded closeout=full validation=passed:4/4 | none | none | required_validation | none | no | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff,audit_no_code_diff |
| core-long-output-artifact | passed | audit_or_regression_check | none | ok | none | agent-run | passed | passed | prompt=2098 tool_schema=3186 tools=15 workflow=guarded closeout=full validation=passed:4/4 | none | none | required_validation,first_code_change | none | no | active=true, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff,audit_no_code_diff |
| core-multi-file-edit | passed | seeded_code_change | none | ok | none | agent-run | passed | passed | prompt=2333 tool_schema=3186 tools=15 workflow=guarded closeout=full validation=passed:5/5 | none | none | required_validation,first_code_change | 3 | yes | active=true, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | none |
| core-permission-rejection-recovery | passed | seeded_code_change | none | ok | none | agent-run | passed | passed | prompt=2083 tool_schema=3186 tools=15 workflow=strict closeout=full validation=passed:6/6 | none | none | required_validation,first_code_change | 3 | yes | active=true, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | none |
| core-provider-roundtrip | passed | audit_or_regression_check | none | ok | none | agent-run | passed | passed | prompt=9858 tool_schema=3186 tools=15 workflow=guarded closeout=full validation=passed:2/2 | none | none | required_validation | none | no | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff,audit_no_code_diff,tool_errors_seen |
| core-rollback-product-path | passed | audit_or_regression_check | none | ok | none | agent-run | passed | passed | prompt=10194 tool_schema=3186 tools=15 workflow=guarded closeout=full validation=passed:2/2 | none | none | required_validation | none | no | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff,audit_no_code_diff |
| core-simple-stale-edit | passed | seeded_code_change | none | ok | none | agent-run | passed | passed | prompt=2060 tool_schema=3186 tools=15 workflow=guarded closeout=full validation=passed:4/4 | none | none | required_validation,first_code_change | 2 | yes | active=true, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | none |
| core-terminal-install-run | passed | audit_or_regression_check | none | ok | none | agent-run | passed | passed | prompt=2793 tool_schema=3186 tools=15 workflow=guarded closeout=full validation=passed:1/1 | none | none | required_validation | none | no | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff,audit_no_code_diff |
| frontend-book-notes-localstorage | passed | seeded_code_change | none | ok | none | agent-run | passed | passed | prompt=5134 tool_schema=3186 tools=15 workflow=guarded closeout=full validation=passed:4/4 | none | none | required_validation,first_code_change | 6 | yes | active=true, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | none |
| live-eval-dashboard-summary | passed | seeded_code_change | none | ok | none | agent-run | passed | passed | prompt=6065 tool_schema=3186 tools=15 workflow=guarded closeout=full validation=passed:7/7 | none | none | required_validation,repeated_no_code_progress,first_code_change | 6 | yes | active=true, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | none |
| memory-save-quality-gate | failed | seeded_code_change | llm_reasoning | failed | none | agent-run | failed | failed | prompt=23644 tool_schema=3186 tools=15 workflow=strict closeout=full validation=failed:3/7 | memory_quality_gate,memory_save_outcome_visibility | failed | required_validation,repeated_no_code_progress,first_code_change,verification_failed,acceptance_rejected | 11 | yes | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | tool_errors_seen |
| persistent-memory-planning-context | failed | seeded_code_change | agent_flow | failed | none | agent-run | failed | failed | prompt=39762 tool_schema=3186 tools=15 workflow=strict closeout=full validation=failed:3/9 | memory_planning_context,memory_retrieval_before_workflow_judgment | failed | required_validation,repeated_no_code_progress,first_code_change,verification_failed,acceptance_rejected | 12 | yes | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | action_checkpoint_invalid_tools |
| skill-promotion-gate | passed | seeded_code_change | none | ok | none | agent-run | passed | passed | prompt=16422 tool_schema=3186 tools=15 workflow=guarded closeout=full validation=passed:8/8 | skill_promotion_gate,skill_evolution_cooldown | passed | required_validation,repeated_no_code_progress,first_code_change | 11 | yes | active=true, recalled=0, conflicts=0, changed_plan=true | active=true, tool_calls=0, usage_events=0, promotion=true | none |

## Notes

- `plan_quality` describes plan-only/API artifacts when present.
- `tool_boundary` separates plan-only, collect-only, and real agent-run reports.
- `verification_status` combines closeout and required-command evidence; it is not a human-quality score.
- `skipped` reports are excluded from pass/fail rate denominators; collect-only reports need passing required commands to be scored.
- `real_code_change_passed` requires an agent-run report with a non-empty diff; plan-only success is tracked separately.
- `memory` and `skill` summarize evidence signals; they do not by themselves mean the task succeeded.
- `behavior_assertions` are explicit sample-level checks; memory/skill behavior assertions are stronger evidence than activity signals alone.
