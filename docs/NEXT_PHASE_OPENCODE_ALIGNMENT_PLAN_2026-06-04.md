# Next Phase opencode Alignment Plan

Date: 2026-06-04

Status: proposed

## 1. Purpose

This plan records the next development phase after comparing Priority Agent
with the local opencode source in `/Users/georgexu/Downloads/opencode-dev`.

Priority Agent is now stable enough that the next phase should not be another
broad feature push. The goal is to learn from opencode's programming chain
ergonomics while preserving Priority Agent's stronger evidence, permission,
checkpoint, repair, memory, and validation contracts.

The target is a coding workflow that feels simpler from the product surface:
clear agent modes, clear plan/build transitions, clear edit previews, clear
post-edit diagnostics, clear rollback, and real durable usage/session
projections.

## 2. Evidence Reviewed

opencode source reviewed:

- `/Users/georgexu/Downloads/opencode-dev/packages/opencode/src/agent/agent.ts`
- `/Users/georgexu/Downloads/opencode-dev/packages/opencode/src/session/processor.ts`
- `/Users/georgexu/Downloads/opencode-dev/packages/opencode/src/session/llm/request.ts`
- `/Users/georgexu/Downloads/opencode-dev/packages/opencode/src/session/tools.ts`
- `/Users/georgexu/Downloads/opencode-dev/packages/opencode/src/session/reminders.ts`
- `/Users/georgexu/Downloads/opencode-dev/packages/opencode/src/session/instruction.ts`
- `/Users/georgexu/Downloads/opencode-dev/packages/opencode/src/tool/read.ts`
- `/Users/georgexu/Downloads/opencode-dev/packages/opencode/src/tool/edit.ts`
- `/Users/georgexu/Downloads/opencode-dev/packages/opencode/src/tool/apply_patch.ts`
- `/Users/georgexu/Downloads/opencode-dev/packages/opencode/src/tool/shell.ts`
- `/Users/georgexu/Downloads/opencode-dev/packages/opencode/src/lsp/diagnostic.ts`
- `/Users/georgexu/Downloads/opencode-dev/packages/opencode/src/snapshot/index.ts`
- `/Users/georgexu/Downloads/opencode-dev/packages/opencode/src/patch/index.ts`
- `/Users/georgexu/Downloads/opencode-dev/packages/core/src/database/migration/20260510033149_session_usage.ts`
- `/Users/georgexu/Downloads/opencode-dev/packages/core/src/database/migration/20260603001617_session_message_projection_indexes.ts`
- `/Users/georgexu/Downloads/opencode-dev/packages/core/src/database/migration/20260603141458_session_input_inbox.ts`

Priority Agent source reviewed:

- `src/engine/conversation_loop/mod.rs`
- `src/engine/conversation_loop/session_processor.rs`
- `src/engine/conversation_loop/request_preparation_controller.rs`
- `src/engine/auto_verify.rs`
- `src/engine/checkpoint.rs`
- `src/cost_tracker/usage_ledger.rs`
- `src/tools/file_tool/mod.rs`
- `src/tools/file_tool/patch.rs`
- `src/tools/file_tool/diagnostics.rs`
- `src/tools/plan_mode_tool/mod.rs`
- `src/tools/bash_tool/command_classifier.rs`
- `src/tools/bash_tool/command_classifier/shell_analysis.rs`

## 3. Current Assessment

Priority Agent is stronger than opencode in runtime proof behavior:

- validation and closeout are hard contracts rather than UI suggestions;
- file mutation has checkpoint and rollback semantics;
- context ledger and tool observations feed failed evidence back into repair;
- memory has review gates, diagnostics, and scoped retrieval;
- cache-stability and provider slow-tail instrumentation are already active;
- daily baseline and live-eval infrastructure are more explicit.

opencode is stronger in programming-chain product ergonomics:

- agent profiles are first-class product modes such as build, plan, explore,
  general, compaction, title, and summary;
- plan mode has a simple permission shape: deny edits except the plan file;
- edit and apply_patch results consistently carry diff, file summary,
  formatting, event publication, and LSP diagnostics;
- snapshots behave like a visible product feature with track, patch, diff,
  restore, and revert;
- session usage, message projection, and input inbox are SQLite-backed;
- shell permission inference uses tree-sitter for bash and PowerShell instead
  of relying only on string heuristics.

The gap is therefore not "can Priority Agent code correctly." The gap is that
Priority Agent exposes a more complex runtime than opencode. The next phase
should make the programming chain easier to see, debug, and trust without
reducing the existing hard gates.

## 4. Product Principles For This Phase

1. Do not rewrite the agent loop.
2. Do not weaken validation, permissions, checkpoints, rollback, or closeout.
3. Keep LLM semantic judgment in the model; keep runtime decisions
   deterministic, observable, and testable.
4. Make the user-visible workflow smaller than the internal runtime.
5. Prefer typed metadata and durable projections over prompt-only instructions.
6. Preserve the cache-stable prefix boundary; dynamic evidence belongs in the
   current-turn tail or persisted projections.
7. Add gates before broadening capability.

## 5. Phase 0: Baseline And Scope Lock

Goal: freeze the current stable behavior before changing the programming-chain
surface.

Work:

- Record current branch, dirty state, and daily baseline status.
- Run the narrow gates for files touched by this plan before starting code
  changes.
- Confirm that current usage ledger, closeout events, route-scoped tools,
  permission tests, checkpoint tests, and desktop/TUI runtime paths are green.
- Decide which existing docs should be status anchors:
  - `docs/PROJECT_STATUS.md`
  - `docs/PROJECT_MAP.md`
  - this plan document

Acceptance:

- A short implementation note is added to this document once work begins.
- No behavior changes land before the baseline is known.

Suggested gates:

```bash
cargo check -q
cargo test -q cost_tracker
cargo test -q route_scoped_tools
cargo test -q closeout
cargo test -q permissions
```

## 6. Phase 1: Product Agent Profiles

Goal: expose a small product-level mode model inspired by opencode, while
keeping the existing route-scoped tool logic underneath.

Recommended product profiles:

- `build`: normal coding mode; can inspect, edit, verify, and close out.
- `plan`: read-only planning mode; can write or update only a plan artifact.
- `explore`: fast read/search mode; no file mutation, no memory writes.
- `repair`: focused repair mode entered after failed validation or failed tool
  evidence; gets bounded repair context and required proof.

Work:

- Map current `AgentMode`, plan-mode tool behavior, intent routing, and
  route-scoped tools onto these product profiles.
- Add a single profile summary to trace/runtime diagnostic output.
- Make profile transitions explicit in TUI/desktop events.
- Keep all advanced tools route-scoped or slash-command scoped; avoid exposing
  a broad default surface in `build`.
- Ensure `plan` denies file write/edit/patch except a plan file path managed by
  the runtime.

Acceptance:

- The UI can show "mode: build/plan/explore/repair" without reading internal
  controller state.
- Tests prove edit tools are unavailable in plan/explore profiles.
- Build mode still reaches the existing validation and closeout gates.

Suggested gates:

```bash
cargo test -q plan_mode
cargo test -q route_scoped_tools
cargo test -q prompt_context
cargo check -q
```

## 7. Phase 2: Unified Edit Result Contract

Goal: make every file mutation return one consistent typed result that can feed
TUI, desktop, trace, repair, rollback, and daily baseline.

Current Priority Agent already records strong data through `file_patch`,
`file_edit`, checkpoints, diagnostics, and file history. The gap is consistency
and product presentation.

Required metadata shape:

- `operation`: `write`, `edit`, `patch`, `delete`, or `move`;
- `files`: list of changed files with resolved path, display path, status,
  additions, deletions, byte count, text format, and replacement count;
- `diff`: per-file diff plus combined diff hash;
- `checkpoint`: checkpoint id, sequence, session id, and restore eligibility;
- `file_changes`: durable file change ids;
- `diagnostics`: LSP status, error/warning counts, first error/warning, and
  collection status;
- `rollback`: present only when a partial write failed or restore was attempted;
- `ui_summary`: short stable string suitable for desktop/TUI cards.

Work:

- Introduce a shared Rust struct for file mutation result metadata.
- Adapt `file_write`, `file_edit`, and `file_patch` to emit the shared shape.
- Keep existing data fields during migration if callers still depend on them.
- Add tests that deserialize and assert the common shape for each mutation tool.

Acceptance:

- TUI and desktop no longer need per-tool ad hoc parsing for edit cards.
- Repair logic can find changed files, diff, diagnostics, and checkpoint from
  one metadata path.
- Existing checkpoint rollback behavior remains unchanged.

Suggested gates:

```bash
cargo test -q file_tool
cargo test -q checkpoint
cargo test -q closeout
cargo check -q
```

## 8. Phase 3: Post-Edit Diagnostics As A First-Class Loop Step

Goal: match opencode's edit UX where LSP diagnostics are attached immediately
after edits, but keep Priority Agent's stronger verification gates.

Work:

- Ensure `file_write`, `file_edit`, and `file_patch` all collect diagnostics
  through the same path.
- Normalize diagnostics into the shared edit result contract.
- Emit a trace event for post-edit diagnostics with file count, checked status,
  error count, warning count, and first error.
- If new errors appear, feed a compact `ToolObservation` or validation-style
  repair hint into the next model turn.
- Do not treat missing LSP as a hard failure. It should be `not_checked` or
  `unavailable`, while validation commands remain the hard proof surface.

Acceptance:

- After an edit, diagnostics are visible in tool output, trace, and desktop/TUI
  cards.
- New diagnostics can trigger bounded repair without pretending validation ran.
- Missing or slow LSP never blocks honest closeout by itself.

Suggested gates:

```bash
cargo test -q file_tool::diagnostics
cargo test -q focused_repair
cargo test -q closeout
cargo check -q
```

## 9. Phase 4: Checkpoint And Snapshot Productization

Goal: keep Priority Agent's checkpoint safety model, but expose it like a
usable product feature similar to opencode snapshot.

Work:

- Add a small checkpoint projection API:
  - latest file changes for session;
  - checkpoint restore eligibility;
  - combined diff for a tool round;
  - restore outcome summary.
- Surface this projection in TUI and desktop run cards.
- Add a command or tool path for "show last changes" and "restore last change"
  that uses existing checkpoint APIs instead of shelling out to git.
- Keep storage under `~/.priority-agent/checkpoints/<session_id>/`.
- Do not replace the current checkpoint implementation with a shadow git repo
  unless the existing backup-based model becomes insufficient.

Acceptance:

- A user can see what changed in the last tool round and whether it can be
  restored.
- Restore actions produce typed evidence and trace events.
- No unsafe restore happens without explicit user approval.

Suggested gates:

```bash
cargo test -q checkpoint
cargo test -q file_tool
cargo test -q permissions
cargo check -q
```

## 10. Phase 5: SQLite Session And Usage Projections

Goal: evolve the current JSONL usage ledger into queryable durable product
state, inspired by opencode's session usage and projection migrations.

Current state:

- `src/cost_tracker/usage_ledger.rs` records JSONL usage entries with session,
  model, prompt/completion tokens, cache hit/miss tokens, cost, stable prefix
  hash, tool schema hash, dynamic tail hash, and miss reason.

Next state:

- Keep JSONL append-only ledger as a simple audit log.
- Add an optional SQLite projection for fast queries and desktop status.
- Project usage by session, model, day, provider, stable prefix hash, tool
  schema hash, and miss reason.
- Let `/cost`, desktop status, and daily baseline read from the projection when
  available, with JSONL fallback.
- Add a small backfill command or startup repair path that rebuilds SQLite from
  JSONL.

Acceptance:

- `/cost` can answer current session, today, and by-model totals from durable
  state.
- Desktop status can show real cache hit rate and latest miss reason without
  relying on in-memory state.
- Daily baseline can archive a real usage summary artifact.
- Corrupt projection can be rebuilt from JSONL.

Suggested gates:

```bash
cargo test -q usage_ledger
cargo test -q cost_tracker
cargo test -q cost_tool
cargo check -q
```

## 11. Phase 6: Shell Parser Hardening

Goal: improve shell permission inference without making shell execution more
permissive.

opencode uses tree-sitter for bash and PowerShell to identify commands,
path-bearing arguments, external directories, and permission prompts. Priority
Agent already has a rich command classifier, but a structural parser would
reduce false positives and false negatives for compound commands.

Work:

- Start with bash only.
- Add a parser-backed analysis path behind a feature flag or environment flag.
- Compare parser-backed output against current classifier output in tests.
- Keep fail-closed behavior for parse errors and ambiguous write paths.
- Preserve existing dangerous-command and high-risk path gates.
- Consider PowerShell only after bash coverage is stable.

Acceptance:

- Parser-backed analysis identifies command segments, redirections, command
  substitutions, heredocs, cwd changes, and likely mutation paths.
- The permission classifier remains at least as strict as today's default.
- Regression tests cover common validation commands, read-only commands, file
  mutation commands, git mutation commands, and destructive commands.

Suggested gates:

```bash
cargo test -q bash_tool
cargo test -q permissions
cargo check -q
```

## 12. Phase 7: TUI And Desktop Programming-Chain Cards

Goal: make the polished programming chain visible to users.

Work:

- Add or update presentation adapters for:
  - profile/mode;
  - tool timeline;
  - diff summary;
  - diagnostics summary;
  - checkpoint/restore status;
  - usage/cache summary.
- Avoid duplicating runtime decisions in frontend code. Frontends should render
  typed events and typed metadata.
- Keep lightweight desktop turns explicitly separate from full-agent turns.

Acceptance:

- A desktop/TUI run clearly shows: plan/explore/build/repair, files changed,
  diagnostics, validation proof, checkpoint availability, and cost/cache
  summary.
- UI cards use the same metadata emitted by CLI tools and traces.
- There is no separate frontend-only interpretation of closeout or permission
  state.

Suggested gates:

```bash
cargo check -q
cd apps/desktop && npm test -- --runInBand
```

If the desktop test command is unavailable, run the narrowest existing desktop
typecheck or build command and record the fallback in this document.

## 13. Phase 8: Daily Baseline Coverage

Goal: make the opencode-inspired improvements hard to regress.

Add deterministic daily-baseline checks for:

- product profile snapshots;
- route-scoped tool surface snapshots;
- edit result metadata schema;
- post-edit diagnostics metadata;
- checkpoint projection;
- usage projection summary;
- shell classifier/parser parity;
- desktop/TUI event parity for profile, edit, diagnostics, closeout, and usage.

Acceptance:

- `scripts/daily-baseline.sh` includes the new gates or calls a subscript that
  does.
- The baseline report distinguishes runtime-flow failures from UI presentation
  failures.
- Weak-provider LLM mistakes are not treated as product-chain regressions unless
  the runtime failed to record, repair, validate, or close out honestly.

Suggested gates:

```bash
bash scripts/daily-baseline.sh
cargo test -q
cargo clippy --all-features -- -D warnings
```

## 14. What Not To Copy From opencode

Do not copy these parts directly:

- broad plugin/provider expansion as a near-term priority;
- prompt-heavy plan behavior that weakens deterministic gates;
- UI-specific runtime decisions;
- any behavior that treats LSP diagnostics as equivalent to validation proof;
- any shortcut that removes read-before-edit, checkpoint, rollback, permission,
  or closeout constraints.

Priority Agent's product advantage is not being a broader generic agent. The
advantage is a local programming partner with stronger memory, evidence,
validation, rollback, and machine-specific workflow.

## 15. Recommended Implementation Order

1. Phase 0 baseline.
2. Phase 2 unified edit result contract.
3. Phase 3 post-edit diagnostics loop.
4. Phase 4 checkpoint projection and restore UX.
5. Phase 5 SQLite usage projection.
6. Phase 1 product profiles, once metadata is stable.
7. Phase 7 TUI/desktop cards.
8. Phase 6 shell parser hardening.
9. Phase 8 daily baseline coverage.

This order keeps the first slices concrete and testable. Product profiles become
more useful after edit, diagnostics, checkpoint, and usage data already have
stable metadata.

## 16. Completion Checklist

- [ ] Baseline status recorded before implementation.
- [ ] Product profiles documented and tested.
- [ ] All file mutation tools emit one shared metadata contract.
- [ ] Post-edit diagnostics are visible in tool output, trace, TUI, and desktop.
- [ ] Checkpoint projection supports last-change viewing and approved restore.
- [ ] Usage JSONL has a SQLite projection with rebuild support.
- [ ] Shell parser hardening is gated and fail-closed.
- [ ] Daily baseline covers the new programming-chain contracts.
- [ ] `docs/PROJECT_STATUS.md` and `docs/PROJECT_MAP.md` are updated if runtime
      boundaries or product status materially change.
