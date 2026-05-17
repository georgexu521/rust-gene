# Live Eval Report: core-simple-stale-edit

- Run id: `real-project-coding-20260517-183221`
- Sample: `evalsets/live_tasks/core-simple-stale-edit.yaml`
- Worktree: `target/live-evals/real-project-coding-20260517-183221/core-simple-stale-edit/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/real-project-coding-20260517-183221/core-simple-stale-edit/env`
- Test status: `ok`
- Generated: `2026-05-17 18:42:06 +0800`

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
- Output: `docs/benchmarks/live-real-project-coding-20260517-183221/core-simple-stale-edit/agent-output.md`
- Events: `docs/benchmarks/live-real-project-coding-20260517-183221/core-simple-stale-edit/agent-events.jsonl`

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
output_chars: 806
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
trace_events: 38
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=2272 tool_schema=3186 tools=15 workflow=guarded closeout=full validation=passed:4/4
adaptive_triggers: required_validation,first_code_change
trace_event_types: workflow.trigger,workflow.fallback,verify.done,reflection.pass,stage.validation,acceptance.review,workflow.plan,memory.sync,workflow.fallback,closeout,runtime.diet,assistant
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
weighted_planning_active: true
closeout_active: true
adaptive_workflow_active: true
active_specialty_signals: 5/7
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
workflow_plan_events: 2
weighted_plan_events: 1
reweighted_plan_events: 1
adaptive_trigger_events: 2
adaptive_triggers: required_validation,first_code_change
latest_top_priority: P1
latest_top_importance_score: 0.7900000214576721
latest_top_weight_share: 0.3526785671710968
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=2272 tool_schema=3186 tools=15 workflow=guarded
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
