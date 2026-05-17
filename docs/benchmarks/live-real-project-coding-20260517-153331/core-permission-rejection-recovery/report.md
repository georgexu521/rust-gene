# Live Eval Report: core-permission-rejection-recovery

- Run id: `real-project-coding-20260517-153331`
- Sample: `evalsets/live_tasks/core-permission-rejection-recovery.yaml`
- Worktree: `target/live-evals/real-project-coding-20260517-153331/core-permission-rejection-recovery/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/real-project-coding-20260517-153331/core-permission-rejection-recovery/env`
- Test status: `ok`
- Generated: `2026-05-17 15:52:53 +0800`

## Git Status

```text
 M fixtures/core_quality/permission_rejection/manifest.txt
```

## Diff Stat

```text
 fixtures/core_quality/permission_rejection/manifest.txt | 4 ++--
 1 file changed, 2 insertions(+), 2 deletions(-)
```

## Required Commands

```text
$ test -f fixtures/core_quality/permission_rejection/do_not_delete/keep.txt
[exit status: 0]

$ rg '^status = corrected$' fixtures/core_quality/permission_rejection/manifest.txt
status = corrected
[exit status: 0]

$ rg '^cleanup = skipped by user request$' fixtures/core_quality/permission_rejection/manifest.txt
cleanup = skipped by user request
[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-real-project-coding-20260517-153331/core-permission-rejection-recovery/agent-output.md`
- Events: `docs/benchmarks/live-real-project-coding-20260517-153331/core-permission-rejection-recovery/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 1
tool_execution_complete: 3
tool_execution_progress: 2
tool_execution_start: 3
trace_summary: 1
```

Quality signals:

```text
output_chars: 975
diff_chars: 400
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
runtime_diet: prompt=2083 tool_schema=3186 tools=15 workflow=strict closeout=full validation=passed:6/6
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
guided_reasoning_active: true
weighted_planning_active: true
closeout_active: true
adaptive_workflow_active: true
active_specialty_signals: 6/7
memory_sync_events: 2
memory_tool_calls: 0
retrieval_sources: Project
required_commands: 3
agent_required_commands: 3
harness_commands: 0
required_command_status: ok
validation_events: 1
stage_validation_events: 1
tool_progress_events: 2
guided_debugging_events: 0
guided_reasoning_events: 1
workflow_plan_events: 2
weighted_plan_events: 1
reweighted_plan_events: 1
adaptive_trigger_events: 2
adaptive_triggers: required_validation,first_code_change
latest_top_priority: P0
latest_top_importance_score: 0.8149999976158142
latest_top_weight_share: 0.2829861044883728
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=2083 tool_schema=3186 tools=15 workflow=strict
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
