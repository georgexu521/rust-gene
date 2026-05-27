# Agent Run Bundle: minimum-agent-verification-repair

- Run id: `flow-mva-20260527-083214`
- Final status: `failed`
- Terminal status: `missing`
- Stop reason: `missing`
- Required command status: `failed`
- Verification status: `failed`
- Closeout status: `missing`
- Runtime spine: `coverage=0/7, status=missing, missing=phase:context,phase:decision,phase:tool_execution,phase:state_update,phase:verification,phase:closeout,event:context_zones_materialized,event:action_decision_evaluated,event:action_reviewed,event:agent_loop_step_evaluated,event:stop_check_evaluated,event:completion_contract_evaluated,completion_status:completed,terminal_status:completed,verification_proof_status:verified,special:verification_proof`
- Outcome score: `0`
- Process score: `55`
- Efficiency score: `87`
- Agent score: `34`
- Score penalties: `run_failed,required_commands_failed,verification_failed,closeout_not_successful,runtime_spine_failed,output_assertions_failed,trajectory_assertions_failed,expected_code_diff_missing,invalid_action,risky_tool_missing_review,runtime_spine_not_passing,observer_outcome_missing,stop_check_missing,failed_actions,user_questions`

## Key Metrics

- Tool calls: `2`
- Failed actions: `1`
- Repeated actions: `0`
- Premature edits: `0`
- Scope drift count: `0`
- Invalid action count: `1`

## Artifacts

- `task.json`
- `steps.jsonl`
- `events.jsonl`
