# Live Eval Report: frontend-book-notes-localstorage

- Run id: `real-project-coding-20260517-192347`
- Sample: `evalsets/live_tasks/frontend-book-notes-localstorage.yaml`
- Worktree: `target/live-evals/real-project-coding-20260517-192347/frontend-book-notes-localstorage/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/real-project-coding-20260517-192347/frontend-book-notes-localstorage/env`
- Test status: `ok`
- Generated: `2026-05-17 19:31:15 +0800`

## Git Status

```text
 M fixtures/live_frontend/book_notes/app.js
```

## Diff Stat

```text
 fixtures/live_frontend/book_notes/app.js | 50 ++++++++++++++++++++++++++------
 1 file changed, 41 insertions(+), 9 deletions(-)
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
- Output: `docs/benchmarks/live-real-project-coding-20260517-192347/frontend-book-notes-localstorage/agent-output.md`
- Events: `docs/benchmarks/live-real-project-coding-20260517-192347/frontend-book-notes-localstorage/agent-events.jsonl`

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
output_chars: 945
diff_chars: 2141
diff_files_changed: 1
tool_executions: 6
first_write_tool_index: 4
forbidden_tool_uses: none
tool_errors: 0
tool_failures: 0
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 68
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=12188 tool_schema=3186 tools=15 workflow=strict closeout=full validation=passed:2/2 recovered_failed:2
adaptive_triggers: required_validation,first_code_change,verification_failed,acceptance_rejected
trace_event_types: tool.start,tool.done,verify.done,reflection.pass,stage.validation,acceptance.review,workflow.plan,memory.sync,workflow.fallback,closeout,runtime.diet,assistant
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
guided_debugging_active: true
guided_reasoning_active: true
weighted_planning_active: true
closeout_active: true
adaptive_workflow_active: true
active_specialty_signals: 7/7
memory_sync_events: 4
memory_tool_calls: 0
retrieval_sources: Project,Session
required_commands: 2
agent_required_commands: 2
harness_commands: 0
required_command_status: ok
validation_events: 3
stage_validation_events: 3
tool_progress_events: 3
guided_debugging_events: 2
guided_reasoning_events: 1
workflow_plan_events: 2
weighted_plan_events: 1
reweighted_plan_events: 1
adaptive_trigger_events: 4
adaptive_triggers: required_validation,first_code_change,verification_failed,acceptance_rejected
latest_top_priority: P2
latest_top_importance_score: 0.4150000512599945
latest_top_weight_share: 0.49112430214881897
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=12188 tool_schema=3186 tools=15 workflow=strict
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
