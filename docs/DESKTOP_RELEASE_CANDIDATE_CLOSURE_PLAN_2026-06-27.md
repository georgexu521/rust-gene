# Desktop Release Candidate Closure Plan - 2026-06-27

Status: Implemented in current RC-closure slice; external RC evidence still
requires release credentials and operator-run gates

Owner: Liz / gex

Source: latest desktop-app risk review from gex, checked against the current
repository after commit `484bc1ea Harden desktop product runtime`.

## Executive Judgment

The review is useful, but several items are already fixed in the latest
desktop product-hardening slice. The remaining work is not another large
architecture rewrite. It is release-candidate closure: make the current
desktop app safer to ship, easier to recover, and clearer about what it does
and does not protect.

Current state:

- No early "big hole" remains in the reviewed desktop path.
- Stop run now reaches the product runtime boundary through
  `CancellationToken`, but provider SDKs and every individual tool runner can
  still add more graceful cancellation hooks.
- Run Review Accept is now persisted, not just dismissed.
- Diagnostics export redacts raw settings/log paths by default.
- Production Tauri CSP no longer allows broad `http://127.0.0.1:*`.
- macOS Keychain save support exists when available, with dotenv retained as
  the runtime activation mirror.
- Controlled validation is still not a sandbox, and should not be described as
  safe for arbitrary untrusted repositories.

The next phase should focus on the release boundary: signing/notarization,
secret-store completeness, cancellation depth, recovery evidence, and
untrusted-workspace policy.

## Implementation Update - 2026-06-27

Completed in this slice:

- Added cancellation propagation evidence to `turn_cancellation.v1`
  diagnostics so traces can distinguish `cancellable`,
  `drops_future_only`, and `external_process_killed` boundaries.
- Extended desktop credential backend semantics with backend health,
  provider backend status, delete, and dotenv-to-Keychain migration commands.
  Settings now exposes provider backend status, migrate-to-Keychain,
  credential delete, and explicit fallback-file reveal actions.
- Persisted active desktop run metadata and surfaced interrupted-run recovery
  in startup state and redacted diagnostics bundles.
- Made workspace execution policy visible in desktop diagnostics with
  `validation_security=controlled_not_sandboxed`.
- Fixed Composer context chip identity so multiple `@file`/symbol contexts can
  coexist without replacing every file context by type.
- Added workbench changed-file metadata and Composer ranking so `@file`
  suggestions can prioritize currently changed files while still surfacing
  symbol matches with selected line ranges.
- Added a native RC failure-path smoke mode for cancel, permission reject,
  redacted diagnostics export, force reset, and Run Review Accept persistence;
  the macOS release wrapper can include it as optional RC evidence.
- Connected the existing dependency audit script into the macOS release
  wrapper as an opt-in RC evidence gate.
- Added `docs/DESKTOP_UNTRUSTED_WORKSPACE_SANDBOX_DESIGN_2026-06-27.md`.

Deferred to operator/external RC gates:

- Real Developer ID signing, notarization, stapling, and Gatekeeper evidence.
- Actual migration of user credentials on a macOS machine with Keychain access.
- Supply-chain audit execution when `cargo-audit` and `cargo-deny` are
  installed; current local run reports both tools missing.
- A true sandbox/container backend for untrusted repositories.

## Repo-Backed Triage Of The New Feedback

### Already Resolved In The Current Code

1. Stop run is no longer only a stream-loop flag.
   - `apps/desktop/src-tauri/src/lib.rs` stores a `CancellationToken` in
     `DesktopRunHandle`.
   - `src/desktop_runtime/mod.rs`, `src/engine/runtime_controller.rs`, and
     `src/engine/streaming.rs` pass cancellation into the full-agent stream.
   - `src/engine/conversation_loop/validation_runner.rs` has a cancel-aware
     child-process helper and a long-command cancellation test.

2. Run Review Accept is no longer only frontend dismiss.
   - `apps/desktop/src-tauri/src/run_review_commands.rs` writes sanitized
     acceptance events to `run-review-acceptances.jsonl`.
   - The frontend calls `acceptRunReview(...)`; Dismiss remains local UI
     dismissal.

3. Diagnostics export no longer exposes raw settings/log paths by default.
   - `DesktopDiagnosticsRedaction.include_full_paths` defaults to false.
   - Settings and diagnostics paths are represented by basename/hash
     descriptors unless the user explicitly opts into full paths.

4. Production CSP no longer contains the broad localhost wildcard.
   - `apps/desktop/src-tauri/tauri.conf.json` uses `connect-src ipc:`.
   - `scripts/check_desktop_release_security.sh` guards against regression.

5. Daily / Engineering / LabRun modes are now real enough for the next beta.
   - Daily hides noisy timeline/runtime elements by default.
   - Engineering exposes deeper runtime inspection.
   - LabRun includes the governance surface and role timeline graph.

6. First-run onboarding can complete provider setup inline.
   - The wizard includes provider/model selection, key entry, save/test
     feedback, and credential-storage acknowledgement.

### Still Valid, But Now Narrower

1. Cancellation should continue downward into provider/tool-specific cleanup.
   - Current behavior is materially better: cancelling drops the active turn
     future and emits cancelled closeout.
   - Remaining work: provider SDK request handles, bash/tool processes, and
     long-running tool controllers should each observe cancellation directly
     where possible.

2. Keychain support should become a full credential backend, not only a save
   path plus dotenv mirror.
   - macOS save path exists.
   - Remaining work: load/status/delete/migrate from Keychain; then Linux
     Secret Service and Windows Credential Manager.

3. Controlled validation is still not sandboxing.
   - This should remain a clear product boundary.
   - Untrusted repositories need an isolated execution backend before the app
     can safely auto-run tests/package scripts there.

4. macOS release delivery still needs real signed/notarized evidence.
   - The checklist and release wrapper exist.
   - Remaining work: use a real Developer ID identity, notarize, staple,
     verify Gatekeeper, and record evidence under `docs/rc/`.

## Target Release Candidate Invariants

Before a public macOS beta/release candidate:

- Stop run produces a clear cancelled closeout and does not leave the desktop
  guard stuck.
- Provider/tool cancellation is best-effort but explicit in logs and trace.
- Run Review acceptance can be used as durable user-review evidence.
- Diagnostics bundles are share-safe by default.
- Production CSP is locked to IPC plus explicitly required local resources.
- API keys are saved in macOS Keychain when available, with dotenv used only
  as a documented runtime mirror/fallback.
- Opening an untrusted workspace does not silently run package scripts,
  build scripts, or arbitrary validation code.
- macOS build artifacts are signed/notarized or explicitly labeled as
  unsigned local builds.

## P0 - Release Candidate Blockers

### 1. Provider And Tool Cancellation Audit

Goal: make cancellation behavior explicit at every long-running boundary.

Plan:

- Add a cancellation propagation map covering:
  - provider request future
  - streaming engine
  - tool execution controller
  - bash/local process tools
  - required validation runner
  - desktop lightweight provider lane
- For each long-running boundary, record one of:
  - `cancellable`
  - `drops_future_only`
  - `not_cancellable`
  - `external_process_killed`
- Add trace/log events when a cancelled run drops a provider/tool future.
- Add tests/smokes:
  - cancel before provider response
  - cancel while permission is pending
  - cancel while synthetic long local process is running
  - duplicate-run guard clears after cancel

Acceptance criteria:

- `/trace` or desktop diagnostics can explain what was cancelled and what was
  only dropped best-effort.
- UI closeout says `cancelled`, not generic failed, for user-initiated stop.
- A cancelled run does not block the next run.

### 2. Complete macOS Keychain Backend Semantics

Goal: make macOS Keychain a real desktop credential backend.

Plan:

- Extend `DesktopCredentialStore` with:
  - `load_status`
  - `delete`
  - `migrate_from_dotenv`
  - `backend_health`
- Store provider keys under service `Priority Agent` and account
  `<provider_id>`.
- Keep dotenv as:
  - runtime activation mirror for current provider loading
  - explicit fallback when Keychain is unavailable
- Add a Settings action:
  - migrate dotenv credentials to Keychain
  - delete provider credential
  - reveal dotenv fallback path only behind explicit debug action
- Add tests with mocked stores for:
  - Keychain available -> preferred backend
  - Keychain save failure -> surfaced error
  - fallback path still works when Keychain unavailable
  - migration availability is reported correctly

Acceptance criteria:

- macOS users can save/delete provider keys through the desktop backend.
- UI clearly says whether the active store is Keychain or dotenv fallback.
- No test invokes the real system Keychain.

### 3. macOS Signing And Notarization Evidence

Goal: turn macOS-first from documentation into a repeatable RC gate.

Plan:

- Fill in `docs/DESKTOP_MACOS_RELEASE_CHECKLIST_2026-06-26.md` with actual
  signing inputs:
  - Developer ID Application identity
  - hardened runtime setting
  - entitlements
  - notarization profile
- Run `scripts/desktop-macos-release.sh` on an RC commit.
- Record evidence under `docs/rc/desktop-macos-release-<timestamp>.md`:
  - commit hash
  - bundle path
  - dmg path
  - SHA256
  - signing identity
  - notarization request/status
  - Gatekeeper verification output
  - native smoke result

Acceptance criteria:

- A real `.dmg` can be verified as signed and notarized, or the RC is
  explicitly labeled unsigned and not public-release-ready.
- Release evidence is committed or archived in `docs/rc/`.

### 4. Untrusted Workspace Execution Policy

Goal: stop users from accidentally treating controlled validation as sandboxed
execution.

Plan:

- Add a desktop workspace trust banner/state for untrusted repositories:
  - package scripts: ask
  - shell validation: ask
  - LabRun daemon: off
  - Developer Auto: off
- For untrusted workspaces, require explicit confirmation before:
  - `npm test`, `pnpm test`, `yarn test`
  - `cargo test` when `build.rs` or proc-macro crates are present
  - pytest/import-heavy Python validation
  - repository shell scripts
- Add proof labels:
  - `validation_security=controlled_not_sandboxed`
  - `workspace_trust=trusted|ask|untrusted`
  - `package_script=ask|allowed|blocked`
- Keep sandbox/container execution as a future backend, not a hidden promise.

Acceptance criteria:

- Desktop and LabRun proof surfaces clearly distinguish controlled validation
  from sandboxed validation.
- Unknown workspace package scripts are not silently auto-run by default.

## P1 - Beta Reliability

### 5. Restart Recovery Evidence

Goal: make interrupted desktop runs understandable after app restart.

Plan:

- Persist active-run metadata:
  - run id
  - session id
  - provider/model
  - started_at
  - last_event_at
  - cancellation state
  - latest closeout if known
- On startup, show a recovery card when the previous run did not close cleanly:
  - Resume session
  - Mark cancelled
  - Export diagnostics
  - Force reset local guard
- Add native smoke:
  - restart while run active
  - restart after cancelled run

Acceptance criteria:

- A crash/restart cannot leave users guessing whether work is still active.
- Recovery actions are logged.

### 6. `@file` Mention Quality Pass

Goal: make the current `@file` feature feel deliberate rather than only
functional.

Plan:

- Improve ranking:
  - exact basename
  - path segment match
  - recent files / changed files
  - symbol name match
- Support multi-select without replacing existing context chips.
- Show selected file/symbol context in the Context Detail drawer.
- Add Playwright coverage for:
  - typing `@`
  - selecting a file
  - selecting a symbol
  - attached context chip includes path/line

Acceptance criteria:

- `@file` is useful in daily work without opening the file picker manually.

### 7. Desktop RC Native Smoke Expansion

Goal: cover realistic beta-user failure paths.

Plan:

- Add native smoke cases:
  - provider unavailable
  - permission rejected
  - cancel mid-provider
  - cancel mid-local-process
  - restart while active run
  - diagnostics export after error
  - Run Review accept and persisted reload
- Keep each smoke deterministic and bounded by timeout.

Acceptance criteria:

- RC evidence includes both happy-path and failure-path desktop behavior.

## P2 - Supply Chain And Long-Term Security

### 8. Dependency And Supply Chain Gate

Goal: move from manual dependency review toward a formal supply-chain gate.

Plan:

- Add or document:
  - `cargo audit` or `cargo deny`
  - license allow/deny policy
  - SBOM generation
  - workflow action pinning policy
  - secret scanning gate
- Keep automation low-noise; avoid re-enabling Dependabot branch spam unless
  there is a triage workflow.

Acceptance criteria:

- Release checklist includes dependency vulnerability and license evidence.

### 9. Future Sandbox Backend

Goal: define the backend needed for truly untrusted repositories.

Plan:

- Evaluate:
  - container runtime
  - macOS sandbox profile
  - network disabled
  - read-only host mounts except workspace scratch
  - no inherited provider secrets
  - CPU/memory/process/time limits
- Add design doc before implementation.

Acceptance criteria:

- Product docs never imply current validation is a sandbox.
- There is a clear path to a future untrusted-workspace execution backend.

## Non-Goals

- Do not add new agent roles or LabRun stages in this phase.
- Do not weaken permission gates to make release smoke easier.
- Do not re-enable broad dependency bots without a triage policy.
- Do not call the desktop app public-release-ready without signed/notarized
  macOS evidence.
- Do not describe controlled validation as sandboxing.

## Suggested Implementation Order

1. Provider/tool cancellation audit and cancellation trace evidence.
2. macOS Keychain load/delete/migration semantics.
3. Untrusted workspace execution policy tightening.
4. Restart recovery metadata and smoke.
5. Run signed/notarized macOS release candidate and record evidence.
6. `@file` ranking/multi-select polish.
7. Supply-chain gates and sandbox design.

## Validation Plan

Narrow gates:

```bash
cargo fmt --check
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml --check
cargo check -q
cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml -q
cargo test -q controller_cancel_token_produces_cancelled_stream_events
cargo test -q shell_output_with_timeout_trace_and_cancel_interrupts_long_command
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -q -- --test-threads=1
corepack pnpm --dir apps/desktop build
corepack pnpm --dir apps/desktop test:ui-smoke
bash -n scripts/desktop-native-smoke.sh
bash -n scripts/desktop-macos-release.sh
git diff --check
bash scripts/desktop-native-smoke.sh --rc-failure-check --no-screenshot --timeout 180
```

Release-candidate gates:

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo doc --workspace --all-features --no-deps
bash scripts/validate_docs.sh
bash scripts/check_desktop_release_security.sh
bash scripts/security_dependency_audit.sh
bash scripts/desktop-macos-release.sh
```

Native smoke should include cancellation, provider failure, permission
rejection, restart recovery, diagnostics export, and Run Review acceptance.
