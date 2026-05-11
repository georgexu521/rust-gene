# Live Eval Report: memory-recall-conflict-precision

- Run id: `batch6-reconnect-20260511-132912`
- Sample: `evalsets/live_tasks/memory-recall-conflict-precision.yaml`
- Worktree: `target/live-evals/batch6-reconnect-20260511-132912/memory-recall-conflict-precision/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/batch6-reconnect-20260511-132912/memory-recall-conflict-precision/env`
- Test status: `ok`
- Generated: `2026-05-11 13:38:07 +0800`

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
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 1186 filtered out; finished in 0.01s

[exit status: 0]

$ cargo test -q memory::recall::tests:: -- --test-threads=1

running 1 test
.
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1194 filtered out; finished in 0.00s

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
test result: ok. 1195 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 47.14s

[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-batch6-reconnect-20260511-132912/memory-recall-conflict-precision/agent-output.md`
- Events: `docs/benchmarks/live-batch6-reconnect-20260511-132912/memory-recall-conflict-precision/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 1
tool_execution_complete: 12
tool_execution_progress: 3
tool_execution_start: 12
trace_summary: 1
```

Quality signals:

```text
output_chars: 1610
diff_chars: 0
tool_executions: 12
first_write_tool_index: none
tool_errors: 0
tool_failures: 0
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 72
test_status: ok
verification_passed: false
stage_validation_passed: false
acceptance_accepted: None
closeout_status: passed
runtime_diet: prompt=18005 tool_schema=2641 tools=12 workflow=strict closeout=full validation=passed:3/3
adaptive_triggers: required_validation
trace_event_types: api.start,workflow.fallback,api.done,tool.start,tool.done,memory.sync,api.start,workflow.fallback,api.done,closeout,runtime.diet,assistant
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
memory_sync_events: 7
memory_tool_calls: 0
retrieval_sources: Project,Session
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
latest_top_importance_score: 0.6474999785423279
latest_top_weight_share: 0.1905812919139862
acceptance_accepted: missing
closeout_status: passed
runtime_diet: prompt=18005 tool_schema=2641 tools=12 workflow=strict
note: guided debugging is expected only after a blocker or failed validation
```

Agent stderr tail:

```text
2026-05-11T05:30:53.264224Z  WARN priority_agent::services::api::retry: Provider request failed transiently; reconnecting 1/5 for MiniMax chat.completions after 551ms: http error: error sending request for url (https://api.minimaxi.com/v1/chat/completions)
2026-05-11T05:30:56.821413Z  WARN priority_agent::services::api::retry: Provider request failed transiently; reconnecting 2/5 for MiniMax chat.completions after 1.145s: http error: error sending request for url (https://api.minimaxi.com/v1/chat/completions)
2026-05-11T05:34:30.615611Z  WARN priority_agent::services::api::retry: Provider request failed transiently; reconnecting 1/5 for MiniMax chat.completions after 651ms: http error: error sending request for url (https://api.minimaxi.com/v1/chat/completions)
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
