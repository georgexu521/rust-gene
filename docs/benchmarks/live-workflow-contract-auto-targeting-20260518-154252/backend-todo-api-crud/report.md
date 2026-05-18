# Live Eval Report: backend-todo-api-crud

- Run id: `workflow-contract-auto-targeting-20260518-154252`
- Sample: `evalsets/live_tasks/backend-todo-api-crud.yaml`
- Worktree: `target/live-evals/workflow-contract-auto-targeting-20260518-154252/backend-todo-api-crud/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/workflow-contract-auto-targeting-20260518-154252/backend-todo-api-crud/env`
- Test status: `failed`
- Generated: `2026-05-18 17:52:43 +0800`

## Git Status

```text
 M fixtures/live_backend/todo_api/todo_api.py
```

## Diff Stat

```text
 fixtures/live_backend/todo_api/todo_api.py | 94 +++++++++++++++++++++++++-----
 1 file changed, 78 insertions(+), 16 deletions(-)
```

## Required Commands

```text
$ python3 -m unittest discover -s fixtures/live_backend/todo_api -p 'test_*.py'
E
======================================================================
ERROR: test_todo_api (unittest.loader._FailedTest.test_todo_api)
----------------------------------------------------------------------
ImportError: Failed to import test module: test_todo_api
Traceback (most recent call last):
  File "/Library/Frameworks/Python.framework/Versions/3.12/lib/python3.12/unittest/loader.py", line 394, in _find_test_path
    module = self._get_module_from_name(name)
             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  File "/Library/Frameworks/Python.framework/Versions/3.12/lib/python3.12/unittest/loader.py", line 337, in _get_module_from_name
    __import__(name)
  File "/Users/georgexu/Desktop/rust-agent/target/live-evals/workflow-contract-auto-targeting-20260518-154252/backend-todo-api-crud/worktree/fixtures/live_backend/todo_api/test_todo_api.py", line 8, in <module>
    import todo_api
  File "/Users/georgexu/Desktop/rust-agent/target/live-evals/workflow-contract-auto-targeting-20260518-154252/backend-todo-api-crud/worktree/fixtures/live_backend/todo_api/todo_api.py", line 15
    def list(self, completed=None):
                                   ^
IndentationError: unindent does not match any outer indentation level


----------------------------------------------------------------------
Ran 1 test in 0.000s

FAILED (errors=1)
[exit status: 1]

$ ! rg 'TODO' fixtures/live_backend/todo_api/todo_api.py
    _PATH_TODOS = re.compile(r"^/todos$")
    _PATH_TODO_ID = re.compile(r"^/todos/(\d+)$")
        m = self._PATH_TODO_ID.match(path)
        m = self._PATH_TODOS.match(self.path)
        m = self._PATH_TODOS.match(self.path)
[exit status: 1]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-workflow-contract-auto-targeting-20260518-154252/backend-todo-api-crud/agent-output.md`
- Events: `docs/benchmarks/live-workflow-contract-auto-targeting-20260518-154252/backend-todo-api-crud/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 2
tool_execution_complete: 10
tool_execution_progress: 7
tool_execution_start: 10
trace_summary: 1
```

Quality signals:

```text
output_chars: 1188
diff_chars: 4624
diff_files_changed: 1
tool_executions: 10
first_write_tool_index: 3
forbidden_tool_uses: none
tool_errors: 2
tool_failures: 2
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 104
test_status: failed
verification_passed: false
stage_validation_passed: false
acceptance_accepted: None
closeout_status: failed
closeout_tool_records: 10
closeout_tool_evidence: tool evidence: records=10 completed=8 failed=2 denied=0 validation=0 closeout=5 repair=7 changed=5 workflows=code_change commands=cd /Users/georgexu/Desktop/rust-agent/target/live-evals/workflow-contract-auto-targeting-20260518-154252/backe...
runtime_diet: prompt=26529 tool_schema=3186 tools=15 workflow=strict closeout=full validation=failed:2/2
adaptive_triggers: required_validation,first_code_change,verification_failed
trace_event_types: stage.validation,guided.debug,memory.sync,api.start,workflow.fallback,api.done,tool.start,tool.done,guided.debug,closeout,runtime.diet,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: seeded_code_change
behavior_assertions: none
behavior_assertion_status: none
warning: tool_errors_seen
warning: earlier_verification_failed_before_repair
warning: earlier_stage_validation_failed_before_repair
warning: required_commands_not_passing
warning: closeout_not_successful
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
weighted_planning_active: false
closeout_active: true
adaptive_workflow_active: true
active_specialty_signals: 5/7
workflow_contract_activation: entry=skipped:auto repair=active_after_failure
workflow_contract_events: 1
memory_sync_events: 8
memory_tool_calls: 0
retrieval_sources: Project
required_commands: 2
agent_required_commands: 2
harness_commands: 0
required_command_status: failed
validation_events: 7
stage_validation_events: 7
tool_progress_events: 7
guided_debugging_events: 9
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
closeout_status: failed
closeout_tool_records: 10
closeout_tool_evidence: tool evidence: records=10 completed=8 failed=2 denied=0 validation=0 closeout=5 repair=7 changed=5 workflows=code_change commands=cd /Users/georgexu/Desktop/rust-agent/target/live-evals/workflow-contract-auto-targeting-20260518-154252/backe...
runtime_diet: prompt=26529 tool_schema=3186 tools=15 workflow=strict
attention: required commands did not pass in the harness
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
