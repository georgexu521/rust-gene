# Live Eval Report: frontend-book-notes-localstorage

- Run id: `post-commit-suite-20260508-222823`
- Sample: `evalsets/live_tasks/frontend-book-notes-localstorage.yaml`
- Worktree: `target/live-evals/post-commit-suite-20260508-222823/frontend-book-notes-localstorage/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/post-commit-suite-20260508-222823/frontend-book-notes-localstorage/env`
- Test status: `failed`
- Generated: `2026-05-08 22:28:46 +0800`

## Git Status

```text
?? notes.md
```

## Diff Stat

```text
```

## Required Commands

```text
$ node fixtures/live_frontend/book_notes/test-book-notes.cjs
/Users/georgexu/Desktop/rust-agent/target/live-evals/post-commit-suite-20260508-222823/frontend-book-notes-localstorage/worktree/fixtures/live_frontend/book_notes/test-book-notes.cjs:40
assert.ok(first.id && second.id && first.id !== second.id, "notes need stable unique ids");
                ^

TypeError: Cannot read properties of null (reading 'id')
    at Object.<anonymous> (/Users/georgexu/Desktop/rust-agent/target/live-evals/post-commit-suite-20260508-222823/frontend-book-notes-localstorage/worktree/fixtures/live_frontend/book_notes/test-book-notes.cjs:40:17)
    at Module._compile (node:internal/modules/cjs/loader:1760:14)
    at Object..js (node:internal/modules/cjs/loader:1893:10)
    at Module.load (node:internal/modules/cjs/loader:1480:32)
    at Module._load (node:internal/modules/cjs/loader:1299:12)
    at TracingChannel.traceSync (node:diagnostics_channel:328:14)
    at wrapModuleLoad (node:internal/modules/cjs/loader:244:24)
    at Module.executeUserEntryPoint [as runMain] (node:internal/modules/run_main:154:5)
    at node:internal/main/run_main_module:33:47

Node.js v24.11.0
[exit status: 1]

$ ! rg 'TODO' fixtures/live_frontend/book_notes/app.js
    // TODO: restore notes from storage, tolerate malformed JSON.
    // TODO: persist notes to storage.
    // TODO: create a note with id, title, body, tags, createdAt.
    // TODO: remove a note by id and persist.
    // TODO: return newest-first notes filtered by title/body search and tag.
[exit status: 1]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-post-commit-suite-20260508-222823/frontend-book-notes-localstorage/agent-output.md`
- Events: `docs/benchmarks/live-post-commit-suite-20260508-222823/frontend-book-notes-localstorage/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
text_chunk: 1
trace_summary: 1
```

Quality signals:

```text
output_chars: 2497
diff_chars: 0
tool_executions: 0
first_write_tool_index: none
tool_errors: 0
tool_failures: 0
has_closeout: false
has_validation_claim: false
trace_status: Completed
trace_events: 11
test_status: failed
verification_passed: false
stage_validation_passed: false
acceptance_accepted: None
closeout_status: missing
runtime_diet: missing
adaptive_triggers: required_validation
trace_event_types: prompt,intent,resource.policy,workflow.trigger,workflow.fallback,task.context,reflection.pass,goal,workflow.route,workflow.done,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: seeded_code_change
warning: no_code_diff
warning: required_commands_not_passing
warning: closeout_not_successful
failure_owner: llm_reasoning
```

Specialty signals:

```text
memory_active: false
automation_active: true
guided_debugging_active: false
guided_reasoning_active: false
weighted_planning_active: false
closeout_active: false
adaptive_workflow_active: true
active_specialty_signals: 2/7
memory_sync_events: 0
memory_tool_calls: 0
retrieval_sources: none
required_commands: 2
required_command_status: failed
validation_events: 0
stage_validation_events: 0
tool_progress_events: 0
guided_debugging_events: 0
guided_reasoning_events: 0
workflow_plan_events: 0
weighted_plan_events: 0
reweighted_plan_events: 0
adaptive_trigger_events: 1
adaptive_triggers: required_validation
latest_top_priority: none
latest_top_importance_score: none
latest_top_weight_share: none
acceptance_accepted: missing
closeout_status: missing
runtime_diet: missing
attention: required commands did not pass in the harness
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
