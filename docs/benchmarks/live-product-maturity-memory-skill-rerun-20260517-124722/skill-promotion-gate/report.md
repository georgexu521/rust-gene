# Live Eval Report: skill-promotion-gate

- Run id: `product-maturity-memory-skill-rerun-20260517-124722`
- Sample: `evalsets/live_tasks/skill-promotion-gate.yaml`
- Worktree: `target/live-evals/product-maturity-memory-skill-rerun-20260517-124722/skill-promotion-gate/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/product-maturity-memory-skill-rerun-20260517-124722/skill-promotion-gate/env`
- Test status: `failed`
- Generated: `2026-05-17 13:04:53 +0800`

## Git Status

```text
 M src/tui/slash_handler/learning.rs
```

## Diff Stat

```text
 src/tui/slash_handler/learning.rs | 47 +++++++++++++++++++++++++++++++++++++++
 1 file changed, 47 insertions(+)
```

## Required Commands

```text
$ cargo test -q skill_evolution -- --test-threads=1
error[E0609]: no field `reason` on type `SkillPromotionGate`
    --> src/tui/slash_handler/learning.rs:1297:44
     |
1297 | ...                   promotion_gate.reason,
     |                                      ^^^^^^ unknown field
     |
help: a field with a similar name exists
     |
1297 |                             promotion_gate.reasons,
     |                                                  +

error[E0609]: no field `fitness_delta` on type `SkillPromotionGate`
    --> src/tui/slash_handler/learning.rs:1298:44
     |
1298 | ...                   promotion_gate.fitness_delta
     |                                      ^^^^^^^^^^^^^ unknown field
     |
     = note: available fields are: `passed`, `old_fitness`, `new_fitness`, `delta`, `regression_rate` ... and 4 others

For more information about this error, try `rustc --explain E0609`.
error: could not compile `priority-agent` (bin "priority-agent" test) due to 2 previous errors
[exit status: 101]

$ cargo test -q slash_handler -- --test-threads=1
error[E0609]: no field `reason` on type `SkillPromotionGate`
    --> src/tui/slash_handler/learning.rs:1297:44
     |
1297 | ...                   promotion_gate.reason,
     |                                      ^^^^^^ unknown field
     |
help: a field with a similar name exists
     |
1297 |                             promotion_gate.reasons,
     |                                                  +

error[E0609]: no field `fitness_delta` on type `SkillPromotionGate`
    --> src/tui/slash_handler/learning.rs:1298:44
     |
1298 | ...                   promotion_gate.fitness_delta
     |                                      ^^^^^^^^^^^^^ unknown field
     |
     = note: available fields are: `passed`, `old_fitness`, `new_fitness`, `delta`, `regression_rate` ... and 4 others

For more information about this error, try `rustc --explain E0609`.
error: could not compile `priority-agent` (bin "priority-agent" test) due to 2 previous errors
[exit status: 101]

$ python3 -c "import re; p='src/tui/slash_handler/learning.rs'; s=open(p).read(); h=s.find('pub fn handle_skill_proposals'); a=s.find('\"apply\" =>', h); b=s.find('let root = user_skill_root()', a); m=re.search(r'validate_skill_promotion_for_apply\\(\\s*&store,\\s*&current,\\s*bound_report\\.as_ref\\(\\)\\s*\\)', s[a:b]); assert h >= 0 and a >= 0 and b >= 0 and m"
[exit status: 0]

$ python3 -c "p='src/tui/slash_handler/learning.rs'; s=open(p).read(); h=s.find('pub fn handle_skill_proposals'); a=s.find('\"apply\" =>', h); r=s.find('store.record_applied_version(id, &path)', a); b=s.find('let loaded = app.skill_runtime.reload()', r); c=s.find('record_evolution_update(', r); assert h >= 0 and a >= 0 and r >= 0 and b >= 0 and c >= 0 and c < b"
[exit status: 0]

$ cargo test -q -- --test-threads=1
error[E0609]: no field `reason` on type `SkillPromotionGate`
    --> src/tui/slash_handler/learning.rs:1297:44
     |
1297 | ...                   promotion_gate.reason,
     |                                      ^^^^^^ unknown field
     |
help: a field with a similar name exists
     |
1297 |                             promotion_gate.reasons,
     |                                                  +

error[E0609]: no field `fitness_delta` on type `SkillPromotionGate`
    --> src/tui/slash_handler/learning.rs:1298:44
     |
1298 | ...                   promotion_gate.fitness_delta
     |                                      ^^^^^^^^^^^^^ unknown field
     |
     = note: available fields are: `passed`, `old_fitness`, `new_fitness`, `delta`, `regression_rate` ... and 4 others

For more information about this error, try `rustc --explain E0609`.
error: could not compile `priority-agent` (bin "priority-agent" test) due to 2 previous errors
[exit status: 101]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-product-maturity-memory-skill-rerun-20260517-124722/skill-promotion-gate/agent-output.md`
- Events: `docs/benchmarks/live-product-maturity-memory-skill-rerun-20260517-124722/skill-promotion-gate/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 1
tool_execution_complete: 13
tool_execution_progress: 5
tool_execution_start: 13
trace_summary: 1
```

Quality signals:

```text
output_chars: 1459
diff_chars: 3142
diff_files_changed: 1
tool_executions: 13
first_write_tool_index: 9
forbidden_tool_uses: none
tool_errors: 0
tool_failures: 5
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 142
test_status: failed
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=29759 tool_schema=3186 tools=15 workflow=strict closeout=full validation=failed:3/10
adaptive_triggers: required_validation,repeated_no_code_progress,first_code_change,verification_failed,acceptance_rejected
trace_event_types: tool.start,tool.done,verify.done,reflection.pass,stage.validation,acceptance.review,workflow.plan,memory.sync,workflow.fallback,closeout,runtime.diet,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: seeded_code_change
behavior_assertions: skill_promotion_gate,skill_evolution_cooldown
behavior_assertion_status: failed
warning: earlier_verification_failed_before_repair
warning: earlier_stage_validation_failed_before_repair
warning: required_commands_not_passing
warning: behavior_assertions_not_passing
failure_owner: agent_flow
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
memory_sync_events: 9
memory_tool_calls: 0
retrieval_sources: Project,Session
required_commands: 5
agent_required_commands: 5
harness_commands: 0
required_command_status: failed
validation_events: 5
stage_validation_events: 5
tool_progress_events: 5
guided_debugging_events: 4
guided_reasoning_events: 1
workflow_plan_events: 2
weighted_plan_events: 1
reweighted_plan_events: 1
adaptive_trigger_events: 5
adaptive_triggers: required_validation,repeated_no_code_progress,first_code_change,verification_failed,acceptance_rejected
latest_top_priority: P3
latest_top_importance_score: 0.2200000137090683
latest_top_weight_share: 0.1725490242242813
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=29759 tool_schema=3186 tools=15 workflow=strict
attention: required commands did not pass in the harness
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
