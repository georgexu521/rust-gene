# Live Eval Report: memory-save-quality-gate

- Run id: `live-eval-20260502-131257`
- Sample: `evalsets/live_tasks/memory-save-quality-gate.yaml`
- Worktree: `target/live-evals/live-eval-20260502-131257/memory-save-quality-gate/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/live-eval-20260502-131257/memory-save-quality-gate/env`
- Test status: `failed`
- Generated: `2026-05-02 13:32:59 +0800`

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

running 90 tests
............................. 29/90
memory::calibration::tests::built_in_calibration_samples_pass --- FAILED
............................................................
failures:

---- memory::calibration::tests::built_in_calibration_samples_pass stdout ----

thread 'memory::calibration::tests::built_in_calibration_samples_pass' (1907759) panicked at src/memory/calibration.rs:197:9:
failed calibration samples: [MemoryCalibrationResult { id: "duplicate_project_fact", expected: Rejected, actual: Accepted, score: Some(0.65165913), passed: false, reason: "write_score=0.65, status=Rejected, relevance=0.85, reuse=0.80, stability=0.76, trust=0.72, novelty=0.00, risk_reduction=0.65, token_cost=0.05, sensitivity_risk=0.00, kind=WorkflowConvention, stable=0.85, utility=0.80, specificity=0.80, volatility=0.20, duplication=1.00", rationale: "Duplicate memories should be rejected even when explicit." }]
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    memory::calibration::tests::built_in_calibration_samples_pass

test result: FAILED. 89 passed; 1 failed; 0 ignored; 0 measured; 913 filtered out; finished in 0.17s

error: test failed, to rerun pass `--bin priority-agent`
[exit status: 101]

$ ! rg 'assess_memory_candidate\(content, category, &existing, true\)' src/tools/memory_tool/mod.rs
[exit status: 0]

$ ! rg 'let status = if explicit \|\| score >= 0\.65' src/memory/quality.rs
[exit status: 0]

$ ! rg 'format!\("Saved: \{\}' src/tui/app.rs
[exit status: 0]

$ cargo test -q -- --test-threads=1

running 1003 tests
....................................................................................... 87/1003
....................................................................................... 174/1003
....................................................................................... 261/1003
....................................................................................... 348/1003
....................................................................................... 435/1003
........................................ 475/1003
memory::calibration::tests::built_in_calibration_samples_pass --- FAILED
....................................................................................... 563/1003
....................................................................................... 650/1003
....................................................................................... 737/1003
....................................................................................... 824/1003
....................................................................................... 911/1003
....................................................................................... 998/1003
.....
failures:

---- memory::calibration::tests::built_in_calibration_samples_pass stdout ----

thread 'memory::calibration::tests::built_in_calibration_samples_pass' (1909346) panicked at src/memory/calibration.rs:197:9:
failed calibration samples: [MemoryCalibrationResult { id: "duplicate_project_fact", expected: Rejected, actual: Accepted, score: Some(0.65165913), passed: false, reason: "write_score=0.65, status=Rejected, relevance=0.85, reuse=0.80, stability=0.76, trust=0.72, novelty=0.00, risk_reduction=0.65, token_cost=0.05, sensitivity_risk=0.00, kind=WorkflowConvention, stable=0.85, utility=0.80, specificity=0.80, volatility=0.20, duplication=1.00", rationale: "Duplicate memories should be rejected even when explicit." }]
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    memory::calibration::tests::built_in_calibration_samples_pass

test result: FAILED. 1002 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 19.60s

error: test failed, to rerun pass `--bin priority-agent`
[exit status: 101]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-live-eval-20260502-131257/memory-save-quality-gate/agent-output.md`
- Events: `docs/benchmarks/live-live-eval-20260502-131257/memory-save-quality-gate/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 1
tool_execution_complete: 19
tool_execution_progress: 6
tool_execution_start: 19
trace_summary: 1
```

Quality signals:

```text
output_chars: 719
diff_chars: 4567
tool_executions: 19
tool_errors: 0
tool_failures: 0
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 118
test_status: failed
verification_passed: true
stage_validation_passed: true
acceptance_accepted: True
closeout_status: passed
trace_event_types: api.done,tool.start,tool.done,verify.done,reflection.pass,stage.validation,acceptance.review,workflow.plan,memory.sync,workflow.fallback,closeout,assistant
warning: earlier_verification_failed_before_repair
warning: earlier_stage_validation_failed_before_repair
warning: required_commands_not_passing
```

Agent stderr tail:

```text
2026-05-02T05:14:32.735925Z  WARN priority_agent::tools::file_tool: File 'src/tui/app.rs' was modified since it was read
2026-05-02T05:25:38.290106Z  WARN priority_agent::tools::file_tool: File 'src/memory/quality.rs' was modified since it was read
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
