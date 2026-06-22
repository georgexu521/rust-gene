# Remaining Structure Refinement Plan - 2026-06-22

Status: implemented follow-up after release structure cleanup baseline.

The release cleanup in
`docs/RELEASE_STRUCTURE_CLEANUP_RECOMMENDATIONS_2026-06-22.md` brought the
repository back to a clean release-gate baseline: all non-test production Rust
files are under the 1500-line project ceiling, LabRun is split by
responsibility, and the full gate sequence passes.

This document tracks the remaining structure refinements completed after that
baseline. These were not release blockers; they make the project easier to
understand, package, and maintain.

## Implementation Summary - 2026-06-22

Completed in this refinement slice:

- Product wording now consistently presents `priority-agent` as a Rust
  programming-agent terminal CLI with local runtime, tools, memory, and desktop
  workbench support.
- TUI command maturity is explicit across six labels: `production`, `usable`,
  `experimental`, `diagnostics`, `placeholder`, and `unavailable`.
- Placeholder and unavailable commands remain hidden from default help and
  command-palette views unless explicitly queried.
- `src/lib.rs` now documents the public API intent and keeps clear
  internal/historical modules crate-private:
  - `src/internal/ai_analyzer`
  - `src/internal/context_manager`
  - `src/internal/priority`
  - `src/internal/task_analyzer`
  - `src/internal/task_manager`
  - `src/internal/team`
- `docs/PROJECT_MAP.md` now includes a source-tree grouping map.
- `scripts/check_source_file_sizes.sh` enforces the 1500-line ceiling for
  non-test production Rust files and is called by `scripts/validate_docs.sh`.

Targeted validation completed:

```bash
cargo fmt --check
cargo check -q
bash scripts/check_source_file_sizes.sh
rg -n "weighted priority desktop|desktop Agent|加权优先级桌面" README.md docs src --glob '!docs/archive/**' --glob '!docs/REMAINING_STRUCTURE_REFINEMENT_PLAN_2026-06-22.md'
cargo test -q tui::commands --lib
cargo test -q --features experimental-api-server api::routes --lib
cargo test -q lab --lib
```

## Current Assessment

The project is much cleaner than the pre-cleanup tree:

- LabRun now has focused submodules for commands, drafting, model types,
  orchestration, and persistence.
- TUI input handling, runtime state helpers, slash handlers, and screen
  rendering are more separated.
- API routes no longer keep bridge v1 handlers in the main routes file.
- Streaming text progress and session-end memory flushing are split from the
  core streaming engine.
- No non-test production Rust file currently exceeds the 1500-line project
  ceiling.

The remaining issues are mostly about public surface area, naming clarity, and
historical feature boundaries.

## Refinement Goals

- Make the top-level source tree easier to scan.
- Reduce accidental public API exposure from `src/lib.rs`.
- Keep near-ceiling files from becoming the next large-file problem.
- Clearly separate production features from experimental, diagnostic, or
  placeholder surfaces.
- Align product wording with the current product direction: Rust
  programming-agent terminal CLI with desktop workbench support.

## Batch 1 - Public Module Surface Audit

Status: implemented.

Problem:

`src/lib.rs` currently exposes most internal modules as `pub mod`. This is
convenient during development, but it makes the crate look like it has a broad
stable public API. For a formal release, that creates unnecessary compatibility
pressure and makes the architecture harder to read.

Scope:

- Classify each `src/lib.rs` module as one of:
  - public product API;
  - public only for binary/desktop/API entrypoints;
  - internal runtime implementation;
  - test-only support;
  - experimental feature-gated surface.
- Convert clearly internal modules from `pub mod` to `pub(crate) mod` where
  downstream visibility allows it.
- Keep feature-gated modules feature-gated.
- Add a short comment block in `src/lib.rs` explaining the intended public
  surface.

High-confidence candidates to review first:

- `src/internal/ai_analyzer`
- `src/internal/context_manager`
- `src/internal/task_analyzer`
- `src/internal/task_manager`
- `src/internal/team`
- `priority`
- `components`
- `tool_output_store`
- `desktop_runtime`

Implemented boundary:

- Moved `ai_analyzer`, `context_manager`, `priority`, `task_analyzer`,
  `task_manager`, and `team` under `src/internal/` as crate-private
  internal/historical support.
- Kept externally consumed modules public, including `desktop_runtime`,
  `tool_output_store`, `engine`, `lab`, `services`, `session_store`, `tools`,
  and `tui`.
- Added a public-surface intent comment to `src/lib.rs`.

Validation:

```bash
cargo fmt --check
cargo check -q
cargo check --features legacy-cli -q
cargo check --features experimental-api-server -q
cargo test -q
cargo clippy --all-targets --all-features -- -D warnings
```

Exit criteria:

- `src/lib.rs` exposes fewer internal modules directly.
- No binary, desktop, API, test, or feature-gated path loses required access.
- Public API intent is documented in one small comment block.

## Batch 2 - Top-Level Source Tree Grouping

Status: implemented as a documentation map; no source directories moved.

Problem:

`src/` currently has many top-level directories. Most are legitimate, but the
top-level namespace is wide enough that it is hard to distinguish core runtime,
product surfaces, adapters, and older support modules at a glance.

Scope:

- Create a proposed grouping map before moving files.
- Prefer low-risk namespace grouping over behavior changes.
- Move only modules whose dependency direction is clear.

Possible grouping direction:

- Runtime core:
  - `engine`
  - `memory`
  - `permissions`
  - `session_store`
  - `tools`
  - `instructions`
- Product surfaces:
  - `shell`
  - `tui`
  - `api`
  - `entry`
  - `desktop_runtime`
- Integrations and adapters:
  - `github`
  - `ide`
  - `plugins`
  - `remote`
  - `bridge`
  - `ports`
- Support and diagnostics:
  - `diagnostics`
  - `telemetry`
  - `quality_gates`
  - `slo`
  - `security`
- Experimental or historical candidates to review:
  - `src/internal/ai_analyzer`
  - `src/internal/context_manager`
  - `src/internal/priority`
  - `src/internal/task_analyzer`
  - `src/internal/task_manager`
  - `src/internal/team`

Recommended first step:

- Do not immediately move all folders.
- Add a short architecture map in `docs/PROJECT_MAP.md` or a focused new doc
  that classifies the current directories.
- After the map is agreed, move one small cluster at a time.

Implemented boundary:

- Added a `Source Tree Grouping` section to `docs/PROJECT_MAP.md`.
- Classified current top-level source directories into runtime core, product
  surfaces, integrations/adapters, support/diagnostics, and internal/historical
  support.
- Deferred physical directory moves to avoid risky churn without a separate
  agreed move plan.

Validation:

```bash
cargo fmt --check
cargo check -q
cargo test -q
rg -n "crate::(ai_analyzer|context_manager|task_analyzer|task_manager|team|priority)" src tests
```

Exit criteria:

- A new contributor can identify core runtime, product surfaces, integrations,
  and support modules without reading every directory.
- Any moved modules keep behavior and tests unchanged.

## Batch 3 - Near-Ceiling File Prevention

Status: implemented.

Problem:

The line-ceiling cleanup is complete, but several files are still close to the
1500-line limit. They are not release blockers, but they should be monitored so
future work does not recreate the old large-file pattern.

Current near-ceiling production files to watch:

```text
1496 src/tools/agent_tool/mod.rs
1489 src/tui/app.rs
1488 src/engine/scenario_matrix.rs
1487 src/engine/conversation_loop/request_preparation_controller.rs
1486 src/tools/bash_tool/command_classifier.rs
1485 src/tui/slash_handler/learning.rs
1479 src/tui/mod.rs
1478 src/cost_tracker/mod.rs
1470 src/engine/context_compressor.rs
1469 src/services/config.rs
```

Scope:

- Add a lightweight script or docs-check step that reports non-test production
  Rust files over 1500 lines.
- For each near-ceiling file, define the next natural extraction point before
  adding new behavior.
- Avoid splitting purely for aesthetics if the file is stable and cohesive.

Likely future extractions:

- `src/tools/agent_tool/mod.rs`: split execution orchestration from command
  construction and result finalization.
- `src/tui/app.rs`: continue moving cohesive state/update helpers into
  `src/tui/app/`.
- `src/engine/scenario_matrix.rs`: split model data, scoring, and report
  rendering.
- `src/tools/bash_tool/command_classifier.rs`: split parser rules, risk
  classifiers, and tests.
- `src/services/config.rs`: split provider config, runtime config, and
  environment resolution.

Validation:

```bash
find src -path '*/tests.rs' -prune -o -name '*.rs' -type f -print \
  | xargs wc -l \
  | awk '$2 != "total" && $1 > 1500 { print }'
cargo fmt --check
cargo check -q
cargo test -q
```

Exit criteria:

- New PRs have a clear signal when a production file crosses 1500 lines.
- Near-ceiling files have documented next extraction points.

Implemented guard:

- Added `scripts/check_source_file_sizes.sh`.
- Integrated it into `scripts/validate_docs.sh`.
- The guard excludes module-local test files and fails when a non-test
  production Rust file exceeds 1500 lines.

## Batch 4 - Placeholder and Maturity Surface Review

Status: implemented for TUI command surfaces and documented known runtime
placeholders.

Problem:

Some user-visible or near-user-visible surfaces intentionally return
placeholder or not-yet-implemented responses. These are not necessarily bad,
but they should be labeled consistently so the product does not overpromise.

Known surfaces to review:

- `src/api/routes/mod.rs`: `stream=true` for session prompt returns an honest
  not-implemented response.
- `src/tui/slash_handler/learning/goal.rs`: some `/goal` subcommands are
  marked Phase 1+ / not implemented.
- `src/lab/orchestrator/collaboration.rs`: deterministic professor review is a
  runtime placeholder and requires provider or explicit professor review before
  closeout.
- `src/engine/intent_router.rs`: route candidate confidence uses a placeholder
  score.
- `src/voice/mod.rs`: voice transcription and synthesis are feature-gated but
  not implemented.

Scope:

- Keep honest not-implemented responses where the functionality is unavailable.
- Ensure help text, command palette metadata, README, and docs do not present
  these as production-ready.
- Prefer explicit maturity labels:
  - production;
  - usable;
  - experimental;
  - diagnostics;
  - placeholder;
  - unavailable.
- Add tests for any user-facing maturity labels that should stay hidden unless
  explicitly requested.

Validation:

```bash
rg -n "not implemented|placeholder|Phase 1\\+|unavailable" src docs README.md
cargo test -q tui::commands --lib
cargo test -q api::routes --lib
cargo test -q lab --lib
```

Exit criteria:

- No placeholder capability is advertised as production-ready.
- Every intentional not-implemented response has clear wording and a test or
  documentation anchor.

Implemented maturity labels:

- Extended `CommandMaturity` with `experimental`, `diagnostics`, and
  `unavailable`.
- Kept `placeholder` and `unavailable` commands hidden from default help and
  command-palette views.
- Added/updated command maturity tests for explicit labels, disjoint maturity
  lists, maturity reports, and hidden default palette behavior.

## Batch 5 - Product Wording Cleanup

Status: implemented for current source and canonical docs.

Problem:

Some comments and older docs still reflect the original "weighted priority
desktop agent" framing. The current product direction is narrower and clearer:
`priority-agent` is a Rust programming-agent terminal CLI with a local runtime,
tools, memory, and desktop workbench support.

Scope:

- Update stale top-level comments, especially `src/main.rs`.
- Search docs and source comments for old product labels:
  - "weighted priority desktop Agent"
  - "desktop Agent"
  - "加权优先级桌面 Agent"
- Keep historical docs unchanged only when they are clearly archived or
  intentionally recording past direction.
- Align help text, README, and canonical docs with the current entrypoints:
  - `priority-agent`
  - `pa`
  - `--cli`
  - `--tui` compatibility mode
  - LabRun mode
  - experimental API server

Validation:

```bash
rg -n "weighted priority desktop|desktop Agent|加权优先级桌面" README.md docs src --glob '!docs/archive/**' --glob '!docs/REMAINING_STRUCTURE_REFINEMENT_PLAN_2026-06-22.md'
cargo fmt --check
cargo check -q
bash scripts/validate_docs.sh
```

Exit criteria:

- Current docs and source comments use the same product framing.
- Archived historical docs can keep old wording if clearly historical.

Implemented wording cleanup:

- Updated `src/main.rs` module docs.
- Updated `/about` output.
- Updated current README wording.
- Kept archived historical docs unchanged.

## Suggested Order

1. Batch 5 first if preparing public-facing release material. It is small and
   improves consistency quickly.
2. Batch 4 next to avoid overpromising unfinished surfaces.
3. Batch 1 before any external API/library claims.
4. Batch 3 as a lightweight guardrail before the next feature batch.
5. Batch 2 last, because directory moves create more churn and should be done
   only after the grouping map is agreed.

## Non-Goals

- Do not weaken validation, permissions, checkpoints, or closeout gates for
  structure cleanliness.
- Do not move modules just to reduce directory count if the dependency direction
  is unclear.
- Do not hide unfinished functionality by pretending it works.
- Do not convert all internal modules at once if that creates a large risky
  visibility diff.

## Definition of Done

This follow-up cleanup is complete when:

- `src/lib.rs` has a deliberate public/internal module boundary.
- Current product docs and comments use consistent release-facing wording.
- Placeholder and unavailable surfaces are consistently labeled.
- A line-ceiling guard exists for non-test production Rust files.
- Any source-tree grouping changes are documented and behavior-preserving.
- The full gate sequence passes:

```bash
cargo fmt --check
cargo check -q
cargo check --features legacy-cli -q
cargo check --features experimental-api-server -q
cargo doc --no-deps -q
cargo clippy --all-targets --all-features -- -D warnings
cargo test -q
bash scripts/validate_docs.sh
git diff --check
```
