# Release Structure Cleanup Recommendations - 2026-06-22

Status: release-blocker cleanup and secondary large-file cleanup implemented;
full release gate sequence passes on the finished cleanup tree.

This note records the release-readiness cleanup work completed before treating
`priority-agent` as formally release-ready. The original audit found CI/docs
gate drift, release metadata placeholders, LabRun maintainability risk, and
several oversized production modules. Those items have now been addressed in
this cleanup branch; the remaining release claim depends on the final full gate
sequence passing on the finished tree.

## Current Progress - 2026-06-22

Completed in the first implementation slice:

- fixed the root `legacy-cli` feature mismatch;
- updated `Cargo.toml` release metadata;
- refreshed `docs/README.md`, `README.md`, and docs validation anchors;
- repaired the LabRun clippy failures without weakening runtime gates;
- split LabRun command, orchestrator, draft, model, and store files into
  responsibility-focused submodules;
- added baseline rustdoc comments to LabRun public models and entry APIs;
- restored green validation for:
  - `cargo fmt --check`
  - `cargo check -q`
  - `cargo check --features legacy-cli -q`
  - `cargo check --features experimental-api-server -q`
  - `cargo doc --no-deps -q`
  - `cargo clippy --lib --all-features -- -D warnings`
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - `cargo test -q lab --lib`
  - `bash scripts/validate_docs.sh`
  - `git diff --check`

Completed in the secondary large-file cleanup slice:

- split TUI input handling from `src/tui/mod.rs`;
- split TUI app runtime/session state helpers from `src/tui/app.rs`;
- moved shell LabRun command helpers and shell tests out of `src/shell/mod.rs`;
- split `/goal` learning slash-command handling from
  `src/tui/slash_handler/learning.rs`;
- split agent-tool support helpers from `src/tools/agent_tool/mod.rs`;
- moved bridge v1 API handlers from `src/api/routes/mod.rs` to
  `src/api/routes/bridge.rs`;
- moved streaming text/memory-flush helpers from `src/engine/streaming.rs`;
- moved session-action tests out of
  `src/tui/slash_handler/session/actions.rs`;
- confirmed all non-test production Rust files are now below the 1500-line
  project ceiling. The largest remaining direct production file is
  `src/tools/agent_tool/mod.rs` at 1496 lines.

Final validation after the secondary cleanup:

- `cargo fmt --check`: pass.
- `cargo check -q`: pass.
- `cargo check --features legacy-cli -q`: pass.
- `cargo check --features experimental-api-server -q`: pass.
- `cargo doc --no-deps -q`: pass.
- `cargo clippy --all-targets --all-features -- -D warnings`: pass.
- `cargo test -q`: pass; 3109 passed, 0 failed, 1 ignored in the main test
  target, plus all remaining integration/binary test targets passed.
- `bash scripts/validate_docs.sh`: pass; required docs present, tool registry
  count 72, command registry count 148, internal check/test pass.
- `git diff --check`: pass.

## Executive Summary

The project is not broken: the main compile path, formatting, experimental API
check, rustdoc, all-features clippy, docs validation, and LabRun targeted tests
were restored in the first implementation slice. The secondary cleanup then
removed the remaining production-file line-ceiling violations across TUI,
shell, API routes, agent tool, streaming, and session action modules.

The final release-readiness question should now be decided by the full gate
sequence on the finished tree, not by the original audit findings.

## Verified Baseline

Commands run during the audit:

```bash
cargo check -q
cargo fmt --check
git diff --check
cargo check --features experimental-api-server -q
cargo doc --no-deps -q
cargo test -q lab --lib
```

Results:

- `cargo check -q`: pass.
- `cargo fmt --check`: pass.
- `git diff --check`: pass.
- `cargo check --features experimental-api-server -q`: pass.
- `cargo doc --no-deps -q`: pass.
- `cargo test -q lab --lib`: pass, 254 tests.

Known failing gates from the original audit:

```bash
cargo clippy --all-targets --all-features -- -D warnings
bash scripts/validate_docs.sh
cargo check --features legacy-cli -q
```

These were repaired in the first implementation slice and must remain green in
the final gate sequence.

## Original Release Blockers and Resolution

### 1. All-features clippy

Original audit finding: `cargo clippy --all-targets --all-features --
-D warnings` failed with 15 errors concentrated in `src/lab`.

Observed examples:

- `src/lab/commands.rs:86`: needless borrow.
- `src/lab/commands.rs:4980`: manual backwards iteration.
- `src/lab/context.rs:44`: unnecessary `u64` cast.
- `src/lab/draft.rs:710`: `DoubleEndedIterator::last`.
- `src/lab/model.rs:34`: manual `Default` can be derived.
- `src/lab/orchestrator.rs:710`, `746`, `757`, `1445`: `DoubleEndedIterator::last`.
- `src/lab/scheduler.rs:214`, `484`, `599`: too many arguments.
- `src/lab/store.rs:1879`: too many arguments.
- `src/lab/store.rs:1916`: unnecessary `u64` cast.

Resolution:

- Mechanical LabRun clippy findings were fixed directly.
- Larger argument lists were handled with focused request/config structs where
  practical instead of blanket warning suppression.
- Final result: `cargo clippy --all-targets --all-features -- -D warnings`
  passes.

### 2. CI feature mismatch

Original audit finding: `.github/workflows/ci.yml` runs:

```bash
cargo check --features legacy-cli
cargo test --features legacy-cli
```

The root `priority-agent` package did not define `legacy-cli`; the feature
existed in `priority-core`, not in the root crate.

Resolution:

- Added a root `legacy-cli` feature that intentionally forwards to
  `priority-core/legacy-cli`.
- Final result: `cargo check --features legacy-cli -q` passes.

### 3. Docs validation and stale docs index

Original audit finding: `bash scripts/validate_docs.sh` failed because it
required
`docs/CLAUDE_CODE_ALIGNMENT_PLAN.md`, which is missing. `docs/README.md` also
linked to many missing historical docs.

Resolution:

- Updated `scripts/validate_docs.sh` required files to match the current
  canonical docs.
- Refreshed `docs/README.md` so the canonical section links to existing files.
- Kept `docs/PROJECT_STATUS.md` and `docs/PROJECT_MAP.md` as primary
  current-state anchors.
- Final result: `bash scripts/validate_docs.sh` passes.

### 4. Release metadata

Original audit finding: `Cargo.toml` contained template or outdated release
metadata:

- `authors = ["George Xu <your.email@example.com>"]`
- an outdated desktop-agent package description
- `repository = "https://github.com/yourusername/priority-agent"`

Resolution:

- Replaced placeholder author and repository values.
- Updated the description to match the current product: a Rust programming-agent
  terminal CLI with desktop workbench support.
- Confirm package name, version, license, keywords, and categories during the
  final publication checklist for the target distribution channel.

## Structure Cleanup

### LabRun maintainability risk was addressed

Original file sizes:

```text
9459 src/lab/commands.rs
6721 src/lab/orchestrator.rs
4003 src/lab/store.rs
3425 src/lab/draft.rs
1482 src/lab/model.rs
```

This area has meaningful test coverage, but the original file shape was too
large for release-grade maintenance.

Implemented split:

- `src/lab/commands/`: split command parsing, report commands, dashboard,
  sponsor-message commands, artifact commands, and scheduler commands.
- `src/lab/orchestrator/`: split stage transitions, artifact creation,
  professor review, meeting/blocker/revision handling, scheduler stepping, and
  closeout gates.
- `src/lab/store/`: split run persistence, artifact persistence, evidence refs,
  cost/usage, sponsor messages, and sqlite summaries.
- `src/lab/draft/`: split provider drafting, deterministic drafting, review,
  hybrid step loops, and prompt/schema helpers.
- `src/lab/model/`: split core run state, roles/tasks, artifacts, evidence,
  cost, scheduler/daemon state, and sponsor messages.

These refactors were kept mechanical and behavior-preserving, with targeted
LabRun tests and all-features clippy restored after the split.

### Other production files above the project line ceiling

Original non-LabRun production files above the 1500-line project ceiling
included:

```text
2787 src/tui/app.rs
2717 src/tui/mod.rs
1833 src/tui/slash_handler/learning.rs
1595 src/tools/agent_tool/mod.rs
1548 src/engine/streaming.rs
1530 src/tui/slash_handler/session/actions.rs
```

Implemented cleanup:

- `src/tui/app.rs`: runtime/session state helpers moved to
  `src/tui/app/runtime_state.rs`.
- `src/tui/mod.rs`: key input handling moved to `src/tui/input.rs`.
- `src/shell/mod.rs`: LabRun command helpers and shell tests moved to focused
  modules.
- `src/tui/slash_handler/learning.rs`: goal handling moved to
  `src/tui/slash_handler/learning/goal.rs`.
- `src/tools/agent_tool/mod.rs`: worktree/support helpers moved to
  `src/tools/agent_tool/support.rs`.
- `src/api/routes/mod.rs`: bridge v1 handlers moved to
  `src/api/routes/bridge.rs`.
- `src/engine/streaming.rs`: text-progress and session-end memory flush helpers
  moved to `src/engine/streaming/text_progress.rs`.
- `src/tui/slash_handler/session/actions.rs`: inline tests moved to
  `src/tui/slash_handler/session/actions/tests.rs`.

Post-cleanup scan: no non-test production Rust file exceeds 1500 lines.

## Public API Comments

`src/lab/model.rs`, `src/lab/store.rs`, `src/lab/orchestrator.rs`,
`src/lab/commands.rs`, and `src/lab/draft.rs` expose about 102 public
types/functions with no `///` documentation lines.

Recommendation:

- Add concise `///` docs to public LabRun artifact types, run state, store
  methods, orchestrator entrypoints, and provider/deterministic draft APIs.
- Avoid noisy comments on obvious private helper code.
- Document invariants that matter for maintenance: artifact source of truth,
  closeout rules, evidence requirements, scheduler behavior, and runtime vs LLM
  responsibility boundaries.

## Placeholder and Product Surface Cleanup

Observed user-visible or near-user-visible placeholder paths:

- `src/lab/orchestrator.rs`: deterministic professor review explicitly reports
  a runtime placeholder.
- `src/api/routes/mod.rs`: `stream=true` for session prompt is not implemented.
- `src/tui/slash_handler/learning.rs`: goal subcommands can return
  `not implemented yet`.
- `src/engine/intent_router.rs`: route candidate confidence uses a placeholder
  value.

Recommendation:

- Keep honest "not implemented" responses where functionality is intentionally
  unavailable.
- Do not advertise placeholder commands as production features.
- Make maturity explicit in help/docs: production, experimental, diagnostics,
  compatibility, or unavailable.
- Avoid weakening runtime validation, permissions, checkpoints, or closeout
  gates to make weaker providers appear greener.

## Documentation Cleanup

Current docs are useful but crowded. `docs/` has many top-level plan/audit
files, and `docs/README.md` contains stale links.

Recommendation:

- Keep only current anchors in the top canonical list:
  - `docs/PROJECT_STATUS.md`
  - `docs/PROJECT_MAP.md`
  - `docs/PERSONAL_AGENT_PRODUCT_PRINCIPLES_2026-05-18.md`
  - current release/desktop/LabRun docs that still reflect the code
- Move old plan documents into `docs/archive/` or clearly mark them as
  historical references.
- Update `README.md` baseline date and validation list after the release gates
  are actually green.
- Add a short `docs/RELEASE_READINESS.md` or update `QUALITY_GATES.md` with the
  exact release gate sequence.

## Work Batches

### Batch 1 - Restore green release gates

Status: complete.

Scope:

- Fix LabRun clippy errors.
- Fix CI `legacy-cli` feature mismatch.
- Fix `scripts/validate_docs.sh` required docs.
- Update `Cargo.toml` release metadata.

Validation:

```bash
cargo fmt --check
cargo check -q
cargo clippy --all-targets --all-features -- -D warnings
cargo check --features experimental-api-server -q
bash scripts/validate_docs.sh
git diff --check
```

### Batch 2 - Refresh docs entrypoints

Status: complete.

Scope:

- Rewrite `docs/README.md` current/canonical sections.
- Update `README.md` current baseline after gates pass.
- Align `docs/PROJECT_STATUS.md` known issues with current validation truth.
- Ensure no canonical doc link points to a missing file.

Validation:

```bash
bash scripts/validate_docs.sh
rg -n "CLAUDE_CODE_ALIGNMENT_PLAN|REMAINING_CLOSURE_PLAN|NEXT_DEVELOPMENT_PLAN" docs README.md scripts
```

### Batch 3 - Split LabRun by responsibility

Status: complete.

Scope:

- Extract command handlers from `src/lab/commands.rs`.
- Extract artifact builders and review/meeting/blocker flows from
  `src/lab/orchestrator.rs`.
- Extract store submodules for artifacts, evidence, cost, and sponsor messages.
- Extract provider/deterministic/hybrid drafting modules.

Validation after each slice:

```bash
cargo fmt --check
cargo test -q lab --lib
cargo check -q
```

Final validation:

```bash
cargo clippy --all-targets --all-features -- -D warnings
cargo test -q
```

### Batch 4 - Public API documentation pass

Status: complete.

Scope:

- Add `///` docs to public LabRun model and entry APIs.
- Add module-level docs to new LabRun submodules.
- Keep comments focused on invariants and cross-module contracts.

Validation:

```bash
cargo doc --no-deps -q
cargo test -q lab --lib
```

### Batch 5 - Secondary large-file cleanup

Status: complete.

Scope:

- Split TUI app and slash learning files.
- Split `src/tools/agent_tool/mod.rs`.
- Split `src/engine/streaming.rs` if new behavior is expected there.

Validation:

```bash
cargo fmt --check
cargo test -q tui --lib
cargo test -q agent_tool --lib
cargo test -q streaming --lib
cargo check -q
```

## Release-Ready Definition

Do not call the project formally release-ready until these are true:

- `cargo fmt --check` passes.
- `cargo check -q` passes.
- `cargo clippy --all-targets --all-features -- -D warnings` passes.
- `cargo test -q` passes or any failures are explicitly documented as
  pre-existing and not release-path failures.
- `cargo check --features experimental-api-server -q` passes.
- `bash scripts/validate_docs.sh` passes.
- CI feature references resolve against root package features.
- README and Cargo metadata no longer contain placeholder release values.
- Top-level docs do not link to missing canonical files.
- LabRun public APIs have basic docs and its largest modules are split enough
  that future changes can be reviewed locally.
