# Live Eval Report: persistent-memory-planning-context

- Run id: `product-maturity-seeded-fixes-20260517-143047`
- Sample: `evalsets/live_tasks/persistent-memory-planning-context.yaml`
- Worktree: `target/live-evals/product-maturity-seeded-fixes-20260517-143047/persistent-memory-planning-context/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/product-maturity-seeded-fixes-20260517-143047/persistent-memory-planning-context/env`
- Test status: `ok`
- Generated: `2026-05-17 14:49:28 +0800`

## Git Status

```text
 M src/engine/conversation_loop/turn_retrieval_context_controller.rs
```

## Diff Stat

```text
 src/engine/conversation_loop/turn_retrieval_context_controller.rs | 7 ++++++-
 1 file changed, 6 insertions(+), 1 deletion(-)
```

## Required Commands

```text
$ cargo test -q learning_planning -- --test-threads=1

running 5 tests
.....
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 1435 filtered out; finished in 0.01s

[exit status: 0]

$ cargo test -q retrieval_context -- --test-threads=1

running 16 tests
................
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 1424 filtered out; finished in 0.02s

[exit status: 0]

$ python3 -c "p='src/engine/conversation_loop/turn_retrieval_context_controller.rs'; s=open(p).read(); assert 'prefetch_retrieval_context_with_llm_rerank' in s and 'Self::merge_context(&mut turn_retrieval_context, memory_ctx)' in s and 'TraceEvent::MemoryPrefetch' in s"
[exit status: 0]

$ python3 -c "p='src/engine/conversation_loop/workflow_contract_controller.rs'; s=open(p).read(); assert 'apply_learning_to_workflow_judgment' in s and 'context.retrieval_context' in s"
[exit status: 0]

$ python3 -c "p='src/engine/conversation_loop/mod.rs'; s=open(p).read(); ctx=s.find('TurnContextBootstrapController::run'); gate=s.find('TurnEntryGateController::run'); assert ctx >= 0 and gate >= 0 and ctx < gate"
[exit status: 0]

$ cargo test -q -- --test-threads=1

running 1440 tests
....................................................................................... 87/1440
....................................................................................... 174/1440
....................................................................................... 261/1440
....................................................................................... 348/1440
....................................................................................... 435/1440
....................................................................................... 522/1440
....................................................................................... 609/1440
....................................................................................... 696/1440
....................................................................................... 783/1440
....................................................................................... 870/1440
....................................................................................... 957/1440
....................................................................................... 1044/1440
....................................................................................... 1131/1440
....................................................................................... 1218/1440
....................................................................................... 1305/1440
....................................................................................... 1392/1440
................................................
test result: ok. 1440 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 21.69s

[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-product-maturity-seeded-fixes-20260517-143047/persistent-memory-planning-context/agent-output.md`
- Events: `docs/benchmarks/live-product-maturity-seeded-fixes-20260517-143047/persistent-memory-planning-context/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 1
tool_execution_complete: 5
tool_execution_progress: 1
tool_execution_start: 5
trace_summary: 1
```

Quality signals:

```text
output_chars: 1494
diff_chars: 1083
diff_files_changed: 1
tool_executions: 5
first_write_tool_index: 5
forbidden_tool_uses: none
tool_errors: 0
tool_failures: 0
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 47
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=11969 tool_schema=3186 tools=15 workflow=strict closeout=full validation=passed:9/9
adaptive_triggers: required_validation,first_code_change
trace_event_types: workflow.trigger,workflow.fallback,verify.done,reflection.pass,stage.validation,acceptance.review,workflow.plan,memory.sync,workflow.fallback,closeout,runtime.diet,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: seeded_code_change
behavior_assertions: memory_planning_context,memory_retrieval_before_workflow_judgment
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
memory_sync_events: 3
memory_tool_calls: 0
retrieval_sources: Project,Session
required_commands: 6
agent_required_commands: 6
harness_commands: 0
required_command_status: ok
validation_events: 1
stage_validation_events: 1
tool_progress_events: 1
guided_debugging_events: 0
guided_reasoning_events: 1
workflow_plan_events: 2
weighted_plan_events: 1
reweighted_plan_events: 1
adaptive_trigger_events: 2
adaptive_triggers: required_validation,first_code_change
latest_top_priority: P1
latest_top_importance_score: 0.7950000762939453
latest_top_weight_share: 0.16528066992759705
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=11969 tool_schema=3186 tools=15 workflow=strict
note: guided debugging is expected only after a blocker or failed validation
```

Agent stderr tail:

```text
[required validation still running after 30s] cargo test -q learning_planning -- --test-threads=1
[required validation still running after 60s] cargo test -q learning_planning -- --test-threads=1
[required validation still running after 90s] cargo test -q learning_planning -- --test-threads=1
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
