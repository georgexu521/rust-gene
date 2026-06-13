# TUI Development Optimization Plan

Date: 2026-06-13  
Scope: `src/tui/` and related engine/session stores  
Goal: Close the highest-value gaps with OpenCode's TUI while preserving `priority-agent`'s existing runtime-panel and validation strengths.

---

## Background

This plan follows a gap analysis against `~/Downloads/opencode-dev/packages/opencode/src/cli/cmd/tui/` (OpenCode TUI, TypeScript + SolidJS + OpenTUI). OpenCode is ahead in five product/architecture areas:

1. Tool visualization (inline/block components, per-tool rendering, diff)
2. Keybindings and input (leader key, mode stack, text selection, attachments)
3. Session/workspace lifecycle (fork, child sessions, share/unshare, compact)
4. Mature message/part projection usage across all renderers
5. Plugin UI slots and KV preference persistence

The current `priority-agent` TUI is already strong in: command palette, runtime panels (`/panel`), validation closeout, session manager tests, tool-row visibility rules, and the new projection spine (`SessionProjectionEvent`, `TuiSyncSnapshot.parts_by_message_id`, persisted `session_parts`, and PTY readiness matrix). The plan below keeps those strengths and attacks the remaining gaps in dependency order.

---

## Phase 1: Finish Message/Part Projection Adoption (Foundation)

### Why first

Tool visualization, reasoning display, and session sharing all depend on renderers consuming one message/part projection. The project already has the core part model; the remaining work is to remove old renderer fallbacks that still parse assistant text or consult tool state through compatibility paths.

### Current state

- Engine emits `StreamEvent::{TextChunk, ThinkingStart/Chunk/Complete, ToolCallStart/Args/Complete, ...}` and TUI maps these through `SessionProjectionEvent`.
- `src/session_store/projection_event.rs` defines the event envelope/bus and persisted-part replay adapter.
- `TuiSyncSnapshot` already has `parts_by_message_id: HashMap<String, Vec<TuiMessagePart>>`, and `tool_run_render_cache` is documented as render/update cache only.
- `TuiMessagePart` has `kind: Text | Thinking | Tool`, `text`, `tool_run`, and `streaming`.
- `visible_timeline_messages()` already projects active assistant text from the sync snapshot.
- Remaining gap: `src/tui/view_model/timeline.rs` and `src/tui/components/message/assistant.rs` still mostly receive flattened `MessageItem.content`, and `src/tui/view_model/reasoning.rs` still parses `<think>` tags for rendering.

### Concrete work

1. **Promote `TuiMessagePart` to the canonical render input**
   - Add a timeline view model that carries `message_id`, role, and `Vec<TuiMessagePart>` for assistant messages.
   - Change `src/tui/components/message/assistant.rs` to render typed parts when present, falling back to plain `MessageItem.content` only for historical sessions that have not been hydrated.
   - Each part dispatches to its own mini-renderer: `TextPart`, `ThinkingPart`, `ToolPart`.
   - Keep `src/tui/view_model/reasoning.rs` as a backward-compatible importer for legacy `<think>` content, not as the normal live rendering path.

2. **Harden durable replay**
   - `src/session_store/session_parts/mod.rs` already has typed part storage and incremental projection.
   - Ensure every tool lifecycle event round-trips to a stable `SessionPart` with `part_id`, `message_id`, and `projected_to_seq`, so reloading a session reconstructs the same timeline.
   - Add tests that hydrate persisted `session_parts` into `TuiSyncSnapshot` and verify no duplicate tool parts, stable anchors, and monotonically advancing projection seq.

3. **Projection reducer in one place**
   - Keep `SessionProjectionEvent` as the adapter boundary; do not introduce a second StreamEvent-to-part path.
   - Extract the bulk of `sync_store.rs::apply_projection_event` into a pure reducer module, for example `src/tui/part_projection.rs`:
     - `fn project_event(state: &mut PartState, event: &SessionProjectionEvent)`
     - `fn apply_envelope(state: &mut PartState, envelope: &SessionProjectionEnvelope)`
     - `fn finalize_streaming_parts(state: &mut PartState)`
   - `TuiSyncStore` should become a thin owner of the reducer state and snapshot accessors.

### Files to change

- `src/tui/sync_store.rs` (thin wrapper around reducer)
- `src/tui/part_projection.rs` (new)
- `src/tui/components/message/assistant.rs`
- `src/tui/view_model/reasoning.rs` (demote to importer)
- `src/tui/view_model/timeline.rs` (carry part projections into renderer)
- `src/session_store/projection_event.rs`, `src/session_store/session_parts/mod.rs`, and tests

### Validation

```bash
cargo test -q sync_store --lib
cargo test -q session_parts --lib
cargo test -q reasoning --lib
cargo test -q timeline --lib
cargo test -q message --lib
```

### Acceptance criteria

- [ ] Assistant messages render as a sequence of typed parts when projection data exists.
- [ ] Reasoning no longer requires `<think>` text parsing for new live streams.
- [ ] Reloaded sessions reconstruct the same part timeline.
- [ ] `TuiMessagePart`/projection snapshot is the single source of truth for live TUI message rendering.

---

## Phase 2: Tool Visualization (Highest User Impact)

### Why now

Once renderers consume parts directly, each `ToolPart` can choose its own renderer. Today tools render through summary rows (`src/tui/tool_view.rs` and `src/tui/view_model/tool_rows.rs`). OpenCode renders each tool type differently: shell output blocks, file read cards, edit/patch diffs, glob/grep summaries, web fetch previews, etc.

### Current state

- `ToolRunView` holds structured `result_data: Option<Value>` in `src/tui/tool_view.rs:16-31`.
- `tool_kind_label` already classifies tools into Edit/Shell/Read/Search/Task/Network/Mcp/Plugin in `src/tui/tool_view.rs:122-156`.
- `render_diff_viewer` is a popup in `src/tui/components/diff_viewer.rs`.
- `tool_rows_for_runs_with_spine` decides visibility and severity in `src/tui/view_model/tool_rows.rs:62-79`.

### Concrete work

1. **Introduce per-tool renderers**
   Create `src/tui/components/tool_renderers/`:
   - `bash.rs` — collapsible output block with line count, exit code, runtime; expandable to full output.
   - `file_read.rs` — file path + line range + truncated indicator + "open" hint.
   - `file_edit.rs` — inline unified diff hunk (reuse `diff_viewer` logic but render inline).
   - `file_patch.rs` — multi-file diff summary with per-file +/- stats; expand to popup.
   - `grep.rs` / `glob.rs` — match count + first few matches.
   - `web_fetch.rs` / `web_search.rs` — title + URL + snippet.
   - `task.rs` — subagent task card with status, duration, and link to child session (when Phase 4 is done).

2. **Inline vs block toggle**
   - Keep canonical `TuiMessagePart` free of UI-only expansion state.
   - Add render state keyed by `part_id` or `tool_call_id` (for example `ToolPartRenderState`) to track expanded/collapsed UI state.
   - Default rule: failed/backgrounded/permission-denied tools start expanded; routine successful reads start collapsed.
   - `ctrl+o` toggles the focused tool; `ctrl+t` opens full output/diff popup.

3. **Reuse diff rendering**
   - Extract `diff_viewer.rs` parsing/highlighting into `src/tui/components/diff_renderer.rs` so both the popup and inline `file_edit`/`file_patch` cards can share it.

4. **Mutation result cards**
   - When `result_data` contains `FileMutationResult`, render the actual `+add/-del/bytes` summary inline instead of a generic "completed" line.

### Files to change

- `src/tui/components/message/assistant.rs` (render `ToolPart`)
- `src/tui/components/tool_renderers/` (new directory)
- `src/tui/components/diff_renderer.rs` (new, split from `diff_viewer.rs`)
- `src/tui/components/diff_viewer.rs` (use shared diff renderer)
- `src/tui/tool_view.rs` (keep summary text fallback)
- `src/tui/view_model/tool_rows.rs` (decide default expand/collapse)

### Validation

```bash
cargo test -q tool_view --lib
cargo test -q tool_rows --lib
cargo test -q diff_viewer --lib
cargo test -q message --lib
cargo test -q app --lib
```

### Acceptance criteria

- [ ] Each major tool family has a dedicated inline renderer.
- [ ] Diff/popup logic is shared between inline cards and `/diff` panel.
- [ ] Successful routine read-only tools collapse by default; failures and mutations stay visible.
- [ ] `ctrl+o` and `ctrl+t` still work on the focused tool.

---

## Phase 3: Keybindings and Input Overhaul

### Why after parts/tools

The input box needs better editing ergonomics and tool cards need local keybindings. It is easier to add those once the message/tool components are stable.

### Current state

- `src/tui/keybindings.rs`: TOML-overridable actions, but some global shortcuts still live directly in `src/tui/mod.rs`.
- `src/tui/components/input.rs`: basic cursor movement, insert/delete, multi-line, CJK width.
- Global shortcuts are partially scattered in `src/tui/mod.rs`.
- Attachment picker/context support exists, but paste-driven attachment intake and richer input editing are not mature.
- Clipboard dependency (`arboard`) already exists in `Cargo.toml`.

### Concrete work

1. **Mode stack adapter**
   - Do not replace `AppMode` everywhere in one large change.
   - Add `mode_stack: Vec<AppMode>` plus `push_mode` / `pop_mode` helpers while keeping `app.mode` as the current-mode compatibility field during migration.
   - Migrate overlays and tool cards one at a time, then remove direct assignments once tests cover the path.

2. **Leader key + keymap layers**
   - Add `leader_timeout_ms` to keybindings config.
   - Support sequences like `<leader> p` → command palette, `<leader> s` → session sidebar, `<leader> d` → diff panel.
   - Keep existing `ctrl+tab`, `ctrl+p`, etc. as aliases.

3. **Rich input box**
   - Add selection state (`selection_start: Option<usize>`) to `InputState`.
   - Support `shift+arrows` to select, `ctrl+c` to copy selection to clipboard, `ctrl+x` to cut, `ctrl+v` to paste.
   - Use the existing `arboard` dependency; gate only platform-specific behavior if tests show CI issues.
   - Support `ctrl+z` / `ctrl+shift+z` undo/redo with a small action history.

4. **Attachment paste**
   - Detect bracketed paste events with file paths or base64 images.
   - Render attachments as virtual tokens in the input line.
   - For now support text files only; images can be a placeholder.

### Files to change

- `src/tui/keybindings.rs` (mode stack, leader, keymap layers)
- `src/tui/app.rs` (mode stack API)
- `src/tui/mod.rs` (use mode stack, move global shortcuts into keymap)
- `src/tui/components/input.rs` (selection, clipboard, undo/redo)
- `Cargo.toml` only if platform-specific clipboard gating is needed

### Validation

```bash
cargo test -q keybindings --lib
cargo test -q input --lib
cargo test -q app --lib
```

### Acceptance criteria

- [ ] Mode stack helpers exist; migrated overlays restore previous mode via `pop_mode`.
- [ ] Leader key sequences are configurable and documented.
- [ ] Input box supports select/copy/cut/paste/undo/redo.
- [ ] Existing shortcuts still work.

---

## Phase 4: Session Lifecycle and Workspace

### Why after tools/input

Fork/child-session navigation and share links need UI chrome (sidebars, dialogs) and stable part storage. Those are ready after Phases 1–3.

### Current state

- `src/session_store/session_ops.rs:92-106` already has `create_child_session`.
- `src/tui/session_manager/mod.rs` supports create/switch/rename/delete/pin.
- No fork/share UI; compact is only `/compact` slash command.
- No workspace concept; working directory is just `std::env::current_dir()`.

### Concrete work

1. **Fork session UI**
   - Add `/fork [title]` slash command that calls `create_child_session` and switches to it.
   - In the session sidebar, show child sessions indented under parent.
   - `d` on a parent session warns "will delete N child sessions".

2. **Manual compact**
   - Add `/compact` UI feedback: a progress toast and a summary of how many messages were compacted.
   - Store compact boundary as a visible "... N messages summarized ..." line in the timeline.

3. **Share / export**
   - Add `/share` slash command that writes a redacted session export to a shareable file path and copies a `file://` link when clipboard is available.
   - Redaction should use the existing session export privacy tier (`SessionExportPrivacy::Redacted`), not config-redaction helpers.
   - Later can upload to a gist bridge; keep local-only for this phase.

4. **Workspace awareness (lightweight)**
   - Add `Workspace` struct: project root, display name, last used session.
   - On startup, detect project root from cwd. If `find_project_root` remains private in `src/instructions/mod.rs`, extract a shared helper instead of reaching into private instruction-loader internals.
   - Session sidebar groups sessions by workspace.
   - Do not build full workspace switcher yet; just tag and group.

### Files to change

- `src/tui/session_manager/mod.rs` (fork, compact feedback, share)
- `src/tui/slash_handler/session/actions.rs` (new slash commands)
- `src/tui/app.rs` (workspace state)
- `src/tui/mod.rs` (sidebar rendering for workspace groups)
- `src/session_store/session_ops.rs` / `src/session_store/export.rs` (ensure share/export metadata)

### Validation

```bash
cargo test -q session_manager --lib
cargo test -q session_store --lib
cargo test -q app --lib
```

### Acceptance criteria

- [ ] `/fork` creates a child session and the sidebar shows hierarchy.
- [ ] `/compact` shows a summary and stores a visible compact boundary.
- [ ] `/share` writes a redacted export file and copies its path.
- [ ] Sessions are grouped by detected workspace in the sidebar.

---

## Phase 5: Plugin UI Slots and KV Preferences

### Why last

This is architecture work that benefits from the prior phases. Plugin slots need the message/tool components to be composable, and KV preferences need a stable session/config foundation.

### Current state

- `src/plugins/mod.rs` is manifest discovery oriented.
- `src/skills/mod.rs` injects context but has no UI hooks.
- Preferences mostly live in `AppConfig`; there is no separate lightweight UI preference store.

### Concrete work

1. **KV preference store**
   - Prefer reusing the existing SQLite/session-store infrastructure where possible; add `src/services/kv.rs` only if a small independent API is still cleaner.
   - API: `get_string`, `set_string`, `get_bool`, `set_bool`.
   - Use it for: theme, tool card default expansion, status-bar density, sidebar width, input history size.
   - Do not immediately migrate stable `AppConfig` settings. Start with volatile UI preferences that are not part of project/runtime config.

2. **Plugin runtime slots (design-only MVP)**
   - Define `TuiSlot` enum: `SidebarTitle`, `SidebarFooter`, `StatusBar`, `MessageBeforeSend`, `ToolCard`.
   - Add `PluginTuiContribution` to `PluginManifest` and `PluginRuntimeFacts`.
   - For this phase, only parse and validate the manifest fields; do not execute plugin UI code.
   - This unblocks future work where plugins can register renderers, without committing to executing third-party UI code in this phase.

3. **Skill-triggered panels**
   - Allow a skill to declare `panel: <name>` in its frontmatter.
   - If declared, `/panel skills` shows the skill's contributed panel content (static markdown for now).
   - This is the safest first UI extension because it uses existing `/panel` infrastructure.

### Files to change

- `src/services/kv.rs` (new)
- `src/services/mod.rs` (export kv)
- `src/plugins/mod.rs` (TUI slot manifest fields)
- `src/plugins/types.rs` or manifest (new fields)
- `src/tui/app.rs` (read KV for UI preferences)
- `src/tui/runtime_panels.rs` (`/panel skills`)

### Validation

```bash
cargo test -q plugins --lib
cargo test -q kv --lib
cargo test -q runtime_panels --lib
```

### Acceptance criteria

- [ ] KV store persists UI preferences across restarts.
- [ ] Plugin manifest can declare TUI slots (validated but not executed).
- [ ] Skills can declare a static panel shown under `/panel skills`.

---

## Cross-Cutting Concerns

### Line-count stewardship

The project enforces a soft 1500-line limit. New TUI work must split into submodules:

- `src/tui/components/tool_renderers/` (per-tool files)
- `src/tui/part_projection.rs`
- `src/services/kv.rs`
- Avoid inflating `src/tui/mod.rs` or `src/tui/app.rs`.

### Tests

Every phase must add or update tests in the relevant module. Prefer `TestBackend` rendering assertions and pure projection tests over full `TuiApp` integration.

### Feature flags

New native dependencies should be behind feature flags. Clipboard work should first use the existing `arboard` dependency and only add a feature gate if CI/platform behavior requires it.

### Provider/runtime boundary

TUI improvements must not change the engine/runtime contract. The engine can keep emitting the same `StreamEvent`s; TUI work should stay on the `SessionProjectionEvent` / sync-store / renderer side unless a runtime contract gap is explicitly found and tested.

---

## Implementation Order

1. Phase 1 — Finish message/part projection adoption
2. Phase 2 — Tool visualization
3. Phase 3 — Keybindings and input overhaul
4. Phase 4 — Session lifecycle and workspace
5. Phase 5 — Plugin UI slots and KV preferences

Run the full gate set after each phase:

```bash
cargo fmt --check
cargo check -q
cargo clippy --all-targets --all-features -- -D warnings
cargo test -q
cargo check --features experimental-api-server -q
```

---

## Summary Table

| Phase | Main value | Touches | Risk |
|-------|-----------|---------|------|
| 1 — Projection adoption | Cleaner foundation for reasoning/tool/session rendering | sync_store, projection_event, message, reasoning, session_parts | Medium (refactor) |
| 2 — Tool visualization | Biggest UX improvement; inline diff, per-tool cards | tool_renderers, diff_renderer, tool_rows | Medium |
| 3 — Keybindings/input | Daily usability; text selection, leader, undo | keybindings, input, app, mod | Medium |
| 4 — Session/workspace | Modern agent workflow; fork/share/compact | session_manager, slash_handler, app | Low-Medium |
| 5 — Plugin/KV | Extensibility and personalization | plugins, skills, kv, panels | Low (design-first) |
