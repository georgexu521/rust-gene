# Codebase Refactoring Plan

Date: 2026-06-03
Updated: 2026-06-04
Scope: current working tree under `src/` and `apps/desktop/src-tauri/src/`

## Executive Summary

Priority Agent is still too large in several core modules. The current working
tree contains roughly 249k Rust lines across 518 Rust files. Excluding test
files, `_old.rs` backup files, and Rust modules mounted only behind
`#[cfg(test)]`, the active production surface is roughly 219k lines across 477
Rust files:

| Budget | Active production files |
|--------|--------------------------|
| `> 500` lines | 169 |
| `> 800` lines | 73 |
| `> 1000` lines | 56 |
| `> 1200` lines | 33 |
| `> 1500` lines | 4 |

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
| `src/engine/mcp.rs` | 1908 | P2 | Split protocol/client/tool surface |
| `src/tui/app.rs` | 1893 | P1 | Continue moving runtime state and handlers out |

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
└── proposal_conflict.rs
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

Status: complete enough for the current refactor queue. Memory store path
helpers and store-path rendering now live in `src/tools/memory_tool/paths.rs`.
Doctor JSON/report DTOs now live in
`src/tools/memory_tool/doctor_types.rs`. Doctor text rendering helpers now live
in `src/tools/memory_tool/render.rs`, and doctor JSON construction now lives in
`src/tools/memory_tool/doctor_json.rs`. The entry file is down to 1187 lines.
Future optional cuts can split the individual save/load/clear command
implementations if this file grows again.

Proposed structure:

```text
src/tools/memory_tool/
├── mod.rs
├── paths.rs
├── doctor_types.rs
├── doctor_json.rs
├── render.rs
├── commands.rs
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

Status: started. Command catalog metadata now lives in
`src/tui/commands/catalog.rs`; `src/tui/commands.rs` remains the registry,
help/search, acceptance-behavior, and default-registration entrypoint. Public
command constants and `ALL_COMMANDS` continue to be re-exported from
`crate::tui::commands::*`, so callers keep the same import paths. The entry file
is down from 1876 lines to 692 lines. The extracted catalog is 1188 lines; it is
mostly static command declarations and can be split again later by command
family if the table becomes difficult to review. Agent listing and agent
worktree subcommands now live in `src/tui/slash_handler/agents/agent_listing.rs`;
doctor/product-readiness formatting helpers now live in
`src/tui/slash_handler/agents/doctor_formatting.rs`. The main
`src/tui/slash_handler/agents.rs` file is down from 1881 lines to 1437 lines and
has exited the `>1500` active queue. Related TUI slash-handler cleanup also
moved `/rewind`, `/diff`, and checkpoint diff/restore formatting helpers into
`src/tui/slash_handler/session/rewind.rs`; `src/tui/slash_handler/session.rs` is
down from 1607 lines to 1209 lines and has also exited the `>1500` queue.

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

Current structure:

```text
src/tui/
├── commands.rs                 # registry, help/search, registration logic
├── commands/
│   ├── catalog.rs              # command constants, maturity lists, ALL_COMMANDS
│   └── tests.rs
└── slash_handler/
    ├── agents/
    │   ├── agent_listing.rs    # /agents listing and worktree subcommands
    │   ├── doctor_formatting.rs # doctor cache/provider/readiness formatting
    │   └── tests.rs
    └── session/
        ├── actions.rs
        └── rewind.rs           # /rewind, /diff, checkpoint diff/restore helpers
```

Current next cuts:

- Split the remaining `/doctor` diagnostic assembly and gap snapshot out of
  `src/tui/slash_handler/agents.rs` if future edits touch that flow.
- Move provider/runtime facade synchronization out of `src/tui/app.rs`.
- Split status bar/transcript/panels out of
  `src/tui/screens/main_screen.rs`.
- Optionally split `commands/catalog.rs` by command family if static command
  declarations become hard to review.

Acceptance:

```bash
cargo fmt --check
cargo check -q
cargo test -q tui
cargo test -q prompt_context
```

### 2.5 Split `src/tools/file_tool/mod.rs`

Status: started. Edit matching helpers now live in
`src/tools/file_tool/edit_match.rs`: exact/fuzzy/normalized occurrence
matching, match-context formatting, file-read line-prefix guidance, and
occurrence line-number extraction. `src/tools/file_tool/mod.rs` is down from
1592 lines to 1443 lines and has exited the `>1500` active queue.

Current structure:

```text
src/tools/file_tool/
├── mod.rs                  # file operation entrypoint and execution flow
├── edit_match.rs           # edit target matching and diagnostic context
├── state.rs
└── tests.rs
```

Current next cuts:

- Split read/list/search result rendering if future file-read work touches that
  flow.
- Split write/edit/stale-conflict orchestration if edit behavior needs another
  correctness pass.
- Keep `edit_match.rs` pure and side-effect free so edit matching remains easy
  to unit-test.

Acceptance:

```bash
cargo fmt --check
cargo check -q
cargo test -q file_tool -- --test-threads=1
```

### 2.6 Split Desktop Tauri Entry

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

### 2.7 Split `src/engine/retrieval_context.rs`

Status: started. Retrieval item operations now live in
`src/engine/retrieval_context/item_ops.rs`: token estimation, preview/XML
escaping, item ID generation, dedupe key construction, ordering, and duplicate
merge formatting. The public
`crate::engine::retrieval_context::estimate_tokens` path is preserved by
re-export. `src/engine/retrieval_context.rs` is down from 1572 lines to 1385
lines and has exited the `>1500` active queue.

Current structure:

```text
src/engine/
├── retrieval_context.rs             # public retrieval context contract
└── retrieval_context/
    └── item_ops.rs                  # item identity, ordering, merge helpers
```

Current next cuts:

- Split memory scoring and scope/cap helpers if memory retrieval behavior needs
  another correctness pass.
- Split prompt rendering only if dynamic retrieval prompt work changes the XML
  block shape.
- Keep constructors and public DTOs in the entry file until call sites are
  stable enough for a directory-module migration.

Acceptance:

```bash
cargo fmt --check
cargo check -q
cargo test -q retrieval_context -- --test-threads=1
cargo test -q memory_retrieval -- --test-threads=1
```

### 2.8 Split `src/engine/conversation_loop/permission_controller.rs`

Status: started. Permission denial counters, denial-state JSON, recovery
feedback, and denied-message formatting now live in
`src/engine/conversation_loop/permission_recovery.rs`. The controller keeps the
approval evaluation flow, metadata construction, prompt construction, and
evidence pipeline. `src/engine/conversation_loop/permission_controller.rs` is
down from 1539 lines to 1414 lines and has exited the `>1500` active queue.
Moving the cleanup helper also removed the stale production dead-code warning
from this file while keeping it available inside the recovery module.

Current structure:

```text
src/engine/conversation_loop/
├── permission_controller.rs        # approval evaluation and evidence assembly
└── permission_recovery.rs          # denial counters and recovery feedback
```

Current next cuts:

- Split command/remote classification helpers if permission metadata grows
  again.
- Split prompt rendering only if the approval UI copy changes.
- Keep hard approval gates in the controller; do not move policy decisions into
  formatting-only helpers.

Acceptance:

```bash
cargo fmt --check
cargo check -q
cargo test -q permission_controller -- --test-threads=1
cargo test -q permission -- --test-threads=1
```

### 2.9 Split `src/engine/workflow_contract.rs`

Status: started. JSON sanitizer helpers now live in
`src/engine/workflow_contract/sanitize.rs`: guided-reasoning trigger
normalization, acceptance-review defaulting, criteria normalization, string
array normalization, and JSON value-to-text conversion. The entry file keeps
the public contract types, prompt builders, analyzer orchestration, parse
entrypoints, and JSON extraction. `src/engine/workflow_contract.rs` is down from
1594 lines to 1336 lines and has exited the `>1500` active queue.

Current structure:

```text
src/engine/
├── workflow_contract.rs              # public contract and analyzer entrypoint
└── workflow_contract/
    └── sanitize.rs                   # tolerant JSON cleanup helpers
```

Current next cuts:

- Split prompt builders if workflow prompt copy changes.
- Split weighting types and reweight helpers only if priority weighting gets
  another product pass.
- Keep parse entrypoints in the public file until callers no longer depend on
  the flat module path.

Acceptance:

```bash
cargo fmt --check
cargo check -q
cargo test -q workflow_contract -- --test-threads=1
```

### 2.10 Split `src/memory/eval.rs`

Status: started. Background-review eval cases now live in
`src/memory/eval/review_workflow.rs`: proposal-only write-boundary coverage and
multi-session background-review quality coverage. The entry file keeps the
suite runner, report types, retrieval/scope evals, proposal gate evals,
multi-session snapshot eval, migration eval, and shared temp/report helpers.
`src/memory/eval.rs` is down from 1598 lines to 1408 lines and has exited the
`>1500` active queue.

Current structure:

```text
src/memory/
├── eval.rs                    # suite runner and remaining eval families
└── eval/
    └── review_workflow.rs     # background-review eval fixtures
```

Current next cuts:

- Split proposal gate eval cases if memory proposal workflow changes again.
- Split migration fixtures only if memory migration behavior gets another
  product pass.
- Keep shared `pass`/`fail` report helpers in the entry file until more eval
  families are extracted.

Acceptance:

```bash
cargo fmt --check
cargo check -q
cargo test -q memory_eval -- --test-threads=1
```

### 2.11 Split `src/tools/bash_tool/mod.rs`

Status: started. Bash execution backend helpers now live in
`src/tools/bash_tool/execution_backend.rs`: backend parsing/defaults, timeout
floor handling, runtime environment cleanup, restricted-command wrapping,
external wrapper configuration, fallback backend parsing, and shell quoting
helpers used by wrapper construction. The entry file keeps command safety,
auto-background decisions, audit/result construction, PTY/background dispatch,
and the `Tool` implementation. `src/tools/bash_tool/mod.rs` is down from 1618
lines to 1440 lines and has exited the `>1500` active queue.

Current structure:

```text
src/tools/bash_tool/
├── mod.rs                    # bash tool execution flow and result assembly
├── execution_backend.rs      # backend/env/wrapper helpers
├── background.rs
├── command_classifier.rs
└── pty.rs
```

Current next cuts:

- Split result/audit data construction if shell output policy changes again.
- Split auto-background decision helpers if background task UX gets another
  product pass.
- Keep process execution and timeout handling in the entry file until PTY and
  foreground execution are split deliberately.

Acceptance:

```bash
cargo fmt --check
cargo check -q
cargo test -q bash_tool -- --test-threads=1
```

### 2.12 Split `src/engine/code_change_workflow.rs`

Status: started. Workflow helper functions now live in
`src/engine/code_change_workflow/helpers.rs`: programming-workflow
classification, no-diff closeout reason matching, runtime validation label
checks, bullet formatting, plan-step runtime state construction, uniqueness and
reason merging helpers, preview/validation evidence summarization, selected
validation evidence lookup, and path labeling. The public
`crate::engine::code_change_workflow::is_programming_workflow` path is
preserved by re-export. `src/engine/code_change_workflow.rs` is down from 1630
lines to 1499 lines and has exited the `>1500` active queue.

Current structure:

```text
src/engine/
├── code_change_workflow.rs             # workflow state and runner
└── code_change_workflow/
    └── helpers.rs                      # formatting and runtime helper funcs
```

Current next cuts:

- Split closeout rendering if final-response workflow copy changes.
- Split validation record construction only if stage validation behavior gets
  another product pass.
- Keep runner state transitions in the entry file until the workflow state
  machine has a dedicated module boundary.

Acceptance:

```bash
cargo fmt --check
cargo check -q
cargo test -q code_change_workflow -- --test-threads=1
```

### 2.13 Split `src/engine/streaming.rs`

Status: started. Turn message construction and reactive context retry helpers
now live in `src/engine/streaming/turn_messages.rs`. Provider fallback error
classification and bounded fallback state now live in
`src/engine/streaming/fallback.rs`. `src/engine/streaming.rs` is down from 1644
lines to 1449 lines and has exited the `>1500` active queue.

Current structure:

```text
src/engine/
├── streaming.rs                    # streaming turn orchestration
└── streaming/
    ├── fallback.rs                 # error classification and fallback state
    └── turn_messages.rs            # per-turn message/context construction
```

Current next cuts:

- Split provider event conversion only after the fallback and retry path stays
  stable under live provider tests.
- Split stream lifecycle metrics if slow-tail/provider productization adds more
  telemetry fields.
- Keep the main streaming turn loop in the entry file until provider behavior is
  covered by a daily baseline.

Acceptance:

```bash
cargo fmt --check
cargo check -q
cargo test -q streaming -- --test-threads=1
```

### 2.14 Split `src/engine/intent_router.rs`

Status: started. Intent keyword heuristics, route signal predicates, and route
tool recommendation helpers now live in `src/engine/intent_router/heuristics.rs`.
The entry file keeps the public route types, the `IntentRouter` decision order,
learning-feedback application, and existing route tests. `src/engine/intent_router.rs`
is down from 1672 lines to 1069 lines and has exited the `>1500` active queue.

Current structure:

```text
src/engine/
├── intent_router.rs                 # route types, route ordering, tests
└── intent_router/
    └── heuristics.rs                # signal predicates and tool recommendations
```

Current next cuts:

- Split learning-feedback adjustment only if route learning grows beyond a small
  confidence/tool modifier.
- Keep the route decision order in the entry file so intent precedence remains
  easy to audit.
- Add golden route fixtures before changing any keyword table or scoring
  heuristic.

Acceptance:

```bash
cargo fmt --check
cargo check -q
cargo test -q intent_router -- --test-threads=1
```

### 2.15 Split `src/engine/evalset.rs`

Status: started. Trace/replay expectation matcher helpers now live in
`src/engine/evalset/replay_matchers.rs`: tool sequence checks, terminal task
matching, run-context matching, checkpoint/rewind matching, context compaction
matching, runtime diet matching, subagent/worktree matching, MCP resource
matching, and MCP repair matching. `src/engine/evalset.rs` keeps suite loading,
report formatting, trace construction, and runner assertions. It is down from
1693 lines to 1230 lines and has exited the `>1500` active queue.

This slice also aligned `evalsets/feature_reality.yaml` with the current Core
tool profile: `project_list` and `tool_search` are no longer default Core tools.
For `unavailable_tools`, an absent registration now counts as unavailable, which
matches the "hidden when unwired" product contract.

Current structure:

```text
src/engine/
├── evalset.rs                       # loading, reporting, runner assertions
└── evalset/
    ├── external_baseline.rs
    ├── model.rs
    ├── replay_matchers.rs           # trace/replay expectation matching
    └── tests.rs
```

Current next cuts:

- Split report formatting if trend/baseline rendering grows again.
- Keep trace construction in the entry file until replay fixture shape
  stabilizes.
- Add golden tests before changing feature-reality tool profile expectations.

Acceptance:

```bash
cargo fmt --check
cargo check -q
cargo test -q evalset -- --test-threads=1
```

### 2.16 Split `src/engine/conversation_loop/tool_result_controller.rs`

Status: started. Provider-visible observation rendering and text utility
helpers now live in
`src/engine/conversation_loop/tool_result_controller/observation_render.rs`:
model visibility selection, raw/observation excerpt rendering, diagnostic line
extraction, failed-test extraction, diff-command detection, string field
collection, de-duplication, and safe truncation. The entry file keeps
normalization orchestration, observation construction, evidence facts, and
ledger recording. It is down from 1695 lines to 1407 lines and has exited the
`>1500` active queue.

Current structure:

```text
src/engine/conversation_loop/
├── tool_result_controller.rs          # normalization and evidence recording
└── tool_result_controller/
    └── observation_render.rs          # provider-visible observation text
```

Current next cuts:

- Split evidence fact classification if validation/proof policy changes again.
- Split observation field construction only with golden tests for normalized
  observation JSON.
- Keep ledger recording in the entry file until evidence boundaries stabilize.

Acceptance:

```bash
cargo fmt --check
cargo check -q
cargo test -q tool_result_controller -- --test-threads=1
```

### 2.17 Split `src/engine/skill_evolution.rs`

Status: started. The inline `#[cfg(test)]` module now lives in
`src/engine/skill_evolution/tests.rs`. This is intentionally a test-layout
split: production logic remains in the entry file, while behavioral coverage is
kept adjacent but out of the main production source. `src/engine/skill_evolution.rs`
is down from 1675 lines to 1367 lines and has exited the `>1500` active queue.

Current structure:

```text
src/engine/
├── skill_evolution.rs               # proposal, scoring, persistence helpers
└── skill_evolution/
    └── tests.rs                     # skill evolution unit tests
```

Current next cuts:

- Split fitness scoring if more promotion metrics are added.
- Split proposal persistence only if store format or migration behavior changes.
- Keep proposal generation in the entry file until skill-evolution product
  semantics stabilize.

Acceptance:

```bash
cargo fmt --check
cargo check -q
cargo test -q skill_evolution -- --test-threads=1
```

### 2.18 Split `src/tui/screens/main_screen.rs`

Status: started. Status bar rendering now lives in
`src/tui/screens/main_screen/status_bar.rs`, while the entry file keeps chat,
input, transcript, popup, sidebar, command-palette, model/provider picker, and
layout helpers. The public `render_status_bar` path is preserved by re-export.
`src/tui/screens/main_screen.rs` is down from 1706 lines to 1486 lines and has
exited the `>1500` active queue.

Current structure:

```text
src/tui/screens/
├── main_screen.rs                   # main screen layout and transcript UI
└── main_screen/
    ├── approvals.rs                 # permission approval popup
    ├── status_bar.rs                # status bar rendering
    └── tests.rs
```

Current next cuts:

- Split transcript windowing/render helpers if the chat area changes again.
- Split command/model/provider popups only with focused TUI snapshot tests.
- Keep top-level screen composition in the entry file until the layout is
  stable.

Acceptance:

```bash
cargo fmt --check
cargo check -q
cargo test -q main_screen -- --test-threads=1
```

### 2.19 Split `src/engine/action_review.rs`

Status: started. The inline `#[cfg(test)]` module now lives in
`src/engine/action_review/tests.rs`. This keeps safety-sensitive production
policy in the entry file while moving the large regression surface into an
adjacent test module. `src/engine/action_review.rs` is down from 1736 lines to
1162 lines and has exited both the `>1500` and `>1200` active queues.

Current structure:

```text
src/engine/
├── action_review.rs                 # action review policy and verdicts
└── action_review/
    └── tests.rs                     # action review unit tests
```

Current next cuts:

- Split permission/scope formatting only if policy copy grows again.
- Split checkpoint review helpers only with focused tests around rollback
  semantics.
- Keep final decision construction in the entry file until action-review policy
  stabilizes.

Acceptance:

```bash
cargo fmt --check
cargo check -q
cargo test -q action_review -- --test-threads=1
```

### 2.20 Split `src/engine/auto_verify.rs`

Status: started. Verification output parsers and Python verification helpers
now live in `src/engine/auto_verify/parsers.rs`: cargo check/test parsing,
Python mypy/pyright/pytest/py_compile parsing and changed-file verification,
TypeScript tsc/jest parsing, and Go build/test parsing. The entry file keeps
verification orchestration, command execution, workspace target resolution, and
language dispatch. `src/engine/auto_verify.rs` is down from 1823 lines to 1053
lines and has exited both the `>1500` and `>1200` active queues.

Current structure:

```text
src/engine/
├── auto_verify.rs                   # verification orchestration and dispatch
└── auto_verify/
    └── parsers.rs                   # language verification output parsing
```

Current next cuts:

- Split command execution/timeouts only if verifier process handling changes.
- Split workspace target resolution if Cargo workspace behavior grows again.
- Keep language dispatch in the entry file until daily baseline coverage is
  stable.

Acceptance:

```bash
cargo fmt --check
cargo check -q
cargo test -q auto_verify -- --test-threads=1
```

### 2.21 Split `src/engine/scenario_matrix.rs`

Status: started. Matrix report formatting and required-kind gap reporting now
live in `src/engine/scenario_matrix/report.rs`; the inline test module now
lives in `src/engine/scenario_matrix/tests.rs`. The entry file keeps scenario
types, required kind lists, deterministic scenario data, runtime-spine case
data, and summary counters. Public format/report functions are preserved by
re-export. `src/engine/scenario_matrix.rs` is down from 1885 lines to 1453
lines and has exited the `>1500` active queue.

Current structure:

```text
src/engine/
├── scenario_matrix.rs               # scenario and runtime-spine matrix data
└── scenario_matrix/
    ├── report.rs                    # matrix formatting and missing-kind checks
    └── tests.rs                     # scenario matrix unit tests
```

Current next cuts:

- Split runtime-spine data tables only if matrix data grows again.
- Keep scenario type definitions in the entry file until the matrix schema
  stabilizes.
- Add fixture-based report tests before changing rendered matrix format.

Acceptance:

```bash
cargo fmt --check
cargo check -q
cargo test -q scenario_matrix -- --test-threads=1
```

### 2.22 Split `src/engine/task_context.rs`

Status: started. `AgentTaskState` methods now live in
`src/engine/task_context/state.rs`. The entry file keeps task context types,
stage/status implementations, `TaskContextBundle` construction, and helper
functions. `src/engine/task_context.rs` is down from 1787 lines to 689 lines
and has exited both the `>1500` and `>1200` active queues. The new state module
is 1101 lines and remains below the `>1200` active queue.

Current structure:

```text
src/engine/
├── task_context.rs                  # task context types and bundle assembly
└── task_context/
    ├── state.rs                     # AgentTaskState transitions and helpers
    └── tests.rs                     # task context unit tests
```

Current next cuts:

- Split state transition policy only after the task-state contract stops
  changing.
- Keep persistence helpers with `AgentTaskState` until a storage boundary is
  introduced.
- Add fixture-based tests before changing serialized task-state fields.

Acceptance:

```bash
cargo fmt --check
cargo check -q
cargo test -q task_context -- --test-threads=1
```

### 2.23 Split `src/engine/conversation_loop/patch_repair_rules.rs`

Status: started. Deterministic patch repair action builders now live in
`src/engine/conversation_loop/patch_repair_rules/action_builders.rs`. The entry
file keeps the repair-rule registry, owner/review metadata, rule dispatch, and
tool-call validation. `src/engine/conversation_loop/patch_repair_rules.rs` is
down from 1551 lines to 291 lines and has exited the `>1500` active queue. The
new action-builder module is 1263 lines, so it remains in the `>1200` queue but
below the high-priority `>1500` budget.

Current structure:

```text
src/engine/conversation_loop/
├── patch_repair_rules.rs            # rule registry and dispatch
└── patch_repair_rules/
    └── action_builders.rs           # deterministic repair action builders
```

Current next cuts:

- Split action builders by behavior family once golden coverage is added for
  deterministic patch synthesis.
- Keep the rule registry in the entry file so owner/review metadata remains
  scan-friendly.
- Avoid changing generated patch payloads during size-only splits.

Acceptance:

```bash
cargo fmt --check
cargo check -q
cargo test -q patch_repair -- --test-threads=1
```

### 2.24 Split `apps/desktop/src-tauri/src/lib.rs`

Status: started. Desktop run-context DTOs and context enrichment helpers now
live in `apps/desktop/src-tauri/src/desktop_context.rs`. The Tauri command
entrypoints remain in `lib.rs`, which keeps the `generate_handler!` registration
stable. `apps/desktop/src-tauri/src/lib.rs` is down from 1803 lines to 1468
lines and has exited the `>1500` active queue. It remains in the `>1200` queue
and should continue to shed command/state groups in later slices.

Current structure:

```text
apps/desktop/src-tauri/src/
├── lib.rs                           # Tauri commands and app setup
├── desktop_context.rs               # run-context DTOs and enrichment helpers
├── desktop_state.rs                 # settings, projects, sessions, logging
└── diagnostics.rs                   # desktop diagnostics and folder opening
```

Current next cuts:

- Move provider/settings commands into a focused module once command handler
  registration is ready for path-qualified functions.
- Move native-smoke fixture emission out of `lib.rs` after preserving the
  existing smoke tests.
- Keep desktop context command registration in `lib.rs` until Tauri handler
  path changes are covered by a compile gate.

Acceptance:

```bash
cargo fmt --check
cargo check -q
(cd apps/desktop/src-tauri && cargo check -q)
(cd apps/desktop/src-tauri && cargo test -q desktop_run_context -- --test-threads=1)
```

## Phase 3: Lower-Risk Large Module Cleanup

These files are large, but they should wait until Phase 1 and Phase 2 reduce
the central runtime blast radius.

| File | Reason to defer |
|------|-----------------|
| `src/tools/agent_tool/mod.rs` | subagent behavior is product-sensitive |
| `src/tools/bash_tool/command_classifier.rs` | command safety classifier needs careful golden tests first |
| `src/engine/mcp.rs` | MCP behavior spans external integrations |

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

Use the existing lightweight report script before enforcing this in CI:

```bash
scripts/file-size-report.sh --threshold 1500 --top 30
```

Recommended output:

Current script behavior:

- reports large runtime, TUI, tool, desktop, script, and test files;
- classifies ordinary Rust test files and `#[cfg(test)]` module files as
  `rust_test`;
- tags large test-only files as `test_exception`;
- supports JSON output for future CI checks;
- supports `--fail-over` for staged enforcement.

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
