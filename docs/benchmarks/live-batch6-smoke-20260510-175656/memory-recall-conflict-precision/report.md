# Live Eval Report: memory-recall-conflict-precision

- Run id: `batch6-smoke-20260510-175656`
- Sample: `evalsets/live_tasks/memory-recall-conflict-precision.yaml`
- Worktree: `target/live-evals/batch6-smoke-20260510-175656/memory-recall-conflict-precision/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/batch6-smoke-20260510-175656/memory-recall-conflict-precision/env`
- Test status: `ok`
- Generated: `2026-05-10 18:05:49 +0800`

## Git Status

```text
```

## Diff Stat

```text
```

## Required Commands

```text
$ cargo test -q retrieval_context -- --test-threads=1

running 9 tests
.........
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 1171 filtered out; finished in 0.01s

[exit status: 0]

$ cargo test -q memory::recall::tests:: -- --test-threads=1

running 1 test
.
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1179 filtered out; finished in 0.00s

[exit status: 0]

$ cargo test -q -- --test-threads=1

running 1180 tests
....................................................................................... 87/1180
....................................................................................... 174/1180
....................................................................................... 261/1180
....................................................................................... 348/1180
....................................................................................... 435/1180
....................................................................................... 522/1180
....................................................................................... 609/1180
....................................................................................... 696/1180
....................................................................................... 783/1180
....................................................................................... 870/1180
....................................................................................... 957/1180
....................................................................................... 1044/1180
....................................................................................... 1131/1180
.................................................
test result: ok. 1180 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 18.66s

[exit status: 0]

```

## Agent Run

- Exit status: `1`
- Output: `docs/benchmarks/live-batch6-smoke-20260510-175656/memory-recall-conflict-precision/agent-output.md`
- Events: `docs/benchmarks/live-batch6-smoke-20260510-175656/memory-recall-conflict-precision/agent-events.jsonl`

Event counts:

```text
error: 1
eval_started: 1
start: 1
tool_execution_complete: 9
tool_execution_progress: 1
tool_execution_start: 9
trace_summary: 1
```

Quality signals:

```text
output_chars: 0
diff_chars: 0
tool_executions: 9
first_write_tool_index: none
tool_errors: 0
tool_failures: 0
has_closeout: false
has_validation_claim: false
trace_status: Failed
trace_events: 64
test_status: ok
verification_passed: false
stage_validation_passed: false
acceptance_accepted: None
closeout_status: missing
runtime_diet: prompt=12133 tool_schema=2641 tools=12 workflow=strict closeout=none validation=api_error
adaptive_triggers: required_validation
trace_event_types: tool.done,memory.sync,api.start,workflow.fallback,api.done,tool.start,tool.done,memory.sync,api.start,workflow.fallback,error,runtime.diet
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: audit_or_regression_check
warning: empty_agent_output
warning: tool_run_without_closeout
warning: no_code_diff
warning: current_head_no_fixture_already_satisfied
warning: closeout_not_successful
warning: recovered_closeout_not_successful
failure_owner: environment
```

Specialty signals:

```text
memory_active: true
automation_active: true
guided_debugging_active: false
guided_reasoning_active: true
weighted_planning_active: true
closeout_active: false
adaptive_workflow_active: true
active_specialty_signals: 5/7
memory_sync_events: 7
memory_tool_calls: 0
retrieval_sources: Project,Session
required_commands: 3
required_command_status: ok
validation_events: 0
stage_validation_events: 0
tool_progress_events: 1
guided_debugging_events: 0
guided_reasoning_events: 1
workflow_plan_events: 1
weighted_plan_events: 1
reweighted_plan_events: 0
adaptive_trigger_events: 1
adaptive_triggers: required_validation
latest_top_priority: P2
latest_top_importance_score: 0.40625
latest_top_weight_share: 0.3152279555797577
acceptance_accepted: missing
closeout_status: missing
runtime_diet: prompt=12133 tool_schema=2641 tools=12 workflow=strict
note: guided debugging is expected only after a blocker or failed validation
```

Agent stderr tail:

```text
2026-05-10T10:00:10.380297Z ERROR priority_agent: Evaluation run failed: Failed to get response from MiniMax API: http error: error sending request for url (https://api.minimaxi.com/v1/chat/completions) (error body unavailable)
Evaluation run failed: Failed to get response from MiniMax API: http error: error sending request for url (https://api.minimaxi.com/v1/chat/completions) (error body unavailable)
```

## Human Review

- accepted: TODO
- task_success: TODO
- mainline_hit: TODO
- plan_coverage: TODO
- rework_count: TODO
- tool_efficiency: TODO
- diff_discipline: TODO
- closeout_accuracy: TODO
- notes: TODO
