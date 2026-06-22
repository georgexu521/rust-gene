# Code Documentation Plan - 2026-06-22

Status: documentation standard and initial boundary-doc rollout implemented.

This plan defines how `priority-agent` adds code comments and rustdoc without
turning the codebase into noisy, mechanically commented code.

The goal is not to comment every line or every private helper. The goal is to
make module boundaries, public APIs, runtime contracts, and non-obvious logic
clear enough for release-grade maintenance.

## Current Implementation - 2026-06-22

Implemented in the initial rollout:

- Added this documentation standard and linked it from `docs/README.md`.
- Added `scripts/audit_rust_docs.py`, an advisory rustdoc audit for
  module-level docs and `pub` / `pub(crate)` items.
- Integrated the advisory audit into `scripts/validate_docs.sh` without making
  it fail the build.
- Added boundary-focused module docs across:
  - LabRun model, store, orchestrator, draft, and command modules;
  - streaming, permissions, tools, file/bash/agent tools, API routes, desktop
    runtime, TUI command registry, session store, provider API layer;
  - provider health, provider status, cache/cost diagnostics, checkpoint
    types, memory reports/provider registry, and TUI app state/action/palette.
- Verified that `cargo doc --no-deps -q` remains green after the documentation
  rollout.

Initial advisory audit baseline after this rollout:

```text
files scanned: 640
findings: 4366
missing-item-doc: 4366
missing-module-doc: 0
```

The audit remains advisory because the remaining findings are item-level gaps,
many of which are intentionally lower-value or generated-by-pattern items such
as command catalog constants and small builder methods. Future batches should
lower this count by documenting real public contracts first, not by adding
mechanical comments.

## Reference Standards

This plan follows the practical overlap of major public standards:

- Rust rustdoc guidance: public APIs should be documented; crate and module
  docs use `//!`; item docs use `///`; good docs explain purpose and usage, not
  only syntax.
- Rust API Guidelines: examples and prose should explain why an item matters;
  rustdoc should show useful API information and avoid exposing irrelevant
  implementation detail.
- Google style guidance: a large codebase is easier to understand when comment
  and documentation style is consistent; module/file docs should describe
  contents and usage.
- Microsoft comment conventions: public members should use formal doc comments;
  short `//` comments should explain non-obvious code; comments should be
  separate, readable lines instead of noisy trailing comments.

## Project Documentation Rule

Do not add comments just because an item exists.

Add documentation when it helps a future maintainer answer one of these
questions:

- What is this module or public item responsible for?
- What is deliberately out of scope?
- What runtime contract must callers preserve?
- What invariant, safety rule, permission rule, checkpoint rule, or validation
  rule matters?
- Why does this code choose a non-obvious behavior?
- How does this boundary connect two major parts of the product?

Avoid comments that only repeat the code:

```rust
// Increment count.
count += 1;
```

Prefer comments that explain intent or constraints:

```rust
// Keep the persisted projection bounded to the current turn so an old timeout
// cannot be rendered as the status of a new user request.
```

## Required Documentation

### Module Docs

Use `//!` at the top of modules that define a real boundary:

- runtime controllers and conversation-loop controllers;
- tool implementations and tool boundary helpers;
- permission, checkpoint, validation, and closeout modules;
- API route groups and DTO modules;
- LabRun command/orchestrator/store/draft/model modules;
- TUI app state, input, slash handlers, screens, and view-model modules;
- desktop runtime bridge modules;
- scripts or generated support modules only when they have non-obvious rules.

Module docs should answer:

- what the module owns;
- what it does not own;
- which callers use it;
- which invariants matter.

Keep module docs short by default: one to three paragraphs.

### Public Item Docs

Use `///` on:

- `pub` and `pub(crate)` structs, enums, traits, type aliases, and constants
  that cross module boundaries;
- `pub` and `pub(crate)` functions or methods used outside their module;
- feature-gated APIs;
- public test helpers shared across modules;
- DTOs that cross the API, desktop, TUI, session-store, or tool boundary.

Public item docs should usually include:

- one sentence explaining what the item is or does;
- caller responsibility if any;
- important failure behavior;
- relationship to persistence, permissions, validation, or runtime state when
  relevant.

Do not force examples on every item. Add examples only for APIs that are
intended to be used directly by external callers or by multiple internal
subsystems.

### Boundary Comments

Use short `//` comments at high-risk boundaries:

- frontend to runtime;
- API to runtime;
- tool execution to permission/checkpoint;
- validation to closeout;
- memory read/write/flush;
- LabRun stage transition and closeout;
- streaming events to persisted session projection;
- compatibility or migration behavior.

Boundary comments should explain why the boundary exists or what must not be
weakened.

### Complex Logic Comments

Use `//` comments inside functions only when the code has a non-obvious reason:

- retry and repair loops;
- timeout/cancel recovery;
- weak-provider repair paths;
- token budgeting and context compaction;
- command routing heuristics;
- file mutation safety;
- irreversible action guards;
- cross-platform shell behavior;
- fallback behavior that looks strange without context.

Do not comment every branch. Comment the decision point.

## Comment Style

- Use `//!` for crate/module docs.
- Use `///` for public item docs.
- Use `//` for local comments.
- Prefer complete, concise sentences.
- Keep comments in English for code consistency.
- Mention exact runtime contracts by name when useful:
  - permission gate;
  - checkpoint;
  - validation proof;
  - closeout;
  - ToolObservation;
  - RuntimeController;
  - SessionStore;
  - LabStore.
- Avoid vague wording such as "handles stuff", "do logic", or "helper".
- Avoid stale promises like "temporary" unless linked to an explicit follow-up
  doc or test.

## What Not To Document

Do not add rustdoc to every private helper.

Skip comments for:

- obvious private getters/setters;
- simple formatting helpers;
- test fixture builders whose name already explains the fixture;
- one-line conversions;
- local variables;
- branches where the condition already says the intent clearly.

If a private helper is important enough to need docs, consider whether it is a
module boundary or should have a clearer name.

## Rollout Batches

### Batch 1 - Documentation Standard And Guardrails

Scope:

- Add this plan as the project documentation standard.
- Add a lightweight audit command for missing docs on public Rust items.
- Decide whether the audit is advisory or blocking.

Recommended implementation:

- Start advisory.
- Report missing docs for `pub` and `pub(crate)` items in `src/`.
- Exclude test modules and generated/docs fixtures.
- Do not block CI until the first two documentation batches are complete.

Validation:

```bash
cargo fmt --check
cargo doc --no-deps -q
bash scripts/validate_docs.sh
```

Exit criteria:

- The standard is documented.
- The audit can produce a current missing-docs list without failing normal
  validation.

### Batch 2 - Runtime Boundary Docs

Scope:

- `src/engine/runtime_controller.rs`
- `src/engine/runtime_facade.rs`
- `src/engine/conversation_loop/`
- `src/engine/streaming.rs`
- `src/engine/streaming/`
- `src/engine/checkpoint/`
- `src/engine/auto_verify/`
- `src/permissions/`

Focus:

- Runtime vs LLM responsibility boundary.
- ToolObservation and failure feedback.
- Permission/checkpoint/validation/closeout invariants.
- Streaming event and persisted projection contracts.

Validation:

```bash
cargo doc --no-deps -q
cargo test -q closeout --lib
cargo test -q checkpoint --lib
cargo test -q streaming --lib
cargo clippy --all-targets --all-features -- -D warnings
```

Exit criteria:

- Public/runtime boundary types and functions have concise rustdoc.
- Complex retry/repair/closeout logic has intent comments where needed.
- No validation or permission language is weakened.

### Batch 3 - Tool Boundary Docs

Scope:

- `src/tools/mod.rs`
- `src/tools/registry.rs`
- high-risk tool modules:
  - `file_tool`
  - `bash_tool`
  - `agent_tool`
  - `worktree_tool`
  - `git_tool`
  - `mcp_tool`
  - `memory_tool`
  - `todo_tool`

Focus:

- Tool contract inputs/outputs.
- Mutation and checkpoint expectations.
- Permission behavior.
- Result metadata and recovery metadata.
- Read-before-write and high-risk path rules.

Validation:

```bash
cargo doc --no-deps -q
cargo test -q file_tool --lib
cargo test -q bash_tool --lib
cargo test -q agent_tool --lib
cargo clippy --all-targets --all-features -- -D warnings
```

Exit criteria:

- Tool boundary docs explain caller expectations and runtime guarantees.
- High-risk mutation paths have comments for why guards exist.

### Batch 4 - LabRun Docs

Scope:

- `src/lab/model.rs`
- `src/lab/model/`
- `src/lab/store.rs`
- `src/lab/store/`
- `src/lab/orchestrator.rs`
- `src/lab/orchestrator/`
- `src/lab/commands.rs`
- `src/lab/commands/`
- `src/lab/draft.rs`
- `src/lab/draft/`

Focus:

- Professor/postdoc/graduate role contracts.
- Stage transition invariants.
- Artifact source of truth.
- Evidence requirements.
- Deterministic vs provider-backed draft behavior.
- Closeout and recovery rules.

Validation:

```bash
cargo doc --no-deps -q
cargo test -q lab --lib
cargo clippy --all-targets --all-features -- -D warnings
```

Exit criteria:

- LabRun public models, store APIs, orchestrator entrypoints, and command
  surfaces have clear rustdoc.
- Placeholder behavior is documented honestly.

### Batch 5 - API, Desktop, TUI Surface Docs

Scope:

- `src/api/`
- `src/desktop_runtime/`
- `src/shell/`
- `src/tui/`
- `apps/desktop/src-tauri/src/`

Focus:

- Frontend-to-runtime command/event boundary.
- Lightweight non-agent lane vs full-agent lane.
- API route maturity and feature gating.
- TUI command maturity labels.
- Desktop runtime snapshot and event contracts.

Validation:

```bash
cargo doc --no-deps -q
cargo test -q tui::commands --lib
cargo test -q --features experimental-api-server api::routes --lib
cargo test -q
bash scripts/validate_docs.sh
```

Exit criteria:

- User-facing and integration-facing surfaces have clear docs.
- Unavailable or experimental routes are labeled consistently.

### Batch 6 - Internal Support Docs

Scope:

- `src/memory/`
- `src/session_store/`
- `src/services/`
- `src/instructions/`
- `src/skills/`
- `src/diagnostics/`
- `src/telemetry/`
- internal/historical modules:
  - `src/internal/ai_analyzer`
  - `src/internal/context_manager`
  - `src/internal/priority`
  - `src/internal/task_analyzer`
  - `src/internal/task_manager`
  - `src/internal/team`

Focus:

- Persistence boundaries.
- Provider lifecycle and credentials behavior.
- Prompt/instruction assembly contracts.
- Memory extraction, ranking, persistence, and flush behavior.
- Historical modules should be clearly marked as internal support if retained.

Validation:

```bash
cargo doc --no-deps -q
cargo test -q memory --lib
cargo test -q instructions --lib
cargo test -q prompt_context --lib
cargo clippy --all-targets --all-features -- -D warnings
```

Exit criteria:

- Important internal support modules have module-level docs.
- Retained historical modules are not confused with current product surfaces.

## Audit Strategy

The first audit should produce counts, not fail the build.

Suggested report fields:

- total public items scanned;
- missing `///` item docs;
- modules missing `//!`;
- ignored test/generated files;
- top 20 files by missing docs count.

After Batches 2-5, consider making the audit fail only for:

- missing docs on `pub` items;
- missing module docs for selected boundary directories;
- new missing docs introduced after the baseline.

Avoid making the audit fail on every private helper.

## Definition Of Done

This documentation rollout is complete when:

- project comment rules are documented and linked from `docs/README.md`;
- runtime, tool, LabRun, API, desktop, TUI, memory, and persistence boundaries
  have module docs;
- public and cross-module APIs have concise rustdoc;
- high-risk logic has useful intent comments;
- obvious private helpers are not mechanically commented;
- `cargo doc --no-deps -q` passes;
- `cargo clippy --all-targets --all-features -- -D warnings` passes;
- `cargo test -q` passes;
- `bash scripts/validate_docs.sh` passes.
