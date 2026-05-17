# Live Eval Report: memory-save-duplicate-demotion

- Run id: `product-maturity-memory-skill-20260517-102935`
- Sample: `evalsets/live_tasks/memory-save-duplicate-demotion.yaml`
- Worktree: `target/live-evals/product-maturity-memory-skill-20260517-102935/memory-save-duplicate-demotion/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/product-maturity-memory-skill-20260517-102935/memory-save-duplicate-demotion/env`
- Test status: `ok`
- Generated: `2026-05-17 11:03:49 +0800`

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
test result: ok. 104 passed; 0 failed; 0 ignored; 0 measured; 1333 filtered out; finished in 0.20s

[exit status: 0]

$ cargo test -q -- --test-threads=1

running 1437 tests
....................................................................................... 87/1437
....................................................................................... 174/1437
....................................................................................... 261/1437
....................................................................................... 348/1437
....................................................................................... 435/1437
....................................................................................... 522/1437
....................................................................................... 609/1437
....................................................................................... 696/1437
....................................................................................... 783/1437
....................................................................................... 870/1437
....................................................................................... 957/1437
....................................................................................... 1044/1437
....................................................................................... 1131/1437
....................................................................................... 1218/1437
....................................................................................... 1305/1437
....................................................................................... 1392/1437
.............................................
test result: ok. 1437 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 20.74s

[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-product-maturity-memory-skill-20260517-102935/memory-save-duplicate-demotion/agent-output.md`
- Events: `docs/benchmarks/live-product-maturity-memory-skill-20260517-102935/memory-save-duplicate-demotion/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 1
tool_execution_complete: 21
tool_execution_progress: 2
tool_execution_start: 21
trace_summary: 1
```

Quality signals:

```text
output_chars: 485
diff_chars: 0
diff_files_changed: 0
tool_executions: 21
first_write_tool_index: none
forbidden_tool_uses: none
tool_errors: 0
tool_failures: 0
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 111
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=18754 tool_schema=3186 tools=15 workflow=guarded closeout=full validation=passed:2/2
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
guided_reasoning_active: false
weighted_planning_active: true
closeout_active: true
adaptive_workflow_active: true
active_specialty_signals: 5/7
memory_sync_events: 13
memory_tool_calls: 0
retrieval_sources: Project,Session
required_commands: 2
agent_required_commands: 2
harness_commands: 0
required_command_status: ok
validation_events: 0
stage_validation_events: 0
tool_progress_events: 2
guided_debugging_events: 0
guided_reasoning_events: 0
workflow_plan_events: 1
weighted_plan_events: 1
reweighted_plan_events: 0
adaptive_trigger_events: 1
adaptive_triggers: required_validation
latest_top_priority: P3
latest_top_importance_score: 0.05000000074505806
latest_top_weight_share: 0.25
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=18754 tool_schema=3186 tools=15 workflow=guarded
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
