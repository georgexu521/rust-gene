# Live Eval Report: frontend-book-notes-localstorage

- Run id: `batch6-smoke-20260510-143451`
- Sample: `evalsets/live_tasks/frontend-book-notes-localstorage.yaml`
- Worktree: `target/live-evals/batch6-smoke-20260510-143451/frontend-book-notes-localstorage/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/batch6-smoke-20260510-143451/frontend-book-notes-localstorage/env`
- Test status: `ok`
- Generated: `2026-05-10 14:37:34 +0800`

## Git Status

```text
 M fixtures/live_frontend/book_notes/app.js
```

## Diff Stat

```text
 fixtures/live_frontend/book_notes/app.js | 64 +++++++++++++++++++++++++++-----
 1 file changed, 55 insertions(+), 9 deletions(-)
```

## Required Commands

```text
$ node fixtures/live_frontend/book_notes/test-book-notes.cjs
book notes behavior ok
[exit status: 0]

$ ! rg 'TODO' fixtures/live_frontend/book_notes/app.js
[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-batch6-smoke-20260510-143451/frontend-book-notes-localstorage/agent-output.md`
- Events: `docs/benchmarks/live-batch6-smoke-20260510-143451/frontend-book-notes-localstorage/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 1
tool_execution_complete: 6
tool_execution_progress: 1
tool_execution_start: 6
trace_summary: 1
```

Quality signals:

```text
output_chars: 1154
diff_chars: 2606
tool_executions: 6
first_write_tool_index: 6
tool_errors: 0
tool_failures: 0
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 49
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=5262 tool_schema=2641 tools=12 workflow=guarded closeout=full validation=passed:5/5
adaptive_triggers: required_validation,first_code_change
trace_event_types: workflow.trigger,workflow.fallback,verify.done,reflection.pass,stage.validation,acceptance.review,workflow.plan,memory.sync,workflow.fallback,closeout,runtime.diet,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: seeded_code_change
failure_owner: none
```

Specialty signals:

```text
memory_active: true
automation_active: true
guided_debugging_active: false
guided_reasoning_active: true
weighted_planning_active: true
closeout_active: true
adaptive_workflow_active: true
active_specialty_signals: 6/7
memory_sync_events: 3
memory_tool_calls: 0
retrieval_sources: Project,Session
required_commands: 2
required_command_status: ok
validation_events: 1
stage_validation_events: 1
tool_progress_events: 1
guided_debugging_events: 0
guided_reasoning_events: 1
workflow_plan_events: 2
weighted_plan_events: 1
reweighted_plan_events: 1
adaptive_trigger_events: 2
adaptive_triggers: required_validation,first_code_change
latest_top_priority: P3
latest_top_importance_score: 0.19999998807907104
latest_top_weight_share: 0.20512819290161133
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=5262 tool_schema=2641 tools=12 workflow=guarded
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
