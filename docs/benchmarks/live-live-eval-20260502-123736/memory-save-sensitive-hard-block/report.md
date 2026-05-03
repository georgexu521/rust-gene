# Live Eval Report: memory-save-sensitive-hard-block

- Run id: `live-eval-20260502-123736`
- Sample: `evalsets/live_tasks/memory-save-sensitive-hard-block.yaml`
- Worktree: `target/live-evals/live-eval-20260502-123736/memory-save-sensitive-hard-block/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/live-eval-20260502-123736/memory-save-sensitive-hard-block/env`
- Test status: `ok`
- Generated: `2026-05-02 12:49:33 +0800`

## Git Status

```text
 M src/memory/quality.rs
 M src/tui/app.rs
```

## Diff Stat

```text
 src/memory/quality.rs | 13 +++++++++++++
 src/tui/app.rs        | 16 ++++++++++++++++
 2 files changed, 29 insertions(+)
```

## Required Commands

```text
$ cargo test -q memory -- --test-threads=1

running 82 tests
..................................................................................
test result: ok. 82 passed; 0 failed; 0 ignored; 0 measured; 911 filtered out; finished in 0.12s

[exit status: 0]

$ cargo test -q tui::app::tests:: -- --test-threads=1

running 35 tests
...................................
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 958 filtered out; finished in 0.23s

[exit status: 0]

$ cargo test -q -- --test-threads=1

running 993 tests
....................................................................................... 87/993
....................................................................................... 174/993
....................................................................................... 261/993
....................................................................................... 348/993
....................................................................................... 435/993
....................................................................................... 522/993
....................................................................................... 609/993
....................................................................................... 696/993
....................................................................................... 783/993
....................................................................................... 870/993
....................................................................................... 957/993
....................................
test result: ok. 993 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 18.43s

[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-live-eval-20260502-123736/memory-save-sensitive-hard-block/agent-output.md`
- Events: `docs/benchmarks/live-live-eval-20260502-123736/memory-save-sensitive-hard-block/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 1
tool_execution_complete: 16
tool_execution_progress: 3
tool_execution_start: 16
trace_summary: 1
```

Quality signals:

```text
output_chars: 500
diff_chars: 1643
tool_executions: 16
tool_errors: 0
tool_failures: 0
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 89
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
trace_event_types: api.done,tool.start,tool.done,verify.done,reflection.pass,stage.validation,acceptance.review,workflow.plan,memory.sync,workflow.fallback,closeout,assistant
warning: earlier_verification_failed_before_repair
warning: earlier_stage_validation_failed_before_repair
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
