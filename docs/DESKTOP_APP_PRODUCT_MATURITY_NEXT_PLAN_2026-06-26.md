# Desktop App Product Maturity Next Plan - 2026-06-26

Status: Implemented

Owner: Liz / gex

Source: latest desktop-app review notes from gex's friend, checked against the
current repository on 2026-06-26 after
`DESKTOP_SECURITY_RELIABILITY_AND_LABRUN_E2E_PLAN_2026-06-26.md` was
implemented and pushed.

Implementation closeout: completed on 2026-06-26 with desktop onboarding,
project-scoped workspace trust, Run Review actions, visible stop/force-reset
recovery, explicit credential-store status, redacted diagnostics export,
macOS-first docs, and macOS release checklist. System keychain storage remains
an explicit future backend; this build reports and uses the dotenv fallback.

## Executive Judgment

The feedback is useful, but it is partly stale because the previous desktop
hardening slice already fixed the most concrete P0/P1 security items:

- Tauri CSP is no longer `null`.
- The desktop first-run/missing permission default is now `auto_low_risk`.
- Provider credential saving now warns that it writes to the local dotenv file,
  not the system keychain.
- `open_file_path` is scoped to selected-project, settings/data, diagnostics,
  and selected-project LabRun roots.
- The Tauri backend has a single-run guard plus `cancel_run` and
  `force_reset_run`.
- Lab daemon automatic supervision is opt-in and visible in Settings.
- Web-preview fixtures and Playwright expectations use neutral sample data.
- Root CI has a dedicated desktop job for frontend build, Playwright smoke,
  Tauri Rust tests, and push-only native smoke.

The remaining value in the review is not "add more panels." The next phase
should make the desktop app feel like a safe product entrypoint:

1. guide first-time users through project/provider/permission/trust choices;
2. create a clear run-review closeout surface for diff, validation, permission,
   risk, accept, revert, and continue;
3. replace desktop dotenv credential storage with a platform secret-store path;
4. add redacted desktop diagnostics export;
5. make macOS-first desktop packaging and release security explicit.

This phase should preserve the current runtime architecture. The desktop app is
still a frontend over the shared Priority Agent runtime, not a separate agent
implementation.

## Repo-Backed Current State

Current strengths:

- The desktop app has a real workbench shape: `Composer`, `Transcript`,
  `SessionHeader`, `StatusBar`, `PermissionCard`, `SettingsDrawer`,
  `TraceDrawer`, `ToolOutputDrawer`, `WorkbenchDrawer`, `RuntimeInspector`, and
  LabRun panels.
- `Transcript.tsx` already groups tool events into a run summary panel with
  Validation, Diff, Permission, Needs attention, and Tools sections.
- Session persistence already supports restore, archive, delete, rename,
  export, compact boundaries, session parts, revert projection, and
  `revert_last_turn`.
- `StartupStateCard.tsx` already handles Lab recovery with Resume, Dashboard,
  and Keep paused actions.
- Desktop security defaults were recently tightened in
  `DESKTOP_SECURITY_RELIABILITY_AND_LABRUN_E2E_PLAN_2026-06-26.md`.
- README and QUICKSTART already disclose that saved keys use dotenv storage
  rather than macOS Keychain, Secret Service, or Windows Credential Manager.

Remaining gaps:

- There is no first-run wizard. A new user still lands directly in the full
  workbench instead of a guided setup path.
- There is no project trust wizard. Permission mode, package-script trust,
  shell validation trust, Lab daemon supervision, and Developer Auto are not
  presented as one coherent workspace-trust decision.
- The run summary is useful, but it is not yet a final Run Review surface with
  clear Accept / Revert / Continue actions.
- `cancel_run` and `force_reset_run` exist at the Tauri command boundary, but
  they are not yet visible as normal desktop controls.
- Desktop credential storage is still dotenv-backed. The UI disclosure is
  honest, but not a production-grade desktop credential store.
- There is no single redacted desktop diagnostics export package that bundles
  settings summary, provider metadata, logs, recent run events, and LabRun proof
  summaries.
- `tauri.conf.json` currently bundles macOS app/dmg targets. That is fine for a
  macOS-first desktop release, but docs should state the desktop packaging
  boundary clearly instead of implying full cross-platform desktop maturity.
- macOS release signing, notarization, update channel, and installer checks are
  not yet represented as an executable release checklist.

## Decision Table

| Review item | Current judgment | Plan |
|-------------|------------------|------|
| CSP should not be `null`. | Already implemented. | Keep as regression guard in desktop build/smoke. |
| Desktop default permission is too aggressive. | Already implemented. | Keep `auto_low_risk` as default; expose trust wizard before Developer Auto. |
| Provider keys need keychain. | Correct. UI disclosure is done; backend still dotenv. | P1 platform credential store. |
| `open_file_path` too broad. | Already implemented. | Keep scoped native-open tests. |
| Backend single-run guard needed. | Already implemented. | P0/P1 expose cancel/reset controls in UI. |
| Lab daemon supervision needs visible switch. | Already implemented. | Keep manual action and visible last/next/result state. |
| Preview fixtures contain personal paths. | Already implemented. | Keep neutral fixture regression checks. |
| First-run onboarding is missing. | Correct. | P0 first-run wizard. |
| Run Review surface is missing. | Mostly correct. There is a summary panel, but no accept/revert/continue closeout. | P0 Run Review panel. |
| Desktop app is macOS-first. | Correct for current bundle. | P1 docs and release checklist clarity. |

## Target Product Invariants

After this plan is complete:

- A first-time desktop user can complete setup without reading docs first:
  project, provider, credential-storage warning, permission default, workspace
  trust, and entry mode are all explicit.
- Developer Auto is never a surprise. It requires an explicit trusted-workspace
  decision.
- Every completed agent run has a reviewable closeout card showing changed
  files, validation, permission decisions, residual risk, and next actions.
- Revert is a first-class desktop action after a run, not only a lower-level
  session command.
- Desktop diagnostics export is redacted by default and safe to attach to a bug
  report.
- Desktop credential storage has a platform secret-store path, with dotenv kept
  as a CLI-compatible fallback.
- Desktop docs clearly say "macOS-first Tauri app" until Windows/Linux desktop
  packaging is implemented and validated.

## P0 - Guided Startup And Run Review

### 1. Add First-Run Wizard

Goal: replace the "full cockpit on first launch" experience with a short,
deterministic setup flow.

Wizard steps:

1. Choose project.
   - default to `PRIORITY_AGENT_DESKTOP_PROJECT_DIR` when present.
   - allow folder picker.
   - show whether the selected path is a Git repository.
2. Choose provider/model.
   - show current configured provider status.
   - allow "skip for now" and keep diagnostics visible.
3. Explain credential storage.
   - state that dotenv is current fallback.
   - show "Use environment variables" and "Save local key" paths.
   - reserve system keychain as a future/experimental option until P1 lands.
4. Choose permission default.
   - recommended: Auto low risk.
   - options: Ask every time, Auto low risk, Developer Auto, Read only.
   - Developer Auto requires an explicit confirmation copy.
5. Choose workspace trust.
   - package scripts: ask / trusted.
   - bash validation: ask / trusted.
   - Lab daemon supervision: off by default.
6. Choose starting mode.
   - Direct task.
   - LabRun.

Implementation notes:

- Add `DesktopOnboardingState` to persisted desktop settings:
  - `onboarding_version`
  - `completed_at`
  - `project_root`
  - `permission_mode`
  - `workspace_trust_summary`
  - `credential_storage_acknowledged`
- Add `OnboardingWizard.tsx` and keep it separate from `SettingsDrawer`.
- Do not block advanced users forever. Provide a "Skip setup" path that keeps
  the conservative defaults and diagnostics warnings.
- Add Playwright coverage for first-run, skip, and completed onboarding restore.

### 2. Add Project Trust Wizard

Goal: make workspace trust a visible project-level decision instead of a set of
scattered toggles.

Trust decisions:

- package-script validation:
  - unknown: ask before accepting package-script validation as proof.
  - trusted: allow configured package-script validation.
- shell validation:
  - unknown: ask.
  - trusted: allow controlled validation commands only.
- Lab daemon supervision:
  - off by default.
  - on only after user choice.
- Developer Auto:
  - allowed only after explicit project trust acknowledgement.

Implementation notes:

- Reuse `src/lab/workspace_trust.rs` for project-scoped trust where possible.
- Add a desktop-facing trust status DTO:
  - canonical project path
  - repo identity/fingerprint where available
  - trust source
  - trusted capabilities
  - last updated
- Surface trust status in Settings and the onboarding wizard.
- Keep trust revocation available from Settings.

### 3. Promote Run Summary Into Run Review

Goal: after a completed run, users should know exactly what happened and what
to do next without opening multiple drawers.

Run Review sections:

- Changed files.
  - file path
  - additions/deletions
  - diff preview
  - checkpoint/rollback id when available
- Validation.
  - commands run
  - pass/fail/unknown
  - proof status
- Permissions.
  - approvals/rejections
  - risk level
  - checkpoint availability
- Residual risks.
  - failed tools
  - not verified closeout
  - missing validation
  - provider/runtime warnings
- Actions.
  - Accept / Dismiss review
  - Revert last turn
  - Continue with fix prompt
  - Open trace
  - Open tool output

Implementation notes:

- The existing `RunGroupPanel` is the right starting point, but it should gain
  an explicit review state and actions after `RunCompleted`.
- "Accept" should not imply code merge or release readiness. It should mean
  "dismiss this run review as accepted by the desktop user."
- "Revert" should call the existing `revert_last_turn` path and refresh session
  parts.
- "Continue" should prefill a concise repair prompt using failures and residual
  risks.
- Add Playwright tests for:
  - successful run review;
  - failed validation review;
  - revert action;
  - continue prompt generation;
  - review remains readable on mobile.

### 4. Make Cancel And Force Reset User-Visible

Goal: expose backend run recovery controls safely.

Required behavior:

- While a run is active, show:
  - Stop run
  - Force reset only in an overflow/recovery menu
- Stop run calls `cancel_run`.
- Force reset calls `force_reset_run` and must require confirmation.
- Duplicate run guard errors should surface as actionable UI:
  - "A run is already active"
  - buttons: Stop run / Open trace / Force reset

Tests:

- Stop button calls the preview API and clears running UI state.
- Duplicate submission shows the guard message without starting a second run.
- Force reset requires confirmation.

## P1 - Credential Store, Diagnostics Export, And Platform Clarity

### 1. Add Desktop Credential Store Abstraction

Goal: move desktop credential storage toward platform expectations while
preserving CLI-compatible dotenv fallback.

Design:

```text
DesktopCredentialStore
  SystemKeychain
  DotenvFallback
  EnvironmentOnly
```

Targets:

- macOS Keychain first.
- Linux Secret Service and Windows Credential Manager stay behind capability
  detection until validated.
- Never silently migrate secrets.
- Show storage backend and last updated source in Settings.

Tests:

- dotenv fallback still works.
- keychain unavailable falls back only after user acknowledgement.
- provider status does not leak raw key material.

### 2. Add Redacted Desktop Diagnostics Export

Goal: create one support artifact that can be safely attached to an issue.

Export contents:

- app version and platform;
- selected project basename and canonical path hash;
- provider/model metadata without keys;
- permission mode and workspace trust summary;
- recent desktop logs with secret redaction;
- recent run event summaries;
- recent LabRun proof summaries;
- recent error banners and diagnostics;
- CI/build info where available.

Required safeguards:

- redacted by default;
- no raw `.env`;
- no provider keys;
- no full file contents unless explicitly selected;
- no unbounded logs.

Suggested command:

```rust
export_desktop_diagnostics_bundle(redaction: DesktopDiagnosticsRedaction)
```

### 3. Clarify Desktop Platform Boundary

Goal: keep the product promise honest.

Docs updates:

- README desktop section:
  - CLI release target: macOS/Linux, Windows best-effort.
  - Desktop app: macOS-first Tauri app today.
  - Windows/Linux desktop packaging: future validation target.
- QUICKSTART desktop section:
  - dev mode command.
  - packaged app command.
  - known limitations.
- `docs/PROJECT_STATUS.md`:
  - record macOS-first desktop boundary.

### 4. Add macOS Release Packaging Checklist

Goal: make desktop release candidate checks explicit before public desktop
distribution.

Checklist items:

- `corepack pnpm --dir apps/desktop build`
- `corepack pnpm --dir apps/desktop test:ui-smoke`
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -- --test-threads=1`
- `bash scripts/desktop-native-smoke.sh`
- `corepack pnpm --dir apps/desktop tauri build`
- signed app or explicit unsigned-local build label
- notarization status, when signing is enabled
- dmg install/open smoke
- CSP check
- credential-storage disclosure check
- redacted diagnostics export check

## P2 - Product Polish And Longer-Term Hardening

- First-run wizard screenshots and short in-app affordances.
- Project switcher trust warnings when moving between repositories.
- Optional run-history dashboard by project.
- Update channel and release notes inside desktop app.
- True sandbox/container validation for untrusted repositories.
- Linux/Windows desktop package validation after macOS stabilizes.
- Usability pass for dense inspector surfaces:
  - Daily View
  - Engineer View
  - LabRun View

## Non-Goals For This Phase

- Do not add new agent roles.
- Do not fork a separate desktop runtime.
- Do not weaken CLI defaults while changing desktop UX.
- Do not claim controlled validation is a sandbox.
- Do not hide proof/trace; make it progressive disclosure instead.
- Do not make Developer Auto the default.

## Suggested Implementation Order

1. Add first-run wizard state model and preview fixture.
2. Build onboarding wizard UI with project/provider/permission/trust steps.
3. Add project trust DTO and Settings surface.
4. Promote `RunGroupPanel` into Run Review with Accept/Revert/Continue actions.
5. Expose Stop run and guarded Force reset controls.
6. Add desktop diagnostics export bundle with redaction.
7. Add credential store abstraction and macOS Keychain backend.
8. Clarify desktop platform docs and macOS release checklist.

## Validation Plan

Narrow desktop checks:

```bash
corepack pnpm --dir apps/desktop build
corepack pnpm --dir apps/desktop test:ui-smoke
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -- --test-threads=1
```

Focused Rust checks:

```bash
cargo test -q desktop_runtime --lib -- --test-threads=1
cargo test -q session_store --lib -- --test-threads=1
cargo test -q lab::workspace_trust --lib -- --test-threads=1
```

Broader gates after shared contracts move:

```bash
cargo fmt --check
git diff --check
cargo check -q
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features -- --test-threads=1
bash scripts/validate_docs.sh
```

Desktop release-candidate checks:

```bash
bash scripts/desktop-native-smoke.sh
corepack pnpm --dir apps/desktop tauri build
```

## Done Definition

This plan is done when:

- a new user can complete desktop setup through the wizard;
- workspace trust is visible and revocable;
- every completed run has an actionable Run Review;
- stop/reset recovery is visible and guarded;
- diagnostics export is redacted and test-covered;
- desktop credential storage has a platform-store path or explicit fallback;
- docs honestly state the macOS-first desktop boundary;
- desktop build, Playwright smoke, Tauri tests, docs validation, and relevant
  Rust gates pass on the final commit.
