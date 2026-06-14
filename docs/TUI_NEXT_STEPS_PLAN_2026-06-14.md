# TUI Next Steps Plan

Date: 2026-06-14
Scope: `src/tui/` and supporting services/plugins
Goal: Close the remaining interaction and extensibility gaps with OpenCode's TUI while keeping `priority-agent`'s runtime-panel and validation strengths.

---

## Background

The previous optimization plan (Phases 1–5) moved the TUI much closer to OpenCode's architecture, but it should not be treated as fully mature parity yet:

- Message/part projection is now the preferred live rendering path.
- Tool runs have per-type inline renderers.
- Input supports selection, clipboard, undo/redo.
- Keybindings use a mode stack and leader key.
- Sessions support fork/share/compact and workspace grouping.
- KV preferences and plugin/skill UI declarations are wired.

The recent reducer and tool-part migration still need normal readiness proof: targeted TUI tests, PTY/readiness matrix runs, and some real sessions. This plan therefore focuses on incremental UX polish and extension points, not another broad rewrite.

The remaining polish and extension gaps are:

1. Collapsible long message/tool output.
2. Composer attachment pills and `@` file autocomplete.
3. Leader-key pending state UI feedback.
4. Rendering static plugin UI slot contributions (not arbitrary plugin UI code).
5. Workspace switcher and session migration.
6. Child-session independent views/tabs.
7. Multi-select file picker.

---

## How to read this plan

Each item follows this structure:

- **Gap**: what users still feel is missing.
- **OpenCode reference**: file/behavior in `~/Downloads/opencode-dev/packages/opencode/src/cli/cmd/tui/`.
- **Our current code**: where the feature lives today.
- **Proposed change**: concrete engineering direction.
- **Files**: expected touch points.
- **Acceptance**: how to know it is done.
- **Validation**: tests/commands to run.

---

## Priority 1: Collapsible long output in messages and tool cards

### Gap

Tool renderers already truncate (`... (N more lines)`), but the user cannot expand the rest without opening the full tool viewer popup (`ctrl+t`). There is also no collapse/expand for very long assistant text or code blocks.

### OpenCode reference

- `feature-plugins/system/session-v2.tsx:206` uses `collapseToolOutput(output(), maxLines, maxChars())`.
- It renders a `BlockTool` with `onClick` toggling `expanded()` and a muted hint: `"Click to expand"` / `"Click to collapse"`.
- `util/collapse-tool-output.ts` (imported at `:32`) computes overflow by line/char budget and returns `{ output, overflow }`.

### Our current code

- `src/tui/components/tool_renderers/{bash,file_read,grep}.rs` hard-cap output at 20–24 lines with a static ellipsis.
- `src/tui/components/message/assistant.rs` renders text parts without any truncation/expansion.
- `src/tui/app.rs` already tracks `expanded_tool_run_id` for the summary row; we can reuse the same idea for inline bodies.

### Proposed change

1. Add a pure `CollapsibleBlock` helper in `src/tui/components/collapsible.rs`.
   - Input: rendered `Line`s plus max line / max char budget.
   - Output: visible lines plus overflow metadata (`hidden_lines`, `hidden_chars`, `is_truncated`).
   - Do **not** put callbacks inside the renderer. Ratatui rendering should stay pure; input handling remains in `TuiApp` / `src/tui/mod.rs`.
2. Use the helper inside each tool renderer's expanded body.
3. Add expansion state in `TuiApp` keyed by canonical ids:
   - `expanded_inline_tool_ids: BTreeSet<String>` keyed by `tool_call_id`.
   - `expanded_message_part_ids: BTreeSet<String>` keyed by `TuiMessagePart.id`.
4. Keep `expanded_tool_run_id` for the summary row focus behavior; do not overload it for inline body truncation.
5. For assistant text parts, add a max-lines budget (default 48) and a footer such as `... N more lines - press Enter to expand`.
6. Add focused-block navigation only if needed for `Enter`; otherwise start with expanding the currently focused tool/message part near the scroll anchor. Mouse click support can remain future work.

### Files

- `src/tui/components/collapsible.rs` (new)
- `src/tui/components/tool_renderers/{bash,file_read,grep}.rs`
- `src/tui/components/message/assistant.rs`
- `src/tui/app.rs` (new expand states)
- `src/tui/screens/main_screen.rs` (pass expansion state into renderers)
- `src/tui/mod.rs` (Enter to toggle the focused/anchored collapsible)

### Acceptance

- [x] Bash/file_read/grep tool bodies can expand/collapse inline.
- [x] Long assistant text/code blocks can expand/collapse inline.
- [x] Focused block toggles with `Enter`; `ctrl+t` still opens the full popup.
- [x] State is keyed per part/tool so toggling one does not expand everything.
- [x] Rendering helpers remain pure and testable without TUI event callbacks.

### Validation

```bash
cargo test -q collapsible --lib
cargo test -q tool_renderers --lib
cargo test -q message --lib
```

---

## Priority 2: Composer attachment pills and `@` file autocomplete

### Gap

Attachments are shown only in the context strip above the input (`files:N`). There are no inline attachment tokens/pills, no `@filename` autocomplete, and pasted file paths go straight to `composer_attachments` without visual feedback in the input.

### OpenCode reference

- `component/prompt/index.tsx:1332` has `pasteAttachment()` that inserts an extmark at the current cursor offset.
- `:194` renders `Locale.truncateMiddle(file, ...)` as an inline pill.
- `component/prompt/autocomplete.tsx` implements `@` / file autocomplete with fuzzy matching and directory expansion.
- `:464` and `:1302` handle clipboard and drag-and-drop attachment intake.

### Our current code

- `src/tui/components/input.rs` is plain text with cursor/selection.
- `src/tui/screens/main_screen/composer.rs` renders attachments in a separate line above the prompt.
- `src/tui/app.rs` has `composer_attachments: Vec<String>` and `/attach` command.
- `src/tui/mod.rs` has `Event::Paste` handling.

### Proposed change

1. Keep `InputState` as a plain text editor. It should continue to own cursor movement, selection, clipboard, undo/redo, and text mutation only.
2. Introduce a small composer-level model:
   - `AttachmentToken { id, path, label, source }`
   - `composer_attachment_tokens: Vec<AttachmentToken>` in `TuiApp`
   - compatibility helpers that still expose the current string paths to submission code.
3. Render attachment pills inline in `composer.rs` using bracketed spans (for example `[file Cargo.toml]`). Avoid storing those visual pills in the text buffer.
4. Add `@` detection in `handle_fallback_key_event`: when the user types `@`, open a file autocomplete overlay. Prefer extending the existing file picker with an `AttachmentAutocomplete` mode over adding file matching logic to `InputState`.
5. Pasted file paths and dropped paths create attachment tokens with visible feedback in the composer.
6. Backspace near an attachment pill removes the token at the composer layer; regular Backspace edits text.

### Files

- `src/tui/components/attachment_token.rs` (new pure token helpers)
- `src/tui/components/input.rs` (only if cursor/selection hooks are needed; do not store tokens here)
- `src/tui/screens/main_screen/composer.rs` (inline pills)
- `src/tui/app.rs` (`AttachmentToken` list and compatibility helpers)
- `src/tui/mod.rs` (`@` autocomplete, paste attachment tokenization)
- `src/tui/components/file_browser.rs` (reuse for autocomplete list)

### Acceptance

- [x] Typing `@` opens a fuzzy file autocomplete overlay.
- [x] Selected files become inline `[file path]` pills in the composer.
- [x] Pasted/dropped file paths become pills and are sent as attachments.
- [x] Backspace removes a pill when the cursor is immediately after it.
- [x] Submitted messages still send the union of text and attachments.

### Validation

```bash
cargo test -q input --lib
cargo test -q attachment_token --lib
cargo test -q app --lib
```

---

## Priority 3: Leader-key pending state UI feedback

### Gap

Pressing the leader key (`\` by default) enters a sequence, but the status bar gives no visual indication. If the user pauses, the sequence expires silently.

### OpenCode reference

- `config/keybind.ts:48` defines `leader` as `ctrl+x` and many commands use `<leader>q` etc.
- `component/prompt/index.tsx:141` uses `const leader = useLeaderActive()`.
- The prompt component likely renders a leader hint (search `useLeaderActive` for consumers).

### Our current code

- `src/tui/app.rs` has `leader_state: Option<LeaderState>` and `begin_leader_sequence()`.
- `src/tui/mod.rs:451` dispatches leader sequences in `handle_leader_sequence()`.
- `src/tui/screens/main_screen/status_bar.rs` renders the bottom status line.

### Proposed change

1. Add `leader_active()` helper on `TuiApp`.
2. In `status_bar.rs`, when `leader_state` is present and not expired, render a "LEADER" mode indicator with the timeout remaining (e.g. `LEADER — p palette, s session sidebar, d diff`).
3. When leader expires between ticks, auto-clear `leader_state` so the UI does not get stuck.

### Files

- `src/tui/app.rs` (helper + tick-based expiry)
- `src/tui/screens/main_screen/status_bar.rs`
- `src/tui/mod.rs` (tick cleanup)

### Acceptance

- [x] Status bar shows a leader indicator while a sequence is pending.
- [x] Indicator lists the available next keys.
- [x] Expired leader state clears automatically.

### Validation

```bash
cargo test -q keybindings --lib
cargo test -q app --lib
```

---

## Priority 4: Render static plugin UI slot contributions

### Gap

We parse `tui.slots` in plugin manifests and expose them in `PluginRuntimeFacts`, but we never render anything contributed by a plugin. The first implementation should prove the slot path without executing arbitrary plugin UI code or adding a full frontend plugin runtime.

### OpenCode reference

- `plugin/slots.tsx` creates a Solid slot registry (`createSolidSlotRegistry`) and exposes `<Slot name="..." mode="replace|single_winner|..." />`.
- `routes/session/sidebar.tsx:48` and `:89` render `<TuiPluginRuntime.Slot name="sidebar_title" ... />` and `sidebar_footer`.
- `routes/session/index.tsx:1268` renders `<TuiPluginRuntime.Slot name="session_prompt" mode="replace" ... />`.
- `plugin/api.tsx:273` exposes `Slot` to plugin code.

### Our current code

- `src/plugins/mod.rs` defines `TuiSlot` enum and `PluginTuiContribution`.
- `src/tui/runtime_panels.rs` is the closest thing to a plugin extension point.

### Proposed change

Keep the scope **safe but real**: do not run arbitrary plugin UI code. Add a small static contribution registry and support only low-risk display slots first.

1. Keep the existing `PluginTuiContribution` manifest type for declared slots. Add a separate runtime content type, for example `PluginUiSlotContent { plugin_id, slot, title, content }`, to avoid overloading the manifest struct.
2. Load a small static `panel.md` from the plugin directory next to `plugin.toml`.
   - Frontmatter may specify `slot = "sidebar_footer"` or `slot = "status_bar"`.
   - If no frontmatter exists, only render it when exactly one declared slot is safe and supported.
3. Store contributions in `TuiApp` after plugin discovery.
4. First supported slots:
   - `SidebarFooter`: append concise static text to the session sidebar.
   - `StatusBar`: append one short status segment.
5. Explicitly defer:
   - `MessageBeforeSend`: requires a safe pre-submit policy boundary.
   - `ToolCard`: requires tool-card renderer injection and conflict rules.
   - `SidebarTitle` replacement: avoid changing core navigation labels until slot ordering is tested.
6. Add `/plugins` or `/plugin list` command showing discovered plugins, declared slots, and active static slot content.

### Files

- `src/plugins/mod.rs` (contribution loading)
- `src/tui/app.rs` (store contributions)
- `src/tui/screens/main_screen.rs` (sidebar footer slot)
- `src/tui/screens/main_screen/status_bar.rs` (status bar slot)
- `src/tui/slash_handler/observability.rs` or new `/plugins` handler

### Acceptance

- [ ] A plugin with `tui.slots = ["sidebar_footer"]` and a `panel.md` renders its content in the session sidebar.
- [ ] A plugin with `tui.slots = ["status_bar"]` appends a segment to the status bar.
- [ ] `/plugins` lists discovered plugins and their declared slots.
- [ ] No plugin code is executed; only static markdown is rendered.
- [ ] Unsupported slots are reported as declared-but-not-rendered, not silently ignored.

### Validation

```bash
cargo test -q plugins --lib
cargo test -q runtime_panels --lib
cargo test -q app --lib
```

---

## Priority 5: Workspace switcher and session migration

### Gap

We detect the workspace and group sessions by it, but there is no UI to switch workspaces, create a new workspace, or migrate a session when the workspace root is no longer available.

### OpenCode reference

- `component/dialog-workspace-list.tsx`: list workspaces with expand/collapse details.
- `component/dialog-workspace-unavailable.tsx`: prompt to restore a session into a new workspace.
- `routes/session/sidebar.tsx:18` shows workspace label and status.
- `routes/session/index.tsx:268` auto-switches workspace when loading a session from a different workspace.

### Our current code

- `src/workspace.rs` detects project root.
- `src/tui/app.rs` stores `workspace: Workspace`.
- `src/tui/session_manager/mod.rs` tags sessions with workspace roots in memory.
- `src/migrations/v20_add_session_workspace.rs` added `workspace_root` column but we do not populate or query it yet.

### Proposed change

Split this into two slices so the UI does not sit on top of non-durable state.

**Slice A: durable workspace metadata**

1. Populate `workspace_root` in session creation and child-session creation.
2. Backfill `workspace_root` for existing sessions when possible:
   - current workspace for sessions without a stored root;
   - preserve `NULL`/unknown only when no local root can be inferred.
3. Add `SessionStore::list_workspaces()` and `list_sessions_by_workspace()`.
4. Keep `TuiSessionManager`'s in-memory tags only as a compatibility cache; durable store data should win.

**Slice B: workspace switcher UI**

5. Add `TuiSessionManager::sessions_by_workspace()` backed by store queries.
6. Add a new overlay `AppMode::WorkspaceSwitcher` bound to `<leader>w`.
7. Render a list of distinct workspace roots; `Enter` switches the current workspace context and sidebar filter.
8. When switching to a session whose stored workspace differs from the current one, show a toast `Switched workspace to ...`.

### Files

- `src/session_store/` session creation/query code (workspace column writes/queries)
- `src/tui/session_manager/mod.rs` (queries)
- `src/tui/app.rs` (`Workspace` state + switch method)
- `src/tui/mod.rs` (workspace switcher keymap + handler)
- `src/tui/screens/main_screen.rs` (render workspace switcher overlay)
- `src/tui/keybindings.rs` (`leader_workspace` binding)

### Acceptance

- [ ] New sessions persist their workspace root.
- [ ] Existing sessions without durable workspace metadata get a deterministic fallback/backfill path.
- [ ] `<leader>w` opens a workspace switcher.
- [ ] Switching workspace filters the sidebar to that workspace (or shows all).
- [ ] Loading a session from another workspace updates the active workspace and shows a toast.
- [ ] Restarting the TUI preserves workspace grouping.

### Validation

```bash
cargo test -q session_store --lib
cargo test -q session_manager --lib
cargo test -q app --lib
```

---

## Priority 6: Child-session independent views/tabs

### Gap

`/fork` creates a child session and switches to it, but there is no way to keep the parent visible or compare timelines side-by-side.

### OpenCode reference

- OpenCode models sessions as routes (`routes/session/index.tsx`) and the sidebar lets the user switch between them quickly.
- There is no literal split-screen, but fast session switching + retained scroll state gives a "tab" feel.

### Our current code

- `src/tui/session_manager/mod.rs::fork_current_session` copies messages and switches.
- Sidebar navigation restores a session.
- `scroll_anchor_id` and `pinned_to_bottom` are per-app, not per-session.

### Proposed change

1. Persist per-session UI state (scroll offset, scroll anchor, pinned-to-bottom, expanded tool ids) in memory keyed by session id.
2. When restoring a session via sidebar `Enter`, restore its scroll state.
3. Add a keybinding (`<leader>g` like OpenCode's session timeline, or `H`/`L`) to cycle recent sessions without opening the sidebar.
4. Add `/back` slash command to return to the previous session.

### Files

- `src/tui/app.rs` (`SessionUiState` map)
- `src/tui/session_manager/mod.rs` (recent session stack)
- `src/tui/mod.rs` (cycle keybindings)
- `src/tui/slash_handler/session.rs` (`/back`)

### Acceptance

- [ ] Restoring a session restores its previous scroll position.
- [ ] Recent session cycling works without losing place.
- [ ] `/back` returns to the previously active session.

### Validation

```bash
cargo test -q app --lib
cargo test -q session_manager --lib
```

---

## Priority 7: Multi-select file picker

### Gap

The file picker (`/attach`, `AppMode::FilePicker`) only selects one file at a time. OpenCode supports multi-select and range selection.

### OpenCode reference

- `component/prompt/autocomplete.tsx` supports file autocomplete with multi-selection via overlay.

### Our current code

- `src/tui/components/file_browser.rs` defines `FileBrowserState`.
- `src/tui/app/actions.rs::open_composer_file_picker` opens single-select mode.
- `src/tui/mod.rs::handle_file_picker_key_event` handles selection.

### Proposed change

1. Extend `FileBrowserState` with `selected_paths: HashSet<PathBuf>` and `selection_mode: Single | Multi`.
2. Add `Space` to toggle selection in multi-mode; `Enter` confirms all selected.
3. Add `/attach multi` command and bind `<leader>a` to multi-select file picker.
4. Render selected count in the picker title.

### Files

- `src/tui/components/file_browser.rs`
- `src/tui/app/actions.rs`
- `src/tui/mod.rs`
- `src/tui/slash_handler/session.rs` (`/attach` args)

### Acceptance

- [ ] Multi-select file picker adds several files at once.
- [ ] Single-select mode remains default for `/attach`.
- [ ] Selected files render as inline pills in the composer.

### Validation

```bash
cargo test -q app --lib
cargo test -q file_browser --lib
```

---

## Cross-cutting concerns

### Line-count stewardship

Continue splitting large files:

- `src/tui/components/collapsible.rs` (< 300 lines)
- `src/tui/components/attachment_token.rs` (new, < 300 lines)
- `src/tui/screens/main_screen/workspace_switcher.rs` (new, < 400 lines)
- Avoid inflating `src/tui/mod.rs`; move new overlay handlers to `src/tui/keymap_handlers/` if they grow.

### Tests

For each item add:

- Unit tests for pure helpers (`collapsible`, `attachment_token`, workspace queries).
- `TestBackend` rendering assertions for overlays and composer pills.
- One integration test in `tui::app::tests` verifying the end-to-end keymap path.

### Readiness proof

After the reducer/tool-part migration and after any priority that touches rendering or session switching, run the focused readiness path before marking it done:

```bash
bash scripts/tui_tool_turn_spine_readiness.sh
```

If the full script is too expensive for a small slice, run the relevant narrow PTY/readiness smoke and record the skipped scope in the commit or plan update.

### Feature flags

No new native dependencies are expected. If plugin slot rendering pulls in a markdown parser, gate it behind `experimental-api-server` or a new `plugin-ui` feature.

### Provider/runtime boundary

These changes stay on the TUI side. The engine/runtime contract remains unchanged.

---

## Recommended order

1. **Priority 1 (collapsible output)** — highest daily value; low risk.
2. **Priority 3 (leader feedback)** — quick win; pairs with mode stack.
3. **Priority 2 (attachment pills + `@`)** — big UX improvement; moderate size.
4. **Priority 7 (multi-select picker)** — natural follow-up to attachment pills.
5. **Priority 5 (workspace switcher)** — builds on existing workspace tag.
6. **Priority 6 (child-session tabs)** — depends on workspace switcher being stable.
7. **Priority 4 (static plugin slot rendering)** — largest architectural change; do last.

---

## Summary table

| Priority | Feature | Main value | Risk | Touches |
|----------|---------|------------|------|---------|
| 1 | Collapsible output | Reduces clutter; keeps context | Low | tool_renderers, message/assistant, app state |
| 2 | Attachment pills + `@` | Modern composer UX | Medium | input, composer, file_browser, app |
| 3 | Leader feedback | Discoverability | Low | app, status_bar |
| 4 | Static plugin slot rendering | Extensibility | High | plugins, app, renderers |
| 5 | Workspace switcher | Multi-project workflow | Medium | session_store, session_manager, keybindings |
| 6 | Child-session tabs | Easier fork navigation | Medium | session_manager, app state, keybindings |
| 7 | Multi-select picker | Batch attachments | Low | file_browser, actions |

---

## Validation commands (run after each priority)

```bash
cargo fmt --check
cargo check -q
cargo clippy --all-targets --all-features -- -D warnings
cargo test -q
cargo check --features experimental-api-server -q
```
