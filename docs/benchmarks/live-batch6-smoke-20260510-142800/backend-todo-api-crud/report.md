# Live Eval Report: backend-todo-api-crud

- Run id: `batch6-smoke-20260510-142800`
- Sample: `evalsets/live_tasks/backend-todo-api-crud.yaml`
- Worktree: `target/live-evals/batch6-smoke-20260510-142800/backend-todo-api-crud/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/batch6-smoke-20260510-142800/backend-todo-api-crud/env`
- Test status: `ok`
- Generated: `2026-05-10 14:31:04 +0800`

## Git Status

```text
 M fixtures/live_backend/todo_api/todo_api.py
```

## Diff Stat

```text
 fixtures/live_backend/todo_api/todo_api.py | 82 +++++++++++++++++++++++++-----
 1 file changed, 68 insertions(+), 14 deletions(-)
```

## Required Commands

```text
$ python3 -m unittest discover -s fixtures/live_backend/todo_api -p 'test_*.py'
..
----------------------------------------------------------------------
Ran 2 tests in 0.510s

OK
[exit status: 0]

$ ! rg 'TODO' fixtures/live_backend/todo_api/todo_api.py
[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-batch6-smoke-20260510-142800/backend-todo-api-crud/agent-output.md`
- Events: `docs/benchmarks/live-batch6-smoke-20260510-142800/backend-todo-api-crud/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 1
tool_execution_complete: 7
tool_execution_progress: 3
tool_execution_start: 7
trace_summary: 1
```

Quality signals:

```text
output_chars: 563
diff_chars: 4483
tool_executions: 7
first_write_tool_index: 5
tool_errors: 0
tool_failures: 0
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 66
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: None
closeout_status: passed
runtime_diet: prompt=8503 tool_schema=2641 tools=12 workflow=strict closeout=full validation=failed:3/18
adaptive_triggers: required_validation,first_code_change,verification_failed
trace_event_types: workflow.fallback,api.done,tool.start,tool.done,verify.done,reflection.pass,stage.validation,memory.sync,workflow.fallback,closeout,runtime.diet,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: seeded_code_change
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
weighted_planning_active: false
closeout_active: false
adaptive_workflow_active: true
active_specialty_signals: 4/7
memory_sync_events: 5
memory_tool_calls: 0
retrieval_sources: Project
required_commands: 2
required_command_status: ok
validation_events: 3
stage_validation_events: 3
tool_progress_events: 3
guided_debugging_events: 2
guided_reasoning_events: 0
workflow_plan_events: 0
weighted_plan_events: 0
reweighted_plan_events: 0
adaptive_trigger_events: 3
adaptive_triggers: required_validation,first_code_change,verification_failed
latest_top_priority: none
latest_top_importance_score: none
latest_top_weight_share: none
acceptance_accepted: missing
closeout_status: passed
runtime_diet: prompt=8503 tool_schema=2641 tools=12 workflow=strict
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
