# Live Eval Report: persistent-memory-planning-context

- Run id: `live-eval-20260501-152616`
- Sample: `evalsets/live_tasks/persistent-memory-planning-context.yaml`
- Worktree: `target/live-evals/live-eval-20260501-152616/persistent-memory-planning-context/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/live-eval-20260501-152616/persistent-memory-planning-context/env`
- Test status: `failed`
- Generated: `2026-05-01 15:32:20 +0800`

## Git Status

```text
```

## Diff Stat

```text
```

## Required Commands

```text
$ cargo test -q learning_planning -- --test-threads=1

running 5 tests
.....
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 978 filtered out; finished in 0.01s

[exit status: 0]

$ cargo test -q retrieval_context -- --test-threads=1

running 7 tests
.......
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 976 filtered out; finished in 0.00s

[exit status: 0]

$ python3 -c "p='src/engine/conversation_loop/mod.rs'; s=open(p).read(); pre=s.find('prefetch_retrieval_context_with_llm_rerank'); apply=s.find('apply_learning_to_workflow_judgment'); assert pre >= 0 and apply >= 0 and pre < apply"
Traceback (most recent call last):
  File "<string>", line 1, in <module>
AssertionError
[exit status: 1]

$ cargo test -q -- --test-threads=1

running 983 tests
....................................................................................... 87/983
....................................................................................... 174/983
....................................................................................... 261/983
....................................................................................... 348/983
....................................................................................... 435/983
....................................................................................... 522/983
....................................................................................... 609/983
....................................................................................... 696/983
....................................................................................... 783/983
....................................................................................... 870/983
....................................................................................... 957/983
..........................
test result: ok. 983 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 44.23s

[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-live-eval-20260501-152616/persistent-memory-planning-context/agent-output.md`
- Events: `docs/benchmarks/live-live-eval-20260501-152616/persistent-memory-planning-context/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 2
tool_execution_complete: 14
tool_execution_start: 14
trace_summary: 1
```

Quality signals:

```text
output_chars: 944
diff_chars: 0
tool_executions: 14
tool_errors: 0
tool_failures: 2
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 94
test_status: failed
verification_passed: false
stage_validation_passed: false
acceptance_accepted: None
closeout_status: not_verified
trace_event_types: tool.start,tool.done,memory.sync,api.start,workflow.fallback,api.done,tool.start,tool.done,workflow.fallback,workflow.fallback,closeout,assistant
warning: no_code_diff
warning: required_commands_not_passing
warning: closeout_not_successful
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
