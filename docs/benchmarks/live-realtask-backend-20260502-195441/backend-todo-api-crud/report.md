# Live Eval Report: backend-todo-api-crud

- Run id: `realtask-backend-20260502-195441`
- Sample: `evalsets/live_tasks/backend-todo-api-crud.yaml`
- Worktree: `target/live-evals/realtask-backend-20260502-195441/backend-todo-api-crud/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/realtask-backend-20260502-195441/backend-todo-api-crud/env`
- Test status: `ok`
- Generated: `2026-05-02 20:00:19 +0800`

## Git Status

```text
 M fixtures/live_backend/todo_api/todo_api.py
```

## Diff Stat

```text
 fixtures/live_backend/todo_api/todo_api.py | 94 +++++++++++++++++++++++++-----
 1 file changed, 79 insertions(+), 15 deletions(-)
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
- Output: `docs/benchmarks/live-realtask-backend-20260502-195441/backend-todo-api-crud/agent-output.md`
- Events: `docs/benchmarks/live-realtask-backend-20260502-195441/backend-todo-api-crud/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 1
tool_execution_complete: 4
tool_execution_progress: 1
tool_execution_start: 4
trace_summary: 1
```

Quality signals:

```text
output_chars: 381
diff_chars: 5164
tool_executions: 4
first_write_tool_index: 4
tool_errors: 0
tool_failures: 0
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 39
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
trace_event_types: api.done,tool.start,tool.done,verify.done,reflection.pass,stage.validation,acceptance.review,workflow.plan,memory.sync,workflow.fallback,closeout,assistant
stale_edit_warnings: 0
failure_owner: none
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
