# Live Eval Report: memory-save-quality-gate

- Run id: `capability-evidence-20260509-173239`
- Sample: `evalsets/live_tasks/memory-save-quality-gate.yaml`
- Worktree: `target/live-evals/capability-evidence-20260509-173239/memory-save-quality-gate/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/capability-evidence-20260509-173239/memory-save-quality-gate/env`
- Test status: `ok`
- Generated: `2026-05-09 17:59:48 +0800`

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

running 94 tests
....................................................................................... 87/94
.......
test result: ok. 94 passed; 0 failed; 0 ignored; 0 measured; 1053 filtered out; finished in 0.18s

[exit status: 0]

$ ! rg 'assess_memory_candidate\(content, category, &existing, true\)' src/tools/memory_tool/mod.rs
[exit status: 0]

$ ! rg 'let status = if explicit \|\| score >= 0\.65' src/memory/quality.rs
[exit status: 0]

$ ! rg 'format!\("Saved: \{\}' src/tui/app.rs
[exit status: 0]

$ cargo test -q -- --test-threads=1

running 1147 tests
....................................................................................... 87/1147
....................................................................................... 174/1147
....................................................................................... 261/1147
....................................................................................... 348/1147
....................................................................................... 435/1147
....................................................................................... 522/1147
....................................................................................... 609/1147
....................................................................................... 696/1147
....................................................................................... 783/1147
....................................................................................... 870/1147
....................................................................................... 957/1147
....................................................................................... 1044/1147
....................................................................................... 1131/1147
................
test result: ok. 1147 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 19.34s

[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-capability-evidence-20260509-173239/memory-save-quality-gate/agent-output.md`
- Events: `docs/benchmarks/live-capability-evidence-20260509-173239/memory-save-quality-gate/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 1
tool_execution_complete: 10
tool_execution_progress: 4
tool_execution_start: 10
trace_summary: 1
```

Quality signals:

```text
output_chars: 1490
diff_chars: 4514
tool_executions: 10
first_write_tool_index: 7
tool_errors: 0
tool_failures: 0
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 62
test_status: ok
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=9647 tool_schema=2641 tools=12 workflow=strict closeout=full validation=passed
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
memory_sync_events: 3
memory_tool_calls: 0
retrieval_sources: Project,Session
required_commands: 5
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
latest_top_priority: P1
latest_top_importance_score: 0.675000011920929
latest_top_weight_share: 0.16707919538021088
acceptance_accepted: True
closeout_status: passed
runtime_diet: prompt=9647 tool_schema=2641 tools=12 workflow=strict
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
