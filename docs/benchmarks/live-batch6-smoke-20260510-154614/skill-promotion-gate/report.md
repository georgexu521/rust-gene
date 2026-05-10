# Live Eval Report: skill-promotion-gate

- Run id: `batch6-smoke-20260510-154614`
- Sample: `evalsets/live_tasks/skill-promotion-gate.yaml`
- Worktree: `target/live-evals/batch6-smoke-20260510-154614/skill-promotion-gate/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/batch6-smoke-20260510-154614/skill-promotion-gate/env`
- Test status: `ok`
- Generated: `2026-05-10 15:53:55 +0800`

## Git Status

```text
 M src/tui/slash_handler/learning.rs
```

## Diff Stat

```text
 src/tui/slash_handler/learning.rs | 9 +++++++++
 1 file changed, 9 insertions(+)
```

## Required Commands

```text
$ cargo test -q skill_evolution -- --test-threads=1

running 9 tests
.........
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 1169 filtered out; finished in 0.01s

[exit status: 0]

$ cargo test -q slash_handler -- --test-threads=1

running 42 tests
..........................................
test result: ok. 42 passed; 0 failed; 0 ignored; 0 measured; 1136 filtered out; finished in 0.09s

[exit status: 0]

$ python3 -c "import re; p='src/tui/slash_handler/learning.rs'; s=open(p).read(); h=s.find('pub fn handle_skill_proposals'); a=s.find('\"apply\" =>', h); b=s.find('let root = user_skill_root()', a); m=re.search(r'validate_skill_promotion_for_apply\\(\\s*&store,\\s*&current,\\s*bound_report\\.as_ref\\(\\)\\s*\\)', s[a:b]); assert h >= 0 and a >= 0 and b >= 0 and m"
[exit status: 0]

$ python3 -c "p='src/tui/slash_handler/learning.rs'; s=open(p).read(); h=s.find('pub fn handle_skill_proposals'); a=s.find('\"apply\" =>', h); r=s.find('store.record_applied_version(id, &path)', a); b=s.find('let loaded = app.skill_runtime.reload()', r); c=s.find('record_evolution_update(', r); assert h >= 0 and a >= 0 and r >= 0 and b >= 0 and c >= 0 and c < b"
[exit status: 0]

$ cargo test -q -- --test-threads=1

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
test result: ok. 1178 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 21.05s

[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-batch6-smoke-20260510-154614/skill-promotion-gate/agent-output.md`
- Events: `docs/benchmarks/live-batch6-smoke-20260510-154614/skill-promotion-gate/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 1
tool_execution_complete: 11
tool_execution_progress: 3
tool_execution_start: 11
trace_summary: 1
```

Quality signals:

```text
output_chars: 617
diff_chars: 1240
tool_executions: 11
first_write_tool_index: 11
tool_errors: 0
tool_failures: 2
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 88
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: None
closeout_status: passed
runtime_diet: prompt=20478 tool_schema=2641 tools=12 workflow=guarded closeout=full validation=passed:8/8
adaptive_triggers: required_validation,repeated_no_code_progress,first_code_change
trace_event_types: tool.start,tool.done,workflow.trigger,workflow.fallback,verify.done,reflection.pass,stage.validation,memory.sync,workflow.fallback,closeout,runtime.diet,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: seeded_code_change
failure_owner: none
```

Specialty signals:

```text
memory_active: true
automation_active: true
guided_debugging_active: false
guided_reasoning_active: false
weighted_planning_active: false
closeout_active: false
adaptive_workflow_active: true
active_specialty_signals: 3/7
memory_sync_events: 6
memory_tool_calls: 0
retrieval_sources: Project,Session
required_commands: 5
required_command_status: ok
validation_events: 1
stage_validation_events: 1
tool_progress_events: 3
guided_debugging_events: 0
guided_reasoning_events: 0
workflow_plan_events: 0
weighted_plan_events: 0
reweighted_plan_events: 0
adaptive_trigger_events: 3
adaptive_triggers: required_validation,repeated_no_code_progress,first_code_change
latest_top_priority: none
latest_top_importance_score: none
latest_top_weight_share: none
acceptance_accepted: missing
closeout_status: passed
runtime_diet: prompt=20478 tool_schema=2641 tools=12 workflow=guarded
note: guided debugging is expected only after a blocker or failed validation
```

Agent stderr tail:

```text
[required validation still running after 30s] cargo test -q skill_evolution -- --test-threads=1
[required validation still running after 60s] cargo test -q skill_evolution -- --test-threads=1
[required validation still running after 90s] cargo test -q skill_evolution -- --test-threads=1
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
