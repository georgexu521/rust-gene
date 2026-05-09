# Live Eval Report: live-eval-dashboard-summary

- Run id: `checkpoint-sanitized-validation-20260509-105451`
- Sample: `evalsets/live_tasks/live-eval-dashboard-summary.yaml`
- Worktree: `target/live-evals/checkpoint-sanitized-validation-20260509-105451/live-eval-dashboard-summary/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-sanitized-validation-20260509-105451/live-eval-dashboard-summary/env`
- Test status: `failed`
- Generated: `2026-05-09 11:02:13 +0800`

## Git Status

```text
 M scripts/run_live_eval.sh
```

## Diff Stat

```text
 scripts/run_live_eval.sh | 189 +++++++++++++++++++++++++++++++++++++++++++++--
 1 file changed, 184 insertions(+), 5 deletions(-)
```

## Required Commands

```text
$ bash -n scripts/run_live_eval.sh
scripts/run_live_eval.sh: line 1493: syntax error near unexpected token `n/a'
scripts/run_live_eval.sh: line 1493: `    pass_rate="0/0 (n/a)"'
[exit status: 2]

$ scripts/run_live_eval.sh --list
scripts/run_live_eval.sh: line 1493: syntax error near unexpected token `n/a'
[exit status: 2]

$ scripts/run_live_eval.sh --mode summary --run-id live-summary-smoke
scripts/run_live_eval.sh: line 1493: syntax error near unexpected token `n/a'
[exit status: 2]

$ cargo test -q -- --test-threads=1

running 1118 tests
....................................................................................... 87/1118
....................................................................................... 174/1118
....................................................................................... 261/1118
....................................................................................... 348/1118
....................................................................................... 435/1118
....................................................................................... 522/1118
....................................................................................... 609/1118
....................................................................................... 696/1118
....................................................................................... 783/1118
....................................................................................... 870/1118
....................................................................................... 957/1118
....................................................................................... 1044/1118
..........................................................................
test result: ok. 1118 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 20.40s

[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-checkpoint-sanitized-validation-20260509-105451/live-eval-dashboard-summary/agent-output.md`
- Events: `docs/benchmarks/live-checkpoint-sanitized-validation-20260509-105451/live-eval-dashboard-summary/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 1
tool_execution_complete: 7
tool_execution_progress: 1
tool_execution_start: 7
trace_summary: 1
```

Quality signals:

```text
output_chars: 505
diff_chars: 6946
tool_executions: 7
first_write_tool_index: 7
tool_errors: 0
tool_failures: 0
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 53
test_status: failed
verification_passed: true
stage_validation_passed: true
acceptance_accepted: None
closeout_status: passed
runtime_diet: prompt=5013 tool_schema=2641 tools=12 workflow=guarded closeout=full validation=passed
adaptive_triggers: required_validation,repeated_no_code_progress,first_code_change
trace_event_types: tool.start,tool.done,workflow.trigger,workflow.fallback,verify.done,reflection.pass,stage.validation,memory.sync,workflow.fallback,closeout,runtime.diet,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: seeded_code_change
warning: required_commands_not_passing
failure_owner: eval_harness
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
memory_sync_events: 3
memory_tool_calls: 0
retrieval_sources: Project
required_commands: 4
required_command_status: failed
validation_events: 1
stage_validation_events: 1
tool_progress_events: 1
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
acceptance_accepted: missing
closeout_status: passed
runtime_diet: prompt=5013 tool_schema=2641 tools=12 workflow=guarded
attention: required commands did not pass in the harness
note: guided debugging is expected only after a blocker or failed validation
```

Agent stderr tail:

```text
2026-05-09T02:55:00.357692Z  WARN priority_agent::engine::conversation_loop: Workflow judgment analysis failed: key must be a string at line 1 column 2
2026-05-09T02:57:27.258718Z  WARN priority_agent::engine::conversation_loop: Patch synthesis JSON actions were not directly applicable: synthesized patch old_string was not found exactly in /Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-sanitized-validation-20260509-105451/live-eval-dashboard-summary/worktree/scripts/run_live_eval.sh; refusing inexact multi-line replacement
[required validation still running after 30s] cargo test -q -- --test-threads=1
[required validation still running after 60s] cargo test -q -- --test-threads=1
[required validation still running after 90s] cargo test -q -- --test-threads=1
[required validation still running after 120s] cargo test -q -- --test-threads=1
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
