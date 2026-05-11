# Live Eval Report: cli-scrollback-polish

- Run id: `batch6-evidencefix-20260511-171653`
- Sample: `evalsets/live_tasks/cli-scrollback-polish.yaml`
- Worktree: `target/live-evals/batch6-evidencefix-20260511-171653/cli-scrollback-polish/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/batch6-evidencefix-20260511-171653/cli-scrollback-polish/env`
- Test status: `ok`
- Generated: `2026-05-11 17:24:46 +0800`

## Git Status

```text
```

## Diff Stat

```text
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
test result: ok. 136 passed; 0 failed; 0 ignored; 0 measured; 765 filtered out; finished in 0.39s

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
test result: ok. 901 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 52.25s

[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-batch6-evidencefix-20260511-171653/cli-scrollback-polish/agent-output.md`
- Events: `docs/benchmarks/live-batch6-evidencefix-20260511-171653/cli-scrollback-polish/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 2
tool_execution_complete: 22
tool_execution_progress: 4
tool_execution_start: 22
trace_summary: 1
```

Quality signals:

```text
output_chars: 1089
diff_chars: 0
tool_executions: 22
first_write_tool_index: none
tool_errors: 3
tool_failures: 3
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 109
test_status: ok
verification_passed: false
stage_validation_passed: false
acceptance_accepted: None
closeout_status: not_verified
runtime_diet: prompt=15418 tool_schema=2641 tools=12 workflow=guarded closeout=full validation=failed:3/4
adaptive_triggers: required_validation
trace_event_types: memory.sync,api.start,workflow.fallback,api.done,tool.start,tool.done,tool.start,tool.done,guided.debug,closeout,runtime.diet,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: audit_or_regression_check
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
closeout_active: false
adaptive_workflow_active: true
active_specialty_signals: 5/7
memory_sync_events: 11
memory_tool_calls: 0
retrieval_sources: Project,Session
required_commands: 3
agent_required_commands: 2
harness_commands: 1
required_command_status: ok
validation_events: 0
stage_validation_events: 0
tool_progress_events: 4
guided_debugging_events: 1
guided_reasoning_events: 0
workflow_plan_events: 1
weighted_plan_events: 1
reweighted_plan_events: 0
adaptive_trigger_events: 1
adaptive_triggers: required_validation
latest_top_priority: P1
latest_top_importance_score: 0.675000011920929
latest_top_weight_share: 0.26706230640411377
acceptance_accepted: missing
closeout_status: not_verified
runtime_diet: prompt=15418 tool_schema=2641 tools=12 workflow=guarded
```

Agent stderr tail:

```text
2026-05-11T09:18:44.892986Z  WARN priority_agent::services::api::retry: Provider request failed transiently; reconnecting 1/5 for MiniMax chat.completions after 679ms: http error: error sending request for url (https://api.minimaxi.com/v1/chat/completions)
2026-05-11T09:22:11.995655Z  WARN priority_agent::services::api::retry: Provider request failed transiently; reconnecting 1/5 for MiniMax chat.completions after 531ms: http error: error sending request for url (https://api.minimaxi.com/v1/chat/completions)
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
