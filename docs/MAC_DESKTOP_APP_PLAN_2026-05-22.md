# macOS Desktop App Plan

Date: 2026-05-22

## Goal

Build an installable macOS desktop app for Priority Agent with a Codex-like
local coding workspace UI.

The first release should feel like a focused desktop coding partner, not a
generic clone. Keep the left navigation and chat composer familiar, but defer
plugins, automations, mobile, cloud sync, marketplace, and broad generic
product surfaces.

## Current Repository Fit

The current runtime is a Rust CLI/TUI programming agent. The reusable core for
the desktop app is not the existing experimental HTTP chat API. It is the
`StreamingQueryEngine` path initialized through `bootstrap::init_components`.

Important current facts:

- `src/main.rs` owns startup routing for `--cli`, `--tui`, `--api`, eval, and
  provider health.
- `src/bootstrap.rs` builds provider, tool registry, LSP, worktree, memory,
  session store, agent manager, and approval channel.
- `src/engine/streaming.rs` exposes `StreamEvent`, including text deltas, tool
  lifecycle events, usage, errors, and permission requests.
- `src/api/state.rs` and `src/api/websocket.rs` currently use direct provider
  chat for user messages, so they bypass the coding agent loop, tool workflow,
  memory, validation, and approval semantics.
- `scripts/package-release.sh` only packages the CLI binary as a tarball. It
  does not create a `.app`, `.dmg`, code signing, notarization, or auto-update
  release.

Conclusion: the desktop app should call the streaming engine directly through a
desktop runtime facade, then later expose the same facade to any local API if
needed.

## Technology Decision

Use Tauri 2 for the first macOS desktop app.

Why:

- The core runtime is already Rust, so Tauri lets the app reuse the runtime
  directly instead of wrapping it behind a second Node/Electron process.
- Tauri can build macOS `.app` and `.dmg` artifacts from the same desktop app
  project.
- The frontend can be React + TypeScript + Vite, which is enough for a dense
  Codex-like workspace UI without turning the backend into JavaScript.
- Tauri commands and events match the needed interaction pattern: frontend
  invokes Rust commands, Rust streams runtime events back to the window.

Avoid Electron for now. Electron is reasonable for VS Code-style extension
ecosystems, but this project should keep the desktop shell small and preserve
the Rust runtime as the product center.

## App Structure

Add a desktop app under:

```text
apps/desktop/
  package.json
  index.html
  src/
    main.tsx
    app/
    components/
    runtime/
    styles/
  src-tauri/
    Cargo.toml
    tauri.conf.json
    src/
      main.rs
      commands.rs
      runtime.rs
```

Add or refactor the root Rust crate so the desktop app can depend on the agent
runtime as a library:

```text
src/lib.rs         # public runtime modules and bootstrap exports
src/main.rs        # thin CLI/TUI/API binary entrypoint
```

This is the preferred path because it keeps one runtime implementation. A
sidecar binary is only a fallback if the library split becomes too risky.

## Runtime Boundary

Create a small desktop-facing Rust facade rather than letting the UI call
internal engine objects directly.

Suggested module:

```text
src/desktop_runtime/
  mod.rs
  app.rs
  events.rs
  sessions.rs
  projects.rs
```

Responsibilities:

- initialize one `StreamingQueryEngine` per active desktop session;
- select working directory/project before initializing tool context;
- map `StreamEvent` into stable JSON events for the frontend;
- bridge `PermissionRequest` to a native modal or in-app approval row;
- expose session history, trace summaries, config, model/provider status, and
  current project metadata;
- keep file system and shell permissions enforced in Rust, not in frontend
  state.

Do not expose raw tool registry access to the UI in the first release. The UI
should observe tool events and answer approval requests, not become a second
tool caller.

## Frontend MVP

Match the Codex-style shape without copying unrelated surfaces:

- left sidebar:
  - New Chat
  - Search
  - Projects
  - recent conversations
  - Settings
- main panel:
  - empty state prompt centered in the first viewport;
  - chat transcript with assistant text, compact tool progress, and validation
    status;
  - bottom composer with model selector, working directory selector, permission
    mode, and send button;
  - inline permission approval card when the runtime asks for tool approval;
  - compact trace drawer for debugging after a run.
- defer:
  - Plugins
  - Automations
  - Mobile
  - marketplace
  - remote/cloud session sync
  - multi-window project orchestration

The first app should be useful for gex's local coding workflow before trying to
serve generic users.

## Event Contract

Start with a stable TypeScript event union:

```ts
type DesktopRunEvent =
  | { type: "run_started"; runId: string; sessionId: string }
  | { type: "assistant_delta"; text: string }
  | { type: "thinking_delta"; text: string }
  | { type: "tool_started"; id: string; name: string }
  | { type: "tool_args_delta"; id: string; delta: string }
  | { type: "tool_completed"; id: string; resultPreview: string; metadata?: unknown }
  | { type: "permission_request"; id: string; toolName: string; arguments: unknown; prompt: string }
  | { type: "usage"; promptTokens: number; completionTokens: number; reasoningTokens?: number }
  | { type: "run_completed" }
  | { type: "run_error"; message: string };
```

This maps directly to `src/engine/streaming.rs::StreamEvent` and keeps the UI
independent from internal trace structs.

## Packaging Path

Phase 1 should produce a local development app:

```bash
pnpm --dir apps/desktop install
pnpm --dir apps/desktop tauri dev
```

Phase 2 should produce an unsigned or ad-hoc signed macOS app for local use:

```bash
corepack pnpm --dir apps/desktop tauri build --bundles app
```

Phase 3 should produce direct-download distribution artifacts:

```bash
corepack pnpm --dir apps/desktop tauri build --bundles app,dmg
```

For external distribution outside the Mac App Store, plan on Developer ID code
signing and notarization. Free/ad-hoc signing is acceptable for local testing
but not for a smooth downloaded app experience.

Do not target the Mac App Store first. The sandbox and entitlement constraints
conflict with a local coding agent that needs project file access, shell
commands, local dev servers, and tool execution.

## Implementation Phases

### Progress - 2026-05-22

Phase 0 is implemented:

- `src/lib.rs` now exposes the reusable runtime modules.
- `src/main.rs` is a thinner binary entrypoint that imports the runtime through
  the library crate.
- `src/desktop_runtime/mod.rs` provides the first desktop-facing facade,
  including stable serialized run events mapped from `StreamEvent`.
- Validation passed:

```bash
cargo fmt --check
cargo check -q
cargo test -q desktop_runtime
cargo test -q instructions
cargo test -q prompt_context
cargo test -q test_detect_startup_mode
cargo check --features experimental-api-server -q
```

Phase 1 initial shell is implemented:

- `apps/desktop` now contains a Tauri 2 + React + TypeScript + Vite desktop
  app.
- The Tauri backend exposes `desktop_health`, `select_project`,
  `send_message`, and `answer_permission`.
- `send_message` initializes the Rust `DesktopRuntime`, calls the
  `StreamingQueryEngine`, and emits `desktop-run-event` payloads for the
  frontend.
- The frontend has a first Codex-like shell: left navigation, project path
  selector, transcript area, bottom composer, compact tool rows, and a
  browser-only web preview fallback.
- A local macOS `.app` bundle builds successfully at:

```text
apps/desktop/src-tauri/target/release/bundle/macos/Priority Agent.app
```

- Validation passed:

```bash
corepack pnpm install
corepack pnpm build
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml --check
cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml -q
cargo fmt --check
cargo check -q
cargo test -q desktop_runtime
corepack pnpm tauri build --bundles app
```

- Browser preview smoke test passed against `http://127.0.0.1:1420/`.

Phase 1 deepening is implemented:

- Recent sessions are loaded from the existing `SessionStore` and shown in the
  sidebar with model and message counts.
- Clicking a recent session loads stored messages into the transcript for
  inspection.
- Project selection now supports a native macOS directory picker through the
  Tauri dialog plugin, while keeping the manual path field.
- Permission requests are rendered as an in-app approval card wired to
  `answer_permission`.
- `apps/desktop/src-tauri/icons/icon.png` is now a generated project icon
  instead of a throwaway placeholder.
- Validation passed:

```bash
corepack pnpm install
corepack pnpm build
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml --check
cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml -q
cargo fmt --check
cargo check -q
cargo test -q desktop_runtime
corepack pnpm tauri build --bundles app
```

- Browser preview smoke test confirmed sidebar session loading and the updated
  layout. The native command bridge still needs a dedicated Tauri dev smoke.

Session resume is implemented:

- `DesktopRuntime::initialize_for_session` now restores an existing session ID
  and conversation history into `StreamingQueryEngine`.
- The desktop app exposes `resume_session`, which loads stored messages and
  replaces the active runtime with one bound to that session.
- Clicking a recent session now prepares the next `send_message` call to
  continue that session instead of starting a fresh one.
- The restored history is also used as model context through
  `StreamingQueryEngine::set_history`.
- Validation passed:

```bash
corepack pnpm build
cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml -q
cargo fmt --check
cargo check -q
cargo test -q desktop_runtime
corepack pnpm tauri build --bundles app
```

Recent session refresh is implemented:

- `run_started` now marks the active session in the sidebar when the runtime
  reports a session ID.
- `run_completed`, `run_error`, and command-level send failures refresh the
  recent-session list so counts and ordering update after a run.
- Validation passed:

```bash
corepack pnpm build
cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml -q
cargo fmt --check
cargo check -q
cargo test -q desktop_runtime
corepack pnpm tauri build --bundles app
```

Desktop smoke path is implemented:

- Tauri backend commands now have testable command-core helpers for health,
  project-path validation, recent session listing, and message loading.
- `scripts/desktop-smoke.sh` runs the reproducible desktop checks:
  frontend build, root runtime formatting/checks, `desktop_runtime` tests,
  Tauri command bridge checks, and `desktop_smoke` tests.
- `scripts/desktop-smoke.sh --bundle` additionally builds the local macOS
  `.app` bundle and verifies that it exists.
- Validation target:

```bash
scripts/desktop-smoke.sh --quick
```

Frontend structure and trace drawer are implemented:

- `App.tsx` is now a coordinator for initialization, API calls, and top-level
  state.
- Transcript rendering, the sidebar, permission approval, the composer, and the
  trace drawer are split into focused components under
  `apps/desktop/src/app/components`.
- `runEventState.ts` owns the pure run-event state transitions, including
  transcript updates, selected-session updates, permission state, terminal run
  state, and trace item collection.
- The trace drawer shows run, tool, permission, usage, and error events without
  mixing debug evidence into the main chat transcript.
- Validation passed:

```bash
corepack pnpm --dir apps/desktop build
```

Reproducible icon and local packaging scripts are implemented:

- `scripts/generate-desktop-icon.py` regenerates the desktop app icon with only
  Python standard library plus macOS `sips`/`iconutil`.
- The generated assets now include both
  `apps/desktop/src-tauri/icons/icon.png` and
  `apps/desktop/src-tauri/icons/icon.icns`.
- `tauri.conf.json` points the bundle at those icon assets.
- `scripts/package-macos-app.sh` wraps the local macOS packaging flow:
  regenerate icons, optionally run frontend/Tauri command checks, build `.app`
  or `.app` + `.dmg`, and optionally ad-hoc sign the `.app`.
- Validation passed:

```bash
scripts/generate-desktop-icon.py
bash -n scripts/package-macos-app.sh
python3 -m py_compile scripts/generate-desktop-icon.py
python3 -m json.tool apps/desktop/src-tauri/tauri.conf.json
scripts/package-macos-app.sh --help
corepack pnpm --dir apps/desktop build
```

Desktop settings persistence is implemented:

- The Tauri backend stores a small `desktop-settings.json` in the app data
  directory.
- The settings file records the selected project and active session ID.
- `desktop_settings` exposes the restored project/session state to the
  frontend at startup.
- `select_project`, `resume_session`, and run-start handling persist the
  current desktop state.
- The frontend initializes from `desktop_settings` and automatically resumes
  the active session when one is available.
- Validation passed:

```bash
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml --check
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -q desktop_smoke
cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml -q
corepack pnpm --dir apps/desktop build
```

Onboarding diagnostics are implemented:

- The Tauri backend exposes `desktop_diagnostics`.
- Diagnostics check provider-key presence without exposing secrets, current
  shell, `git`, Rust toolchain, Corepack, Xcode command line tools, selected
  project access, and desktop settings storage access.
- The frontend shows a compact diagnostics panel at startup with warning/error
  summaries and a manual refresh action.
- Diagnostics refresh after project selection changes.
- Validation passed:

```bash
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml --check
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -q desktop_smoke
cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml -q
corepack pnpm --dir apps/desktop build
```

Settings drawer and provider setup entrypoints are implemented:

- The sidebar Settings action opens a focused settings drawer.
- The drawer shows the current project, active session ID, desktop settings
  file path, provider setup hints, and the current diagnostics list.
- `provider_setup_info` reports the shell profile path, accepted provider key
  names, and a safe example without exposing secret values.
- `open_settings_folder` opens the desktop app data folder.
- `open_shell_profile` creates the shell profile file when missing and opens it
  for provider-key setup.
- Validation passed:

```bash
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml --check
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -q desktop_smoke
cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml -q
corepack pnpm --dir apps/desktop build
```

First-run provider setup flow is implemented:

- The Settings drawer now promotes missing provider keys into a focused setup
  guide.
- When no provider key is detected, the guide shows the shell profile path,
  accepted provider environment variables, a safe `export ...` example, and the
  restart/refresh requirement.
- The flow can open the shell profile and refresh diagnostics from the same
  panel.
- When a provider key is present, the same surface shows the configured
  provider status without exposing secret values.
- Validation passed:

```bash
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml --check
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -q desktop_smoke
cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml -q
corepack pnpm --dir apps/desktop build
```

Permission-default settings are implemented:

- `desktop-settings.json` now stores a persisted `permission_mode`.
- The backend exposes `permission_mode_options` and `set_permission_mode`.
- Supported desktop defaults are `default`, `auto_low_risk`, `auto`, and
  `read_only`; one-shot permission mode is intentionally not persisted as a
  desktop default.
- Runtime initialization applies the persisted permission mode to the
  `StreamingQueryEngine`.
- Changing the permission mode in Settings updates the active runtime when one
  is already loaded.
- The Settings drawer now includes a Permission defaults section with clear
  descriptions for each mode.
- Validation passed:

```bash
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml --check
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -q desktop_smoke
cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml -q
corepack pnpm --dir apps/desktop build
```

Provider/model selector is implemented:

- `desktop-settings.json` now stores optional `provider_name` and `model`
  fields.
- The backend exposes `provider_model_status` and `set_provider_model`.
- Provider status is built from the same environment-backed provider registry
  as the TUI, with missing default providers shown as disabled options.
- Runtime initialization and resumed sessions apply the persisted desktop
  provider/model selection when the provider is configured.
- The composer now shows provider and model selectors next to the project
  controls, and switching either one updates the active runtime immediately.
- Validation passed:

```bash
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml --check
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -q desktop_smoke
cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml -q
corepack pnpm --dir apps/desktop build
```

GUI screenshot smoke is implemented for the desktop frontend:

- `apps/desktop/playwright.config.ts` starts the built Vite preview server.
- `apps/desktop/tests/desktop-ui-smoke.spec.ts` checks the desktop shell,
  diagnostics panel, provider/model selectors, Settings drawer, and mobile
  composer layout.
- The smoke asserts that the app has no horizontal overflow and that the core
  desktop sections keep a stable vertical stack.
- The test writes desktop, settings-drawer, and mobile screenshots under
  `apps/desktop/test-results/`, which is intentionally gitignored.
- A native `tauri dev` launch caught and fixed a missing Tauri event-listen
  capability. The main window now starts without the previous
  `event.listen not allowed` runtime error.
- Native whole-screen capture works through `screencapture`; exact window
  cropping is blocked until macOS Accessibility permission is granted for the
  automation process.
- Validation passed:

```bash
corepack pnpm --dir apps/desktop build
corepack pnpm --dir apps/desktop test:ui-smoke
corepack pnpm --dir apps/desktop tauri dev
```

Native dev project-root detection is fixed:

- The desktop backend now resolves the default project from
  `PRIORITY_AGENT_DESKTOP_PROJECT_DIR` when set, then falls back to walking up
  from the current process directory until it finds the repo root
  (`.git` + `Cargo.toml`).
- This prevents `tauri dev` from defaulting to
  `apps/desktop/src-tauri` just because the native process starts there.
- If an older desktop settings file already stored `apps/desktop` or
  `apps/desktop/src-tauri`, startup migrates it back to the repo root.
- Native `tauri dev` verification now shows project access for
  `/Users/georgexu/Desktop/rust-agent`.
- Validation passed:

```bash
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml --check
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -q desktop_smoke
cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml -q
corepack pnpm --dir apps/desktop build
corepack pnpm --dir apps/desktop test:ui-smoke
corepack pnpm --dir apps/desktop tauri dev
```

Codex-like desktop workbench foundations are implemented:

- The desktop UI has been visually reworked toward a denser Codex-like
  workbench: tighter sidebar spacing, mature topbar chrome, more deliberate
  empty state, clearer transcript hierarchy, and a higher-density Settings
  drawer.
- The transcript now renders agent run activity as first-class timeline cards
  instead of plain tool log rows.
- Tool events are grouped by tool id, with specialized cards for shell
  validation, file edits/patches, diff previews, long output previews, failed
  tools, permission requests, and usage/completion events.
- Permission cards can be approved/rejected directly from the timeline, and
  timeline cards can open the trace drawer on the corresponding debug event.
- Desktop run/session ergonomics now include new chat, persisted active
  session restore, session rename, real SQLite/FTS-backed session search,
  archive/delete, recent project shortcuts, project switching, and clearer
  startup state in Settings.
- Validation passed:

```bash
cargo fmt --check
corepack pnpm --dir apps/desktop build
cargo test -q session_store::tests::test_search_sessions_matches_title_and_message_fts
cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml -q
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -q desktop_smoke
corepack pnpm --dir apps/desktop test:ui-smoke
cargo check -q
git diff --check
```

Current product assessment:

- The desktop app has crossed from scaffold into usable MVP. It can run the
  real Rust runtime, stream agent activity, resume sessions, manage common
  session/project state, show diagnostics, and package locally.
- The UI is not yet mature commercial-software quality. It is good enough to
  continue using as the primary development surface, but it still needs focused
  product design work before it should be considered a polished user-facing
  release.
- Desktop remains the product priority. Mobile, plugins, automations,
  marketplace, cloud sync, and broad generic surfaces should stay deferred
  until the desktop workflow feels excellent for daily coding.

### Next Desktop UI And Frontend Maturity Stage

Goal: move the desktop app from "functional Codex-like MVP" to "credible
commercial desktop coding app". User adoption will depend heavily on visual
density, interaction clarity, and trust in the interface, so this stage should
prioritize UI quality as a product feature, not cosmetic cleanup.

#### Track A - Visual Density And Layout Maturity

- Refine the window chrome, topbar, sidebar, transcript, composer, and Settings
  drawer as one coherent desktop layout system.
- Make spacing, type scale, icon sizing, borders, hover states, selected states,
  disabled states, and section hierarchy consistent across the app.
- Keep the app dense and work-focused: avoid marketing-page composition,
  oversized decorative areas, nested cards, and vague empty filler.
- Improve the empty state so it feels like the real starting point of a coding
  workbench: current project, provider/model, diagnostics summary, recent
  session affordances, and a clear composer path.
- Tighten responsive behavior for small MacBook windows and mobile-width smoke
  without making the desktop UI feel sparse.

Acceptance:

```bash
corepack pnpm --dir apps/desktop build
corepack pnpm --dir apps/desktop test:ui-smoke
```

Also review screenshots in:

```text
apps/desktop/test-results/
```

#### Track B - Transcript And Timeline Product Polish

- Make assistant messages, user messages, timeline events, tool cards, and
  final answers visually distinct without increasing clutter.
- Improve timeline card hierarchy for common real runs: command, path, file
  diff, validation status, failure reason, permission decision, and trace link.
- Add better empty/running/completed/failed states for the run timeline.
- Keep trace details available in the drawer, but make the main transcript
  enough for normal use.
- Defer stop/retry until the Rust runtime exposes reliable cancellation/retry
  controls.

Acceptance:

```bash
corepack pnpm --dir apps/desktop build
corepack pnpm --dir apps/desktop test:ui-smoke
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -q desktop_smoke
```

Status: started. The main run timeline card now carries a dedicated run summary
for runtime connection, tool progress, permission waits, completion, and failure
states, so normal transcript reading no longer depends on opening trace first.
It also aggregates multi-tool runs into compact stats for tool count, failures,
file changes, validations, and recovery guidance. Assistant replies that follow
run/tool activity are now marked as final answers, giving conclusions a clearer
visual rhythm after the process timeline. Low-value usage and trace controls are
now visually quieter so the main transcript reads less like a debug log.
Successful shell/validation tools are compact, while failures, file diffs, and
permissions stay expanded as high-value cards. Consecutive timeline items now
carry run boundary and run-step classes so long runs scan as one coherent
section before the final answer.

#### Track C - Session And Project Ergonomics

- Turn search into a more complete desktop interaction: keyboard focus,
  empty/no-result states, result highlighting, and faster switching.
- Add safer session management affordances: archive visibility, undo or restore
  for archived sessions, clearer delete confirmation, and selected-session
  status.
- Improve project switching with a real recent-project list, current-project
  metadata, and diagnostics after switching.
- Make startup restore explicit: user should know whether the app restored a
  previous session, opened a project with no active session, or needs provider
  setup.

Acceptance:

```bash
cargo test -q session_store
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -q desktop_smoke
corepack pnpm --dir apps/desktop test:ui-smoke
```

Status: started. Session search now shows result counts, highlights title/model
matches, and provides a clear-search control so filtered session lists recover
without manual text deletion. Archive now has an undo path backed by
`restore_archived_session`, so accidental archives can be recovered without
deleting session data. Delete now uses an in-app confirmation dialog with
session metadata instead of the browser-native confirm prompt. The sidebar and
Settings drawer now show a readable current-session state, so restored sessions
and new conversations are distinguishable without inspecting raw ids. Settings
now lists recent projects with full paths and direct switching controls instead
of only exposing a count. The main workspace now surfaces startup restore state
directly, distinguishing restored sessions from new-conversation launches.

#### Track D - Frontend Architecture And Test Hardening

- Add focused tests for `runEventState.ts` and session/search/archive/delete
  state transitions.
- Keep `App.tsx` as a coordinator, but split more UI-specific state and panels
  when the component starts carrying too many workflows.
- Prefer stable component props and typed API wrappers over ad hoc state
  coupling between Sidebar, Transcript, Composer, Settings, and TraceDrawer.
- Add screenshot assertions or visual diff thresholds once the UI stops
  changing rapidly.
- Add a real native WebView/Tauri automation path so Chromium preview smoke is
  not the only frontend confidence gate.

Acceptance:

```bash
corepack pnpm --dir apps/desktop build
corepack pnpm --dir apps/desktop test:ui-smoke
cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml -q
cargo check -q
```

Status: started. `apps/desktop/tests/run-event-state.spec.ts` now covers the
core pure state transitions for user submit, run completion, assistant delta
coalescing, shell/file timeline summaries, permission answers, and session
message loading. `apps/desktop/tests/desktop-api-state.spec.ts` now covers the
web-preview API state flow for session search, rename, archive, delete, project
selection, and new-conversation startup recovery.

#### Track E - Release-Quality Desktop Hardening

- Add app-level crash/log diagnostics and a visible way to open diagnostic logs.
- Complete the formal macOS distribution path: Developer ID signing,
  notarization, DMG polish, first-run install notes, and update mechanism.
- Keep local/ad-hoc builds fast for development, but document the difference
  between dev builds, local signed builds, and release builds.
- Revisit permissions after more real desktop runs: approvals should feel
  predictable, scoped, and easy to audit.

Acceptance:

```bash
scripts/package-macos-app.sh --help
bash -n scripts/package-macos-app.sh
corepack pnpm --dir apps/desktop tauri build --bundles app,dmg
```

#### Recommended Execution Order

1. Track A first: visual density and layout maturity. This has the highest
   impact on whether the app feels worth using every day.
2. Track D in parallel where cheap: add focused frontend state tests before
   large UI rewrites make regressions hard to spot.
3. Track B next: transcript and timeline are the main product surface during
   real agent work.
4. Track C after the main layout settles: session/project ergonomics should
   feel integrated, not bolted onto the sidebar.
5. Track E before any external distribution: packaging polish matters only
   after the core desktop experience is strong.

### Phase 0 - Runtime Extraction

- Add `src/lib.rs` and turn `src/main.rs` into a thin binary entrypoint.
- Keep all current CLI/TUI/API behavior unchanged.
- Add `desktop_runtime` facade with a non-UI test that initializes enough state
  to prove the boundary compiles.
- Validate with:

```bash
cargo check -q
cargo test -q instructions
cargo test -q prompt_context
```

### Phase 1 - Tauri Shell

- Add `apps/desktop` Tauri 2 + React + TypeScript + Vite scaffold.
- Add commands:
  - `desktop_health`
  - `list_recent_sessions`
  - `select_project`
  - `send_message`
  - `answer_permission`
- Implement event streaming from Rust to frontend for one active run.
- Keep UI plain but usable.
- Validate with:

```bash
pnpm --dir apps/desktop build
cargo check -q
cargo check --features experimental-api-server -q
```

### Phase 2 - Codex-like MVP UX

- Build the left sidebar, centered empty state, bottom composer, session list,
  project selector, permission card, and compact tool progress rendering.
- Persist active project/session in the existing session store or a small
  desktop settings file under the app data directory.
- Keep final answers concise and show detailed evidence in a trace drawer.
- Add screenshot smoke checks once the dev server is stable.

### Phase 3 - macOS Packaging

- Add `scripts/package-macos-app.sh` that wraps the Tauri build command.
- Configure app identifier, product name, icons, minimum macOS version, and
  local resources.
- Build local `.app` first, then `.dmg`.
- Document dev/ad-hoc signing and Developer ID notarization requirements.

### Phase 4 - Product Hardening

- Add onboarding for provider config and project workspace selection.
- Add permission defaults suitable for a desktop app.
- Add local update path only after the signed app flow is stable.
- Add failure diagnostics for missing shell `PATH`, missing developer tools,
  and missing provider keys.

## Immediate Next Slice

The next concrete slice should start Track A:

1. Audit current desktop screenshots and list the highest-impact UI density
   problems in sidebar, topbar, transcript, composer, and Settings drawer.
2. Refactor CSS into clearer layout/component groups if needed before changing
   many visual rules.
3. Polish one full first-screen workflow: restored session or new chat empty
   state, diagnostics strip, transcript area, composer, and session sidebar.
4. Keep `corepack pnpm --dir apps/desktop test:ui-smoke` passing after each
   visual batch.

The immediate success criterion is not "looks prettier"; it is that the desktop
app feels like a focused, mature coding workbench when opened, before any agent
run starts.
