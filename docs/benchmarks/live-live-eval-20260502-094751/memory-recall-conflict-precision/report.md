# Live Eval Report: memory-recall-conflict-precision

- Run id: `live-eval-20260502-094751`
- Sample: `evalsets/live_tasks/memory-recall-conflict-precision.yaml`
- Worktree: `target/live-evals/live-eval-20260502-094751/memory-recall-conflict-precision/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/live-eval-20260502-094751/memory-recall-conflict-precision/env`
- Test status: `failed`
- Generated: `2026-05-02 10:09:46 +0800`

## Git Status

```text
 M src/engine/retrieval_context.rs
```

## Diff Stat

```text
 src/engine/retrieval_context.rs | 46 ++++++++++++++++++++++++++++++++++++-----
 1 file changed, 41 insertions(+), 5 deletions(-)
```

## Required Commands

```text
$ cargo test -q retrieval_context -- --test-threads=1

running 7 tests
.......
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 984 filtered out; finished in 0.01s

[exit status: 0]

$ {'cargo test -q memory::recall::tests:': '-- --test-threads=1'}
bash: {cargo test -q memory::recall::tests::: command not found
[exit status: 127]

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
test result: ok. 991 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 19.96s

[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-live-eval-20260502-094751/memory-recall-conflict-precision/agent-output.md`
- Events: `docs/benchmarks/live-live-eval-20260502-094751/memory-recall-conflict-precision/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 1
tool_execution_complete: 14
tool_execution_progress: 5
tool_execution_start: 14
trace_summary: 1
```

Quality signals:

```text
output_chars: 2314
diff_chars: 2530
tool_executions: 14
tool_errors: 0
tool_failures: 1
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 107
test_status: failed
verification_passed: false
stage_validation_passed: false
acceptance_accepted: False
closeout_status: failed
trace_event_types: api.start,workflow.fallback,api.done,tool.start,tool.done,verify.done,reflection.pass,stage.validation,guided.debug,acceptance.review,closeout,assistant
warning: earlier_verification_failed_before_repair
warning: earlier_stage_validation_failed_before_repair
warning: required_commands_not_passing
warning: closeout_not_successful
warning: acceptance_review_rejected
warning: stage_validation_failed
warning: verification_failed
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
