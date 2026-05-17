# Live Eval Report: core-terminal-install-run

- Run id: `real-project-coding-20260517-183221`
- Sample: `evalsets/live_tasks/core-terminal-install-run.yaml`
- Worktree: `target/live-evals/real-project-coding-20260517-183221/core-terminal-install-run/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/real-project-coding-20260517-183221/core-terminal-install-run/env`
- Test status: `ok`
- Generated: `2026-05-17 18:43:37 +0800`

## Git Status

```text
?? .venv/
?? fixtures/core_quality/terminal_app/core_terminal_demo.egg-info/
?? fixtures/core_quality/terminal_app/core_terminal_demo/__pycache__/
```

## Diff Stat

```text
```

## Required Commands

```text
$ test -x .venv/bin/python
[exit status: 0]

$ . .venv/bin/activate && python -m core_terminal_demo --self-test | rg '^core-terminal-demo-ok$'
core-terminal-demo-ok
[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-real-project-coding-20260517-183221/core-terminal-install-run/agent-output.md`
- Events: `docs/benchmarks/live-real-project-coding-20260517-183221/core-terminal-install-run/agent-events.jsonl`

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
output_chars: 1719
diff_chars: 0
diff_files_changed: 0
tool_executions: 9
first_write_tool_index: none
forbidden_tool_uses: none
tool_errors: 1
tool_failures: 1
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 58
test_status: ok
verification_passed: false
stage_validation_passed: false
acceptance_accepted: None
closeout_status: not_verified
runtime_diet: prompt=3005 tool_schema=3186 tools=15 workflow=guarded closeout=full validation=failed:1/2
adaptive_triggers: required_validation
trace_event_types: api.done,tool.start,tool.done,tool.start,tool.done,memory.sync,api.start,workflow.fallback,api.done,closeout,runtime.diet,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: audit_or_regression_check
behavior_assertions: none
behavior_assertion_status: none
warning: no_code_diff
warning: tool_errors_seen
warning: closeout_not_successful
failure_owner: agent_flow
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
memory_sync_events: 5
memory_tool_calls: 0
retrieval_sources: Project
required_commands: 2
agent_required_commands: 2
harness_commands: 0
required_command_status: ok
validation_events: 0
stage_validation_events: 0
tool_progress_events: 6
guided_debugging_events: 0
guided_reasoning_events: 0
workflow_plan_events: 1
weighted_plan_events: 1
reweighted_plan_events: 0
adaptive_trigger_events: 1
adaptive_triggers: required_validation
latest_top_priority: P3
latest_top_importance_score: 0.2200000137090683
latest_top_weight_share: 0.16541354358196259
acceptance_accepted: missing
closeout_status: not_verified
runtime_diet: prompt=3005 tool_schema=3186 tools=15 workflow=guarded
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
