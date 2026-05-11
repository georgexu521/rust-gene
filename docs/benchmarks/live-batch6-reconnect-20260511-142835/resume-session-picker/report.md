# Live Eval Report: resume-session-picker

- Run id: `batch6-reconnect-20260511-142835`
- Sample: `evalsets/live_tasks/resume-session-picker.yaml`
- Worktree: `target/live-evals/batch6-reconnect-20260511-142835/resume-session-picker/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/batch6-reconnect-20260511-142835/resume-session-picker/env`
- Test status: `ok`
- Generated: `2026-05-11 14:42:20 +0800`

## Git Status

```text
 M src/tools/memory_tool/mod.rs
```

## Diff Stat

```text
 src/tools/memory_tool/mod.rs | 2 +-
 1 file changed, 1 insertion(+), 1 deletion(-)
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
test result: ok. 911 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 19.89s

[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-batch6-reconnect-20260511-142835/resume-session-picker/agent-output.md`
- Events: `docs/benchmarks/live-batch6-reconnect-20260511-142835/resume-session-picker/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 1
tool_execution_complete: 25
tool_execution_progress: 4
tool_execution_start: 25
trace_summary: 1
```

Quality signals:

```text
output_chars: 2517
diff_chars: 673
tool_executions: 25
first_write_tool_index: 21
tool_errors: 0
tool_failures: 3
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 159
test_status: ok
verification_passed: false
stage_validation_passed: false
acceptance_accepted: False
closeout_status: failed
runtime_diet: prompt=37323 tool_schema=2641 tools=12 workflow=strict closeout=full validation=failed:4/21
adaptive_triggers: required_validation,repeated_no_code_progress,first_code_change,verification_failed,acceptance_rejected
trace_event_types: guided.debug,acceptance.review,workflow.fallback,memory.sync,api.start,workflow.fallback,api.done,tool.start,tool.done,closeout,runtime.diet,assistant
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
memory_sync_events: 10
memory_tool_calls: 0
retrieval_sources: Project,Session
required_commands: 3
required_command_status: ok
validation_events: 3
stage_validation_events: 3
tool_progress_events: 4
guided_debugging_events: 3
guided_reasoning_events: 1
workflow_plan_events: 1
weighted_plan_events: 1
reweighted_plan_events: 0
adaptive_trigger_events: 5
adaptive_triggers: required_validation,repeated_no_code_progress,first_code_change,verification_failed,acceptance_rejected
latest_top_priority: P1
latest_top_importance_score: 0.6800000071525574
latest_top_weight_share: 0.21501976251602173
acceptance_accepted: False
closeout_status: failed
runtime_diet: prompt=37323 tool_schema=2641 tools=12 workflow=strict
```

Agent stderr tail:

```text
2026-05-11T06:28:56.514690Z  WARN priority_agent::services::api::retry: Provider request failed transiently; reconnecting 1/5 for MiniMax chat.completions after 551ms: http error: error sending request for url (https://api.minimaxi.com/v1/chat/completions)
2026-05-11T06:29:00.072131Z  WARN priority_agent::services::api::retry: Provider request failed transiently; reconnecting 2/5 for MiniMax chat.completions after 1.146s: http error: error sending request for url (https://api.minimaxi.com/v1/chat/completions)
2026-05-11T06:29:04.224630Z  WARN priority_agent::services::api::retry: Provider request failed transiently; reconnecting 3/5 for MiniMax chat.completions after 2.085s: http error: error sending request for url (https://api.minimaxi.com/v1/chat/completions)
2026-05-11T06:31:26.679944Z  WARN priority_agent::engine::conversation_loop::patch_recovery: Patch synthesis JSON actions were not directly applicable: patch synthesis declined without a reason; patch synthesis declined without a reason
[required validation still running after 30s] cargo test -q resume -- --test-threads=1
[required validation still running after 60s] cargo test -q resume -- --test-threads=1
[required validation still running after 90s] cargo test -q resume -- --test-threads=1
[required validation still running after 30s] cargo test -q -- --test-threads=1
[required validation still running after 30s] cargo test -q -- --test-threads=1
2026-05-11T06:40:31.995365Z  WARN priority_agent::services::api::retry: Provider request failed transiently; reconnecting 1/5 for MiniMax chat.completions after 531ms: http error: error sending request for url (https://api.minimaxi.com/v1/chat/completions)
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
