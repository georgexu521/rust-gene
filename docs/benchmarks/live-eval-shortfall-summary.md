# Live Eval Shortfall Summary

- Generated: `2026-05-09 13:04:15 +0800`
- Runs scanned: `136`
- Task reports scanned: `136`
- Pass rate: `35/136` (25.7%)
- Failure rate: `101/136` (74.3%)
- Real code-change passes: `8`
- Plan-only passes: `0`
- Seeded no-diff failures: `16`
- Required-command failures: `71`
- Verification failures: `100`

## Shortfall Distribution

| dimension | count | share |
|---|---|---|
| failed_tasks | 101 | 74.3% |
| required_command_failed | 71 | 52.2% |
| verification_failed | 100 | 73.5% |
| closeout_not_successful | 96 | 70.6% |
| recovered_validation_failures | 64 | 47.1% |
| seeded_no_diff_failed | 16 | 11.8% |
| owner_metadata_missing | 92 | 67.6% |
| real_code_change_passed | 8 | 5.9% |
| plan_only_passed | 0 | 0.0% |

## Failure Owners

| owner | count | share |
|---|---|---|
| missing | 92 | 67.6% |
| agent_flow | 19 | 14.0% |
| none | 13 | 9.6% |
| llm_reasoning | 9 | 6.6% |
| eval_harness | 3 | 2.2% |

## Inferred Owners

| owner | count | share |
|---|---|---|
| agent_flow | 63 | 46.3% |
| none | 35 | 25.7% |
| llm_reasoning | 35 | 25.7% |
| eval_harness | 3 | 2.2% |

## Metadata Coverage

| dimension | count | share |
|---|---|---|
| structured_failure_owner | 44 | 32.4% |
| structured_eval_intent | 28 | 20.6% |
| adaptive_trigger_metadata | 21 | 15.4% |
| instrumented_task_reports | 44 | 32.4% |

## Instrumented Slice

| dimension | count | share |
|---|---|---|
| task_reports | 44 | 100.0% |
| passed | 13 | 29.5% |
| failed | 31 | 70.5% |
| required_command_failed | 22 | 50.0% |
| verification_failed | 31 | 70.5% |
| seeded_no_diff_failed | 16 | 36.4% |

### Instrumented Owners

| owner | count | share |
|---|---|---|
| agent_flow | 19 | 43.2% |
| none | 13 | 29.5% |
| llm_reasoning | 9 | 20.5% |
| eval_harness | 3 | 6.8% |

### Instrumented Failure Modes

| mode | count |
|---|---|
| verification_failed | 31 |
| closeout_not_successful | 30 |
| required_command_failed | 22 |
| warning:no_code_diff | 19 |
| warning:action_checkpoint_no_patch | 8 |
| warning:action_checkpoint_invalid_tools | 6 |
| warning:tool_errors_seen | 2 |
| warning:current_head_no_fixture_already_satisfied | 1 |

## Failure Modes

| mode | count |
|---|---|
| closeout_not_successful | 96 |
| required_commands_not_passing | 70 |
| warning:no_code_diff | 45 |
| acceptance_review_rejected | 35 |
| stage_validation_failed | 35 |
| verification_failed | 35 |
| earlier_verification_failed_before_repair | 32 |
| earlier_stage_validation_failed_before_repair | 32 |
| warning:action_checkpoint_invalid_tools | 29 |
| expected_code_diff_missing | 27 |
| warning:tool_errors_seen | 22 |
| empty_agent_output | 17 |

## Agent Flow Stops

| mode | count | share |
|---|---|---|
| action_checkpoint_invalid_tools | 33 | 24.3% |
| action_checkpoint_no_patch | 12 | 8.8% |
| empty_agent_output | 17 | 12.5% |
| missing_trace_summary | 15 | 11.0% |
| patch_synthesis_no_change | 2 | 1.5% |
| tool_run_without_closeout | 17 | 12.5% |

## Adaptive Workflow Triggers

| trigger | count | share |
|---|---|---|
| required_validation | 21 | 15.4% |
| repeated_no_code_progress | 17 | 12.5% |
| first_code_change | 10 | 7.4% |
| verification_failed | 6 | 4.4% |
| acceptance_rejected | 6 | 4.4% |

## Eval Intents

| intent | count | share |
|---|---|---|
| missing | 108 | 79.4% |
| seeded_code_change | 28 | 20.6% |

## Seeded No-Diff Tasks

| run | task | owner | required | closeout | warnings |
|---|---|---|---|---|---|
| capability-dashboard-summary-20260503-213148 | live-eval-dashboard-summary | llm_reasoning | failed | not_verified | no_code_diff,action_checkpoint_no_patch |
| capability-dashboard-summary-rerun-20260503-235256 | live-eval-dashboard-summary | llm_reasoning | failed | not_verified | no_code_diff,action_checkpoint_no_patch |
| checkpoint-function-boundary-20260509-115326 | live-eval-dashboard-summary | agent_flow | failed | not_verified | no_code_diff,action_checkpoint_invalid_tools |
| checkpoint-highlight-guard-20260509-112915 | live-eval-dashboard-summary | agent_flow | failed | not_verified | no_code_diff,action_checkpoint_invalid_tools |
| checkpoint-recovery-20260508-230559 | live-eval-dashboard-summary | agent_flow | failed | not_verified | no_code_diff,action_checkpoint_no_patch |
| live-eval-20260506-105101 | code-change-verification-repair-loop | llm_reasoning | failed | failed | none |
| live-eval-20260506-112213 | code-change-verification-repair-loop | llm_reasoning | failed | not_verified | no_code_diff,tool_errors_seen |
| live-eval-20260506-113254 | code-change-verification-repair-loop | agent_flow | failed | failed | tool_errors_seen,action_checkpoint_no_patch |
| live-eval-20260506-114009 | code-change-verification-repair-loop | llm_reasoning | failed | failed | none |
| live-eval-20260506-134158 | code-change-verification-repair-loop | agent_flow | failed | not_verified | no_code_diff,action_checkpoint_no_patch |
| patch-synth-evidence-20260508-231044 | live-eval-dashboard-summary | agent_flow | failed | not_verified | no_code_diff,action_checkpoint_invalid_tools |
| patch-synth-probe-20260508-224845 | live-eval-dashboard-summary | agent_flow | failed | not_verified | no_code_diff,action_checkpoint_invalid_tools |
| post-commit-suite-20260508-222022 | live-eval-dashboard-summary | llm_reasoning | failed | missing | no_code_diff |
| post-commit-suite-20260508-222823 | frontend-book-notes-localstorage | llm_reasoning | failed | missing | no_code_diff |
| post-commit-suite-20260508-222926 | memory-save-quality-gate | llm_reasoning | failed | missing | no_code_diff |
| post-route-fix-20260508-224035 | live-eval-dashboard-summary | agent_flow | failed | not_verified | no_code_diff,action_checkpoint_no_patch |

## Recent Failed Tasks

| run | task | intent | owner | inferred_owner | required | verification | diff | triggers | warnings |
|---|---|---|---|---|---|---|---|---|---|
| live-eval-20260502-143038 | skill-promotion-gate | missing | missing | agent_flow | ok | failed | no | none | tool_errors_seen |
| live-eval-20260506-105101 | code-change-verification-repair-loop | seeded_code_change | llm_reasoning | llm_reasoning | failed | failed | no | required_validation,repeated_no_code_progress,first_code_change,verification_failed,acceptance_rejected | none |
| live-eval-20260506-112213 | code-change-verification-repair-loop | seeded_code_change | llm_reasoning | llm_reasoning | failed | failed | no | required_validation,repeated_no_code_progress | no_code_diff,tool_errors_seen |
| live-eval-20260506-113254 | code-change-verification-repair-loop | seeded_code_change | agent_flow | agent_flow | failed | failed | no | required_validation,repeated_no_code_progress,first_code_change,verification_failed,acceptance_rejected | tool_errors_seen,action_checkpoint_no_patch |
| live-eval-20260506-114009 | code-change-verification-repair-loop | seeded_code_change | llm_reasoning | llm_reasoning | failed | failed | no | required_validation,repeated_no_code_progress,first_code_change,verification_failed,acceptance_rejected | none |
| live-eval-20260506-134158 | code-change-verification-repair-loop | seeded_code_change | agent_flow | agent_flow | failed | failed | no | required_validation,repeated_no_code_progress | no_code_diff,action_checkpoint_no_patch |
| live-memory-planning-20260502-200232 | persistent-memory-planning-context | missing | agent_flow | agent_flow | ok | failed | no | none | none |
| live-memory-planning-20260502-224641 | persistent-memory-planning-context | missing | agent_flow | agent_flow | ok | failed | no | none | none |
| patch-synth-evidence-20260508-231044 | live-eval-dashboard-summary | seeded_code_change | agent_flow | agent_flow | failed | failed | no | required_validation,repeated_no_code_progress | no_code_diff,action_checkpoint_invalid_tools |
| patch-synth-probe-20260508-224845 | live-eval-dashboard-summary | seeded_code_change | agent_flow | agent_flow | failed | failed | no | required_validation,repeated_no_code_progress | no_code_diff,action_checkpoint_invalid_tools |
| patch-synth-relaxed-20260508-232627 | live-eval-dashboard-summary | seeded_code_change | eval_harness | eval_harness | failed | failed | yes | none | none |
| post-commit-suite-20260508-214812 | code-change-verification-repair-loop | seeded_code_change | agent_flow | agent_flow | ok | failed | yes | none | none |
| post-commit-suite-20260508-222022 | live-eval-dashboard-summary | seeded_code_change | llm_reasoning | llm_reasoning | failed | failed | no | required_validation | no_code_diff |
| post-commit-suite-20260508-222823 | frontend-book-notes-localstorage | seeded_code_change | llm_reasoning | llm_reasoning | failed | failed | no | required_validation | no_code_diff |
| post-commit-suite-20260508-222926 | memory-save-quality-gate | seeded_code_change | llm_reasoning | llm_reasoning | failed | failed | no | required_validation | no_code_diff |
| post-route-fix-20260508-224035 | live-eval-dashboard-summary | seeded_code_change | agent_flow | agent_flow | failed | failed | no | required_validation,repeated_no_code_progress | no_code_diff,action_checkpoint_no_patch |
| realflow-guided-20260503-170614 | memory-save-quality-gate | missing | llm_reasoning | llm_reasoning | failed | failed | yes | none | none |
| realtask-backend-20260502-181555 | backend-todo-api-crud | missing | missing | agent_flow | failed | failed | no | none | tool_errors_seen |
| realtask-frontend-20260502-161816 | frontend-book-notes-localstorage | missing | missing | agent_flow | failed | failed | no | none | tool_errors_seen |
| realtask-frontend-20260502-164958 | frontend-book-notes-localstorage | missing | missing | agent_flow | ok | failed | no | none | tool_errors_seen |

## Recent Passed Tasks

| run | task | intent | owner | required | verification | diff | triggers | warnings |
|---|---|---|---|---|---|---|---|---|
| live-eval-20260502-151157 | skill-promotion-gate | missing | missing | ok | passed | no | none | none |
| live-eval-20260502-153305 | code-change-verification-repair-loop | missing | missing | ok | passed | no | none | none |
| live-eval-20260503-152320 | code-change-verification-repair-loop | missing | none | ok | passed | yes | none | none |
| live-eval-20260506-134904 | code-change-verification-repair-loop | seeded_code_change | none | ok | passed | no | required_validation,repeated_no_code_progress,first_code_change | tool_errors_seen |
| live-eval-20260506-142145 | code-change-verification-repair-loop | seeded_code_change | none | ok | passed | no | required_validation,repeated_no_code_progress,first_code_change | none |
| live-isolated-minimax-1 | memory-save-quality-gate | missing | missing | missing | unknown | no | none | none |
| post-commit-suite-20260508-222425 | backend-todo-api-crud | seeded_code_change | none | ok | passed | yes | required_validation,first_code_change,verification_failed,acceptance_rejected | none |
| realflow-memory-20260503-163910 | persistent-memory-planning-context | missing | none | ok | passed | yes | none | none |
| realtask-backend-20260502-183603 | backend-todo-api-crud | missing | none | ok | passed | no | none | none |
| realtask-backend-20260502-195441 | backend-todo-api-crud | missing | none | ok | passed | no | none | none |
| realtask-frontend-20260502-173045 | frontend-book-notes-localstorage | missing | missing | ok | passed | no | none | none |
| realtask-frontend-20260502-194738 | frontend-book-notes-localstorage | missing | none | ok | passed | no | none | none |

## Reading

- `real_code_change_passed` requires an agent-run report with a non-empty diff.
- `plan_only_passed` is tracked separately so planning success is not counted as code-change success.
- `seeded_no_diff_failed` is the strongest signal for agents that inspect but do not patch.
- `inferred_owner` is a conservative backfill for older reports that predate structured `failure_owner` fields.
- `owner_metadata_missing` tracks that historical evidence gap separately from inferred product failures.
- `instrumented_task_reports` is the cleaner current slice because it excludes reports with no structured owner, intent, or trigger metadata.
