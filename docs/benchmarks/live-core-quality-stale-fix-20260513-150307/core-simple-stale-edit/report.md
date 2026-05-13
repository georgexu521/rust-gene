# Live Eval Report: core-simple-stale-edit

- Run id: `core-quality-stale-fix-20260513-150307`
- Sample: `evalsets/live_tasks/core-simple-stale-edit.yaml`
- Worktree: `target/live-evals/core-quality-stale-fix-20260513-150307/core-simple-stale-edit/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/core-quality-stale-fix-20260513-150307/core-simple-stale-edit/env`
- Test status: `ok`
- Generated: `2026-05-13 15:05:45 +0800`

## Git Status

```text
 M fixtures/core_quality/simple_edit/settings.py
?? fixtures/core_quality/simple_edit/__pycache__/
```

## Diff Stat

```text
 fixtures/core_quality/simple_edit/settings.py | 2 +-
 1 file changed, 1 insertion(+), 1 deletion(-)
```

## Required Commands

```text
$ python3 fixtures/core_quality/simple_edit/test_settings.py
.
----------------------------------------------------------------------
Ran 1 test in 0.000s

OK
[exit status: 0]

$ rg 'DEFAULT_TIMEOUT = 10' fixtures/core_quality/simple_edit/settings.py
DEFAULT_TIMEOUT = 10
[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-core-quality-stale-fix-20260513-150307/core-simple-stale-edit/agent-output.md`
- Events: `docs/benchmarks/live-core-quality-stale-fix-20260513-150307/core-simple-stale-edit/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 1
tool_execution_complete: 3
tool_execution_progress: 1
tool_execution_start: 3
trace_summary: 1
```

Quality signals:

```text
output_chars: 476
diff_chars: 328
diff_files_changed: 1
tool_executions: 3
first_write_tool_index: 3
forbidden_tool_uses: none
tool_errors: 0
tool_failures: 0
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 32
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: None
closeout_status: passed
runtime_diet: prompt=1983 tool_schema=3186 tools=15 workflow=guarded closeout=full validation=passed:3/3
adaptive_triggers: first_code_change
trace_event_types: tool.start,tool.done,workflow.trigger,workflow.fallback,verify.done,reflection.pass,stage.validation,memory.sync,workflow.fallback,closeout,runtime.diet,assistant
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
guided_reasoning_active: false
weighted_planning_active: false
closeout_active: false
adaptive_workflow_active: true
active_specialty_signals: 3/7
memory_sync_events: 2
memory_tool_calls: 0
retrieval_sources: Project
required_commands: 2
agent_required_commands: 2
harness_commands: 0
required_command_status: ok
validation_events: 1
stage_validation_events: 1
tool_progress_events: 1
guided_debugging_events: 0
guided_reasoning_events: 0
workflow_plan_events: 0
weighted_plan_events: 0
reweighted_plan_events: 0
adaptive_trigger_events: 1
adaptive_triggers: first_code_change
latest_top_priority: none
latest_top_importance_score: none
latest_top_weight_share: none
acceptance_accepted: missing
closeout_status: passed
runtime_diet: prompt=1983 tool_schema=3186 tools=15 workflow=guarded
note: guided debugging is expected only after a blocker or failed validation
```

Agent stderr tail:

```text
2026-05-13T07:05:12.535095Z  WARN priority_agent::services::api::retry: Provider request failed transiently; reconnecting 1/5 for MiniMax chat.completions after 572ms: http error: error sending request for url (https://api.minimaxi.com/v1/chat/completions)
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
