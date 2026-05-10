# Live Eval Report: memory-recall-conflict-precision

- Run id: `batch6-rerun-20260510-230329`
- Sample: `evalsets/live_tasks/memory-recall-conflict-precision.yaml`
- Worktree: `target/live-evals/batch6-rerun-20260510-230329/memory-recall-conflict-precision/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/batch6-rerun-20260510-230329/memory-recall-conflict-precision/env`
- Test status: `ok`
- Generated: `2026-05-10 23:10:26 +0800`

## Git Status

```text
```

## Diff Stat

```text
```

## Required Commands

```text
$ cargo test -q retrieval_context -- --test-threads=1

running 9 tests
.........
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 1174 filtered out; finished in 0.01s

[exit status: 0]

$ cargo test -q memory::recall::tests:: -- --test-threads=1

running 1 test
.
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1182 filtered out; finished in 0.00s

[exit status: 0]

$ cargo test -q -- --test-threads=1

running 1183 tests
....................................................................................... 87/1183
....................................................................................... 174/1183
....................................................................................... 261/1183
....................................................................................... 348/1183
....................................................................................... 435/1183
....................................................................................... 522/1183
....................................................................................... 609/1183
....................................................................................... 696/1183
....................................................................................... 783/1183
....................................................................................... 870/1183
....................................................................................... 957/1183
....................................................................................... 1044/1183
....................................................................................... 1131/1183
....................................................
test result: ok. 1183 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 22.79s

[exit status: 0]

```

## Agent Run

- Exit status: `1`
- Output: `docs/benchmarks/live-batch6-rerun-20260510-230329/memory-recall-conflict-precision/agent-output.md`
- Events: `docs/benchmarks/live-batch6-rerun-20260510-230329/memory-recall-conflict-precision/agent-events.jsonl`

Event counts:

```text
error: 1
eval_started: 1
start: 1
tool_execution_complete: 8
tool_execution_progress: 3
tool_execution_start: 8
trace_summary: 1
```

Quality signals:

```text
output_chars: 0
diff_chars: 0
tool_executions: 8
first_write_tool_index: none
tool_errors: 0
tool_failures: 0
has_closeout: false
has_validation_claim: false
trace_status: Failed
trace_events: 50
test_status: ok
verification_passed: false
stage_validation_passed: false
acceptance_accepted: None
closeout_status: missing
runtime_diet: prompt=8240 tool_schema=2641 tools=12 workflow=strict closeout=none validation=api_error
adaptive_triggers: required_validation
trace_event_types: workflow.fallback,api.done,tool.start,tool.done,tool.start,tool.done,workflow.fallback,memory.sync,api.start,workflow.fallback,error,runtime.diet
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: audit_or_regression_check
warning: empty_agent_output
warning: tool_run_without_closeout
warning: no_code_diff
warning: current_head_no_fixture_already_satisfied
warning: closeout_not_successful
failure_owner: environment
```

Specialty signals:

```text
memory_active: true
automation_active: true
guided_debugging_active: false
guided_reasoning_active: true
weighted_planning_active: true
closeout_active: false
adaptive_workflow_active: true
active_specialty_signals: 5/7
memory_sync_events: 4
memory_tool_calls: 0
retrieval_sources: Project,Session
required_commands: 3
required_command_status: ok
validation_events: 0
stage_validation_events: 0
tool_progress_events: 3
guided_debugging_events: 0
guided_reasoning_events: 1
workflow_plan_events: 1
weighted_plan_events: 1
reweighted_plan_events: 0
adaptive_trigger_events: 1
adaptive_triggers: required_validation
latest_top_priority: P1
latest_top_importance_score: 0.7549999952316284
latest_top_weight_share: 0.1431279480457306
acceptance_accepted: missing
closeout_status: missing
runtime_diet: prompt=8240 tool_schema=2641 tools=12 workflow=strict
note: guided debugging is expected only after a blocker or failed validation
```

Agent stderr tail:

```text
2026-05-10T15:09:37.368132Z ERROR priority_agent: Evaluation run failed: Failed to get response from MiniMax API: http error: error sending request for url (https://api.minimaxi.com/v1/chat/completions) (error body unavailable)
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
