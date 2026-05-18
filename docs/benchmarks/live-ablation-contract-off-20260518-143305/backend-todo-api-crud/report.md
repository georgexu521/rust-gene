# Live Eval Report: backend-todo-api-crud

- Run id: `ablation-contract-off-20260518-143305`
- Sample: `evalsets/live_tasks/backend-todo-api-crud.yaml`
- Worktree: `target/live-evals/ablation-contract-off-20260518-143305/backend-todo-api-crud/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/ablation-contract-off-20260518-143305/backend-todo-api-crud/env`
- Test status: `ok`
- Generated: `2026-05-18 15:12:57 +0800`

## Git Status

```text
 M fixtures/live_backend/todo_api/todo_api.py
```

## Diff Stat

```text
 fixtures/live_backend/todo_api/todo_api.py | 118 +++++++++++++++++++++++++----
 1 file changed, 103 insertions(+), 15 deletions(-)
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
- Output: `docs/benchmarks/live-ablation-contract-off-20260518-143305/backend-todo-api-crud/agent-output.md`
- Events: `docs/benchmarks/live-ablation-contract-off-20260518-143305/backend-todo-api-crud/agent-events.jsonl`

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
output_chars: 700
diff_chars: 5903
diff_files_changed: 1
tool_executions: 10
first_write_tool_index: 3
forbidden_tool_uses: none
tool_errors: 0
tool_failures: 0
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 100
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
closeout_tool_records: 10
closeout_tool_evidence: tool evidence: records=10 completed=10 failed=0 denied=0 validation=0 closeout=7 repair=7 changed=7 workflows=code_change commands=none
runtime_diet: prompt=24726 tool_schema=3186 tools=15 workflow=strict closeout=full validation=passed:2/2 recovered_failed:2
adaptive_triggers: required_validation,first_code_change,verification_failed
trace_event_types: api.done,tool.start,tool.done,verify.done,reflection.pass,stage.validation,acceptance.review,memory.sync,workflow.fallback,closeout,runtime.diet,assistant
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
guided_debugging_active: false
guided_reasoning_active: false
weighted_planning_active: false
closeout_active: true
adaptive_workflow_active: true
active_specialty_signals: 4/7
memory_sync_events: 9
memory_tool_calls: 0
retrieval_sources: Project
required_commands: 2
agent_required_commands: 2
harness_commands: 0
required_command_status: ok
validation_events: 8
stage_validation_events: 8
tool_progress_events: 7
guided_debugging_events: 0
guided_reasoning_events: 0
workflow_plan_events: 0
weighted_plan_events: 0
reweighted_plan_events: 0
adaptive_trigger_events: 3
adaptive_triggers: required_validation,first_code_change,verification_failed
latest_top_priority: none
latest_top_importance_score: none
latest_top_weight_share: none
acceptance_accepted: True
closeout_status: passed
closeout_tool_records: 10
closeout_tool_evidence: tool evidence: records=10 completed=10 failed=0 denied=0 validation=0 closeout=7 repair=7 changed=7 workflows=code_change commands=none
runtime_diet: prompt=24726 tool_schema=3186 tools=15 workflow=strict
note: guided debugging is expected only after a blocker or failed validation
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
