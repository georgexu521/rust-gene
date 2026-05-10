# Live Eval Report: memory-save-sensitive-hard-block

- Run id: `batch6-rerun-20260510-232124`
- Sample: `evalsets/live_tasks/memory-save-sensitive-hard-block.yaml`
- Worktree: `target/live-evals/batch6-rerun-20260510-232124/memory-save-sensitive-hard-block/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/batch6-rerun-20260510-232124/memory-save-sensitive-hard-block/env`
- Test status: `ok`
- Generated: `2026-05-10 23:25:53 +0800`

## Git Status

```text
```

## Diff Stat

```text
```

## Required Commands

```text
$ cargo test -q memory -- --test-threads=1

running 95 tests
....................................................................................... 87/95
........
test result: ok. 95 passed; 0 failed; 0 ignored; 0 measured; 1089 filtered out; finished in 0.17s

[exit status: 0]

$ cargo test -q tui::app::tests:: -- --test-threads=1

running 38 tests
......................................
test result: ok. 38 passed; 0 failed; 0 ignored; 0 measured; 1146 filtered out; finished in 0.13s

[exit status: 0]

$ cargo test -q -- --test-threads=1

running 1184 tests
....................................................................................... 87/1184
....................................................................................... 174/1184
....................................................................................... 261/1184
....................................................................................... 348/1184
....................................................................................... 435/1184
....................................................................................... 522/1184
....................................................................................... 609/1184
....................................................................................... 696/1184
....................................................................................... 783/1184
....................................................................................... 870/1184
....................................................................................... 957/1184
....................................................................................... 1044/1184
....................................................................................... 1131/1184
.....................................................
test result: ok. 1184 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 50.07s

[exit status: 0]

```

## Agent Run

- Exit status: `1`
- Output: `docs/benchmarks/live-batch6-rerun-20260510-232124/memory-save-sensitive-hard-block/agent-output.md`
- Events: `docs/benchmarks/live-batch6-rerun-20260510-232124/memory-save-sensitive-hard-block/agent-events.jsonl`

Event counts:

```text
error: 1
eval_started: 1
start: 1
trace_summary: 1
```

Quality signals:

```text
output_chars: 0
diff_chars: 0
tool_executions: 0
first_write_tool_index: none
tool_errors: 0
tool_failures: 0
has_closeout: false
has_validation_claim: false
trace_status: Failed
trace_events: 17
test_status: ok
verification_passed: false
stage_validation_passed: false
acceptance_accepted: None
closeout_status: missing
runtime_diet: prompt=2704 tool_schema=2641 tools=12 workflow=strict closeout=none validation=api_error
adaptive_triggers: required_validation
trace_event_types: workflow.fallback,workflow.judgment,workflow.plan,task.context,implementation.intent,reflection.pass,goal,workflow.route,api.start,workflow.fallback,error,runtime.diet
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: audit_or_regression_check
warning: empty_agent_output
warning: no_code_diff
warning: current_head_no_fixture_already_satisfied
warning: closeout_not_successful
failure_owner: environment
```

Specialty signals:

```text
memory_active: false
automation_active: true
guided_debugging_active: false
guided_reasoning_active: true
weighted_planning_active: true
closeout_active: false
adaptive_workflow_active: true
active_specialty_signals: 4/7
memory_sync_events: 0
memory_tool_calls: 0
retrieval_sources: Project,Session
required_commands: 3
required_command_status: ok
validation_events: 0
stage_validation_events: 0
tool_progress_events: 0
guided_debugging_events: 0
guided_reasoning_events: 1
workflow_plan_events: 1
weighted_plan_events: 1
reweighted_plan_events: 0
adaptive_trigger_events: 1
adaptive_triggers: required_validation
latest_top_priority: P0
latest_top_importance_score: 0.8500000238418579
latest_top_weight_share: 0.22457066178321838
acceptance_accepted: missing
closeout_status: missing
runtime_diet: prompt=2704 tool_schema=2641 tools=12 workflow=strict
note: guided debugging is expected only after a blocker or failed validation
```

Agent stderr tail:

```text
2026-05-10T15:23:18.934197Z ERROR priority_agent: Evaluation run failed: Failed to get response from MiniMax API: http error: error sending request for url (https://api.minimaxi.com/v1/chat/completions) (error body unavailable)
Evaluation run failed: Failed to get response from MiniMax API: http error: error sending request for url (https://api.minimaxi.com/v1/chat/completions) (error body unavailable)
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
