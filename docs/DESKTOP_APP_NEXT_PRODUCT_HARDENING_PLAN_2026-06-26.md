# Desktop App Next Product Hardening Plan - 2026-06-26

Status: Completed for the desktop product-hardening objective; release-track
signing, update, crash-reporting, and non-macOS secret stores remain follow-up
work

Owner: Liz / gex

Source: latest desktop-app review notes from gex, checked against the current
repository on 2026-06-26 after
`DESKTOP_APP_PRODUCT_MATURITY_NEXT_PLAN_2026-06-26.md` was implemented and
pushed.

## Executive Judgment

The feedback is useful. The previous desktop slice moved the app from a Tauri
shell into a real local agent workbench: first-run onboarding, project-scoped
workspace trust, visible run recovery, Run Review, redacted diagnostics export,
and macOS-first release docs now exist.

This new review is pointing at the correct next layer: product sharpness and
release-grade boundaries. The current desktop app is now dogfoodable, but it
still has a few places where UI semantics are ahead of runtime semantics:

- Daily mode exists, but it does not yet strongly reduce information density.
- Stop run is visible, but cancellation is still cooperative at the stream
  consumption layer.
- Run Review has an Accept button, but Accept is currently a frontend dismiss,
  not a persisted review event.
- Onboarding explains provider state and credential storage, but it does not
  yet let the user complete provider setup inline.
- Diagnostics export redacts secrets, but still includes raw settings and log
  paths.
- CSP is no longer `null`, but production still allows a broad
  `http://127.0.0.1:*` connection surface.
- Credential saving is honestly labeled as dotenv fallback, but system
  keychain support is not implemented.

The plan below should not add more cockpit panels. It should make the desktop
entrypoint quieter by default, more explicit when it accepts work, and safer to
ship as a macOS-first product.

## Implementation Closeout - 2026-06-27

Implemented in this slice:

- Daily is now the default desktop detail level. Daily hides nonessential
  timeline stats, trace buttons, facts, JumpBar, and the primary Runtime
  Inspector until the user asks for deeper evidence. Engineering and LabRun are
  explicit view modes in Composer and Settings.
- Daily-mode inspector actions now open the Runtime Inspector drawer instead
  of silently depending on a hidden primary inspector. The drawer no longer
  duplicates the primary inspector tab tree.
- Production Tauri CSP no longer allows the broad `http://127.0.0.1:*`
  wildcard. `scripts/check_desktop_release_security.sh` fails the release path
  if the wildcard returns.
- Run Review Accept is now a backend command. It writes a sanitized
  `run-review-acceptances.jsonl` event with run/session, changed files,
  validation status, permission summary, residual-risk count, trace refs, and
  tool-output refs. Dismiss remains local UI dismissal.
- Run Review cards now expose clearer evidence actions: `View diff` for
  changed-file rows and `View validation log` for validation rows.
- Onboarding now supports inline provider/model selection, key entry, save,
  refresh/test feedback, and credential-storage acknowledgement without forcing
  the user into Settings.
- Desktop diagnostics bundles redact raw settings/log paths by default. They
  retain basename/path hashes and include full paths only through explicit
  `include_full_paths`.
- Desktop log redaction now catches local paths embedded after prefixes such
  as `settings_path=/Users/...`.
- Desktop credential storage now has a small `DesktopCredentialStore` backend
  trait with mocked tests. The production backend prefers macOS Keychain when
  available and mirrors to the dotenv activation path so the current runtime
  can still use the provider. Non-macOS and unavailable-Keychain builds
  continue to report the dotenv fallback honestly.
- Stop run now owns a `CancellationToken` on the active desktop run. Full-agent
  turns pass that token through DesktopRuntime, RuntimeController, and
  StreamingQueryEngine; cancellation emits explicit runtime diagnostic,
  closeout, and error events. Lightweight desktop provider calls also select
  on the same token.
- Required validation process execution now has a cancel-aware helper that
  kills the child process group and returns `Interrupted`; a focused test
  proves a long validation command can be cancelled quickly.
- LabRun view now includes a compact Professor -> Postdoc -> Graduate ->
  Validation -> Postdoc audit -> Professor review timeline graph derived from
  the current LabRun snapshot.
- Composer now supports a practical `@file` entrypoint. Typing `@...` opens
  indexed file/symbol suggestions backed by the workbench symbol index, and
  selected items attach project file context with line metadata when available.
- A repeatable macOS release wrapper was added at
  `scripts/desktop-macos-release.sh`, recording release evidence under
  `docs/rc/`.

Release-track follow-ups still not claimed as complete:

- Individual provider SDKs and every tool runner can still add their own
  cooperative cancellation hooks for more graceful cleanup, but desktop Stop
  now reaches the product runtime boundary and drops the active turn future.
- Linux Secret Service and Windows Credential Manager backends are still
  future desktop release work.
- Auto-update channel, crash reporter integration, notarized public
  distribution evidence, and signed release provenance remain release-track
  follow-ups.
- `@file` can be expanded later with richer fuzzy ranking and multi-file
  selection; the current version is usable and connected to file/symbol
  context.

## Repo-Backed Findings

### 1. Information Density Is Still High

Current state:

- `detail_level` supports `coding` and `daily`.
- Settings and Composer expose the distinction.
- The main desktop surface still keeps the same major systems available:
  transcript, Run Review, JumpBar, Trace, Tool Output, Workbench, Runtime
  Inspector surfaces, Context Detail, Settings, Permission Card, GoalProgress,
  Composer, and LabRun.

Assessment:

The review is right. Daily mode is currently more of a label and preference
than a strong view contract. A new user still sees an engineering workbench
unless they know what to ignore.

### 2. CSP Needs Dev/Production Separation

Current state:

`apps/desktop/src-tauri/tauri.conf.json` uses:

```text
connect-src ipc: http://127.0.0.1:*
```

Assessment:

This is reasonable for Vite/dev flows, but too broad as a production desktop
default. Production should not keep an arbitrary localhost wildcard unless a
specific feature requires it.

### 3. Stop Run Is Cooperative

Current state:

- `cancel_run` sets `cancel_requested = true` on the active desktop run handle.
- The stream loop checks `desktop_run_cancel_requested(...)` only after the
  next stream event arrives.
- `force_reset_run` clears the desktop run guard and emits a frontend error,
  but it does not cancel an underlying provider request or long-running tool.

Assessment:

The review is correct. This is useful UI recovery, but not true cancellation.
If the provider request or tool execution is stuck before the next event, Stop
run may not interrupt it promptly.

### 4. Credentials Are Still Dotenv Fallback

Current state:

- `DesktopCredentialStorageStatus.active_store = "dotenv_fallback"`.
- `system_keychain_available = false`.
- UI discloses the fallback.

Assessment:

This is acceptable for alpha/dogfood because the app is honest. It is not yet
the expected storage model for a polished desktop app.

### 5. Onboarding Lacks Inline Provider Completion

Current state:

- `OnboardingWizard.tsx` shows provider ready/not configured.
- It explains that setup can be skipped.
- It does not inline provider/model selection, key paste, save, refresh, or
  connection test.
- Provider repair exists in Composer/Settings.

Assessment:

The review is right. First-run setup should allow a normal user to finish the
provider path without leaving onboarding.

### 6. Run Review Accept Is Not Persisted

Current state:

- Run Review shows Accept and Dismiss review.
- Both buttons call `onDismissRunReview(runId)`.
- No backend event records `run_review_accepted`.

Assessment:

This is the most important semantic mismatch in the UI. Accept should mean
"user reviewed and accepted this run outcome", not merely "hide the card".

### 7. Diagnostics Export Still Includes Raw Local Paths

Current state:

The diagnostics payload hashes the selected project path, but still includes:

- `settings_path`
- `diagnostic_logs_path`

Assessment:

The review is right. A support bundle intended for sharing should not expose
the user's home directory or local directory layout by default.

### 8. macOS-First Is Clear, But Not Yet Full Release Delivery

Current state:

- Tauri bundle targets are `app` and `dmg`.
- `DESKTOP_MACOS_RELEASE_CHECKLIST_2026-06-26.md` records build/smoke,
  signing/notarization, and known-limitations requirements.
- Signing, notarization, update channel, and crash-report packaging are not yet
  implemented.

Assessment:

This is the correct next release track, but it should remain macOS-first until
we have signed/notarized artifacts and update behavior validated.

## Target Product Invariants

After this plan is complete:

- Daily View is quiet by default: chat, essential tool progress, permissions,
  and Run Review are visible; deep runtime evidence is one click away.
- Engineering View exposes trace, tool output, runtime inspector, context
  budget, and workbench details.
- LabRun View focuses on Professor/Postdoc/Graduate state, proof, reports,
  daemon, and governance tasks.
- Production CSP does not allow arbitrary localhost wildcard connections.
- Stop run can propagate cancellation intent into runtime/provider/tool layers,
  not only the frontend stream loop.
- Accept in Run Review writes a durable event with session/run/proof metadata.
- First-run onboarding can complete provider selection, key saving, and a
  connection test inline.
- Diagnostics export is share-safe by default and includes raw local paths only
  through an explicit advanced debug opt-in.
- Desktop credential storage has a platform secret-store backend path, with
  dotenv preserved as a CLI-compatible fallback.

## P0 - Semantic And Safety Hardening

### 1. Persist Run Review Accept

Goal: make Accept a real product/runtime event.

Implementation plan:

- Add a desktop command:
  - `accept_run_review(run_id, session_id, summary)`
- Add a DTO:
  - `DesktopRunReviewAcceptance`
  - fields: `run_id`, `session_id`, `accepted_at`, `changed_files`,
    `validation_status`, `permission_summary`, `residual_risk_count`,
    `trace_refs`, `tool_output_refs`
- Persist the event in the session store or a desktop-specific event table/file.
- Update Run Review:
  - Accept calls `accept_run_review`.
  - Dismiss review stays frontend-only and is labeled as local dismissal.
- Add tests:
  - Accept writes a persisted event.
  - Dismiss does not write acceptance.
  - Reloaded session can show accepted review state.

Acceptance criteria:

- Accept and Dismiss no longer share the same semantics.
- A support/debug view can show that a run was accepted by the user.
- The persisted event does not contain secret payloads or full tool output.

### 2. Redact Diagnostics Paths By Default

Goal: make desktop diagnostics bundles safe to share without leaking usernames
or local directory structure.

Implementation plan:

- Replace raw `settings_path` and `diagnostic_logs_path` in redacted bundles
  with:
  - basename
  - path hash
  - parent hash
  - path kind, for example `settings` or `diagnostics`
- Add `include_full_paths: Option<bool>` to `DesktopDiagnosticsRedaction`.
- Keep full paths only when `include_full_paths == true`.
- Redact path-like strings inside log previews where practical.
- Add tests for:
  - `/Users/example/...` not appearing in default export.
  - path hash remains stable.
  - advanced debug export can include full paths when explicitly requested.

Acceptance criteria:

- Default diagnostics export contains no raw home directory path.
- Existing secret redaction tests continue to pass.

### 3. Split Dev And Production CSP

Goal: keep Vite/dev convenience without shipping broad localhost access in
production.

Implementation plan:

- Define a production CSP without `http://127.0.0.1:*`:
  - `default-src 'self'`
  - `connect-src ipc:`
  - `script-src 'self'`
  - `style-src 'self' 'unsafe-inline'`
  - `img-src 'self' asset: data:`
  - `object-src 'none'`
  - `frame-ancestors 'none'`
- Keep dev localhost access in a dev-only config or build-time generated
  Tauri config.
- Add a script/check that fails if production `tauri.conf.json` contains
  `http://127.0.0.1:*`.
- Add docs to the macOS release checklist.

Acceptance criteria:

- Production config has no arbitrary localhost wildcard.
- Dev workflow still works with `pnpm tauri dev`.

### 4. Inline Provider Setup In Onboarding

Goal: let a first-time user finish provider setup inside the wizard.

Implementation plan:

- Extract provider repair controls from Composer/Settings into a reusable
  `ProviderSetupPanel`.
- Use it in:
  - Composer provider popover
  - Settings provider page
  - Onboarding Provider/Credentials steps
- In onboarding:
  - choose provider
  - choose model
  - paste key
  - save key
  - refresh provider status
  - run a lightweight connection check when available
- Keep "skip provider setup" available.
- Make storage warning visible before saving.

Acceptance criteria:

- A clean profile can complete project, provider, credentials, permissions,
  trust, and start mode without opening Settings.
- Playwright covers configured and skipped provider paths.

### 5. Start Real Cancellation Plumbing

Goal: move Stop run from stream-level cooperative cancellation toward true
runtime cancellation.

Implementation plan:

- Add a cancellation token/handle to `DesktopRunHandle`.
- Pass cancellation into:
  - `DesktopRuntime::run_full_turn_with_agent_mode`
  - streaming query engine
  - provider request future
  - tool execution controller
  - bash/validation process runner
- First implementation can be staged:
  1. Desktop run handle owns a cancellation token.
  2. Stream loop selects over next event and cancellation.
  3. Bash/validation runner can terminate child process on cancel.
  4. Provider future is aborted or timed out on cancel where supported.
- Keep `force_reset_run` as the emergency UI fallback.
- Add native smoke:
  - run cancelled while waiting on synthetic long tool.
  - run cancelled while permission is pending.
  - duplicate run guard clears after cancellation.

Acceptance criteria:

- Stop run emits cancellation immediately even if no new stream event arrives.
- Long-running local process validation can be terminated by cancel.
- Runtime closeout says `cancelled`, not `failed`, when cancellation is user
  initiated.

## P1 - Product Maturity And Release Delivery

### 6. Make View Modes Real

Goal: reduce cockpit pressure without removing expert affordances.

View modes:

- Daily View:
  - transcript
  - compact tool progress
  - permission card
  - Run Review
  - composer
  - trace/tool/workbench behind secondary actions
- Engineering View:
  - Daily View plus Trace, Tool Output, Workbench, Runtime Inspector,
    context budget, diagnostics, and richer timeline facts
- LabRun View:
  - Professor/Postdoc/Graduate state
  - proof and evidence
  - reports/artifacts
  - daemon supervision
  - LabRun next actions

Implementation plan:

- Extend `detail_level` or add `desktop_view_mode`.
- Gate nonessential timeline metadata in Daily View.
- Keep deep drawers accessible, but do not make them primary visual weight.
- Make Workbench default to project readiness in Daily View.
- Add Settings and Composer controls for the three modes.
- Add Playwright coverage for major elements hidden/shown per mode.

Acceptance criteria:

- Daily View is visibly quieter in screenshots and smoke tests.
- Engineering View remains fully diagnostic.
- LabRun View makes LabRun state the main work surface.

### 7. System Credential Store Backend

Goal: meet normal desktop-user expectations for API key storage.

Implementation plan:

- Add a credential store trait:
  - `DesktopCredentialStore`
  - `save`, `load_status`, `delete`, `backend_label`
- macOS backend:
  - Keychain item per provider.
  - service name: `Priority Agent`.
  - account: provider id.
- Linux backend:
  - Secret Service when available.
- Windows backend:
  - Credential Manager when available.
- Keep dotenv fallback behind explicit user acknowledgement.
- Update `DesktopCredentialStorageStatus`:
  - active backend
  - fallback state
  - migration availability
  - last save backend
- Add tests with mocked credential store.

Acceptance criteria:

- macOS desktop saving can use Keychain.
- Dotenv remains available but is no longer the default desktop save target
  when Keychain is available.
- UI clearly shows where a key was saved.

### 8. macOS Signed/Notarized Release Track

Goal: turn the macOS-first checklist into a repeatable release path.

Implementation plan:

- Add signing configuration docs:
  - Developer ID Application certificate
  - hardened runtime
  - entitlements
  - notarization credentials
- Add scripts or Make targets for:
  - signed app build
  - notarization submit/wait
  - staple verification
  - Gatekeeper verification
- Add release evidence template:
  - commit
  - build hash
  - signing identity
  - notarization request id/status
  - smoke test log paths
- Keep unsigned local builds clearly labeled.

Acceptance criteria:

- A release candidate can produce a signed/notarized `.dmg`.
- Checklist evidence is recorded under `docs/rc/` or `apps/desktop/test-artifacts/`.

### 9. Crash And Restart Recovery

Goal: make interrupted desktop runs understandable and recoverable.

Implementation plan:

- Persist active desktop run metadata:
  - run id
  - session id
  - started at
  - provider/model
  - last event time
  - cancellation state
- On startup, show previous run recovery:
  - Resume session
  - Mark cancelled
  - Force reset local guard
  - Export diagnostics
- Add smoke for restart while run active.

Acceptance criteria:

- Restart after an active run does not leave users guessing.
- Recovery actions are visible and logged.

## P2 - Experience Polish

### 10. Improve Run Review Navigation

Goal: make evidence easy to inspect from the review card.

Plan:

- Add direct `View diff` action per changed file.
- Add direct `View validation log` action per validation item.
- Show validation command and proof status compactly in Daily View.
- Keep full trace/tool output in Engineering View.

### 11. LabRun Timeline Graph

Goal: make LabRun's role workflow visually understandable.

Plan:

- Add a compact graph:
  - Professor
  - Postdoc plan
  - Graduate task
  - Validation
  - Postdoc audit
  - Professor review
- Link graph nodes to proof/artifacts.
- Keep graph read-only and evidence-backed.

### 12. File And Symbol Mentions In Composer

Goal: make desktop task framing faster.

Plan:

- Add `@file` picker backed by project index.
- Add symbol mention when symbol index is available.
- Insert context chips rather than raw path text where possible.
- Keep all mention expansion visible in the prompt context detail drawer.

## Non-Goals

- Do not fork a separate desktop-only agent runtime.
- Do not hide validation or permissions to make the UI feel simpler.
- Do not treat controlled validation as a sandbox.
- Do not ship Windows/Linux desktop packages as release-ready until they have
  dedicated build/smoke evidence.
- Do not make Developer Auto the default path in onboarding.

## Suggested Implementation Order

1. Diagnostics path redaction and production CSP split.
2. Run Review persisted Accept event.
3. Onboarding provider setup extraction and reuse.
4. Cancellation token plumbing, starting with desktop stream cancellation and
   bash/validation process termination.
5. Real Daily / Engineering / LabRun view modes.
6. macOS Keychain backend.
7. Signed/notarized macOS release path.

## Validation Plan

Narrow gates:

```bash
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml --check
cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml -q
cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -q -- --test-threads=1
corepack pnpm --dir apps/desktop build
corepack pnpm --dir apps/desktop test:ui-smoke
git diff --check
```

Release-candidate gates:

```bash
bash scripts/validate_docs.sh
bash scripts/desktop-native-smoke.sh
corepack pnpm --dir apps/desktop tauri build
```

New smoke/test coverage should include:

- production CSP has no localhost wildcard;
- diagnostics export redacts raw paths by default;
- Run Review Accept writes a durable event;
- onboarding can save/test provider setup inline;
- Stop run cancels without waiting for a next stream event;
- Daily/Engineering/LabRun modes visibly change information density.
