# Live Eval Report: skill-promotion-gate

- Run id: `real-project-coding-20260517-192347`
- Sample: `evalsets/live_tasks/skill-promotion-gate.yaml`
- Worktree: `target/live-evals/real-project-coding-20260517-192347/skill-promotion-gate/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/real-project-coding-20260517-192347/skill-promotion-gate/env`
- Test status: `ok`
- Generated: `2026-05-17 19:57:01 +0800`

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
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 1441 filtered out; finished in 0.01s

[exit status: 0]

$ cargo test -q slash_handler -- --test-threads=1

running 44 tests
............................................
test result: ok. 44 passed; 0 failed; 0 ignored; 0 measured; 1406 filtered out; finished in 0.11s

[exit status: 0]

$ python3 -c "import re; p='src/tui/slash_handler/learning.rs'; s=open(p).read(); h=s.find('pub fn handle_skill_proposals'); a=s.find('\"apply\" =>', h); b=s.find('let root = user_skill_root()', a); m=re.search(r'validate_skill_promotion_for_apply\\(\\s*&store,\\s*&current,\\s*bound_report\\.as_ref\\(\\)\\s*\\)', s[a:b]); assert h >= 0 and a >= 0 and b >= 0 and m"
[exit status: 0]

$ python3 -c "p='src/tui/slash_handler/learning.rs'; s=open(p).read(); h=s.find('pub fn handle_skill_proposals'); a=s.find('\"apply\" =>', h); r=s.find('store.record_applied_version(id, &path)', a); b=s.find('let loaded = app.skill_runtime.reload()', r); c=s.find('record_evolution_update(', r); assert h >= 0 and a >= 0 and r >= 0 and b >= 0 and c >= 0 and c < b"
[exit status: 0]

$ cargo test -q -- --test-threads=1

running 1450 tests
....................................................................................... 87/1450
....................................................................................... 174/1450
....................................................................................... 261/1450
....................................................................................... 348/1450
....................................................................................... 435/1450
....................................................................................... 522/1450
....................................................................................... 609/1450
....................................................................................... 696/1450
....................................................................................... 783/1450
....................................................................................... 870/1450
....................................................................................... 957/1450
....................................................................................... 1044/1450
....................................................................................... 1131/1450
....................................................................................... 1218/1450
....................................................................................... 1305/1450
....................................................................................... 1392/1450
..........................................................
test result: ok. 1450 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 21.77s

[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-real-project-coding-20260517-192347/skill-promotion-gate/agent-output.md`
- Events: `docs/benchmarks/live-real-project-coding-20260517-192347/skill-promotion-gate/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 1
tool_execution_complete: 11
tool_execution_progress: 2
tool_execution_start: 11
trace_summary: 1
```

Quality signals:

```text
output_chars: 1071
diff_chars: 1240
diff_files_changed: 1
tool_executions: 11
first_write_tool_index: 10
forbidden_tool_uses: none
tool_errors: 0
tool_failures: 1
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 81
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=22502 tool_schema=3186 tools=15 workflow=guarded closeout=full validation=passed:5/5
adaptive_triggers: required_validation,repeated_no_code_progress,first_code_change
trace_event_types: workflow.trigger,workflow.fallback,verify.done,reflection.pass,stage.validation,acceptance.review,workflow.plan,memory.sync,workflow.fallback,closeout,runtime.diet,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: seeded_code_change
behavior_assertions: skill_promotion_gate,skill_evolution_cooldown
behavior_assertion_status: passed
failure_owner: none
```

Specialty signals:

```text
memory_active: true
automation_active: true
guided_debugging_active: false
guided_reasoning_active: true
weighted_planning_active: true
closeout_active: true
adaptive_workflow_active: true
active_specialty_signals: 6/7
memory_sync_events: 5
memory_tool_calls: 0
retrieval_sources: Project,Session
required_commands: 5
agent_required_commands: 5
harness_commands: 0
required_command_status: ok
validation_events: 1
stage_validation_events: 1
tool_progress_events: 2
guided_debugging_events: 0
guided_reasoning_events: 1
workflow_plan_events: 2
weighted_plan_events: 1
reweighted_plan_events: 1
adaptive_trigger_events: 3
adaptive_triggers: required_validation,repeated_no_code_progress,first_code_change
latest_top_priority: P0
latest_top_importance_score: 0.9900000095367432
latest_top_weight_share: 0.2395644187927246
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=22502 tool_schema=3186 tools=15 workflow=guarded
note: guided debugging is expected only after a blocker or failed validation
```

Agent stderr tail:

```text
[required validation still running after 30s] cargo test -q skill_evolution -- --test-threads=1
[required validation still running after 60s] cargo test -q skill_evolution -- --test-threads=1
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
