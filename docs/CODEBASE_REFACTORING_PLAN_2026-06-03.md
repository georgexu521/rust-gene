# Codebase Refactoring Plan

Date: 2026-06-03
Updated: 2026-06-04
Scope: current working tree under `src/` and `apps/desktop/src-tauri/src/`

## Executive Summary

Priority Agent is still too large in several core modules. The current working
tree contains roughly 248k Rust lines across 490 Rust files. Excluding test
files and `_old.rs` backup files, the active production surface is roughly 459
Rust files:

| Budget | Active production files |
|--------|--------------------------|
| `> 500` lines | 168 |
| `> 800` lines | 76 |
| `> 1000` lines | 59 |
| `> 1200` lines | 41 |
| `> 1500` lines | 27 |

The goal is not to chase a mechanical line-count rule. The practical goal is
reviewable, testable, single-responsibility modules. A 500-line file budget is a
good target for this project, but it should be enforced progressively:

- `<= 500` lines: preferred steady state.
- `501-800` lines: acceptable when the file has one cohesive responsibility.
- `801-1200` lines: needs a local split plan or an explicit exception.
- `> 1200` lines: enters the active refactoring queue.
- `> 1500` lines: high-priority unless it is generated, test-only, or already
  being split.

Tests can exceed these budgets more often than production code, but large test
files should still be split when they hide distinct behavior lanes.

## Current Snapshot Notes

This snapshot was taken from a dirty working tree. In particular, the memory
manager refactor is now mostly closed:

- `src/memory/manager.rs` is deleted.
- `src/memory/manager/` exists with focused files.
- `docs/MEMORY_MANAGER_REFACTORING_PROGRESS_2026-06-03.md` exists but is
  untracked.
- `src/memory/manager/mod.rs` is 688 lines after the helper cleanup; the
  largest remaining memory-manager file is `tests.rs`, which is intentionally
  test-only.

Therefore, Phase 0 should be treated as cleanup verification rather than a new
source split.

## Refactoring Principles

1. One concern per file.
2. Preserve public API with `pub use` re-exports during migration.
3. Move tests only when it clarifies behavior; do not bury test-only churn in a
   source split.
4. Prefer extracting cohesive submodules over thin wrapper files that only move
   complexity around.
5. Keep `mod.rs` as an ownership boundary: small public surface, declarations,
   and re-exports.
6. Each split must be independently buildable and test-gated.
7. Avoid broad runtime behavior changes during file splits.

## Current Top Production Offenders

Excluding tests and `_old.rs` backup files:

| File | Lines | Priority | Recommended action |
|------|-------|----------|--------------------|
| `src/tools/agent_tool/mod.rs` | 1995 | P2 | Split after runtime/tool-core work stabilizes |
| `src/tools/bash_tool/command_classifier.rs` | 1974 | P2 | Split classifier tables and shell analysis helpers |
| `src/memory/provider.rs` | 1968 | P1 | Split provider traits, registry, local provider, migration |
| `src/tools/memory_tool/mod.rs` | 1952 | P1 | Split commands, rendering, validation, execution |
| `src/engine/mcp.rs` | 1908 | P2 | Split protocol/client/tool surface |
| `src/tui/app.rs` | 1893 | P1 | Continue moving runtime state and handlers out |
| `src/engine/scenario_matrix.rs` | 1885 | P2 | Split types, matrix construction, evaluation, reporting |
| `src/tui/slash_handler/agents.rs` | 1881 | P1 | Split doctor/status/cache/provider sections |
| `src/tui/commands.rs` | 1876 | P1 | Split registry, command metadata, execution adapters |
| `src/engine/auto_verify.rs` | 1823 | P1 | Split verifier orchestration, command policy, summaries |
| `apps/desktop/src-tauri/src/lib.rs` | 1803 | P1 | Split desktop commands/state/session bridge |
| `src/engine/task_context.rs` | 1787 | P1 | Split task bundle, context pack, serialization |
| `src/engine/action_review.rs` | 1736 | P1 | Split types, review policy, formatting |
| `src/tui/screens/main_screen.rs` | 1706 | P1 | Split status bar, transcript, panels |
| `src/engine/conversation_loop/tool_result_controller.rs` | 1695 | P1 | Split result parsing, proof extraction, ledger updates |
| `src/engine/evalset.rs` | 1693 | P1 | Split suite loading, execution, and reporting |
| `src/engine/skill_evolution.rs` | 1675 | P1 | Split analysis, proposal, and persistence lanes |
| `src/engine/intent_router.rs` | 1672 | P1 | Split intent scoring, route construction, and labels |

## Phase 0: Finish In-Progress Memory Manager Cleanup

The memory manager split is now structurally complete enough to stop blocking
Phase 1. Keep it under observation, but do not restart this refactor unless a
behavioral bug or a new size regression appears.

### Tasks

1. Verify `src/memory/manager/mod.rs` is the active manager entrypoint. Done.
2. Confirm `src/memory/manager_old.rs` is not referenced by any module. Done;
   the file is no longer present in `src/`.
3. Review `docs/MEMORY_MANAGER_REFACTORING_PROGRESS_2026-06-03.md` and either
   commit it with the split or fold its status into this plan.
4. Run the memory lane before starting the next memory-adjacent split.

### Acceptance Criteria

```bash
cargo fmt --check
cargo check -q
cargo test -q memory
cargo test -q memory_manager
```

### Done State

- No stale `_old.rs` file remains under `src/`.
- `src/memory/manager/mod.rs` stays below 1000 lines in the near term; it is
  currently 688 lines.
- The memory split has its own commit before broader refactors begin.

## Phase 1: Highest Impact Refactors

Phase 1 should focus on files that are both large and central to runtime
correctness. Each item should be a separate small commit.

### 1.1 Split `src/tools/mod.rs`

Status: mostly complete. `src/tools/mod.rs` is now a 145-line module surface,
with registry/result/schema/tool trait code extracted. The remaining follow-up
is `src/tools/tool_trait.rs` at 1093 lines, which should be split only after the
public tool contract stabilizes.

Proposed structure:

```text
src/tools/
├── mod.rs                  # public module declarations and re-exports
├── registry.rs             # ToolRegistry and registry construction
├── registry_profile.rs     # ToolRegistryProfile and env parsing
├── result.rs               # ToolResult, ToolErrorCode
├── schema.rs               # ToolSchema and metadata types
└── tool_trait.rs           # Tool trait and ToolContext
```

Acceptance:

```bash
cargo fmt --check
cargo check -q
cargo test -q tools
cargo test -q route_scoped_tools
```

Current follow-up:

- Keep the default Core profile narrow. Low-frequency tools such as agent,
  MCP, LSP, worktree, workbench, tool search, and project listing belong in the
  Full profile unless route-specific product work deliberately promotes them.
- Split `tool_trait.rs` into context, metadata, validation, and execution
  contract helpers if it remains above 1000 lines after the next tool-contract
  pass.

### 1.2 Split `src/engine/task_contract/`

Status: mostly complete. The file has moved to
`src/engine/task_contract/mod.rs`; base task/context/report types now live in
`types.rs`; memory proposal construction now lives in `memory_proposal.rs`;
proposal review persistence/batch operations now live in `proposal_store.rs`;
and proposal gates/evidence/scope checks now live in `proposal_gates.rs`. The
conflict grouping logic now lives in `proposal_conflict.rs`; background review
packet/worker logic now lives in `background_review.rs`. The `mod.rs` file is
now 728 lines, below the active queue threshold and inside the 501-800
acceptable band.

Remaining responsibilities:

- task contract derivation/formatting helpers. A future optional cleanup can
  extract these to `contract.rs` if we want to push below 500 lines.

Proposed structure:

```text
src/engine/task_contract/
├── mod.rs
├── background_review.rs
├── types.rs
├── contract.rs
├── memory_proposal.rs
├── proposal_store.rs
├── proposal_gates.rs
├── proposal_conflict.rs
└── background_review.rs
```

Acceptance:

```bash
cargo fmt --check
cargo check -q
cargo test -q task_contract
cargo test -q memory_proposals
```

### 1.3 Split `src/engine/trace/`

Status: complete enough for the current refactor queue. The path moved to
`src/engine/trace/mod.rs`; collector/store logic now lives in `collector.rs`,
user-facing trace summary/recent-line rendering now lives in `formatting.rs`,
diagnostics and latest-summary queries now live in `diagnostic.rs`, and
`TraceEvent` variants now live in `event.rs`. `TraceEvent::label` now lives in
`event_label.rs`. Workflow/task trace summaries now live in
`event_summary_workflow.rs`; the remaining summaries stay in
`event_summary.rs`. The entry module is down to 124 lines, while `event.rs` is
800 lines and `event_summary.rs` is 1019 lines. No trace production file now
exceeds 1200 lines.

Current responsibilities:

- `TraceEvent` definition lives in `event.rs`;
- event summaries;
- turn trace container;
- trace diagnostics and helper queries live in `diagnostic.rs`;
- event labels live in `event_label.rs`;
- workflow/task event summaries live in `event_summary_workflow.rs`;
- remaining event presentation lives in `event_summary.rs`.

Proposed structure:

```text
src/engine/trace/
├── mod.rs
├── event.rs
├── event_label.rs
├── event_summary.rs
├── event_summary_workflow.rs
├── collector.rs
├── diagnostic.rs
└── formatting.rs
```

Current next cuts:

- None required for the active queue. Future optional cleanup can split
  `event_summary.rs` further by memory/provider/tool families if that file grows
  again.

Acceptance:

```bash
cargo fmt --check
cargo check -q
cargo test -q trace
```

### 1.4 Split `src/engine/conversation_loop/tool_execution_controller.rs`

Status: entry split complete. Batch/result aggregation now lives in
`tool_execution_controller/batch.rs`, with the public conversation-loop surface
kept through `tool_execution_controller::ToolExecutionBatch`. Per-round runtime
context, runtime metadata attachment, observer action signals, and memory action
signals now live in `tool_execution_controller/runtime_context.rs`. Action
decision tracing, action review metadata, and tool-observation trace helpers now
live in `tool_execution_controller/action_review.rs`. The entry file is down to
907 lines, below the active queue threshold and below 1000. Permission/risk gate
logic now lives in `tool_execution_controller/gate.rs`.

Current responsibilities:

- tool execution orchestration;
- permission and risk gate;
- read-only parallelization;
- read-write serial execution;
- runtime context;
- observer/memory action signals;
- action review recording;
- lifecycle and storm suppression.

Proposed structure:

```text
src/engine/conversation_loop/
├── tool_execution_controller.rs          # controller entry and orchestration
└── tool_execution_controller/
    ├── batch.rs                          # batch/result aggregation
    ├── gate.rs                           # permission/risk gate
    ├── runtime_context.rs                # per-round context and action signals
    └── action_review.rs                  # action review trace/metadata helpers
```

Acceptance:

```bash
cargo fmt --check
cargo check -q
cargo test -q tool_execution
cargo test -q conversation_loop
```

## Phase 2: Runtime, Storage, And Product UI Slices

### 2.1 Split `src/session_store/mod.rs`

Status: started. Durable record/insert/upsert structs now live in
`src/session_store/records.rs` and are re-exported from `session_store`, keeping
existing public paths stable. Session CRUD methods now live in
`src/session_store/session_ops.rs`, and message add/get/delete/rewrite/restore
methods now live in `src/session_store/message_ops.rs`. Search/list methods now
live in `src/session_store/search.rs`. Compact boundary persistence now lives in
`src/session_store/compact_store.rs`. Agent artifact and task-state persistence
now lives in `src/session_store/agent_store.rs`. Turn trace persistence now
lives in `src/session_store/trace_store.rs`. Learning-event and context-ledger
persistence now lives in `src/session_store/learning_store.rs`.
`src/session_store/mod.rs` is down to 797 lines. Phase 2.1 is now below the
`801-1200` queue threshold; only split startup/migration helpers further if a
future behavior change touches that code.

Proposed structure:

```text
src/session_store/
├── mod.rs
├── records.rs
├── session_ops.rs
├── message_ops.rs
├── search.rs
├── compact_store.rs
├── agent_store.rs
├── trace_store.rs
├── learning_store.rs
└── migrations.rs
```

Acceptance:

```bash
cargo fmt --check
cargo check -q
cargo test -q session_store
```

### 2.2 Split `src/memory/provider.rs`

Status: complete enough for the current refactor queue. Provider contract types now live in
`src/memory/provider/types.rs`, and the `MemoryProvider` trait now lives in
`src/memory/provider/traits.rs`. `src/memory/provider.rs` remains the module
entrypoint and re-exports those items, so existing `crate::memory::provider::*`
paths stay stable. Registry orchestration now lives in
`src/memory/provider/registry.rs`, and the read-only fixture provider now lives
in `src/memory/provider/no_network_provider.rs`. The entry file is down to 1113
lines. Future optional cuts can split local-provider helper families if this
file grows again.

Proposed structure:

```text
src/memory/provider/
├── provider.rs             # current module entry; can move to mod.rs later
├── traits.rs
├── registry.rs
├── no_network_provider.rs
├── local_provider.rs
├── migration.rs
├── projection.rs
└── types.rs
```

Acceptance:

```bash
cargo fmt --check
cargo check -q
cargo test -q memory_provider
cargo test -q memory
```

### 2.3 Split `src/tools/memory_tool/mod.rs`

Status: started. Memory store path helpers and store-path rendering now live in
`src/tools/memory_tool/paths.rs`. The entry file is down to 1878 lines. The next
useful cuts are doctor JSON/report DTOs and doctor rendering helpers, then the
individual save/load/clear command implementations.

Proposed structure:

```text
src/tools/memory_tool/
├── mod.rs
├── paths.rs
├── commands.rs
├── render.rs
├── validation.rs
├── execute.rs
└── tests.rs
```

Acceptance:

```bash
cargo fmt --check
cargo check -q
cargo test -q memory_tool
cargo test -q memory
```

### 2.4 Split TUI Runtime Surfaces

Targets:

- `src/tui/app.rs`
- `src/tui/slash_handler/agents.rs`
- `src/tui/commands.rs`
- `src/tui/screens/main_screen.rs`

Recommended order:

1. Move provider/runtime facade synchronization out of `app.rs`.
2. Split `/doctor`, provider status, cache status, and agent listings out of
   `slash_handler/agents.rs`.
3. Split command metadata and execution handling out of `commands.rs`.
4. Split status bar/transcript/panels out of `main_screen.rs`.

Acceptance:

```bash
cargo fmt --check
cargo check -q
cargo test -q tui
cargo test -q prompt_context
```

### 2.5 Split Desktop Tauri Entry

Target: `apps/desktop/src-tauri/src/lib.rs`.

Proposed structure:

```text
apps/desktop/src-tauri/src/
├── lib.rs
├── commands.rs
├── session_commands.rs
├── runtime_commands.rs
├── app_state.rs
└── bridge.rs
```

Acceptance:

```bash
cargo check -q
cargo test -q desktop_runtime
cargo check --features experimental-api-server -q
```

## Phase 3: Lower-Risk Large Module Cleanup

These files are large, but they should wait until Phase 1 and Phase 2 reduce
the central runtime blast radius.

| File | Reason to defer |
|------|-----------------|
| `src/tools/agent_tool/mod.rs` | subagent behavior is product-sensitive |
| `src/tools/bash_tool/command_classifier.rs` | command safety classifier needs careful golden tests first |
| `src/engine/mcp.rs` | MCP behavior spans external integrations |
| `src/engine/scenario_matrix.rs` | eval/reporting shape should stay stable during daily baseline work |
| `src/engine/auto_verify.rs` | verification behavior is a correctness boundary |
| `src/engine/action_review.rs` | action review policy is safety-sensitive |
| `src/engine/evalset.rs` | eval workflows need stable baselines before reshaping |

## Per-Slice Checklist

For every refactor slice:

- [ ] Start from a clean or clearly understood dirty tree.
- [ ] Record pre-split line count for the target file.
- [ ] Extract one responsibility group only.
- [ ] Preserve public imports with re-exports.
- [ ] Avoid behavior changes unless explicitly called out.
- [ ] Move or add focused tests for the extracted responsibility.
- [ ] Run `cargo fmt --check`.
- [ ] Run `cargo check -q`.
- [ ] Run the narrow module tests.
- [ ] Commit the slice before moving to another target.

## Line Budget Gate

Add a lightweight script before enforcing this in CI:

```bash
scripts/report-rust-file-sizes.sh
```

Recommended output:

- total Rust files;
- active production files;
- test files;
- top 50 production files by line count;
- counts above 500, 800, 1000, 1200, and 1500 lines.

Recommended staged enforcement:

1. Report only.
2. Fail new production files above 1200 lines.
3. Fail modified production files above 1500 lines unless listed as an
   exception.
4. Later, lower the modified-file threshold to 1200 lines.

Do not immediately fail all files above 500 lines. That would make the current
tree noisy and reduce momentum.

## Exceptions

Allowed exceptions must be documented in this file or in the size-report script:

- generated code;
- large test fixtures;
- stable generated enum/table-like declarations;
- files actively being split in the current sprint;
- platform glue that cannot be split without making the API harder to follow.

Exceptions expire after one sprint unless renewed.

## Done Definition

This plan is complete when:

- no active production file exceeds 1500 lines;
- no stale `_old.rs` backup remains under `src/`;
- all files above 1200 lines have either been split or have an explicit
  exception;
- the top runtime/tool/TUI files are below 800-1200 lines;
- new production files target `<= 500` lines;
- the file-size report exists and is part of the regular engineering workflow;
- all touched slices pass their narrow gates and `cargo check -q`.

The long-term quality target is that ordinary production modules settle around
300-700 lines, with 500 lines as the preferred reviewable size.
