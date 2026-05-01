# Live Eval Report: memory-save-quality-gate

- Run id: `live-eval-20260501-095510`
- Sample: `evalsets/live_tasks/memory-save-quality-gate.yaml`
- Worktree: `target/live-evals/live-eval-20260501-095510/memory-save-quality-gate/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/live-eval-20260501-095510/memory-save-quality-gate/env`
- Test status: `failed`
- Generated: `2026-05-01 09:57:33 +0800`

## Git Status

```text
```

## Diff Stat

```text
```

## Required Commands

```text
$ cargo test -q memory -- --test-threads=1
warning: function `format_memory_write_outcome` is never used
   --> src/tui/app.rs:187:4
    |
187 | fn format_memory_write_outcome(
    |    ^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |
    = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default


running 77 tests
...................... 22/77
memory::calibration::tests::built_in_calibration_samples_pass --- FAILED
................................. 56/77
memory::quality::tests::explicit_does_not_accept_duplicate_memory --- FAILED
memory::quality::tests::explicit_does_not_accept_low_quality_note --- FAILED
...................
failures:

---- memory::calibration::tests::built_in_calibration_samples_pass stdout ----

thread 'memory::calibration::tests::built_in_calibration_samples_pass' (47466) panicked at src/memory/calibration.rs:197:9:
failed calibration samples: [MemoryCalibrationResult { id: "explicit_low_value_note", expected: NotAccepted, actual: Accepted, score: Some(0.56542045), passed: false, reason: "write_score=0.57, status=Proposed, relevance=0.65, reuse=0.65, stability=0.33, trust=0.70, novelty=1.00, risk_reduction=0.25, token_cost=0.05, sensitivity_risk=0.00, kind=Note, stable=0.65, utility=0.65, specificity=0.55, volatility=0.70, duplication=0.00", rationale: "Explicit save can lower friction but must not bypass quality gates." }, MemoryCalibrationResult { id: "duplicate_project_fact", expected: Rejected, actual: Accepted, score: Some(0.65165913), passed: false, reason: "write_score=0.65, status=Rejected, relevance=0.85, reuse=0.80, stability=0.76, trust=0.72, novelty=0.00, risk_reduction=0.65, token_cost=0.05, sensitivity_risk=0.00, kind=WorkflowConvention, stable=0.85, utility=0.80, specificity=0.80, volatility=0.20, duplication=1.00", rationale: "Duplicate memories should be rejected even when explicit." }]
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- memory::quality::tests::explicit_does_not_accept_duplicate_memory stdout ----

thread 'memory::quality::tests::explicit_does_not_accept_duplicate_memory' (47502) panicked at src/memory/quality.rs:276:9:
assertion `left != right` failed
  left: Accepted
 right: Accepted

---- memory::quality::tests::explicit_does_not_accept_low_quality_note stdout ----

thread 'memory::quality::tests::explicit_does_not_accept_low_quality_note' (47503) panicked at src/memory/quality.rs:267:9:
assertion `left != right` failed
  left: Accepted
 right: Accepted


failures:
    memory::calibration::tests::built_in_calibration_samples_pass
    memory::quality::tests::explicit_does_not_accept_duplicate_memory
    memory::quality::tests::explicit_does_not_accept_low_quality_note

test result: FAILED. 74 passed; 3 failed; 0 ignored; 0 measured; 901 filtered out; finished in 0.07s

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
warning: function `format_memory_write_outcome` is never used
   --> src/tui/app.rs:187:4
    |
187 | fn format_memory_write_outcome(
    |    ^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |
    = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default


running 978 tests
....................................................................................... 87/978
....................................................................................... 174/978
....................................................................................... 261/978
....................................................................................... 348/978
....................................................................................... 435/978
...................... 457/978
memory::calibration::tests::built_in_calibration_samples_pass --- FAILED
................................. 491/978
memory::quality::tests::explicit_does_not_accept_duplicate_memory --- FAILED
memory::quality::tests::explicit_does_not_accept_low_quality_note --- FAILED
....................................................................................... 580/978
....................................................................................... 667/978
....................................................................................... 754/978
....................................................................................... 841/978
....................................................................................... 928/978
..................................................
failures:

---- memory::calibration::tests::built_in_calibration_samples_pass stdout ----

thread 'memory::calibration::tests::built_in_calibration_samples_pass' (49205) panicked at src/memory/calibration.rs:197:9:
failed calibration samples: [MemoryCalibrationResult { id: "explicit_low_value_note", expected: NotAccepted, actual: Accepted, score: Some(0.56542045), passed: false, reason: "write_score=0.57, status=Proposed, relevance=0.65, reuse=0.65, stability=0.33, trust=0.70, novelty=1.00, risk_reduction=0.25, token_cost=0.05, sensitivity_risk=0.00, kind=Note, stable=0.65, utility=0.65, specificity=0.55, volatility=0.70, duplication=0.00", rationale: "Explicit save can lower friction but must not bypass quality gates." }, MemoryCalibrationResult { id: "duplicate_project_fact", expected: Rejected, actual: Accepted, score: Some(0.65165913), passed: false, reason: "write_score=0.65, status=Rejected, relevance=0.85, reuse=0.80, stability=0.76, trust=0.72, novelty=0.00, risk_reduction=0.65, token_cost=0.05, sensitivity_risk=0.00, kind=WorkflowConvention, stable=0.85, utility=0.80, specificity=0.80, volatility=0.20, duplication=1.00", rationale: "Duplicate memories should be rejected even when explicit." }]
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- memory::quality::tests::explicit_does_not_accept_duplicate_memory stdout ----

thread 'memory::quality::tests::explicit_does_not_accept_duplicate_memory' (49240) panicked at src/memory/quality.rs:276:9:
assertion `left != right` failed
  left: Accepted
 right: Accepted

---- memory::quality::tests::explicit_does_not_accept_low_quality_note stdout ----

thread 'memory::quality::tests::explicit_does_not_accept_low_quality_note' (49241) panicked at src/memory/quality.rs:267:9:
assertion `left != right` failed
  left: Accepted
 right: Accepted


failures:
    memory::calibration::tests::built_in_calibration_samples_pass
    memory::quality::tests::explicit_does_not_accept_duplicate_memory
    memory::quality::tests::explicit_does_not_accept_low_quality_note

test result: FAILED. 975 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 44.73s

error: test failed, to rerun pass `--bin priority-agent`
[exit status: 101]

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
