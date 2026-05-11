# Live Eval Report: memory-recall-conflict-precision

- Run id: `batch6-provider-gated-20260511-101147`
- Sample: `evalsets/live_tasks/memory-recall-conflict-precision.yaml`
- Worktree: `target/live-evals/batch6-provider-gated-20260511-101147/memory-recall-conflict-precision/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/batch6-provider-gated-20260511-101147/memory-recall-conflict-precision/env`
- Test status: `ok`
- Generated: `2026-05-11 10:19:39 +0800`

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
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 1178 filtered out; finished in 0.01s

[exit status: 0]

$ cargo test -q memory::recall::tests:: -- --test-threads=1

running 1 test
.
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1186 filtered out; finished in 0.00s

[exit status: 0]

$ cargo test -q -- --test-threads=1

running 1187 tests
....................................................................................... 87/1187
....................................................................................... 174/1187
....................................................................................... 261/1187
....................................................................................... 348/1187
....................................................................................... 435/1187
....................................................................................... 522/1187
....................................................................................... 609/1187
....................................................................................... 696/1187
....................................................................................... 783/1187
....................................................................................... 870/1187
....................................................................................... 957/1187
....................................................................................... 1044/1187
....................................................................................... 1131/1187
........................................................
test result: ok. 1187 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 22.24s

[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-batch6-provider-gated-20260511-101147/memory-recall-conflict-precision/agent-output.md`
- Events: `docs/benchmarks/live-batch6-provider-gated-20260511-101147/memory-recall-conflict-precision/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 2
tool_execution_complete: 10
tool_execution_progress: 3
tool_execution_start: 10
trace_summary: 1
```

Quality signals:

```text
output_chars: 3489
diff_chars: 0
tool_executions: 10
first_write_tool_index: none
tool_errors: 0
tool_failures: 0
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 60
test_status: ok
verification_passed: false
stage_validation_passed: false
acceptance_accepted: None
closeout_status: not_verified
runtime_diet: prompt=15515 tool_schema=2641 tools=12 workflow=strict closeout=full validation=passed:3/3
adaptive_triggers: required_validation
trace_event_types: api.done,tool.start,tool.done,tool.start,tool.done,memory.sync,api.start,workflow.fallback,api.done,closeout,runtime.diet,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: audit_or_regression_check
warning: no_code_diff
warning: current_head_no_fixture_already_satisfied
warning: closeout_not_successful
failure_owner: agent_flow
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
memory_sync_events: 5
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
latest_top_priority: P0
latest_top_importance_score: 0.8799999952316284
latest_top_weight_share: 0.23311257362365723
acceptance_accepted: missing
closeout_status: not_verified
runtime_diet: prompt=15515 tool_schema=2641 tools=12 workflow=strict
note: guided debugging is expected only after a blocker or failed validation
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
