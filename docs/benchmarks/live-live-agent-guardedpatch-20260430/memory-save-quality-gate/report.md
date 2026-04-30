# Live Eval Report: memory-save-quality-gate

- Run id: `live-agent-guardedpatch-20260430`
- Sample: `evalsets/live_tasks/memory-save-quality-gate.yaml`
- Worktree: `target/live-evals/live-agent-guardedpatch-20260430/memory-save-quality-gate/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/live-agent-guardedpatch-20260430/memory-save-quality-gate/env`
- Test status: `failed`
- Generated: `2026-04-30 16:35:06 +0800`

## Git Status

```text
```

## Diff Stat

```text
```

## Required Commands

```text
$ cargo test -q memory -- --test-threads=1

running 75 tests
.................... 20/75
memory::calibration::tests::built_in_calibration_samples_pass --- FAILED
................................. 54/75
memory::quality::tests::explicit_does_not_accept_duplicate_memory --- FAILED
memory::quality::tests::explicit_does_not_accept_low_quality_note --- FAILED
...................
failures:

---- memory::calibration::tests::built_in_calibration_samples_pass stdout ----

thread 'memory::calibration::tests::built_in_calibration_samples_pass' (1393800) panicked at src/memory/calibration.rs:197:9:
failed calibration samples: [MemoryCalibrationResult { id: "explicit_low_value_note", expected: NotAccepted, actual: Accepted, score: Some(0.56542045), passed: false, reason: "write_score=0.57, status=Proposed, relevance=0.65, reuse=0.65, stability=0.33, trust=0.70, novelty=1.00, risk_reduction=0.25, token_cost=0.05, sensitivity_risk=0.00, kind=Note, stable=0.65, utility=0.65, specificity=0.55, volatility=0.70, duplication=0.00", rationale: "Explicit save can lower friction but must not bypass quality gates." }, MemoryCalibrationResult { id: "duplicate_project_fact", expected: Rejected, actual: Accepted, score: Some(0.65165913), passed: false, reason: "write_score=0.65, status=Rejected, relevance=0.85, reuse=0.80, stability=0.76, trust=0.72, novelty=0.00, risk_reduction=0.65, token_cost=0.05, sensitivity_risk=0.00, kind=WorkflowConvention, stable=0.85, utility=0.80, specificity=0.80, volatility=0.20, duplication=1.00", rationale: "Duplicate memories should be rejected even when explicit." }]
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- memory::quality::tests::explicit_does_not_accept_duplicate_memory stdout ----

thread 'memory::quality::tests::explicit_does_not_accept_duplicate_memory' (1393837) panicked at src/memory/quality.rs:276:9:
assertion `left != right` failed
  left: Accepted
 right: Accepted

---- memory::quality::tests::explicit_does_not_accept_low_quality_note stdout ----

thread 'memory::quality::tests::explicit_does_not_accept_low_quality_note' (1393838) panicked at src/memory/quality.rs:267:9:
assertion `left != right` failed
  left: Accepted
 right: Accepted


failures:
    memory::calibration::tests::built_in_calibration_samples_pass
    memory::quality::tests::explicit_does_not_accept_duplicate_memory
    memory::quality::tests::explicit_does_not_accept_low_quality_note

test result: FAILED. 72 passed; 3 failed; 0 ignored; 0 measured; 874 filtered out; finished in 0.07s

error: test failed, to rerun pass `--bin priority-agent`
[exit status: 101]

$ ! rg 'assess_memory_candidate\(content, category, &existing, true\)' src/tools/memory_tool/mod.rs
            match crate::memory::assess_memory_candidate(content, category, &existing, true) {
[exit status: 1]

$ ! rg 'let status = if explicit \|\| score >= 0\.65' src/memory/quality.rs
    let status = if explicit || score >= 0.65 { MemoryStatus::Accepted } else { write_decision.status };
[exit status: 1]

$ ! rg 'format!\("Saved: \{\}' src/tui/app.rs
                            format!("Saved: {}", save_content)
                            format!("Saved: {}", save_content)
[exit status: 1]

$ cargo test -q -- --test-threads=1

running 949 tests
....................................................................................... 87/949
....................................................................................... 174/949
....................................................................................... 261/949
....................................................................................... 348/949
....................................................................................... 435/949
memory::calibration::tests::built_in_calibration_samples_pass --- FAILED
................................. 469/949
memory::quality::tests::explicit_does_not_accept_duplicate_memory --- FAILED
memory::quality::tests::explicit_does_not_accept_low_quality_note --- FAILED
....................................................................................... 558/949
....................................................................................... 645/949
....................................................................................... 732/949
....................................................................................... 819/949
....................................................................................... 906/949
...........................................
failures:

---- memory::calibration::tests::built_in_calibration_samples_pass stdout ----

thread 'memory::calibration::tests::built_in_calibration_samples_pass' (1395901) panicked at src/memory/calibration.rs:197:9:
failed calibration samples: [MemoryCalibrationResult { id: "explicit_low_value_note", expected: NotAccepted, actual: Accepted, score: Some(0.56542045), passed: false, reason: "write_score=0.57, status=Proposed, relevance=0.65, reuse=0.65, stability=0.33, trust=0.70, novelty=1.00, risk_reduction=0.25, token_cost=0.05, sensitivity_risk=0.00, kind=Note, stable=0.65, utility=0.65, specificity=0.55, volatility=0.70, duplication=0.00", rationale: "Explicit save can lower friction but must not bypass quality gates." }, MemoryCalibrationResult { id: "duplicate_project_fact", expected: Rejected, actual: Accepted, score: Some(0.65165913), passed: false, reason: "write_score=0.65, status=Rejected, relevance=0.85, reuse=0.80, stability=0.76, trust=0.72, novelty=0.00, risk_reduction=0.65, token_cost=0.05, sensitivity_risk=0.00, kind=WorkflowConvention, stable=0.85, utility=0.80, specificity=0.80, volatility=0.20, duplication=1.00", rationale: "Duplicate memories should be rejected even when explicit." }]
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- memory::quality::tests::explicit_does_not_accept_duplicate_memory stdout ----

thread 'memory::quality::tests::explicit_does_not_accept_duplicate_memory' (1395936) panicked at src/memory/quality.rs:276:9:
assertion `left != right` failed
  left: Accepted
 right: Accepted

---- memory::quality::tests::explicit_does_not_accept_low_quality_note stdout ----

thread 'memory::quality::tests::explicit_does_not_accept_low_quality_note' (1395937) panicked at src/memory/quality.rs:267:9:
assertion `left != right` failed
  left: Accepted
 right: Accepted


failures:
    memory::calibration::tests::built_in_calibration_samples_pass
    memory::quality::tests::explicit_does_not_accept_duplicate_memory
    memory::quality::tests::explicit_does_not_accept_low_quality_note

test result: FAILED. 946 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 52.61s

error: test failed, to rerun pass `--bin priority-agent`
[exit status: 101]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-live-agent-guardedpatch-20260430/memory-save-quality-gate/agent-output.md`
- Events: `docs/benchmarks/live-live-agent-guardedpatch-20260430/memory-save-quality-gate/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 2
tool_execution_complete: 10
tool_execution_start: 10
trace_summary: 1
```

Quality signals:

```text
output_chars: 839
diff_chars: 0
tool_executions: 10
tool_errors: 0
tool_failures: 0
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 60
test_status: failed
verification_passed: false
stage_validation_passed: false
acceptance_accepted: None
closeout_status: not_verified
trace_event_types: tool.done,memory.sync,api.start,workflow.fallback,api.done,tool.start,tool.done,workflow.fallback,workflow.fallback,workflow.fallback,closeout,assistant
warning: no_code_diff
warning: required_commands_not_passing
warning: closeout_not_successful
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
