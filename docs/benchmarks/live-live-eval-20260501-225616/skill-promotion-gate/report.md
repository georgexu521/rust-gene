# Live Eval Report: skill-promotion-gate

- Run id: `live-eval-20260501-225616`
- Sample: `evalsets/live_tasks/skill-promotion-gate.yaml`
- Worktree: `target/live-evals/live-eval-20260501-225616/skill-promotion-gate/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/live-eval-20260501-225616/skill-promotion-gate/env`
- Test status: `ok`
- Generated: `2026-05-01 23:00:40 +0800`

## Git Status

```text
```

## Diff Stat

```text
```

## Required Commands

```text
$ cargo test -q skill_evolution -- --test-threads=1

running 9 tests
.........
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 940 filtered out; finished in 0.01s

[exit status: 0]

$ cargo test -q slash_handler -- --test-threads=1

running 36 tests
....................................
test result: ok. 36 passed; 0 failed; 0 ignored; 0 measured; 913 filtered out; finished in 0.09s

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
test result: ok. 949 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 43.13s

[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-live-eval-20260501-225616/skill-promotion-gate/agent-output.md`
- Events: `docs/benchmarks/live-live-eval-20260501-225616/skill-promotion-gate/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 2
tool_execution_complete: 9
tool_execution_start: 9
trace_summary: 1
```

Quality signals:

```text
output_chars: 901
diff_chars: 0
tool_executions: 9
tool_errors: 0
tool_failures: 0
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 58
test_status: ok
verification_passed: false
stage_validation_passed: false
acceptance_accepted: None
closeout_status: not_verified
trace_event_types: tool.done,memory.sync,api.start,workflow.fallback,api.done,tool.start,tool.done,workflow.fallback,workflow.fallback,workflow.fallback,closeout,assistant
warning: no_code_diff
warning: closeout_not_successful
```

Agent stderr tail:

```text
2026-05-01T14:58:00.749201Z  WARN priority_agent::engine::conversation_loop: Patch synthesis JSON actions were not directly applicable: patch synthesis declined without a reason; patch synthesis declined without a reason
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
