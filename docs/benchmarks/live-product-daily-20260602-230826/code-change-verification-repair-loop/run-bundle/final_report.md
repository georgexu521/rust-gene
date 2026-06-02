# Agent Run Bundle: code-change-verification-repair-loop

- Run id: `product-daily-20260602-230826`
- Final status: `failed`
- Terminal status: `missing`
- Stop reason: `missing`
- Required command status: `failed`
- Verification status: `failed`
- Closeout status: `missing`
- Runtime spine: `coverage=0/7, status=missing, missing=phase:context,phase:decision,phase:tool_execution,phase:state_update,phase:verification,phase:closeout,event:action_decision_evaluated,event:stop_check_evaluated,special:verification_proof`
- Outcome score: `0`
- Process score: `42`
- Efficiency score: `93`
- Agent score: `31`
- Score penalties: `run_failed,required_commands_failed,verification_failed,closeout_not_successful,runtime_spine_failed,expected_code_diff_missing,repeated_action,invalid_action,risky_tool_missing_review,runtime_spine_not_passing,observer_outcome_missing,stop_check_missing,repeated_actions`

## Key Metrics

- Tool calls: `3`
- Failed actions: `0`
- Repeated actions: `1`
- Premature edits: `0`
- Scope drift count: `0`
- Invalid action count: `2`

## Artifacts

- `task.json`
- `steps.jsonl`
- `events.jsonl`
