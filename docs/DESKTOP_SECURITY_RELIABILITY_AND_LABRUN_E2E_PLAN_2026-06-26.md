# Desktop Security, Reliability, And LabRun E2E Hardening Plan - 2026-06-26

Status: Implemented for P0/P1 baseline; P2 remains future product maturity

Owner: Liz / gex

Source: latest review notes on desktop readiness and LabRun child-agent
boundaries, checked against the current repository on 2026-06-26.

## Executive Judgment

The suggestions are useful and should be adopted. The core runtime and LabRun
execution boundary no longer show the earlier high-risk holes, but the next
release-readiness gap has shifted to two areas:

1. LabRun child-agent scope needs a live-path end-to-end proof, not only unit
   tests around binding and policy functions.
2. The desktop app should move from a capable workbench to a safer product
   entrypoint with conservative defaults, visible state, backend run guards, and
   explicit desktop CI.

This plan should be treated as the next focused stream after
`LABRUN_SUBAGENT_SCOPE_VALIDATION_PROFILE_HARDENING_PLAN_2026-06-26.md`.

The work should not add new agent roles or broaden LabRun behavior. The goal is
to prove existing hard boundaries through real execution paths, then harden the
desktop surface that users will touch first.

## Implementation Update - 2026-06-26

Implemented in this slice:

- Added LabRun child-scope live-path tests through the real tool execution gate:
  out-of-scope Graduate child `file_write` and shell-style mutation attempts are
  denied before mutation, leave `README.md` unchanged, and persist
  `labrun_policy_blocked` proof events tied to the active GraduateTask and
  dispatch.
- Replaced the desktop Tauri `csp = null` setting with a conservative CSP for
  self scripts/styles, local IPC/connect targets, local/data images, and no
  object/frame embedding.
- Changed the desktop first-run/missing permission default to `auto_low_risk`
  while preserving explicit `auto` as deliberate Developer Auto.
- Scoped `open_file_path` to the selected project, app settings/data roots,
  diagnostic logs, and selected-project `.priority-agent/lab` paths, with
  canonicalized traversal rejection and tests.
- Added visible desktop credential-storage disclosure near the Provider key
  save action, including an "Open settings folder" control. Dotenv remains the
  current storage backend; system keychain support remains P2.
- Added a Tauri backend single-run guard, cancellation command, and force-reset
  command so duplicate desktop submissions cannot create parallel streams.
- Made Lab daemon automatic supervision opt-in, persisted as a desktop setting,
  visible in Settings with last supervision, last result, and next scheduled
  supervision state. Manual supervision remains available from the LabRun panel.
- Added a dedicated desktop CI job for the separate Tauri workspace: frontend
  build, Playwright UI smoke, Tauri Rust tests, and push-only native smoke.
- Neutralized desktop web-preview fixtures and Playwright expectations away
  from personal paths and specific provider/model examples.

Deferred intentionally:

- First-run wizard, project-trust wizard, system keychain/secret-store backend,
  true sandbox/container validation for untrusted workspaces, redacted
  diagnostics export, and SQLite-authoritative LabStore transactions.

Validation completed:

```bash
cargo fmt --check
git diff --check
cargo test -q labrun_child_ --lib -- --test-threads=1
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -q -- --test-threads=1
corepack pnpm --dir apps/desktop build
corepack pnpm --dir apps/desktop test:ui-smoke
bash scripts/validate_docs.sh
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features -- --test-threads=1
cargo doc --workspace --all-features --no-deps
```

## Repo-Backed Observations

The review matches the current repository in several important places:

- `src/lab/execution_binding.rs` and `src/lab/policy_overlay.rs` now carry and
  enforce a typed `LabExecutionBinding`, but existing coverage is still mostly
  focused on binding parsing, scope checks, policy review, and orchestrator
  paths. We should add a true tool-path E2E test that proves out-of-scope child
  `file_write` and shell-style mutation are blocked before mutation.
- Controlled validation now records `validation_security =
  controlled_not_sandboxed`, which is honest and correct. It is not a container
  sandbox, so the project should keep that boundary explicit instead of trying
  to solve untrusted-code execution with more regex rules.
- `apps/desktop/src-tauri/tauri.conf.json` still has `app.security.csp = null`.
  That is too loose for a desktop UI that renders model output, tool output,
  diffs, reports, diagnostics, and artifact content.
- `apps/desktop/src-tauri/src/desktop_state.rs` defaults missing desktop
  permission mode to `auto`, and `parse_desktop_permission_mode("auto")` maps to
  `PermissionMode::AutoAll`. That is acceptable for CLI dogfood but too
  aggressive for the first desktop launch.
- `apps/desktop/src-tauri/src/lib.rs::open_file_path` opens a path's parent if
  it exists, without checking the selected project, app data directory, export
  directory, LabRun artifact directory, or diagnostics directory.
- Desktop provider credentials currently call
  `priority_agent::services::api::credentials::save_credential`, which persists
  to the local dotenv path. That should be visible in the desktop UI before a
  formal desktop release, and later replaced by a platform secret-store adapter.
- The React app supervises the Lab daemon on startup and every 120 seconds.
  That is useful for recovery, but it should become an explicit, visible
  desktop setting with last/next supervision status.
- The desktop backend `send_message` path does not currently expose a clear
  single-run guard, cancellation token, or force-reset command at the Tauri
  command boundary.
- `apps/desktop/src/runtime/desktopApi.ts` and several Playwright fixtures still
  contain local preview values such as personal absolute paths and specific
  provider/model examples. These are test/preview data, not runtime secrets, but
  they should be neutralized before treating desktop as a product entrypoint.
- The root CI has strong Rust gates and now includes dependency security plus a
  focused macOS job, but `apps/desktop` is a separate Tauri workspace. Desktop
  build, UI smoke, Tauri Rust tests, and native smoke should be their own CI job
  instead of only ad hoc or release-smoke checks.

## Target Invariants

After this plan is complete:

- A Graduate child agent with `allowed_scope = ["src/lab"]` cannot write
  `README.md` through `file_write`, `file_edit`, `file_patch`, or an ordinary
  shell mutation path.
- An out-of-scope child mutation leaves the target file unchanged and writes a
  durable `labrun_policy_blocked` proof event tied to the active task/dispatch
  binding.
- Desktop starts in a conservative permission mode unless the user explicitly
  chooses developer auto.
- Desktop renders with a real Tauri CSP, with exceptions added intentionally.
- Desktop path-opening commands are scoped to the selected project and known app
  data/diagnostic/export/LabRun locations.
- Desktop credential storage behavior is explicit to the user, even before
  system keychain support lands.
- Only one backend desktop run can be active at a time unless a future design
  explicitly supports multi-run streams.
- Lab daemon supervision is visible, configurable, and not a hidden state
  mutation loop.
- Desktop CI proves the Tauri workspace and frontend do not silently drift away
  from the root Rust runtime.

## P0 - Hard Proof And Desktop Security Defaults

### 1. Add LabRun Child-Agent Live E2E Scope Tests

Goal: prove the binding-based child-agent boundary through real tool execution,
not only policy helper functions.

Required behavior:

- Create a GraduateTask with `allowed_scope = ["src/lab"]`.
- Dispatch a child execution with a valid `LabExecutionBinding`.
- Attempt child `file_write("README.md", ...)`.
- Assert the tool is rejected before mutation.
- Assert `README.md` content is unchanged.
- Assert a `labrun_policy_blocked` event is persisted with:
  - `lab_run_id`
  - `active_graduate_task_id`
  - `active_dispatch_id`
  - `action_family`
  - blocked path
  - binding-derived allowed scope

Add a second shell-style scenario:

- GraduateTask scope remains `src/lab`.
- Child execution attempts to write outside scope through a shell command.
- Expected result is either pre-execution block, or a clear ask/block outcome
  when the shell command cannot prove its write target is in scope.
- The out-of-scope file must remain unchanged.

Implementation notes:

- Prefer a deterministic test provider or direct tool-executor path that still
  invokes the real tool boundary. The test must not stop at
  `review_labrun_tool_action(...)`.
- If full agent streaming is too slow or brittle, build a focused harness around
  `ToolContext`, the real tool implementation, and the LabRun policy overlay.
  The key is that the actual mutation-capable tool path is exercised.
- Add a regression case for tools whose `input_paths()` are incomplete. A
  mutation-capable tool with no path evidence should fail closed under
  `lab-graduate` binding.

Suggested files:

- `tests/labrun_child_scope_e2e.rs`, or
- focused tests near `src/lab/policy_overlay.rs` plus a real tool invocation
  harness if integration-test setup is too heavy.

### 2. Set A Conservative Tauri CSP

Replace `csp = null` in `apps/desktop/src-tauri/tauri.conf.json`.

Initial target:

```json
"security": {
  "csp": "default-src 'self'; img-src 'self' asset: data:; style-src 'self' 'unsafe-inline'; script-src 'self'; connect-src ipc: http://127.0.0.1:*; object-src 'none'; frame-ancestors 'none';"
}
```

Required behavior:

- Start with the conservative CSP above.
- Run desktop build and UI smoke.
- Add only the exceptions proven necessary by tests.
- Document any exception in a short comment or security note.

### 3. Change Desktop First-Run Permission Default

Current behavior:

- missing desktop permission mode normalizes to `auto`
- `auto` maps to `PermissionMode::AutoAll`

Target behavior:

- Missing or first-run permission mode should normalize to `auto_low_risk`, or
  to `default` if we choose stricter "ask every time" first launch.
- Existing explicit user settings should be respected.
- "Developer auto" should remain available but must be a deliberate user
  selection.

Required tests:

- `normalized_permission_mode_label(None)` returns the new conservative
  default.
- Explicit `"auto"` still maps to developer auto.
- Persisted existing settings are not silently rewritten except through the
  normal settings-save path.

### 4. Scope Desktop `open_file_path`

Replace broad `open_file_path(path)` behavior with a state-aware scoped command.

Target command shape:

```rust
async fn open_project_file_path(path: String, state: State<'_, DesktopAppState>) -> Result<(), String>
```

Allowed roots:

- selected project root
- app settings/data directory
- diagnostic logs directory
- explicit session export output directory
- `.priority-agent/lab` under the selected project

Required behavior:

- Canonicalize the requested path and allowed roots before comparison.
- Open the file when the file exists, otherwise open the nearest allowed parent
  only when the requested path is still under an allowed root.
- Reject arbitrary absolute paths outside allowed roots.
- Keep a clear user-facing error message.

Required tests:

- selected-project file opens.
- selected-project LabRun artifact path opens.
- diagnostics path opens.
- `/tmp/outside` or another unrelated absolute path is rejected.
- path traversal through `..` is rejected after canonicalization.

### 5. Make Desktop Credential Storage Explicit

Short-term target:

- In the provider credential UI, state that keys are currently saved to the
  local Priority Agent dotenv file, not to the OS keychain.
- Add a button or link to open the settings folder.
- Keep the security warning concise and visible near the save action.

Medium-term target:

- Add a credential-store abstraction with a desktop backend for:
  - macOS Keychain
  - Linux Secret Service
  - Windows Credential Manager
- Keep dotenv as CLI-compatible fallback, not the preferred desktop storage.

## P1 - Desktop Runtime Reliability And CI

### 1. Add Backend Single-Run Guard

The frontend already checks `runState.isRunning`, but the Tauri backend should
also defend itself.

Target design:

```rust
struct DesktopAppState {
    active_run: Mutex<Option<DesktopRunHandle>>,
}
```

Required behavior:

- `send_message` rejects or queues a second run while one is active.
- `send_message` clears `active_run` on success, failure, cancellation, or
  panic-safe cleanup path.
- Add `cancel_run` and `force_reset_run` Tauri commands.
- Emit visible run-state events when duplicate submission is blocked or a run is
  cancelled.

Required tests:

- two concurrent `send_message` calls cannot create two active streams.
- cancellation releases the guard.
- failed run releases the guard.
- force reset can recover from stale state.

### 2. Make Lab Daemon Supervision Visible And Configurable

Current behavior is useful but too implicit for desktop product use.

Target behavior:

- Add a desktop setting:
  - `lab_daemon_supervision_enabled`
- Show in UI:
  - enabled/disabled
  - last supervision time
  - last supervision result
  - next scheduled supervision time
- Startup supervision should respect this setting.
- The manual "Supervise Lab daemon" action remains available.

Required tests:

- disabled setting prevents automatic startup/interval supervision.
- manual supervision still works.
- status snapshot reflects last result.

### 3. Add Desktop CI Job

Root CI should explicitly cover the separate Tauri workspace.

Suggested job:

```yaml
desktop:
  runs-on: macos-latest
  steps:
    - checkout
    - install Rust
    - setup Node/corepack
    - corepack pnpm --dir apps/desktop install --frozen-lockfile
    - corepack pnpm --dir apps/desktop build
    - corepack pnpm --dir apps/desktop test:ui-smoke
    - cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml
    - bash scripts/desktop-native-smoke.sh
```

If native smoke is too slow for every PR:

- run build, UI smoke, and Tauri Rust tests on PR.
- run native smoke on push to `main` and release candidate tags.

### 4. Neutralize Desktop Preview Fixtures

Replace local personal/demo values in preview-only code and tests.

Targets:

- `apps/desktop/src/runtime/desktopApi.ts`
- `apps/desktop/tests/*.spec.ts`
- native smoke text that assumes `rust-agent` where a neutral sample project
  name is clearer

Preferred fixture values:

- `/Users/example/projects/priority-agent-demo`
- `sample-provider`
- `sample-model`
- `priority-agent-demo`

Better long-term shape:

- move preview data into `fixtures/desktop-preview.json`
- keep tests asserting behavior, not gex-specific machine paths

## P2 - Product Maturity And Larger Governance

These items are useful, but should not block the P0/P1 desktop hardening slice.

- First-run wizard:
  - select project
  - choose provider
  - choose permission mode
  - explain credential storage
- Project trust wizard:
  - package scripts
  - bash validation
  - LabRun daemon supervision
  - workspace trust source
- Redacted runtime diagnostics export:
  - settings summary
  - provider/model metadata without secrets
  - desktop logs
  - recent run events
  - LabRun proof summaries
- True desktop secret-store backend:
  - Keychain / Secret Service / Credential Manager
- Container or OS sandbox for untrusted validation:
  - network off by default
  - mounted workspace only
  - sanitized environment
  - CPU/memory/process limits
- SQLite-authoritative LabStore transactions:
  - JSON artifacts remain human-readable mirrors
  - task/dispatch/artifact/gate/event state changes become transactional

## Non-Goals For This Slice

- Do not redesign LabRun roles.
- Do not add new agent capabilities.
- Do not claim validation is a sandbox.
- Do not replace the entire desktop UI.
- Do not change CLI permission defaults while changing desktop defaults.
- Do not require system keychain support before making the current dotenv
  behavior visible and honest.

## Suggested Implementation Order

1. Add the LabRun child-agent live E2E tests and fix any tool-path gaps they
   expose.
2. Set Tauri CSP and run desktop build/smoke.
3. Change desktop first-run permission default and tests.
4. Scope `open_file_path` and update frontend API name/calls.
5. Add desktop credential-storage disclosure.
6. Add backend single-run guard and cancellation/reset commands.
7. Add daemon supervision setting/status.
8. Add desktop CI job.
9. Neutralize preview fixtures.
10. Update `docs/PROJECT_STATUS.md`, `docs/PROJECT_MAP.md`, and
    `docs/README.md` after implementation.

## Validation Plan

Run the narrow tests first:

```bash
cargo test -q lab::execution_binding --lib -- --test-threads=1
cargo test -q lab::policy_overlay --lib -- --test-threads=1
cargo test -q tools::agent_tool --lib -- --test-threads=1
cargo test -q tools::file_tool --lib -- --test-threads=1
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -q
corepack pnpm --dir apps/desktop build
corepack pnpm --dir apps/desktop test:ui-smoke
```

Then broaden:

```bash
cargo fmt --check
git diff --check
cargo check -q
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features -- --test-threads=1
bash scripts/check_source_file_sizes.sh
bash scripts/validate_docs.sh
```

For desktop release-candidate confidence:

```bash
bash scripts/desktop-native-smoke.sh
corepack pnpm --dir apps/desktop tauri build
```

## Release-Readiness Impact

Completing this plan would materially improve release trust because it covers
the remaining gap between "the runtime core is hardened" and "the product entry
is safe enough for real users":

- LabRun scope is proven through a live tool path.
- Desktop does not launch in the most permissive mode by default.
- Model/tool output is rendered under a CSP.
- Native commands are path-scoped.
- Credential persistence is honest.
- Long-running desktop state is visible and cancellable.
- The separate desktop workspace is continuously tested.
