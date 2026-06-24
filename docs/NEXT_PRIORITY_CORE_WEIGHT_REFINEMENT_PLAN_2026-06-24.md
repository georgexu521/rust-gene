# Next Priority Core Weight Refinement Plan
Status: Implemented
Created: 2026-06-24
Implemented: 2026-06-24

## Purpose

This document turns the latest external review into the next scoped development
plan for Priority Agent.

The project is no longer broadly disorganized: repository identity, public
README wording, quality gates, cache defaults, credential disclosure, and
platform boundaries are materially clearer after the 2026-06-24 release-trust
cleanup.

The next quality pass should stay narrow. The main remaining maintenance risk is
the priority/weight line:

- `priority-core` now honestly describes itself as an experimental priority
  model crate.
- But `priority-core/src/weight_engine/mod.rs` still exports a compiled
  `WeightAnalysisTool` stub that returns empty weights.
- `priority-core/src/weight_engine/weight_skill.rs` contains a more complete
  weight analyzer, but it is not wired into the module tree and imports
  `crate::skills::{Skill, SkillMeta}`, which does not exist inside
  `priority-core`.
- `src/internal/priority` currently consumes the compiled stub through
  `priority_core::weight_engine::{WeightAnalysisResult, WeightAnalysisTool}`.

That means the code compiles, but the ownership story is still confusing and
the internal priority scheduler can receive empty weight analysis.

## Executive Conclusion

The external review is mostly correct.

Current project state:

- Good enough to keep dogfooding and alpha testing.
- Much cleaner than before for outside readers.
- Not yet clean enough to call the priority/weight subsystem mature.

The next phase should not be a broad architecture rewrite. It should close the
remaining priority-core ambiguity, make task-completion state deterministic,
refresh status docs, and make `scripts/validate_docs.sh` describe exactly what
it verifies.

## Evidence Checked Against Current Repo

| Finding | Current evidence | Assessment |
|---------|------------------|------------|
| `priority-core` has a compiled weight-analysis stub | `priority-core/src/weight_engine/mod.rs` exports `WeightAnalysisTool::analyze_project` that returns an empty `HashMap`. | Confirmed. This is the highest-value cleanup target. |
| `weight_skill.rs` is orphaned | `priority-core/src/weight_engine/mod.rs` only declares `calculator` and `types`; `weight_skill.rs` is not declared. | Confirmed. |
| `weight_skill.rs` cannot simply be wired into `priority-core` | It imports `crate::skills::{Skill, SkillMeta}`, but `priority-core` has no `skills` module. | Confirmed. This code belongs in the root crate if kept. |
| `src/internal/priority` uses the stub | It imports `priority_core::weight_engine::{WeightAnalysisResult, WeightAnalysisTool}` and then sorts tasks by the returned weights. | Confirmed. Empty weights fall back to `0.0`. |
| Task completion has two sources of truth | `Task.status == TaskStatus::Completed` is stored on the task, while dependency checks use `WeightCalculator.completed_tasks`. | Confirmed. A loaded project can say a dependency is completed while the calculator still treats it as incomplete. |
| `Weight::new` does not guard non-finite input | `Weight::new` currently stores `value.clamp(0.0, 1.0)`, which does not sanitize `NaN`. | Confirmed. Small but cheap correctness fix. |
| Status docs lag behind the latest cleanup | `docs/PROJECT_STATUS.md` says `Last updated: 2026-06-23`; README baseline still says `2026-06-22`. | Confirmed. |
| `scripts/validate_docs.sh` wording overclaims | It says it extracts Production/Usable commands and verifies implementations, but currently counts registry entries, checks file size, runs advisory rustdoc audit, check, and tests. | Confirmed. Fix wording or implement the stronger check. |

## Non-Goals

- Do not rename the product or repository again in this phase.
- Do not make `priority-core` depend on root-crate product concepts such as
  `Skill`, CLI commands, TUI surfaces, provider runtime, or orchestration.
- Do not broaden Windows support beyond the current documented best-effort
  boundary.
- Do not turn the docs validation script into a heavyweight replacement for CI
  unless the implementation remains fast and deterministic.

## P0: Priority/Weight Correctness And Ownership

### P0.1 Replace The Weight Analysis Stub

Problem:

- The currently exported `WeightAnalysisTool` in
  `priority-core/src/weight_engine/mod.rs` returns empty weights.
- `src/internal/priority::PriorityScheduler` consumes that exported stub.
- `weight_skill.rs` contains a more useful analyzer but is not compiled and
  has the wrong dependency direction for `priority-core`.

Plan:

1. Keep `priority-core` as a pure model/algorithm crate.
2. Move any product-level skill metadata out of `priority-core` if it is still
   needed.
3. Replace the stub with a real pure analysis wrapper backed by
   `WeightCalculator`.
4. Delete or relocate `priority-core/src/weight_engine/weight_skill.rs` so there
   is no orphan implementation.
5. Keep the public result type small and deterministic. Suggested fields:
   - `project_name`
   - `total_tasks`
   - `weights: Vec<(String, f64)>` or a documented map type
   - `next_task`
6. Update `src/internal/priority` to consume the real result shape.

Acceptance:

- `rg -n "stub|crate::skills|weight_skill" priority-core/src` returns no
  production orphan/stub implementation.
- `PriorityScheduler::create_execution_plan` no longer receives all-zero
  weights for a project with weighted tasks.
- `cargo test -q -p priority-core` passes.
- `cargo test -q internal::priority --lib` or the closest available focused
  internal-priority test passes.

### P0.2 Unify Completed-Task Semantics

Problem:

- `Task.status` can say a dependency is completed.
- `WeightCalculator.completed_tasks` can say it is not.
- Dependency checks currently rely only on the calculator overlay.

Plan:

1. Treat completed status on the project as the canonical persisted state.
2. Treat `WeightCalculator::mark_completed` as a session overlay or helper, not
   the only truth.
3. Build a merged completed set from:
   - tasks with `status == TaskStatus::Completed`
   - calculator-marked completed task IDs
4. Use that merged set for:
   - dependency satisfaction
   - executable task selection
   - progress report pending/blocked classification
5. Prefer `HashSet<TaskId>` for repeated membership checks.

Acceptance:

- A task whose dependency has `TaskStatus::Completed` becomes executable even
  without calling `mark_completed`.
- `mark_completed` still works for in-memory/session updates.
- `ProgressReport` blocked/pending counts match `get_executable_tasks`.
- Existing dependency-cycle tests continue to pass.

### P0.3 Sanitize `Weight::new`

Problem:

- The executable-task comparator now bounds non-finite priority scores, but
  `Weight::new` itself can still preserve `NaN`.

Plan:

```rust
pub fn new(value: f64) -> Self {
    if value.is_finite() {
        Self(value.clamp(0.0, 1.0))
    } else {
        Self(0.0)
    }
}
```

Tests:

- `Weight::new(f64::NAN).value() == 0.0`
- `Weight::new(f64::INFINITY).value() == 0.0`
- `Weight::new(f64::NEG_INFINITY).value() == 0.0`
- existing clamp tests still pass

Acceptance:

- No priority-core weight value can silently carry `NaN` or infinity through the
  public constructor.

## P1: Status And Validation Truthfulness

### P1.1 Refresh Project Status And README Baseline

Problem:

- The latest 2026-06-24 release-trust cleanup is recorded in the dedicated plan,
  but the main status entry and README baseline still point to older dates.

Plan:

1. Add a `Post-Review Release Trust Cleanup - 2026-06-24` section to
   `docs/PROJECT_STATUS.md`.
2. Summarize the completed changes:
   - repository/product naming contract
   - CI quality gates and release artifacts
   - cache fail-closed policy
   - priority comparator and dependency-depth cycle guard
   - credential/platform disclosures
3. Refresh README's latest baseline date and point readers to
   `QUALITY_GATES.md` plus the 2026-06-24 plan.
4. Keep `docs/PROJECT_STATUS.md` honest about this new follow-up: the project is
   cleaner, but priority/weight still has a focused cleanup lane until this plan
   is implemented.

Acceptance:

- README and `docs/PROJECT_STATUS.md` no longer imply that 2026-06-22 or
  2026-06-23 is the latest cleanup baseline.
- The status page names the remaining priority/weight work directly.

### P1.2 Make `scripts/validate_docs.sh` Match Its Actual Contract

Problem:

- The script comments currently imply a stronger doc/implementation check than
  the script performs.

Plan option A, preferred first:

- Rename the comments/output to describe the current behavior honestly:
  required docs, registry-count smoke checks, source-file ceiling, advisory
  rustdoc audit, `cargo check`, and workflow-enabled tests.

Plan option B, follow-up if needed:

- Actually parse `CAPABILITY_MATRIX.md` and compare Production/Usable commands
  against command/tool registries.

Command alignment:

- Consider whether the script should use the same check command as CI:

```bash
cargo check --workspace --all-targets --all-features
```

- If that makes the script too slow for local doc validation, leave the faster
  command but document the difference and point to `QUALITY_GATES.md` for full
  CI parity.

Acceptance:

- The script no longer overclaims what it validates.
- `bash scripts/validate_docs.sh` still passes.

## P2: Repo Hygiene Follow-Through

### P2.1 Keep Docs Root Current But Not Crowded

Current state:

- `docs/` root has 26 files, which is much cleaner than the previous 70+ root
  document state.
- The new plan can remain in root while active.

Plan:

- When this plan is implemented and closed out, decide whether the previous
  friend-review plan should move to `docs/archive/` or stay as a canonical
  release-trust reference.
- Do not archive current status, project map, quality gates, product principles,
  or active workstream plans.

Acceptance:

- `docs/README.md` can still guide a new contributor without making the root
  docs directory feel like a historical workbench.

### P2.2 Add Regression Searches To The Cleanup Closeout

Before closing this plan, run targeted searches:

```bash
rg -n "stub|orphan|TODO|crate::skills" priority-core/src
rg -n "WeightAnalysisTool|WeightAnalysisResult|weight_skill" priority-core/src src/internal/priority
rg -n "Last updated: 2026-06-23|Latest release-cleanup baseline recorded on 2026-06-22" README.md docs/PROJECT_STATUS.md
```

Acceptance:

- Any remaining hits are intentional and documented.

## Suggested Execution Order

1. Fix `priority-core` weight-analysis ownership:
   - replace stub with real pure analyzer
   - delete/relocate orphan `weight_skill.rs`
   - update focused tests
2. Unify completed-task semantics:
   - merge task status and session overlay
   - add dependency/report regression tests
3. Sanitize `Weight::new`:
   - finite guard
   - boundary tests
4. Refresh status docs:
   - README baseline
   - `docs/PROJECT_STATUS.md`
   - `docs/README.md` link if needed
5. Make validation script wording truthful:
   - update comments/output or implement real capability-matrix matching
6. Run validation and close out this document.

## Validation Gate For This Plan

Minimum focused gate:

```bash
cargo fmt --check
git diff --check
cargo test -q -p priority-core
cargo test -q internal::priority --lib
bash scripts/validate_docs.sh
```

Broader release-trust gate after touching shared contracts:

```bash
cargo check -q
cargo check --features legacy-cli -q
cargo check --features experimental-api-server -q
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features -- --test-threads=1
```

## Done Definition

This plan is complete when:

- `priority-core` has no compiled empty analyzer stub.
- `weight_skill.rs` is either removed or moved to the root crate where its
  `Skill` dependency belongs.
- `src/internal/priority` uses real calculated weights.
- completed task status and calculator-marked completions produce consistent
  dependency decisions.
- `Weight::new` rejects non-finite values.
- README and `docs/PROJECT_STATUS.md` reflect the 2026-06-24 cleanup baseline
  and this follow-up lane.
- `scripts/validate_docs.sh` no longer overclaims its checks.
- focused tests and docs validation pass.

## Implementation Closeout

Completed on 2026-06-24.

Implemented changes:

- Replaced the compiled empty `WeightAnalysisTool` stub with a real pure
  `WeightCalculator`-backed analysis wrapper.
- Removed the orphan `priority-core/src/weight_engine/weight_skill.rs` file
  because its `Skill` dependency belongs to the root crate, not `priority-core`.
- Added deterministic `WeightAnalysisResult` fields and a `weight_for_task`
  accessor used by `src/internal/priority`.
- Updated `PriorityScheduler::create_execution_plan` coverage so weighted tasks
  sort by real calculated weights.
- Unified dependency readiness around a merged completed-task set built from
  persisted `TaskStatus::Completed` values plus session-level
  `mark_completed` overlays.
- Updated progress reporting to use the same merged completed-task semantics as
  executable-task selection.
- Sanitized `Weight::new` so `NaN`, positive infinity, and negative infinity
  normalize to `0.0`.
- Refreshed README and `docs/PROJECT_STATUS.md` to the 2026-06-24 cleanup
  baseline.
- Updated `scripts/validate_docs.sh` to describe its actual validation contract
  and to run `cargo check --workspace --all-targets --all-features`.
- Kept the previous friend-review plan in `docs/` as a canonical
  release-trust reference while this follow-up plan remains an active/current
  closeout record.

Regression search results:

```bash
rg -n "stub|orphan|TODO|crate::skills" priority-core/src
rg -n "Last updated: 2026-06-23|Latest release-cleanup baseline recorded on 2026-06-22" README.md docs/PROJECT_STATUS.md
```

Both searches returned no hits. `WeightAnalysisTool` and `WeightAnalysisResult`
still appear intentionally as the real `priority-core` analysis API.

Validation evidence:

```bash
cargo fmt --check
git diff --check
cargo test -q -p priority-core
cargo test -q internal::priority --lib
bash scripts/validate_docs.sh
cargo check -q
cargo check --features legacy-cli -q
cargo check --features experimental-api-server -q
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features -- --test-threads=1
cargo doc --workspace --all-features --no-deps
```

Notable verification details:

- `cargo test -q -p priority-core` passed with 16 tests.
- `cargo test -q internal::priority --lib` passed the focused internal priority
  tests.
- `bash scripts/validate_docs.sh` passed with all required docs present, 72
  registered tools, 148 registered commands, source file ceiling clean,
  workspace all-targets/all-features check passing, and 3144 workflow-enabled
  tests passing with 0 failures and 1 ignored test.
- `cargo test --workspace --all-features -- --test-threads=1` passed with root,
  integration, `priority-core`, and doc-test targets green.
