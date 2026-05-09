# Live Eval Report: live-eval-dashboard-summary

- Run id: `checkpoint-required-commands-20260509-110945`
- Sample: `evalsets/live_tasks/live-eval-dashboard-summary.yaml`
- Worktree: `target/live-evals/checkpoint-required-commands-20260509-110945/live-eval-dashboard-summary/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-required-commands-20260509-110945/live-eval-dashboard-summary/env`
- Test status: `failed`
- Generated: `2026-05-09 11:23:52 +0800`

## Git Status

```text
 M scripts/run_live_eval.sh
```

## Diff Stat

```text
 scripts/run_live_eval.sh | 189 ++++++++++++++++++++++++++++++++++++++++++++---
 1 file changed, 177 insertions(+), 12 deletions(-)
```

## Required Commands

```text
$ bash -n scripts/run_live_eval.sh
scripts/run_live_eval.sh: line 1355: syntax error near unexpected token `**'
scripts/run_live_eval.sh: line 1355: `**summary_task()** {'
[exit status: 2]

$ scripts/run_live_eval.sh --list
scripts/run_live_eval.sh: line 1355: syntax error near unexpected token `**'
[exit status: 2]

$ scripts/run_live_eval.sh --mode summary --run-id live-summary-smoke
scripts/run_live_eval.sh: line 1355: syntax error near unexpected token `**'
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
test result: ok. 1118 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 20.69s

[exit status: 0]

```

## Agent Run

- Exit status: ``
- Events: `docs/benchmarks/live-checkpoint-required-commands-20260509-110945/live-eval-dashboard-summary/agent-events.jsonl`

Event counts:

```text
eval_started: 1
start: 1
tool_execution_complete: 6
tool_execution_progress: 2
tool_execution_start: 6
```

Quality signals:

```text
output_chars: 0
diff_chars: 7314
tool_executions: 6
first_write_tool_index: 4
tool_errors: 0
tool_failures: 0
has_closeout: false
has_validation_claim: false
trace_status: missing
trace_events: 0
test_status: failed
verification_passed: false
stage_validation_passed: false
acceptance_accepted: None
closeout_status: missing
runtime_diet: missing
adaptive_triggers: none
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: seeded_code_change
warning: empty_agent_output
warning: tool_run_without_closeout
warning: missing_trace_summary
warning: required_commands_not_passing
warning: closeout_not_successful
failure_owner: agent_flow
```

Specialty signals:

```text
memory_active: false
automation_active: true
guided_debugging_active: false
guided_reasoning_active: false
weighted_planning_active: false
closeout_active: false
adaptive_workflow_active: false
active_specialty_signals: 1/7
memory_sync_events: 0
memory_tool_calls: 0
retrieval_sources: none
required_commands: 4
required_command_status: failed
validation_events: 0
stage_validation_events: 0
tool_progress_events: 2
guided_debugging_events: 0
guided_reasoning_events: 0
workflow_plan_events: 0
weighted_plan_events: 0
reweighted_plan_events: 0
adaptive_trigger_events: 0
adaptive_triggers: none
latest_top_priority: none
latest_top_importance_score: none
latest_top_weight_share: none
acceptance_accepted: missing
closeout_status: missing
runtime_diet: missing
attention: required commands did not pass in the harness
note: guided debugging is expected only after a blocker or failed validation
```

Agent stderr tail:

```text
2026-05-09T03:11:41.185319Z  WARN priority_agent::engine::conversation_loop: Patch synthesis JSON actions were not directly applicable: synthesized patch old_string was not found exactly in /Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-required-commands-20260509-110945/live-eval-dashboard-summary/worktree/scripts/run_live_eval.sh; refusing inexact multi-line replacement; patch synthesis declined without a reason
[required validation still running after 30s] cargo test -q -- --test-threads=1
[required validation still running after 60s] cargo test -q -- --test-threads=1
[required validation still running after 90s] cargo test -q -- --test-threads=1
[required validation still running after 120s] cargo test -q -- --test-threads=1
2026-05-09T03:16:30.534583Z  WARN priority_agent::engine::conversation_loop: Patch synthesis JSON actions were not directly applicable: response was not valid patch JSON; patch synthesis declined without a reason
2026-05-09T03:20:45.992524Z  WARN priority_agent::engine::conversation_loop: Patch synthesis JSON actions were not directly applicable: synthesized patch old_string was not found exactly in /Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-required-commands-20260509-110945/live-eval-dashboard-summary/worktree/scripts/run_live_eval.sh; refusing inexact multi-line replacement
2026-05-09T03:22:41.714805Z  WARN priority_agent::engine::conversation_loop: Patch synthesis JSON actions were not directly applicable: synthesized patch old_string was not found exactly in /Users/georgexu/Desktop/rust-agent/target/live-evals/checkpoint-required-commands-20260509-110945/live-eval-dashboard-summary/worktree/scripts/run_live_eval.sh; refusing inexact multi-line replacement
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
