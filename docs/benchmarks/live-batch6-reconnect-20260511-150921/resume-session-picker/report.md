# Live Eval Report: resume-session-picker

- Run id: `batch6-reconnect-20260511-150921`
- Sample: `evalsets/live_tasks/resume-session-picker.yaml`
- Worktree: `target/live-evals/batch6-reconnect-20260511-150921/resume-session-picker/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/batch6-reconnect-20260511-150921/resume-session-picker/env`
- Test status: `ok`
- Generated: `2026-05-11 15:27:59 +0800`

## Git Status

```text
 M src/tui/app.rs
 M src/tui/slash_handler/session.rs
```

## Diff Stat

```text
 src/tui/app.rs                   |  3 ++
 src/tui/slash_handler/session.rs | 65 ++++++++++++++++++++++++++++++++++++++--
 2 files changed, 65 insertions(+), 3 deletions(-)
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
test result: ok. 44 passed; 0 failed; 0 ignored; 0 measured; 867 filtered out; finished in 0.08s

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
test result: ok. 911 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 18.14s

[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-batch6-reconnect-20260511-150921/resume-session-picker/agent-output.md`
- Events: `docs/benchmarks/live-batch6-reconnect-20260511-150921/resume-session-picker/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 1
tool_execution_complete: 21
tool_execution_progress: 11
tool_execution_start: 21
trace_summary: 1
```

Quality signals:

```text
output_chars: 4690
diff_chars: 4470
tool_executions: 21
first_write_tool_index: 12
tool_errors: 0
tool_failures: 2
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 177
test_status: ok
verification_passed: false
stage_validation_passed: false
acceptance_accepted: False
closeout_status: failed
runtime_diet: prompt=47905 tool_schema=2641 tools=12 workflow=strict closeout=full validation=failed:22/50
adaptive_triggers: required_validation,repeated_no_code_progress,first_code_change,verification_failed,acceptance_rejected
trace_event_types: tool.start,tool.done,verify.done,reflection.pass,stage.validation,guided.debug,acceptance.review,workflow.fallback,memory.sync,closeout,runtime.diet,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: seeded_code_change
warning: earlier_verification_failed_before_repair
warning: earlier_stage_validation_failed_before_repair
warning: closeout_not_successful
warning: acceptance_review_rejected
warning: stage_validation_failed
warning: verification_failed
warning: recovered_acceptance_review_rejected
warning: recovered_stage_validation_failed
warning: recovered_verification_failed
failure_owner: agent_flow
```

Specialty signals:

```text
memory_active: true
automation_active: true
guided_debugging_active: true
guided_reasoning_active: true
weighted_planning_active: true
closeout_active: true
adaptive_workflow_active: true
active_specialty_signals: 7/7
memory_sync_events: 12
memory_tool_calls: 0
retrieval_sources: Project,Session
required_commands: 3
required_command_status: ok
validation_events: 8
stage_validation_events: 8
tool_progress_events: 11
guided_debugging_events: 8
guided_reasoning_events: 1
workflow_plan_events: 1
weighted_plan_events: 1
reweighted_plan_events: 0
adaptive_trigger_events: 5
adaptive_triggers: required_validation,repeated_no_code_progress,first_code_change,verification_failed,acceptance_rejected
latest_top_priority: P2
latest_top_importance_score: 0.512499988079071
latest_top_weight_share: 0.23295453190803528
acceptance_accepted: False
closeout_status: failed
runtime_diet: prompt=47905 tool_schema=2641 tools=12 workflow=strict
```

Agent stderr tail:

```text
2026-05-11T07:09:50.153792Z  WARN priority_agent::services::api::retry: Provider request failed transiently; reconnecting 1/5 for MiniMax chat.completions after 690ms: http error: error sending request for url (https://api.minimaxi.com/v1/chat/completions)
[required validation still running after 30s] cargo test -q resume -- --test-threads=1
[required validation still running after 60s] cargo test -q resume -- --test-threads=1
[required validation still running after 90s] cargo test -q resume -- --test-threads=1
2026-05-11T07:15:24.182772Z  WARN priority_agent::services::api::retry: Provider request failed transiently; reconnecting 1/5 for MiniMax chat.completions after 702ms: http error: error sending request for url (https://api.minimaxi.com/v1/chat/completions)
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
