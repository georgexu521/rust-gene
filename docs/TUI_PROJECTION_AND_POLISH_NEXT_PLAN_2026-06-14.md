# TUI Projection And Polish Next Plan

Date: 2026-06-14
Status: Complete
Scope: `src/tui/`, `src/session_store/`, `src/engine/runtime_facade.rs`, TUI PTY/readiness scripts

## Goal

Move the TUI from "usable and tool-turn hardened" to "opencode-style mature enough
for daily use" without starting another broad UI rewrite.

The next phase should stay narrow:

1. Make message/part projection the only authoritative render source.
2. Prove the same contract with real providers, not just fixtures.
3. Improve the prompt/composer path where daily friction is still visible.
4. Tighten session/workspace UX around the projection model.
5. Keep plugin UI expansion behind safe boundaries.

This plan intentionally does **not** prioritize new sidebar panels, theme work,
large plugin runtimes, or desktop-specific surfaces until the projection spine is
fully stable.

---

## Current State

The TUI has moved in the right direction.

Current strengths:

- Runtime stream events are adapted through `SessionProjectionEvent`.
- `TuiSyncStore` owns `TuiSyncSnapshot`.
- `TuiMessagePart` represents text, thinking, and tool parts.
- `part_projection.rs` is the central reducer from projection events to TUI parts.
- Timeline rendering already consults `sync_snapshot.parts_for_message(...)`.
- Persisted `session_parts` can hydrate back into projection events.
- `scripts/tui_tool_turn_spine_readiness.sh` produces a readiness report.
- `scripts/tui_tool_turn_spine_nightly.sh` repeats the real-provider matrix.
- Recent TUI interaction polish landed:
  - collapsible tool/assistant output;
  - composer attachment pills;
  - leader pending state;
  - workspace switcher;
  - per-session UI state;
  - multi-select picker;
  - static plugin display slots.

Recent targeted gates have passed:

```bash
cargo test -q attachment --lib
cargo test -q app --lib
cargo fmt --check
cargo check -q
cargo clippy --all-targets --all-features -- -D warnings
git diff --check
```

The project is therefore ready for the next hardening pass, but it is not yet at
opencode TUI maturity.

---

## OpenCode Reference Points

Local source used for comparison:

`/Users/georgexu/Downloads/opencode-dev/packages/opencode/src/cli/cmd/tui/`

Relevant patterns:

- `context/sync-v2.tsx`
  - Owns a per-session message store.
  - Subscribes to typed session events.
  - Updates message content parts in one reducer-like path.
  - Tool lifecycle is represented as part state: pending, running, completed,
    error.
- `routes/session/index.tsx`
  - Renders `UserMessage` and `AssistantMessage` from message + parts.
  - Prompt is replaceable through plugin slots, but default prompt remains the
    normal path.
- `component/prompt/index.tsx`
  - Stores prompt parts separately from plain text.
  - Uses extmarks for virtual attachment labels.
  - Supports paste attachment intake and inline prompt state.
- `component/prompt/autocomplete.tsx`
  - Handles richer inline autocomplete than our current `@` file picker.
- `component/dialog-session-list.tsx`
  - Session list supports delete, rename, workspace warp, status display, and
    confirmation flows.
- `component/dialog-workspace-list.tsx`
  - Workspace list has current/expanded/deleting/removing states.
- `plugin/slots.tsx`, `plugin/runtime.ts`, `routes/session/sidebar.tsx`
  - Plugin slots are runtime surfaces, not only static text.

The useful lesson is not to copy the UI framework. The useful lesson is the
contract:

```text
runtime/session event -> sync store -> message/part model -> renderer
```

Every TUI improvement should strengthen that path.

---

## Current Local Touch Points

Projection and runtime:

- `src/session_store/projection_event.rs`
  - `SessionProjectionEvent`
  - `SessionProjectionEnvelope`
  - `SessionProjectionEventBus`
  - `from_stream_event(...)`
  - `from_persisted_part(...)`
- `src/tui/sync_store.rs`
  - `TuiSyncSnapshot`
  - `TuiMessageProjection`
  - `TuiMessagePart`
  - `tool_run_render_cache`
- `src/tui/part_projection.rs`
  - central event-to-part reducer.
- `src/tui/app.rs`
  - stream task converts `StreamEvent` into `SessionProjectionEvent`;
  - keeps `messages`, `sync_snapshot`, and other compatibility state.
- `src/tui/app/status_tools.rs`
  - hydration from persisted session parts back into projection events.
- `src/tui/view_model/timeline.rs`
  - builds render timeline from `MessageItem` plus `TuiMessagePart`.
- `src/engine/runtime_facade.rs`
  - consumes projection events for runtime state.

Prompt/composer:

- `src/tui/components/input.rs`
- `src/tui/components/attachment_token.rs`
- `src/tui/components/file_browser.rs`
- `src/tui/screens/main_screen/composer.rs`
- `src/tui/mod.rs`
- `src/tui/app.rs`

Session/workspace:

- `src/tui/session_manager/mod.rs`
- `src/tui/session_manager/export_builder.rs`
- `src/tui/slash_handler/session.rs`
- `src/tui/slash_handler/session/actions.rs`
- `src/tui/screens/main_screen.rs`
- `src/tui/screens/main_screen/popups.rs`

Readiness:

- `scripts/tui_tool_turn_spine_fixture_matrix.sh`
- `scripts/tui_tool_turn_spine_matrix.sh`
- `scripts/tui_tool_turn_spine_readiness.sh`
- `scripts/tui_tool_turn_spine_nightly.sh`
- `scripts/tui_readiness_report.py`

---

## Non-Goals For This Phase

- Do not build arbitrary plugin UI execution yet.
- Do not add a new frontend framework.
- Do not replace Ratatui wholesale.
- Do not make sidebar/theme/landing-style polish the priority.
- Do not weaken runtime validation, permission checks, evidence gates, or
  closeout proof to make a weak provider look better.
- Do not let the TUI create a second interpretation of tool state separate from
  `SessionProjectionEvent`.

---

## Phase 1: Make Projection The TUI Render Source Of Truth

### Problem

The TUI has a projection store, but several compatibility paths still exist:

- `TuiApp.messages` remains the base timeline list.
- `TuiSyncSnapshot.tool_run_render_cache` exists as a render/update cache.
- `visible_tool_runs()` and `projected_tool_runs()` merge multiple sources.
- Some tests still call `sync_snapshot.set_tool_runs_for_message(...)`
  directly.
- Hydration and live stream paths both exist, but the invariant is not yet
  explicit enough: "same events produce same render tree."

This is the main gap versus opencode's `sync-v2.tsx`, where the store is the
main model for session UI rendering.

### Proposed Work

1. Introduce a pure render model:

   ```rust
   pub struct TuiRenderSession {
       pub phase: TuiSessionPhase,
       pub messages: Vec<TuiRenderMessage>,
       pub last_projection_seq: u64,
       pub last_error: Option<String>,
   }

   pub struct TuiRenderMessage {
       pub id: String,
       pub role: TuiMessageRole,
       pub parts: Vec<TuiMessagePart>,
       pub metadata: HashMap<String, String>,
   }
   ```

   This should live near `src/tui/sync_store.rs` or a new
   `src/tui/render_session.rs`.

2. Add `TuiSyncSnapshot::render_session(&self, fallback_messages: &[MessageItem])`
   as the single bridge from legacy messages to renderable timeline state.

3. Move timeline construction to consume `TuiRenderSession` instead of
   `Vec<&MessageItem>` plus ad hoc part lookup.

4. Keep `tool_run_render_cache` only as a private implementation detail, or
   rename it to make the role clear:

   - acceptable: `derived_tool_run_cache`;
   - not acceptable: a public field that renderer treats as a fact source.

5. Replace direct renderer use of `projected_tool_runs()` where possible with
   part-derived helpers:

   - `TuiRenderMessage::tool_parts()`
   - `TuiRenderSession::all_tool_parts()`
   - `TuiRenderSession::active_tool_parts()`

6. Add tests that compare live and persisted hydration paths:

   - stream events -> projection -> render session;
   - persisted parts -> projection -> render session;
   - expected render session shape is identical.

### Files

- `src/tui/sync_store.rs`
- `src/tui/part_projection.rs`
- `src/tui/view_model/timeline.rs`
- `src/tui/app/actions.rs`
- `src/tui/app/status_tools.rs`
- `src/tui/screens/main_screen.rs`
- `src/tui/app/tests.rs`
- `src/tui/screens/main_screen/tests.rs`

### Acceptance

- [ ] Timeline renderer consumes a single render-session object.
- [ ] Tool rows displayed in the timeline are derived from message parts.
- [ ] `tool_run_render_cache` is private or clearly named as derived cache.
- [ ] Existing compatibility methods are reduced or marked temporary with a
      removal target.
- [ ] Live stream projection and persisted hydration produce the same render
      session in tests.
- [ ] Resume/export/diagnostic code does not need a separate tool-state query to
      explain visible TUI state.

### Validation

```bash
cargo test -q sync_store --lib
cargo test -q part_projection --lib
cargo test -q timeline --lib
cargo test -q app --lib
cargo test -q main_screen --lib
cargo check -q
```

---

## Phase 2: Real Provider Soak Uses The Same Readiness Contract

### Problem

Fixture readiness is strong, but opencode-level maturity needs evidence from
longer real-provider runs. This matters because the recent failures were mostly
OpenAI-compatible provider edge cases:

- malformed tool calls;
- delayed second-turn summaries;
- provider timeout after tool result;
- async-openai deserialization differences;
- weak-model partial compliance.

We already have scripts, but the next step is to make the report part of daily
development rather than a one-off manual run.

### Proposed Work

1. Standardize the real-provider nightly output:

   - keep `scripts/tui_tool_turn_spine_nightly.sh`;
   - ensure manifest includes provider/model/base URL family, timeout, DB
     recovery flag, and git SHA;
   - ensure readiness report includes fixture + real provider sections when
     both are available.

2. Add a short "last run summary" command or document command:

   - optional slash command: `/tui-ready`;
   - lower-risk first step: document how to read
     `target/tui-tool-turn-spine-nightly/<run-id>/_readiness/readiness.md`.

3. Expand real-provider matrix only along existing tool-turn dimensions:

   - one successful bash tool;
   - one failing bash tool;
   - long output;
   - invalid args;
   - multi tool;
   - partial failure;
   - malformed tool-call repair;
   - interrupt;
   - provider timeout after result.

4. Add thresholds:

   - fixture readiness must pass 100%;
   - real provider readiness can be WARN when provider/network failure is
     classified, but must not be silently green;
   - deserialization/raw JSON errors must be surfaced as provider/parser
     failures, not UI completion.

### Files

- `scripts/tui_tool_turn_spine_nightly.sh`
- `scripts/tui_tool_turn_spine_matrix.sh`
- `scripts/tui_readiness_report.py`
- `docs/PROJECT_STATUS.md`
- optional: `src/tui/slash_handler/observability.rs`

### Acceptance

- [ ] Nightly manifest records git SHA, provider, model, timeout, DB recovery
      mode, and round count.
- [ ] Readiness report distinguishes fixture, real-provider pass, provider
      failure, harness failure, and projection failure.
- [ ] Real-provider failure never reports "passed" without evidence.
- [ ] The report includes enough artifact paths to inspect ANSI/text output.
- [ ] A failed real-provider run still preserves useful diagnostics.

### Validation

```bash
bash scripts/tui_tool_turn_spine_readiness.sh
TUI_TOOL_TURN_SPINE_NIGHTLY_ROUNDS=1 bash scripts/tui_tool_turn_spine_nightly.sh
python3 -m py_compile scripts/tui_readiness_report.py
```

Run the nightly command only when API keys/network are intentionally available.

---

## Phase 3: Prompt Composer Parity Without Polluting InputState

### Problem

The composer is improved, but still behind opencode:

- `@` opens a reusable file picker, not inline fuzzy autocomplete.
- Attachment tokens render as simple bracketed pills, not true cursor-aware
  virtual spans.
- `composer_attachments` remains as a legacy compatibility vector.
- Pasted file and text paths are handled, but richer prompt parts are not yet a
  first-class model.

OpenCode's prompt keeps prompt parts separate from plain text and uses virtual
attachment labels. We can borrow that model without importing its UI framework.

### Proposed Work

1. Add a small composer model:

   ```rust
   pub struct ComposerState {
       pub text: InputState,
       pub parts: Vec<ComposerPart>,
   }

   pub enum ComposerPart {
       File(AttachmentToken),
       PastedText { id: String, label: String, content: String },
       Image { id: String, label: String, content: String },
   }
   ```

   Keep this as a TUI model. Do not push it into the engine prompt format until
   the submission path is explicit.

2. Migrate from:

   - `TuiApp.input`
   - `pasted_blocks`
   - `composer_attachment_tokens`
   - `composer_attachments`

   toward:

   - `TuiApp.composer: ComposerState`

   Keep compatibility accessors temporarily so slash commands and tests can be
   migrated incrementally.

3. Replace `@` picker with two-stage behavior:

   - MVP 1: `@` opens picker with filter already focused.
   - MVP 2: typing `@foo` in input opens inline suggestions and filters by
     `foo`.
   - MVP 3: selecting a file removes the `@foo` query and inserts a file part.

4. Add fuzzy matching helper in `file_browser.rs` or a new pure module:

   - ranked substring first;
   - path basename weighting;
   - directory-first option;
   - no filesystem scans outside workspace unless user explicitly chooses root.

5. Submission should build one explicit payload string from composer parts:

   ```text
   Attached context:
   - path

   Pasted context:
   - paste label

   User request:
   ...
   ```

   Later, export can record structured prompt parts separately.

### Files

- `src/tui/app.rs`
- `src/tui/components/input.rs`
- `src/tui/components/attachment_token.rs`
- `src/tui/components/file_browser.rs`
- `src/tui/screens/main_screen/composer.rs`
- `src/tui/mod.rs`
- `src/tui/slash_handler/agents/history_mode.rs`
- `src/tui/app/tests.rs`
- `src/tui/screens/main_screen/tests.rs`

### Acceptance

- [ ] `InputState` remains plain text and does not store attachment markers.
- [ ] Composer parts are the only source for attachment/paste/image summaries.
- [ ] Legacy `composer_attachments` is removed or fully deprecated behind
      accessors.
- [ ] Typing `@` opens an attachment picker with filtering immediately active.
- [ ] Typing `@foo` can narrow suggestions without submitting literal `@foo`.
- [ ] Backspace/delete behavior is deterministic for text vs parts.
- [ ] Submitted prompt includes all visible composer parts exactly once.

### Validation

```bash
cargo test -q input --lib
cargo test -q attachment --lib
cargo test -q file_browser --lib
cargo test -q composer --lib
cargo test -q app --lib
cargo test -q main_screen --lib
```

---

## Phase 4: Session And Workspace UX Around The Same Store

### Problem

The project now has session restore, delete, rename, fork, workspace grouping,
and `/back`, but opencode's session/workspace UX is still more complete:

- richer session list states;
- workspace labels and statuses;
- confirmation flows;
- workspace warp/create/delete;
- fork from timeline;
- session action dialogs;
- sidebar/plugin slots around session state.

We should improve the daily flows, but not by adding isolated state. Session UI
should read the same session/projection data that timeline and export use.

### Proposed Work

1. Define a `SessionListViewModel`:

   ```rust
   pub struct SessionListViewModel {
       pub current_session_id: Option<String>,
       pub rows: Vec<SessionListRow>,
       pub workspace_groups: Vec<WorkspaceGroup>,
       pub pending_delete: Option<String>,
       pub rename: Option<RenameState>,
   }
   ```

2. Move grouping/label logic out of `main_screen.rs` into a pure view-model
   module.

3. Make workspace status explicit:

   - current;
   - known;
   - missing path;
   - untagged legacy session;
   - restored and switched workspace.

4. Improve session actions:

   - rename flow has validation and cancel;
   - delete flow shows second-confirm hint and failure reason;
   - fork flow can optionally start from a selected message once timeline
     selection is stable;
   - resume restores projection parts before rendering first frame.

5. Keep workspace create/delete as a later item unless a local primitive already
   exists. Do not invent fake workspace semantics only for UI parity.

### Files

- `src/tui/screens/main_screen.rs`
- `src/tui/screens/main_screen/popups.rs`
- `src/tui/session_manager/mod.rs`
- `src/tui/session_manager/export_builder.rs`
- `src/tui/slash_handler/session.rs`
- `src/tui/slash_handler/session/actions.rs`
- new: `src/tui/view_model/session_list.rs`
- new or existing tests under `src/tui/screens/main_screen/tests.rs`

### Acceptance

- [ ] Session sidebar/list is rendered from a pure view model.
- [ ] Rename/delete/pin/current/workspace grouping are covered by unit tests.
- [ ] Restoring a session hydrates projection parts before the timeline is
      inspected.
- [ ] Session restore/export/readiness agree on message and tool part counts.
- [ ] Missing workspace roots are visible as a warning, not silently treated as
      current.

### Validation

```bash
cargo test -q session_manager --lib
cargo test -q session_list --lib
cargo test -q main_screen --lib
cargo test -q app --lib
bash scripts/tui_tool_turn_spine_readiness.sh
```

---

## Phase 5: Plugin UI Boundaries Stay Safe

### Problem

OpenCode has runtime plugin slots. Our current implementation supports safe
static display slots:

- `SidebarFooter`
- `StatusBar`

That is the right MVP. The next step should improve boundaries, not jump to
arbitrary plugin UI execution.

### Proposed Work

1. Keep static slots as the default.
2. Add a slot capability matrix in docs and `/plugins`.
3. Make deferred slots explicit:

   - `SidebarTitle`
   - `MessageBeforeSend`
   - `ToolCard`
   - prompt replacement

4. If dynamic slots are explored, require:

   - explicit feature flag;
   - trusted plugin source;
   - no filesystem/network side effects from render path;
   - deterministic timeout/failure handling;
   - visible fallback UI.

### Files

- `src/plugins/mod.rs`
- `src/tui/slash_handler/observability.rs`
- `src/tui/screens/main_screen.rs`
- `src/tui/view_model/footer.rs`
- docs for plugin slot contract

### Acceptance

- [ ] `/plugins` clearly shows active static slots and deferred dynamic slots.
- [ ] Unsupported slot declarations never crash TUI rendering.
- [ ] Static `panel.md` failures surface as plugin warnings.
- [ ] No arbitrary plugin code runs during TUI render by default.

### Validation

```bash
cargo test -q plugins --lib
cargo test -q observability --lib
cargo test -q footer --lib
```

---

## Phase 6: Polish And Release Readiness

This phase happens only after Phases 1-4 are stable.

Candidates:

- help/which-key style command hints;
- stronger toast/error taxonomy;
- theme density polish;
- keyboard shortcut audit;
- visual regression text snapshots for common TUI screens;
- public "TUI readiness" section in `docs/PROJECT_STATUS.md`.

Acceptance should be empirical:

- no raw provider JSON leaks into normal TUI;
- no false "done" status on provider/parser failure;
- long output remains inspectable without freezing;
- resume/export/replay do not duplicate tool rows;
- real-provider nightly has at least several consecutive usable runs.

---

## Suggested Execution Order

1. Phase 1: projection source-of-truth.
2. Phase 2: real-provider readiness reporting.
3. Phase 3 MVP 1: `@` opens focused filter, composer state cleanup.
4. Phase 4: session list view model and restore/export consistency.
5. Phase 3 MVP 2/3: inline `@foo` autocomplete and prompt parts.
6. Phase 5: plugin slot boundary polish.
7. Phase 6: UI polish only after the above is stable.

The first milestone should be small enough to complete in one focused pass:

```text
Milestone A:
- TuiRenderSession exists.
- Timeline renderer consumes it.
- live projection and persisted hydration produce identical render-session tests.
- readiness fixture still passes.
```

---

## Minimum Gates Before Marking This Plan Done

Code gates:

```bash
cargo fmt --check
cargo check -q
cargo test -q sync_store --lib
cargo test -q part_projection --lib
cargo test -q timeline --lib
cargo test -q app --lib
cargo test -q main_screen --lib
cargo clippy --all-targets --all-features -- -D warnings
git diff --check
```

TUI readiness gates:

```bash
bash scripts/tui_tool_turn_spine_readiness.sh
```

Real-provider gate, when API/network are intentionally available:

```bash
TUI_TOOL_TURN_SPINE_NIGHTLY_ROUNDS=1 bash scripts/tui_tool_turn_spine_nightly.sh
```

Documentation gates:

- Update `docs/PROJECT_STATUS.md` only after a phase lands.
- Keep this plan focused; do not add broad new product areas unless a real TUI
  testing gap requires it.

---

## Final Readiness Definition

This plan is complete when:

1. TUI rendering primarily reads `TuiRenderSession` / message parts.
2. Tool rows are derived from projection parts, not parallel TUI state.
3. Resume/export/readiness agree on message/part/tool counts.
4. Fixture readiness passes.
5. At least one real-provider readiness run is recorded, or skipped with a
   concrete environment reason.
6. Composer attachment/paste state has one fact source.
7. Session/workspace UI uses pure view models with tests.

At that point we can say the TUI is much closer to opencode's architecture and
daily-use shape. It still may not match opencode's full plugin ecosystem or UI
surface area, but the core product path will be mature enough for sustained
real use.
