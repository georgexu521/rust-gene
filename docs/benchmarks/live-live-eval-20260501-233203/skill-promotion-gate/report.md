# Live Eval Report: skill-promotion-gate

- Run id: `live-eval-20260501-233203`
- Sample: `evalsets/live_tasks/skill-promotion-gate.yaml`
- Worktree: `target/live-evals/live-eval-20260501-233203/skill-promotion-gate/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/live-eval-20260501-233203/skill-promotion-gate/env`
- Test status: `failed`
- Generated: `2026-05-01 23:47:23 +0800`

## Git Status

```text
 M src/tui/slash_handler/config.rs
```

## Diff Stat

```text
 src/tui/slash_handler/config.rs | 9 +++++++++
 1 file changed, 9 insertions(+)
```

## Required Commands

```text
$ cargo test -q skill_evolution -- --test-threads=1

running 9 tests
.........
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 982 filtered out; finished in 0.01s

[exit status: 0]

$ cargo test -q slash_handler -- --test-threads=1

running 36 tests
....................................
test result: ok. 36 passed; 0 failed; 0 ignored; 0 measured; 955 filtered out; finished in 0.08s

[exit status: 0]

$ python3 -c "p='src/tui/slash_handler/config.rs'; s=open(p).read(); a=s.find('\"apply\" =>'); b=s.find('let root = user_skill_root()', a); c=s.find('validate_skill_promotion_for_apply(&store, &current, bound_report.as_ref())', a); assert a >= 0 and b >= 0 and c >= 0 and c < b"
Traceback (most recent call last):
  File "<string>", line 1, in <module>
AssertionError
[exit status: 1]

$ python3 -c "p='src/tui/slash_handler/config.rs'; s=open(p).read(); a=s.find('store.record_applied_version(id, &path)'); b=s.find('let loaded = app.skill_runtime.reload()', a); c=s.find('record_evolution_update(', a); assert a >= 0 and b >= 0 and c >= 0 and c < b"
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
test result: ok. 991 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 20.83s

[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-live-eval-20260501-233203/skill-promotion-gate/agent-output.md`
- Events: `docs/benchmarks/live-live-eval-20260501-233203/skill-promotion-gate/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 2
tool_execution_complete: 20
tool_execution_progress: 5
tool_execution_start: 20
trace_summary: 1
```

Quality signals:

```text
output_chars: 1653
diff_chars: 1232
tool_executions: 20
tool_errors: 1
tool_failures: 1
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 103
test_status: failed
verification_passed: false
stage_validation_passed: false
acceptance_accepted: False
closeout_status: failed
trace_event_types: workflow.fallback,memory.sync,api.start,workflow.fallback,api.done,tool.start,tool.done,workflow.fallback,workflow.fallback,workflow.fallback,closeout,assistant
warning: tool_errors_seen
warning: earlier_verification_failed_before_repair
warning: earlier_stage_validation_failed_before_repair
warning: required_commands_not_passing
warning: closeout_not_successful
warning: acceptance_review_rejected
warning: stage_validation_failed
warning: verification_failed
```

Agent stderr tail:

```text
2026-05-01T15:34:44.921765Z  WARN priority_agent::tools::file_tool: File 'src/tui/slash_handler/config.rs' was modified since it was read
2026-05-01T15:46:32.028625Z  WARN priority_agent::engine::conversation_loop: Patch synthesis JSON actions were not directly applicable: synthesized patch old_string was not found exactly in /Users/georgexu/Desktop/rust-agent/target/live-evals/live-eval-20260501-233203/skill-promotion-gate/worktree/src/tui/slash_handler/config.rs; refusing inexact multi-line replacement
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
