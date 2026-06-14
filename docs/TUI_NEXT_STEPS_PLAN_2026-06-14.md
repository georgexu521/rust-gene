# TUI Next Steps Plan

Date: 2026-06-14
Scope: `src/tui/` and supporting services/plugins
Goal: Close the remaining interaction and extensibility gaps with OpenCode's TUI while keeping `priority-agent`'s runtime-panel and validation strengths.

---

## Background

The previous optimization plan (Phases 1–5) brought the TUI to parity on architecture:

- Message/part projection is canonical.
- Tool runs have per-type inline renderers.
- Input supports selection, clipboard, undo/redo.
- Keybindings use a mode stack and leader key.
- Sessions support fork/share/compact and workspace grouping.
- KV preferences and plugin/skill UI declarations are wired.

This plan focuses on the polish and extension points that are still rough or missing compared to OpenCode:

1. Collapsible long message/tool output.
2. Composer attachment pills and `@` file autocomplete.
3. Leader-key pending state UI feedback.
4. Actually executing plugin UI slots (not just validating manifest fields).
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

1. Add a `CollapsibleBlock` helper in `src/tui/components/collapsible.rs` that takes a list of `Line`s, a max-visible-lines budget, and a toggle callback.
2. Use it inside each tool renderer's expanded body.
3. Add per-`tool_call_id` expansion state keyed by `expanded_inline_tool_id` in `TuiApp`, separate from `expanded_tool_run_id`.
4. For assistant text parts, add a `max_lines` config (default 48) and a `message_part_expanded_id` state; long parts render a "... (N more lines) — press Enter to expand" footer.

### Files

- `src/tui/components/collapsible.rs` (new)
- `src/tui/components/tool_renderers/{bash,file_read,grep}.rs`
- `src/tui/components/message/assistant.rs`
- `src/tui/app.rs` (new expand states)
- `src/tui/mod.rs` (Enter/click to toggle focused collapsible)

### Acceptance

- [ ] Bash/file_read/grep tool bodies can expand/collapse inline.
- [ ] Long assistant text/code blocks can expand/collapse inline.
- [ ] Focused block toggles with `Enter`; `ctrl+t` still opens the full popup.
- [ ] State is keyed per part/tool so toggling one does not expand everything.

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

1. Keep `InputState` as the text buffer, but add a parallel `composer_attachments: Vec<AttachmentToken>` to the composer state in `TuiApp`.
2. Render attachment pills inline after the text in `composer.rs` using bracketed spans (e.g. `[📎 Cargo.toml]`).
3. Add `@` detection in `handle_fallback_key_event`: when the user types `@`, open a file autocomplete overlay (`AppMode::FilePicker` variant or new `AppMode::AttachmentAutocomplete`).
4. Pasted file paths and dropped paths insert an attachment token at the cursor instead of silently populating `composer_attachments`.
5. Backspace on an attachment pill removes it; regular Backspace edits text.

### Files

- `src/tui/components/input.rs` (track attachment tokens separately from text)
- `src/tui/screens/main_screen/composer.rs` (inline pills)
- `src/tui/app.rs` (`AttachmentToken` type, token list)
- `src/tui/mod.rs` (`@` autocomplete, paste attachment tokenization)
- `src/tui/components/file_browser.rs` (reuse for autocomplete list)

### Acceptance

- [ ] Typing `@` opens a fuzzy file autocomplete overlay.
- [ ] Selected files become inline `[📎 path]` pills in the composer.
- [ ] Pasted/dropped file paths become pills and are sent as attachments.
- [ ] Backspace removes a pill when the cursor is immediately after it.
- [ ] Submitted messages still send the union of text and attachments.

### Validation

```bash
cargo test -q input --lib
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

- [ ] Status bar shows a leader indicator while a sequence is pending.
- [ ] Indicator lists the available next keys.
- [ ] Expired leader state clears automatically.

### Validation

```bash
cargo test -q keybindings --lib
cargo test -q app --lib
```

---

## Priority 4: Execute plugin UI slots (not just validate)

### Gap

We parse `tui.slots` in plugin manifests and expose them in `PluginRuntimeFacts`, but we never render anything contributed by a plugin.

### OpenCode reference

- `plugin/slots.tsx` creates a Solid slot registry (`createSolidSlotRegistry`) and exposes `<Slot name="..." mode="replace|single_winner|..." />`.
- `routes/session/sidebar.tsx:48` and `:89` render `<TuiPluginRuntime.Slot name="sidebar_title" ... />` and `sidebar_footer`.
- `routes/session/index.tsx:1268` renders `<TuiPluginRuntime.Slot name="session_prompt" mode="replace" ... />`.
- `plugin/api.tsx:273` exposes `Slot` to plugin code.

### Our current code

- `src/plugins/mod.rs` defines `TuiSlot` enum and `PluginTuiContribution`.
- `src/tui/runtime_panels.rs` is the closest thing to a plugin extension point.

### Proposed change

Keep the scope **safe but real**: do not run arbitrary plugin UI code; instead, add a small plugin contribution registry and let plugins contribute static markdown/routes to known slots.

1. Add `PluginUiContribution` struct in `src/plugins/mod.rs`: `{ slot: TuiSlot, title: String, content: String, route: Option<String> }`.
2. Load a `CONTRIBUTION.md` (or `panel.md`) from the plugin directory next to `plugin.toml`.
3. Store contributions in `TuiApp` after plugin discovery.
4. Render contributions in the matching slots:
   - `SidebarTitle`/`SidebarFooter`: append text to the session sidebar.
   - `StatusBar`: append a segment to the status bar when active.
   - `MessageBeforeSend`: show a confirmation/warning line before submitting a message that matches a plugin pattern.
   - `ToolCard`: for now, only a placeholder; actual tool card injection is future work.
5. Add `/plugins` or `/plugin list` command showing registered contributions.

### Files

- `src/plugins/mod.rs` (contribution loading)
- `src/tui/app.rs` (store contributions)
- `src/tui/screens/main_screen.rs` (sidebar footer slot)
- `src/tui/screens/main_screen/status_bar.rs` (status bar slot)
- `src/tui/screens/main_screen/composer.rs` (message-before-send slot)
- `src/tui/slash_handler/observability.rs` or new `/plugins` handler

### Acceptance

- [ ] A plugin with `tui.slots = ["sidebar_footer"]` and a `panel.md` renders its content in the session sidebar.
- [ ] A plugin with `tui.slots = ["status_bar"]` appends a segment to the status bar.
- [ ] `/plugins` lists discovered plugins and their declared slots.
- [ ] No plugin code is executed; only static markdown is rendered.

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

1. Populate `workspace_root` in `SessionStore::create_session` and `create_child_session`.
2. Add `SessionStore::list_workspaces()` and `list_sessions_by_workspace()`.
3. Add `TuiSessionManager::sessions_by_workspace()`.
4. Add a new overlay `AppMode::WorkspaceSwitcher` bound to `<leader>w`.
5. Render a list of distinct workspace roots; `Enter` switches the current workspace context and sidebar filter.
6. When switching to a session whose stored workspace differs from the current one, show a toast "Switched workspace to ...".

### Files

- `src/session_store/session_ops.rs` (workspace column writes/queries)
- `src/tui/session_manager/mod.rs` (queries)
- `src/tui/app.rs` (`Workspace` state + switch method)
- `src/tui/mod.rs` (workspace switcher keymap + handler)
- `src/tui/screens/main_screen.rs` (render workspace switcher overlay)
- `src/tui/keybindings.rs` (`leader_workspace` binding)

### Acceptance

- [ ] New sessions persist their workspace root.
- [ ] `<leader>w` opens a workspace switcher.
- [ ] Switching workspace filters the sidebar to that workspace (or shows all).
- [ ] Loading a session from another workspace updates the active workspace and shows a toast.

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
7. **Priority 4 (plugin slot execution)** — largest architectural change; do last.

---

## Summary table

| Priority | Feature | Main value | Risk | Touches |
|----------|---------|------------|------|---------|
| 1 | Collapsible output | Reduces clutter; keeps context | Low | tool_renderers, message/assistant, app state |
| 2 | Attachment pills + `@` | Modern composer UX | Medium | input, composer, file_browser, app |
| 3 | Leader feedback | Discoverability | Low | app, status_bar |
| 4 | Plugin slot execution | Extensibility | High | plugins, app, renderers |
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
