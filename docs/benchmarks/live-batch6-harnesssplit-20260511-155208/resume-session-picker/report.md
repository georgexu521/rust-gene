# Live Eval Report: resume-session-picker

- Run id: `batch6-harnesssplit-20260511-155208`
- Sample: `evalsets/live_tasks/resume-session-picker.yaml`
- Worktree: `target/live-evals/batch6-harnesssplit-20260511-155208/resume-session-picker/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/batch6-harnesssplit-20260511-155208/resume-session-picker/env`
- Test status: `ok`
- Generated: `2026-05-11 15:58:45 +0800`

## Git Status

```text
 M src/tui/app.rs
```

## Diff Stat

```text
 src/tui/app.rs | 28 ++++++++++++++++++++++++++--
 1 file changed, 26 insertions(+), 2 deletions(-)
```

## Required Commands

```text
$ cargo test -q resume -- --test-threads=1

running 1 test
.
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 910 filtered out; finished in 0.00s

[exit status: 0]

$ cargo test -q session -- --test-threads=1

running 44 tests
............................................
test result: ok. 44 passed; 0 failed; 0 ignored; 0 measured; 867 filtered out; finished in 0.07s

[exit status: 0]

$ cargo test -q -- --test-threads=1

running 911 tests
....................................................................................... 87/911
....................................................................................... 174/911
....................................................................................... 261/911
....................................................................................... 348/911
....................................................................................... 435/911
....................................................................................... 522/911
....................................................................................... 609/911
....................................................................................... 696/911
....................................................................................... 783/911
....................................................................................... 870/911
.........................................
test result: ok. 911 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 21.48s

[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-batch6-harnesssplit-20260511-155208/resume-session-picker/agent-output.md`
- Events: `docs/benchmarks/live-batch6-harnesssplit-20260511-155208/resume-session-picker/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 1
tool_execution_complete: 15
tool_execution_progress: 1
tool_execution_start: 15
trace_summary: 1
```

Quality signals:

```text
output_chars: 1304
diff_chars: 1729
tool_executions: 15
first_write_tool_index: 15
tool_errors: 0
tool_failures: 0
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 82
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=17518 tool_schema=2641 tools=12 workflow=guarded closeout=full validation=passed:5/5
adaptive_triggers: required_validation,repeated_no_code_progress,first_code_change
trace_event_types: workflow.trigger,workflow.fallback,verify.done,reflection.pass,stage.validation,acceptance.review,workflow.plan,memory.sync,workflow.fallback,closeout,runtime.diet,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: seeded_code_change
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
memory_sync_events: 5
memory_tool_calls: 0
retrieval_sources: Project,Session
required_commands: 3
agent_required_commands: 2
harness_commands: 1
required_command_status: ok
validation_events: 1
stage_validation_events: 1
tool_progress_events: 1
guided_debugging_events: 0
guided_reasoning_events: 1
workflow_plan_events: 2
weighted_plan_events: 1
reweighted_plan_events: 1
adaptive_trigger_events: 3
adaptive_triggers: required_validation,repeated_no_code_progress,first_code_change
latest_top_priority: P3
latest_top_importance_score: 0.23000001907348633
latest_top_weight_share: 0.1684981882572174
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=17518 tool_schema=2641 tools=12 workflow=guarded
note: guided debugging is expected only after a blocker or failed validation
```

Agent stderr tail:

```text
2026-05-11T07:52:39.610575Z  WARN priority_agent::services::api::retry: Provider request failed transiently; reconnecting 1/5 for MiniMax chat.completions after 647ms: http error: error sending request for url (https://api.minimaxi.com/v1/chat/completions)
2026-05-11T07:52:43.263043Z  WARN priority_agent::services::api::retry: Provider request failed transiently; reconnecting 2/5 for MiniMax chat.completions after 1.087s: http error: error sending request for url (https://api.minimaxi.com/v1/chat/completions)
2026-05-11T07:52:47.356856Z  WARN priority_agent::services::api::retry: Provider request failed transiently; reconnecting 3/5 for MiniMax chat.completions after 2.217s: http error: error sending request for url (https://api.minimaxi.com/v1/chat/completions)
2026-05-11T07:52:52.580466Z  WARN priority_agent::services::api::retry: Provider request failed transiently; reconnecting 4/5 for MiniMax chat.completions after 4.228s: http error: error sending request for url (https://api.minimaxi.com/v1/chat/completions)
2026-05-11T07:52:59.814752Z  WARN priority_agent::services::api::retry: Provider request failed transiently; reconnecting 5/5 for MiniMax chat.completions after 8.249s: http error: error sending request for url (https://api.minimaxi.com/v1/chat/completions)
[required validation still running after 30s] cargo test -q resume -- --test-threads=1
[required validation still running after 60s] cargo test -q resume -- --test-threads=1
[required validation still running after 90s] cargo test -q resume -- --test-threads=1
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
