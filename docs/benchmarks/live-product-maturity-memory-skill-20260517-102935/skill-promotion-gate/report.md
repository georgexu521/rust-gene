# Live Eval Report: skill-promotion-gate

- Run id: `product-maturity-memory-skill-20260517-102935`
- Sample: `evalsets/live_tasks/skill-promotion-gate.yaml`
- Worktree: `target/live-evals/product-maturity-memory-skill-20260517-102935/skill-promotion-gate/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/product-maturity-memory-skill-20260517-102935/skill-promotion-gate/env`
- Test status: `ok`
- Generated: `2026-05-17 10:44:35 +0800`

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
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 1428 filtered out; finished in 0.01s

[exit status: 0]

$ cargo test -q slash_handler -- --test-threads=1

running 44 tests
............................................
test result: ok. 44 passed; 0 failed; 0 ignored; 0 measured; 1393 filtered out; finished in 0.10s

[exit status: 0]

$ python3 -c "import re; p='src/tui/slash_handler/learning.rs'; s=open(p).read(); h=s.find('pub fn handle_skill_proposals'); a=s.find('\"apply\" =>', h); b=s.find('let root = user_skill_root()', a); m=re.search(r'validate_skill_promotion_for_apply\\(\\s*&store,\\s*&current,\\s*bound_report\\.as_ref\\(\\)\\s*\\)', s[a:b]); assert h >= 0 and a >= 0 and b >= 0 and m"
[exit status: 0]

$ python3 -c "p='src/tui/slash_handler/learning.rs'; s=open(p).read(); h=s.find('pub fn handle_skill_proposals'); a=s.find('\"apply\" =>', h); r=s.find('store.record_applied_version(id, &path)', a); b=s.find('let loaded = app.skill_runtime.reload()', r); c=s.find('record_evolution_update(', r); assert h >= 0 and a >= 0 and r >= 0 and b >= 0 and c >= 0 and c < b"
[exit status: 0]

$ cargo test -q -- --test-threads=1

running 1437 tests
....................................................................................... 87/1437
....................................................................................... 174/1437
....................................................................................... 261/1437
....................................................................................... 348/1437
....................................................................................... 435/1437
....................................................................................... 522/1437
....................................................................................... 609/1437
....................................................................................... 696/1437
....................................................................................... 783/1437
....................................................................................... 870/1437
....................................................................................... 957/1437
....................................................................................... 1044/1437
....................................................................................... 1131/1437
....................................................................................... 1218/1437
....................................................................................... 1305/1437
....................................................................................... 1392/1437
.............................................
test result: ok. 1437 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 21.85s

[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-product-maturity-memory-skill-20260517-102935/skill-promotion-gate/agent-output.md`
- Events: `docs/benchmarks/live-product-maturity-memory-skill-20260517-102935/skill-promotion-gate/agent-events.jsonl`

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
output_chars: 1192
diff_chars: 1248
diff_files_changed: 1
tool_executions: 11
first_write_tool_index: 10
forbidden_tool_uses: none
tool_errors: 0
tool_failures: 2
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 97
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=17610 tool_schema=3186 tools=15 workflow=strict closeout=full validation=failed:1/9
adaptive_triggers: required_validation,repeated_no_code_progress,first_code_change,verification_failed,acceptance_rejected
trace_event_types: tool.start,tool.done,verify.done,reflection.pass,stage.validation,acceptance.review,workflow.plan,memory.sync,workflow.fallback,closeout,runtime.diet,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: seeded_code_change
behavior_assertions: skill_promotion_gate,skill_evolution_cooldown
behavior_assertion_status: passed
warning: earlier_verification_failed_before_repair
warning: earlier_stage_validation_failed_before_repair
failure_owner: none
```

Specialty signals:

```text
memory_active: true
automation_active: true
guided_debugging_active: true
guided_reasoning_active: true
weighted_planning_active: true
closeout_active: true
adaptive_workflow_active: true
active_specialty_signals: 7/7
memory_sync_events: 6
memory_tool_calls: 0
retrieval_sources: Project,Session
required_commands: 5
agent_required_commands: 5
harness_commands: 0
required_command_status: ok
validation_events: 2
stage_validation_events: 2
tool_progress_events: 2
guided_debugging_events: 1
guided_reasoning_events: 1
workflow_plan_events: 2
weighted_plan_events: 1
reweighted_plan_events: 1
adaptive_trigger_events: 5
adaptive_triggers: required_validation,repeated_no_code_progress,first_code_change,verification_failed,acceptance_rejected
latest_top_priority: P3
latest_top_importance_score: 0.3199999928474426
latest_top_weight_share: 0.3106796145439148
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=17610 tool_schema=3186 tools=15 workflow=strict
```

Agent stderr tail:

```text
[required validation still running after 30s] cargo test -q skill_evolution -- --test-threads=1
[required validation still running after 60s] cargo test -q skill_evolution -- --test-threads=1
[required validation still running after 90s] cargo test -q skill_evolution -- --test-threads=1
2026-05-17T02:43:13.162875Z  WARN priority_agent::engine::conversation_loop::patch_recovery: Patch synthesis JSON actions were not directly applicable: synthesized patch old_string was not found exactly in /Users/georgexu/Desktop/rust-agent/target/live-evals/product-maturity-memory-skill-20260517-102935/skill-promotion-gate/worktree/src/tui/slash_handler/learning.rs; refusing inexact multi-line replacement; patch synthesis declined: Need exact current file content. The evidence showed lines 1290-1304 but the actual file may differ. Will re-read to get precise old_string.
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
