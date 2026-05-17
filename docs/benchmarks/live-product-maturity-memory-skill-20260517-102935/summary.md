# Live Eval Summary: product-maturity-memory-skill-20260517-102935

- Run directory: `docs/benchmarks/live-product-maturity-memory-skill-20260517-102935`
- Tasks found: `6`
- Pass rate: `5/6` (83.3%)
- Failure rate: `1/6` (16.7%)
- Real code-change passes: `2`
- Plan-only passes: `0`
- Seeded no-diff failures: `1`
- Memory active tasks: `5`
- Memory changed-plan tasks: `2`
- Memory recalled items: `0`
- Memory conflicts: `0`
- Skill active tasks: `1`
- Skill promotion-evidence tasks: `1`
- Behavior assertion tasks: `6`
- Behavior assertions passed: `5`
- Status counts: failed=1, passed=5
- Failure owners: agent_flow=1, none=5
- Eval intents: audit_or_regression_check=3, seeded_code_change=3

## Failure Modes

- `warning:no_code_diff`: `4`
- `warning:audit_no_code_diff`: `3`
- `warning:current_head_no_fixture_already_satisfied`: `3`
- `earlier_stage_validation_failed_before_repair`: `2`
- `earlier_verification_failed_before_repair`: `2`
- `associated functions `build_memory_context` and `record_memory_prefetch` are never used`: `1`
- `behavior_assertions_not_passing`: `1`
- `closeout_not_successful`: `1`
- `empty_agent_output`: `1`
- `expected_code_diff_missing`: `1`
- `fields `memory_manager`, `provider`, and `model` are never read`: `1`
- `required_commands_not_passing`: `1`

## Memory And Skill Evidence

| dimension | count | meaning |
|-----------|-------|---------|
| memory_active_tasks | 5 | Tasks where retrieval, sync, or memory tools were active. |
| memory_changed_plan_tasks | 2 | Tasks where memory or learning signals reweighted planning. |
| memory_recalled_items | 0 | Retrieved memory-backed context items across tasks. |
| memory_conflicts | 0 | Retrieval-context conflict count from memory-backed context. |
| skill_active_tasks | 1 | Tasks where skill tools or skill-specific signals were active. |
| skill_promotion_evidence_tasks | 1 | Tasks with promotion-related skill evidence. |
| behavior_assertion_tasks | 6 | Tasks with explicit behavior assertions in the live-eval sample. |
| behavior_assertions_passed | 5 | Explicit behavior-assertion tasks whose required checks passed. |
| memory_behavior_assertion_tasks | 5 | Behavior assertions covering memory semantics rather than only memory activity signals. |
| skill_behavior_assertion_tasks | 1 | Behavior assertions covering skill semantics rather than only skill activity signals. |

## Outcome Classes

| class | count | meaning |
|-------|-------|---------|
| real_code_change_passed | 2 | Agent-run tasks with passing status and a real diff. |
| plan_only_passed | 0 | Planning/API-only artifacts that passed their available checks. |
| seeded_no_diff_failed | 1 | Seeded code-change tasks where the agent did not produce a diff. |

## Task Matrix

| task | status | intent | owner | required | plan_quality | tool_boundary | verification_status | closeout | runtime_diet | behavior_assertions | behavior_status | triggers | first_write | diff | memory | skill | warnings |
|------|--------|--------|-------|----------|--------------|---------------|---------------------|----------|--------------|---------------------|-----------------|----------|-------------|------|--------|-------|----------|
| memory-recall-conflict-precision | passed | audit_or_regression_check | none | ok | none | agent-run | passed | passed | prompt=17914 tool_schema=3186 tools=15 workflow=strict closeout=full validation=passed:3/3 | memory_conflict_precision,memory_recall_demotion | passed | required_validation | none | no | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff,audit_no_code_diff,current_head_no_fixture_already_satisfied |
| memory-save-duplicate-demotion | passed | audit_or_regression_check | none | ok | none | agent-run | passed | passed | prompt=18754 tool_schema=3186 tools=15 workflow=guarded closeout=full validation=passed:2/2 | memory_duplicate_demotion,memory_namespace_precision | passed | required_validation | none | no | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff,audit_no_code_diff,current_head_no_fixture_already_satisfied |
| memory-save-quality-gate | passed | seeded_code_change | none | ok | none | agent-run | passed | passed | prompt=16653 tool_schema=3186 tools=15 workflow=strict closeout=full validation=passed:7/7 recovered_failed:1 | memory_quality_gate,memory_save_outcome_visibility | passed | required_validation,repeated_no_code_progress,first_code_change,verification_failed,acceptance_rejected | 10 | yes | active=true, recalled=0, conflicts=0, changed_plan=true | active=false, tool_calls=0, usage_events=0, promotion=false | none |
| memory-save-sensitive-hard-block | passed | audit_or_regression_check | none | ok | none | agent-run | passed | passed | prompt=22240 tool_schema=3186 tools=15 workflow=strict closeout=full validation=passed:3/3 | memory_sensitive_hard_block,memory_save_outcome_visibility | passed | required_validation | none | no | active=true, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff,audit_no_code_diff,current_head_no_fixture_already_satisfied |
| persistent-memory-planning-context | failed | seeded_code_change | agent_flow | failed | none | agent-run | failed | missing | missing | memory_planning_context,memory_retrieval_before_workflow_judgment | failed | required_validation | none | no | active=false, recalled=0, conflicts=0, changed_plan=false | active=false, tool_calls=0, usage_events=0, promotion=false | no_code_diff |
| skill-promotion-gate | passed | seeded_code_change | none | ok | none | agent-run | passed | passed | prompt=17610 tool_schema=3186 tools=15 workflow=strict closeout=full validation=failed:1/9 | skill_promotion_gate,skill_evolution_cooldown | passed | required_validation,repeated_no_code_progress,first_code_change,verification_failed,acceptance_rejected | 10 | yes | active=true, recalled=0, conflicts=0, changed_plan=true | active=true, tool_calls=0, usage_events=0, promotion=true | none |

## Notes

- `plan_quality` describes plan-only/API artifacts when present.
- `tool_boundary` separates plan-only, collect-only, and real agent-run reports.
- `verification_status` combines closeout and required-command evidence; it is not a human-quality score.
- `real_code_change_passed` requires an agent-run report with a non-empty diff; plan-only success is tracked separately.
- `memory` and `skill` summarize evidence signals; they do not by themselves mean the task succeeded.
- `behavior_assertions` are explicit sample-level checks; memory/skill behavior assertions are stronger evidence than activity signals alone.
