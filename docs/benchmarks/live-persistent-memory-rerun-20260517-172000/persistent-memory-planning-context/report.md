# Live Eval Report: persistent-memory-planning-context

- Run id: `persistent-memory-rerun-20260517-172000`
- Sample: `evalsets/live_tasks/persistent-memory-planning-context.yaml`
- Worktree: `target/live-evals/persistent-memory-rerun-20260517-172000/persistent-memory-planning-context/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/persistent-memory-rerun-20260517-172000/persistent-memory-planning-context/env`
- Test status: `ok`
- Generated: `2026-05-17 17:08:44 +0800`

## Git Status

```text
 M src/engine/conversation_loop/turn_retrieval_context_controller.rs
```

## Diff Stat

```text
 src/engine/conversation_loop/turn_retrieval_context_controller.rs | 4 ++++
 1 file changed, 4 insertions(+)
```

## Required Commands

```text
$ cargo test -q learning_planning -- --test-threads=1

running 5 tests
.....
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 1441 filtered out; finished in 0.01s

[exit status: 0]

$ cargo test -q retrieval_context -- --test-threads=1

running 16 tests
................
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 1430 filtered out; finished in 0.01s

[exit status: 0]

$ python3 -c "p='src/engine/conversation_loop/turn_retrieval_context_controller.rs'; s=open(p).read(); assert 'prefetch_retrieval_context_with_llm_rerank' in s and 'Self::merge_context(&mut turn_retrieval_context, memory_ctx)' in s and 'TraceEvent::MemoryPrefetch' in s"
[exit status: 0]

$ python3 -c "p='src/engine/conversation_loop/workflow_contract_controller.rs'; s=open(p).read(); assert 'apply_learning_to_workflow_judgment' in s and 'context.retrieval_context' in s"
[exit status: 0]

$ python3 -c "p='src/engine/conversation_loop/mod.rs'; s=open(p).read(); ctx=s.find('TurnContextBootstrapController::run'); gate=s.find('TurnEntryGateController::run'); assert ctx >= 0 and gate >= 0 and ctx < gate"
[exit status: 0]

$ cargo test -q -- --test-threads=1

running 1446 tests
....................................................................................... 87/1446
....................................................................................... 174/1446
....................................................................................... 261/1446
....................................................................................... 348/1446
....................................................................................... 435/1446
....................................................................................... 522/1446
....................................................................................... 609/1446
....................................................................................... 696/1446
....................................................................................... 783/1446
....................................................................................... 870/1446
....................................................................................... 957/1446
....................................................................................... 1044/1446
....................................................................................... 1131/1446
....................................................................................... 1218/1446
....................................................................................... 1305/1446
....................................................................................... 1392/1446
......................................................
test result: ok. 1446 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 22.75s

[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-persistent-memory-rerun-20260517-172000/persistent-memory-planning-context/agent-output.md`
- Events: `docs/benchmarks/live-persistent-memory-rerun-20260517-172000/persistent-memory-planning-context/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 1
tool_execution_complete: 9
tool_execution_progress: 4
tool_execution_start: 9
trace_summary: 1
```

Quality signals:

```text
output_chars: 1333
diff_chars: 845
diff_files_changed: 1
tool_executions: 9
first_write_tool_index: 7
forbidden_tool_uses: none
tool_errors: 0
tool_failures: 2
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 92
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=19137 tool_schema=3186 tools=15 workflow=strict closeout=full validation=failed:1/12
adaptive_triggers: required_validation,repeated_no_code_progress,first_code_change,verification_failed,acceptance_rejected
trace_event_types: tool.start,tool.done,verify.done,reflection.pass,stage.validation,acceptance.review,workflow.plan,memory.sync,workflow.fallback,closeout,runtime.diet,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: seeded_code_change
behavior_assertions: memory_planning_context,memory_retrieval_before_workflow_judgment
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
memory_sync_events: 5
memory_tool_calls: 0
retrieval_sources: Project,Session
required_commands: 6
agent_required_commands: 6
harness_commands: 0
required_command_status: ok
validation_events: 2
stage_validation_events: 2
tool_progress_events: 4
guided_debugging_events: 1
guided_reasoning_events: 1
workflow_plan_events: 2
weighted_plan_events: 1
reweighted_plan_events: 1
adaptive_trigger_events: 5
adaptive_triggers: required_validation,repeated_no_code_progress,first_code_change,verification_failed,acceptance_rejected
latest_top_priority: P1
latest_top_importance_score: 0.7850000262260437
latest_top_weight_share: 0.22380614280700684
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=19137 tool_schema=3186 tools=15 workflow=strict
```

Agent stderr tail:

```text
2026-05-17T09:06:25.062349Z  WARN priority_agent::engine::conversation_loop::patch_recovery: Patch synthesis JSON actions were not directly applicable: synthesized patch old_string was not found exactly in /Users/georgexu/Desktop/rust-agent/target/live-evals/persistent-memory-rerun-20260517-172000/persistent-memory-planning-context/worktree/src/engine/conversation_loop/turn_retrieval_context_controller.rs; refusing inexact multi-line replacement
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
