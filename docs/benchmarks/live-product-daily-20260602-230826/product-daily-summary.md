# Product Daily Gate Summary

- Run id: `product-daily-20260602-230826`
- Generated: 2026-06-02T23:52:41.906176
- Pass rate: **0/9**

## Results

| Case                                     | Status       | Score  | Owner           | Closeout     | Phases               | Memory   |
|------------------------------------------|--------------|--------|-----------------|--------------|----------------------|----------|
| core-inspection-grounding                | status=ok
failure_owner=none
warning=audit_no_code_diff | -      | unknown         | unknown      | -                    | no       |
| core-simple-stale-edit                   | status=failed
failure_owner=mixed
failure=forbidden_tool_used | -      | unknown         | unknown      | -                    | no       |
| core-multi-file-edit                     | status=ok
failure_owner=none | -      | unknown         | unknown      | -                    | no       |
| core-rust-multi-file-refactor            | status=ok
failure_owner=none
warning=tool_errors_seen
warning=earlier_verification_failed_before_repair
warning=earlier_stage_validation_failed_before_repair | -      | unknown         | unknown      | -                    | no       |
| code-change-verification-repair-loop     | status=failed
failure_owner=agent_flow
failure=empty_agent_output
failure=tool_run_without_closeout
failure=missing_trace_summary
failure=required_commands_not_passing
failure=runtime_spine_assertions_not_passing
failure=closeout_not_successful
failure=expected_code_diff_missing
warning=no_code_diff | -      | unknown         | unknown      | -                    | no       |
| project-partner-resume-with-memory       | status=failed
failure_owner=agent_flow
failure=trajectory_assertions_not_passing
warning=audit_no_code_diff | -      | unknown         | unknown      | -                    | no       |
| memory-recall-conflict-precision         | status=failed
failure_owner=mixed
failure=required_commands_not_passing
failure=behavior_assertions_not_passing
failure=closeout_not_successful
warning=audit_no_code_diff
warning=tool_errors_seen | -      | unknown         | unknown      | -                    | no       |
| minimum-agent-verification-repair        | status=ok
failure_owner=none | -      | unknown         | unknown      | -                    | no       |
| desktop-ui-smoke-polish                  | status=failed
failure_owner=mixed
failure=required_commands_not_passing
failure=closeout_not_successful
warning=audit_no_code_diff
warning=tool_errors_seen | -      | unknown         | unknown      | -                    | no       |

## Failure Owners

- `unknown`: 9

## Runtime Spine Coverage

- No spine data

## Next Steps

- All cases passed. No immediate action required.
