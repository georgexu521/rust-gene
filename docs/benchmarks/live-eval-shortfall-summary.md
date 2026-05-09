# Live Eval Shortfall Summary

- Generated: `2026-05-09 14:14:51 +0800`
- Runs scanned: `138`
- Task reports scanned: `138`
- Pass rate: `37/138` (26.8%)
- Failure rate: `101/138` (73.2%)
- Real code-change passes: `10`
- Plan-only passes: `0`
- Seeded no-diff failures: `16`
- Required-command failures: `71`
- Verification failures: `100`

## Shortfall Distribution

| dimension | count | share |
|---|---|---|
| failed_tasks | 101 | 73.2% |
| required_command_failed | 71 | 51.4% |
| verification_failed | 100 | 72.5% |
| closeout_not_successful | 96 | 69.6% |
| recovered_validation_failures | 66 | 47.8% |
| seeded_no_diff_failed | 16 | 11.6% |
| owner_metadata_missing | 92 | 66.7% |
| real_code_change_passed | 10 | 7.2% |
| plan_only_passed | 0 | 0.0% |

## Failure Owners

| owner | count | share |
|---|---|---|
| missing | 92 | 66.7% |
| agent_flow | 19 | 13.8% |
| none | 15 | 10.9% |
| llm_reasoning | 9 | 6.5% |
| eval_harness | 3 | 2.2% |

## Inferred Owners

| owner | count | share |
|---|---|---|
| agent_flow | 63 | 45.7% |
| none | 37 | 26.8% |
| llm_reasoning | 35 | 25.4% |
| eval_harness | 3 | 2.2% |

## Metadata Coverage

| dimension | count | share |
|---|---|---|
| structured_failure_owner | 46 | 33.3% |
| structured_eval_intent | 30 | 21.7% |
| adaptive_trigger_metadata | 23 | 16.7% |
| instrumented_task_reports | 46 | 33.3% |

## Instrumented Slice

| dimension | count | share |
|---|---|---|
| task_reports | 46 | 100.0% |
| passed | 15 | 32.6% |
| failed | 31 | 67.4% |
| required_command_failed | 22 | 47.8% |
| verification_failed | 31 | 67.4% |
| seeded_no_diff_failed | 16 | 34.8% |

### Instrumented Owners

| owner | count | share |
|---|---|---|
| agent_flow | 19 | 41.3% |
| none | 15 | 32.6% |
| llm_reasoning | 9 | 19.6% |
| eval_harness | 3 | 6.5% |

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
| earlier_verification_failed_before_repair | 33 |
| earlier_stage_validation_failed_before_repair | 33 |
| warning:action_checkpoint_invalid_tools | 29 |
| expected_code_diff_missing | 27 |
| warning:tool_errors_seen | 23 |
| empty_agent_output | 17 |

## Agent Flow Stops

| mode | count | share |
|---|---|---|
| action_checkpoint_invalid_tools | 33 | 23.9% |
| action_checkpoint_no_patch | 12 | 8.7% |
| empty_agent_output | 17 | 12.3% |
| missing_trace_summary | 15 | 10.9% |
| patch_synthesis_no_change | 2 | 1.4% |
| tool_run_without_closeout | 17 | 12.3% |

## Adaptive Workflow Triggers

| trigger | count | share |
|---|---|---|
| required_validation | 23 | 16.7% |
| repeated_no_code_progress | 19 | 13.8% |
| first_code_change | 12 | 8.7% |
| verification_failed | 7 | 5.1% |
| acceptance_rejected | 7 | 5.1% |

## Eval Intents

| intent | count | share |
|---|---|---|
| missing | 108 | 78.3% |
| seeded_code_change | 30 | 21.7% |

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
