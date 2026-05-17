# Live Eval Report: memory-save-quality-gate

- Run id: `memory-save-rerun-20260517-170500`
- Sample: `evalsets/live_tasks/memory-save-quality-gate.yaml`
- Worktree: `target/live-evals/memory-save-rerun-20260517-170500/memory-save-quality-gate/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/memory-save-rerun-20260517-170500/memory-save-quality-gate/env`
- Test status: `ok`
- Generated: `2026-05-17 16:47:31 +0800`

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

running 105 tests
....................................................................................... 87/105
..................
test result: ok. 105 passed; 0 failed; 0 ignored; 0 measured; 1340 filtered out; finished in 0.19s

[exit status: 0]

$ ! rg 'assess_memory_candidate\(content, category, &existing, true\)' src/tools/memory_tool/mod.rs
[exit status: 0]

$ ! rg 'let status = if explicit \|\| score >= 0\.65' src/memory/quality.rs
[exit status: 0]

$ ! rg 'format!\("Saved: \{\}' src/tui/app.rs
[exit status: 0]

$ cargo test -q -- --test-threads=1

running 1445 tests
....................................................................................... 87/1445
....................................................................................... 174/1445
....................................................................................... 261/1445
....................................................................................... 348/1445
....................................................................................... 435/1445
....................................................................................... 522/1445
....................................................................................... 609/1445
....................................................................................... 696/1445
....................................................................................... 783/1445
....................................................................................... 870/1445
....................................................................................... 957/1445
....................................................................................... 1044/1445
....................................................................................... 1131/1445
....................................................................................... 1218/1445
....................................................................................... 1305/1445
....................................................................................... 1392/1445
.....................................................
test result: ok. 1445 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 21.95s

[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-memory-save-rerun-20260517-170500/memory-save-quality-gate/agent-output.md`
- Events: `docs/benchmarks/live-memory-save-rerun-20260517-170500/memory-save-quality-gate/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 1
tool_execution_complete: 14
tool_execution_progress: 4
tool_execution_start: 14
trace_summary: 1
```

Quality signals:

```text
output_chars: 1282
diff_chars: 4514
diff_files_changed: 3
tool_executions: 14
first_write_tool_index: 11
forbidden_tool_uses: none
tool_errors: 0
tool_failures: 0
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 80
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=9352 tool_schema=3186 tools=15 workflow=strict closeout=full validation=passed:7/7
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
tool_progress_events: 4
guided_debugging_events: 0
guided_reasoning_events: 1
workflow_plan_events: 2
weighted_plan_events: 1
reweighted_plan_events: 1
adaptive_trigger_events: 3
adaptive_triggers: required_validation,repeated_no_code_progress,first_code_change
latest_top_priority: P0
latest_top_importance_score: 0.8300000429153442
latest_top_weight_share: 0.1764080822467804
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=9352 tool_schema=3186 tools=15 workflow=strict
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
