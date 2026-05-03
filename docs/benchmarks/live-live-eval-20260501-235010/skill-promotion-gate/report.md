# Live Eval Report: skill-promotion-gate

- Run id: `live-eval-20260501-235010`
- Sample: `evalsets/live_tasks/skill-promotion-gate.yaml`
- Worktree: `target/live-evals/live-eval-20260501-235010/skill-promotion-gate/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/live-eval-20260501-235010/skill-promotion-gate/env`
- Test status: `failed`
- Generated: `2026-05-01 23:53:38 +0800`

## Git Status

```text
```

## Diff Stat

```text
```

## Required Commands

```text
$ cargo test -q skill_evolution -- --test-threads=1
warning: fields `total`, `passed`, and `failed` are never read
    --> src/tui/slash_handler/config.rs:3893:5
     |
3890 | struct BoundSkillEvalReport {
     |        -------------------- fields in this struct
...
3893 |     total: usize,
     |     ^^^^^
3894 |     passed: usize,
     |     ^^^^^^
3895 |     failed: usize,
     |     ^^^^^^
     |
     = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: function `validate_skill_promotion_for_apply` is never used
    --> src/tui/slash_handler/config.rs:3941:4
     |
3941 | fn validate_skill_promotion_for_apply(
     |    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: function `skill_fitness_from_bound_eval` is never used
    --> src/tui/slash_handler/config.rs:4014:4
     |
4014 | fn skill_fitness_from_bound_eval(
     |    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: function `estimate_skill_semantic_drift` is never used
    --> src/tui/slash_handler/config.rs:4044:4
     |
4044 | fn estimate_skill_semantic_drift(proposal: &crate::engine::skill_evolution::SkillProposal) -> f32 {
     |    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^


running 9 tests
.........
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 982 filtered out; finished in 0.01s

[exit status: 0]

$ cargo test -q slash_handler -- --test-threads=1
warning: fields `total`, `passed`, and `failed` are never read
    --> src/tui/slash_handler/config.rs:3893:5
     |
3890 | struct BoundSkillEvalReport {
     |        -------------------- fields in this struct
...
3893 |     total: usize,
     |     ^^^^^
3894 |     passed: usize,
     |     ^^^^^^
3895 |     failed: usize,
     |     ^^^^^^
     |
     = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: function `validate_skill_promotion_for_apply` is never used
    --> src/tui/slash_handler/config.rs:3941:4
     |
3941 | fn validate_skill_promotion_for_apply(
     |    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: function `skill_fitness_from_bound_eval` is never used
    --> src/tui/slash_handler/config.rs:4014:4
     |
4014 | fn skill_fitness_from_bound_eval(
     |    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: function `estimate_skill_semantic_drift` is never used
    --> src/tui/slash_handler/config.rs:4044:4
     |
4044 | fn estimate_skill_semantic_drift(proposal: &crate::engine::skill_evolution::SkillProposal) -> f32 {
     |    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^


running 36 tests
....................................
test result: ok. 36 passed; 0 failed; 0 ignored; 0 measured; 955 filtered out; finished in 0.09s

[exit status: 0]

$ python3 -c "import re; p='src/tui/slash_handler/config.rs'; s=open(p).read(); a=s.find('\"apply\" =>'); b=s.find('let root = user_skill_root()', a); m=re.search(r'validate_skill_promotion_for_apply\\(\\s*&store,\\s*&current,\\s*bound_report\\.as_ref\\(\\)\\s*\\)', s[a:b]); assert a >= 0 and b >= 0 and m"
Traceback (most recent call last):
  File "<string>", line 1, in <module>
AssertionError
[exit status: 1]

$ python3 -c "p='src/tui/slash_handler/config.rs'; s=open(p).read(); a=s.find('store.record_applied_version(id, &path)'); b=s.find('let loaded = app.skill_runtime.reload()', a); c=s.find('record_evolution_update(', a); assert a >= 0 and b >= 0 and c >= 0 and c < b"
Traceback (most recent call last):
  File "<string>", line 1, in <module>
AssertionError
[exit status: 1]

$ cargo test -q -- --test-threads=1
warning: fields `total`, `passed`, and `failed` are never read
    --> src/tui/slash_handler/config.rs:3893:5
     |
3890 | struct BoundSkillEvalReport {
     |        -------------------- fields in this struct
...
3893 |     total: usize,
     |     ^^^^^
3894 |     passed: usize,
     |     ^^^^^^
3895 |     failed: usize,
     |     ^^^^^^
     |
     = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: function `validate_skill_promotion_for_apply` is never used
    --> src/tui/slash_handler/config.rs:3941:4
     |
3941 | fn validate_skill_promotion_for_apply(
     |    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: function `skill_fitness_from_bound_eval` is never used
    --> src/tui/slash_handler/config.rs:4014:4
     |
4014 | fn skill_fitness_from_bound_eval(
     |    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: function `estimate_skill_semantic_drift` is never used
    --> src/tui/slash_handler/config.rs:4044:4
     |
4044 | fn estimate_skill_semantic_drift(proposal: &crate::engine::skill_evolution::SkillProposal) -> f32 {
     |    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^


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
test result: ok. 991 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 46.04s

[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-live-eval-20260501-235010/skill-promotion-gate/agent-output.md`
- Events: `docs/benchmarks/live-live-eval-20260501-235010/skill-promotion-gate/agent-events.jsonl`

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
output_chars: 456
diff_chars: 0
tool_executions: 9
tool_errors: 0
tool_failures: 1
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 58
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

Agent stderr tail:

```text
2026-05-01T15:50:27.415818Z  WARN priority_agent::engine::conversation_loop: Workflow judgment analysis failed: workflow judgment response did not contain JSON
2026-05-01T15:51:27.172568Z  WARN priority_agent::engine::conversation_loop: Patch synthesis JSON actions were not directly applicable: response was not valid patch JSON; response was not valid patch JSON
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
