# Live Eval Shortfall Summary

- Generated: `2026-05-06 14:12:19 +0800`
- Runs scanned: `116`
- Task reports scanned: `116`
- Pass rate: `30/116` (25.9%)
- Failure rate: `86/116` (74.1%)
- Real code-change passes: `26`
- Plan-only passes: `0`
- Seeded no-diff failures: `4`
- Required-command failures: `57`
- Verification failures: `85`

## Shortfall Distribution

| dimension | count | share |
|---|---|---|
| failed_tasks | 86 | 74.1% |
| required_command_failed | 57 | 49.1% |
| verification_failed | 85 | 73.3% |
| closeout_not_successful | 81 | 69.8% |
| recovered_validation_failures | 60 | 51.7% |
| seeded_no_diff_failed | 4 | 3.4% |
| owner_metadata_missing | 92 | 79.3% |
| real_code_change_passed | 26 | 22.4% |
| plan_only_passed | 0 | 0.0% |

## Failure Owners

| owner | count | share |
|---|---|---|
| missing | 92 | 79.3% |
| agent_flow | 9 | 7.8% |
| none | 8 | 6.9% |
| llm_reasoning | 6 | 5.2% |
| eval_harness | 1 | 0.9% |

## Inferred Owners

| owner | count | share |
|---|---|---|
| llm_reasoning | 45 | 38.8% |
| agent_flow | 40 | 34.5% |
| none | 30 | 25.9% |
| eval_harness | 1 | 0.9% |

## Metadata Coverage

| dimension | count | share |
|---|---|---|
| structured_failure_owner | 24 | 20.7% |
| structured_eval_intent | 8 | 6.9% |
| adaptive_trigger_metadata | 6 | 5.2% |
| instrumented_task_reports | 24 | 20.7% |

## Instrumented Slice

| dimension | count | share |
|---|---|---|
| task_reports | 24 | 100.0% |
| passed | 8 | 33.3% |
| failed | 16 | 66.7% |
| required_command_failed | 8 | 33.3% |
| verification_failed | 16 | 66.7% |
| seeded_no_diff_failed | 4 | 16.7% |

### Instrumented Owners

| owner | count | share |
|---|---|---|
| agent_flow | 9 | 37.5% |
| none | 8 | 33.3% |
| llm_reasoning | 6 | 25.0% |
| eval_harness | 1 | 4.2% |

### Instrumented Failure Modes

| mode | count |
|---|---|
| verification_failed | 16 |
| closeout_not_successful | 16 |
| warning:no_code_diff | 10 |
| required_command_failed | 8 |
| warning:action_checkpoint_no_patch | 6 |
| warning:action_checkpoint_invalid_tools | 2 |
| warning:tool_errors_seen | 2 |
| warning:current_head_no_fixture_already_satisfied | 1 |

## Failure Modes

| mode | count |
|---|---|
| closeout_not_successful | 81 |
| required_commands_not_passing | 56 |
| warning:no_code_diff | 36 |
| acceptance_review_rejected | 34 |
| stage_validation_failed | 34 |
| verification_failed | 34 |
| earlier_verification_failed_before_repair | 30 |
| earlier_stage_validation_failed_before_repair | 30 |
| warning:action_checkpoint_invalid_tools | 24 |
| warning:tool_errors_seen | 22 |
| expected_code_diff_missing | 18 |
| empty_agent_output | 12 |

## Agent Flow Stops

| mode | count | share |
|---|---|---|
| action_checkpoint_invalid_tools | 24 | 20.7% |
| action_checkpoint_no_patch | 8 | 6.9% |
| empty_agent_output | 12 | 10.3% |
| missing_trace_summary | 10 | 8.6% |
| patch_synthesis_no_change | 2 | 1.7% |
| tool_run_without_closeout | 12 | 10.3% |

## Adaptive Workflow Triggers

| trigger | count | share |
|---|---|---|
| required_validation | 6 | 5.2% |
| repeated_no_code_progress | 6 | 5.2% |
| first_code_change | 4 | 3.4% |
| verification_failed | 3 | 2.6% |
| acceptance_rejected | 3 | 2.6% |

## Eval Intents

| intent | count | share |
|---|---|---|
| missing | 108 | 93.1% |
| seeded_code_change | 8 | 6.9% |

## Seeded No-Diff Tasks

| run | task | owner | required | closeout | warnings |
|---|---|---|---|---|---|
| capability-dashboard-summary-20260503-213148 | live-eval-dashboard-summary | llm_reasoning | failed | not_verified | no_code_diff,action_checkpoint_no_patch |
| capability-dashboard-summary-rerun-20260503-235256 | live-eval-dashboard-summary | llm_reasoning | failed | not_verified | no_code_diff,action_checkpoint_no_patch |
| live-eval-20260506-112213 | code-change-verification-repair-loop | llm_reasoning | failed | not_verified | no_code_diff,tool_errors_seen |
| live-eval-20260506-134158 | code-change-verification-repair-loop | agent_flow | failed | not_verified | no_code_diff,action_checkpoint_no_patch |

## Recent Failed Tasks

| run | task | intent | owner | inferred_owner | required | verification | diff | triggers | warnings |
|---|---|---|---|---|---|---|---|---|---|
| live-eval-20260501-235010 | skill-promotion-gate | missing | missing | agent_flow | failed | failed | no | none | no_code_diff,action_checkpoint_invalid_tools |
| live-eval-20260502-084615 | skill-promotion-gate | missing | missing | llm_reasoning | failed | failed | yes | none | tool_errors_seen |
| live-eval-20260502-094751 | memory-recall-conflict-precision | missing | missing | llm_reasoning | failed | failed | yes | none | none |
| live-eval-20260502-101528 | memory-recall-conflict-precision | missing | missing | agent_flow | ok | failed | no | none | no_code_diff,action_checkpoint_invalid_tools |
| live-eval-20260502-104533 | memory-save-duplicate-demotion | missing | missing | llm_reasoning | failed | failed | yes | none | none |
| live-eval-20260502-115116 | memory-save-sensitive-hard-block | missing | missing | llm_reasoning | failed | failed | yes | none | none |
| live-eval-20260502-125317 | memory-save-quality-gate | missing | missing | agent_flow | failed | failed | no | none | no_code_diff |
| live-eval-20260502-131257 | memory-save-quality-gate | missing | missing | llm_reasoning | failed | failed | yes | none | none |
| live-eval-20260502-143038 | skill-promotion-gate | missing | missing | agent_flow | ok | failed | yes | none | tool_errors_seen |
| live-eval-20260506-105101 | code-change-verification-repair-loop | seeded_code_change | llm_reasoning | llm_reasoning | failed | failed | yes | required_validation,repeated_no_code_progress,first_code_change,verification_failed,acceptance_rejected | none |
| live-eval-20260506-112213 | code-change-verification-repair-loop | seeded_code_change | llm_reasoning | llm_reasoning | failed | failed | no | required_validation,repeated_no_code_progress | no_code_diff,tool_errors_seen |
| live-eval-20260506-113254 | code-change-verification-repair-loop | seeded_code_change | agent_flow | agent_flow | failed | failed | yes | required_validation,repeated_no_code_progress,first_code_change,verification_failed,acceptance_rejected | tool_errors_seen,action_checkpoint_no_patch |
| live-eval-20260506-114009 | code-change-verification-repair-loop | seeded_code_change | llm_reasoning | llm_reasoning | failed | failed | yes | required_validation,repeated_no_code_progress,first_code_change,verification_failed,acceptance_rejected | none |
| live-eval-20260506-134158 | code-change-verification-repair-loop | seeded_code_change | agent_flow | agent_flow | failed | failed | no | required_validation,repeated_no_code_progress | no_code_diff,action_checkpoint_no_patch |
| live-memory-planning-20260502-200232 | persistent-memory-planning-context | missing | agent_flow | agent_flow | ok | failed | yes | none | none |
| live-memory-planning-20260502-224641 | persistent-memory-planning-context | missing | agent_flow | agent_flow | ok | failed | yes | none | none |
| realflow-guided-20260503-170614 | memory-save-quality-gate | missing | llm_reasoning | llm_reasoning | failed | failed | yes | none | none |
| realtask-backend-20260502-181555 | backend-todo-api-crud | missing | missing | llm_reasoning | failed | failed | yes | none | tool_errors_seen |
| realtask-frontend-20260502-161816 | frontend-book-notes-localstorage | missing | missing | llm_reasoning | failed | failed | yes | none | tool_errors_seen |
| realtask-frontend-20260502-164958 | frontend-book-notes-localstorage | missing | missing | agent_flow | ok | failed | yes | none | tool_errors_seen |

## Recent Passed Tasks

| run | task | intent | owner | required | verification | diff | triggers | warnings |
|---|---|---|---|---|---|---|---|---|
| live-eval-20260502-134336 | memory-save-quality-gate | missing | missing | ok | passed | yes | none | none |
| live-eval-20260502-141037 | persistent-memory-planning-context | missing | missing | ok | passed | yes | none | none |
| live-eval-20260502-151157 | skill-promotion-gate | missing | missing | ok | passed | yes | none | none |
| live-eval-20260502-153305 | code-change-verification-repair-loop | missing | missing | ok | passed | yes | none | none |
| live-eval-20260503-152320 | code-change-verification-repair-loop | missing | none | ok | passed | yes | none | none |
| live-eval-20260506-134904 | code-change-verification-repair-loop | seeded_code_change | none | ok | passed | yes | required_validation,repeated_no_code_progress,first_code_change | tool_errors_seen |
| live-isolated-minimax-1 | memory-save-quality-gate | missing | missing | missing | unknown | no | none | none |
| realflow-memory-20260503-163910 | persistent-memory-planning-context | missing | none | ok | passed | yes | none | none |
| realtask-backend-20260502-183603 | backend-todo-api-crud | missing | none | ok | passed | yes | none | none |
| realtask-backend-20260502-195441 | backend-todo-api-crud | missing | none | ok | passed | yes | none | none |
| realtask-frontend-20260502-173045 | frontend-book-notes-localstorage | missing | missing | ok | passed | yes | none | none |
| realtask-frontend-20260502-194738 | frontend-book-notes-localstorage | missing | none | ok | passed | yes | none | none |

## Reading

- `real_code_change_passed` requires an agent-run report with a non-empty diff.
- `plan_only_passed` is tracked separately so planning success is not counted as code-change success.
- `seeded_no_diff_failed` is the strongest signal for agents that inspect but do not patch.
- `inferred_owner` is a conservative backfill for older reports that predate structured `failure_owner` fields.
- `owner_metadata_missing` tracks that historical evidence gap separately from inferred product failures.
- `instrumented_task_reports` is the cleaner current slice because it excludes reports with no structured owner, intent, or trigger metadata.
