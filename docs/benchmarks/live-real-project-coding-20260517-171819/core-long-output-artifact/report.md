# Live Eval Report: core-long-output-artifact

- Run id: `real-project-coding-20260517-171819`
- Sample: `evalsets/live_tasks/core-long-output-artifact.yaml`
- Worktree: `target/live-evals/real-project-coding-20260517-171819/core-long-output-artifact/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/real-project-coding-20260517-171819/core-long-output-artifact/env`
- Test status: `ok`
- Generated: `2026-05-17 17:40:59 +0800`

## Git Status

```text
?? fixtures/core_quality/long_output/output.log
```

## Diff Stat

```text
```

## Required Commands

```text
$ test -s fixtures/core_quality/long_output/output.log
[exit status: 0]

$ rg 'line 0537 ERROR_ANCHOR payment retry budget exceeded' fixtures/core_quality/long_output/output.log
line 0537 ERROR_ANCHOR payment retry budget exceeded
[exit status: 0]

$ python3 -c 'from pathlib import Path; assert len(Path("fixtures/core_quality/long_output/output.log").read_text().splitlines()) == 800'
[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-real-project-coding-20260517-171819/core-long-output-artifact/agent-output.md`
- Events: `docs/benchmarks/live-real-project-coding-20260517-171819/core-long-output-artifact/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 1
tool_execution_complete: 2
tool_execution_progress: 1
tool_execution_start: 2
trace_summary: 1
```

Quality signals:

```text
output_chars: 808
diff_chars: 0
diff_files_changed: 0
tool_executions: 2
first_write_tool_index: none
forbidden_tool_uses: none
tool_errors: 0
tool_failures: 0
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 37
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=2132 tool_schema=3186 tools=15 workflow=guarded closeout=full validation=passed:4/4
adaptive_triggers: required_validation,first_code_change
trace_event_types: workflow.trigger,workflow.fallback,verify.done,reflection.pass,stage.validation,acceptance.review,workflow.plan,memory.sync,workflow.fallback,closeout,runtime.diet,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: audit_or_regression_check
behavior_assertions: none
behavior_assertion_status: none
warning: no_code_diff
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
required_commands: 3
agent_required_commands: 3
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
latest_top_importance_score: 0.7224999666213989
latest_top_weight_share: 0.3921302556991577
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=2132 tool_schema=3186 tools=15 workflow=guarded
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
