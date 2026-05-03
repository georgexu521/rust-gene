# Live Eval Report: memory-save-duplicate-demotion

- Run id: `live-eval-20260502-112835`
- Sample: `evalsets/live_tasks/memory-save-duplicate-demotion.yaml`
- Worktree: `target/live-evals/live-eval-20260502-112835/memory-save-duplicate-demotion/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/live-eval-20260502-112835/memory-save-duplicate-demotion/env`
- Test status: `ok`
- Generated: `2026-05-02 11:43:10 +0800`

## Git Status

```text
 M src/memory/manager.rs
 M src/memory/quality.rs
```

## Diff Stat

```text
 src/memory/manager.rs | 64 +++++++++++++++++++++++++--------------------------
 src/memory/quality.rs |  2 +-
 2 files changed, 33 insertions(+), 33 deletions(-)
```

## Required Commands

```text
$ cargo test -q memory -- --test-threads=1

running 80 tests
................................................................................
test result: ok. 80 passed; 0 failed; 0 ignored; 0 measured; 911 filtered out; finished in 0.12s

[exit status: 0]

$ cargo test -q -- --test-threads=1

running 991 tests
....................................................................................... 87/991
....................................................................................... 174/991
....................................................................................... 261/991
....................................................................................... 348/991
....................................................................................... 435/991
....................................................................................... 522/991
....................................................................................... 609/991
....................................................................................... 696/991
....................................................................................... 783/991
....................................................................................... 870/991
....................................................................................... 957/991
..................................
test result: ok. 991 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 20.29s

[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-live-eval-20260502-112835/memory-save-duplicate-demotion/agent-output.md`
- Events: `docs/benchmarks/live-live-eval-20260502-112835/memory-save-duplicate-demotion/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 1
tool_execution_complete: 13
tool_execution_progress: 4
tool_execution_start: 13
trace_summary: 1
```

Quality signals:

```text
output_chars: 520
diff_chars: 4307
tool_executions: 13
tool_errors: 0
tool_failures: 1
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 84
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
trace_event_types: api.done,tool.start,tool.done,verify.done,reflection.pass,stage.validation,acceptance.review,workflow.plan,memory.sync,workflow.fallback,closeout,assistant
warning: earlier_verification_failed_before_repair
warning: earlier_stage_validation_failed_before_repair
```

Agent stderr tail:

```text
2026-05-02T03:31:06.155997Z  WARN priority_agent::tools::file_tool: File 'src/memory/manager.rs' was modified since it was read
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
