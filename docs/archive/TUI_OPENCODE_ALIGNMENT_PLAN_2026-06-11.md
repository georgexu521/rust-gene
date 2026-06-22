# TUI opencode Alignment Plan
Date: 2026-06-11
Status: In progress

## Implementation Progress

As of 2026-06-11, the first implementation slice is in place:

## Tool Turn Spine Pivot

As of 2026-06-12, the next implementation focus is no longer new sidebar,
composer, or attachment features. The remaining product gap is the tool-turn
spine:

```text
requested -> accepted -> executing -> result_observed -> sent_back_to_model
          -> final_answer -> persisted
```

The first spine slice is now in code:

- `RuntimeStateSnapshot.tool_turns` exposes frontend-neutral
  `ToolTurnSnapshot` records.
- `ToolTurnPhase` is the shared contract for requested, accepted, executing,
  result-observed, sent-back-to-model, final-answer, persisted, failed,
  cancelled, and timed-out states.
- `RuntimeFacadeState::process_stream_event` maps existing `StreamEvent`
  values into the spine without removing the legacy `ToolRunView` path.
- TUI stream consumption calls `process_stream_event_with_parent` for the same
  event stream it already renders, attaching the current user message id as the
  tool turn's parent anchor.
- TUI marks tool turns as persisted after the assistant message persistence
  path completes.
- `active_turn_status` can read the spine when no legacy active tool row is
  available, so the UI can report states such as `sent tool result to model`
  instead of guessing from provider wait text.
- `tool_rows_for_runs_with_spine` now overlays `ToolTurnSnapshot` state onto
  legacy `ToolRunView` rows by tool id. The old run model still owns grouping,
  expansion, and detailed output rendering, but row icon, severity, status
  label, failure preview, and result preview now come from the spine whenever a
  matching snapshot exists.
- Timeline height estimation and main-screen tool-group rendering now call the
  spine-aware projection, so stale legacy `waiting` / `running` display state is
  no longer authoritative once the facade has observed a newer tool phase.
- `ToolTurnSnapshot.parent_message_id` is now populated for TUI live turns,
  which gives the next transcript slice a deterministic grouping key instead of
  inferring ownership from transient legacy maps.
- `scripts/tui_pty_smoke.py` now reports spine-specific evidence fields:
  `saw_tool_result_observed`, `saw_tool_sent_back`, and
  `saw_tool_final_answer`.
- `StreamEvent::ToolResultsReadyForModel` now explicitly records that tool
  results have been appended to the next model context. Eval/event-log mode
  records this as `tool_results_ready_for_model`, and runtime frontends can
  map it to `ToolTurnPhase::SentBackToModel` without waiting for the next
  provider-start diagnostic.
- Tool outcome learning persistence now runs best-effort in the background
  from the tool execution controller, so learning/context-ledger writes cannot
  block the critical path from tool completion to model continuation.
- TUI now has a post-tool stall watchdog: if a turn remains in
  `result_observed` or `sent_back_to_model` beyond the configured LLM timeout
  after the provider request has completed, the run is aborted with an explicit
  timeout error and active tool turns are marked `timed_out`.
- 2026-06-12 PTY smoke evidence before the post-tool watchdog:
  `python3 scripts/tui_pty_smoke.py --prompt tool-pwd --size 120x35 --timeout 90 --settle 6 --out-dir target/tui-pty-smoke-tool-spine`
  reached visible `result observed` state and did not leak
  `async_openai::error` or `failed deserialization of`, but it did not reach a
  visible shell row, final answer, or persisted state before the harness cleaned
  up the still-running process. Treat this as partial spine evidence, not a
  completed tool-turn proof.
- 2026-06-12 eval/event-log evidence:
  `PRIORITY_AGENT_LLM_REQUEST_TIMEOUT_SECS=45 target/debug/priority-agent --eval-run --prompt-file target/tui-debug/tool-pwd-prompt.txt --events target/tui-debug/tool-pwd-events.jsonl --output target/tui-debug/tool-pwd-output.json`
  recorded `tool_execution_complete`,
  `tool_results_ready_for_model`, a second provider request, final text, and
  `complete`. This proves the backend stream contract can close the successful
  tool-turn loop when the provider returns normally.
- 2026-06-12 TUI timeout evidence:
  `PRIORITY_AGENT_LLM_REQUEST_TIMEOUT_SECS=30 python3 scripts/tui_pty_smoke.py --prompt tool-pwd --size 120x35 --timeout 55 --settle 5 --out-dir target/tui-pty-smoke-tool-spine-timeout`
  reached `result observed`, did not leak raw provider/deserialization errors,
  did not rely on harness cancellation, and exited with a visible
  `tool turn stalled after result observation for 30.0s` error. This proves the
  failed/timeout terminal path is now accurate for a stalled TUI provider turn.

Open items for the next slice:

- move transcript grouping to `ToolTurnSnapshot.parent_message_id`, then use
  `ToolRunView` only as optional presentation detail;
- make the TUI tool transcript show the shell/result row reliably for
  DeepSeek/openai-compatible tool runs, not only the bottom active status;
- prove a real TUI provider tool turn reaches `[Shell]` / result observation /
  `sent_back_to_model` / final answer / persisted in PTY when the provider
  returns normally;
- make timeout/cancel terminal states abort the background provider task
  cleanly, not merely render a visible error.

- Phase 0 is implemented in code:
  - `src/tui/view_model/activity.rs` now selects one active turn status;
  - `src/tui/screens/main_screen/composer.rs` keeps the composer visible while
    a turn is running and renders the single active status row there;
  - `src/tui/screens/main_screen/status_bar.rs` no longer repeats transient
    provider/tool wait state in the default footer;
  - `src/tui/components/message.rs` removes the heavy user-message slab and
    hides user timestamps by default.
- Phase 1 has its first tool-row foundation:
  - `src/tui/view_model/tool_rows.rs` maps `ToolRunView` into typed row DTOs;
  - routine successful read/search tools can be hidden behind a compact count;
  - shell, file mutation, permission, background, cancelled, timed-out, and
    failed tools remain visible;
  - long output preview is bounded before rendering.
- Phase 2 has its first timeline projection:
  - `src/tui/view_model/timeline.rs` projects messages and tool groups into
    stable timeline items;
  - main-screen rendering consumes that projection instead of rebuilding the
    message/tool sequence inline.
- Phase 2 now owns transcript height estimates:
  - message and tool-group height estimation moved into
    `src/tui/view_model/timeline.rs`;
  - tool row line generation/height now lives in
    `src/tui/view_model/tool_rows.rs`;
  - bottom anchoring, `/jump`, and Vim-normal collapse now count timeline items
    instead of assuming message indices;
  - Tab collapse maps a tool-group anchor back to the nearest parent message.
- Phase 2 now has stable-id scroll anchors for manual navigation:
  - `TuiApp::scroll_anchor_id` records the stable timeline item id when the
    user scrolls, jumps, or search-jumps;
  - rendering resolves that id against the current timeline before falling back
    to the numeric offset;
  - message search maps message indices through the timeline projection instead
    of writing raw indices directly;
  - bottom anchoring clears the manual anchor and stays pinned to the newest
    timeline item.
- Phase 2 also has the first footer projection:
  - `src/tui/view_model/footer.rs` selects default/debug footer items;
  - `src/tui/screens/main_screen/status_bar.rs` now only maps footer tones to
    theme colors and renders separators.
- Phase 4 has a minimal composer context strip:
  - mode, provider/model, memory mode, and paste-block count are visible in the
    composer without replacing the prompt.
- Phase 4 now exposes prompt reuse affordances:
  - `/prompt-history` shows recently submitted prompts;
  - `/prompt-stash` saves/restores/clears the current composer draft;
  - `Ctrl+R` opens a prompt history/stash picker and copies the selected item
    back into the composer without consuming the stash;
  - `/paste [n]` previews a collapsed paste block in the existing viewer modal;
  - command palette context boosts prompt history/stash when relevant;
  - composer context strip shows `hist:n`, `stash`, and long-paste line/char
    summaries.
- Phase 4 now has a first file/context attachment workflow:
  - `/attach <path>` adds an existing file or directory path to the next
    composer prompt;
  - `/attach list`, `/attach remove <n>`, and `/attach clear` manage the
    attachment set;
  - `/attach browse [root]` opens a file-picker modal backed by the existing
    file-browser component;
  - the file picker supports Up/Down or `j`/`k` navigation, Enter to expand
    directories or attach files, Space to toggle directories, and Esc/q to
    close;
  - `/` enters file-picker filtering, typed characters live-filter files,
    Backspace edits the filter, and Enter/Esc leaves filter mode;
  - composer and Context sidebar strips show attached file counts and numbered
    attached path summaries;
  - `/attach preview [n]` opens the selected attachment in the existing viewer:
    text files show bounded content, directories show a deterministic listing,
    and missing paths show an explicit unavailable message;
  - ordinary message submission injects an `Attached context:` block before the
    user request, then clears the one-shot attachment set.
- Phase 4 now has richer attachment metadata in the composer:
  - composer strip, Context sidebar, `/attach list`, and the injected
    `Attached context:` payload all use the same attachment summary;
  - summaries include stable indices, file/dir/missing state, file size, or
    directory item count;
  - composer strip now surfaces Backspace removal, and `/attach list` shows
    preview/remove commands next to each attachment.
- Phase 4 now has keyboard-driven inline attachment removal:
  - when the prompt input is empty, Backspace removes the last composer
    attachment and shows a short toast;
  - Backspace still edits text normally when the prompt contains content.
- Phase 4 now has a dedicated composer attachment row:
  - the main context strip keeps only a compact file count;
  - attached file/directory summaries render on their own row with bounded
    previews, overflow count, `/attach preview`, and Backspace removal hints;
  - this keeps provider/model/history metadata readable when paths are long or
    multiple files are attached.
- Phase 4 now keeps long attachment rows actionable on narrow terminals:
  - attachment summaries are truncated by terminal display width before
    rendering;
  - `/attach preview` remains visible even when a long path must be shortened;
  - wide-character paths are handled without hard terminal clipping.
- Phase 4 now keeps the composer context strip width-aware:
  - long provider/model labels are truncated by terminal display width before
    they can crowd out composer state;
  - `hist:N`, `stash`, `files:N`, and `paste:N` stay visible on narrow
    terminals when possible;
  - narrow context strips use compact paste counts instead of long paste
    summaries.
- Phase 4 now removes default-mode duplication from the composer strip:
  - default `auto` mode is hidden in the composer because the footer already
    renders it as `● auto`;
  - non-default modes such as `plan` still appear in the composer strip where
    they can affect prompt intent;
  - deterministic snapshots assert the default strip starts with provider/model
    context instead of `auto · ...`.
- Phase 5 has the first sidebar safety rule:
  - terminals below 140 columns render the sidebar as an overlay;
  - overlay sidebar leaves the composer/footer visible;
  - overlay sidebar clears the transcript backdrop behind the panel so narrow
    terminals do not show orphaned message fragments beside the sidebar;
  - wide terminals keep the inline sidebar with a fixed 40-column session panel
    so model names and selected-session previews stay readable;
  - 100-column and 120-column terminals no longer compress the main timeline
    when the sidebar is visible.
- Phase 5 has initial timeline jump navigation:
  - `/jump user` jumps to the latest user prompt;
  - `/jump failed` jumps to the latest failed/timed-out/cancelled tool group;
  - `/jump edit` jumps to the latest file mutation group;
  - `/jump latest` returns to the bottom.
- Phase 5 has a more useful Context sidebar:
  - the Context panel now shows session id, message/timeline counts, active
    runtime state, latest token usage, permission mode, composer history/stash
    and paste state, memory mode, tool totals, failures, and current expansion;
  - this replaces the prior placeholder text-only panel.
- Phase 5 now has a more scannable Sessions sidebar:
  - session rows render as a title line plus a metadata line instead of a
    cramped single row;
  - metadata includes short session id, compact model label, and full message
    count within the fixed 40-column inline sidebar width;
  - selected/current/pinned sessions have distinct markers, and delete-confirm
    state is visible inline;
  - session filtering now matches title, short/full id, and model name.
- Phase 5 now has selected-session preview snippets:
  - the selected Sessions row shows the latest user/assistant preview line when
    message history is available;
  - previews reuse the existing session-manager `recent_preview_lines`
    projection and are bounded to the actual sidebar width before rendering,
    keeping unselected rows compact and avoiding hard terminal clipping.
  - preview/title truncation uses terminal display width, so wide characters
    such as Chinese text cannot overflow the fixed-width sidebar.
- Phase 5 now persists pinned sessions:
  - `AppConfig.ui.pinned_sessions` stores pinned session ids across TUI
    restarts;
  - sidebar pin/unpin updates the in-memory order immediately and attempts to
    save the updated config without blocking the current session if saving
    fails.
- Phase 5 shortcut help now exposes sidebar/reasoning affordances:
  - `Ctrl+O` is documented as reasoning expansion first, then tool details;
  - Sessions/Context panel switching, session filtering, session movement,
    session switching, pinning, deleting, and renaming are listed in a dedicated
    Sidebar section.
- Phase 3 has its first message-component split:
  - user, assistant, text body, reasoning summary, notice, and tool renderers
    now live under
    `src/tui/components/message/`;
  - the public `message.rs` entry keeps role routing, card-kind detection,
    compact rendering, and helper exports.
- Phase 3 now has a first assistant reasoning projection:
  - `src/tui/view_model/reasoning.rs` folds provider `<think>...</think>`
    leakage into a muted one-line `Thinking hidden`/`Thinking...` summary;
  - assistant rendering shows only visible answer text by default;
  - timeline height estimation uses the same folded assistant content, so long
    reasoning blocks no longer distort scroll windows.
- Phase 3 now has user-controlled reasoning expansion:
  - `src/tui/view_model/reasoning.rs` preserves bounded hidden reasoning text
    separately from the visible answer;
  - `Ctrl+O` expands/collapses reasoning for the assistant message at the
    current timeline anchor before falling back to tool-detail cycling;
  - `src/tui/components/message/reasoning.rs` renders expanded reasoning on
    demand only, with a bounded preview and overflow count;
  - timeline height estimation accounts for the expanded reasoning body, so
    scroll placement stays consistent with what is rendered.
- Phase 3 now has first completed-assistant metadata:
  - completed assistant messages receive UI metadata for model, completion
    tokens, total tokens, reasoning tokens, and cached prompt tokens when the
    stream finishes;
  - assistant headers render that metadata after streaming ends instead of
    showing token/model details only during the active stream;
  - this keeps the timeline useful after a turn completes without changing the
    persistent session storage contract yet.
- Phase 3 now has richer in-memory assistant completion metadata:
  - completed assistant messages also receive turn elapsed time, provider
    terminal phase, tool count, failed-tool count, and validation-tool
    pass/fail status when that evidence exists in the current TUI run;
  - assistant headers render these fields compactly as historical metadata
    instead of requiring the user to keep watching the live footer;
  - `messages.metadata` now persists those string fields through session
    storage, and session reload restores them into `MessageItem.metadata`.
- Phase 3 now has first typed message parts:
  - `src/tui/components/message/text.rs` owns markdown body conversion into
    owned ratatui lines;
  - `src/tui/components/message/reasoning.rs` owns collapsed reasoning summary
    rendering;
  - `src/tui/components/message/notice.rs` owns system/error/warning notice
    rendering;
  - `src/tui/components/message/system_tool.rs` is now limited to tool cards.
- Phase 6 has a first raw-log regression guard:
  - TUI startup keeps the default tracing level at `off` and routes terminal
    logs to `io::sink`, preventing provider/library tracing errors from
    painting over the alternate-screen UI;
  - `src/main.rs` now has a focused test that locks this behavior for TUI
    while preserving CLI/API logging defaults.
- Phase 6 now has deterministic full-screen render coverage:
  - simulated turn state with user/assistant messages, hidden reasoning,
    active tool state, sidebar, selected-session preview, and composer
    attachment renders at 100x30, 120x35, and 160x45;
  - the test asserts one active label, no duplicate provider wait while a
    concrete tool is active, no legacy `Thinking...` placeholder, folded
    reasoning, visible attachment affordances, and visible footer shortcuts.
- Phase 6 now has a repeatable snapshot artifact path:
  - `PRIORITY_AGENT_TUI_SNAPSHOT_DIR=target/tui-snapshots cargo test -q opencode_alignment_snapshots_can_be_dumped_for_visual_review --lib`
    writes 100x30, 120x35, and 160x45 rendered-screen text snapshots;
  - the same test asserts no raw provider logs, no duplicate active wait state,
    no `Thinking...` placeholder, bounded viewport width, visible attachment
    affordances, no narrow-sidebar overlay bleed, and readable session metadata;
  - the active tool fixture uses a fixed runtime elapsed value so generated
    snapshots stay stable across machines and repeated runs;
  - snapshot artifact writing normalizes volatile short session ids to
    `sess_demo`, keeping diffs focused on actual UI changes;
  - `PRIORITY_AGENT_TUI_SNAPSHOT_DIR=target/tui-snapshots cargo test -q completed_tool_turn_snapshots_can_be_dumped_for_visual_review --lib`
    writes a successful tool-turn snapshot at the same viewport sizes, covering
    user prompt, completed shell row, command, command result, assistant
    metadata, validation status, and final answer text;
  - the snapshots are generated under `target/` and are for manual visual
    review, not committed artifacts.
- Phase 6 also has provider/model label fallback polish:
  - composer and footer labels now fall back to the runtime facade provider and
    model when no `StreamingQueryEngine` is attached;
  - openai-compatible DeepSeek turns render as `DeepSeek / deepseek-v4-flash`
    instead of leaking the protocol-family label `openai_compatible` into the
    daily TUI surface;
  - the chat/session intro uses the same display provider/model label, so the
    first transcript line no longer regresses to bare model text followed by a
    duplicated default mode;
  - default `auto` mode is hidden from the chat/session intro, while non-default
    modes remain visible as deliberate workflow state;
  - provider-only active rows use the same display label, so waiting states
    read as `waiting on DeepSeek`;
  - permission mode falls back to the configured default (`auto`) instead of
    `unknown` in the same degraded render path;
  - together these avoid `unknown` status noise in deterministic snapshots and
    degraded render states, while keeping protocol-family detail in the
    runtime facade;
  - this is locked by a focused `status_tools` unit test and the completed
    tool-turn snapshot assertions.
- Phase 6 now has first semantic-color regression coverage:
  - completed shell/tool rows render with the success tone instead of the muted
    metadata tone, making finished work visually distinct from passive status
    text;
  - failed, timed-out, cancelled, and permission-blocked tool rows keep their
    existing warning/error tones;
  - a buffer-level style test now locks the completed tool row, assistant reply
    header, and provider footer labels to their expected semantic colors.
- Phase 6 now has footer spacing polish:
  - status bar rendering reserves a leading cell before the first item;
  - this prevents the inline sidebar border and footer mode glyph from visually
    merging on wide terminals;
  - a focused render test locks the left padding behavior.
- Phase 6 now has narrow-footer compaction:
  - status bar rendering now uses the actual terminal width before choosing
    which footer items to show;
  - low-priority debug and usage items drop first, while mode, provider/model,
    non-default permission, and `? shortcuts` stay visible when possible;
  - long provider/model labels truncate by terminal display width with an
    ellipsis instead of relying on hard terminal clipping;
  - extremely narrow terminals now have an explicit fallback that can drop
    `? shortcuts` and then provider/model before allowing footer overflow.
- Phase 6 now keeps package version out of the default footer:
  - `v*` package version is moved behind Debug footer density instead of
    occupying the daily session footer;
  - default snapshots assert version text does not return to the normal
    footer;
  - Debug footer still exposes the version for diagnostics.
- Phase 6 now removes default-permission footer duplication:
  - default `auto` permission is hidden from the footer because agent mode
    already renders as `● auto`;
  - non-default permission labels remain available as warning-tone durable
    environment state;
  - full-screen snapshots now assert the footer does not regress to
    `sess_demo · auto · ...` duplication;
  - full-screen snapshots also assert the chat intro does not regress to
    `deepseek-v4-flash · auto` duplication.
- Phase 6 now validates full-screen snapshots by terminal display width:
  - snapshot tests now measure rendered line width with `unicode_width` instead
    of raw character count;
  - the same display-width coordinate mapping is used when locating styled
    cells in rendered buffers;
  - a CJK regression test proves double-width text cannot slip through the
    viewport-width assertion.
- Phase 6 now reconstructs snapshot text with terminal cell width awareness:
  - generated snapshot lines skip wide-character placeholder cells instead of
    treating them as real spaces;
  - provider-failure snapshots now preserve Chinese text such as `你好` without
    rendering misleading `你 好` artifacts;
  - styled-cell lookup uses the same reconstructed text path, keeping snapshot
    assertions and visual review aligned.
- Phase 6 now has deterministic provider-failure screen coverage:
  - assistant messages that clearly contain provider/runtime errors render with
    an `Error` header and error tone instead of a green `Reply` header;
  - the visible error body strips internal `[Error: ...]` wrappers so the
    transcript reads like a product error state rather than a leaked runtime
    string;
  - a provider-failure fixture models the observed DeepSeek failure path at
    100x30, 120x35, and 160x45;
  - the snapshot test asserts the screen stays product-shaped: no raw
    `async_openai::error`, no `failed deserialization of`, no stale
    `Thinking...`, no duplicate `waiting on`, no misleading `Reply`, no
    bracketed `[Error: ...]` body, and the visible error still carries
    provider/model context.
- Phase 6 now has a repeatable PTY smoke harness:
  - `scripts/tui_pty_smoke.py` launches `target/debug/priority-agent --tui`
    inside a real PTY at one or more terminal sizes;
  - it can submit a startup-only, provider short-reply, or `pwd` tool-attempt
    prompt and writes both raw ANSI capture and stripped text under
    `target/tui-pty-smoke`;
  - the JSON summary reports whether the prompt was sent, whether a reply,
    optional interrupt key, shell/tool evidence, slow-provider state, raw
    provider logs, or deserialization noise appeared;
  - this is intentionally evidence-gathering, not a fake pass/fail wrapper, so
    incomplete provider/tool behavior remains visible.
- Phase 6 now improves long-wait active state copy:
  - generic `Thinking` states now include local turn elapsed time even before a
    provider diagnostic arrives;
  - if no tool/reply evidence appears after the slow-provider threshold, the
    active state is promoted to `slow <provider> (...)` using the local turn
    timer;
  - stale provider `Started` facade snapshots also use the local turn timer, so
    the UI does not depend solely on provider diagnostics to explain a long
    wait.
- Phase 6 now has first active-run cancellation plumbing:
  - Chat-mode `Esc` maps to the cancel action promised by `esc to interrupt`;
  - cancelling an active run aborts the stream task, clears querying state,
    marks active tool rows as cancelled, and fills an empty assistant placeholder
    with a cancelled notice;
  - query-time `Ctrl-C` now takes the same cancel path instead of immediately
    quitting the whole TUI, so a stuck provider/tool turn can return control to
    the composer;
  - quit cleanup still aborts an active run first and skips memory flush/agent
    cleanup for that interrupted path so exit does not wait on the stuck
    runtime turn;
  - the active refresh path no longer writes the full `AppContext` every tick,
    and the main loop bounds individual tick work so input handling stays
    responsive while a provider request is slow.
- Phase 6 now records DeepSeek tool-call compatibility more honestly:
  - DeepSeek still uses the OpenAI-compatible protocol family for ordinary text
    turns, but provider capability detection now marks DeepSeek tool-call
    requests as non-streaming compatibility paths;
  - this avoids the observed live state where DeepSeek streaming tool deltas
    reached `Running · queued` and never advanced into execution;
  - the UI now renders the compatibility state as
    `non-streaming tool request (DeepSeek)` and then `slow DeepSeek (...)`
    instead of leaving the user with a silent queued tool row.
- Phase 6 now has a visible provider-timeout guard for long TUI waits:
  - provider lifecycle snapshots can mark a declared provider timeout even if
    the provider request does not emit a terminal diagnostic;
  - the active-turn selector renders a product-level
    `Error: provider request timed out after ...` state when the visible turn
    has exceeded the effective request timeout;
  - explicit `PRIORITY_AGENT_LLM_REQUEST_TIMEOUT_SECS` values narrow the
    visible timeout guard for smoke/debug runs without changing MiniMax's
    default long non-streaming tool profile;
  - queued tool placeholders no longer block provider-wait timeout rendering;
  - TUI cancellation/timeout helpers now have both async cleanup and immediate
    visible-message paths, but real PTY evidence still shows that background
    provider task teardown needs more work.

Current verified gates:

```bash
cargo test -q tui::view_model --lib
cargo test -q main_screen --lib
cargo test -q rendered_query_state --lib
cargo test -q rendered_mid_width --lib
cargo test -q rendered_wide_sidebar --lib
cargo test -q rendered_turn_visual_state_stays_clean --lib
cargo test -q sidebar_layout --lib
cargo test -q snapshot_normalization_replaces_volatile_session_ids --lib
cargo test -q opencode_alignment_snapshots_can_be_dumped_for_visual_review --lib
cargo test -q completed_tool_turn_snapshots_can_be_dumped_for_visual_review --lib
cargo test -q snapshot_width_assertion_uses_terminal_display_width --lib
cargo test -q rendered_snapshot_lines_do_not_insert_placeholder_spaces_after_cjk --lib
cargo test -q completed_tool_turn_uses_semantic_styles --lib
cargo test -q provider_failure_turn_snapshots_stay_product_shaped --lib
cargo test -q provider_failure_turn_uses_error_semantic_style --lib
cargo test -q assistant_provider_error_uses_error_header_and_clean_body --lib
cargo test -q provider_and_model_labels_fallback_to_runtime_facade --lib
cargo test -q normal_footer_hides_default_permission_auto_to_avoid_mode_duplication --lib
cargo test -q normal_footer_keeps_version_out_of_the_daily_surface --lib
cargo test -q keeps_shell_and_failures_visible --lib
cargo test -q render_status_bar_keeps_left_padding --lib
cargo test -q render_status_bar_compacts_long_provider_model_without_losing_shortcuts --lib
cargo test -q render_status_bar_has_explicit_fallback_for_tiny_widths --lib
cargo test -q render_input_area_truncates_long_attachment_but_keeps_preview_hint --lib
cargo test -q render_input_area_compacts_context_strip_but_keeps_action_counts --lib
cargo test -q render_input_area_shows_non_default_mode_in_context_strip --lib
cargo test -q render_sessions_sidebar_metadata_fits_inline_width --lib
cargo test -q render_sessions_sidebar --lib
cargo test -q session_preview_truncation_uses_display_width --lib
cargo test -q ctrl_r_prompt_picker_restores_selected_prompt --lib
cargo test -q test_tui_startup_suppresses_terminal_logs_by_default --bin priority-agent
cargo test -q tui::view_model::activity --lib
cargo test -q test_action_for_chat_mode --lib
cargo test -q cancel_active_run_interrupts_query_and_marks_tool_cancelled --lib
cargo test -q test_message_metadata_round_trips --lib
cargo test -q session_store --lib
cargo test -q tui --lib
cargo check -q
git diff --check
```

Latest local verification after reclaiming disk space (2026-06-12):

```bash
cargo fmt --check
cargo test -q snapshots --lib
PRIORITY_AGENT_TUI_SNAPSHOT_DIR=target/tui-snapshots cargo test -q snapshots --lib
cargo test -q tui --lib
cargo check -q
git diff --check
rg -n "deepseek-v4-flash · auto|auto · DeepSeek|openai_compatible|v0\\.1\\.0|你 好|async_openai::error|failed deserialization of" target/tui-snapshots/*.txt
```

Result: formatting passed; snapshot tests passed (`5 passed`); generated
snapshot artifacts were refreshed under `target/tui-snapshots`; TUI module
tests passed (`332 passed`); `cargo check -q` passed; `git diff --check`
passed; the negative snapshot grep returned no matches. The refreshed
160-column alignment snapshot starts with
`◈ sess_demo · DeepSeek / deepseek-v4-flash`, proving the chat/session intro
now uses the display provider/model label and no longer shows
`deepseek-v4-flash · auto`.

Latest PTY provider smoke after adding `scripts/tui_pty_smoke.py` (2026-06-12):

```bash
cargo build -q
python3 -m py_compile scripts/tui_pty_smoke.py
cargo test -q tui::view_model::activity --lib
cargo test -q test_action_for_chat_mode --lib
cargo test -q cancel_active_run_interrupts_query_and_marks_tool_cancelled --lib
cargo test -q ctrl_c_cancels_active_query_without_quitting --lib
cargo test -q provider_lifecycle_marks_declared_timeout_without_provider_event --lib
cargo test -q facade_snapshot_marks_stale_provider_timeout --lib
cargo test -q selector_renders_explicit_timeout_as_error_state --lib
cargo test -q selector_shows_provider_timeout_before_generic_slow_wait --lib
cargo test -q provider_watchdog_honors_explicit_shorter_timeout --lib
cargo test -q provider_watchdog_times_out_query_when_provider_phase_is_lost --lib
cargo test -q provider_watchdog_ignores_queued_tool_placeholder --lib
cargo test -q timeout_active_run_immediate_writes_visible_error_without_await --lib
cargo test -q timeout_active_run_finishes_query_and_marks_tool_failed --lib
cargo test -q refresh_response_times_out_stale_provider_wait --lib
cargo test -q deepseek_tool_calls_use_nonstreaming_compatibility_path --lib
cargo test -q deepseek_frontend_tool_requests_use_nonstreaming_tool_path --lib
cargo test -q test_provider_type_capabilities --lib
cargo test -q snapshots --lib
cargo test -q tui --lib
cargo check -q
git diff --check
python3 scripts/tui_pty_smoke.py --prompt provider-ok --size 100x30 --size 120x35 --size 160x45 --timeout 75 --out-dir target/tui-pty-smoke
```

Result: script syntax passed; activity tests passed (`5 passed`);
Chat-mode keybinding, active-run cancellation, Ctrl-C cancellation, and
DeepSeek provider capability focused tests passed;
snapshot tests passed (`5 passed`); TUI module tests passed (`337 passed`);
`cargo check -q` and `git diff --check` passed. Real TUI PTY runs at 100x30,
120x35, and 160x45 all sent the provider prompt, produced a `Reply`, included
`DeepSeek / deepseek-v4-flash`, and did not show raw `async_openai::error` or
`failed deserialization of` noise. Captures were written to
`target/tui-pty-smoke-provider-final/`.

Latest PTY tool-attempt smoke:

```bash
python3 scripts/tui_pty_smoke.py --prompt tool-pwd --size 120x35 --timeout 120 --out-dir target/tui-pty-smoke-tool-after-deepseek-nonstreaming
PRIORITY_AGENT_LLM_REQUEST_TIMEOUT_SECS=30 python3 scripts/tui_pty_smoke.py --prompt tool-pwd --size 120x35 --timeout 60 --out-dir target/tui-pty-smoke-tool-timeout-render-error
```

Result: after DeepSeek was moved to the non-streaming tool-call compatibility
path, the prompt was sent in a real 120x35 TUI PTY and the provider/model label
rendered correctly. The previous `Running · queued` tool-row stall no longer
appeared, and the UI rendered `non-streaming tool request (DeepSeek)` followed
by `slow DeepSeek (...)`. No raw provider/deserialization noise appeared.
With a 30s explicit timeout override, the latest PTY capture also rendered a
visible `Error: provider request timed out after 30.0s` active state
(`saw_error=true`) instead of leaving the user with an indefinite slow-provider
line. However, the request still did not naturally produce `[Shell]`, `$ pwd`,
or a completed assistant/tool result, and the smoke harness still had to clean
up the live process after observing the visible error. Successful real DeepSeek
tool-turn completion and full background task teardown remain open.

Real-provider nightly/soak wrapper:

```bash
TUI_TOOL_TURN_SPINE_NIGHTLY_ROUNDS=3 \
  bash scripts/tui_tool_turn_spine_nightly.sh target/tui-tool-turn-spine-nightly
```

This runs `scripts/tui_tool_turn_spine_matrix.sh` repeatedly and writes each
round under `target/tui-tool-turn-spine-nightly/<run-id>/round-N/`, then emits
one combined readiness report at
`target/tui-tool-turn-spine-nightly/<run-id>/_readiness/readiness.md`. The
wrapper records provider/model/timeout settings in `manifest.json`, so a
failed nightly has the same contract fields as local PTY matrix results:
session events, terminal contract, persistence, projection, raw provider-log
leak checks, and provider label checks.

Latest PTY interrupt smoke:

```bash
python3 scripts/tui_pty_smoke.py --prompt tool-pwd --size 120x35 --timeout 20 --interrupt-after 0.5 --interrupt-key ctrl-c --out-dir target/tui-pty-smoke-interrupt-early
python3 scripts/tui_pty_smoke.py --prompt tool-pwd --size 120x35 --timeout 35 --interrupt-after 5 --interrupt-key ctrl-c --out-dir target/tui-pty-smoke-interrupt-final
```

Result: early Ctrl-C before the tool-call queued state exits cleanly
(`exitstatus=0`). After the same prompt reaches the active provider/tool
request state, Ctrl-C is also handled by the TUI (`exitstatus=0`), the capture
includes a cancelled reply, and the process no longer has to be terminated by
the smoke harness. This closes the queued/streaming responsiveness bug;
remaining risk is provider/tool natural completion, not user control recovery.

Manual smoke:

```bash
cargo run --quiet -- --tui
stty cols 100 rows 30; cargo run --quiet -- --tui
stty cols 120 rows 35; cargo run --quiet -- --tui
stty cols 160 rows 45; cargo run --quiet -- --tui
```

Result: startup in a PTY produced no raw stderr/log overlay during the initial
poll window and exited on Ctrl-C at 100x30, 120x35, and 160x45. The latest
120x35 smoke after the metadata/picker/attachment changes also produced no raw
stderr/log output before Ctrl-C. This verifies startup cleanliness and exit
behavior, but it does not replace full visual screenshot review during real
model/tool turns.

Latest live-provider smoke:

```bash
stty rows 30 cols 100; cargo run --quiet -- --tui
stty rows 35 cols 120; cargo run --quiet -- --tui
stty rows 45 cols 160; cargo run --quiet -- --tui
```

Result: with DeepSeek / `deepseek-v4-flash`, a short prompt entered a real
provider turn at 100x30, 120x35, and 160x45, showed a single active
`waiting on openai_compatible` row, and then rendered
`[Error: Failed to get response from deepseek API]` inside the assistant reply
area. No raw `async_openai::error` or JSON deserialization log painted over the
TUI during these runs. This validates the earlier log-suppression fix for the
common viewport live-provider failure path, but the provider did not return a
successful answer in these smokes.

Still not complete:

- live screenshot validation during real model/tool turns at 100x30, 120x35,
  and 160x45 with a successful provider response and a tool-using turn; the
  provider-failure path has been checked at all three sizes, and deterministic
  successful tool-turn snapshots now cover the completed transcript shape;
- deeper visual polish beyond the current snapshot text artifacts, especially
  color/contrast review against real terminal screenshots.

## 0. Why This Plan Exists

The current TUI is technically connected to the full runtime, but the product
surface is not yet usable enough for daily work. The latest manual run exposed
the main failure modes:

- duplicated live status: `Running · waiting`, input-area `Thinking...`, and
  bottom-bar `waiting on provider` all describe the same turn;
- message cards are too heavy: user headers occupy full-width grey bands and
  waste vertical space;
- tool state is noisy: multiple active/waiting rows appear without a clear
  single turn summary;
- bottom status bar is overloaded and truncates important state;
- the UI still feels like debug output arranged on a screen, not a coherent
  coding-agent terminal.

This plan compares the local opencode source at:

```text
/Users/georgexu/Downloads/opencode-dev/packages/opencode/src/cli/cmd/tui
```

against priority-agent's current TUI:

```text
src/tui/
```

The goal is not to port opencode's TSX/OpenTUI stack. The goal is to adopt the
product structure that makes opencode feel usable: a clean session timeline,
one authoritative turn status, compact tool rows, a capable prompt composer,
and predictable side surfaces.

## 1. opencode Patterns Worth Borrowing

### 1.1 Session Page Is a Composition Root

opencode's main session route is:

```text
packages/opencode/src/cli/cmd/tui/routes/session/index.tsx
```

Important patterns:

- session route owns high-level layout only;
- timeline is a scrollbox with stable message ids;
- user, assistant, reasoning, text, and tool parts are separate components;
- prompt, permission prompt, question prompt, subagent footer, sidebar, toast,
  and dialogs are separate surfaces;
- command bindings are registered from one list instead of scattered across
  render code.

Relevant code shape:

- `messages()` is projected from sync state;
- `pending()` is derived from the last unfinished assistant message;
- `scrollbox` has sticky bottom behavior and message-id navigation;
- `UserMessage`, `AssistantMessage`, `ReasoningPart`, `TextPart`, `ToolPart`
  render different content types independently.

### 1.2 One Footer, Not Three Competing Status Lines

opencode's session footer:

```text
packages/opencode/src/cli/cmd/tui/routes/session/footer.tsx
```

It is deliberately small:

- left: current directory;
- right: connection, permissions, LSP, MCP, `/status`;
- it does not duplicate the active assistant/tool row;
- it uses muted text by default and only highlights actionable warnings.

The active model/tool state is shown near the active message/tool part, not
repeated in the footer, prompt, and timeline.

### 1.3 Prompt Is a First-Class Composer

opencode prompt composer:

```text
packages/opencode/src/cli/cmd/tui/component/prompt/index.tsx
```

Important behaviors:

- supports history and prompt stash;
- supports paste blocks and file attachments;
- shows editor/file context inline;
- owns autocomplete;
- reacts to session status but does not replace the entire input with a
  separate `Thinking...` block while the model is running;
- has slots for right-side provider/session metadata.

### 1.4 Tool Rendering Is Typed and Collapsible

opencode tool rendering lives in the session route:

```text
packages/opencode/src/cli/cmd/tui/routes/session/index.tsx
```

Useful pieces:

- `ToolPart` dispatches by tool kind: shell/read/grep/write/edit/task/etc.;
- simple completed tools can render as compact inline rows;
- large shell output renders as a block with a bounded preview;
- generic tool output is hidden by default and can be toggled;
- failed tools expand inline on click;
- output collapse uses a tiny deterministic function:

```text
packages/opencode/src/cli/cmd/tui/util/collapse-tool-output.ts
```

### 1.5 TUI State Is a Projected Store

opencode sync context:

```text
packages/opencode/src/cli/cmd/tui/context/sync.tsx
```

It keeps UI state as projected DTOs:

- sessions;
- session status;
- messages;
- parts;
- permissions;
- questions;
- todos;
- LSP/MCP/provider state.

This makes rendering deterministic: the UI consumes stable projections instead
of interpreting raw event fragments in many places.

## 2. Initial priority-agent Gaps

These were the gaps observed before the first implementation slice. The
implementation progress section above is authoritative for what is already
fixed.

### 2.1 Main Screen Did Too Much

Current file:

```text
src/tui/screens/main_screen.rs
```

Initial problems:

- `render_chat_area()` handles intro line, scroll math, message rendering,
  tool grouping, scroll indicators, and active-turn anchoring;
- `render_input_area()` also owns querying state (`Thinking...`);
- `render_live_activity_row()` is another active status surface;
- tool runs are injected between user messages instead of being projected as
  assistant/tool parts;
- scroll model is item-index based and approximate, not stable message-id based.

### 2.2 Message Cards Were Too Heavy

Current file:

```text
src/tui/components/message.rs
```

Initial problems:

- user message header uses a full-width background band;
- assistant response header is separated from body but not integrated with
  final metadata cleanly;
- relative time is always shown in the user header, creating noise;
- markdown body is always indented with fixed prefixes;
- reasoning/thinking is not a collapsible assistant part.

### 2.3 Runtime Status Was Duplicated

Current files:

```text
src/tui/screens/main_screen.rs
src/tui/screens/main_screen/status_bar.rs
src/tui/app/runtime.rs
```

Initial problems:

- `render_live_activity_row()` and `render_status_bar()` both render active
  provider/tool status;
- `render_input_area()` replaces the prompt with `Thinking...`;
- active tool rows in the transcript render another running/waiting state;
- the bottom bar mixes turn status, provider, permission, model, context,
  cache, validation, changed files, version, shortcuts, vim, and memory.

### 2.4 Tool Rows Were Not Yet Product-Grade

Current files:

```text
src/tui/tool_view.rs
src/tui/screens/main_screen.rs
```

Good foundation already exists:

- `ToolRunView` has typed status;
- `summary()` and `detail_line()` already classify common tools;
- `result_preview` and `result_body` exist;
- tool viewer modal exists.

Initial gaps:

- active/waiting rows can repeat visually;
- completed read-only tools are too visible for routine turns;
- output preview policy is not as crisp as opencode's inline/block split;
- tool grouping is tied to user message placement, not assistant part order.

### 2.5 Prompt Was Still a Text Box, Not a Composer

Current files:

```text
src/tui/screens/main_screen.rs
src/tui/app.rs
src/tui/components/input.rs
```

Initial gaps:

- querying state replaces input content instead of disabling or dimming the
  composer;
- no first-class prompt attachment/file-context strip;
- paste handling and prompt history are not surfaced as composer affordances;
- provider/model and agent-mode metadata lives mostly in the footer.

## 3. Product Target

The target TUI should feel like this:

1. **Timeline first.** The main pane is a stable session timeline.
2. **One active turn indicator.** At any moment there is one authoritative line
   for "waiting on provider", "running bash", "editing file", or "thinking".
3. **Compact routine work.** Completed reads, searches, and successful shell
   commands collapse to one line unless expanded.
4. **Rich only when useful.** File edits, failed tools, approvals, diffs, and
   validation get richer blocks.
5. **Prompt remains available.** The input region stays visually stable while
   the turn runs; it should not become a giant `Thinking...` placeholder.
6. **Footer is quiet.** The footer reports durable environment state, not the
   same transient turn status already shown above.

## 4. Proposed Architecture

### 4.1 Add a TUI View Model Layer

New module:

```text
src/tui/view_model/
```

Initial files:

```text
src/tui/view_model/mod.rs
src/tui/view_model/timeline.rs
src/tui/view_model/activity.rs
src/tui/view_model/tool_rows.rs
src/tui/view_model/footer.rs
```

Responsibilities:

- convert `TuiApp` state into render DTOs;
- keep layout decisions out of raw rendering functions;
- make status precedence explicit;
- provide stable ids for scroll and expansion state;
- expose small pure functions that are easy to unit test.

Core DTO sketch:

```rust
pub enum TimelineItem {
    User(UserBubble),
    Assistant(AssistantBlock),
    Reasoning(ReasoningBlock),
    Tool(ToolRow),
    Notice(NoticeRow),
}

pub struct ActiveTurnStatus {
    pub phase: ActivePhase,
    pub label: String,
    pub detail: Option<String>,
    pub elapsed_ms: Option<u64>,
    pub interrupt_hint: bool,
}

pub enum ActivePhase {
    Idle,
    ProviderWaiting,
    Thinking,
    ToolRunning,
    PermissionWaiting,
    Finalizing,
    Failed,
}
```

### 4.2 Split Main Screen Rendering

Refactor current `src/tui/screens/main_screen.rs` into smaller modules:

```text
src/tui/screens/main_screen/mod.rs
src/tui/screens/main_screen/timeline.rs
src/tui/screens/main_screen/composer.rs
src/tui/screens/main_screen/activity.rs
src/tui/screens/main_screen/sidebar.rs
src/tui/screens/main_screen/popups.rs
src/tui/screens/main_screen/status_bar.rs
```

Keep ratatui. Do not switch UI frameworks.

The render flow should become:

```text
render_main()
  build_view_model(app)
  render_timeline(items)
  render_active_turn(status)        # only if active
  render_composer(composer_state)
  render_footer(footer_state)
  render_overlays()
```

## 5. Implementation Phases

### Phase 0: Stabilize the Current Broken Surface

Goal: fix the screenshot-level problems without a broad refactor.

Tasks:

1. Remove input-area `Thinking...` replacement.
   - File: `src/tui/screens/main_screen.rs`
   - Behavior: while querying, keep the prompt line visible but disabled/muted.
   - Active status moves to one dedicated row above the footer or above the
     prompt.

2. Choose one active status owner.
   - Keep: a single `render_active_turn_row()`.
   - Demote: bottom status bar should not repeat provider wait text.
   - Demote: tool transcript should show active tools, but not duplicate generic
     "Running · waiting" rows when there is no concrete tool identity.

3. Simplify status bar density.
   - Default footer should show only:
     - mode;
     - short session id;
     - provider/model;
     - memory mode;
     - permissions if non-default;
     - `/status` or `?`.
   - Move cache/context/validation/changed files behind `StatusBarDensity::Debug`
     or a `/status` panel.

4. Reduce user message visual weight.
   - Remove full-width user background band by default.
   - Use a narrow left marker or compact header.
   - Hide timestamps by default; expose via shortcut/slash command later.

Acceptance:

- Screenshot no longer shows three separate active statuses.
- User message header no longer occupies a full-width grey band.
- While a request is running, the input area remains stable.
- `cargo test -q tui --lib` passes.
- Manual `cargo run --quiet -- --tui` shows no stderr/log overlay.

### Phase 1: Tool Row Normalization

Goal: make tools readable and quiet.

Tasks:

1. Add `src/tui/view_model/tool_rows.rs`.
2. Convert `ToolRunView` into `ToolRow`:
   - icon;
   - color class;
   - one-line summary;
   - detail;
   - preview;
   - expandable flag;
   - severity.
3. Port opencode-style output collapse:
   - max lines for inline preview;
   - max chars based on terminal width;
   - overflow indicator.
4. Hide routine completed read-only tools by default when successful.
5. Always show:
   - file mutations;
   - shell commands;
   - failed/timed-out/cancelled tools;
   - permission waits;
   - validation and closeout proof.

Acceptance:

- A simple `cargo check -q` turn shows one active shell row and one compact
  completed shell row.
- Failed command expands to show stderr/error.
- Long output does not push the prompt off-screen.
- Unit tests cover collapse and severity mapping.

Narrow gates:

```bash
cargo test -q tool_view --lib
cargo test -q tui::view_model --lib
cargo check -q
```

### Phase 2: Timeline View Model

Goal: stop rendering directly from mixed app state.

Tasks:

1. Add `TimelineItem` DTO.
2. Build timeline from:
   - `app.visible_messages()`;
   - `app.tool_runs_for_message()`;
   - active runtime snapshot;
   - closeout/validation events if already projected.
3. Preserve stable ids:
   - message id;
   - tool call id;
   - synthetic notice ids.
4. Move transcript height estimation to item-specific methods.
5. Replace index-only scrolling with stable-item anchors where possible.

Status: implemented for manual scroll, `/jump`, message-search jump, top,
bottom, half-page movement, and Vim-normal collapse. Numeric offsets remain as
fallback if an anchor id disappears.

Acceptance:

- Existing collapse/expand state survives new messages.
- Scroll-to-bottom does not jump unpredictably while streaming.
- Active turn stays visible without hiding the user's prompt context.
- Manual scroll anchors preserve the selected timeline item when new items are
  inserted before it.

Narrow gates:

```bash
cargo test -q main_screen --lib
cargo test -q session_manager --lib
cargo check -q
```

### Phase 3: Message Rendering Refresh

Goal: make the transcript feel like a professional coding-agent terminal.

Tasks:

1. Split `src/tui/components/message.rs`:

```text
src/tui/components/message/mod.rs
src/tui/components/message/user.rs
src/tui/components/message/assistant.rs
src/tui/components/message/reasoning.rs
src/tui/components/message/notice.rs
```

Status: first split implemented for `user.rs`, `assistant.rs`, `text.rs`,
`reasoning.rs`, `notice.rs`, and `system_tool.rs`. Remaining work is richer
assistant part behavior, not the basic module split.

2. Add display preferences:
   - timestamps hidden by default;
   - show/hide thinking;
   - compact/expanded tool details.

3. Add reasoning block support:
   - streaming spinner while active;
   - collapsed `Thought: summary · duration` when complete;
   - expanded body only on demand.

Status: collapsed reasoning summary, bounded on-demand expanded reasoning body,
and completed-assistant UI metadata are implemented for model, token usage,
elapsed duration, provider phase, tool outcome count, and validation-tool
status. Persistent metadata across session reloads is implemented through the
`messages.metadata` JSON column.

4. Clean markdown body:
   - less fixed indentation;
   - no unnecessary blank line after every header;
   - table/code rendering remains readable.

Acceptance:

- Normal chat uses fewer vertical rows than current TUI.
- Assistant answer metadata appears once after completion.
- Reasoning/thinking does not dominate the screen.

### Phase 4: Composer Upgrade

Goal: make the prompt area an actual working surface.

Tasks:

1. Create `src/tui/screens/main_screen/composer.rs`.
2. Keep composer visible while querying.
3. Add compact context strip:
   - current provider/model;
   - active agent mode;
   - attached file/context count;
   - pending paste block count.
4. Add visible prompt history/stash affordance in command palette first; avoid
   a big bespoke UI until the basics are clean.
   - Status: implemented with `/prompt-history`, `/prompt-stash`, contextual
     palette boosting, composer strip indicators, and the `Ctrl+R`
     prompt-history/stash picker.
5. Improve paste behavior:
   - pasted multi-line blocks become a visible collapsed attachment row;
   - expanded view available in a modal.
   - Status: collapsed strip summary implemented. `/paste [n]` opens the
     selected paste block in the existing viewer modal.
6. Add first file/context attachments:
   - attach existing paths with `/attach <path>`;
   - list, remove, and clear attachments;
   - inject the one-shot attachment list into the next normal prompt.
   - Status: command-driven attachment workflow implemented.
7. Add first file-picker attachment path:
   - `/attach browse [root]` opens a modal file picker;
   - Enter expands directories or attaches selected files;
   - selected files flow through the same one-shot attachment payload.
   - Status: implemented with `AppMode::FilePicker` and
     `src/tui/components/file_browser.rs`.
8. Add file-picker filtering:
   - `/` focuses the filter row;
   - typed characters live-filter file names and paths;
   - Backspace edits the filter, Enter/Esc exits filter mode;
   - filtered selection can still attach the matched file.
   - Status: implemented in `FileBrowserState` and FilePicker key handling.
9. Add attachment preview:
   - `/attach preview [n]` opens the existing viewer modal for the selected
     attachment;
   - files show a bounded text preview;
   - directories show a sorted listing;
   - attachment summaries include stable one-based indices for preview/remove.
   - Status: implemented.
10. Add richer attachment metadata and discoverable removal:
   - composer strip, Context sidebar, `/attach list`, and prompt payload all
     show stable indexed summaries with file/dir/missing state;
   - file attachments show size, directory attachments show item count;
   - composer surfaces Backspace removal, while `/attach list` shows preview and
     remove commands next to the attachment.
   - Status: implemented.
11. Add keyboard-driven inline attachment removal:
   - Backspace removes the last attachment when the prompt is empty;
   - Backspace continues to edit prompt text when text is present.
   - Status: implemented.
12. Move attachment details to a dedicated composer row:
   - keep the context strip focused on mode/provider/model/history;
   - render indexed attachment summaries separately with overflow count and
     preview/removal hints.
   - Status: implemented.

Acceptance:

- Input area height is stable.
- Multi-line input wraps correctly.
- Running state does not replace user draft.
- Composer text never overlaps footer.
- Attached file/context paths are visible before send and included in the next
  prompt payload.

### Phase 5: Sidebar and Navigation

Goal: make session navigation useful without stealing space.

Tasks:

1. Keep sidebar hidden by default on narrow terminals.
2. Use width threshold similar to opencode:
   - auto sidebar only on wide terminals;
   - overlay sidebar on narrow terminals.
   - Status: implemented with a 140-column inline threshold and overlay
     fallback for narrower terminals. Inline sidebars use a fixed 40-column
     panel; overlay sidebars leave composer/footer visible and clear the
     transcript backdrop behind the panel to prevent text bleed.
3. Move context/session side panels behind explicit commands.
4. Add message jump/timeline command:
   - jump to user messages;
   - jump to failed tools;
   - jump to latest edit.
   - Status: first slice implemented with `/jump [user|failed|edit|latest]`.
5. Add selected-session preview snippets:
   - show one bounded recent user/assistant preview line under the selected
     session;
   - keep unselected rows compact so the session list remains scannable.
   - Status: implemented.
6. Persist pinned sessions:
   - store pinned session ids in `AppConfig.ui.pinned_sessions`;
   - load them on TUI startup;
   - save after sidebar pin/unpin.
   - Status: implemented.

Acceptance:

- 100-column terminal remains usable.
- Sidebar never compresses the timeline below a useful width.
- User can jump between important events without scrolling through every tool.

### Phase 6: Theme and Visual System

Goal: visual polish after structure is fixed.

Tasks:

1. Make the default palette quieter:
   - less full-width background;
   - fewer saturated warning colors;
   - consistent muted metadata;
   - one accent color per semantic role.
2. Add theme token audit:
   - message backgrounds;
   - active row;
   - tool success/error/warning;
   - footer muted/active.
3. Add screenshot-based smoke test path:
   - run TUI in PTY;
   - capture known states;
   - assert no raw logs, no duplicate status strings, no overlapping footer.
   - Status: deterministic snapshot artifact generation is implemented through
     `PRIORITY_AGENT_TUI_SNAPSHOT_DIR=target/tui-snapshots cargo test -q opencode_alignment_snapshots_can_be_dumped_for_visual_review --lib`
     and
     `PRIORITY_AGENT_TUI_SNAPSHOT_DIR=target/tui-snapshots cargo test -q completed_tool_turn_snapshots_can_be_dumped_for_visual_review --lib`.
     Provider-failure snapshots are covered by
     `PRIORITY_AGENT_TUI_SNAPSHOT_DIR=target/tui-snapshots cargo test -q provider_failure_turn_snapshots_stay_product_shaped --lib`.

Acceptance:

- Manual snapshot comparison against opencode-like target passes for simulated
  active tool, selected-sidebar, completed validation-turn, and provider-failure
  states; real successful provider/tool screenshots still need broader coverage.
- Default theme reads as calm, dense, and work-focused.
- No text overlaps at 100x30 and 160x45.

## 6. Specific Current Bugs to Fix First

These are not optional polish; they directly affect usability.

### Bug A: Duplicate Active Status

Original sources:

```text
render_input_area()           -> "Thinking..."
render_live_activity_row()    -> current tool/thinking
render_status_bar()           -> waiting on provider/tool
render_tool_runs_message()    -> running/waiting tool rows
```

Fix:

- introduce one `ActiveTurnStatus` selector;
- render it exactly once;
- status bar gets only durable state;
- tool rows render concrete tool activity, not generic provider wait.

Status: implemented in the first slice.

### Bug B: User Message Band Is Too Heavy

Current source:

```text
src/tui/components/message.rs::render_user_message()
```

Fix:

- remove default full-row background;
- show timestamp only when `show_timestamps` is enabled;
- compact short messages into two lines: header + body.

Status: default slab/timestamp behavior is implemented. A user-facing
`show_timestamps` preference is still future work.

### Bug C: Old Config/Provider Logs Pollute TUI

Partially fixed in recent work:

```text
src/main.rs
src/services/config.rs
```

Remaining requirement:

- TUI startup must never write logs into the alternate screen;
- provider/runtime warnings should become in-app toasts or `/status` entries,
  not raw stderr.

### Bug D: Provider Wait Can Look Hung

Current screenshot shows:

```text
waiting on openai_compatible · 2.7s · esc to interrupt
```

Fix:

- distinguish provider wait, model thinking, tool running, closeout finalizing;
- show elapsed and provider/model once;
- after slow threshold, show one concise hint:
  `provider slow · 12.4s · esc interrupt · /status details`.

Status: active provider/tool/wait precedence is implemented. Closeout
finalizing and `/status details` remain future work.

## 7. Migration From Existing TUI Plans

Existing docs:

```text
docs/archive/NEXT_DEV_PLAN_2026-06-09.md
docs/archive/TUI_OPTIMIZATION_PLANS_2026-06-09.md
docs/archive/TUI_GAP_ANALYSIS_2026-06-02.md
```

Recommendation:

- keep old plans as historical context;
- treat this document as the next active TUI execution plan;
- defer old diff-viewer/session-selector polish until Phase 0-2 are done;
- do not start with more features. The current blocker is basic usability.

## 8. Validation Plan

### Unit Tests

```bash
cargo test -q tui::view_model --lib
cargo test -q tool_view --lib
cargo test -q main_screen --lib
cargo test -q status_bar --lib
cargo check -q
```

### Manual TUI Scenarios

Run in real terminal:

```bash
cargo run --quiet -- --tui
```

Scenarios:

1. Simple greeting.
   - No raw logs.
   - No duplicate running/waiting state.
   - User/assistant transcript is compact.

2. `cargo check -q`.
   - One active shell row.
   - One completed shell row.
   - Footer remains quiet.

3. Long output command.
   - Preview collapses.
   - Prompt remains visible.
   - Tool viewer can show full output.

4. Failed command.
   - Error row is visible.
   - Details are expandable.
   - Final answer cannot claim success unless evidence exists.

5. Permission request.
   - Permission prompt owns the interaction.
   - Footer shows permission count, not full duplicated status.

### Visual Acceptance

Capture screenshots at:

- 100x30;
- 120x35;
- 160x45.

Reject if:

- same active state appears in more than one place;
- prompt/footer overlap;
- raw JSON/log lines appear;
- completed read-only tools dominate transcript;
- timestamps are always visible by default;
- message backgrounds create full-width slabs for routine chat.

## 9. Non-Goals

- Do not rewrite the TUI in TypeScript/OpenTUI.
- Do not create a desktop-first replacement for this work.
- Do not add more slash commands before the default screen is usable.
- Do not hide verification/permission evidence; move it to the right surface.
- Do not weaken runtime gates to make the UI cleaner.

## 10. First Implementation Slice

Scope:

```text
src/tui/view_model/activity.rs
src/tui/view_model/footer.rs
src/tui/view_model/tool_rows.rs
src/tui/view_model/timeline.rs
src/tui/screens/main_screen/composer.rs
src/tui/screens/main_screen.rs
src/tui/screens/main_screen/status_bar.rs
src/tui/components/message.rs
```

Deliverables:

1. `ActiveTurnStatus` pure selector.
2. One active row rendered in one place.
3. Input composer remains visible while querying.
4. Default footer becomes quiet.
5. User message card becomes compact.
6. `ToolRow` projection for compact tool rows and hidden routine read/search
   summaries.
7. `TimelineItem` projection for message/tool grouping.
8. `FooterItem` projection for quiet default footer and debug-only diagnostics.
9. Composer context strip for mode, provider/model, memory, and paste blocks.
10. Tests for active-status precedence, tool-row visibility, timeline
    projection, footer projection, and composer rendering.

This slice should immediately make the TUI less frustrating while leaving the
larger timeline, reasoning, sidebar, and screenshot-validation work explicit.
