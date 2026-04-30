# Live Eval Report: memory-save-quality-gate

- Run id: `live-agent-statusdirect-20260430`
- Sample: `evalsets/live_tasks/memory-save-quality-gate.yaml`
- Worktree: `target/live-evals/live-agent-statusdirect-20260430/memory-save-quality-gate/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/live-agent-statusdirect-20260430/memory-save-quality-gate/env`
- Test status: `ok`
- Generated: `2026-04-30 13:50:33 +0800`

## Git Status

```text
 M src/memory/quality.rs
```

## Diff Stat

```text
 src/memory/quality.rs | 2 +-
 1 file changed, 1 insertion(+), 1 deletion(-)
```

## Required Commands

```text
$ cargo test -q memory -- --test-threads=1

running 75 tests
...........................................................................
test result: ok. 75 passed; 0 failed; 0 ignored; 0 measured; 874 filtered out; finished in 0.07s

[exit status: 0]

$ cargo test -q -- --test-threads=1

running 949 tests
....................................................................................... 87/949
....................................................................................... 174/949
....................................................................................... 261/949
....................................................................................... 348/949
....................................................................................... 435/949
....................................................................................... 522/949
....................................................................................... 609/949
....................................................................................... 696/949
....................................................................................... 783/949
....................................................................................... 870/949
...............................................................................
test result: ok. 949 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 18.81s

[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-live-agent-statusdirect-20260430/memory-save-quality-gate/agent-output.md`
- Events: `docs/benchmarks/live-live-agent-statusdirect-20260430/memory-save-quality-gate/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 1
tool_execution_complete: 11
tool_execution_progress: 1
tool_execution_start: 11
trace_summary: 1
```

Quality signals:

```text
output_chars: 2209
diff_chars: 689
tool_executions: 11
tool_errors: 0
tool_failures: 1
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 100
test_status: ok
verification_passed: false
stage_validation_passed: false
acceptance_accepted: False
closeout_status: failed
trace_event_types: api.start,workflow.fallback,api.done,tool.start,tool.done,verify.done,reflection.pass,stage.validation,guided.debug,acceptance.review,closeout,assistant
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
