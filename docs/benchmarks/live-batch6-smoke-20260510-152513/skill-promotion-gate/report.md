# Live Eval Report: skill-promotion-gate

- Run id: `batch6-smoke-20260510-152513`
- Sample: `evalsets/live_tasks/skill-promotion-gate.yaml`
- Worktree: `target/live-evals/batch6-smoke-20260510-152513/skill-promotion-gate/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/batch6-smoke-20260510-152513/skill-promotion-gate/env`
- Test status: `failed`
- Generated: `2026-05-10 15:30:28 +0800`

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
    --> src/tui/slash_handler/learning.rs:1480:5
     |
1477 | struct BoundSkillEvalReport {
     |        -------------------- fields in this struct
...
1480 |     total: usize,
     |     ^^^^^
1481 |     passed: usize,
     |     ^^^^^^
1482 |     failed: usize,
     |     ^^^^^^
     |
     = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: function `validate_skill_promotion_for_apply` is never used
    --> src/tui/slash_handler/learning.rs:1528:4
     |
1528 | fn validate_skill_promotion_for_apply(
     |    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: function `skill_fitness_from_bound_eval` is never used
    --> src/tui/slash_handler/learning.rs:1601:4
     |
1601 | fn skill_fitness_from_bound_eval(
     |    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: function `estimate_skill_semantic_drift` is never used
    --> src/tui/slash_handler/learning.rs:1631:4
     |
1631 | fn estimate_skill_semantic_drift(proposal: &crate::engine::skill_evolution::SkillProposal) -> f32 {
     |    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^


running 9 tests
.........
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 1169 filtered out; finished in 0.01s

[exit status: 0]

$ cargo test -q slash_handler -- --test-threads=1
warning: fields `total`, `passed`, and `failed` are never read
    --> src/tui/slash_handler/learning.rs:1480:5
     |
1477 | struct BoundSkillEvalReport {
     |        -------------------- fields in this struct
...
1480 |     total: usize,
     |     ^^^^^
1481 |     passed: usize,
     |     ^^^^^^
1482 |     failed: usize,
     |     ^^^^^^
     |
     = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: function `validate_skill_promotion_for_apply` is never used
    --> src/tui/slash_handler/learning.rs:1528:4
     |
1528 | fn validate_skill_promotion_for_apply(
     |    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: function `skill_fitness_from_bound_eval` is never used
    --> src/tui/slash_handler/learning.rs:1601:4
     |
1601 | fn skill_fitness_from_bound_eval(
     |    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: function `estimate_skill_semantic_drift` is never used
    --> src/tui/slash_handler/learning.rs:1631:4
     |
1631 | fn estimate_skill_semantic_drift(proposal: &crate::engine::skill_evolution::SkillProposal) -> f32 {
     |    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^


running 42 tests
..........................................
test result: ok. 42 passed; 0 failed; 0 ignored; 0 measured; 1136 filtered out; finished in 0.12s

[exit status: 0]

$ python3 -c "import re; p='src/tui/slash_handler/learning.rs'; s=open(p).read(); h=s.find('pub fn handle_skill_proposals'); a=s.find('\"apply\" =>', h); b=s.find('let root = user_skill_root()', a); m=re.search(r'validate_skill_promotion_for_apply\\(\\s*&store,\\s*&current,\\s*bound_report\\.as_ref\\(\\)\\s*\\)', s[a:b]); assert h >= 0 and a >= 0 and b >= 0 and m"
Traceback (most recent call last):
  File "<string>", line 1, in <module>
AssertionError
[exit status: 1]

$ python3 -c "p='src/tui/slash_handler/learning.rs'; s=open(p).read(); h=s.find('pub fn handle_skill_proposals'); a=s.find('\"apply\" =>', h); r=s.find('store.record_applied_version(id, &path)', a); b=s.find('let loaded = app.skill_runtime.reload()', r); c=s.find('record_evolution_update(', r); assert h >= 0 and a >= 0 and r >= 0 and b >= 0 and c >= 0 and c < b"
Traceback (most recent call last):
  File "<string>", line 1, in <module>
AssertionError
[exit status: 1]

$ cargo test -q -- --test-threads=1
warning: fields `total`, `passed`, and `failed` are never read
    --> src/tui/slash_handler/learning.rs:1480:5
     |
1477 | struct BoundSkillEvalReport {
     |        -------------------- fields in this struct
...
1480 |     total: usize,
     |     ^^^^^
1481 |     passed: usize,
     |     ^^^^^^
1482 |     failed: usize,
     |     ^^^^^^
     |
     = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: function `validate_skill_promotion_for_apply` is never used
    --> src/tui/slash_handler/learning.rs:1528:4
     |
1528 | fn validate_skill_promotion_for_apply(
     |    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: function `skill_fitness_from_bound_eval` is never used
    --> src/tui/slash_handler/learning.rs:1601:4
     |
1601 | fn skill_fitness_from_bound_eval(
     |    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: function `estimate_skill_semantic_drift` is never used
    --> src/tui/slash_handler/learning.rs:1631:4
     |
1631 | fn estimate_skill_semantic_drift(proposal: &crate::engine::skill_evolution::SkillProposal) -> f32 {
     |    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^


running 1178 tests
....................................................................................... 87/1178
....................................................................................... 174/1178
....................................................................................... 261/1178
....................................................................................... 348/1178
....................................................................................... 435/1178
....................................................................................... 522/1178
....................................................................................... 609/1178
....................................................................................... 696/1178
....................................................................................... 783/1178
....................................................................................... 870/1178
....................................................................................... 957/1178
....................................................................................... 1044/1178
....................................................................................... 1131/1178
...............................................
test result: ok. 1178 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 60.80s

[exit status: 0]

```

## Agent Run

- Exit status: `1`
- Output: `docs/benchmarks/live-batch6-smoke-20260510-152513/skill-promotion-gate/agent-output.md`
- Events: `docs/benchmarks/live-batch6-smoke-20260510-152513/skill-promotion-gate/agent-events.jsonl`

Event counts:

```text
error: 1
eval_started: 1
start: 1
trace_summary: 1
```

Quality signals:

```text
output_chars: 0
diff_chars: 0
tool_executions: 0
first_write_tool_index: none
tool_errors: 0
tool_failures: 0
has_closeout: false
has_validation_claim: false
trace_status: Failed
trace_events: 17
test_status: failed
verification_passed: false
stage_validation_passed: false
acceptance_accepted: None
closeout_status: missing
runtime_diet: prompt=2879 tool_schema=2641 tools=12 workflow=guarded closeout=none validation=api_error
adaptive_triggers: required_validation
trace_event_types: workflow.fallback,workflow.judgment,workflow.plan,task.context,implementation.intent,reflection.pass,goal,workflow.route,api.start,workflow.fallback,error,runtime.diet
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: seeded_code_change
warning: empty_agent_output
warning: no_code_diff
warning: required_commands_not_passing
warning: closeout_not_successful
failure_owner: agent_flow
```

Specialty signals:

```text
memory_active: false
automation_active: true
guided_debugging_active: false
guided_reasoning_active: true
weighted_planning_active: true
closeout_active: false
adaptive_workflow_active: true
active_specialty_signals: 4/7
memory_sync_events: 0
memory_tool_calls: 0
retrieval_sources: Project,Session
required_commands: 5
required_command_status: failed
validation_events: 0
stage_validation_events: 0
tool_progress_events: 0
guided_debugging_events: 0
guided_reasoning_events: 1
workflow_plan_events: 1
weighted_plan_events: 1
reweighted_plan_events: 0
adaptive_trigger_events: 1
adaptive_triggers: required_validation
latest_top_priority: P3
latest_top_importance_score: 0.23499999940395355
latest_top_weight_share: 0.1666666567325592
acceptance_accepted: missing
closeout_status: missing
runtime_diet: prompt=2879 tool_schema=2641 tools=12 workflow=guarded
attention: required commands did not pass in the harness
note: guided debugging is expected only after a blocker or failed validation
```

Agent stderr tail:

```text
2026-05-10T07:27:26.854245Z ERROR priority_agent: Evaluation run failed: Failed to get response from MiniMax API
Evaluation run failed: Failed to get response from MiniMax API
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
