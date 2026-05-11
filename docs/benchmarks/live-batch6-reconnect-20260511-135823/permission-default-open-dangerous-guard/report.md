# Live Eval Report: permission-default-open-dangerous-guard

- Run id: `batch6-reconnect-20260511-135823`
- Sample: `evalsets/live_tasks/permission-default-open-dangerous-guard.yaml`
- Worktree: `target/live-evals/batch6-reconnect-20260511-135823/permission-default-open-dangerous-guard/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/batch6-reconnect-20260511-135823/permission-default-open-dangerous-guard/env`
- Test status: `ok`
- Generated: `2026-05-11 14:06:01 +0800`

## Git Status

```text
```

## Diff Stat

```text
```

## Required Commands

```text
$ cargo test -q permissions -- --test-threads=1

running 46 tests
..............................................
test result: ok. 46 passed; 0 failed; 0 ignored; 0 measured; 1149 filtered out; finished in 0.01s

[exit status: 0]

$ cargo test -q tools::bash_tool -- --test-threads=1

running 19 tests
...................
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 1176 filtered out; finished in 0.02s

[exit status: 0]

$ cargo test -q -- --test-threads=1

running 1195 tests
....................................................................................... 87/1195
....................................................................................... 174/1195
....................................................................................... 261/1195
....................................................................................... 348/1195
....................................................................................... 435/1195
....................................................................................... 522/1195
....................................................................................... 609/1195
....................................................................................... 696/1195
....................................................................................... 783/1195
....................................................................................... 870/1195
....................................................................................... 957/1195
....................................................................................... 1044/1195
....................................................................................... 1131/1195
................................................................
test result: ok. 1195 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 46.42s

[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-batch6-reconnect-20260511-135823/permission-default-open-dangerous-guard/agent-output.md`
- Events: `docs/benchmarks/live-batch6-reconnect-20260511-135823/permission-default-open-dangerous-guard/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 1
tool_execution_complete: 21
tool_execution_progress: 3
tool_execution_start: 21
trace_summary: 1
```

Quality signals:

```text
output_chars: 485
diff_chars: 0
tool_executions: 21
first_write_tool_index: none
tool_errors: 0
tool_failures: 0
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 111
test_status: ok
verification_passed: false
stage_validation_passed: false
acceptance_accepted: None
closeout_status: passed
runtime_diet: prompt=15577 tool_schema=2641 tools=12 workflow=strict closeout=full validation=passed:3/3
adaptive_triggers: required_validation
trace_event_types: memory.sync,api.start,workflow.fallback,api.done,tool.start,tool.done,tool.start,tool.done,memory.sync,closeout,runtime.diet,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: audit_or_regression_check
warning: no_code_diff
warning: current_head_no_fixture_already_satisfied
failure_owner: none
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
memory_sync_events: 13
memory_tool_calls: 0
retrieval_sources: Project
required_commands: 3
required_command_status: ok
validation_events: 0
stage_validation_events: 0
tool_progress_events: 3
guided_debugging_events: 0
guided_reasoning_events: 1
workflow_plan_events: 1
weighted_plan_events: 1
reweighted_plan_events: 0
adaptive_trigger_events: 1
adaptive_triggers: required_validation
latest_top_priority: P1
latest_top_importance_score: 0.6000000238418579
latest_top_weight_share: 0.25695931911468506
acceptance_accepted: missing
closeout_status: passed
runtime_diet: prompt=15577 tool_schema=2641 tools=12 workflow=strict
note: guided debugging is expected only after a blocker or failed validation
```

Agent stderr tail:

```text
2026-05-11T05:58:51.405157Z  WARN priority_agent::services::api::retry: Provider request failed transiently; reconnecting 1/5 for MiniMax chat.completions after 692ms: http error: error sending request for url (https://api.minimaxi.com/v1/chat/completions)
2026-05-11T06:02:27.139571Z  WARN priority_agent::services::api::retry: Provider request failed transiently; reconnecting 1/5 for MiniMax chat.completions after 675ms: http error: error sending request for url (https://api.minimaxi.com/v1/chat/completions)
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
