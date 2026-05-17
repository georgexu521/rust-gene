# Live Eval Report: core-rollback-product-path

- Run id: `real-project-coding-20260517-153331`
- Sample: `evalsets/live_tasks/core-rollback-product-path.yaml`
- Worktree: `target/live-evals/real-project-coding-20260517-153331/core-rollback-product-path/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/real-project-coding-20260517-153331/core-rollback-product-path/env`
- Test status: `ok`
- Generated: `2026-05-17 15:56:42 +0800`

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
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 1438 filtered out; finished in 0.01s

[exit status: 0]

$ cargo test -q checkpoint -- --test-threads=1

running 30 tests
..............................
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 1414 filtered out; finished in 0.02s

[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-real-project-coding-20260517-153331/core-rollback-product-path/agent-output.md`
- Events: `docs/benchmarks/live-real-project-coding-20260517-153331/core-rollback-product-path/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 2
tool_execution_complete: 6
tool_execution_progress: 2
tool_execution_start: 6
trace_summary: 1
```

Quality signals:

```text
output_chars: 3023
diff_chars: 0
diff_files_changed: 0
tool_executions: 6
first_write_tool_index: none
forbidden_tool_uses: none
tool_errors: 0
tool_failures: 0
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 48
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=10194 tool_schema=3186 tools=15 workflow=guarded closeout=full validation=passed:2/2
adaptive_triggers: required_validation
trace_event_types: api.start,workflow.fallback,api.done,tool.start,tool.done,memory.sync,api.start,workflow.fallback,api.done,closeout,runtime.diet,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: audit_or_regression_check
behavior_assertions: none
behavior_assertion_status: none
warning: no_code_diff
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
memory_sync_events: 4
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
guided_reasoning_events: 1
workflow_plan_events: 1
weighted_plan_events: 1
reweighted_plan_events: 0
adaptive_trigger_events: 1
adaptive_triggers: required_validation
latest_top_priority: P3
latest_top_importance_score: 0.39500001072883606
latest_top_weight_share: 0.262458473443985
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=10194 tool_schema=3186 tools=15 workflow=guarded
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
