# Live Eval Report: memory-save-quality-gate

- Run id: `real-project-coding-20260517-153331`
- Sample: `evalsets/live_tasks/memory-save-quality-gate.yaml`
- Worktree: `target/live-evals/real-project-coding-20260517-153331/memory-save-quality-gate/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/real-project-coding-20260517-153331/memory-save-quality-gate/env`
- Test status: `failed`
- Generated: `2026-05-17 16:14:07 +0800`

## Git Status

```text
 M src/memory/quality.rs
 M src/tools/memory_tool/mod.rs
 M src/tui/app.rs
?? src/tui/app_patch_temp.rs
```

## Diff Stat

```text
 src/memory/quality.rs        |  9 ++++++++-
 src/tools/memory_tool/mod.rs |  2 +-
 src/tui/app.rs               | 28 +++++++++++++++-------------
 3 files changed, 24 insertions(+), 15 deletions(-)
```

## Required Commands

```text
$ cargo test -q memory -- --test-threads=1
error: unexpected closing delimiter: `}`
   --> src/tui/app.rs:138:1
    |
 72 |             match event {
    |                         - this delimiter might not be properly closed...
...
 79 |                 } if latest_acceptance.is_none() => {
    |                 - ...as it matches this but it has different indentation
...
138 | }
    | ^ unexpected closing delimiter

error: could not compile `priority-agent` (bin "priority-agent" test) due to 1 previous error
[exit status: 101]

$ ! rg 'assess_memory_candidate\(content, category, &existing, true\)' src/tools/memory_tool/mod.rs
[exit status: 0]

$ ! rg 'let status = if explicit \|\| score >= 0\.65' src/memory/quality.rs
[exit status: 0]

$ ! rg 'format!\("Saved: \{\}' src/tui/app.rs
[exit status: 0]

$ cargo test -q -- --test-threads=1
error: unexpected closing delimiter: `}`
   --> src/tui/app.rs:138:1
    |
 72 |             match event {
    |                         - this delimiter might not be properly closed...
...
 79 |                 } if latest_acceptance.is_none() => {
    |                 - ...as it matches this but it has different indentation
...
138 | }
    | ^ unexpected closing delimiter

error: could not compile `priority-agent` (bin "priority-agent" test) due to 1 previous error
[exit status: 101]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-real-project-coding-20260517-153331/memory-save-quality-gate/agent-output.md`
- Events: `docs/benchmarks/live-real-project-coding-20260517-153331/memory-save-quality-gate/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 1
tool_execution_complete: 17
tool_execution_progress: 7
tool_execution_start: 17
trace_summary: 1
```

Quality signals:

```text
output_chars: 2973
diff_chars: 5207
diff_files_changed: 3
tool_executions: 17
first_write_tool_index: 11
forbidden_tool_uses: none
tool_errors: 1
tool_failures: 7
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 147
test_status: failed
verification_passed: false
stage_validation_passed: false
acceptance_accepted: False
closeout_status: failed
runtime_diet: prompt=23644 tool_schema=3186 tools=15 workflow=strict closeout=full validation=failed:3/7
adaptive_triggers: required_validation,repeated_no_code_progress,first_code_change,verification_failed,acceptance_rejected
trace_event_types: workflow.fallback,api.done,tool.start,tool.done,verify.done,reflection.pass,stage.validation,guided.debug,acceptance.review,closeout,runtime.diet,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: seeded_code_change
behavior_assertions: memory_quality_gate,memory_save_outcome_visibility
behavior_assertion_status: failed
warning: tool_errors_seen
warning: earlier_verification_failed_before_repair
warning: earlier_stage_validation_failed_before_repair
warning: required_commands_not_passing
warning: behavior_assertions_not_passing
warning: closeout_not_successful
warning: acceptance_review_rejected
warning: stage_validation_failed
warning: verification_failed
failure_owner: llm_reasoning
```

Specialty signals:

```text
memory_active: true
automation_active: true
guided_debugging_active: true
guided_reasoning_active: false
weighted_planning_active: true
closeout_active: true
adaptive_workflow_active: true
active_specialty_signals: 6/7
memory_sync_events: 7
memory_tool_calls: 0
retrieval_sources: Project,Session
required_commands: 5
agent_required_commands: 5
harness_commands: 0
required_command_status: failed
validation_events: 4
stage_validation_events: 4
tool_progress_events: 7
guided_debugging_events: 4
guided_reasoning_events: 0
workflow_plan_events: 1
weighted_plan_events: 1
reweighted_plan_events: 0
adaptive_trigger_events: 5
adaptive_triggers: required_validation,repeated_no_code_progress,first_code_change,verification_failed,acceptance_rejected
latest_top_priority: P2
latest_top_importance_score: 0.40625
latest_top_weight_share: 0.24828112125396729
acceptance_accepted: False
closeout_status: failed
runtime_diet: prompt=23644 tool_schema=3186 tools=15 workflow=strict
attention: required commands did not pass in the harness
```

Agent stderr tail:

```text
[required validation still running after 30s] cargo test -q memory -- --test-threads=1
[required validation still running after 60s] cargo test -q memory -- --test-threads=1
[required validation still running after 90s] cargo test -q memory -- --test-threads=1
2026-05-17T08:10:37.526947Z  WARN priority_agent::engine::conversation_loop::patch_recovery: Patch synthesis JSON actions were not directly applicable: patch synthesis declined without a reason; patch synthesis declined without a reason
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
