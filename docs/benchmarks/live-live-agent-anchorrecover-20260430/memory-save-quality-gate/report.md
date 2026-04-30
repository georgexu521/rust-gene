# Live Eval Report: memory-save-quality-gate

- Run id: `live-agent-anchorrecover-20260430`
- Sample: `evalsets/live_tasks/memory-save-quality-gate.yaml`
- Worktree: `target/live-evals/live-agent-anchorrecover-20260430/memory-save-quality-gate/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/live-agent-anchorrecover-20260430/memory-save-quality-gate/env`
- Test status: `failed`
- Generated: `2026-04-30 11:50:06 +0800`

## Git Status

```text
 M src/memory/quality.rs
```

## Diff Stat

```text
 src/memory/quality.rs | 4 +---
 1 file changed, 1 insertion(+), 3 deletions(-)
```

## Required Commands

```text
$ cargo test -q memory -- --test-threads=1
error[E0425]: cannot find value `write_decision` in this scope
   --> src/memory/quality.rs:178:14
    |
178 | let status = write_decision.status;
    |              ^^^^^^^^^^^^^^ not found in this scope

error[E0425]: cannot find value `write_decision` in this scope
   --> src/memory/quality.rs:182:9
    |
182 |         write_decision.reason
    |         ^^^^^^^^^^^^^^ not found in this scope

error[E0425]: cannot find value `score` in this scope
   --> src/memory/quality.rs:194:9
    |
194 |         score,
    |         ^^^^^ not found in this scope

warning: unused import: `score_memory_write`
 --> src/memory/quality.rs:3:40
  |
3 |     memory_write_factors_from_signals, score_memory_write, MemoryWriteFactors,
  |                                        ^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

For more information about this error, try `rustc --explain E0425`.
error: could not compile `priority-agent` (bin "priority-agent" test) due to 3 previous errors; 1 warning emitted
[exit status: 101]

$ cargo test -q -- --test-threads=1
error[E0425]: cannot find value `write_decision` in this scope
   --> src/memory/quality.rs:178:14
    |
178 | let status = write_decision.status;
    |              ^^^^^^^^^^^^^^ not found in this scope

error[E0425]: cannot find value `write_decision` in this scope
   --> src/memory/quality.rs:182:9
    |
182 |         write_decision.reason
    |         ^^^^^^^^^^^^^^ not found in this scope

error[E0425]: cannot find value `score` in this scope
   --> src/memory/quality.rs:194:9
    |
194 |         score,
    |         ^^^^^ not found in this scope

warning: unused import: `score_memory_write`
 --> src/memory/quality.rs:3:40
  |
3 |     memory_write_factors_from_signals, score_memory_write, MemoryWriteFactors,
  |                                        ^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

For more information about this error, try `rustc --explain E0425`.
error: could not compile `priority-agent` (bin "priority-agent" test) due to 3 previous errors; 1 warning emitted
[exit status: 101]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-live-agent-anchorrecover-20260430/memory-save-quality-gate/agent-output.md`
- Events: `docs/benchmarks/live-live-agent-anchorrecover-20260430/memory-save-quality-gate/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 1
tool_execution_complete: 7
tool_execution_progress: 3
tool_execution_start: 7
trace_summary: 1
```

Quality signals:

```text
output_chars: 1763
diff_chars: 726
tool_executions: 7
tool_errors: 1
tool_failures: 3
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 82
test_status: failed
verification_passed: false
stage_validation_passed: false
acceptance_accepted: False
closeout_status: failed
trace_event_types: api.start,workflow.fallback,api.done,tool.start,tool.done,verify.done,reflection.pass,stage.validation,guided.debug,acceptance.review,closeout,assistant
warning: tool_errors_seen
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
