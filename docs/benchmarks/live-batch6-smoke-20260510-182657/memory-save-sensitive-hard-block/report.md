# Live Eval Report: memory-save-sensitive-hard-block

- Run id: `batch6-smoke-20260510-182657`
- Sample: `evalsets/live_tasks/memory-save-sensitive-hard-block.yaml`
- Worktree: `target/live-evals/batch6-smoke-20260510-182657/memory-save-sensitive-hard-block/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/batch6-smoke-20260510-182657/memory-save-sensitive-hard-block/env`
- Test status: `ok`
- Generated: `2026-05-10 18:34:28 +0800`

## Git Status

```text
```

## Diff Stat

```text
```

## Required Commands

```text
$ cargo test -q memory -- --test-threads=1

running 94 tests
....................................................................................... 87/94
.......
test result: ok. 94 passed; 0 failed; 0 ignored; 0 measured; 1087 filtered out; finished in 0.17s

[exit status: 0]

$ cargo test -q tui::app::tests:: -- --test-threads=1

running 38 tests
......................................
test result: ok. 38 passed; 0 failed; 0 ignored; 0 measured; 1143 filtered out; finished in 0.13s

[exit status: 0]

$ cargo test -q -- --test-threads=1

running 1181 tests
....................................................................................... 87/1181
....................................................................................... 174/1181
....................................................................................... 261/1181
....................................................................................... 348/1181
....................................................................................... 435/1181
....................................................................................... 522/1181
....................................................................................... 609/1181
....................................................................................... 696/1181
....................................................................................... 783/1181
....................................................................................... 870/1181
....................................................................................... 957/1181
....................................................................................... 1044/1181
....................................................................................... 1131/1181
..................................................
test result: ok. 1181 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 18.03s

[exit status: 0]

```

## Agent Run

- Exit status: `1`
- Output: `docs/benchmarks/live-batch6-smoke-20260510-182657/memory-save-sensitive-hard-block/agent-output.md`
- Events: `docs/benchmarks/live-batch6-smoke-20260510-182657/memory-save-sensitive-hard-block/agent-events.jsonl`

Event counts:

```text
error: 1
eval_started: 1
start: 1
tool_execution_complete: 14
tool_execution_progress: 6
tool_execution_start: 14
trace_summary: 1
```

Quality signals:

```text
output_chars: 0
diff_chars: 0
tool_executions: 14
first_write_tool_index: none
tool_errors: 0
tool_failures: 0
has_closeout: false
has_validation_claim: false
trace_status: Failed
trace_events: 69
test_status: ok
verification_passed: false
stage_validation_passed: false
acceptance_accepted: None
closeout_status: missing
runtime_diet: prompt=18286 tool_schema=753 tools=4 workflow=none closeout=none validation=api_error
adaptive_triggers: required_validation
trace_event_types: api.start,workflow.fallback,api.done,tool.start,tool.start,tool.done,tool.done,memory.sync,api.start,workflow.fallback,error,runtime.diet
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
failure_owner: environment
```

Specialty signals:

```text
memory_active: true
automation_active: true
guided_debugging_active: false
guided_reasoning_active: false
weighted_planning_active: false
closeout_active: false
adaptive_workflow_active: true
active_specialty_signals: 3/7
memory_sync_events: 7
memory_tool_calls: 0
retrieval_sources: none
required_commands: 3
required_command_status: ok
validation_events: 0
stage_validation_events: 0
tool_progress_events: 6
guided_debugging_events: 0
guided_reasoning_events: 0
workflow_plan_events: 0
weighted_plan_events: 0
reweighted_plan_events: 0
adaptive_trigger_events: 1
adaptive_triggers: required_validation
latest_top_priority: none
latest_top_importance_score: none
latest_top_weight_share: none
acceptance_accepted: missing
closeout_status: missing
runtime_diet: prompt=18286 tool_schema=753 tools=4 workflow=none
note: guided debugging is expected only after a blocker or failed validation
```

Agent stderr tail:

```text
2026-05-10T10:29:35.974093Z ERROR priority_agent: Evaluation run failed: Failed to get response from MiniMax API: http error: error sending request for url (https://api.minimaxi.com/v1/chat/completions) (error body unavailable)
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
