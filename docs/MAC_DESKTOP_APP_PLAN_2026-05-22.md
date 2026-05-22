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

The next concrete code slice should deepen Phase 1:

1. Add GUI-level Tauri automation once the native app test harness is stable;
   the current smoke covers the Rust command bridge and bundle path.
2. Add focused tests around `runEventState.ts` once a frontend test runner is
   introduced.
3. Add a model/provider selector that reflects the detected provider state.
4. Add GUI-level Tauri automation once the native app test harness is stable;
   the current smoke covers the Rust command bridge and bundle path.

The first shell, macOS `.app` bundle, session resume, and command smoke now
exist. The frontend is split, icons are reproducible, and local packaging has a
single script entrypoint. Desktop project/session state now persists across app
launches, and startup diagnostics now identify missing provider/tooling setup.
The settings drawer now exposes that state and provider setup entrypoints. The
first-run provider setup flow now guides shell profile setup without exposing
secrets. Permission defaults now persist and are applied to active and future
runtime sessions. The next useful work is a provider/model selector and a true
GUI automation path when we are ready to spend the disk/build time.
