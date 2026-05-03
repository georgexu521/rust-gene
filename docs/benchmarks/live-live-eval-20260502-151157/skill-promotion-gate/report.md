# Live Eval Report: skill-promotion-gate

- Run id: `live-eval-20260502-151157`
- Sample: `evalsets/live_tasks/skill-promotion-gate.yaml`
- Worktree: `target/live-evals/live-eval-20260502-151157/skill-promotion-gate/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/live-eval-20260502-151157/skill-promotion-gate/env`
- Test status: `ok`
- Generated: `2026-05-02 15:28:07 +0800`

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
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 996 filtered out; finished in 0.01s

[exit status: 0]

$ cargo test -q slash_handler -- --test-threads=1

running 36 tests
....................................
test result: ok. 36 passed; 0 failed; 0 ignored; 0 measured; 969 filtered out; finished in 0.11s

[exit status: 0]

$ python3 -c "import re; p='src/tui/slash_handler/config.rs'; s=open(p).read(); h=s.find('pub fn handle_skill_proposals'); a=s.find('\"apply\" =>', h); b=s.find('let root = user_skill_root()', a); m=re.search(r'validate_skill_promotion_for_apply\\(\\s*&store,\\s*&current,\\s*bound_report\\.as_ref\\(\\)\\s*\\)', s[a:b]); assert h >= 0 and a >= 0 and b >= 0 and m"
[exit status: 0]

$ python3 -c "p='src/tui/slash_handler/config.rs'; s=open(p).read(); h=s.find('pub fn handle_skill_proposals'); a=s.find('\"apply\" =>', h); r=s.find('store.record_applied_version(id, &path)', a); b=s.find('let loaded = app.skill_runtime.reload()', r); c=s.find('record_evolution_update(', r); assert h >= 0 and a >= 0 and r >= 0 and b >= 0 and c >= 0 and c < b"
[exit status: 0]

$ cargo test -q -- --test-threads=1

running 1005 tests
....................................................................................... 87/1005
....................................................................................... 174/1005
....................................................................................... 261/1005
....................................................................................... 348/1005
....................................................................................... 435/1005
....................................................................................... 522/1005
....................................................................................... 609/1005
....................................................................................... 696/1005
....................................................................................... 783/1005
....................................................................................... 870/1005
....................................................................................... 957/1005
................................................
test result: ok. 1005 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 24.99s

[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-live-eval-20260502-151157/skill-promotion-gate/agent-output.md`
- Events: `docs/benchmarks/live-live-eval-20260502-151157/skill-promotion-gate/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 1
tool_execution_complete: 12
tool_execution_progress: 3
tool_execution_start: 12
trace_summary: 1
```

Quality signals:

```text
output_chars: 583
diff_chars: 1232
tool_executions: 12
tool_errors: 0
tool_failures: 3
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 86
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
2026-05-02T07:14:21.978824Z  WARN priority_agent::tools::file_tool: File 'src/tui/slash_handler/config.rs' was modified since it was read
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
