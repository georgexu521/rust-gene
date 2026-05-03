# Live Eval Report: backend-todo-api-crud

- Run id: `realtask-backend-20260502-181555`
- Sample: `evalsets/live_tasks/backend-todo-api-crud.yaml`
- Worktree: `target/live-evals/realtask-backend-20260502-181555/backend-todo-api-crud/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/realtask-backend-20260502-181555/backend-todo-api-crud/env`
- Test status: `failed`
- Generated: `2026-05-02 18:27:30 +0800`

## Git Status

```text
 M fixtures/live_backend/todo_api/todo_api.py
?? fixtures/live_backend/todo_api/__pycache__/
```

## Diff Stat

```text
 fixtures/live_backend/todo_api/todo_api.py | 110 ++++++++++++++++++++++++-----
 1 file changed, 93 insertions(+), 17 deletions(-)
```

## Required Commands

```text
$ python3 -m unittest fixtures/live_backend/todo_api/test_todo_api.py
E
======================================================================
ERROR: test_todo_api (unittest.loader._FailedTest.test_todo_api)
----------------------------------------------------------------------
ImportError: Failed to import test module: test_todo_api
Traceback (most recent call last):
  File "/Library/Frameworks/Python.framework/Versions/3.12/lib/python3.12/unittest/loader.py", line 137, in loadTestsFromName
    module = __import__(module_name)
             ^^^^^^^^^^^^^^^^^^^^^^^
  File "/Users/georgexu/Desktop/rust-agent/target/live-evals/realtask-backend-20260502-181555/backend-todo-api-crud/worktree/fixtures/live_backend/todo_api/test_todo_api.py", line 8, in <module>
    import todo_api
ModuleNotFoundError: No module named 'todo_api'


----------------------------------------------------------------------
Ran 1 test in 0.000s

FAILED (errors=1)
[exit status: 1]

$ ! rg 'TODO' fixtures/live_backend/todo_api/todo_api.py
[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-realtask-backend-20260502-181555/backend-todo-api-crud/agent-output.md`
- Events: `docs/benchmarks/live-realtask-backend-20260502-181555/backend-todo-api-crud/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
permission_request: 1
start: 1
text_chunk: 2
tool_execution_complete: 5
tool_execution_progress: 2
tool_execution_start: 5
trace_summary: 1
```

Quality signals:

```text
output_chars: 1115
diff_chars: 5272
tool_executions: 5
tool_errors: 2
tool_failures: 2
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 53
test_status: failed
verification_passed: false
stage_validation_passed: false
acceptance_accepted: False
closeout_status: failed
trace_event_types: workflow.fallback,memory.sync,api.start,workflow.fallback,api.done,tool.start,permission.request,permission.resolve,tool.done,guided.debug,closeout,assistant
warning: tool_errors_seen
warning: earlier_verification_failed_before_repair
warning: earlier_stage_validation_failed_before_repair
warning: required_commands_not_passing
warning: closeout_not_successful
warning: acceptance_review_rejected
warning: stage_validation_failed
warning: verification_failed
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
