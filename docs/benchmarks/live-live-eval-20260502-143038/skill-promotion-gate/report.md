# Live Eval Report: skill-promotion-gate

- Run id: `live-eval-20260502-143038`
- Sample: `evalsets/live_tasks/skill-promotion-gate.yaml`
- Worktree: `target/live-evals/live-eval-20260502-143038/skill-promotion-gate/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/live-eval-20260502-143038/skill-promotion-gate/env`
- Test status: `ok`
- Generated: `2026-05-02 15:02:53 +0800`

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
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 995 filtered out; finished in 0.01s

[exit status: 0]

$ cargo test -q slash_handler -- --test-threads=1

running 36 tests
....................................
test result: ok. 36 passed; 0 failed; 0 ignored; 0 measured; 968 filtered out; finished in 0.11s

[exit status: 0]

$ python3 -c "import re; p='src/tui/slash_handler/config.rs'; s=open(p).read(); h=s.find('pub fn handle_skill_proposals'); a=s.find('\"apply\" =>', h); b=s.find('let root = user_skill_root()', a); m=re.search(r'validate_skill_promotion_for_apply\\(\\s*&store,\\s*&current,\\s*bound_report\\.as_ref\\(\\)\\s*\\)', s[a:b]); assert h >= 0 and a >= 0 and b >= 0 and m"
[exit status: 0]

$ python3 -c "p='src/tui/slash_handler/config.rs'; s=open(p).read(); h=s.find('pub fn handle_skill_proposals'); a=s.find('\"apply\" =>', h); r=s.find('store.record_applied_version(id, &path)', a); b=s.find('let loaded = app.skill_runtime.reload()', r); c=s.find('record_evolution_update(', r); assert h >= 0 and a >= 0 and r >= 0 and b >= 0 and c >= 0 and c < b"
[exit status: 0]

$ cargo test -q -- --test-threads=1

running 1004 tests
....................................................................................... 87/1004
....................................................................................... 174/1004
....................................................................................... 261/1004
....................................................................................... 348/1004
....................................................................................... 435/1004
....................................................................................... 522/1004
....................................................................................... 609/1004
....................................................................................... 696/1004
....................................................................................... 783/1004
....................................................................................... 870/1004
....................................................................................... 957/1004
...............................................
test result: ok. 1004 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 24.45s

[exit status: 0]

```

## Agent Run

- Exit status: `124`
- Events: `docs/benchmarks/live-live-eval-20260502-143038/skill-promotion-gate/agent-events.jsonl`

Event counts:

```text
eval_started: 1
start: 1
tool_execution_complete: 17
tool_execution_progress: 7
tool_execution_start: 17
```

Quality signals:

```text
output_chars: 0
diff_chars: 1232
tool_executions: 17
tool_errors: 1
tool_failures: 0
has_closeout: false
has_validation_claim: false
trace_status: missing
trace_events: 0
test_status: ok
verification_passed: false
stage_validation_passed: false
acceptance_accepted: None
closeout_status: missing
warning: empty_agent_output
warning: tool_run_without_closeout
warning: tool_errors_seen
warning: missing_trace_summary
warning: closeout_not_successful
```

Agent stderr tail:

```text
2026-05-02T06:31:35.685806Z  WARN priority_agent::tools::file_tool: File 'src/tui/slash_handler/config.rs' was modified since it was read
2026-05-02T06:59:38.205484Z  WARN priority_agent::tools::bash_tool: Command timed out after 180s, killing process tree (pid: Some(28780))

[timeout after 1800s]
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
