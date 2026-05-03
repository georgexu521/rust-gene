# Live Eval Report: live-eval-dashboard-summary

- Run id: `capability-dashboard-summary-20260503-213148`
- Sample: `evalsets/live_tasks/live-eval-dashboard-summary.yaml`
- Worktree: `target/live-evals/capability-dashboard-summary-20260503-213148/live-eval-dashboard-summary/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/capability-dashboard-summary-20260503-213148/live-eval-dashboard-summary/env`
- Test status: `failed`
- Generated: `2026-05-03 21:35:22 +0800`

## Git Status

```text
```

## Diff Stat

```text
```

## Required Commands

```text
$ bash -n scripts/run_live_eval.sh
[exit status: 0]

$ scripts/run_live_eval.sh --list
PyYAML is required for live eval parsing: No module named 'yaml'
[exit status: 1]

$ cargo test -q -- --test-threads=1

running 1057 tests
....................................................................................... 87/1057
....................................................................................... 174/1057
....................................................................................... 261/1057
....................................................................................... 348/1057
....................................................................................... 435/1057
....................................................................................... 522/1057
....................................................................................... 609/1057
....................................................................................... 696/1057
....................................................................................... 783/1057
....................................................................................... 870/1057
....................................................................................... 957/1057
....................................................................................... 1044/1057
.............
test result: ok. 1057 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 44.45s

[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-capability-dashboard-summary-20260503-213148/live-eval-dashboard-summary/agent-output.md`
- Events: `docs/benchmarks/live-capability-dashboard-summary-20260503-213148/live-eval-dashboard-summary/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 2
tool_execution_complete: 14
tool_execution_start: 14
trace_summary: 1
```

Quality signals:

```text
output_chars: 1054
diff_chars: 0
tool_executions: 14
first_write_tool_index: none
tool_errors: 0
tool_failures: 2
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 87
test_status: failed
verification_passed: false
stage_validation_passed: false
acceptance_accepted: None
closeout_status: not_verified
trace_event_types: tool.done,memory.sync,api.start,workflow.fallback,api.done,tool.start,tool.done,workflow.fallback,workflow.fallback,workflow.fallback,closeout,assistant
stale_edit_warnings: 0
eval_intent: seeded_code_change
warning: no_code_diff
warning: required_commands_not_passing
warning: closeout_not_successful
failure_owner: llm_reasoning
```

Specialty signals:

```text
memory_active: true
automation_active: true
guided_debugging_active: false
guided_reasoning_active: true
weighted_planning_active: true
closeout_active: false
active_specialty_signals: 4/6
memory_sync_events: 7
memory_tool_calls: 0
retrieval_sources: Project
required_commands: 3
required_command_status: failed
validation_events: 0
stage_validation_events: 0
tool_progress_events: 0
guided_debugging_events: 0
guided_reasoning_events: 1
workflow_plan_events: 1
weighted_plan_events: 1
reweighted_plan_events: 0
latest_top_priority: P3
latest_top_importance_score: 0.39499998092651367
latest_top_weight_share: 0.36915889382362366
acceptance_accepted: missing
closeout_status: not_verified
attention: required commands did not pass in the harness
note: guided debugging is expected only after a blocker or failed validation
```

## Human Review

- accepted: false
- task_success: false
- mainline_hit: missed
- plan_coverage: partial
- rework_count: 0
- tool_efficiency: poor
- diff_discipline: failed
- closeout_accuracy: accurate
- notes: Seeded code-change task failed. The agent inspected
  `scripts/run_live_eval.sh` and the task YAML repeatedly but never produced a
  model-led edit. The action checkpoint blocked false success with
  `closeout_status=not_verified`. Required commands also failed because the
  isolated worktree could not import PyYAML for `scripts/run_live_eval.sh
  --list`; full Rust tests still passed (`1057 passed; 0 failed`).
