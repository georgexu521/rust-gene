# Live Eval Report: core-provider-roundtrip

- Run id: `real-project-coding-20260517-171819`
- Sample: `evalsets/live_tasks/core-provider-roundtrip.yaml`
- Worktree: `target/live-evals/real-project-coding-20260517-171819/core-provider-roundtrip/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/real-project-coding-20260517-171819/core-provider-roundtrip/env`
- Test status: `ok`
- Generated: `2026-05-17 17:44:12 +0800`

## Git Status

```text
```

## Diff Stat

```text
```

## Required Commands

```text
$ cargo test -q provider_health -- --test-threads=1

running 3 tests
...
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 1443 filtered out; finished in 0.00s

[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-real-project-coding-20260517-171819/core-provider-roundtrip/agent-output.md`
- Events: `docs/benchmarks/live-real-project-coding-20260517-171819/core-provider-roundtrip/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
permission_request: 1
start: 1
text_chunk: 2
tool_execution_complete: 6
tool_execution_progress: 1
tool_execution_start: 6
trace_summary: 1
```

Quality signals:

```text
output_chars: 2426
diff_chars: 0
diff_files_changed: 0
tool_executions: 6
first_write_tool_index: none
forbidden_tool_uses: none
tool_errors: 1
tool_failures: 1
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 52
test_status: ok
verification_passed: false
stage_validation_passed: false
acceptance_accepted: None
closeout_status: not_verified
runtime_diet: prompt=7264 tool_schema=3186 tools=15 workflow=guarded closeout=full validation=failed:1/2
adaptive_triggers: required_validation
trace_event_types: tool.start,permission.request,permission.resolve,tool.done,guided.debug,memory.sync,api.start,workflow.fallback,api.done,closeout,runtime.diet,assistant
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
guided_debugging_active: true
guided_reasoning_active: false
weighted_planning_active: true
closeout_active: true
adaptive_workflow_active: true
active_specialty_signals: 6/7
memory_sync_events: 4
memory_tool_calls: 0
retrieval_sources: Project
required_commands: 1
agent_required_commands: 1
harness_commands: 0
required_command_status: ok
validation_events: 0
stage_validation_events: 0
tool_progress_events: 1
guided_debugging_events: 1
guided_reasoning_events: 0
workflow_plan_events: 1
weighted_plan_events: 1
reweighted_plan_events: 0
adaptive_trigger_events: 1
adaptive_triggers: required_validation
latest_top_priority: P1
latest_top_importance_score: 0.6200000643730164
latest_top_weight_share: 0.8611111044883728
acceptance_accepted: missing
closeout_status: not_verified
runtime_diet: prompt=7264 tool_schema=3186 tools=15 workflow=guarded
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
