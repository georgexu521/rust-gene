# Live Eval Report: backend-todo-api-crud

- Run id: `repair-planner-backend-20260517-182004`
- Sample: `evalsets/live_tasks/backend-todo-api-crud.yaml`
- Worktree: `target/live-evals/repair-planner-backend-20260517-182004/backend-todo-api-crud/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/repair-planner-backend-20260517-182004/backend-todo-api-crud/env`
- Test status: `ok`
- Generated: `2026-05-17 18:27:03 +0800`

## Git Status

```text
 M fixtures/live_backend/todo_api/todo_api.py
```

## Diff Stat

```text
 fixtures/live_backend/todo_api/todo_api.py | 123 +++++++++++++++++++++++++----
 1 file changed, 107 insertions(+), 16 deletions(-)
```

## Required Commands

```text
$ python3 -m unittest discover -s fixtures/live_backend/todo_api -p 'test_*.py'
..
----------------------------------------------------------------------
Ran 2 tests in 0.512s

OK
[exit status: 0]

$ ! rg 'TODO' fixtures/live_backend/todo_api/todo_api.py
[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-repair-planner-backend-20260517-182004/backend-todo-api-crud/agent-output.md`
- Events: `docs/benchmarks/live-repair-planner-backend-20260517-182004/backend-todo-api-crud/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 1
tool_execution_complete: 8
tool_execution_progress: 6
tool_execution_start: 8
trace_summary: 1
```

Quality signals:

```text
output_chars: 1147
diff_chars: 6045
diff_files_changed: 1
tool_executions: 8
first_write_tool_index: 3
forbidden_tool_uses: none
tool_errors: 0
tool_failures: 5
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 129
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=22684 tool_schema=3186 tools=15 workflow=strict closeout=full validation=failed:1/6
adaptive_triggers: required_validation,first_code_change,verification_failed,acceptance_rejected
trace_event_types: tool.start,tool.done,verify.done,reflection.pass,stage.validation,acceptance.review,workflow.plan,memory.sync,workflow.fallback,closeout,runtime.diet,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: seeded_code_change
behavior_assertions: none
behavior_assertion_status: none
warning: earlier_verification_failed_before_repair
warning: earlier_stage_validation_failed_before_repair
failure_owner: none
```

Specialty signals:

```text
memory_active: true
automation_active: true
guided_debugging_active: true
guided_reasoning_active: false
weighted_planning_active: true
closeout_active: true
adaptive_workflow_active: true
active_specialty_signals: 6/7
memory_sync_events: 7
memory_tool_calls: 0
retrieval_sources: Project
required_commands: 2
agent_required_commands: 2
harness_commands: 0
required_command_status: ok
validation_events: 6
stage_validation_events: 6
tool_progress_events: 6
guided_debugging_events: 5
guided_reasoning_events: 0
workflow_plan_events: 2
weighted_plan_events: 1
reweighted_plan_events: 1
adaptive_trigger_events: 4
adaptive_triggers: required_validation,first_code_change,verification_failed,acceptance_rejected
latest_top_priority: P1
latest_top_importance_score: 0.6850000023841858
latest_top_weight_share: 0.3784530460834503
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=22684 tool_schema=3186 tools=15 workflow=strict
```

Agent stderr tail:

```text
2026-05-17T10:26:39.460379Z  WARN priority_agent::engine::conversation_loop::patch_recovery: Patch synthesis JSON actions were not directly applicable: synthesized patch old_string was not found exactly in /Users/georgexu/Desktop/rust-agent/target/live-evals/repair-planner-backend-20260517-182004/backend-todo-api-crud/worktree/fixtures/live_backend/todo_api/todo_api.py; refusing inexact multi-line replacement; synthesized patch old_string was not found exactly in /Users/georgexu/Desktop/rust-agent/target/live-evals/repair-planner-backend-20260517-182004/backend-todo-api-crud/worktree/fixtures/live_backend/todo_api/todo_api.py; refusing inexact multi-line replacement; synthesized patch old_string was not found exactly in /Users/georgexu/Desktop/rust-agent/target/live-evals/repair-planner-backend-20260517-182004/backend-todo-api-crud/worktree/fixtures/live_backend/todo_api/todo_api.py; refusing inexact multi-line replacement; synthesized patch old_string was not found exactly in /Users/georgexu/Desktop/rust-agent/target/live-evals/repair-planner-backend-20260517-182004/backend-todo-api-crud/worktree/fixtures/live_backend/todo_api/todo_api.py; refusing inexact multi-line replacement; synthesized patch old_string was not found exactly in /Users/georgexu/Desktop/rust-agent/target/live-evals/repair-planner-backend-20260517-182004/backend-todo-api-crud/worktree/fixtures/live_backend/todo_api/todo_api.py; refusing inexact multi-line replacement
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
