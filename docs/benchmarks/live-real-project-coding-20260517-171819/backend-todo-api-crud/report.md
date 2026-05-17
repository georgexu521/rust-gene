# Live Eval Report: backend-todo-api-crud

- Run id: `real-project-coding-20260517-171819`
- Sample: `evalsets/live_tasks/backend-todo-api-crud.yaml`
- Worktree: `target/live-evals/real-project-coding-20260517-171819/backend-todo-api-crud/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/real-project-coding-20260517-171819/backend-todo-api-crud/env`
- Test status: `failed`
- Generated: `2026-05-17 17:26:09 +0800`

## Git Status

```text
 M fixtures/live_backend/todo_api/todo_api.py
```

## Diff Stat

```text
 fixtures/live_backend/todo_api/todo_api.py | 98 +++++++++++++++++++++++++-----
 1 file changed, 84 insertions(+), 14 deletions(-)
```

## Required Commands

```text
$ python3 -m unittest discover -s fixtures/live_backend/todo_api -p 'test_*.py'
.F
======================================================================
FAIL: test_crud_and_filtering (test_todo_api.TodoApiTest.test_crud_and_filtering)
----------------------------------------------------------------------
Traceback (most recent call last):
  File "/Users/georgexu/Desktop/rust-agent/target/live-evals/real-project-coding-20260517-171819/backend-todo-api-crud/worktree/fixtures/live_backend/todo_api/test_todo_api.py", line 60, in test_crud_and_filtering
    self.assertEqual(status, 200)
AssertionError: 404 != 200

----------------------------------------------------------------------
Ran 2 tests in 0.510s

FAILED (failures=1)
[exit status: 1]

$ ! rg 'TODO' fixtures/live_backend/todo_api/todo_api.py
[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-real-project-coding-20260517-171819/backend-todo-api-crud/agent-output.md`
- Events: `docs/benchmarks/live-real-project-coding-20260517-171819/backend-todo-api-crud/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 2
tool_execution_complete: 9
tool_execution_progress: 6
tool_execution_start: 9
trace_summary: 1
```

Quality signals:

```text
output_chars: 2272
diff_chars: 5157
diff_files_changed: 1
tool_executions: 9
first_write_tool_index: 4
forbidden_tool_uses: none
tool_errors: 3
tool_failures: 7
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 119
test_status: failed
verification_passed: false
stage_validation_passed: false
acceptance_accepted: False
closeout_status: failed
runtime_diet: prompt=19299 tool_schema=3186 tools=15 workflow=strict closeout=full validation=failed:1/5
adaptive_triggers: required_validation,first_code_change,verification_failed,acceptance_rejected
trace_event_types: workflow.fallback,workflow.fallback,workflow.fallback,api.start,workflow.fallback,api.done,tool.start,tool.done,guided.debug,closeout,runtime.diet,assistant
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
memory_sync_events: 6
memory_tool_calls: 0
retrieval_sources: Project
required_commands: 2
agent_required_commands: 2
harness_commands: 0
required_command_status: failed
validation_events: 4
stage_validation_events: 4
tool_progress_events: 6
guided_debugging_events: 6
guided_reasoning_events: 0
workflow_plan_events: 1
weighted_plan_events: 1
reweighted_plan_events: 0
adaptive_trigger_events: 4
adaptive_triggers: required_validation,first_code_change,verification_failed,acceptance_rejected
latest_top_priority: P3
latest_top_importance_score: 0.21000000834465027
latest_top_weight_share: 0.3414634168148041
acceptance_accepted: False
closeout_status: failed
runtime_diet: prompt=19299 tool_schema=3186 tools=15 workflow=strict
attention: required commands did not pass in the harness
```

Agent stderr tail:

```text
2026-05-17T09:25:44.920502Z  WARN priority_agent::engine::conversation_loop::patch_recovery: Patch synthesis JSON actions were not directly applicable: synthesized patch old_string was not found exactly in /Users/georgexu/Desktop/rust-agent/target/live-evals/real-project-coding-20260517-171819/backend-todo-api-crud/worktree/fixtures/live_backend/todo_api/todo_api.py; refusing inexact multi-line replacement
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
