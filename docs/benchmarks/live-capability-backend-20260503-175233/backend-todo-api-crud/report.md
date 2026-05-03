# Live Eval Report: backend-todo-api-crud

- Run id: `capability-backend-20260503-175233`
- Sample: `evalsets/live_tasks/backend-todo-api-crud.yaml`
- Worktree: `target/live-evals/capability-backend-20260503-175233/backend-todo-api-crud/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/capability-backend-20260503-175233/backend-todo-api-crud/env`
- Test status: `ok`
- Generated: `2026-05-03 17:59:06 +0800`

## Git Status

```text
 M fixtures/live_backend/todo_api/todo_api.py
```

## Diff Stat

```text
 fixtures/live_backend/todo_api/todo_api.py | 108 +++++++++++++++++++++++++----
 1 file changed, 93 insertions(+), 15 deletions(-)
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
- Output: `docs/benchmarks/live-capability-backend-20260503-175233/backend-todo-api-crud/agent-output.md`
- Events: `docs/benchmarks/live-capability-backend-20260503-175233/backend-todo-api-crud/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 1
tool_execution_complete: 8
tool_execution_progress: 4
tool_execution_start: 8
trace_summary: 1
```

Quality signals:

```text
output_chars: 780
diff_chars: 5513
tool_executions: 8
first_write_tool_index: 4
tool_errors: 2
tool_failures: 2
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 90
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
trace_event_types: api.done,tool.start,tool.done,verify.done,reflection.pass,stage.validation,acceptance.review,workflow.plan,memory.sync,workflow.fallback,closeout,assistant
stale_edit_warnings: 0
warning: tool_errors_seen
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
active_specialty_signals: 5/6
memory_sync_events: 7
memory_tool_calls: 0
retrieval_sources: Project,Session
required_commands: 2
required_command_status: ok
validation_events: 5
stage_validation_events: 5
tool_progress_events: 4
guided_debugging_events: 6
guided_reasoning_events: 0
workflow_plan_events: 2
weighted_plan_events: 1
reweighted_plan_events: 1
latest_top_priority: P3
latest_top_importance_score: 0.2600000202655792
latest_top_weight_share: 0.31707316637039185
acceptance_accepted: True
closeout_status: passed
```

## Human Review

- accepted: true
- task_success: true
- mainline_hit: true
- plan_coverage: complete
- rework_count: 4
- tool_efficiency: mixed
- diff_discipline: good
- closeout_accuracy: accurate
- notes: Agent implemented the stdlib todo API in one relevant file, passed unit tests and TODO checks, and did not claim success until validation and acceptance passed. The run included two tool failures and earlier rejected acceptance reviews, but guided debugging and validation eventually produced a correct passed closeout.
