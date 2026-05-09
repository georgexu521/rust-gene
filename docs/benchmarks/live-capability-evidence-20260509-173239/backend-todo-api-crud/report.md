# Live Eval Report: backend-todo-api-crud

- Run id: `capability-evidence-20260509-173239`
- Sample: `evalsets/live_tasks/backend-todo-api-crud.yaml`
- Worktree: `target/live-evals/capability-evidence-20260509-173239/backend-todo-api-crud/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/capability-evidence-20260509-173239/backend-todo-api-crud/env`
- Test status: `ok`
- Generated: `2026-05-09 17:52:23 +0800`

## Git Status

```text
 M fixtures/live_backend/todo_api/todo_api.py
```

## Diff Stat

```text
 fixtures/live_backend/todo_api/todo_api.py | 90 +++++++++++++++++++++++++-----
 1 file changed, 75 insertions(+), 15 deletions(-)
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
- Output: `docs/benchmarks/live-capability-evidence-20260509-173239/backend-todo-api-crud/agent-output.md`
- Events: `docs/benchmarks/live-capability-evidence-20260509-173239/backend-todo-api-crud/agent-events.jsonl`

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
output_chars: 917
diff_chars: 4726
tool_executions: 8
first_write_tool_index: 5
tool_errors: 0
tool_failures: 0
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 72
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=8092 tool_schema=2641 tools=12 workflow=strict closeout=full validation=passed
adaptive_triggers: required_validation,repeated_no_code_progress,first_code_change,verification_failed,acceptance_rejected
trace_event_types: tool.start,tool.done,verify.done,reflection.pass,stage.validation,acceptance.review,workflow.plan,memory.sync,workflow.fallback,closeout,runtime.diet,assistant
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
guided_reasoning_active: true
weighted_planning_active: true
closeout_active: true
adaptive_workflow_active: true
active_specialty_signals: 7/7
memory_sync_events: 4
memory_tool_calls: 0
retrieval_sources: Project
required_commands: 2
required_command_status: ok
validation_events: 2
stage_validation_events: 2
tool_progress_events: 6
guided_debugging_events: 1
guided_reasoning_events: 1
workflow_plan_events: 2
weighted_plan_events: 1
reweighted_plan_events: 1
adaptive_trigger_events: 5
adaptive_triggers: required_validation,repeated_no_code_progress,first_code_change,verification_failed,acceptance_rejected
latest_top_priority: P1
latest_top_importance_score: 0.6650000810623169
latest_top_weight_share: 0.4353518784046173
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=8092 tool_schema=2641 tools=12 workflow=strict
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
