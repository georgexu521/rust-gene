# Live Eval Report: memory-save-duplicate-demotion

- Run id: `product-maturity-memory-skill-rerun-20260517-124722`
- Sample: `evalsets/live_tasks/memory-save-duplicate-demotion.yaml`
- Worktree: `target/live-evals/product-maturity-memory-skill-rerun-20260517-124722/memory-save-duplicate-demotion/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/product-maturity-memory-skill-rerun-20260517-124722/memory-save-duplicate-demotion/env`
- Test status: `ok`
- Generated: `2026-05-17 13:32:30 +0800`

## Git Status

```text
```

## Diff Stat

```text
```

## Required Commands

```text
$ cargo test -q memory -- --test-threads=1

running 104 tests
....................................................................................... 87/104
.................
test result: ok. 104 passed; 0 failed; 0 ignored; 0 measured; 1336 filtered out; finished in 0.20s

[exit status: 0]

$ cargo test -q -- --test-threads=1

running 1440 tests
....................................................................................... 87/1440
....................................................................................... 174/1440
....................................................................................... 261/1440
....................................................................................... 348/1440
....................................................................................... 435/1440
....................................................................................... 522/1440
....................................................................................... 609/1440
....................................................................................... 696/1440
....................................................................................... 783/1440
....................................................................................... 870/1440
....................................................................................... 957/1440
....................................................................................... 1044/1440
....................................................................................... 1131/1440
....................................................................................... 1218/1440
....................................................................................... 1305/1440
....................................................................................... 1392/1440
................................................
test result: ok. 1440 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 21.19s

[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-product-maturity-memory-skill-rerun-20260517-124722/memory-save-duplicate-demotion/agent-output.md`
- Events: `docs/benchmarks/live-product-maturity-memory-skill-rerun-20260517-124722/memory-save-duplicate-demotion/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 1
tool_execution_complete: 23
tool_execution_progress: 3
tool_execution_start: 23
trace_summary: 1
```

Quality signals:

```text
output_chars: 485
diff_chars: 0
diff_files_changed: 0
tool_executions: 23
first_write_tool_index: none
forbidden_tool_uses: none
tool_errors: 0
tool_failures: 0
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 115
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=29638 tool_schema=3186 tools=15 workflow=guarded closeout=full validation=passed:2/2
adaptive_triggers: required_validation
trace_event_types: tool.start,tool.done,memory.sync,api.start,workflow.fallback,api.done,tool.start,tool.done,memory.sync,closeout,runtime.diet,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: audit_or_regression_check
behavior_assertions: memory_duplicate_demotion,memory_namespace_precision
behavior_assertion_status: passed
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
closeout_active: true
adaptive_workflow_active: true
active_specialty_signals: 6/7
memory_sync_events: 13
memory_tool_calls: 0
retrieval_sources: Project,Session
required_commands: 2
agent_required_commands: 2
harness_commands: 0
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
latest_top_priority: P3
latest_top_importance_score: 0.21000000834465027
latest_top_weight_share: 0.20895522832870483
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=29638 tool_schema=3186 tools=15 workflow=guarded
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
