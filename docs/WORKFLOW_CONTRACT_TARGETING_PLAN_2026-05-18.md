# Workflow Contract Targeting Plan - 2026-05-18

## Goal

Keep the workflow contract layer as a high-risk repair and acceptance hardening
mechanism, not as a universal coding-task booster.

The 2026-05-18 ablation showed a mixed result:

- ordinary backend CRUD passed with the contract disabled and failed with it
  enabled because the model left a real `_TODO_PATH` / `_TODOS_PATH` typo;
- the repair-loop case passed with the contract enabled and failed with it
  disabled because the model preserved a bad acceptance pattern while claiming
  success.

The product direction is therefore targeted activation.

## Activation Policy

Add explicit workflow contract modes through `PRIORITY_AGENT_WORKFLOW_CONTRACT`:

- `off`: never run workflow-contract model calls.
- `force`: preserve the old behavior and run entry workflow judgment for every
  eligible programming route.
- `auto`: default. Run entry workflow judgment only when the turn is likely to
  need it:
  - high-risk route;
  - bug-fix/debugging route;
  - complex required-validation surface.

Failure-time guided debugging and repair review should remain available in
`auto` mode after validation failure, acceptance rejection, or tool-failure
evidence. This preserves the repair-loop strength without forcing ordinary
feature work through extra model judgment.

## Implementation Steps

1. Add a typed mode/parser in `workflow_runtime.rs`.
2. Route turn-entry workflow judgment through a decision object that records
   mode, active/skipped status, phase, and reason into the trace.
3. Keep guided tool-failure and validation-failure debugging gated only by
   global mode (`auto` or `force`, not `off`).
4. Surface activation decisions in live-eval reports and summaries.
5. Validate with targeted unit tests, then rerun the focused ablation cases.

## Success Criteria

- Existing `0/false/off/no` environment values still disable the contract.
- Existing `1/true/on/yes` values preserve force-on behavior for compatibility.
- Unset environment defaults to `auto`.
- Medium ordinary code-change routes skip entry workflow judgment in `auto`.
- High-risk, bug-fix, or complex validation routes run entry workflow judgment in
  `auto`.
- Guided debugging still runs after validation/tool failures in `auto`.
- Live-eval report output shows whether workflow contract was skipped, active at
  turn entry, or active during repair.

## 2026-05-18 Implementation Result

- Added typed `off` / `auto` / `force` workflow-contract modes.
- Kept legacy compatibility:
  - `0`, `false`, `off`, `no` disable the contract.
  - `1`, `true`, `on`, `yes` preserve force-on behavior.
  - unset now defaults to `auto`.
- Entry workflow judgment now records a `workflow_contract_activation` trace
  event and only runs in `auto` for high-risk routes, bug-fix routes, or complex
  required-validation surfaces.
- Guided debugging after tool/validation failure remains available in `auto`.
- Live-eval reports and summaries now include a `contract` column such as
  `entry=skipped:auto repair=none` or
  `entry=active:auto repair=active_after_failure`.

Validation:

```text
cargo test -q workflow_runtime -- --test-threads=1
cargo test -q workflow_contract_controller -- --test-threads=1
cargo test -q trace_summary_includes_runtime_diet_report -- --test-threads=1
cargo fmt --check
cargo check -q
cargo test -q
cargo clippy --all-features -- -D warnings
bash -n scripts/run_live_eval.sh
python3 -m py_compile scripts/live_eval_report_parser.py
```

Focused live auto run:

```text
workflow-contract-auto-targeting-20260518-154252: 3/4 passed
backend-todo-api-crud: failed, entry=skipped:auto repair=active_after_failure
frontend-book-notes-localstorage: passed, entry=skipped:auto repair=none
code-change-verification-repair-loop: passed, entry=active:auto repair=active_after_failure
core-permission-rejection-recovery: passed, entry=active:auto repair=not_needed
```

Interpretation:

- The targeting policy behaved as intended: ordinary medium feature work skipped
  entry judgment, while high-risk repair kept the contract active.
- The repair-loop strength survived the targeting change.
- The backend failure is not evidence that entry workflow contract is still
  over-triggering; its report shows entry was skipped. The model produced an
  invalid Python diff with a missing `__init__` method indentation, then stopped
  after repeated failed bash attempts.
