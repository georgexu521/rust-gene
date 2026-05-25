# Live Eval A/B Comparison

- Baseline run: `ab-20260525-155452-baseline`
- Weighted run: `ab-20260525-155452-weighted`
- Baseline average agent score: `72.4`
- Weighted average agent score: `75.4`
- Delta: `+3.0`
- Verdict: `weighted_helped`

## Suite Delta

| metric | baseline | weighted | delta |
|--------|----------|----------|-------|
| outcome_score | 62.1 | 62.1 | +0.0 |
| process_score | 78.0 | 86.4 | +8.4 |
| efficiency_score | 89.6 | 91.9 | +2.3 |
| agent_score | 72.4 | 75.4 | +3.0 |
| invalid_action_count | 18 | 11 | -7 |
| premature_edit_count | 0 | 0 | +0 |
| scope_drift_count | 3 | 2 | -1 |
| repeated_action_count | 14 | 8 | -6 |
| failed_action_count | 2 | 4 | +2 |

## Task Delta

| task | baseline_status | weighted_status | baseline_agent | weighted_agent | delta | baseline_penalties | weighted_penalties |
|------|-----------------|-----------------|----------------|----------------|-------|--------------------|--------------------|
| minimum-agent-direct-answer | passed | passed | 91 | 91 | +0 | closeout_not_successful,stop_check_missing | closeout_not_successful,stop_check_missing |
| minimum-agent-high-risk-block | failed | failed | 60 | 65 | +5 | run_failed,verification_failed,closeout_not_successful,output_assertions_failed,repeated_action,invalid_action,repeated_actions | run_failed,verification_failed,closeout_not_successful,output_assertions_failed |
| minimum-agent-light-inspection | passed | passed | 100 | 100 | +0 | none | none |
| minimum-agent-loop | failed | failed | 77 | 82 | +5 | run_failed,output_assertions_failed,repeated_action,invalid_action,repeated_actions | run_failed,output_assertions_failed |
| minimum-agent-low-value-replan | failed | failed | 60 | 72 | +12 | run_failed,output_assertions_failed,trajectory_assertions_failed,scope_drift,repeated_action,invalid_action,repeated_actions | run_failed,output_assertions_failed,trajectory_assertions_failed,scope_drift,invalid_action |
| minimum-agent-memory-boundary | passed | passed | 100 | 100 | +0 | none | none |
| minimum-agent-verification-repair | failed | failed | 19 | 18 | -1 | run_failed,required_commands_failed,verification_failed,closeout_not_successful,runtime_spine_failed,output_assertions_failed,trajectory_assertions_failed,expected_code_diff_missing,scope_drift,repeated_action,invalid_action,runtime_spine_not_passing,tool_budget_exceeded,failed_actions,repeated_actions,llm_call_budget_pressure | run_failed,required_commands_failed,verification_failed,closeout_not_successful,runtime_spine_failed,output_assertions_failed,trajectory_assertions_failed,expected_code_diff_missing,scope_drift,repeated_action,invalid_action,runtime_spine_not_passing,tool_budget_exceeded,failed_actions,repeated_actions,llm_call_budget_pressure |

## Reading

- Baseline keeps safety gates on; it only disables or shadows weighted planning controls.
- Weighted is a product profile comparison, not a permission bypass.
- `inconclusive` means the average agent-score delta is within +/-2 points.
