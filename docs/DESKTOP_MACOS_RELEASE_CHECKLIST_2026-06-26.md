# Desktop macOS Release Checklist - 2026-06-26

Status: Active release-candidate checklist

Scope: macOS-first Tauri desktop app under `apps/desktop`. This checklist does
not declare Windows or Linux desktop packages release-ready.

## Required Build And Smoke Gates

Run from the repository root on the exact desktop release candidate commit:

```bash
corepack pnpm --dir apps/desktop build
corepack pnpm --dir apps/desktop test:ui-smoke
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -- --test-threads=1
bash scripts/desktop-native-smoke.sh
corepack pnpm --dir apps/desktop tauri build
```

## Product Safety Checks

- First-run onboarding appears on a clean desktop settings profile.
- Onboarding can complete with conservative defaults.
- Developer Auto requires explicit project trust acknowledgement.
- Workspace trust is visible in Settings and can be revoked.
- Lab daemon supervision is off by default and visible when enabled.
- Completed runs show Run Review sections for changed files, validation,
  permissions, residual risks, and actions.
- Stop run is visible while a run is active.
- Force reset requires confirmation.
- Duplicate run guard errors offer Stop run, Open trace, and Force reset.

## Security Checks

- `apps/desktop/src-tauri/tauri.conf.json` has a non-null CSP.
- Native path opening stays scoped to selected-project, app data, diagnostics,
  and selected-project LabRun roots.
- Provider key saving clearly states the active storage backend.
- Desktop credential storage reports dotenv fallback when system keychain is
  not active.
- Redacted diagnostics export does not include raw `.env` content, provider
  keys, Authorization headers, Bearer tokens, private keys, or unbounded logs.
- Controlled validation is still documented as controlled, not sandboxed.

## macOS Distribution Checks

- App bundle launches from the built `.app`.
- DMG installs and opens on a clean macOS account.
- Unsigned local builds are labeled as unsigned local builds.
- Signed builds record certificate identity and signing timestamp.
- Notarization status is recorded when signing is enabled.
- App opens after Gatekeeper quarantine on a clean machine.
- Native smoke logs are saved under `apps/desktop/test-artifacts/`.

## Release Notes Inputs

- Exact git commit.
- Build commands and pass/fail status.
- Desktop credential-store backend.
- Known limitations:
  - macOS-first desktop package.
  - Windows/Linux desktop packages not yet validated.
  - system keychain backend not active in this build.
  - controlled validation is not a sandbox for untrusted repositories.
