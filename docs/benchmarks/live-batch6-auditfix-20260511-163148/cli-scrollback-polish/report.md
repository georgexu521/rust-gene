# Live Eval Report: cli-scrollback-polish

- Run id: `batch6-auditfix-20260511-163148`
- Sample: `evalsets/live_tasks/cli-scrollback-polish.yaml`
- Worktree: `target/live-evals/batch6-auditfix-20260511-163148/cli-scrollback-polish/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/batch6-auditfix-20260511-163148/cli-scrollback-polish/env`
- Test status: `ok`
- Generated: `2026-05-11 16:37:54 +0800`

## Git Status

```text
 M src/services/api/kimi.rs
```

## Diff Stat

```text
 src/services/api/kimi.rs | 2 +-
 1 file changed, 1 insertion(+), 1 deletion(-)
```

## Required Commands

```text
$ cargo test -q shell -- --test-threads=1

running 16 tests
................
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 885 filtered out; finished in 0.00s

[exit status: 0]

$ cargo test -q tui -- --test-threads=1

running 136 tests
....................................................................................... 87/136
.................................................
test result: ok. 136 passed; 0 failed; 0 ignored; 0 measured; 765 filtered out; finished in 0.37s

[exit status: 0]

$ cargo test -q -- --test-threads=1

running 901 tests
....................................................................................... 87/901
....................................................................................... 174/901
....................................................................................... 261/901
....................................................................................... 348/901
....................................................................................... 435/901
....................................................................................... 522/901
....................................................................................... 609/901
....................................................................................... 696/901
....................................................................................... 783/901
....................................................................................... 870/901
...............................
test result: ok. 901 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 17.73s

[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-batch6-auditfix-20260511-163148/cli-scrollback-polish/agent-output.md`
- Events: `docs/benchmarks/live-batch6-auditfix-20260511-163148/cli-scrollback-polish/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 1
tool_execution_complete: 18
tool_execution_progress: 4
tool_execution_start: 18
trace_summary: 1
```

Quality signals:

```text
output_chars: 744
diff_chars: 548
tool_executions: 18
first_write_tool_index: 18
tool_errors: 1
tool_failures: 1
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 113
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=22071 tool_schema=2641 tools=12 workflow=guarded closeout=full validation=failed:1/8
adaptive_triggers: required_validation,first_code_change
trace_event_types: workflow.trigger,workflow.fallback,verify.done,reflection.pass,stage.validation,acceptance.review,workflow.plan,memory.sync,workflow.fallback,closeout,runtime.diet,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: audit_or_regression_check
warning: tool_errors_seen
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
memory_sync_events: 13
memory_tool_calls: 0
retrieval_sources: Project,Session
required_commands: 3
agent_required_commands: 2
harness_commands: 1
required_command_status: ok
validation_events: 1
stage_validation_events: 1
tool_progress_events: 4
guided_debugging_events: 0
guided_reasoning_events: 1
workflow_plan_events: 2
weighted_plan_events: 1
reweighted_plan_events: 1
adaptive_trigger_events: 2
adaptive_triggers: required_validation,first_code_change
latest_top_priority: P0
latest_top_importance_score: 0.8274999856948853
latest_top_weight_share: 0.3551502227783203
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=22071 tool_schema=2641 tools=12 workflow=guarded
note: guided debugging is expected only after a blocker or failed validation
```

Agent stderr tail:

```text
2026-05-11T08:32:07.060055Z  WARN priority_agent::services::api::retry: Provider request failed transiently; reconnecting 1/5 for MiniMax chat.completions after 596ms: http error: error sending request for url (https://api.minimaxi.com/v1/chat/completions)
2026-05-11T08:32:10.660622Z  WARN priority_agent::services::api::retry: Provider request failed transiently; reconnecting 2/5 for MiniMax chat.completions after 1.234s: http error: error sending request for url (https://api.minimaxi.com/v1/chat/completions)
2026-05-11T08:35:15.912277Z  WARN priority_agent::services::api::retry: Provider request failed transiently; reconnecting 1/5 for MiniMax chat.completions after 697ms: http error: error sending request for url (https://api.minimaxi.com/v1/chat/completions)
2026-05-11T08:35:19.613543Z  WARN priority_agent::services::api::retry: Provider request failed transiently; reconnecting 2/5 for MiniMax chat.completions after 1.187s: http error: error sending request for url (https://api.minimaxi.com/v1/chat/completions)
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
