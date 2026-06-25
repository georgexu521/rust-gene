# LabRun Policy, Audit Scale, And Supply Chain Hardening Plan
Status: Implemented

Date: 2026-06-25

This plan reviews the latest post-governance feedback after the LabRun policy
overlay, audit redaction, CodeQL, and governance-doc slices. The feedback is
useful. I do not see a new critical issue at the same level as the earlier
provider-authored raw shell validation problem, but the comments identify real
release-hardening gaps in LabRun policy activation, Runtime-owner privilege,
audit evidence size control, and dependency/supply-chain governance.

The priority for the next slice should be precision, not broad feature growth:
make the existing safety model less surprising for normal coding turns, make
Runtime-owner mutation fail closed for model-proposed tools, cap durable audit
evidence size, and add dependency/security tooling that does not recreate public
branch noise.

Implementation closeout, 2026-06-25: this plan was implemented as a focused
policy/audit/supply-chain hardening slice. LabRun policy activation is now
reasoned by run status, Runtime-owner mutation no longer grants model tools a
blanket write bypass, Postdoc audit has bounded file/diff evidence and
low-value path omission, and dependency/license governance has a non-noisy
`deny.toml` plus `scripts/security_dependency_audit.sh` entrypoint.

## Executive Summary

- P0: Narrow LabRun policy overlay activation so paused, blocked, or
  needs-user LabRuns do not unexpectedly block ordinary non-Lab coding turns.
- P0: Split Runtime deterministic maintenance from model-proposed tool actions.
  Runtime-owned LabRun stages should not grant broad mutation permission to
  normal tool calls.
- P1: Add Postdoc audit file/diff byte budgets. Large files, generated files,
  lockfiles, vendor bundles, and binary assets should record metadata, hash, and
  bounded previews rather than full read/compact payloads.
- P1: Add supply-chain checks that fit the repository's current preference for
  a clean public branch list: `cargo audit`, `cargo deny`, license policy, and
  manual dependency-review docs before re-enabling automated dependency PRs.
- P2: Prepare optional release-level provenance, SBOM, and secret scanning once
  the release-candidate path is stable.

## Review Verdict

| Finding | Verdict | Repo evidence | Decision |
|---|---|---|---|
| LabRun policy overlay may apply too broadly to paused/blocked runs. | Correct. | `labrun_policy_applies_to_status()` currently excludes only `Completed`, `Failed`, and `Cancelled`, so `Paused`, `PausedShutdown`, `NeedsUser`, and `Blocked` still apply. | P0. Define explicit activation rules for active Lab Mode versus ordinary full-agent turns. |
| Runtime owner allows mutation too broadly. | Correct. | `review_labrun_tool_action()` allows any mutation when `run.internal_owner == LabRole::Runtime`. That is safe for deterministic internal maintenance, but too broad for model-proposed tools. | P0. Treat Runtime owner as internal-maintenance-only for writes; normal model tools should be read-only or ask/deny. |
| Postdoc audit reads whole files and whole diffs before compacting. | Correct. | `audit_file_snippet_payload()` uses `std::fs::read(parent_path)`, and `git_diff_for_path()` collects full stdout before compacting. | P1. Add file/diff byte limits, hash-first metadata, and bounded preview paths. |
| Audit should omit low-value bulky paths, not only sensitive paths. | Correct. | Sensitive-path suppression exists, but there is no generated/vendor/lockfile/binary low-value path policy. | P1. Add audit omission classes with path reason and hash metadata. |
| Large-company supply-chain governance remains incomplete. | Correct. | CodeQL and governance docs exist. Dependency-update automation is intentionally disabled, and `cargo audit`, `cargo deny`, SBOM, signing, provenance, and secret scanning remain future hardening. | P1/P2. Add non-noisy checks first; keep dependency PR automation disabled until it is worth triaging. |

## Current Strengths To Preserve

- Lab validation no longer executes provider-authored validation through raw
  shell strings.
- Graduate path scope, provider isolation, validation proof, and semantic gates
  are runtime-backed.
- Postdoc audit now redacts secret-like content and suppresses sensitive-path
  snippets/diffs.
- LabRun policy overlay is already visible in action review and `/lab proof`.
- Dependency automation branch noise was removed; the public remote currently
  stays focused on `main`.

Do not loosen validation, scope checks, permissions, or evidence gates to reduce
friction. The goal is to make the gates more precise and more explainable.

## P0. Narrow LabRun Policy Overlay Activation

### Problem

The current overlay applies whenever the latest LabRun is not terminal:

```text
applies = status not in Completed / Failed / Cancelled
```

That means a paused, blocked, or needs-user LabRun can still affect normal tool
actions. This is safe from a "protect the LabRun workspace" perspective, but it
can surprise the user:

```text
I paused LabRun, then asked a normal coding question, but file edits are still
blocked because the latest LabRun owner is Professor/Postdoc.
```

### Target Shape

Add an explicit policy-activation model:

```text
LabRunPolicyActivation
  Inactive:
    no overlay
  ActiveLabMode:
    full role/stage overlay
  PausedProtection:
    read-only overlay or ask before mutation
  NeedsUserProtection:
    ask before mutation, include user-facing reason
  BlockedProtection:
    ask before mutation unless the action is explicit unblock/recovery
```

The action-review boundary should know whether the current tool call is part of
a LabRun turn or an ordinary full-agent turn. If that signal is not available
yet, the first implementation can be conservative but explicit:

- `Active` + active LabRun context: apply full overlay.
- `Paused` / `PausedShutdown`: do not hard-block ordinary non-Lab file edits;
  either mark not applicable or return ask-required with a clear reason.
- `NeedsUser` / `Blocked`: do not silently hard-block normal work; require
  explicit LabRun recovery/resume intent before applying workflow mutation
  rules.
- terminal statuses remain not applicable.

### Implementation Steps

1. Add a small policy context type near `src/lab/policy_overlay.rs`:
   - `LabRunPolicyActivation`
   - current run status;
   - whether the current turn is explicitly LabRun-scoped;
   - whether mutation is a LabRun recovery action.
2. Change `labrun_policy_applies_to_status()` from a boolean to a reasoned
   activation decision.
3. Thread the activation decision into `ActionReview` debug metadata and
   `/lab proof` policy events.
4. Add tests:
   - active LabRun + postdoc mutation is blocked;
   - paused LabRun + ordinary tool action is not hard-blocked;
   - needs-user LabRun records ask/recovery guidance instead of a confusing
     professor/postdoc mutation denial;
   - terminal LabRun remains not applicable.

### Acceptance Criteria

- Pausing LabRun does not unexpectedly block ordinary coding turns.
- Explicit LabRun turns still protect professor/postdoc/graduate boundaries.
- Policy output includes an understandable activation reason.

## P0. Restrict Runtime Owner Mutation To Internal Maintenance

### Problem

The current Runtime role path says:

```text
LabRole::Runtime => allowed mutation
```

That is appropriate for deterministic internal maintenance such as writing
LabRun proof events, compression summaries, scheduler state, and runtime-owned
artifacts under `.priority-agent/lab`. It is too broad for ordinary
model-proposed tool calls. If a normal tool call happens while
`internal_owner=Runtime`, the overlay currently does not add meaningful
protection.

### Target Shape

Split Runtime actions by source:

```text
Runtime deterministic maintenance:
  allow writes only to .priority-agent/lab and known runtime stores

Model-proposed tool action while owner=Runtime:
  read-only allowed
  mutation ask/deny unless explicitly scoped as runtime maintenance
```

### Implementation Steps

1. Add `LabRunActionSource` or equivalent:
   - `ModelTool`
   - `RuntimeMaintenance`
   - `LabCommand`
   - future `Scheduler`
2. Default `ActionReview` tool calls to `ModelTool`.
3. Allow `RuntimeMaintenance` only through internal code paths that do not call
   normal model tools, or only for paths under `.priority-agent/lab`.
4. For Runtime owner + model mutation:
   - return `LabRunPolicyViolation` or ask-required;
   - recovery text should say "Runtime owner does not grant model mutation
     permission; resume LabRun or route through scoped graduate task."
5. Add tests:
   - Runtime owner + `file_edit README.md` blocks or asks;
   - Runtime maintenance path under `.priority-agent/lab` is allowed only when
     explicitly marked as internal maintenance;
   - Runtime owner + read-only file read remains allowed.

### Acceptance Criteria

- Runtime owner cannot be used as a blanket bypass for normal mutation tools.
- Deterministic LabRun maintenance still works.
- The reason is visible in action review metadata and proof events.

## P1. Add Postdoc Audit Size Budgets

### Problem

Postdoc audit currently reads full file bytes and full `git diff` stdout, then
redacts and compacts. This is acceptable for normal source files, but risky for
large files, generated outputs, lockfiles, vendor bundles, binaries, or huge
diffs.

### Target Shape

Add explicit audit budgets:

```text
max_audit_file_bytes = 256 KiB
max_audit_diff_bytes = 512 KiB
snippet_preview_chars = existing compact preview budget
```

If a file or diff is too large:

- do not persist raw full content;
- record `content_hash` / `diff_hash`;
- record byte length;
- record reason such as `omitted_large_file` or `omitted_large_diff`;
- optionally include bounded first/last text chunks after redaction when it is
  safe and useful.

### Implementation Steps

1. Add constants near the audit helpers:
   - `MAX_POSTDOC_AUDIT_FILE_BYTES`
   - `MAX_POSTDOC_AUDIT_DIFF_BYTES`
2. Replace `std::fs::read(parent_path)` with metadata-first logic:
   - read metadata length;
   - if above limit, hash via streaming read or omit raw content;
   - only read full file when below limit.
3. Add a bounded diff helper:
   - either cap captured stdout safely;
   - or use `git diff --numstat` / `--shortstat` plus a bounded patch preview.
4. Add tests:
   - large changed file records `snippet_omitted=true`;
   - large diff records `diff_omitted=true`;
   - hashes are present;
   - raw large body content is absent from audit JSON.

### Acceptance Criteria

- Audit cannot load or persist unbounded file/diff content.
- Reviewers still get path, size, hash, and omission reason.
- Existing small-file audit behavior remains intact.

## P1. Add Low-Value / Bulky Path Omission Policy

### Problem

Sensitive paths are now suppressed, but non-sensitive bulky paths can still be
low value or expensive to inspect. Examples:

- `Cargo.lock`, `package-lock.json`, `pnpm-lock.yaml`, `yarn.lock`;
- `dist/`, `build/`, `target/`, generated bundles;
- vendor directories;
- binary/media files such as `*.png`, `*.jpg`, `*.pdf`, `*.zip`, `*.sqlite`.

### Target Shape

Add a second path classifier separate from secret redaction:

```text
audit_path_class
  sensitive_path -> no raw text
  generated_or_vendor -> no raw text
  lockfile -> metadata/hash only
  binary_or_media -> metadata/hash only
  normal_text -> redacted bounded preview
```

### Implementation Steps

1. Extend `src/lab/audit_redaction.rs` or add `src/lab/audit_path_policy.rs`.
2. Keep secret-sensitive reasons separate from low-value omission reasons.
3. Add tests for common lockfiles, generated directories, vendor paths, and
   binary extensions.
4. Include `audit_omission_reason` in Postdoc audit JSON.

### Acceptance Criteria

- Lockfiles and generated/binary paths do not create large snippets.
- Secret redaction and low-value omission are distinguishable in audit output.
- Audit JSON remains useful without becoming noisy or huge.

## P1. Add Non-Noisy Dependency And License Governance

### Problem

The project now has CodeQL and governance docs, but dependency-update automation
was disabled to keep GitHub branches clean. That is reasonable for the current
stage, but release readiness still needs dependency and license checks.

### Target Shape

Add checks that do not create public branches:

- `cargo audit` for known RustSec advisories;
- `cargo deny` for advisory, license, duplicate, and banned dependency policy;
- a checked-in `deny.toml` with explicit allowed licenses and advisory policy;
- documentation in `QUALITY_GATES.md` and
  `docs/SECURITY_RELEASE_CHECKLIST.md`;
- optional CI job that runs on push/PR without opening dependency PRs.

### Implementation Steps

1. Add `deny.toml` with a conservative first policy:
   - allow common OSS licenses currently used by the repo;
   - warn or deny unmaintained/yanked advisories according to release stage;
   - document exceptions inline.
2. Add scripts:
   - `scripts/security_dependency_audit.sh`
   - or Makefile targets if the repo prefers Makefile entrypoints.
3. Add CI job or quality-gate docs for:
   - `cargo audit`;
   - `cargo deny check`.
4. Update release checklist and threat model to say dependency automation is
   manual, but vulnerability/license checks are explicit.
5. Keep automated dependency PR tooling disabled unless the user decides branch
   noise is acceptable later.

### Acceptance Criteria

- A release candidate has repeatable dependency and license checks.
- The checks do not create GitHub branches.
- Known advisory/license exceptions are documented, not implicit.

## P2. Prepare Release Supply-Chain Provenance

### Problem

Release signing, SBOM, provenance attestations, and secret scanning are still
future hardening. They are not required for current dogfood, but they matter
before a formal external release.

### Target Shape

Add release-candidate optional gates:

- SBOM generation, likely CycloneDX or SPDX;
- artifact checksum plus future signing;
- GitHub Actions provenance or SLSA-style attestation if practical;
- secret scanning gate using either platform features or a local scanner;
- documentation of what is required versus optional for the first public
  release.

### Implementation Steps

1. Add `docs/SUPPLY_CHAIN_RELEASE_HARDENING_PLAN_2026-06-25.md` only if the P1
   dependency work becomes too large for this plan.
2. Add checklist entries and CI placeholders that fail honestly when tools are
   unavailable rather than reporting fake PASS.
3. Defer mandatory enforcement until the release artifact path is stable.

### Acceptance Criteria

- The project has a clear release supply-chain roadmap.
- The first release candidate can report which supply-chain gates passed,
  skipped, or remain future work.

## Recommended Implementation Order

1. P0 LabRun overlay activation boundaries.
2. P0 Runtime-owner mutation restriction.
3. P1 Postdoc audit size budgets.
4. P1 low-value/bulky path omission policy.
5. P1 `cargo audit` / `cargo deny` and `deny.toml`.
6. P2 SBOM/provenance/secret scanning release plan.

## Validation Plan

Targeted tests:

```bash
cargo test -q lab::policy_overlay --lib -- --test-threads=1
cargo test -q action_review --lib -- --test-threads=1
cargo test -q lab::orchestrator --lib -- --test-threads=1
cargo test -q lab::audit_redaction --lib -- --test-threads=1
```

Security/dependency checks after P1 dependency work:

```bash
cargo audit
cargo deny check
```

Shared gates:

```bash
cargo fmt --check
git diff --check
cargo check -q
bash scripts/validate_docs.sh
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo doc --workspace --all-features --no-deps
```

## Definition Of Done

- Paused, blocked, or needs-user LabRuns do not unexpectedly block ordinary
  non-Lab coding turns.
- Runtime owner does not grant broad mutation permission to model-proposed
  tools.
- Postdoc audit has bounded file and diff evidence collection.
- Sensitive, generated, vendor, lockfile, binary, and large paths have explicit
  audit policies.
- Dependency vulnerability and license checks exist without reintroducing
  automatic public branch creation.
- Release supply-chain gaps remain tracked honestly until they are implemented.

## Closeout Notes

Implemented files and surfaces:

- `src/lab/policy_overlay.rs`: added reasoned LabRun policy activation, status
  and source metadata, paused/needs-user/blocked non-hard-block behavior, and
  Runtime owner model-tool mutation denial.
- `src/lab/commands/view.rs`: `/lab proof` now includes policy status,
  activation, and action source for allowed/blocked policy events.
- `src/lab/audit_redaction.rs`: added bounded stream capture, streaming hash,
  and low-value/bulky audit path classification.
- `src/lab/orchestrator/runtime.rs`: Postdoc audit now uses file/diff byte
  budgets and metadata-only persistence for sensitive, bulky, lockfile, binary,
  generated, or large evidence.
- `deny.toml` and `scripts/security_dependency_audit.sh`: added non-noisy
  dependency vulnerability/license checks without automatic branch creation.
- `QUALITY_GATES.md`, `SECURITY.md`, `docs/THREAT_MODEL.md`, and
  `docs/SECURITY_RELEASE_CHECKLIST.md`: documented the new security dependency
  gate and remaining release supply-chain work.

Remaining future hardening:

- Install and run `cargo-audit` / `cargo-deny` in the release environment before
  formal tagging; the script intentionally fails with installation guidance
  when tools are absent.
- Add SBOM generation, artifact signing, provenance attestations, and automated
  secret scanning once the release-candidate artifact path is stable.
- Revisit whether a first-class LabRun turn/source signal should distinguish
  ordinary active-run full-agent turns from explicit Lab Mode tool calls.

Validation recorded for closeout:

```bash
cargo test -q lab::policy_overlay --lib -- --test-threads=1
cargo test -q lab::audit_redaction --lib -- --test-threads=1
cargo test -q action_review --lib -- --test-threads=1
cargo test -q lab::orchestrator --lib -- --test-threads=1
cargo test -q lab::commands --lib -- --test-threads=1
cargo test -q lab:: --lib -- --test-threads=1
cargo fmt --check
git diff --check
cargo check -q
bash scripts/validate_docs.sh
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo doc --workspace --all-features --no-deps
bash -n scripts/security_dependency_audit.sh
```

`scripts/validate_docs.sh` passed with 3184 tests passed, 0 failed, and 1
ignored.

`scripts/security_dependency_audit.sh` was executed and correctly reported that
`cargo-audit` and `cargo-deny` are not installed on this machine. That gate is
therefore wired and honest, but it was not able to produce dependency advisory
or license results in this local environment.
