# Live Eval Report: memory-save-quality-gate

- Run id: `realflow-guided-20260503-170614`
- Sample: `evalsets/live_tasks/memory-save-quality-gate.yaml`
- Worktree: `target/live-evals/realflow-guided-20260503-170614/memory-save-quality-gate/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/realflow-guided-20260503-170614/memory-save-quality-gate/env`
- Test status: `failed`
- Generated: `2026-05-03 17:15:43 +0800`

## Git Status

```text
 M src/memory/quality.rs
 M src/tools/memory_tool/mod.rs
```

## Diff Stat

```text
 src/memory/quality.rs        | 3 ++-
 src/tools/memory_tool/mod.rs | 2 +-
 2 files changed, 3 insertions(+), 2 deletions(-)
```

## Required Commands

```text
$ cargo test -q memory -- --test-threads=1

running 91 tests
....................................................................................... 87/91
....
test result: ok. 91 passed; 0 failed; 0 ignored; 0 measured; 963 filtered out; finished in 0.18s

[exit status: 0]

$ ! rg 'assess_memory_candidate\(content, category, &existing, true\)' src/tools/memory_tool/mod.rs
[exit status: 0]

$ ! rg 'let status = if explicit \|\| score >= 0\.65' src/memory/quality.rs
[exit status: 0]

$ ! rg 'format!\("Saved: \{\}' src/tui/app.rs
                            format!("Saved: {}", save_content)
                            format!("Saved: {}", save_content)
[exit status: 1]

$ cargo test -q -- --test-threads=1

running 1054 tests
....................................................................................... 87/1054
....................................................................................... 174/1054
....................................................................................... 261/1054
....................................................................................... 348/1054
....................................................................................... 435/1054
....................................................................................... 522/1054
....................................................................................... 609/1054
....................................................................................... 696/1054
....................................................................................... 783/1054
....................................................................................... 870/1054
....................................................................................... 957/1054
....................................................................................... 1044/1054
..........
test result: ok. 1054 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 19.06s

[exit status: 0]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-realflow-guided-20260503-170614/memory-save-quality-gate/agent-output.md`
- Events: `docs/benchmarks/live-realflow-guided-20260503-170614/memory-save-quality-gate/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 1
tool_execution_complete: 17
tool_execution_progress: 2
tool_execution_start: 17
trace_summary: 1
```

Quality signals:

```text
output_chars: 1647
diff_chars: 1364
tool_executions: 17
first_write_tool_index: 13
tool_errors: 0
tool_failures: 1
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 109
test_status: failed
verification_passed: false
stage_validation_passed: false
acceptance_accepted: False
closeout_status: failed
trace_event_types: api.start,workflow.fallback,api.done,tool.start,tool.done,verify.done,reflection.pass,stage.validation,guided.debug,acceptance.review,closeout,assistant
stale_edit_warnings: 0
warning: earlier_verification_failed_before_repair
warning: earlier_stage_validation_failed_before_repair
warning: required_commands_not_passing
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
guided_reasoning_active: true
weighted_planning_active: true
closeout_active: true
active_specialty_signals: 6/6
memory_sync_events: 8
memory_tool_calls: 0
retrieval_sources: Project,Session
required_commands: 5
required_command_status: failed
validation_events: 4
stage_validation_events: 4
tool_progress_events: 2
guided_debugging_events: 4
guided_reasoning_events: 1
workflow_plan_events: 1
weighted_plan_events: 1
reweighted_plan_events: 0
latest_top_priority: P1
latest_top_importance_score: 0.7150000333786011
latest_top_weight_share: 0.15409480035305023
acceptance_accepted: False
closeout_status: failed
attention: required commands did not pass in the harness
```

Agent stderr tail:

```text
[required validation still running after 30s] cargo test -q memory -- --test-threads=1
[required validation still running after 60s] cargo test -q memory -- --test-threads=1
```

## Human Review

- accepted: false
- task_success: false
- mainline_hit: partial
- plan_coverage: incomplete
- rework_count: 4
- tool_efficiency: mixed
- diff_discipline: good
- closeout_accuracy: accurate
- notes: The workflow correctly refused to claim success: required commands failed, guided debugging fired, acceptance review rejected the result, and closeout status stayed failed. The model fixed `src/memory/quality.rs` and `src/tools/memory_tool/mod.rs` but missed the required `src/tui/app.rs` `/save` outcome path, leaving two `format!("Saved: {}")` call sites. This is an LLM task-surface miss, not a harness failure.
