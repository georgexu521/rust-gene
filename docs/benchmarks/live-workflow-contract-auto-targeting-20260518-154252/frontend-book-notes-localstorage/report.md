# Live Eval Report: frontend-book-notes-localstorage

- Run id: `workflow-contract-auto-targeting-20260518-154252`
- Sample: `evalsets/live_tasks/frontend-book-notes-localstorage.yaml`
- Worktree: `target/live-evals/workflow-contract-auto-targeting-20260518-154252/frontend-book-notes-localstorage/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/workflow-contract-auto-targeting-20260518-154252/frontend-book-notes-localstorage/env`
- Test status: `ok`
- Generated: `2026-05-18 17:41:59 +0800`

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
- Output: `docs/benchmarks/live-workflow-contract-auto-targeting-20260518-154252/frontend-book-notes-localstorage/agent-output.md`
- Events: `docs/benchmarks/live-workflow-contract-auto-targeting-20260518-154252/frontend-book-notes-localstorage/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 1
tool_execution_complete: 7
tool_execution_progress: 3
tool_execution_start: 7
trace_summary: 1
```

Quality signals:

```text
output_chars: 700
diff_chars: 2780
diff_files_changed: 1
tool_executions: 7
first_write_tool_index: 7
forbidden_tool_uses: none
tool_errors: 0
tool_failures: 0
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 56
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
closeout_tool_records: 5
closeout_tool_evidence: tool evidence: records=5 completed=5 failed=0 denied=0 validation=0 closeout=1 repair=1 changed=1 workflows=code_change commands=none
runtime_diet: prompt=5397 tool_schema=3186 tools=15 workflow=guarded closeout=full validation=passed:2/2
adaptive_triggers: required_validation,repeated_no_code_progress,first_code_change
trace_event_types: tool.done,workflow.trigger,workflow.fallback,verify.done,reflection.pass,stage.validation,acceptance.review,memory.sync,workflow.fallback,closeout,runtime.diet,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: seeded_code_change
behavior_assertions: none
behavior_assertion_status: none
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
workflow_contract_activation: entry=skipped:auto repair=none
workflow_contract_events: 1
memory_sync_events: 4
memory_tool_calls: 0
retrieval_sources: Project,Session
required_commands: 2
agent_required_commands: 2
harness_commands: 0
required_command_status: ok
validation_events: 1
stage_validation_events: 1
tool_progress_events: 3
guided_debugging_events: 0
guided_reasoning_events: 0
workflow_plan_events: 0
weighted_plan_events: 0
reweighted_plan_events: 0
adaptive_trigger_events: 3
adaptive_triggers: required_validation,repeated_no_code_progress,first_code_change
latest_top_priority: none
latest_top_importance_score: none
latest_top_weight_share: none
acceptance_accepted: True
closeout_status: passed
closeout_tool_records: 5
closeout_tool_evidence: tool evidence: records=5 completed=5 failed=0 denied=0 validation=0 closeout=1 repair=1 changed=1 workflows=code_change commands=none
runtime_diet: prompt=5397 tool_schema=3186 tools=15 workflow=guarded
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
