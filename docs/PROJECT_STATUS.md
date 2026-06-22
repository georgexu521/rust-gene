# Project Status
Status: Current

Last updated: 2026-06-22

## Release Structure Cleanup — Structure Baseline Complete (2026-06-22)

Current release-structure cleanup is tracked in
`docs/RELEASE_STRUCTURE_CLEANUP_RECOMMENDATIONS_2026-06-22.md`.

The release cleanup repaired the root `legacy-cli` feature mismatch, refreshed
the docs index away from missing historical links, updated release metadata,
removed the LabRun clippy failures, split the oversized LabRun
command/orchestrator/draft/model/store modules by responsibility, and added
baseline rustdoc comments to the LabRun public model and entry APIs.

The secondary structure pass also split the remaining non-LabRun production
files that exceeded the 1500-line project ceiling: TUI input/app runtime state,
shell LabRun commands/tests, learning goal slash commands, agent-tool support,
API bridge routes, streaming text-progress helpers, and session-action tests.
Current scan result: no non-test production Rust file exceeds 1500 lines; the
largest direct production file is `src/tools/agent_tool/mod.rs` at 1496 lines.

Validated in this slice:

```bash
cargo fmt --check
cargo check -q
cargo check --features legacy-cli -q
cargo check --features experimental-api-server -q
cargo doc --no-deps -q
cargo clippy --lib --all-features -- -D warnings
cargo clippy --all-targets --all-features -- -D warnings
cargo test -q lab --lib
cargo test -q
bash scripts/validate_docs.sh
git diff --check
```

Remaining before a broad repository release-ready claim:

- no repository-structure cleanup items remain from
  `docs/RELEASE_STRUCTURE_CLEANUP_RECOMMENDATIONS_2026-06-22.md`; continue
  with the normal release publication checklist for packaging/distribution.

## Remaining Structure Refinement — Implemented (2026-06-22)

The follow-up refinement plan is tracked in
`docs/REMAINING_STRUCTURE_REFINEMENT_PLAN_2026-06-22.md`.

Implemented in this slice:

- Current product wording is aligned around the Rust programming-agent terminal
  CLI direction in source comments, README, and `/about` output.
- TUI command maturity now has explicit `production`, `usable`,
  `experimental`, `diagnostics`, `placeholder`, and `unavailable` labels.
  Placeholder and unavailable commands stay hidden from default help/palette
  surfaces unless explicitly queried.
- `scripts/check_source_file_sizes.sh` now enforces the non-test production
  Rust file line ceiling, and `scripts/validate_docs.sh` runs it.
- `docs/PROJECT_MAP.md` now classifies the top-level source tree by runtime
  role: runtime core, product surfaces, integrations/adapters, diagnostics, and
  internal/historical support.
- `src/lib.rs` now documents the intended public surface and keeps
  `ai_analyzer`, `context_manager`, `priority`, `task_analyzer`,
  `task_manager`, and `team` crate-private as internal/historical support.

Validation so far:

```bash
cargo fmt --check
cargo check -q
bash scripts/check_source_file_sizes.sh
current product wording search excluding archived docs and the refinement-plan self-reference
cargo test -q tui::commands --lib
cargo test -q --features experimental-api-server api::routes --lib
cargo test -q lab --lib
```

## Desktop Frontend Workbench — Native Real-Provider QA Passing (2026-06-21)

Desktop frontend work is now tracked in
`docs/DESKTOP_FRONTEND_PRODUCT_PLAN_2026-06-21.md`. The direction is to turn
`apps/desktop` into a daily-use agent workbench modeled after Codex/OpenCode
interaction patterns while preserving the existing Rust runtime boundary.
The current broad dirty worktree is mapped in
`docs/DESKTOP_FRONTEND_CHANGESET_CLOSEOUT_2026-06-22.md`, including suggested
commit scopes, validation evidence, and the remaining gates before this work
can be called release-ready.
Requirement-level completion status is audited in
`docs/DESKTOP_FRONTEND_COMPLETION_AUDIT_2026-06-22.md`; it concludes the
implementation is ready for commit closeout, but the overall goal should remain
open until a clean commit boundary exists.

### Product target

- Keep two parallel modes: Direct Agent Mode for normal Codex/OpenCode-style
  coding tasks, and LabRun Mode for professor/postdoc/graduate project loops.
- Use a three-pane desktop workbench: left project/session navigation, center
  transcript/composer, right persistent inspector.
- Make runtime truth visible: tool calls, permissions, validation, context,
  cache, compression, LabRun status, reports, and subagent artifacts should come
  from typed runtime APIs rather than frontend inference.
- Keep Tauri + React; borrow OpenCode's product structure and component
  patterns, not its Electron stack.

### Current status

- P0 audit and product plan are complete.
- P1 workbench shell is complete: the desktop app now has a session header,
  clear Direct Agent / LabRun mode entry, and a persistent right-side runtime
  inspector with Context, Files, Execution, Subagents, LabRun, and Diagnostics
  tabs. The Files inspector now includes a selected-file preview path backed by
  a bounded Tauri API that only reads files inside the selected project.
- Existing `apps/desktop` already has substantial runtime plumbing: health,
  settings, provider/model status, sessions, diagnostics, context/workbench
  snapshots, trace/tool output drawers, goal row, permission recovery, and Lab
  daemon supervision.
- Existing Workbench drawer remains available for narrow screens and legacy
  action flows.
- P2 Direct Agent polish is complete for this slice: composer slash commands now surface common Direct Agent,
  goal, context, and LabRun commands from the input box without bypassing the
  normal runtime send path. Composer prompt history now recalls same-session
  submitted prompts with ↑ / ↓ while preserving slash-command navigation. A
  global `/` shortcut now focuses the composer and opens slash commands when
  the user is not already typing. Composer context attachments now show project
  context, current-diff/file summaries, explicit open actions, and explicit
  remove actions. The Execution inspector now shows trace evidence directly and
  reads stored tool-output index/page data for the active session, while the
  drawers remain available for larger views. The Context inspector now expands
  the status-bar summary into token budget, runtime estimate, history/tool/
  memory token split, stable-prefix fingerprint, prompt-cache read/miss/hit-rate
  diagnostics, compression attempts, and the latest provider usage event. It
  now shows real provider input/output/total/reasoning/cache-write tokens when
  the runtime event exposes them, while missing provider fields remain
  explicitly `unavailable`. Transcript run rows now include a grouped run summary panel for
  validation, diff/file changes, permission requests, failures, generic tools,
  final text, and trace links while preserving the raw timeline evidence below.
- Provider/model selection is now a clearer popover with missing-provider setup
  repair: users can see unconfigured providers, open the setup path for a
  provider, paste an API key through the existing Rust credential command, and
  refresh provider diagnostics without leaving the composer flow.
- P3 LabRun desktop surface is complete for the current typed snapshot: the LabRun inspector now has
  proposal/intake and approve/draft actions, project controls for resume,
  pause, continue, and meeting open, plus a dedicated professor side-channel
  that stages professor/intervention messages into the normal runtime command
  path instead of letting the frontend command postdoc/graduate agents directly.
- The LabRun inspector also now has a status board for stage, owner, cycle,
  task progress, blockers, needs-user state, meeting recommendation, and topic;
  report/artifact actions for latest report, report list, and review state; and
  a cost/context/cache panel backed by the runtime context snapshot.
- Desktop API now exposes structured LabRun artifacts, reports, and evidence
  refs from `LabStore`; the LabRun inspector renders those rows directly, with
  report-open and artifact-review actions still routed through existing runtime
  commands.
- LabRun artifact/report rows now include short report previews, and the LabRun
  inspector supports local search across artifact metadata, report preview text,
  and evidence refs without adding frontend judgment about task quality.
- Desktop API now exposes guarded paged LabRun markdown report reads through
  `desktop_lab_report_page`; it resolves paths under the selected project's
  `.priority-agent/lab` tree and rejects non-markdown/out-of-tree reads. The
  LabRun inspector can preview full report pages in place with previous/next
  paging while keeping the existing runtime command/open-report actions.
- Desktop API now exposes guarded LabRun artifact body reads through
  `desktop_lab_artifact_body`; it resolves the latest LabRun from the selected
  project and only reads artifact ids registered on that run. The LabRun
  inspector can preview the structured body JSON in place next to report
  previews, without letting the frontend read arbitrary artifact files.
- Desktop frontend state cleanup has started: `useDesktopBootstrap` now owns
  desktop health/settings/provider/session/diagnostics bootstrap plus refresh
  helpers, `useWorkbenchSnapshots` now owns context/workbench snapshot loading,
  and `useRunEvents` now owns run event subscription, idle watchdog, permission
  answer handling, and submit-message event plumbing. The long-lived event
  subscription now reads latest provider/settings/refresh callbacks through refs
  instead of holding stale startup values. `App.tsx` stays responsible for shell
  orchestration, conversation recovery, commands, drawers, and layout, but is
  back under the project 1500-line source-file ceiling.
- Desktop inspector state cleanup has also started: shared metric, key-value,
  empty-state, token, and byte-format helpers now live in
  `InspectorPrimitives.tsx`, leaving `InspectorPanel.tsx` under the project
  1500-line source-file ceiling while preserving the Context, Files, Execution,
  Subagents, LabRun, and Diagnostics inspector behavior.
- Desktop runtime API source cleanup has started: shared desktop DTO/type
  definitions now live in `desktopTypes.ts`, goal command helpers live in
  `desktopGoalApi.ts`, and `desktopApi.ts` re-exports the same public
  type/function surface for existing callers while dropping back under the
  project 1500-line source-file ceiling. The browser-preview fixture path has
  also moved into `desktopPreview.ts`: preview run events, permission answers,
  manual compaction, LabRun report/artifact fixtures, file previews, and
  listener fanout are now separated from the real Tauri API boundary, reducing
  `desktopApi.ts` to 1065 lines and leaving room for later API additions.
- Desktop run-event source cleanup has started: tool/permission presentation
  helpers now live in `runEventPresentation.ts`, while `runEventState.ts`
  retains the state-transition API used by `useRunEvents`, transcript loading,
  permission answers, idle warnings, and error handling.
- Desktop app shell cleanup continues: startup recovery/restored-session banner
  rendering now lives in `StartupStateCard.tsx`, preserving Lab recovery
  Resume/Dashboard/Keep paused behavior while keeping `App.tsx` below the
  project 1500-line source-file ceiling.
- Desktop app shell cleanup continues: topbar rendering, context meter,
  environment popover, and workbench/trace/output header controls now live in
  `WorkspaceTopbar.tsx`; `App.tsx` keeps the orchestration callbacks while the
  topbar behavior remains covered by the desktop UI smoke suite.
- Desktop composer polish continues: the Add context menu no longer presents
  Screenshot as a disabled/dead action before native screenshot context support
  exists. It is now rendered as a non-actionable unavailable note while Current
  diff and File remain real context actions, with desktop layout smoke coverage.
- Desktop destructive-action polish continues: deleting a session now uses a
  focused `DeleteSessionDialog` with initial focus on Cancel, Tab/Shift+Tab
  containment, and Escape-to-cancel behavior, while preserving the existing
  delete API path.
- Desktop export feedback now has an explicit `ExportNoticeBanner`: successful
  exports render as a status message with Open export and Dismiss actions rather
  than a persistent inline banner with embedded styling.
- Desktop environment popover polish continues: the topbar environment summary
  now closes with Escape and outside clicks, and the desktop smoke covers both
  paths while preserving the existing runtime-sourced environment details.
- Desktop Rust state cleanup has started: native smoke project preparation,
  schedule helpers, and injected smoke scripts now live in
  `desktop_state/native_smoke.rs`, while `desktop_state.rs` keeps
  settings/provider/session state helpers and is back under the project
  1500-line source-file ceiling. The split preserves the existing
  `desktop_state::*` surface consumed by `lib.rs`.
- Desktop Tauri DTO cleanup has started: shared command response/settings/
  diagnostic/provider/session DTOs now live in `desktop_types.rs`, while
  `lib.rs` keeps command handlers and runtime orchestration. The split keeps
  existing command names and frontend API payload shapes unchanged.
- Desktop Tauri command cleanup has also started: health, session/tool-output,
  preview/read, goal, and revert commands now live in focused modules
  (`health_commands.rs`, `session_commands.rs`, `preview_commands.rs`,
  `goal_commands.rs`, and `revert_commands.rs`). `lib.rs` is back under the
  project 1500-line source-file ceiling while retaining Tauri command
  registration and app startup orchestration.
- Desktop CSS source cleanup has started: `global.css` is now a small ordered
  import entrypoint, and the existing style rules live in `styles/parts/*.css`
  by UI domain. The split preserves selector order and keeps each desktop
  source/style file in this slice under the project 1500-line ceiling.
- Release-readiness QA found that frontend-only stale-session cleanup was too
  late: native startup could still surface `session not found` from a stale
  Rust-side `active_session_id`. The desktop backend now validates active
  session ids before returning settings or initializing runtime state, clears
  stale ids, and persists the corrected settings.
- Native Tauri smoke now passes and produces
  `apps/desktop/test-artifacts/native-smoke.png`; narrow viewport QA produces
  `apps/desktop/test-artifacts/desktop-narrow-loaded.png`. The smoke fixture was
  updated to accept the current run-state/context-usage UI wording. The
  initialization path now ignores stale `active_session_id` values that no
  longer exist in the recent session list, preventing a red `session not found`
  banner on a fresh desktop launch.
- Native workflow QA now covers Settings provider setup, LabRun inspector,
  LabRun search, Execution inspector, context details, trace drawer, permission
  approval, final answer, and usage surfaces in a real Tauri window. The native
  smoke script now also stops pre-existing Priority Agent processes, activates
  the smoke window by process id, rejects visible `session not found`, and then
  captures the screenshot. The latest smoke log records
  `native_interaction_smoke ok=true` with `no-stale-session-error` in
  `apps/desktop/test-artifacts/native-app-desktop.log`, and the corresponding
  screenshot shows the Execution inspector without the stale-session error.
- Native real-provider smoke now supports explicit provider/model overrides and
  passes against both MiniMax and DeepSeek:
  `scripts/desktop-native-smoke.sh --live-provider --provider minimax --timeout 180 --no-screenshot`
  and
  `scripts/desktop-native-smoke.sh --live-provider --provider deepseek --timeout 180 --no-screenshot`.
  The latest per-provider evidence is in
  `apps/desktop/test-artifacts/native-live-provider-minimax-app-desktop.log`
  and
  `apps/desktop/test-artifacts/native-live-provider-deepseek-app-desktop.log`;
  both logs record the selected provider, real provider request completion,
  final answer, usage visibility, LabRun visibility, and Execution inspector
  visibility.
- Restart recovery smoke now passes on a real DeepSeek provider run via
  `scripts/desktop-native-smoke.sh --live-provider --provider deepseek --restart-check --timeout 180 --no-screenshot`.
  The latest evidence is in
  `apps/desktop/test-artifacts/native-live-provider-deepseek-restart-app-desktop.log`;
  it records the first real provider run, a second desktop startup using the
  same app data directory, and restored user message, assistant answer, and
  session metadata without `session not found`.
- Native multi-tool edit smoke now passes on a real DeepSeek provider run via
  `scripts/desktop-native-smoke.sh --live-provider --provider deepseek --multi-tool-check --timeout 240 --no-screenshot`.
  The latest evidence is in
  `apps/desktop/test-artifacts/native-multitool-deepseek-app-desktop.log`; it
  records Build mode, real provider tool calls, file read/edit execution,
  provider usage, verified closeout, and an isolated project file changed to
  the expected content. This slice also fixed the desktop mode path so
  non-Auto modes are passed into `RuntimeController` and bypass the lightweight
  direct-answer lane.
- Native two-turn soak smoke now passes on a real DeepSeek provider run via
  `scripts/desktop-native-smoke.sh --live-provider --provider deepseek --soak-check --timeout 420 --no-screenshot`.
  The latest evidence is in
  `apps/desktop/test-artifacts/native-soak-deepseek-app-desktop.log`; it records
  two consecutive Build-mode desktop turns through the packaged app's Tauri
  `send_message` path, four real tool executions, two verified closeouts,
  provider usage on both turns, and two isolated project files changed to the
  expected contents.
- Native two-turn soak restart smoke now passes on a real DeepSeek provider run
  via
  `scripts/desktop-native-smoke.sh --live-provider --provider deepseek --soak-check --restart-check --timeout 480 --no-screenshot`.
  The latest evidence is in
  `apps/desktop/test-artifacts/native-soak-deepseek-restart-app-desktop.log`;
  it records the same two-turn Build-mode desktop tool flow, restarts the
  packaged app against the same temporary home/project, and verifies restored
  session messages, restored UI text, and `desktop_file_preview` reads for both
  changed files.
- Native two-turn soak restart smoke now also passes on a real MiniMax provider
  run via
  `scripts/desktop-native-smoke.sh --live-provider --provider minimax --soak-check --restart-check --timeout 480 --no-screenshot`.
  The latest evidence is in
  `apps/desktop/test-artifacts/native-soak-minimax-app-desktop.log` and
  `apps/desktop/test-artifacts/native-soak-minimax-restart-app-desktop.log`;
  it records `agent_mode=build`, five tool executions, two verified closeouts,
  and restart recovery of session messages, restored UI text, and project file
  previews.
- Native three-turn extended soak restart smoke now passes on a real DeepSeek
  provider run via
  `scripts/desktop-native-smoke.sh --live-provider --provider deepseek --extended-soak-check --restart-check --timeout 720 --no-screenshot`.
  The latest evidence is in
  `apps/desktop/test-artifacts/native-extended-soak-deepseek-app-desktop.log`
  and
  `apps/desktop/test-artifacts/native-extended-soak-deepseek-restart-app-desktop.log`;
  it records `agent_mode=build`, six real tool executions across three
  consecutive Build-mode file-edit turns, three verified closeouts, per-turn
  hard `desktop_file_preview` checks including unchanged future-target checks,
  and restart recovery of session messages, restored UI text, and all three
  project file previews. This slice fixed the desktop smoke config path so
  extended soak applies `PRIORITY_AGENT_DESKTOP_SMOKE_AGENT_MODE=build`.
  Failed native smoke runs now keep the isolated HOME/project by default for
  post-failure DB/log/file inspection, and unattended live-provider smoke fails
  early when a provider enters `ask_user`.
- Native three-turn extended soak restart smoke now also passes on a real
  MiniMax provider run via
  `scripts/desktop-native-smoke.sh --live-provider --provider minimax --extended-soak-check --restart-check --timeout 720 --no-screenshot`.
  The latest evidence is in
  `apps/desktop/test-artifacts/native-extended-soak-minimax-app-desktop.log`
  and
  `apps/desktop/test-artifacts/native-extended-soak-minimax-restart-app-desktop.log`;
  it records `agent_mode=build`, six real tool executions across three
  consecutive Build-mode file-edit turns, three verified closeouts, all three
  project files changed to expected content, and restart recovery. The native
  smoke task contract was tightened so every QA turn explicitly requires
  read/write/cat tool evidence rather than accepting text-only completion.
- Native LabRun recovery/report smoke now passes without a live provider via
  `scripts/desktop-native-smoke.sh --lab-recovery-check --timeout 120 --no-screenshot`.
  The latest evidence is in
  `apps/desktop/test-artifacts/native-lab-recovery-app-desktop.log`; it prepares
  a real file-backed paused LabRun through the existing Lab command handler,
  then verifies `desktop_workbench_snapshot`, `desktop_lab_report_page`, the
  LabRun tab, artifact search, and the full report viewer in a packaged Tauri
  window.
- Native LabRun recovery restart smoke now passes without a live provider via
  `scripts/desktop-native-smoke.sh --lab-recovery-check --restart-check --timeout 150 --no-screenshot`.
  The latest evidence is in
  `apps/desktop/test-artifacts/native-lab-recovery-restart-app-desktop.log`; it
  starts the packaged app once to prepare and verify a paused LabRun, restarts
  against the same temporary home/project, and verifies the same
  `desktop_workbench_snapshot`, report page, LabRun tab, search, and full report
  viewer path again without re-preparing the project.
  After the `StartupStateCard.tsx` and `WorkspaceTopbar.tsx` shell split, the
  same native gate was rerun with a fresh packaged app using
  `scripts/desktop-native-smoke.sh --lab-recovery-check --restart-check --timeout 180 --no-screenshot`;
  the latest log records `snapshot-verified`, `report-page-verified`,
  `labrun-tab-open`, `report-preview-open`, and `full-report-visible` before and
  after restart.
  The native smoke wrapper now also writes those key diagnostic lines into the
  summary smoke logs (`native-lab-recovery-smoke.log` and
  `native-lab-recovery-restart-smoke.log`) instead of leaving them empty when
  the Tauri process itself has no stdout/stderr output.
- Desktop closeout validation was refreshed on 2026-06-22 against the current
  broad dirty tree: `cargo check -q` passed,
  `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -q` passed with
  38 desktop Tauri Rust tests, and a freshly rebuilt packaged app passed
  `scripts/desktop-native-smoke.sh --restart-check --lab-recovery-check --timeout 240 --no-screenshot`.
  The latest native evidence is in
  `apps/desktop/test-artifacts/native-lab-recovery-smoke.log` and
  `apps/desktop/test-artifacts/native-lab-recovery-app-desktop.log`.
- Desktop now defaults to DeepSeek v4 flash when no explicit desktop/user/env
  provider selection is set and DeepSeek is configured. Explicit
  `PRIORITY_AGENT_DEFAULT_PROVIDER`, saved desktop provider settings, and
  user-selected provider/model still win.
- Broad runtime regression now passes after the desktop API and native-smoke
  changes: `cargo test -q` reports 3109 main-crate tests passed, 1 ignored, and
  all follow-on integration/doc test batches passing. This closes the P4
  runtime-regression verification item in the desktop workbench plan.
- LabRun structured artifact body viewing now passes targeted backend and UI
  validation: `desktop_smoke_lab_status_reads_file_backed_labrun_state` covers
  `desktop_lab_artifact_body`, and `corepack pnpm --dir apps/desktop
  test:ui-smoke` covers the LabRun artifact body viewer in the inspector.
- A repeatable desktop release dogfood suite now passes from a freshly rebuilt
  packaged app plus `scripts/desktop-release-dogfood.sh --skip-build --timeout
  720 --repeat 2`. It runs the release-critical packaged-app checks in
  sequence: DeepSeek three-turn extended soak + restart, MiniMax three-turn
  extended soak + restart, and paused LabRun recovery/report/artifact UI +
  restart. The latest summary is
  `apps/desktop/test-artifacts/desktop-release-dogfood.log`; the current run
  records `PASS desktop_release_dogfood repeat=2`, with DeepSeek, MiniMax, and
  LabRun recovery PASS markers for both `iteration=1/2` and `iteration=2/2`.
  The wrapper supports `--repeat count` for unattended repeated release gates;
  the first iteration honors the build setting and later iterations reuse the
  packaged app. MiniMax initially exposed a third-turn no-tool failure; the
  native extended-soak harness now has a bounded third-turn repair task that
  still requires real `desktop_file_preview` file evidence before success, and
  the final MiniMax runs verified all three target files plus restart recovery.
- The current closeout dirty tree was rerun through the packaged-app release
  dogfood suite on 2026-06-22 local time with
  `scripts/desktop-release-dogfood.sh --skip-build --timeout 720 --repeat 1`.
  It passed DeepSeek extended soak + restart, MiniMax extended soak + restart,
  and LabRun recovery + restart. The summary is
  `apps/desktop/test-artifacts/desktop-release-dogfood.log`; current evidence
  logs include
  `apps/desktop/test-artifacts/native-extended-soak-deepseek-app-desktop.log`,
  `apps/desktop/test-artifacts/native-extended-soak-minimax-app-desktop.log`,
  and `apps/desktop/test-artifacts/native-lab-recovery-app-desktop.log`.
- Daily-use UI polish now has a regression guard for narrow screens: the mobile
  topbar and session header no longer overlap or clip the Output/Trace actions,
  the mobile workspace explicitly uses the first grid column, the bottom
  statusbar spans the viewport and scrolls horizontally so provider/cache/token/
  context/model/workspace status remains reachable, the mobile session metadata
  now shows the full provider/model value, and the inspector trace detail list
  wraps long permission/tool evidence instead of truncating it into unreadable
  one-line text. The mobile composer empty-context hint now wraps inside the
  composer card instead of truncating the guidance text, and the restored-session
  startup card now shows the full session/project detail on mobile. The bottom
  statusbar is now actionable: provider/API and model open Settings,
  cache/tokens/context open context details, and workspace opens file/project
  context. On narrow viewports those inspector actions now open the real Runtime
  inspector as a keyboard-managed drawer, reusing the same Context, Files,
  Execution, Subagents, LabRun, and Diagnostics tabs with separate DOM ids from
  the desktop inspector. Mobile command-palette inspector navigation uses the
  same drawer path, so taps always produce visible tab-level feedback. The drawer
  now has the same focus contract as the other desktop drawers: initial focus
  lands on Close, Tab/Shift+Tab stay inside, Escape closes, and focus returns to
  the trigger. The mobile session header mode switcher is also covered as a
  product-mode entry: Direct Agent starts selected, LabRun opens the Runtime
  inspector drawer directly on the LabRun tab with project controls, and Direct
  Agent can be selected again without relying on the hidden desktop inspector.
  The latest
  validation is `corepack pnpm --dir apps/desktop build`, the targeted mobile
  Playwright case, full
  `corepack pnpm --dir apps/desktop test:ui-smoke`, and `git diff --check`.
- Narrow viewport access is now less dependent on the hidden sidebar: the
  topbar has an explicit settings button for provider setup, permissions, and
  diagnostics, and the `More conversation actions` button now opens the command
  palette instead of being an inert control. The command palette now fits narrow
  viewports, clamps command labels and hint text inside the palette instead of
  letting long content widen the panel, and mobile smoke verifies `More
  conversation actions` -> `Command palette` -> `New Chat` as the sidebar-free
  primary navigation path. The narrow topbar Trace and Output actions also now
  expose expanded state and are covered as real drawer entries: mobile smoke
  opens each drawer, verifies the shared focus trap, closes with Escape, and
  checks focus returns to the triggering button. This path is covered by the
  desktop and mobile Playwright smoke assertions.
- Primary drawer routing is now mutually exclusive for daily-use panels:
  Settings, Workbench, Run Trace, Tool Output, and the Runtime inspector drawer
  use one opening path so main drawers do not stack over each other. Context
  Details remains a nested detail drawer. Mobile smoke asserts that only one
  primary drawer is mounted after opening Settings, Trace, Output, and Runtime
  inspector paths.
- Mobile Settings now has a stronger provider/permissions path: the provider key
  setup row uses responsive class-backed controls instead of inline layout,
  stacks provider select, API-key input, and save action on narrow screens, and
  smoke verifies Provider plus Permissions settings content stays inside the
  Settings drawer viewport.
- Settings now has the expected baseline keyboard flow for a desktop workbench:
  opening the drawer moves focus to `Back to app`, Tab/Shift+Tab stay inside
  the drawer instead of leaking into the background app, Escape closes it, and
  focus returns to the launcher button on both desktop sidebar and mobile topbar
  paths.
- The same drawer keyboard contract now applies to Workbench, Run Trace,
  Context Details, and Tool Output through a shared frontend hook. Nested
  drawers only handle keys while focus is inside the active drawer, so opening
  Context Details from Trace does not let one Escape close both layers.
- Runtime errors and watchdog warnings now render as an actionable alert near
  the composer instead of a bare text banner. The alert keeps the runtime/event
  as the source of truth, but gives the user direct actions to open the relevant
  trace, switch to Diagnostics, or dismiss the alert after reading it. The
  web-preview fixture includes a run-error path so this interaction is covered
  by Playwright smoke. Narrow/mobile smoke now covers the same recovery path:
  `Open trace` opens the visible Run Trace drawer, and `Diagnostics` opens the
  Runtime inspector drawer on the Diagnostics tab while preserving primary
  drawer exclusivity.
- The command palette is now a keyboard-first desktop command surface: `Ctrl+K`
  opens a focused combobox, result rows expose listbox/option semantics,
  ArrowUp/ArrowDown wrap through commands, Home/End jump to result boundaries,
  Enter runs the selected command, Escape closes the palette, Tab/Shift+Tab stay
  inside the dialog, and focus returns to the launcher. This keeps LabRun and
  Direct Agent commands on the normal composer/runtime route while making the
  high-frequency command path usable without the mouse.
- The Direct Agent goal progress row now has active-state preview coverage
  instead of only absent-state layout coverage. `previewFixture=goal` renders an
  active goal row, the icon controls expose explicit Edit/Pause/Clear accessible
  names, the objective editor is labelled, Escape cancels an unsaved edit draft,
  and smoke keeps the composer visible below the row.
- Command palette navigation now covers the main workbench surfaces directly:
  Workbench, Trace, Tool Output, Context, Files, Execution, Subagents, LabRun,
  and Diagnostics. These commands do not invent new runtime behavior; they only
  open existing drawers or switch existing inspector tabs, while LabRun commands
  continue to stage through the composer/runtime route.
- Shared drawer keyboard handling now covers a real nested-drawer edge case:
  after closing Context Details from Trace, Escape can still close the parent
  Trace drawer even if focus briefly falls back to the page; if focus is inside
  a different active overlay, the parent drawer still ignores Escape so one key
  press does not close multiple layers.
- Remaining desktop release risk: run longer unattended/background sessions
  beyond the current repeat-2 dogfood suite, repeat the same gate over time, and
  add any additional coding providers used for daily work.

### Validation gates for upcoming desktop work

```bash
corepack pnpm --dir apps/desktop build
corepack pnpm --dir apps/desktop test:ui-smoke
corepack pnpm --dir apps/desktop test:native-smoke
scripts/desktop-native-smoke.sh --live-provider --timeout 180
scripts/desktop-native-smoke.sh --live-provider --provider deepseek --timeout 180 --no-screenshot
scripts/desktop-native-smoke.sh --live-provider --provider deepseek --restart-check --timeout 180 --no-screenshot
scripts/desktop-native-smoke.sh --live-provider --provider deepseek --multi-tool-check --timeout 240 --no-screenshot
scripts/desktop-native-smoke.sh --live-provider --provider deepseek --soak-check --timeout 420 --no-screenshot
scripts/desktop-native-smoke.sh --live-provider --provider deepseek --soak-check --restart-check --timeout 480 --no-screenshot
scripts/desktop-native-smoke.sh --live-provider --provider deepseek --extended-soak-check --restart-check --timeout 720 --no-screenshot
scripts/desktop-native-smoke.sh --live-provider --provider minimax --extended-soak-check --restart-check --timeout 720 --no-screenshot
scripts/desktop-native-smoke.sh --live-provider --provider minimax --soak-check --restart-check --timeout 480 --no-screenshot
scripts/desktop-native-smoke.sh --lab-recovery-check --timeout 120 --no-screenshot
scripts/desktop-native-smoke.sh --lab-recovery-check --restart-check --timeout 150 --no-screenshot
scripts/desktop-release-dogfood.sh --skip-build
cargo fmt --check
cargo check --features experimental-api-server -q
cargo test -q
```

P1 validation completed:

```bash
corepack pnpm --dir apps/desktop build
corepack pnpm --dir apps/desktop test:ui-smoke
```

P2 slash-command, context-attachment, Execution inspector, Context inspector,
grouped run-card, and provider setup-repair validation completed with the same
desktop build and smoke gates.

P3 LabRun proposal/control/status/report/context surface validation completed
with:

```bash
corepack pnpm --dir apps/desktop build
corepack pnpm --dir apps/desktop test:ui-smoke
cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml -q
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -q desktop_smoke_lab_status_reads_file_backed_labrun_state
corepack pnpm --dir apps/desktop test:native-smoke
```

P3 paged LabRun report viewer validation completed with:

```bash
corepack pnpm --dir apps/desktop build
corepack pnpm --dir apps/desktop test:ui-smoke
cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml -q
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -q desktop_smoke_lab_status_reads_file_backed_labrun_state
```

Desktop state hook and run-event hook refactor validation completed with:

```bash
corepack pnpm --dir apps/desktop build
corepack pnpm --dir apps/desktop test:ui-smoke
cargo fmt --check
cargo check --features experimental-api-server -q
cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml -q
git diff --check
```

Inspector primitive split validation completed with:

```bash
corepack pnpm --dir apps/desktop build
corepack pnpm --dir apps/desktop test:ui-smoke
```

Desktop runtime API type/goal split validation completed with:

```bash
corepack pnpm --dir apps/desktop build
corepack pnpm --dir apps/desktop test:ui-smoke
```

Desktop Rust native-smoke split validation completed with:

```bash
cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml -q
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -q desktop_smoke_lab_status_reads_file_backed_labrun_state
scripts/desktop-native-smoke.sh --lab-recovery-check --restart-check --timeout 180 --no-screenshot
cargo fmt --check
```

## LabRun Graduate Execution Policy Alignment (2026-06-21)

LabRun graduate execution is being aligned with the policy discussion in
`docs/LAB_GRADUATE_EXECUTION_POLICY_DISCUSSION_2026-06-21.md`: provider identity
is diagnostic context, while task evidence and postdoc/professor artifacts remain
the workflow authority.

### What changed

- Lab graduate dispatch is provider-neutral: historical provider records now
  inform diagnostics, but provider/model names do not hard-block graduate task
  execution before evidence is produced.
- `/lab provider`, `/lab provider compare`, and tool diagnostics use
  diagnostic/evidence language instead of certification/unsupported execution
  language.
- Runtime tick and scheduler paths no longer silently create professor/postdoc
  placeholder artifacts to satisfy strategic or review gates.
- Deterministic professor review output is non-closeout-capable and creates
  revision work instead of auto-accepting based on fields.
- Runtime meeting recommendation surfaces are now runtime escalation signals,
  not professor intent.
- Mandatory professor-checkpoint signals now cover closeout, cycle boundaries,
  graduate-task/postdoc-acceptance intervals, high cost/context growth, and
  failure-budget exhaustion.
- Graduate dispatch records persist explicit cleanup state
  (`cleanup_pending`, `cleanup_done`, `cleanup_blocked`), and `/lab review`,
  `/lab dashboard`, and `/lab recovery` expose stale worktree cleanup state.
- Postdoc/professor prompts and handoff text now require escalation and steering
  to stay anchored to current blockers, evidence, changed files, validation
  results, cost/context signals, and the exact tradeoff under review.

### Validation gates used

```bash
cargo fmt --check
cargo check -q
cargo test -q provider_certification
cargo test -q graduate_dispatch_records_are_persisted_and_listable
cargo test -q task_worktree_command_merges_and_cleans_durable_task_id_worktree
```

## CLI Default Interface — Phases 0–6 Complete + P2 Cleanup (2026-06-16)

The scrollback-first CLI documented in `docs/CLI_COMPLETION_PLAN.md` is now the
default interactive interface. A follow-up P2 code-review cleanup pass shared
`/provider switch`, `/doctor`, and `/audit` between CLI and TUI, removed dead
code, hardened permission diff fallback, and added integration tests for local
command dispatch.

### Completed phases

| Phase | Theme | Commit |
|-------|-------|--------|
| Phase 0 | Decouple shared frontend components for CLI/TUI reuse | `002f4281` |
| Phase 1 | Split-footer prompt and scrollback-first renderer | `f7f19867` |
| Phase 2 | Attachments and Composer integration | `34960bf9` |
| Phase 3 | Permission diff previews and `@` mention file completion | `0b0ae1f2` |
| Phase 4 | Question UI, copyable code blocks, help polish | `3606846a` |
| Phase 5 | `ShellHost` trait and CLI slash command wrappers | `77725be9` |
| Phase 6 | Help/entry cleanup, provider text commands, dead-code removal | this change |
| P2 cleanup | Shared `/provider`, `/doctor`, `/audit`; diff fallback; tests | `5865fd09` |
| P3 | OutputTruncated continuation, `--no-footer`, shared `/status`/`/model`, stability | `265f5b9c` |

### What changed

- `priority-agent` / `pa` now defaults to the scrollback-first CLI.
- `--cli` is explicitly the default terminal interface; `--tui` is the legacy
  full-screen terminal interface (alternative).
- CLI supports streaming output, split-footer prompt, attachments,
  `@` file completion, permission diff previews, question UI, and the most
  common slash commands.
- Shared slash handlers are routed through `ShellHost`, implemented by both
  `CliHost` and `TuiApp`.
- `/provider switch` logic lives in `ProviderRegistry::switch_provider`, used by
  both CLI and TUI palette.
- `/doctor` and `/audit` live in `shell::slash` and are delegated by TUI handlers.
- `generate_unified_diff` now cleans up temp files and falls back to a pure-Rust
  line diff if `diff -u` is unavailable.
- Added `src/shell/test_support.rs` and integration tests for local command
  dispatch (`/help`, `/exit`, `/new`, `/clear`, `/model`, `/status`, attachments,
  unknown slash, plain message).
- P3 adds `OutputTruncated` continuation prompt with context summary,
  `--no-footer` CLI flag, shared `/status` and `/model` handlers, and main-loop
  error recovery for local commands and streaming turns.
- TUI remains available and unmodified as an alternative interface.

### Validation gates

```bash
cargo fmt --check
cargo check -q
cargo test -q
cargo clippy --all-targets --all-features -- -D warnings
cargo check --features experimental-api-server -q
bash scripts/workflow-production-gates.sh
```

### Known issues

- `cargo test -q` has 4 pre-existing TUI failures unrelated to CLI work.

---

## TUI Projection And Polish — Phases 1–6 Complete (2026-06-14)

The TUI projection-and-polish pass documented in
`docs/TUI_PROJECTION_AND_POLISH_NEXT_PLAN_2026-06-14.md` is complete.

### Completed phases

| Phase | Theme | Commit |
|-------|-------|--------|
| Phase 1 | Projection render model (`TuiRenderSession`) as timeline source of truth | `faeeb944` |
| Phase 2 | Real-provider soak readiness reporting (nightly manifest + report sections) | `1609c729` |
| Phase 3 | Composer parity (`ComposerState`/`ComposerPart`, focused `@` picker, fuzzy file filter) | `45245994` |
| Phase 4 | Session/workspace view model (`SessionListViewModel`, workspace status, restore hydration) | `0a171ae7` |
| Phase 5 | Plugin UI slot boundaries (active static vs deferred slots, panel.md warnings) | `689d5a5f` |
| Phase 6 | Polish and release readiness (snapshot tests, `PROJECT_STATUS.md` section) | this change |

### What changed

- Timeline rendering consumes a single `TuiRenderSession` produced from projection events.
- Tool rows are derived from message parts, not parallel TUI state.
- Composer has one fact source: `ComposerState` with `File`, `PastedText`, and `Image` parts.
- Session sidebar uses a pure `SessionListViewModel` with workspace grouping and status.
- Restoring a session hydrates projection parts before the timeline is inspected.
- Plugin TUI slots are classified as active static (`SidebarFooter`, `StatusBar`) or deferred (`SidebarTitle`, `MessageBeforeSend`, `ToolCard`); unsupported declarations fail safely.
- Visual text snapshots cover empty state, sidebar sessions, composer attachments, completed tool turn, and provider failure.
- Fixture TUI readiness passes 9/9 cases.

### Validation gates

```bash
cargo fmt --check
cargo check -q
cargo test -q
cargo clippy --all-targets --all-features -- -D warnings
cargo check --features experimental-api-server -q
bash scripts/tui_tool_turn_spine_readiness.sh  # 9/9 fixture cases passed
```

### Real-provider gate

Run only when API keys/network are intentionally available:

```bash
TUI_TOOL_TURN_SPINE_NIGHTLY_ROUNDS=1 bash scripts/tui_tool_turn_spine_nightly.sh
```

### Remaining

- (none in this plan)

## TUI Next Steps — Priorities 1–7 In Progress (2026-06-14)

See `docs/TUI_NEXT_STEPS_PLAN_2026-06-14.md` for the full gap analysis.

### Completed

- **Priority 1**: Inline collapsible tool bodies and assistant text parts (`90e5b310`).
- **Priority 3**: Leader-key pending state in status bar (`59e5751f`).
- **Priority 2**: Composer attachment pills and `@` file autocomplete (`cb4995d6`).
- **Priority 7**: Multi-select file picker (`5b855dfc`).
- **Priority 5**: Workspace switcher and durable workspace metadata (`291a60a6`).
- **Priority 6**: Per-session UI state cache, `/back`, and leader+g cycling (`e0aaf8da`).
- **Priority 4**: Static plugin UI slot rendering.

### Remaining

- (none)

## Priority 4 — Static Plugin UI Slot Rendering (2026-06-14)

Implemented static plugin UI slots for safe display-only surfaces:

- Added `PluginUiSlotContent` runtime content type in `src/plugins/mod.rs`.
- Plugins declare TUI slots in `plugin.toml` under `[tui] slots` and may provide
  `panel.md` with TOML frontmatter (`slot`, `title`) plus markdown body.
- Only `SidebarFooter` and `StatusBar` slots are rendered; `MessageBeforeSend`,
  `ToolCard`, and `SidebarTitle` are explicitly deferred.
- `TuiApp` discovers plugin facts on startup and stores static contributions in
  `plugin_ui_contributions`.
- `SidebarFooter` contributions render inside the context panel.
- `StatusBar` contributions append as `plugin:<text>` footer items.
- New `/plugins` slash command lists discovered plugins, status, declared slots,
  active static slots, and deferred slots.
- Added unit tests for `load_static_ui_contributions` covering panel.md parsing,
  unsupported-slot filtering, and fallback without panel.md.

Key source files:

```
src/plugins/mod.rs                     — slot types, panel.md loader, tests
src/tui/app.rs                         — plugin discovery on startup
src/tui/screens/main_screen.rs         — SidebarFooter rendering
src/tui/view_model/footer.rs           — StatusBar slot footer items
src/tui/slash_handler/observability.rs — /plugins command
src/tui/app/slash_commands.rs          — /plugins dispatch
```

## Codex Goal Mode — Implemented (2026-06-11)

Codex-style durable goal mode landed in 8 commits across 7 phases. See
`docs/CODEX_GOAL_MODE_ALIGNMENT_PLAN_2026-06-11.md` for the full plan.

### What was built

- **`/goal <objective>`**: starts a persistent goal that drives multiple
  full-agent turns until completed, paused, blocked, failed, or needs user.
- **`GoalRunner`**: deterministic outer scheduler that invokes
  `RuntimeController` per turn, extracts closeout/verification evidence, runs
  `GoalDecisionEngine`, and decides whether to continue.
- **`GoalDecisionEngine`**: screens turn-level evidence (closeout status,
  verification proof, permission, blocker, budget) against hard rules to produce
  `Complete`/`Continue`/`Pause`/`Blocked`/`Failed`/`NeedsUser` decisions.
- **Durable persistence**: `goal_runs` and `goal_steps` tables (v17/v18
  migrations) with full CRUD on `SessionStore`.
- **`/goal pause`/`resume`/`clear`/`edit`/`log`**: full lifecycle management.
- **`/quick`** and **`/active-task`**: show goal runner status (turn count,
  budget, decision, blocker, proof).
- **Desktop goal progress row**: compact inline controls above the composer
  (objective, status, turns, pause/resume/edit/clear buttons). Backed by 7
  Tauri commands (`goal_status`, `goal_start`, `goal_pause`, etc.).
- **Scored eval config**: optional `ScoredEvalConfig` on `GoalStopRules` with
  command, parser, threshold, and max attempts. Score tracks in `GoalStep`.
- **Export**: goal summary with steps, decisions, scores, and blockers in
  session export JSON.
- **Restart safety**: active goals are paused on startup; user must
  `/goal resume` to continue.
- **Steer/queue**: while a goal is active, user messages are persisted with
  `InputDelivery::Steer` so they interrupt and redirect the current turn.

### Key source files

```
src/engine/goal/
  mod.rs        — module root
  model.rs      — GoalRun, GoalStep, GoalBudget, GoalStopRules, ScoredEvalConfig
  decision.rs   — GoalDecisionEngine (deterministic evidence screening)
  runner.rs     — GoalRunner (outer scheduler)
src/migrations/
  v17_add_goal_runs.rs
  v18_add_goal_step_score.rs
src/session_store/goal_store.rs   — CRUD methods
```

### Validation gates

```bash
cargo test -q goal --lib          # 47 tests (decision + model)
cargo test -q session_goal --lib  # 6 tests
cargo test -q session_store --lib # 51 tests
cargo test -q                     # 2535+ tests, 0 failures
```

### Outstanding

- Scored eval command execution is not yet wired into the runner loop
  (score extraction currently reads from trace verification events).
- Desktop progress row only has web-preview stubs; full interactive testing
  requires a Tauri dev server session.

## 2026-06-09 Stabilization And Docs Audit

The latest local stabilization commit is `90cd9e11`
(`fix: stabilize history restore and CI gates`). It fixed these current
runtime/test risks:

- `StreamingQueryEngine` is the single owner for persisted chat history restore;
  `ConversationLoop` no longer performs a second SQLite restore.
- Selective message compression defaults back to enabled, preserving the
  runtime-diet path where compaction can happen by message role/content instead
  of only by hard collapse.
- CI lint parity now uses
  `cargo clippy --all-targets --all-features -- -D warnings`.
- Full `cargo test -q` is clean after isolating memory/progress state with
  `PRIORITY_AGENT_MEMORY_ROOT`,
  `PRIORITY_AGENT_MEMORY_PROPOSALS_PATH`, and
  `PRIORITY_AGENT_PROJECT_PROGRESS_PATH` instead of mutating `HOME`.
- `src/tools/file_tool/tests.rs::test_file_write_and_read` now uses an isolated
  temp path and unique session id.

Latest verified local gates:

```bash
cargo fmt --check
cargo check -q
cargo clippy --all-targets --all-features -- -D warnings
git diff --check
cargo test -q
```

Docs/structure audit result:

- Root entry docs have been refreshed around current provider order, validation
  gates, prompt-injected `AGENTS.md`, and compact Claude Code compatibility
  guidance.
- `docs/README.md` is the docs navigation index. The docs tree is intentionally
  dense but partitioned into current, active-note, reference, archive,
  workflow, generated, and proposal-asset areas.
- The top-level source layout is still coherent: `src/`, `apps/`, `tests/`,
  `scripts/`, `docs/`, `evalsets/`, `fixtures/`, and `priority-core/` each have
  a clear role. Generated local artifacts such as `.priority-agent/`,
  `target/`, `__pycache__/`, and `.DS_Store` should stay ignored/cleaned rather
  than reorganized into tracked source.

## Opencode Alignment — Implemented, Hardening In Progress

The implementation from `docs/OPENCODE_AGENT_ENGINE_ALIGNMENT_PLAN_2026-06-06.md`
has landed. Follow-up hardening on 2026-06-06 fixed usage-ledger rebuild
idempotency, checkpoint test isolation, `/diagnostic` session snapshots, and
dynamic-zone counting for user-message injected context. Provider runtime
metadata now flows from the request controller into usage ledger entries for
successful requests with usage.

### Tool & Permission Hardening (Phase A)
- 26 tools: explicit ToolOperationKind overrides
- `test_all_tools_have_correct_tool_family`: full registry semantic check
- FileMutationResult `formatted` field + consistency tests
- TUI permission diff: file_patch/format + bash mutation warnings
- `/permissions explain`: match keys + matched rules (pattern+source)

### TUI Daily Ops (Phase B)
- `/cost`: cache diagnostic with DynamicZoneTier breakdown
- `/changes`: per-round file changes with (+adds/-dels)
- `/validate`: validation summary with file changes and tool rounds

### Provider & Usage (Phase C)
- UsageLedgerEntry: 8 runtime fields + SQLite migration
- Provider family, latency, finish reason, and retry count are populated on
  successful main-loop provider requests when usage is available
- Progressive output cap: 8192→4096→2048→1024 for consecutive repairs
- `/diagnostic`: session snapshot export (run_report.v1) with usage totals,
  changed files, tool rounds, failed tools, provider latency, and validation
  status inferred from recorded TUI tool runs

### Cache & Evidence (Phase D)
- DynamicZoneTier: stable-prefix/last-user/repair-only classification
- Dynamic-zone counts include dynamic context prepended to the last user message
- EvidenceCategory: tool/validation/diagnostic/user
- CacheMissReason detail includes dynamic zone breakdown

### Test Infra & Docs (Phase E)
- RunReport schema (run_report.v1) + /diagnostic export
- FailureOwner::category() → Framework/ProviderModel/Harness/Environment
- Route narrowing: CodeChange/BugFix file tools prioritized
- Bash write risk gate: WorkflowFallback trace on bash-only mutations
- `/memory status/review/files` productized
- TUI tool cards with [Edit]/[Shell]/[Read]/[Search]/[Task] labels
- Permission approval panel: 4-zone display
- Turn-level additions/deletions from stored diffs
- `fixtures/TASK_MATRIX.md` + `docs/CONTROLLER_INDEX.md` (52 controllers)
- Desktop + TUI share same RuntimeFacade

### Gates
Latest targeted gates from the 2026-06-06 hardening pass:
`cargo fmt --check`, `cargo check -q`, `cargo test -q cost_tracker --lib`,
`cargo test -q checkpoint --lib`, `cargo test -q cache_stability --lib`, and
`cargo test -q tui --lib` passed.

2026-06-05 opencode programming-chain gap fixes landed:
- File-edit deterministic recovery is now product-wired, not test-only. Exact
  replace failures can surface structured candidates, unique multi-line
  stale-safe candidates can auto-apply when replacement count matches, and
  ambiguous or single-line whitespace matches remain diagnostics instead of
  silent fuzzy edits.
- Provider cost control now applies explicit output caps across main coding,
  non-streaming fallback, reactive compaction retry, query-simple, and LLM
  compaction summary paths. Usage ledger entries record request phase,
  effective output cap, tool schema tokens, tool round count, and compaction
  decision metadata so `/cost` can distinguish prompt/cache/completion/schema
  and capped-request behavior.
- Session todos are durable and transactional through `SessionStore`; failed
  todo persistence now returns a tool error instead of a successful response
  with hidden persistence failure text.
- Phase 4 structural shell parser remains intentionally deferred after risk
  review; the existing classifier/checkpoint contract stays authoritative until
  an opt-in parser enrichment has a command corpus and fail-closed tests.
- Targeted gates passed: `cargo fmt --check`, `cargo check -q`,
  `cargo check --features experimental-api-server -q`,
  `cargo clippy --all-features -- -D warnings`,
  `cargo test -q edit_match`, `cargo test -q file_tool`,
  `cargo test -q todo_store`, `cargo test -q usage_ledger`,
  `cargo test -q route_scoped_tools`, and
  `cargo test -q context_compressor`.

2026-06-04 Reasonix alignment Phases 0-7 complete:
- Phase 0: Runtime boundary audit documented in PROJECT_MAP.md
- Phase 1: RuntimeController (src/engine/runtime_controller.rs) + DesktopRuntime/TUI full-agent paths wired
- Phase 2: Cache-stability tests 15→27 (all Phase 2 scenarios covered)
- Phase 3: Route-scoped tool tests 14→18 + snapshot regression guards
- Phase 4: Memory diagnostics in /doctor panel (standing/saved/recall)
- Phase 5: Permission/checkpoint tests 44→51 + 7→9
- Phase 6: Event parity includes runtime diagnostic, usage, permission, and closeout events across StreamEvent/TurnEvent/DesktopRunEvent
- Phase 7: scripts/daily-baseline.sh created (17 gate deterministic baseline)
- No normal production file exceeds 1500 lines without documented exception.
  Full lib test gate: 2244 passed, 0 failed, 1 ignored.
- Release gate spot-check after findings fixes:
  `cargo check --all-features -q`, `cargo clippy --all-features -- -D warnings`,
  and `cargo test -q --all-features -- --test-threads=1` passed.
- Phase 8 (code size stewardship) remains active; watch-list files tracked
  in daily-baseline file-size-report gate.

## Summary

Priority Agent is now an interactive coding CLI with a stateful runtime spine:
intent routing, turn traces, session goals, memory, permissions, recovery plans,
MCP health, CLI observability panels, and required validation closeout.
The active runtime work is keeping the shared CLI/TUI/desktop runtime simple:
hard limits, permissions, tool contracts, evidence, validation proof, and
closeout gates stay deterministic, while the LLM owns semantic and engineering
judgment.

2026-06-04: Completed Phase 0 runtime boundary audit (Reasonix alignment).
Documented the frontend-to-engine command/event map in `docs/PROJECT_MAP.md#runtime-boundary`.
Full-agent desktop and TUI paths now enter through `RuntimeController`, while
desktop lightweight turn remains explicitly non-agent; `classify_turn_ingress()`
is runtime-owned. Closeout status is now a first-class stream/desktop event
instead of only trace-derived state.

Product direction: Priority Agent should stay narrow, deep, personal, and
verifiable rather than chasing broad generic-agent parity. The durable goal is
to be the programming assistant that best understands gex's machine, projects,
habits, validation loops, and local workflow. See
`docs/PERSONAL_AGENT_PRODUCT_PRINCIPLES_2026-05-18.md`.

## Memory System Audit (2026-06-18)

A systematic audit of `src/memory/` and its runtime integration
(`docs/MEMORY_SYSTEM_AUDIT_2026-06-17.md`) confirmed the following current
state:

- The default closeout behavior is **review-first**, not silent auto-memory.
- Automatic memory write is available through
  `PRIORITY_AGENT_AUTO_MEMORY_WRITE=legacy` or `=narrow`, but the default
  `review_only` policy surfaces proposals for manual review via
  `/memory-proposals` instead of silently writing to long-term memory.
- Active memory recall is opt-in (`PRIORITY_AGENT_ACTIVE_MEMORY=true`).
- Closeout background review and memory nudge both generate review-required
  proposals and do not auto-persist long-term memory by default.
- Read paths (snapshot injection, dynamic recall, prefetch) and manual tool
  paths (`memory_save`, `memory_load`) are fully wired.
- Three minor cleanup items were completed (dead async function removed,
  duplicate constant unified, visibility tightened), and the nudge background
  review path now preserves the proposal-only default boundary. Three
  lower-priority items (file lock consolidation, helper dedup, eval split) are
  deferred to opportunistic refactoring passes.
- `src/agent/memory.rs` is intentionally separate from `src/memory/` — the
  former is a sub-agent KV store, the latter is long-term user/project memory.

Documents:
- `docs/MEMORY_SYSTEM_AUDIT_2026-06-17.md`
- `docs/CONTEXT_INJECTION_AUDIT_2026-06-17.md`
- `docs/CACHE_COMPRESSION_AUDIT_2026-06-17.md`
- `docs/AUTOMATION_AUDIT_2026-06-17.md`
- `docs/OPENCODE_COMPARISON_2026-06-17.md`

Current stage:

- The latest runtime-diet pass is implemented in commit `4430647b` and recorded
  in `docs/RUNTIME_DIET_UPDATE_2026-06-02.md`. It removed duplicate read-only
  runtime closeout, cached read-result substitution, directory-read
  redirection, workflow-specific non-tool double-tap finishing, and
  score/advisory-driven `StopChecker` branches. The remaining loop contract is
  simpler: a valid no-tool assistant response ends the turn; empty responses get
  a bounded retry; exact repeated tool storms are handled by the shared storm
  guard; the iteration budget and force-summary remain the runaway-loop safety
  net; verification, permissions, rollback, model-output protocol repair, and
  destructive-scope checks remain hard constraints.
- OpenClaw/Hermes reference implementation has started from
  `docs/OPENCLAW_HERMES_REFERENCE_AUDIT_2026-05-26.md`. Phase 1 is implemented:
  the instruction loader now supports optional project-root `SOUL.md`,
  `USER.md`, and `TOOLS.md` as compact supplemental context after `AGENTS.md`.
  `AGENTS.md` remains the runtime-policy source; supplemental files are labelled
  persona/user/tool context only and cannot override runtime, sandbox,
  permission, validation, checkpoint, or tool-safety rules. Prompt-context
  reports now surface these root context layers, and
  `docs/SOUL_USER_TOOLS_CONTEXT.md` documents their intended use. Phase 2 has
  also started: turn-level retrieval context is now injected as an explicit
  `<relevant_material>` zone before the current user request, and
  context-zone tracing excludes dynamic zone system messages from the stable
  prefix fingerprint. Phase 3 has begun with a `MemoryProviderRegistry` owned by
  `MemoryManager`, structured provider lifecycle outcomes, local plus optional
  single external provider registration, and fanout wrappers for initialize,
  prompt blocks, prefetch, queued prefetch, turn sync, session end,
  pre-compress, write notifications, and shutdown. Local provider extraction now
  covers safe snapshot prompt blocks, scope-filtered typed-record prefetch and
  search, and idempotent typed-record write notifications, with `MemoryManager`
  registering a base-bound `LocalMemoryProvider`. Phase 4 has started by adding
  an active `MemoryScope` to `MemoryManager` and setting it from the conversation
  session id and working directory during turn bootstrap; turn-level LLM
  extraction, forked background extraction, and trailing session extraction now
  inherit that active scope, trailing runs notify memory providers through
  `on_session_end`, async memory writes notify providers through idempotent
  `on_memory_write`, and streaming flush paths without a persistent session id
  skip memory writes instead of falling back to `unbound-session`. Phase 5 has
  started by applying the memory
  safety scanner on persisted load paths: unsafe `MEMORY.md`, `USER.md`, topic
  memory files, and typed records are skipped during snapshot/retrieval loading
  instead of being injected as background context. Phase 6 has started with a
  rebuildable local SQLite FTS5 index at `memory/search.sqlite`; it indexes safe
  project/user/topic memory plus accepted typed records and feeds hits into the
  existing memory retrieval ranking while keeping Markdown and `records.jsonl`
  as canonical storage. Phase 7 has started by making skills a safer procedural
  memory surface: workspace `.agents/skills` now wins over workspace `skills`,
  user-configured roots load after workspace roots, bundled skills remain the
  fallback, third-party and URL-loaded skills pass through a scanner and
  optional `PRIORITY_AGENT_SKILL_ALLOWLIST`, loaded skills carry source/trust
  metadata, and `docs/SKILL_ROOTS_AND_TRUST.md` documents the trust model. Phase
  8 now has an opt-in active-memory prototype: `PRIORITY_AGENT_ACTIVE_MEMORY=1`
  enables a gated, read-only local FTS worker only for user-facing persistent
  sessions, skips eval/headless/automation/internal paths, fences output as
  untrusted retrieval context, records `memory.active` trace events, and is
  documented in `docs/ACTIVE_MEMORY_PROTOTYPE.md`. Closeout-time and lifecycle
  memory writes now default to review-only: the runtime surfaces
  `MemoryProposal`, skipped-review-only flush records, and execution/progress
  evidence, while legacy automatic persistence requires
  `PRIORITY_AGENT_AUTO_MEMORY_WRITE=legacy`; the
  narrower `PRIORITY_AGENT_AUTO_MEMORY_WRITE=narrow` policy only auto-persists
  explicit user preference statements during turn closeout. The memory doctor
  panel now exposes provider lifecycle state (`initialize`, prompt block,
  prefetch/search, turn/session sync, pre-compress, write notification, and
  shutdown) with provider kind/availability and active scope. `MemoryManager`
  now delegates local typed-record reads to the base-bound `LocalMemoryProvider`
  instead of owning the JSONL implementation directly. Memory proposals are
  persisted into a review queue and can be inspected or moved through
  `list/show/accept/reject/apply` with `/memory-proposals`. `/active-task`
  exposes one combined progress surface from goal state, workflow trace,
  verification/closeout, and memory proposal evidence. Controlled self-evolution
  now uses the explicit `proposal -> eval -> accept/apply -> rollback` loop for
  improvement proposals, with `/evolution status` summarizing improvement and
  skill evolution state; improvement proposals can also be bound to named
  evalsets before apply.
- `docs/LLM_RUNTIME_SIMPLIFICATION_PLAN_2026-05-08.md` is complete through its
  original follow-up implementation phases and now has a 2026-06-02 addendum
  for the Reasonix-style simplification pass. Future runtime-diet work should
  remove or downgrade pseudo-intelligent runtime branches before adding new
  ones.
- `docs/AGENT_LEARNING_NOTES_PROJECT_ALIGNMENT_2026-05-24.md` is the current
  implementation record for the agent-design notes review. The active slice has
  landed five-zone context assembly, `AgentTaskState`, phase-aware tool
  exposure, action scoring, verification proof semantics, stop checks, expanded
  context-ledger evidence, state-backed goal drift, direct-task regressions,
  edit/repair snapshots, a control-loop diagnostic in `/trace`, and a
  cross-module runtime-spine behavior regression covering context-zone order,
  phase transition, action scoring, stop check, and proof semantics together.
  The live-eval/reporting harness now consumes those runtime-spine signals:
  reports expose phase coverage, required trace-event assertions, proof status,
  missing runtime-spine assertions, and aggregate pass/fail coverage, with
  representative core live tasks carrying explicit `runtime_spine_assertions`.
  The desktop app now receives the same runtime spine through a stable
  `runtime_diagnostic` run event and renders task state, verification proof,
  and control-loop coverage in the run summary and trace drawer without making
  the UI part of the core algorithm. `DesktopRuntime` is the desktop-facing
  facade over the same `StreamingQueryEngine` used by CLI/TUI/headless
  eval-run, and `scripts/agent-runtime-dogfood.sh` is now the fast first check
  for complex full-runtime behavior before desktop packaging. The light/full
  routing gap now has a runtime-owned scoring surface as well: `TaskModeScore`
  records complexity, risk, uncertainty, tool need, and user impact, and
  `LightweightPlan` gives tool-assisted light turns bounded
  scope/observe/respond steps without invoking the heavy workflow contract.
- `docs/AGENT_MINIMUM_VIABLE_ARCHITECTURE_ALIGNMENT_PLAN_2026-05-25.md` is
  implemented as a thin contract layer over the existing runtime spine rather
  than as a second agent loop. The runtime now emits `AgentLoopStepEvaluated`,
  `ContextZonesMaterialized`, and `CompletionContractEvaluated` diagnostics,
  keeps explicit stage-transition history in `AgentTaskState`, supports
  shadow/gated candidate-action ranking, exposes an opt-in MVA audit tool
  profile, and adds `minimum-agent-*` live-eval coverage plus parser and
  aggregate-report metrics for these signals. The follow-up plan in
  `docs/AGENT_MINIMUM_VIABLE_ARCHITECTURE_FOLLOWUP_PLAN_2026-05-25.md` is also
  implemented: MVA live evals now activate the minimum-viable runtime profile,
  report candidate-action calibration, context-zone budgets, MVA state
  snapshots, observer/memory-boundary signals, and exact completion/proof
  assertions. The current seven-case MVA baseline has passing targeted
  `agent-run` evidence, with the latest low-value replan and memory-boundary
  checks from run `live-eval-20260525-145541`.
- `docs/AGENT_EVALUATION_OBSERVABILITY_MVP_FOLLOWUP_PLAN_2026-05-25.md` is
  implemented for the evaluation/reporting layer. Live-eval reports now expose
  deterministic `outcome_score`, `process_score`, `efficiency_score`, and
  `agent_score`; summaries aggregate invalid actions, premature edits, scope
  drift, repeated actions, and failed actions; `minimum-agent-*` tasks carry
  semantic output and trajectory assertions; reports export a redacted
  `run-bundle/` with `task.json`, `steps.jsonl`, `events.jsonl`, and
  `final_report.md`; and `scripts/live-eval-ab-compare.sh` can compare
  baseline vs weighted profiles on the `mvp-weighted-agent` suite. The first
  real A/B run (`ab-20260525-155452-baseline` vs
  `ab-20260525-155452-weighted`) showed weighted process-score improvement
  without outcome improvement. Follow-up hardening fixed the assertion/reporting
  false negatives for high-risk block and loop, and improved stale/missing
  `old_string` edit-repair guidance. An earlier bounded duplicate read-only
  closeout path was removed by the 2026-06-02 runtime diet; exact duplicate
  tool loops now belong to the shared storm guard and iteration budget. Latest
  full
  weighted suite: `mva-followup-full-20260525-165257`, `6/7` passed with
  average `agent_score=87.1`; the remaining `minimum-agent-verification-repair`
  failure is a model/task-completion failure with honest `not_verified`
  closeout and failed required commands, not a false-success verification path.
  `docs/LLM_FLOW_FAILURE_AUDIT_2026-05-25.md` records the follow-up split
  between model variability and flow defects; it also fixed a prompt-derived
  tool-policy over-block where forbidding `file_write`/`file_patch` could be
  misread as forbidding explicitly allowed `file_edit`. The targeted rerun
  `mva-prompt-policy-verification-20260525-llm-flow-audit` passed with required
  validation proof and no code-write-forbidden fallback.
- `docs/NEXT_AGENT_CORE_CODING_QUALITY_PLAN_2026-05-11.md` is complete for its
  current Phase 1-4 scope: main-loop splitting reached the first line-count
  target, terminal and file-quality contracts are in place, and the real
  `core-coding-quality` rerun is current.
- `docs/CONVERSATION_LOOP_RESPONSIBILITY_MAP_2026-05-11.md` remains the
  ownership map for future loop work, but the next step is no longer more
  low-level `run_inner` extraction by default.
- `docs/AGENT_PRODUCTIZATION_REFERENCE_AUDIT_2026-05-10.md` remains the
  reference map for product maturity work against Claude Code and opencode.
- Current implementation focus has moved to product maturity: broader
  trace-backed evaluation, behavior-level memory/skill assertions, long-running
  terminal UX, CLI polish, and external baseline data.
- The next product-maturity slice is now the real-project coding gauntlet:
  `docs/REAL_PROJECT_CODING_GAUNTLET_PLAN_2026-05-17.md`. The live-eval runner
  supports `--case real-project-coding`, and live summaries now include a
  coding-gauntlet evidence section for agent-run tasks without changing the
  existing task matrix format. The first 15-task gauntlet run exposed two
  memory/product-maturity failures, and both have passing targeted reruns with
  behavior assertions and required validation now green. The first generic
  repair-planner slice now appends source snippets from failed required
  validation output, with targeted backend and frontend repair reruns passing.
- The first provider-protocol regression matrix slice is complete:
  OpenAI-compatible, MiniMax, and Kimi request conversion now share provider
  message normalization; pure assistant `tool_calls` omit empty content,
  orphan/aborted tool results are dropped before provider requests, MiniMax
  keeps system-message merging, and provider 400s that mention tool-result
  ordering are classified as `provider_protocol` instead of generic unknown
  failures.
- Workflow contract targeting is now implemented. `PRIORITY_AGENT_WORKFLOW_CONTRACT`
  supports `off`, `auto`, and `force`; unset defaults to `auto`. Entry workflow
  judgment is skipped for ordinary medium feature work, but remains active when
  `RiskSignalController` sees concrete high-risk runtime, provider, memory,
  tool-execution, permission, config/schema, required-validation, or acceptance
  signals; dynamic validation/tool failures now record `risk.signal` trace
  events. See `docs/WORKFLOW_CONTRACT_TARGETING_PLAN_2026-05-18.md` and
  `docs/RISK_SIGNAL_CONTROLLER_PLAN_2026-05-18.md`.
- Claude Code parity implementation is current through the first Phase 11
  provider/bridge slice, with Phase 6 product-hardening resumed: permission
  approvals now return a structured `ToolApprovalResponse` so trace and runtime
  consumers can distinguish approve-once, session/project/global allow, and
  global deny decisions with rule persistence evidence. Hook runtime visibility
  now has a structured lifecycle surface as well: `HookLifecycleSnapshot`
  reports configured hook registrations, provider/event/scope, timeout and
  fail-open/fail-closed policy, recent execution statistics, `/hooks`, and
  `/panel hooks`. `/doctor` now includes a product readiness summary plus
  machine-readable `product_ready` metadata derived from diagnostics and runtime
  selectors for failed tools, backgrounded tools, pending approvals, and MCP
  repair hints. Placeholder commands are also gated from default help and empty
  command-palette results, while explicit search still exposes them and inserts
  rather than executes. `/panel all` now also includes stable agent and trace
  panels for profile definitions, running agents, durable task states, artifacts,
  recent traces, and replay entrypoints. Phase 7 has started on subagent
  reliability: code-change agent definitions and built-in mutating profiles now
  default to `isolated_worktree_fork`/`isolated_write`, and the `agent` tool also
  infers isolated worktree context whenever the resolved tool surface includes
  mutating file tools. Spawned subagents that fail or time out while waiting for
  results now update durable task state to `failed`/`timed_out` instead of
  remaining `running`, while preserving cleanup metadata; `agent_id` plus
  `action=cancel` now stops a running subagent and marks durable state
  `cancelled`, and `action=read` loads durable task state plus persisted result
  artifacts after in-memory manager results are gone. `action=list` now reports
  active in-memory agents plus recent durable task states without requiring a
  specific `agent_id`, and durable `action=read` no longer needs an
  `AgentManager`. Agent worktree safety guards now have direct coverage for
  dirty status, untracked paths, safe agent branch deletion, rejection of
  non-isolated task records, and merge-conflict recovery messaging for
  branch/diff merges. Worktree manager git calls now run from the
  captured repo root, porcelain worktree paths are parsed correctly, internal
  `.claude/worktrees/` storage no longer blocks parent merge cleanliness checks,
  and a real temporary-git-repo flow covers agent review, tracked-diff merge,
  cleanup skip, forced cleanup, and safe branch deletion. Hook and permission
  failures now share the recovery spine: failed/blocked hooks emit `/hooks`
  recovery plans, pre-tool hook blocks are classified as hook runtime failures,
  and permission denials emit
  `/permissions explain` recovery plans in trace.
- Phase 8 has started on long-session coherence. Runtime compaction records and
  `ContextCompacted` trace events now expose structured trigger, token pressure,
  strategy, compact boundary, preserved tail count, and retained-item facts for
  preflight and reactive context-error compaction. Retained facts include
  head/tail preservation, recent tool results, sanitized tool-call pairs,
  compact boundaries, and session-memory signals. The `context` tool now has an
  `action=explain` path that reports retained memory/retrieval and skill
  inclusion reasons with provenance, trust, conflict status, and token
  estimates. Background heuristic and LLM memory extraction now share one
  strict write-gate decision path before appending bullets, with source, status,
  quality score, duplicate status, write outcome, and reason captured in the
  decision. Skill matching now produces compact activation evidence for matched
  keywords and fields, and `skills_list action=explain` reports why a skill
  matched without loading full skill bodies into the prompt.
- Phase 12 verification work has completed its local deterministic replay and
  external-baseline ingestion surfaces: `src/engine/scenario_matrix.rs`
  declares the six required parity scenarios and maps each one to concrete
  runtime/trace/recovery evidence, with `/eval matrix` exposing the current
  readout. All six local deterministic replay fixtures are now replay-ready:
  `file_edit_rewind`,
  `bash_background_task`, `permission_denial_retry`, `compaction_boundary`,
  `subagent_worktree_worker`, and `mcp_auth_repair`. Deterministic eval replay
  can emit permission
  request/resolution and recovery-plan trace events, terminal task records for
  background shell handles, `bash_output`, and `bash_cancel` paths,
  file-change/checkpoint records with rewind restore assertions,
  `ContextCompacted` and `RuntimeDietReport` records with boundary and budget
  assertions, plus subagent task-state and isolated worktree review, merge, and
  cleanup records, plus MCP resource access and MCP repair/retry records.
  `evalsets/coding_replay_matrix.yaml` includes
  `file-edit-rewind-checkpoint`, `bash-background-task-handle`,
  `permission-denial-retry-recovery`, upgraded `context-compaction-safe`, and
  `subagent-worktree-worker-review-merge`, and
  `mcp-auth-repair-approval-retry`. External Claude/Codex baselines are still
  pending as real evidence artifacts, but the import/validate/compare path is
  now in place: `/eval baseline
  [provider|all]` loads YAML/JSON files from `evalsets/external_baselines/`
  and reports provider coverage, pass/fail/blocked/not-run counts, missing
  scenario ids, and per-scenario evidence metadata. `/eval baseline-template
  <provider> [model]` and `/eval baseline-write <provider> [model]` generate
  complete `not_run` templates for the six scenario ids, and
  `scripts/external-baseline-artifact.py` creates a Markdown run-record
  skeleton under `target/external-runs/` for real external-agent transcripts,
  including per-scenario run cards and minimum evidence notes.

The recent closure plan is complete:

| Area | Status | Commit |
|------|--------|--------|
| Tool failure recovery and tool outcome learning | Complete | `fd74714` |
| Learning-driven tool selection | Complete | `44b4250` |
| Goal drift visibility | Complete | `bd12f64` |
| Memory namespace search and conflict hints | Complete | `934f7fe` |
| MCP health-aware visibility and resource traces | Complete | `f0f4a95` |

Historical deterministic local test baseline observed during the 2026-05-16 and
2026-05-17 core-coding-quality closure, file-patch evidence integration,
file-patch encoded bytes history accuracy, terminal-task `/status` visibility,
terminal-task summary metadata,
foreground/PTY terminal-task data, background-shell terminal-task data,
route-scoped tool-test move, file-edit LSP document sync/diagnostics summary,
file-edit diff metadata, file-mutation lock, file text codec,
file-state tracker, partial-read edit state, file-read/search evidence
metadata, foreground PTY smoke, interactive-shell PTY diagnostic,
background-shell handles/output artifacts/task listing, shell-result
duration/schema/artifacts, shell-command UI summary, shell-command category
permission risk, shell-command category classifier, terminal provider-schema
exposure diagnostic, explicit patch-synthesis fallback boundary,
focused-repair proposal boundary, provider-protocol matrix,
permission-controller, context-budget, tool-result-budget, schema-gate, and
tool-result normalizer work, the file-patch write-mode guard,
live-eval unscored-report classification fix, deterministic patch-rule
priority, required-validation acceptance closeout fallback, and required-command
closeout evidence for no-diff audit tasks, plus Phase 4 ToolExecutionRecord
evidence-ledger persistence and first consumer-integration slices, including
durable route/resource-policy and execution-mode context, permission approval
provenance, file-evidence links, structured output/timing metadata,
route-aware relevance policy reasons, closeout tool-evidence summaries, and
live-eval coding-gauntlet report surfacing, plus first repair-planner consumer
integration that injects repair-relevant tool-record evidence into `RepairSpec`
and guided validation debugging, and first `/trace` replay/debug surfacing of
durable tool-record evidence, including persisted current-session recent-trace
replay merged with in-memory traces, plus workflow-contract auto targeting and
activation trace/report surfacing, plus
risk-signal controller targeting for high-risk runtime/provider/memory/tool/
permission/config/schema/validation surfaces, plus runtime-spine
live-eval/report assertion surfacing:

```text
1468 passed; 0 failed
```

Latest live product baseline:

```text
core-quality-real-rerun-20260517-091952: 8/8 passed
failure_owner=none for every case
required_command_status=ok for every case
real code-change pass=3, audit/no-diff pass=5
```

Latest real-project coding gauntlet checkpoint:

```text
latest full rerun after terminal closeout evidence:
real-project-coding-20260517-192347: 15/15 passed
behavior_assertions=3, behavior_assertions_passed=3
required-validation passes=15/15
coding-gauntlet likely clean passes=7, repaired passes=4
real code-change passes=10, audit/no-diff passes=5
failure_owner=none for every case
closeout_status=passed for every case
backend-todo-api-crud: status=ok, failure_owner=none
frontend-book-notes-localstorage: status=ok, failure_owner=none
core-terminal-install-run: status=ok, failure_owner=none
core-provider-roundtrip: status=ok, failure_owner=none
memory-save-quality-gate: status=ok, failure_owner=none
persistent-memory-planning-context: status=ok, failure_owner=none
skill-promotion-gate: status=ok, failure_owner=none
warnings observed but non-failing:
- audit/no-diff warnings on audit/regression-check tasks
- tool_errors_seen in repaired/probe tasks with passing required commands
targeted Phase 3 repair reruns after required-validation source context:
- repair-planner-frontend-20260517-181652: status=ok, failure_owner=none,
  required_command_status=ok, closeout_status=passed
- repair-planner-backend-20260517-182004: status=ok, failure_owner=none,
  required_command_status=ok, closeout_status=passed,
  warnings=earlier_verification_failed_before_repair,
  earlier_stage_validation_failed_before_repair

Latest workflow-contract targeting checkpoint:

```text
workflow-contract-auto-targeting-20260518-154252: 3/4 passed
backend-todo-api-crud: failed, failure_owner=llm_reasoning,
  contract=entry=skipped:auto repair=active_after_failure
frontend-book-notes-localstorage: status=ok,
  contract=entry=skipped:auto repair=none
code-change-verification-repair-loop: status=ok,
  contract=entry=active:auto repair=active_after_failure
core-permission-rejection-recovery: status=ok,
  contract=entry=active:auto repair=not_needed
```
Historical risk-signal controller checkpoint:
- deterministic local tests: 1468 passed; 0 failed
- `risk_signal_high` now upgrades workflow policy for concrete high-risk
  surfaces before editing
- live-eval reports now include `risk_signal: entry=<level> runtime=<level>`
  and a `risk` summary column

Historical agent-runtime alignment checkpoint:

```text
2026-05-24 combined runtime-spine cleanup validation:
cargo test -q -- --test-threads=1
cargo clippy --all-features -- -D warnings
cargo check -q
cargo check --features experimental-api-server -q
cargo fmt --check
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml --check
corepack pnpm --dir apps/desktop exec tsc --noEmit
corepack pnpm --dir apps/desktop build
corepack pnpm --dir apps/desktop test:ui-smoke
git diff --check
status=passed
```
targeted closeout-evidence rerun:
- terminal-closeout-20260517-191432: status=ok, failure_owner=none,
  required_command_status=ok, closeout_status=passed,
  runtime validation=passed:2/2
previous full rerun before terminal closeout evidence:
real-project-coding-20260517-183221: 14/15 passed

previous post-repair full rerun:
real-project-coding-20260517-171819: 12/15 passed

first gauntlet baseline before targeted repair:
real-project-coding-20260517-153331: 13/15 passed
coding-gauntlet evidence: likely clean passes=7, repaired passes=2,
required-validation passes=13/15, first-write observed=10/15
failures:
- memory-save-quality-gate: failure_owner=llm_reasoning
- persistent-memory-planning-context: failure_owner=agent_flow
targeted repair reruns:
- memory-save-rerun-20260517-170500: status=ok, failure_owner=none,
  behavior_assertions=passed, required_command_status=ok
- persistent-memory-rerun-20260517-172000: status=ok, failure_owner=none,
  behavior_assertions=passed, required_command_status=ok
note=persistent-memory-rerun-20260517-171000 exposed the intermediate
build_memory_context borrow repair path before the final passing rerun
```

Latest memory/skill product-maturity behavior baseline:

```text
product-maturity-seeded-fixes-20260517-143047: 3/3 passed
behavior_assertions=3, behavior_assertions_passed=3
status_counts=passed=3
failure_owner=none for memory-save-quality-gate, skill-promotion-gate, and
persistent-memory-planning-context
real code-change pass=3
deterministic patch repair rules now run before model patch synthesis when a
high-confidence rule matches current evidence
required validation acceptance can close out deterministically even when
workflow judgment was skipped after a non-JSON response
```

Current terminal slice: `bash mode=background` returns a shell handle,
`bash_output` reads bounded output from that handle, `bash_cancel` stops the
process group, foreground timeout results now carry structured
`shell_result.timed_out=true`, and CLI/TUI tool views have explicit
backgrounded/timed-out/cancelled states. Long background output now also writes
an `output_path` artifact under `.priority-agent/tool-results/<session>/...`.
`bash_tasks` lists known background shell handles when the model needs to
recover or inspect active tasks. Foreground bash, `bash mode=pty`, and
background-shell outputs now expose `terminal_task` structured facts with task
id, status, timestamps, duration, artifact path, terminal kind, PTY marker, and
read/cancel handles; `bash_tasks` also exposes the background task list as
`terminal_tasks`. Tool execution summaries now copy compact terminal task
status metadata into machine-readable `tool_summary` fields, so traces can
inspect shell task state without parsing provider text. `/status` now also
shows a read-only terminal task count by running/completed/failed/cancelled/
timed-out state. Obvious interactive commands such as bare `python3`,
`node -i`, bare `ssh` sessions, and `npm init` are now classified as requiring
PTY support; non-PTY bash returns a structured `mode=pty` recovery diagnostic
instead of starting a command it cannot control. `bash mode=pty` now runs
foreground commands through a `portable-pty` backend and records
`terminal_requirement.pty_used=true` in the tool result. PTY execution now uses
the same non-login `bash -c` command shape as foreground bash, avoiding hangs
from user login-shell startup files during short PTY commands.

Current file-quality slice: `file_read` and `grep` now preserve a clearer
raw/display boundary in structured tool data. File reads record path, resolved
path, displayed line range, total/displayed line counts, truncation state,
content hashes, and whether visible content contains line-number display
prefixes. Grep records search kind, display format, raw match lines, line
ranges, byte offsets, and line hashes. EvidenceLedger keeps those file-fact
metadata fields so closeout and later repair logic can use structured facts
instead of relying only on rendered text.
Read state now distinguishes full-file reads from targeted line-range reads.
Exact/insert edits require a full read by default, while line-range edits are
allowed when the requested range is covered by a previous targeted read. The
legacy bypass is explicit via `PRIORITY_AGENT_ALLOW_EDIT_WITHOUT_READ=1`. File
state is now owned by `FileStateTracker`, and file read/edit/write metadata
exposes lexical, resolved, canonical, display, and state-key path identity so
relative, absolute, and canonicalized paths share the same stale-read boundary.
File reads now expose `text_format`
metadata for encoding, BOM, and line ending; edits and writes preserve UTF-8
BOM, UTF-16LE BOM, and LF/CRLF style instead of normalizing files by accident.
File mutations now share a per-canonical-path async lock, and text writes use
a temporary sibling file plus rename so same-file edits are serialized and
write failures do not leave partial file contents. `file_patch` records the
actual encoded bytes written for each patched file, so UTF-16LE/BOM file
history matches disk bytes instead of normalized UTF-8 string length. Each
successful `file_patch` file now also enters EvidenceLedger as changed-file
evidence with patch kind, changed line range, diff truncation state, and compact
bytes-written metadata. `file_patch` partial write failures now return structured
rollback evidence with failed path, checkpoint metadata, written-before-failure
paths, rollback success, and restored/removed/failed rollback files. `file_edit`
success results now include additions,
deletions, changed line range, and a bounded unified diff preview so later
closeout and diagnostics paths can cite actual edit evidence. `file_edit` also
returns a non-blocking `diagnostics` summary with first-error, first-warning,
affected-line-range, and compact EvidenceLedger first-error metadata. It samples
cached LSP diagnostics only from already-initialized clients, avoids triggering
slow language-server startup on the edit path, and records compact LSP
status/counts in EvidenceLedger file facts. The LSP sync path now tracks
documents already sent through `textDocument/didOpen`; follow-up edits use
`didChange` plus `didSave` with monotonic document versions instead of repeating
`didOpen`.

Validated locally with:

```bash
cargo fmt --check
cargo test -q file_tool -- --test-threads=1
cargo test -q evidence_ledger -- --test-threads=1
cargo test -q lsp -- --test-threads=1
cargo test -q tool_result -- --test-threads=1
cargo test -q test_bash_tool_pty_mode_runs_with_tty_stdout -- --test-threads=1
cargo test -q bash_tool -- --test-threads=1
cargo check -q
cargo test -q
cargo clippy --all-features -- -D warnings
bash scripts/workflow-production-gates.sh
```

Historical expanded live-eval checkpoint:

```text
batch6-harnesssplit-20260511-155208 resume-session-picker: ok
diff=yes agent_required_commands=2 harness_commands=1 required_command_status=ok
verification_passed=true stage_validation_passed=true
acceptance_accepted=true closeout_status=passed failure_owner=none
note=full-suite cargo test is now harness-only for this case, keeping agent validation focused while preserving release-level evidence
batch6-evidencefix2-20260511-173535 cli-scrollback-polish: ok
intent=audit_or_regression_check diff=no required_command_status=ok
agent_required_commands=2 harness_commands=1 tool_errors=0 closeout_status=passed failure_owner=none runtime_validation=passed:2/2
note=agent and harness validation environments now agree; audit/no-diff closeout remains valid while runtime validation no longer reports stale recovered failures
```

Latest recovery commits and planning artifacts:

| Area | Commit |
|------|--------|
| Route live coding evals as code changes | `e18de91` |
| Harden live eval patch recovery | `b2ff20c` |
| Record live eval recovery evidence | `6df0039` |
| Add next development plan | `467c3b0` |
| Add bash exposure diagnostics | `d025d6a` |
| Guard terminal and filesystem grounding | `2b1852e` |
| Keep grep evidence raw for patching | `3344363` |
| Extract patch recovery module | this change |
| Extract validation runner helpers | this change |
| Extract repair controller helpers | this change |
| Extract closeout controller helpers | this change |
| Extract tool orchestrator helpers | this change |
| Surface companion helper context | `f33174a` |
| Give focused repair two targeted lookups | this change |
| Add next-stage productization plan | this change |
| Add reference audit and architecture map | this change |
| Normalize pure tool-call assistant messages for strict providers | this change |

The all-features clippy and experimental API checks are clean as of this
focused lookup-budget change.

Latest live coding workflow smoke:

```text
checkpoint-function-anchor-20260509-120047 live-eval-dashboard-summary: ok
diff=yes required_command_status=ok verification_passed=true
stage_validation_passed=true closeout_status=passed failure_owner=none
```

Latest Batch 3 live-suite run:

```text
capability-now-20260509-144729 live-eval-dashboard-summary: ok
diff=yes required_command_status=ok verification_passed=true
stage_validation_passed=true closeout_status=passed failure_owner=none
rerun_after=3344363 grep raw evidence fix
capability-now-20260509-143251 live-eval-dashboard-summary: failed
diff=no required_command_status=failed closeout_status=not_verified failure_owner=agent_flow
root_cause=grep markdown highlighting polluted patch anchors
capability-now-20260509-142349 memory-save-quality-gate: ok
diff=yes required_command_status=ok verification_passed=true
stage_validation_passed=true closeout_status=passed failure_owner=none
capability-now-20260509-141759 frontend-book-notes-localstorage: ok
diff=yes required_command_status=ok verification_passed=true
stage_validation_passed=true closeout_status=passed failure_owner=none
capability-now-20260509-140733 backend-todo-api-crud: ok
diff=yes required_command_status=ok verification_passed=true
stage_validation_passed=true closeout_status=passed failure_owner=none
warnings=tool_errors_seen,earlier_verification_failed_before_repair
capability-now-20260509-135556 code-change-verification-repair-loop: ok
diff=yes required_command_status=ok verification_passed=true
stage_validation_passed=true closeout_status=passed failure_owner=none
```

Latest capability live-suite run:

```text
capability-evidence-20260509-173239: 6/6 passed, all real code-change passes
cases=code-change-verification-repair-loop, live-eval-dashboard-summary,
backend-todo-api-crud, frontend-book-notes-localstorage,
memory-save-quality-gate, skill-promotion-gate
memory_active_tasks=6 memory_changed_plan_tasks=5
skill_active_tasks=1 skill_promotion_evidence_tasks=1
note=live-eval-dashboard-summary recovered from invalid action checkpoint before passing
```

Latest dashboard model-led repair rerun:

```text
dashboard-patch-retry-20260509-200245 live-eval-dashboard-summary: ok
diff=yes required_command_status=ok verification_passed=true
stage_validation_passed=true acceptance_accepted=true closeout_status=passed failure_owner=none
model_file_edit=true patch_synthesis_used=false first_write_tool_index=5
warnings=tool_errors_seen,earlier_verification_failed_before_repair,earlier_stage_validation_failed_before_repair
rerun_after=8d4658b targeted file-read cache fix, 4f4aa8f/ea337e6/cd31b56 checkpoint deferral and retry
note=model produced and repaired its own edits; deterministic patch synthesis did not take over
```

Latest dashboard focused-repair lookup-budget rerun:

```text
focused-lookup-budget-20260509-212938 live-eval-dashboard-summary: ok
diff=yes required_command_status=ok verification_passed=true
stage_validation_passed=true acceptance_accepted=true closeout_status=passed failure_owner=none
model_file_edit=true patch_synthesis_used=false first_write_tool_index=6
tool_errors=0 tool_failures=3 changed_files=1
note=model used two targeted read/search rounds, consumed a line-range correction after a failed edit, then produced its own edit without deterministic patch synthesis
```

Latest aggregate live-eval snapshot:

```text
generated=2026-05-09 14:58:04 +0800
runs_scanned=142 task_reports=142 pass_rate=40/142
instrumented_slice=18/50 passed
real_code_change_passes=13 seeded_no_diff_failures=17
```

Read this aggregate as historical plus current evidence. It still includes many
older reports from before structured `failure_owner`, `eval_intent`, and
adaptive-trigger metadata, while the newest dashboard-summary recovery is a
current passing run with a real code diff.

## Completed Runtime Spine

- `TurnTrace` records prompt, routing, memory, context, tool, permission,
  recovery, goal drift, assistant, and MCP resource events.
- Maintainability cleanup is underway: focused action-checkpoint helpers now
  live outside the core conversation loop, deterministic patch repair is routed
  through a named rule registry with owner/review metadata, live-eval report
  parsing is shared by summary and aggregate scripts, and `/permissions` has
  its own slash-handler module.
- `IntentRouter` chooses workflow/retrieval/reasoning policy and now consumes
  learning events from tool outcomes.
- `SessionGoal` tracks the active goal; high drift requires approval and
  medium drift is visible in `/goal drift` and `/quick`.
- Tool failures attach recovery metadata and persist `tool_outcome` learning
  events.
- Code-change turns record implementation intent before edits, and env-prefixed
  validation commands are classified as validation evidence.
- Final closeout now includes an evidence summary with changed-file,
  validation-status, and acceptance-status counts.
- Core coding tools now attach structured execution summaries; file edits refuse
  stale-read writes by default; bash command classification covers shell/env
  wrappers and common validation families; git tool execution now honors the
  tool working directory and returns structured summary/recovery metadata.
- Memory search spans project, user, topic, and agent namespaces with simple
  conflict detection.
- MCP status and tool/resource visibility are health-aware and approval-aware.
- Hook execution uses typed lifecycle events, records structured run results,
  and is visible through TurnTrace and `/hooks`.
- Tool execution progress labels are classifier-aware for bash validation
  commands, so cargo test/check/clippy and similar commands show specific
  progress instead of generic shell execution text.
- Subagents have explicit profile contracts, independent tool scopes, lifecycle
  trace events, durable result artifacts, and manager tests for timeout, failure,
  cancellation, and resumable results.
- Slash commands are labeled as `production`, `usable`, or `placeholder` in help
  and command-palette surfaces, `/help maturity` reports the current maturity
  buckets, and rendered command-palette smoke tests cover placeholder, usable,
  and contextual permission actions plus approval-panel smoke tests for bash and
  file-write review flows, statusline active-tool state, tool-output viewer
  controls, context/task/MCP/permission runtime-panel output, and diff viewer
  output/empty states.
- MCP repair flow distinguishes explicit approval, explicit OAuth auth, and safe
  circuit-breaker reset actions; `/mcp repair --all` applies only the circuit
  reset subset.
- MCP runtime resource parity is wired through both the `mcp` management tool
  and slash commands: `list_resources`, `read_resource`, `repair_server`,
  `/mcp resources [server]`, and `/mcp read <server> <uri>` share approval,
  health, and per-agent MCP server scope checks.
- Provider protocol hardening now has explicit capability records for provider
  family, streaming/tool-call support, reasoning-token support, MiniMax
  non-streaming tool-call routing, system-message merging, and tool-result
  adjacency. Provider type/config detection feeds those records, and the
  conversation loop uses them for MiniMax-style non-streaming tool requests.
- Terminal API failures now record classified recovery plans. Provider protocol
  and request-schema failures are surfaced as non-safe-retry trace events with
  `/trace last` as the next diagnostic command instead of disappearing into a
  generic API failure.
- Provider fallback is now a visible runtime policy rather than prompt text:
  `ResourcePolicy` exposes `allow_fallback_model` through traces and
  `/resource`; transient provider failures can retry once with
  `PRIORITY_AGENT_FALLBACK_MODEL`, while context-size errors still prefer
  reactive compaction before falling back.
- API request traces now expose provider protocol facts, including detected
  provider family, non-streaming tool-call requirements, and strict tool-result
  adjacency requirements, so `/trace last` can explain why a request changed
  shape for MiniMax/Kimi/OpenAI-compatible providers.
- Plugin reload has a shared lifecycle report for startup injection,
  `plugin_manage reload`, and `/reload plugins`, including discovered/enabled
  counts, injected tool names, skipped reason counts, and trust mode.
- Bridge/remote state is now visible from `/panel bridge` and `/remote status`.
  The panel reports bridge URL source, auth-token presence, tenant id, replay
  cursor state, remote environment detection, saved SSH sessions, and active
  `remote_trigger`/`remote_dev` tool exposure. These facts are also part of
  `RuntimeAppState`/`RuntimeStatusSnapshot`, and the remote trigger tool now
  uses the same bridge config resolver as the TUI status surface.
- Remote bridge actions now flow through the same permission/recovery/trace
  spine as local tools: `remote_trigger` and `remote_dev` classify run/create/
  sync/SSH exec risk, attach remote facts to permission request metadata, record
  `remote.bridge` trace events, and route remote failures toward `/remote status`
  with conservative non-safe-retry guidance.
- Evalsets include a deterministic coding replay matrix, JSON report output,
  `/eval record <name|all>` persisted report files for pass/fail trend
  collection, and `/eval baseline [provider|all]` external-provider comparison
  plus template/import generation for Phase 12 parity scenarios. `/eval
  baseline-import <artifact_path> <provider> [model]` converts existing
  baseline YAML/JSON or Markdown run tables into the shared external-baseline
  schema; `scripts/external-baseline-artifact.py` creates a Markdown
  run-record skeleton with per-scenario evidence cards first. `/eval
  baseline-validate [provider|all]` checks those files for missing scenarios,
  unknown/duplicate ids, placeholder evidence, and pass records that lack
  validation/evidence-backed metadata. `/eval parity [provider|all]` combines
  local replay readiness with imported external provider outcomes and labels
  each Phase 12 scenario gap. `/eval parity-record [provider|all]` writes the
  same report to `target/eval-reports/` as an auditable timestamped artifact.
  `/eval trend [limit]` summarizes recent persisted reports, deltas against
  the previous run, and optional external baseline metadata when present.
- The layered workflow gates now cover focused, standard, full-local, and
  opt-in live-smoke validation; the latest live smoke exercised the real
  code-change repair path and passed with full-suite validation.
- CLI panels are increasingly backed by actual runtime state, not decoration.
- `karpathy-guidelines` is bundled as a coding behavior skill and exposed
  through `/skills`, `/karpathy <task>`, and code-change reflection checks.
- Repeated successful workflows can now become reviewed skill candidates through
  `/skill-proposals`; accepted candidates are untrusted until explicitly
  applied into the user skill path.
- Learning and high-confidence retrieved memory now feed back into workflow
  planning weights with traceable before/after audit records.
- Root `AGENTS.md` is now a compact runtime guide with historical material
  archived under `docs/archive/`; instruction loading prefers the
  `## Agent Runtime Guidance` section instead of prefix-truncating long project
  notes.
- Runtime diet gates now cover representative prompt samples for direct answer,
  scoped file deletion, Python code creation, running-issue debugging, and
  Claude/opencode instruction-design comparison. Tool-surface tests enforce
  route-level caps and prompt-context tests enforce a common-turn token budget.
- Core tool contracts now carry common usage boundaries directly: `file_edit`
  rejects `file_read` line-prefix copy/paste in edit anchors, `file_write`
  reports full-file replacement guidance when overwriting, `bash` is scoped to
  shell/validation work, `agent` discourages blocking delegation, and
  `skill_view` fences skill text as guidance rather than user instruction.
- Built-in subagent profiles now have role-scoped default tool surfaces:
  explorer/planner/verifier stay read-only or validation-only, implementer gets
  edit/write/validation tools, and no built-in profile exposes recursive
  `agent` or `swarm` by default.
- Memory and skill context are now fenced as background guidance. Light/Web/None
  routes do not receive stale memory context, skill listing is compact
  discovery-only, and runtime diet traces report memory, retrieval, and skill
  summary budgets.
- User-facing closeout now defaults to concise assistant text for ordinary
  passed or not-verified low/medium-risk code changes, while high-risk,
  failed, partial, explicit debug/full, and live-eval closeouts retain the full
  structured `Closeout:` block.
- Terminal and filesystem truth guards now catch two high-trust UX failures:
  claiming bash is unavailable when it is exposed, and answering current local
  filesystem state without first using exposed read/list tools. The correction
  stays runtime-owned instead of adding longer always-on prompt rules.
- `glob` now treats `**/` as zero-or-more directories for agent-facing patterns
  and sorts shallow paths first before truncation, so broad local inspection is
  less likely to hide top-level entry files.
- `grep` now leaves visible match lines as raw source and carries match text in
  structured metadata, preventing Markdown emphasis from contaminating
  file-edit anchors and patch synthesis.
- Code-change turns now surface concise companion-context hints after targeted
  `file_read`/`grep` evidence when nearby helper/parser files strongly match
  the inspected file and task. This keeps helper discovery in runtime evidence
  instead of adding more always-on prompt rules.
- Bash command failures now add a concrete compatibility hint for macOS bash
  3.x associative-array errors (`declare -A`), steering repair toward portable
  shell, awk/temp-file, or existing Python helper paths.
- The current agent-runtime alignment slice adds a typed five-zone context
  assembly plan, a compact `AgentTaskState` recordbook, phase-aware programming
  tool exposure, runtime-owned action scoring, proof-backed closeout semantics,
  state-backed stop checks, edit/diff/validation/user-confirmation ledger
  evidence, task-state goal/scope drift checks, direct-task over-control
  regressions, bounded edit/repair snapshots, and a `/trace` control-loop
  diagnostic map. It also adds a cross-module runtime-spine behavior regression
  so context-zone order, phase transitions, action scoring, no-progress stop
  checks, and verification proof semantics are covered together.

## Product Surface

Primary interface:

- `priority-agent`
- `priority-agent --cli`

Compatibility:

- `priority-agent --tui` starts the compatibility full-screen terminal interface.

Secondary interfaces:

- HTTP API with REST/WebSocket/SSE behind `experimental-api-server`.
- Platform adapter framework with Telegram implemented.
- MCP client over stdio, WebSocket, and HTTP.

## Documentation Status

**Docs index**: `docs/README.md` — categorized navigation for all docs.

### Canonical current docs

- `README.md`
- `AGENTS.md`
- `docs/README.md`
- `docs/PROJECT_STATUS.md`
- `docs/PROJECT_MAP.md`
- `docs/CLAUDE_CODE_GAP_MATRIX_2026-05-03.md`
- `docs/CLAUDE_CODE_ALIGNMENT_PLAN.md`
- `docs/CLAUDE_CODE_PARITY_IMPLEMENTATION_PLAN_2026-05-20.md`
- `docs/REMAINING_CLOSURE_PLAN.md`
- `docs/LLM_RUNTIME_SIMPLIFICATION_PLAN_2026-05-08.md`
- `docs/RUNTIME_DIET_UPDATE_2026-06-02.md`
- `docs/UNIFIED_RUNTIME_ENTRYPOINTS_2026-06-01.md`
- `docs/NEXT_DEVELOPMENT_PLAN_2026-05-09.md`
- `docs/PERSONAL_AGENT_PRODUCT_PRINCIPLES_2026-05-18.md`
- `docs/AGENT_LEARNING_NOTES_PROJECT_ALIGNMENT_2026-05-24.md`
- `docs/AGENT_TESTING_MATRIX_2026-05-08.md`

### Active plans (recent, may still be in progress)

- `docs/MEMORY_SYSTEM_SIMPLIFICATION_PLAN_2026-06-02.md`
- `docs/AGENT_SKILLS_OPTIMIZATION_PLAN_2026-06-01.md`
- `docs/LIVE_CODING_TEST_PLAN_2026-06-01.md`
- `docs/NEW_FEATURE_EVAL_PLAN_2026-06-01.md`
- `docs/REAL_WORLD_TEST_PLAN_2026-06-01.md`
- `docs/TEST_LANES_2026-05-29.md`
- `docs/DEVELOPMENT_REFACTORING_PLAN_2026-05-28.md`
- `docs/SELF_EVOLUTION_EVAL_LOOP_PLAN_2026-05-28.md`
- `docs/CODING_FLOW_POLISH_OBSERVABILITY_PLAN_2026-05-28.md`
- `docs/BEHAVIOR_ASSERTION_NO_EFFECTIVE_DIFF_REPAIR_PLAN_2026-05-28.md`
- `docs/FLOW_STABILIZATION_TEST_PLAN_2026-05-27.md`
- `docs/FLOW_STABILIZATION_TEST_RUN_2026-05-27.md`
- `docs/HERMES_MEMORY_FEATURE_FOLLOWUP_PLAN_2026-05-27.md`
- `docs/RUNTIME_SPINE_STABILITY_REVIEW_PLAN_2026-05-26.md`

### Reference docs (design notes, architecture, principles)

- `docs/AGENTIC_DESIGN_PATTERNS_REVIEW.md`
- `docs/AGENT_RUNTIME_CONTRACT_PLAN.md`
- `docs/CODING_AGENT_WORKFLOW_DISCUSSION.md`
- `docs/CONVERSATION_LOOP_RESPONSIBILITY_MAP_2026-05-11.md`
- `docs/FUNCTIONAL_REALITY_AUDIT.md`
- `docs/MEMORY_CONTROLLED_SELF_EVOLUTION_DESIGN.md`
- `docs/PROJECT_FLOW_AND_RUNTIME_ARCHITECTURE_2026-05-26.md`
- `docs/HERMES_MEMORY_SELF_EVOLUTION_REVIEW.md`
- `docs/ACTIVE_MEMORY_PROTOTYPE.md`
- `docs/SKILL_ROOTS_AND_TRUST.md`
- `docs/SOUL_USER_TOOLS_CONTEXT.md`

### Historical docs (archived)

Completed/expired plans moved to `docs/archive/`. See `docs/archive/ARCHIVE_INDEX.md` for the full inventory and归档原因.

Also kept for reference:

- `PLAN.md`
- `CAPABILITY_MATRIX.md`
- `docs/workflow/*`

## Remaining Work

The latest 5-item closure plan is complete, and the first Claude-gap P0/P1
implementation batch is now landed. The current plan is
`docs/NEXT_DEVELOPMENT_PLAN_2026-05-09.md`: treat Priority Agent as a reliable
LLM execution environment, move hard constraints into runtime/tool contracts,
and measure progress with current live-eval evidence instead of prompt length.
The remaining work is now product maturity and behavior coverage, not missing
foundations:

1. Continue measuring broad code-change first-pass success and repair count
   against the replay matrix and live eval tasks.
2. Execute the next plan in order: Batch 1 baseline hygiene, Batch 2
   terminal/filesystem truth, Batch 3 five-case live suite, and Batch 4
   conversation-loop extraction are now landed. Batch 5 has started with
   explicit coding agent modes, mode-visible status, and stronger `/doctor`
   tool-exposure diagnostics. Batch 6 report-layer memory/skill evidence is
   now landed in live summary and aggregate reporting, and has a first
   six-case live baseline. Behavior-level memory/skill assertion metadata is
   now part of the live-eval report layer, and the affected recommended
   memory/skill cases have been rerun. The current rerun shows the audit/no-diff
   memory cases passing, and the three previously failing seeded memory/skill
   code-change cases now have a targeted `3/3` rerun with required commands,
   behavior assertions, and closeout passing. A full six-case rerun can refresh
   the combined recommended baseline, but the previous blockers are no longer
   active.
3. Continue hardening long-running command progress around cancellation,
   timeout, and streamed partial output.
4. Expand rendered command-level smoke tests beyond core panels into broader
   settings and history surfaces.
5. Populate persisted eval reports with real external Claude/Codex baseline
   data once those baseline runs are available.
6. Continue CLI polish based on trace-backed state: command palette, statusline,
   approval panels, tool expansion, and settings visibility.
7. Harden ecosystem integrations: MCP server mode, plugins, remote workflows,
   Discord/Slack adapters if they become product priorities.
8. Keep docs synchronized with tests and current behavior.
9. Run real desktop dogfood cases against the new runtime spine and compare
   task state, proof state, trace coverage, and visible desktop evidence.
10. Tune `TaskModeScore`, `LightweightPlan`, and runtime-spine assertions from
    those real runs instead of adding more broad skeleton code.

Latest maintenance note:

- `core-quality-real-rerun-20260517-091952` refreshed the real
  `core-coding-quality` agent-run baseline on 2026-05-17: `8/8` passed,
  required commands were `ok` for every case, failure owner was `none` for every
  case, `3` seeded code-change tasks produced real diffs, and `5` audit/no-diff
  tasks passed as expected.
- `product-maturity-memory-skill-20260517-102935` refreshed the affected
  recommended memory/skill behavior-assertion baseline on 2026-05-17: `5/6`
  passed, `5/6` behavior assertion tasks passed, and
  `persistent-memory-planning-context` failed with `failure_owner=agent_flow`.
  The stale fixture path was updated from the old main loop to
  `TurnRetrievalContextController`; the remaining failure is a real runtime
  flow issue where reflection permission stopped the seeded code-change task
  before tools, leaving the required memory prefetch, merge, and trace
  assertions unrestored.
- `persistent-reflection-fix-20260517-113556` fixes that pre-tool reflection
  stop path: entry-gate task context now treats extracted required validation
  commands as acceptance checks before the reflection gate runs. The targeted
  rerun passed with `required_command_status=ok`,
  `behavior_assertion_status=passed`, `failure_owner=none`, one real code diff,
  and isolated full-suite evidence of `1438 passed; 0 failed`. The later
  recommended memory/skill rerun is the current broader baseline and shows that
  the targeted fix did not fully generalize.
- `product-maturity-memory-skill-rerun-20260517-124722` reran the six affected
  recommended memory/skill cases after the file-patch write guard and
  live-eval scoring fixes: `3/6` passed, all three audit/no-diff memory cases
  passed, and all three seeded code-change cases failed. The failures are
  `memory-save-quality-gate` (`failure_owner=agent_flow`,
  `closeout_not_successful`), `skill-promotion-gate`
  (`failure_owner=agent_flow`, required commands and behavior assertions
  failing), and `persistent-memory-planning-context`
  (`failure_owner=llm_reasoning`, required commands, behavior assertions,
  stage validation, verification, acceptance, and closeout failing).
- `product-maturity-seeded-fixes-20260517-143047` reran the three previously
  failing seeded memory/skill code-change cases after the closeout and
  deterministic patch-rule priority fixes: `3/3` passed, all three produced
  real diffs, all required commands were `ok`, all behavior assertions passed,
  closeout passed, and `failure_owner=none` for every case.
- Patch synthesis now gives high-confidence deterministic repair rules first
  refusal when they match current evidence. This prevents a valid-but-wrong
  model JSON patch from overriding known regression repairs such as
  `memory-save-quality-gate` and `skill-promotion-gate`.
- Required-validation acceptance can now produce a deterministic accepted
  review from task-bundle acceptance checks when all required validation
  commands pass but workflow judgment was skipped after a non-JSON model
  response. This keeps closeout from leaving required-command checks pending
  after successful validation.
- Live-eval summaries now carry explicit `behavior_assertions` and
  `behavior_assertion_status` fields. The first product-maturity slice tags the
  memory and skill recommended tasks so summary and aggregate reports can show
  memory/skill behavior coverage separately from memory/skill activity signals.
- `file_patch` write-mode operations now honor the same existing-file
  read-before-edit and stale-read checks as targeted patch operations, and the
  live-eval report parser now marks unverified collect-only reports as
  `skipped` instead of counting them as passed.
- `cargo test -q` is clean as of 2026-05-17 with `1444 passed; 0 failed`.
- Provider API calls now use a bounded reconnect policy for transient transport
  failures. `PRIORITY_AGENT_PROVIDER_RECONNECT_ATTEMPTS` defaults to `5`
  reconnect opportunities, with exponential backoff, and does not retry
  auth/schema/400-class request-contract failures.
- Provider health preflight is now protocol-focused: plain chat, tool-call, and
  tool-result continuation must work, but the continuation probe no longer
  requires the model to repeat an exact Closeout phrase before live evals can
  start.
- Historical validation for the companion-context slice: `cargo fmt --check`,
  `cargo test -q companion_context`, `cargo test -q shell_compatibility_hint`,
  `cargo test -q agent_mode`, `cargo check -q`, `cargo test -q`,
  `cargo clippy --all-features -- -D warnings`,
  `cargo check --features experimental-api-server -q`, and `git diff --check`.
- Historical validation for the focused lookup-budget slice:
  `cargo fmt --check`, `cargo test -q focused_repair_prompt`,
  `cargo test -q file_edit_failure_correction`, `cargo check -q`,
  `cargo test -q`, `cargo clippy --all-features -- -D warnings`,
  `cargo check --features experimental-api-server -q`,
  `bash -n scripts/run_live_eval.sh`,
  `bash -n scripts/live-eval-aggregate-summary.sh`,
  `bash scripts/live-eval-summary-smoke.sh`, `git diff --check`,
  and `scripts/run_live_eval.sh --case live-eval-dashboard-summary --mode agent-run --run-tests --label focused-lookup-budget --overlay-working-tree`.
- Batch 5 product mode work has an explicit runtime `AgentMode`
  (`auto/build/plan/explore/review`) that flows from TUI `/mode` into
  streaming and `ConversationLoop` route/tool exposure. `/status`, `/quick`,
  status bar, and `/doctor` now show the current mode; `/doctor` also reports
  how the current mode affects bash and write-tool visibility.
- Validation for the Batch 5 mode slice: `cargo fmt --check`,
  `cargo test -q agent_mode`, `cargo test -q mode_`,
  `cargo test -q doctor_route_summary_applies_agent_mode_before_exposure_checks`,
  `cargo test -q status`, `cargo test -q quick`, `cargo test -q tool_view`,
  `cargo check -q`, and `cargo test -q`.
- Batch 6 reporting now surfaces memory/skill evidence in
  `scripts/run_live_eval.sh --mode summary` and
  `scripts/live-eval-aggregate-summary.sh`: memory active tasks, recalled
  items, conflict counts, changed-plan signals, skill active tasks, usage
  events, and promotion-evidence tasks. Validation: `python3 -m py_compile
  scripts/live_eval_report_parser.py`, `bash -n scripts/run_live_eval.sh`,
  `bash -n scripts/live-eval-aggregate-summary.sh`,
  `bash scripts/live-eval-summary-smoke.sh`, `cargo test -q memory`,
  `cargo test -q retrieval_context`, `cargo test -q skills`,
  `bash scripts/coding-workflow-gates.sh standard`, and `cargo check -q`.
- Historical six-case live capability suite:
  `docs/benchmarks/live-capability-evidence-20260509-173239/summary.md` with
  `6/6` passed real code-change tasks. During this pass, stale live-eval
  fixtures were refreshed for the extracted repair controller and learning
  slash-handler modules, and skill-promotion evidence detection was widened so
  `skill-promotion-gate` is counted as a skill-specific task.
- `conversation_loop/mod.rs` is down to 722 lines after moving turn setup,
  entry gates, loop bootstrap, iteration sequencing, tool-round sequencing,
  post-change closeout, retrieval helpers, workflow-runtime helpers, and
  tool-context helpers into dedicated conversation-loop modules, then moving
  route-scoped tool exposure tests out of the main module.
- `cargo clippy --all-targets --all-features -- -D warnings` was last recorded
  clean on 2026-06-09 after the history-restore and CI gate stabilization
  fixes.
- Historical `scripts/validate_docs.sh` run counted 74 registered tool entries
  and 130 command constants, then passed all required docs, all-features build,
  and the workflow-enabled full test suite.
- `/resume` now resolves recent conversations by number, id prefix, title/model
  keyword, or message search, and restored sessions show a recent context
  preview.
- The scrollback-first interactive shell now prints concise long-running tool
  progress lines, so validation work is visible without switching to a
  full-screen interface.
- Live eval task parsing no longer depends on PyYAML for prepare/collect paths,
  and the dashboard-summary seeded fixture now preserves the summary entrypoint
  while stubbing only `summary_task()`.
- Historical dashboard-summary agent-run,
  `checkpoint-function-anchor-20260509-120047`, produced a real diff, passed
  required commands, passed verification/stage validation, and ended with
  `failure_owner=none`.
- Live eval summaries now include pass/failure rates, real code-change pass
  counts, plan-only pass counts, seeded no-diff failure counts, and aggregated
  failure modes. `scripts/live-eval-summary-smoke.sh` covers this without
  running an LLM and is part of the quick coding workflow gate.
- `scripts/live-eval-aggregate-summary.sh` now reads benchmark `report.md` and
  quality artifacts directly instead of overwriting per-run `summary.md` files,
  then writes `docs/benchmarks/live-eval-shortfall-summary.md`; the current
  aggregate scans 136 task reports, with 35 passed and 101 failed. The cleaner
  instrumented slice has 44 reports, 13 passed, 31 failed, and shows
  `agent_flow` at 43.2%, `llm_reasoning` at 20.5%, and eval-harness failures at
  6.8%.
- Live eval reports now classify action-checkpoint stops separately
  (`action_checkpoint_no_patch`, `action_checkpoint_invalid_tools`,
  `patch_synthesis_no_change`) and the aggregate report has an `Agent Flow
  Stops` section. This separates model reasoning failures from execution-loop
  failures where the agent never produced an applicable patch.
- Focused repair prompts now consistently allow up to two targeted
  `file_read`/`grep` lookups before patching instead of contradicting that with
  a blanket read/search ban. Action-checkpoint unexposed-tool errors now list
  the currently exposed tools and the expected repair path.
- Action checkpoint now enforces that targeted lookup budget in the exposed
  tool set: after the budget is used, the next focused repair request hides
  read/search tools and forces patch tools only.
- A live A/B on `memory-save-quality-gate` confirmed the lookup-budget change
  moved the run from `agent_flow/action_checkpoint_no_patch` with zero edits to
  a real repair loop with changed files, validation, guided debugging,
  acceptance review, and final `llm_reasoning` failure. The remaining failure is
  product reasoning/repair quality, not checkpoint tool flow.
- Verification and acceptance failures now generate a deterministic
  `RepairSpec` prompt that lists failed commands, extracted failing tests,
  required next-patch constraints, forbidden fixes, and validation commands.
  This gives the model a structured repair target without writing the product
  patch for it.
- Action-checkpoint patch repair now records patch synthesis `source` plus
  owner/reason in traces. When a high-confidence deterministic repair rule
  matches current evidence, deterministic patch synthesis runs before model
  JSON/tool-call synthesis. Model synthesis remains the fallback for evidence
  without a matching deterministic rule.
- `/status` and `/doctor` terminal diagnostics now include bash provider-schema
  compatibility (`schema=ok` or a concrete schema failure reason) alongside
  registry, availability, permission, and route exposure checks.
- Bash command classification now has a finer `ShellCommandCategory` shared by
  bash result metadata, evidence, tool summaries, and progress labels. Ordinary
  `rg ...` commands are search operations; only explicit `! rg ...` assertions
  count as required validation.
- Bash permission risk now uses that shared category: read/list/search and
  validation commands are low risk, while package install, dev server, file
  mutation, git mutation, network, outside-workspace, and destructive commands
  keep stronger confirmation behavior.
- TUI bash summaries now reuse the shared classifier, so terminal UI labels for
  listing, search, validation, package install, dev server, git mutation, and
  shell mutation match runtime evidence and permission semantics.
- Bash tool results now include structured `shell_result` metadata and store
  long combined output under `.priority-agent/tool-results/<session>/...` while
  keeping provider-facing output bounded to a preview. Tool execution metadata
  fills `shell_result.duration_ms` after the controller measures elapsed time.
- Code-change workflow strictness is now adaptive instead of medium-risk by
  default: required validation, first code change, failed verification,
  acceptance rejection, and repeated no-edit progress activate the heavier
  judgment/validation/repair path automatically. Closeout evidence records the
  trigger labels so benchmark reports can explain why strict mode engaged.
- Adaptive workflow triggers are first-class trace events and live-eval
  summaries now expose a `triggers` column plus aggregate trigger distribution,
  so strict-mode activation can be measured without parsing fallback prose.
- Audit/regression live evals now route through the code workflow without
  requiring arbitrary diffs, bash child processes strip agent runtime env vars
  before running validation commands, and workflow judgment factor parsing
  tolerates missing optional fields. After the reconnect policy and
  protocol-only provider health update, `batch6-reconnect-20260511-132912`,
  `batch6-reconnect-20260511-133851`, and
  `batch6-reconnect-20260511-135823` passed as audit/no-diff checks with
  required commands ok, full `1195 passed; 0 failed`,
  `closeout_status=passed`, and `failure_owner=none`.
- The expanded 12-case recommended suite now has current passing evidence.
  Cases 8-10 passed as audit/no-diff checks in the reconnect batch.
  `resume-session-picker` passed in `batch6-harnesssplit-20260511-155208`
  after focused agent-visible validation was split from harness-only full-suite
  validation. `cli-scrollback-polish` passed in
  `batch6-evidencefix2-20260511-173535` after runtime validation labels and
  live-eval provider environments were aligned.
- Provider health preflight is now available as
  `priority-agent --provider-health` and is enabled by default for
  `scripts/run_live_eval.sh --mode agent-run`. It probes plain chat, tool-call,
  and tool-result continuation before spending a live-eval run; failures are
  written as provider-health artifacts and task reports classify the stop as an
  environment/provider issue. Use `--skip-provider-health` or
  `PRIORITY_AGENT_LIVE_EVAL_PROVIDER_HEALTH=0` only for debugging the gate
  itself.
