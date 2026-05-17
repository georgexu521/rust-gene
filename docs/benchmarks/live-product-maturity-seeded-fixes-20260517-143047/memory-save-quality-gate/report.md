# Live Eval Report: memory-save-quality-gate

- Run id: `product-maturity-seeded-fixes-20260517-143047`
- Sample: `evalsets/live_tasks/memory-save-quality-gate.yaml`
- Worktree: `target/live-evals/product-maturity-seeded-fixes-20260517-143047/memory-save-quality-gate/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/product-maturity-seeded-fixes-20260517-143047/memory-save-quality-gate/env`
- Test status: `ok`
- Generated: `2026-05-17 14:37:27 +0800`

## Git Status

```text
 M src/memory/quality.rs
 M src/tools/memory_tool/mod.rs
 M src/tui/app.rs
```

## Diff Stat

```text
 src/memory/quality.rs        |  2 +-
 src/tools/memory_tool/mod.rs |  2 +-
 src/tui/app.rs               | 24 ++++++++++++------------
 3 files changed, 14 insertions(+), 14 deletions(-)
```

## Required Commands

```text
$ cargo test -q memory -- --test-threads=1

running 104 tests
....................................................................................... 87/104
.................
test result: ok. 104 passed; 0 failed; 0 ignored; 0 measured; 1336 filtered out; finished in 0.19s

[exit status: 0]

$ ! rg 'assess_memory_candidate\(content, category, &existing, true\)' src/tools/memory_tool/mod.rs
[exit status: 0]

$ ! rg 'let status = if explicit \|\| score >= 0\.65' src/memory/quality.rs
[exit status: 0]

$ ! rg 'format!\("Saved: \{\}' src/tui/app.rs
[exit status: 0]

$ cargo test -q -- --test-threads=1

running 1440 tests
....................................................................................... 87/1440
....................................................................................... 174/1440
....................................................................................... 261/1440
....................................................................................... 348/1440
....................................................................................... 435/1440
....................................................................................... 522/1440
....................................................................................... 609/1440
....................................................................................... 696/1440
....................................................................................... 783/1440
....................................................................................... 870/1440
....................................................................................... 957/1440
....................................................................................... 1044/1440
....................................................................................... 1131/1440
....................................................................................... 1218/1440
....................................................................................... 1305/1440
....................................................................................... 1392/1440
................................................
test result: ok. 1440 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 21.88s

[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-product-maturity-seeded-fixes-20260517-143047/memory-save-quality-gate/agent-output.md`
- Events: `docs/benchmarks/live-product-maturity-seeded-fixes-20260517-143047/memory-save-quality-gate/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 1
tool_execution_complete: 13
tool_execution_progress: 5
tool_execution_start: 13
trace_summary: 1
```

Quality signals:

```text
output_chars: 1517
diff_chars: 4514
diff_files_changed: 3
tool_executions: 13
first_write_tool_index: 10
forbidden_tool_uses: none
tool_errors: 0
tool_failures: 0
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 78
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=6521 tool_schema=3186 tools=15 workflow=strict closeout=full validation=passed:7/7
adaptive_triggers: required_validation,repeated_no_code_progress,first_code_change
trace_event_types: workflow.trigger,workflow.fallback,verify.done,reflection.pass,stage.validation,acceptance.review,workflow.plan,memory.sync,workflow.fallback,closeout,runtime.diet,assistant
stale_edit_warnings: 0
action_checkpoint_no_patch: false
action_checkpoint_invalid_tools: false
patch_synthesis_no_change: false
eval_intent: seeded_code_change
behavior_assertions: memory_quality_gate,memory_save_outcome_visibility
behavior_assertion_status: passed
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
required_commands: 5
agent_required_commands: 5
harness_commands: 0
required_command_status: ok
validation_events: 1
stage_validation_events: 1
tool_progress_events: 5
guided_debugging_events: 0
guided_reasoning_events: 1
workflow_plan_events: 2
weighted_plan_events: 1
reweighted_plan_events: 1
adaptive_trigger_events: 3
adaptive_triggers: required_validation,repeated_no_code_progress,first_code_change
latest_top_priority: P2
latest_top_importance_score: 0.40625
latest_top_weight_share: 0.1383567601442337
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=6521 tool_schema=3186 tools=15 workflow=strict
note: guided debugging is expected only after a blocker or failed validation
```

Agent stderr tail:

```text
[required validation still running after 30s] cargo test -q memory -- --test-threads=1
[required validation still running after 60s] cargo test -q memory -- --test-threads=1
[required validation still running after 90s] cargo test -q memory -- --test-threads=1
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
