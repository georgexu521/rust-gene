# Agent Run Bundle: minimum-agent-verification-repair

- Run id: `ab-20260525-155452-weighted`
- Final status: `failed`
- Terminal status: `partial`
- Stop reason: `repeated_tool_failure`
- Required command status: `failed`
- Verification status: `failed`
- Closeout status: `not_verified`
- Runtime spine: `coverage=6/7, status=failed, missing=completion_status:completed,terminal_status:completed,verification_proof_status:verified`
- Outcome score: `0`
- Process score: `30`
- Efficiency score: `43`
- Agent score: `18`
- Score penalties: `run_failed,required_commands_failed,verification_failed,closeout_not_successful,runtime_spine_failed,output_assertions_failed,trajectory_assertions_failed,expected_code_diff_missing,scope_drift,repeated_action,invalid_action,runtime_spine_not_passing,tool_budget_exceeded,failed_actions,repeated_actions,llm_call_budget_pressure`

## Key Metrics

- Tool calls: `11`
- Failed actions: `4`
- Repeated actions: `8`
- Premature edits: `0`
- Scope drift count: `1`
- Invalid action count: `10`

## Artifacts

- `task.json`
- `steps.jsonl`
- `events.jsonl`
