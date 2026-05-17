# Live Eval Report: core-rollback-product-path

- Run id: `core-quality-real-rerun-20260517-091952`
- Sample: `evalsets/live_tasks/core-rollback-product-path.yaml`
- Worktree: `target/live-evals/core-quality-real-rerun-20260517-091952/core-rollback-product-path/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/core-quality-real-rerun-20260517-091952/core-rollback-product-path/env`
- Test status: `ok`
- Generated: `2026-05-17 09:38:21 +0800`

## Git Status

```text
```

## Diff Stat

```text
```

## Required Commands

```text
$ cargo test -q rollback -- --test-threads=1

running 6 tests
......
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 1431 filtered out; finished in 0.01s

[exit status: 0]

$ cargo test -q checkpoint -- --test-threads=1

running 30 tests
..............................
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 1407 filtered out; finished in 0.02s

[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-core-quality-real-rerun-20260517-091952/core-rollback-product-path/agent-output.md`
- Events: `docs/benchmarks/live-core-quality-real-rerun-20260517-091952/core-rollback-product-path/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 1
tool_execution_complete: 19
tool_execution_progress: 2
tool_execution_start: 19
trace_summary: 1
```

Quality signals:

```text
output_chars: 413
diff_chars: 0
diff_files_changed: 0
tool_executions: 19
first_write_tool_index: none
forbidden_tool_uses: none
tool_errors: 0
tool_failures: 0
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 106
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=21072 tool_schema=3186 tools=15 workflow=guarded closeout=full validation=passed:2/2
adaptive_triggers: required_validation
trace_event_types: tool.start,tool.done,memory.sync,api.start,workflow.fallback,api.done,tool.start,tool.done,memory.sync,closeout,runtime.diet,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: audit_or_regression_check
warning: no_code_diff
failure_owner: none
```

Specialty signals:

```text
memory_active: true
automation_active: true
guided_debugging_active: false
guided_reasoning_active: false
weighted_planning_active: false
closeout_active: true
adaptive_workflow_active: true
active_specialty_signals: 4/7
memory_sync_events: 13
memory_tool_calls: 0
retrieval_sources: Project
required_commands: 2
agent_required_commands: 2
harness_commands: 0
required_command_status: ok
validation_events: 0
stage_validation_events: 0
tool_progress_events: 2
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
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=21072 tool_schema=3186 tools=15 workflow=guarded
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
