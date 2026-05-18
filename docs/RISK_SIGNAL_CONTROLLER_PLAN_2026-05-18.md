# Risk Signal Controller Plan - 2026-05-18

## Goal

Make workflow-contract activation depend on concrete runtime signals instead
of long always-on prompting.

The intended behavior is:

- ordinary coding tasks stay light;
- risky runtime, permission, provider, memory, tool-execution, git, config, and
  schema changes become heavier before editing;
- required validation, broad/full-suite commands, and acceptance assertions
  become heavier;
- validation failures, syntax errors, and tool failures dynamically escalate
  repair behavior;
- small docs, fixture, style, and UI-copy changes stay ordinary unless another
  concrete risk signal is present.

## Implementation

- Added `RiskSignalController` as a deterministic controller under
  `src/engine/conversation_loop/`.
- Added a new adaptive workflow trigger, `risk_signal_high`, so high-risk
  signals upgrade the code-change policy to request workflow judgment, stage
  validation, final closeout, and at least two repair attempts.
- Moved `auto` workflow-contract entry activation to read the risk-signal
  assessment instead of the previous coarse `route high / bugfix / command
  count` check.
- Added `risk.signal` trace events for turn-entry assessment and runtime
  failure assessment.
- Surfaced risk-signal status and reasons in live-eval reports and summary
  tables.

## Current Risk Signals

High entry risk:

- route risk is already high;
- bug-fix workflow, unless all referenced files are low-risk fixture/style/docs
  surfaces;
- required validation commands are present;
- broad validation commands such as full cargo test/clippy/workspace checks are
  requested;
- multiple acceptance checks or explicit acceptance/assertion language is
  present;
- referenced paths touch core runtime areas such as conversation loop,
  workflow, tools, memory, providers, session store, permissions, config, or
  provider/schema/tool-execution surfaces;
- three or more referenced files, or two or more modules, unless all paths are
  low-risk fixture/style/docs surfaces;
- implementation requests mention runtime-sensitive domains such as provider,
  permission, memory, git, config, MCP, auth, public API, schema, migration, or
  compatibility.

Dynamic runtime risk:

- failed validation commands;
- syntax or parse errors in verification evidence;
- tool failures before useful progress.

## Validation

Implemented deterministic unit coverage for:

- core runtime path detection;
- multi-module risk detection;
- required-validation risk detection;
- ordinary fixture/style/UI-copy classification;
- dynamic validation/syntax failure escalation;
- policy upgrade through `risk_signal_high`;
- workflow-contract activation through risk-signal assessment.

Validation:

```text
cargo test -q risk_signal_controller -- --test-threads=1
cargo test -q workflow_runtime -- --test-threads=1
cargo test -q workflow_contract_controller -- --test-threads=1
cargo test -q turn_task_context_controller -- --test-threads=1
cargo test -q turn_context_bootstrap_controller -- --test-threads=1
cargo test -q high_risk_signal_trigger_requests_workflow_judgment -- --test-threads=1
cargo test -q required_validation_trigger_requests_workflow_judgment -- --test-threads=1
cargo test -q guided_tool_failure_debugging_only_runs_after_failed_tool_evidence -- --test-threads=1
cargo fmt --check
cargo check -q
cargo check --features experimental-api-server -q
cargo test -q
cargo clippy --all-features -- -D warnings
git diff --check
bash -n scripts/run_live_eval.sh
python3 -m py_compile scripts/live_eval_report_parser.py
bash scripts/live-eval-summary-smoke.sh
```

Full deterministic baseline after the change:

```text
1468 passed; 0 failed
```
