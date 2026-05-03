# Live Eval Report: backend-todo-api-crud

- Run id: `realtask-backend-20260502-183603`
- Sample: `evalsets/live_tasks/backend-todo-api-crud.yaml`
- Worktree: `target/live-evals/realtask-backend-20260502-183603/backend-todo-api-crud/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/realtask-backend-20260502-183603/backend-todo-api-crud/env`
- Test status: `ok`
- Generated: `2026-05-02 19:40:01 +0800`

## Git Status

```text
 M fixtures/live_backend/todo_api/todo_api.py
?? fixtures/live_backend/todo_api/__pycache__/
```

## Diff Stat

```text
 fixtures/live_backend/todo_api/todo_api.py | 103 ++++++++++++++++++++++++-----
 1 file changed, 88 insertions(+), 15 deletions(-)
```

## Required Commands

```text
$ python3 -m unittest discover -s fixtures/live_backend/todo_api -p 'test_*.py'
..
----------------------------------------------------------------------
Ran 2 tests in 0.520s

OK
[exit status: 0]

$ ! rg 'TODO' fixtures/live_backend/todo_api/todo_api.py
[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-realtask-backend-20260502-183603/backend-todo-api-crud/agent-output.md`
- Events: `docs/benchmarks/live-realtask-backend-20260502-183603/backend-todo-api-crud/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 1
tool_execution_complete: 6
tool_execution_progress: 3
tool_execution_start: 6
trace_summary: 1
```

Quality signals:

```text
output_chars: 652
diff_chars: 5074
tool_executions: 6
first_write_tool_index: 4
tool_errors: 0
tool_failures: 0
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 63
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
trace_event_types: api.done,tool.start,tool.done,verify.done,reflection.pass,stage.validation,acceptance.review,workflow.plan,memory.sync,workflow.fallback,closeout,assistant
stale_edit_warnings: 2
warning: repeated_stale_edit_warnings
warning: earlier_verification_failed_before_repair
warning: earlier_stage_validation_failed_before_repair
failure_owner: none
```

Agent stderr tail:

```text
2026-05-02T10:42:00.080560Z  WARN priority_agent::tools::file_tool: File 'fixtures/live_backend/todo_api/todo_api.py' was modified since it was read
2026-05-02T10:46:21.269564Z  WARN priority_agent::tools::file_tool: File 'fixtures/live_backend/todo_api/todo_api.py' was modified since it was read
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
