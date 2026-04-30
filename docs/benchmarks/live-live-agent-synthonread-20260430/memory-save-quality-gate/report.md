# Live Eval Report: memory-save-quality-gate

- Run id: `live-agent-synthonread-20260430`
- Sample: `evalsets/live_tasks/memory-save-quality-gate.yaml`
- Worktree: `target/live-evals/live-agent-synthonread-20260430/memory-save-quality-gate/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/live-agent-synthonread-20260430/memory-save-quality-gate/env`
- Test status: `failed`
- Generated: `2026-04-30 13:26:40 +0800`

## Git Status

```text
 M src/memory/quality.rs
 M src/tools/memory_tool/mod.rs
```

## Diff Stat

```text
 src/memory/quality.rs        | 6 +++++-
 src/tools/memory_tool/mod.rs | 2 +-
 2 files changed, 6 insertions(+), 2 deletions(-)
```

## Required Commands

```text
$ cargo test -q memory -- --test-threads=1

running 75 tests
.................... 20/75
memory::calibration::tests::built_in_calibration_samples_pass --- FAILED
......................................................
failures:

---- memory::calibration::tests::built_in_calibration_samples_pass stdout ----

thread 'memory::calibration::tests::built_in_calibration_samples_pass' (629190) panicked at src/memory/calibration.rs:197:9:
failed calibration samples: [MemoryCalibrationResult { id: "duplicate_project_fact", expected: Rejected, actual: Accepted, score: Some(0.65165913), passed: false, reason: "write_score=0.65, status=Rejected, relevance=0.85, reuse=0.80, stability=0.76, trust=0.72, novelty=0.00, risk_reduction=0.65, token_cost=0.05, sensitivity_risk=0.00, kind=WorkflowConvention, stable=0.85, utility=0.80, specificity=0.80, volatility=0.20, duplication=1.00", rationale: "Duplicate memories should be rejected even when explicit." }]
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    memory::calibration::tests::built_in_calibration_samples_pass

test result: FAILED. 74 passed; 1 failed; 0 ignored; 0 measured; 874 filtered out; finished in 0.07s

error: test failed, to rerun pass `--bin priority-agent`
[exit status: 101]

$ cargo test -q -- --test-threads=1

running 949 tests
....................................................................................... 87/949
....................................................................................... 174/949
....................................................................................... 261/949
....................................................................................... 348/949
....................................................................................... 435/949
memory::calibration::tests::built_in_calibration_samples_pass --- FAILED
....................................................................................... 523/949
....................................................................................... 610/949
....................................................................................... 697/949
....................................................................................... 784/949
....................................................................................... 871/949
..............................................................................
failures:

---- memory::calibration::tests::built_in_calibration_samples_pass stdout ----

thread 'memory::calibration::tests::built_in_calibration_samples_pass' (630387) panicked at src/memory/calibration.rs:197:9:
failed calibration samples: [MemoryCalibrationResult { id: "duplicate_project_fact", expected: Rejected, actual: Accepted, score: Some(0.65165913), passed: false, reason: "write_score=0.65, status=Rejected, relevance=0.85, reuse=0.80, stability=0.76, trust=0.72, novelty=0.00, risk_reduction=0.65, token_cost=0.05, sensitivity_risk=0.00, kind=WorkflowConvention, stable=0.85, utility=0.80, specificity=0.80, volatility=0.20, duplication=1.00", rationale: "Duplicate memories should be rejected even when explicit." }]
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    memory::calibration::tests::built_in_calibration_samples_pass

test result: FAILED. 948 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 21.81s

error: test failed, to rerun pass `--bin priority-agent`
[exit status: 101]

```

## Agent Run

- Exit status: `124`
- Events: `docs/benchmarks/live-live-agent-synthonread-20260430/memory-save-quality-gate/agent-events.jsonl`

Event counts:

```text
eval_started: 1
start: 1
tool_execution_complete: 15
tool_execution_progress: 3
tool_execution_start: 15
```

Quality signals:

```text
output_chars: 0
diff_chars: 1480
tool_executions: 15
tool_errors: 0
tool_failures: 0
has_closeout: false
has_validation_claim: false
trace_status: missing
trace_events: 0
test_status: failed
verification_passed: false
stage_validation_passed: false
acceptance_accepted: None
closeout_status: missing
warning: empty_agent_output
warning: tool_run_without_closeout
warning: missing_trace_summary
warning: required_commands_not_passing
warning: closeout_not_successful
```

Agent stderr tail:

```text

[timeout after 900s]
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
