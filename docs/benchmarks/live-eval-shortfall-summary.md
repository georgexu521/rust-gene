# Live Eval Shortfall Summary

- Generated: `2026-05-05 23:21:22 +0800`
- Runs scanned: `111`
- Task reports scanned: `110`
- Pass rate: `29/110` (26.4%)
- Failure rate: `81/110` (73.6%)
- Real code-change passes: `25`
- Plan-only passes: `0`
- Seeded no-diff failures: `2`
- Required-command failures: `52`
- Verification failures: `80`

## Shortfall Distribution

| dimension | count | share |
|---|---|---|
| failed_tasks | 81 | 73.6% |
| required_command_failed | 52 | 47.3% |
| verification_failed | 80 | 72.7% |
| closeout_not_successful | 76 | 69.1% |
| recovered_validation_failures | 56 | 50.9% |
| seeded_no_diff_failed | 2 | 1.8% |
| owner_metadata_missing | 92 | 83.6% |
| real_code_change_passed | 25 | 22.7% |
| plan_only_passed | 0 | 0.0% |

## Failure Owners

| owner | count | share |
|---|---|---|
| missing | 92 | 83.6% |
| none | 7 | 6.4% |
| agent_flow | 7 | 6.4% |
| llm_reasoning | 3 | 2.7% |
| eval_harness | 1 | 0.9% |

## Inferred Owners

| owner | count | share |
|---|---|---|
| llm_reasoning | 42 | 38.2% |
| agent_flow | 38 | 34.5% |
| none | 29 | 26.4% |
| eval_harness | 1 | 0.9% |

## Failure Modes

| mode | count |
|---|---|
| closeout_not_successful | 76 |
| required_commands_not_passing | 51 |
| warning:no_code_diff | 34 |
| acceptance_review_rejected | 31 |
| stage_validation_failed | 31 |
| verification_failed | 31 |
| earlier_stage_validation_failed_before_repair | 28 |
| earlier_verification_failed_before_repair | 28 |
| warning:action_checkpoint_invalid_tools | 24 |
| warning:tool_errors_seen | 19 |
| expected_code_diff_missing | 16 |
| empty_agent_output | 12 |

## Agent Flow Stops

| mode | count | share |
|---|---|---|
| action_checkpoint_invalid_tools | 24 | 21.8% |
| action_checkpoint_no_patch | 4 | 3.6% |
| empty_agent_output | 12 | 10.9% |
| missing_trace_summary | 10 | 9.1% |
| patch_synthesis_no_change | 2 | 1.8% |
| tool_run_without_closeout | 12 | 10.9% |

## Eval Intents

| intent | count | share |
|---|---|---|
| missing | 108 | 98.2% |
| seeded_code_change | 2 | 1.8% |

## Seeded No-Diff Tasks

| run | task | owner | required | closeout | warnings |
|---|---|---|---|---|---|
| capability-dashboard-summary-20260503-213148 | live-eval-dashboard-summary | llm_reasoning | failed | not_verified | no_code_diff,action_checkpoint_no_patch |
| capability-dashboard-summary-rerun-20260503-235256 | live-eval-dashboard-summary | llm_reasoning | failed | not_verified | no_code_diff,action_checkpoint_no_patch |

## Recent Failed Tasks

| run | task | intent | owner | inferred_owner | required | verification | diff | warnings |
|---|---|---|---|---|---|---|---|---|
| live-eval-20260501-211109 | code-change-verification-repair-loop | missing | missing | llm_reasoning | ok | failed | yes | none |
| live-eval-20260501-215158 | code-change-verification-repair-loop | missing | missing | agent_flow | failed | failed | no | no_code_diff,action_checkpoint_invalid_tools |
| live-eval-20260501-225616 | skill-promotion-gate | missing | missing | agent_flow | ok | failed | no | no_code_diff,action_checkpoint_invalid_tools |
| live-eval-20260501-231638 | skill-promotion-gate | missing | missing | agent_flow | failed | failed | no | no_code_diff,action_checkpoint_invalid_tools |
| live-eval-20260501-233203 | skill-promotion-gate | missing | missing | llm_reasoning | failed | failed | yes | tool_errors_seen,action_checkpoint_invalid_tools |
| live-eval-20260501-235010 | skill-promotion-gate | missing | missing | agent_flow | failed | failed | no | no_code_diff,action_checkpoint_invalid_tools |
| live-eval-20260502-084615 | skill-promotion-gate | missing | missing | llm_reasoning | failed | failed | yes | tool_errors_seen |
| live-eval-20260502-094751 | memory-recall-conflict-precision | missing | missing | llm_reasoning | failed | failed | yes | none |
| live-eval-20260502-101528 | memory-recall-conflict-precision | missing | missing | agent_flow | ok | failed | no | no_code_diff,action_checkpoint_invalid_tools |
| live-eval-20260502-104533 | memory-save-duplicate-demotion | missing | missing | llm_reasoning | failed | failed | yes | none |
| live-eval-20260502-115116 | memory-save-sensitive-hard-block | missing | missing | llm_reasoning | failed | failed | yes | none |
| live-eval-20260502-125317 | memory-save-quality-gate | missing | missing | agent_flow | failed | failed | no | no_code_diff |
| live-eval-20260502-131257 | memory-save-quality-gate | missing | missing | llm_reasoning | failed | failed | yes | none |
| live-eval-20260502-143038 | skill-promotion-gate | missing | missing | agent_flow | ok | failed | yes | tool_errors_seen |
| live-memory-planning-20260502-200232 | persistent-memory-planning-context | missing | agent_flow | agent_flow | ok | failed | yes | none |
| live-memory-planning-20260502-224641 | persistent-memory-planning-context | missing | agent_flow | agent_flow | ok | failed | yes | none |
| realflow-guided-20260503-170614 | memory-save-quality-gate | missing | llm_reasoning | llm_reasoning | failed | failed | yes | none |
| realtask-backend-20260502-181555 | backend-todo-api-crud | missing | missing | llm_reasoning | failed | failed | yes | tool_errors_seen |
| realtask-frontend-20260502-161816 | frontend-book-notes-localstorage | missing | missing | llm_reasoning | failed | failed | yes | tool_errors_seen |
| realtask-frontend-20260502-164958 | frontend-book-notes-localstorage | missing | missing | agent_flow | ok | failed | yes | tool_errors_seen |

## Reading

- `real_code_change_passed` requires an agent-run report with a non-empty diff.
- `plan_only_passed` is tracked separately so planning success is not counted as code-change success.
- `seeded_no_diff_failed` is the strongest signal for agents that inspect but do not patch.
- `inferred_owner` is a conservative backfill for older reports that predate structured `failure_owner` fields.
- `owner_metadata_missing` tracks that historical evidence gap separately from inferred product failures.
