# Live Eval Report: memory-recall-conflict-precision

- Run id: `capability-memory-conflict-visible-errors-20260503-204111`
- Sample: `evalsets/live_tasks/memory-recall-conflict-precision.yaml`
- Worktree: `target/live-evals/capability-memory-conflict-visible-errors-20260503-204111/memory-recall-conflict-precision/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/capability-memory-conflict-visible-errors-20260503-204111/memory-recall-conflict-precision/env`
- Test status: `ok`
- Generated: `2026-05-03 20:46:28 +0800`

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
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 1048 filtered out; finished in 0.01s

[exit status: 0]

$ cargo test -q memory::recall::tests:: -- --test-threads=1

running 1 test
.
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1056 filtered out; finished in 0.00s

[exit status: 0]

$ cargo test -q -- --test-threads=1

running 1057 tests
....................................................................................... 87/1057
....................................................................................... 174/1057
....................................................................................... 261/1057
....................................................................................... 348/1057
....................................................................................... 435/1057
....................................................................................... 522/1057
....................................................................................... 609/1057
....................................................................................... 696/1057
....................................................................................... 783/1057
....................................................................................... 870/1057
....................................................................................... 957/1057
....................................................................................... 1044/1057
.............
test result: ok. 1057 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 44.02s

[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-capability-memory-conflict-visible-errors-20260503-204111/memory-recall-conflict-precision/agent-output.md`
- Events: `docs/benchmarks/live-capability-memory-conflict-visible-errors-20260503-204111/memory-recall-conflict-precision/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 2
tool_execution_complete: 13
tool_execution_start: 13
trace_summary: 1
```

Quality signals:

```text
output_chars: 1178
diff_chars: 0
tool_executions: 13
first_write_tool_index: none
tool_errors: 0
tool_failures: 1
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 78
test_status: ok
verification_passed: false
stage_validation_passed: false
acceptance_accepted: None
closeout_status: not_verified
trace_event_types: api.start,workflow.fallback,api.done,tool.start,tool.start,tool.done,tool.done,workflow.fallback,workflow.fallback,workflow.fallback,closeout,assistant
stale_edit_warnings: 0
warning: no_code_diff
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
active_specialty_signals: 4/6
memory_sync_events: 6
memory_tool_calls: 0
retrieval_sources: Project,Session
required_commands: 3
required_command_status: ok
validation_events: 0
stage_validation_events: 0
tool_progress_events: 0
guided_debugging_events: 0
guided_reasoning_events: 1
workflow_plan_events: 1
weighted_plan_events: 1
reweighted_plan_events: 0
latest_top_priority: P0
latest_top_importance_score: 0.9249999523162842
latest_top_weight_share: 0.21816037595272064
acceptance_accepted: missing
closeout_status: not_verified
note: guided debugging is expected only after a blocker or failed validation
```

## Human Review

- accepted: false
- task_success: false
- mainline_hit: partial
- plan_coverage: partial
- rework_count: 0
- tool_efficiency: poor
- diff_discipline: neutral
- closeout_accuracy: accurate
- notes: Patch synthesis remained disabled and the run ended without a code
  diff. The important new finding is eval staleness: the unchanged baseline
  already contains the generic-token conflict guard and related tests, and all
  required commands pass. The quality gate correctly rejected the run because
  this is a code-change task with no diff, but this case is no longer a clean
  measure of model editing ability on the current branch.
