# Live Eval Report: backend-todo-api-crud

- Run id: `ablation-contract-on-20260518-143305`
- Sample: `evalsets/live_tasks/backend-todo-api-crud.yaml`
- Worktree: `target/live-evals/ablation-contract-on-20260518-143305/backend-todo-api-crud/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/ablation-contract-on-20260518-143305/backend-todo-api-crud/env`
- Test status: `failed`
- Generated: `2026-05-18 14:51:10 +0800`

## Git Status

```text
 M fixtures/live_backend/todo_api/todo_api.py
```

## Diff Stat

```text
 fixtures/live_backend/todo_api/todo_api.py | 111 +++++++++++++++++++++++++----
 1 file changed, 96 insertions(+), 15 deletions(-)
```

## Required Commands

```text
$ python3 -m unittest discover -s fixtures/live_backend/todo_api -p 'test_*.py'
----------------------------------------
Exception occurred during processing of request from ('127.0.0.1', 53596)
Traceback (most recent call last):
  File "/Library/Frameworks/Python.framework/Versions/3.12/lib/python3.12/socketserver.py", line 692, in process_request_thread
    self.finish_request(request, client_address)
  File "/Library/Frameworks/Python.framework/Versions/3.12/lib/python3.12/socketserver.py", line 362, in finish_request
    self.RequestHandlerClass(request, client_address, self)
  File "/Library/Frameworks/Python.framework/Versions/3.12/lib/python3.12/socketserver.py", line 761, in __init__
    self.handle()
  File "/Library/Frameworks/Python.framework/Versions/3.12/lib/python3.12/http/server.py", line 436, in handle
    self.handle_one_request()
  File "/Library/Frameworks/Python.framework/Versions/3.12/lib/python3.12/http/server.py", line 424, in handle_one_request
    method()
  File "/Users/georgexu/Desktop/rust-agent/target/live-evals/ablation-contract-on-20260518-143305/backend-todo-api-crud/worktree/fixtures/live_backend/todo_api/todo_api.py", line 100, in do_POST
    m = self._TODO_PATH.match(parsed.path)
        ^^^^^^^^^^^^^^^
AttributeError: 'TodoHandler' object has no attribute '_TODO_PATH'. Did you mean: '_TODOS_PATH'?
----------------------------------------
E----------------------------------------
Exception occurred during processing of request from ('127.0.0.1', 53598)
Traceback (most recent call last):
  File "/Library/Frameworks/Python.framework/Versions/3.12/lib/python3.12/socketserver.py", line 692, in process_request_thread
    self.finish_request(request, client_address)
  File "/Library/Frameworks/Python.framework/Versions/3.12/lib/python3.12/socketserver.py", line 362, in finish_request
    self.RequestHandlerClass(request, client_address, self)
  File "/Library/Frameworks/Python.framework/Versions/3.12/lib/python3.12/socketserver.py", line 761, in __init__
    self.handle()
  File "/Library/Frameworks/Python.framework/Versions/3.12/lib/python3.12/http/server.py", line 436, in handle
    self.handle_one_request()
  File "/Library/Frameworks/Python.framework/Versions/3.12/lib/python3.12/http/server.py", line 424, in handle_one_request
    method()
  File "/Users/georgexu/Desktop/rust-agent/target/live-evals/ablation-contract-on-20260518-143305/backend-todo-api-crud/worktree/fixtures/live_backend/todo_api/todo_api.py", line 100, in do_POST
    m = self._TODO_PATH.match(parsed.path)
        ^^^^^^^^^^^^^^^
AttributeError: 'TodoHandler' object has no attribute '_TODO_PATH'. Did you mean: '_TODOS_PATH'?
----------------------------------------
E
======================================================================
ERROR: test_bad_json_and_unknown_route (test_todo_api.TodoApiTest.test_bad_json_and_unknown_route)
----------------------------------------------------------------------
Traceback (most recent call last):
  File "/Users/georgexu/Desktop/rust-agent/target/live-evals/ablation-contract-on-20260518-143305/backend-todo-api-crud/worktree/fixtures/live_backend/todo_api/test_todo_api.py", line 86, in test_bad_json_and_unknown_route
    urllib.request.urlopen(req, timeout=3)
  File "/Library/Frameworks/Python.framework/Versions/3.12/lib/python3.12/urllib/request.py", line 215, in urlopen
    return opener.open(url, data, timeout)
           ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  File "/Library/Frameworks/Python.framework/Versions/3.12/lib/python3.12/urllib/request.py", line 515, in open
    response = self._open(req, data)
               ^^^^^^^^^^^^^^^^^^^^^
  File "/Library/Frameworks/Python.framework/Versions/3.12/lib/python3.12/urllib/request.py", line 532, in _open
    result = self._call_chain(self.handle_open, protocol, protocol +
             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  File "/Library/Frameworks/Python.framework/Versions/3.12/lib/python3.12/urllib/request.py", line 492, in _call_chain
    result = func(*args)
             ^^^^^^^^^^^
  File "/Library/Frameworks/Python.framework/Versions/3.12/lib/python3.12/urllib/request.py", line 1373, in http_open
    return self.do_open(http.client.HTTPConnection, req)
           ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  File "/Library/Frameworks/Python.framework/Versions/3.12/lib/python3.12/urllib/request.py", line 1348, in do_open
    r = h.getresponse()
        ^^^^^^^^^^^^^^^
  File "/Library/Frameworks/Python.framework/Versions/3.12/lib/python3.12/http/client.py", line 1423, in getresponse
    response.begin()
  File "/Library/Frameworks/Python.framework/Versions/3.12/lib/python3.12/http/client.py", line 331, in begin
    version, status, reason = self._read_status()
                              ^^^^^^^^^^^^^^^^^^^
  File "/Library/Frameworks/Python.framework/Versions/3.12/lib/python3.12/http/client.py", line 292, in _read_status
    line = str(self.fp.readline(_MAXLINE + 1), "iso-8859-1")
               ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  File "/Library/Frameworks/Python.framework/Versions/3.12/lib/python3.12/socket.py", line 707, in readinto
    return self._sock.recv_into(b)
           ^^^^^^^^^^^^^^^^^^^^^^^
ConnectionResetError: [Errno 54] Connection reset by peer

======================================================================
ERROR: test_crud_and_filtering (test_todo_api.TodoApiTest.test_crud_and_filtering)
----------------------------------------------------------------------
Traceback (most recent call last):
  File "/Users/georgexu/Desktop/rust-agent/target/live-evals/ablation-contract-on-20260518-143305/backend-todo-api-crud/worktree/fixtures/live_backend/todo_api/test_todo_api.py", line 45, in test_crud_and_filtering
    status, payload = self.request("POST", "/todos", {"title": "Write tests"})
                      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  File "/Users/georgexu/Desktop/rust-agent/target/live-evals/ablation-contract-on-20260518-143305/backend-todo-api-crud/worktree/fixtures/live_backend/todo_api/test_todo_api.py", line 33, in request
    with urllib.request.urlopen(req, timeout=3) as response:
         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  File "/Library/Frameworks/Python.framework/Versions/3.12/lib/python3.12/urllib/request.py", line 215, in urlopen
    return opener.open(url, data, timeout)
           ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  File "/Library/Frameworks/Python.framework/Versions/3.12/lib/python3.12/urllib/request.py", line 515, in open
    response = self._open(req, data)
               ^^^^^^^^^^^^^^^^^^^^^
  File "/Library/Frameworks/Python.framework/Versions/3.12/lib/python3.12/urllib/request.py", line 532, in _open
    result = self._call_chain(self.handle_open, protocol, protocol +
             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  File "/Library/Frameworks/Python.framework/Versions/3.12/lib/python3.12/urllib/request.py", line 492, in _call_chain
    result = func(*args)
             ^^^^^^^^^^^
  File "/Library/Frameworks/Python.framework/Versions/3.12/lib/python3.12/urllib/request.py", line 1373, in http_open
    return self.do_open(http.client.HTTPConnection, req)
           ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  File "/Library/Frameworks/Python.framework/Versions/3.12/lib/python3.12/urllib/request.py", line 1348, in do_open
    r = h.getresponse()
        ^^^^^^^^^^^^^^^
  File "/Library/Frameworks/Python.framework/Versions/3.12/lib/python3.12/http/client.py", line 1423, in getresponse
    response.begin()
  File "/Library/Frameworks/Python.framework/Versions/3.12/lib/python3.12/http/client.py", line 331, in begin
    version, status, reason = self._read_status()
                              ^^^^^^^^^^^^^^^^^^^
  File "/Library/Frameworks/Python.framework/Versions/3.12/lib/python3.12/http/client.py", line 292, in _read_status
    line = str(self.fp.readline(_MAXLINE + 1), "iso-8859-1")
               ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  File "/Library/Frameworks/Python.framework/Versions/3.12/lib/python3.12/socket.py", line 707, in readinto
    return self._sock.recv_into(b)
           ^^^^^^^^^^^^^^^^^^^^^^^
ConnectionResetError: [Errno 54] Connection reset by peer

----------------------------------------------------------------------
Ran 2 tests in 0.511s

FAILED (errors=2)
[exit status: 1]

$ ! rg 'TODO' fixtures/live_backend/todo_api/todo_api.py
    _TODOS_PATH = re.compile(r"^/todos(?:/(\d+))?$")
        m = self._TODOS_PATH.match(parsed.path)
        m = self._TODO_PATH.match(parsed.path)
        m = self._TODO_PATH.match(parsed.path)
        m = self._TODO_PATH.match(parsed.path)
[exit status: 1]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-ablation-contract-on-20260518-143305/backend-todo-api-crud/agent-output.md`
- Events: `docs/benchmarks/live-ablation-contract-on-20260518-143305/backend-todo-api-crud/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 1
tool_execution_complete: 10
tool_execution_progress: 7
tool_execution_start: 10
trace_summary: 1
```

Quality signals:

```text
output_chars: 2305
diff_chars: 5337
diff_files_changed: 1
tool_executions: 10
first_write_tool_index: 3
forbidden_tool_uses: none
tool_errors: 0
tool_failures: 4
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 151
test_status: failed
verification_passed: false
stage_validation_passed: false
acceptance_accepted: False
closeout_status: failed
closeout_tool_records: 14
closeout_tool_evidence: tool evidence: records=14 completed=10 failed=4 denied=0 validation=1 closeout=7 repair=10 changed=6 workflows=code_change commands=cd /Users/georgexu/Desktop/rust-agent/target/live-evals/ablation-contract-on-20260518-143305/backend-todo-ap...
runtime_diet: prompt=32342 tool_schema=3186 tools=15 workflow=strict closeout=full validation=failed:2/2
adaptive_triggers: required_validation,first_code_change,verification_failed,acceptance_rejected
trace_event_types: tool.start,tool.done,verify.done,reflection.pass,stage.validation,guided.debug,acceptance.review,workflow.fallback,memory.sync,closeout,runtime.diet,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: seeded_code_change
behavior_assertions: none
behavior_assertion_status: none
warning: earlier_verification_failed_before_repair
warning: earlier_stage_validation_failed_before_repair
warning: required_commands_not_passing
warning: closeout_not_successful
warning: acceptance_review_rejected
warning: stage_validation_failed
warning: verification_failed
failure_owner: llm_reasoning
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
memory_sync_events: 9
memory_tool_calls: 0
retrieval_sources: Project
required_commands: 2
agent_required_commands: 2
harness_commands: 0
required_command_status: failed
validation_events: 8
stage_validation_events: 8
tool_progress_events: 7
guided_debugging_events: 8
guided_reasoning_events: 0
workflow_plan_events: 3
weighted_plan_events: 1
reweighted_plan_events: 2
adaptive_trigger_events: 4
adaptive_triggers: required_validation,first_code_change,verification_failed,acceptance_rejected
latest_top_priority: P2
latest_top_importance_score: 0.45000001788139343
latest_top_weight_share: 0.420560747385025
acceptance_accepted: False
closeout_status: failed
closeout_tool_records: 14
closeout_tool_evidence: tool evidence: records=14 completed=10 failed=4 denied=0 validation=1 closeout=7 repair=10 changed=6 workflows=code_change commands=cd /Users/georgexu/Desktop/rust-agent/target/live-evals/ablation-contract-on-20260518-143305/backend-todo-ap...
runtime_diet: prompt=32342 tool_schema=3186 tools=15 workflow=strict
attention: required commands did not pass in the harness
```

Agent stderr tail:

```text
2026-05-18T06:44:52.067205Z  WARN priority_agent::engine::conversation_loop::patch_recovery: Patch synthesis JSON actions were not directly applicable: synthesized patch old_string was not found exactly in /Users/georgexu/Desktop/rust-agent/target/live-evals/ablation-contract-on-20260518-143305/backend-todo-api-crud/worktree/fixtures/live_backend/todo_api/todo_api.py; refusing inexact multi-line replacement; synthesized patch old_string was not found exactly in /Users/georgexu/Desktop/rust-agent/target/live-evals/ablation-contract-on-20260518-143305/backend-todo-api-crud/worktree/fixtures/live_backend/todo_api/todo_api.py; refusing inexact multi-line replacement; synthesized patch old_string was not found exactly in /Users/georgexu/Desktop/rust-agent/target/live-evals/ablation-contract-on-20260518-143305/backend-todo-api-crud/worktree/fixtures/live_backend/todo_api/todo_api.py; refusing inexact multi-line replacement; synthesized patch old_string was not found exactly in /Users/georgexu/Desktop/rust-agent/target/live-evals/ablation-contract-on-20260518-143305/backend-todo-api-crud/worktree/fixtures/live_backend/todo_api/todo_api.py; refusing inexact multi-line replacement; synthesized patch old_string was not found exactly in /Users/georgexu/Desktop/rust-agent/target/live-evals/ablation-contract-on-20260518-143305/backend-todo-api-crud/worktree/fixtures/live_backend/todo_api/todo_api.py; refusing inexact multi-line replacement; synthesized patch old_string was not found exactly in /Users/georgexu/Desktop/rust-agent/target/live-evals/ablation-contract-on-20260518-143305/backend-todo-api-crud/worktree/fixtures/live_backend/todo_api/todo_api.py; refusing inexact multi-line replacement
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
