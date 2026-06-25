# LabRun Security And Governance Next Plan
Status: Implemented

Date: 2026-06-25

This plan reviews the latest external feedback after the LabRun validation,
semantic-gate, isolated-worktree, postdoc audit, and validation-trust follow-up
slices. The feedback is useful. The project no longer shows the earlier class
of severe issue where provider-authored validation was executed through a raw
shell, but the new code-aware audit and release-trust posture introduce another
set of security and governance boundaries that should be tightened before
formal release.

Implementation closeout, 2026-06-25: this plan was implemented as a focused
security/governance hardening slice. The code now includes Lab audit redaction,
validation key/value path hardening, stricter package-script trust behavior, a
LabRun role/stage policy overlay at action review, and proof visibility for the
policy decisions. The repository also now has baseline governance files,
CodeQL, a threat model, and a security release checklist. Dependency-update
automation was intentionally disabled after the initial automated rollout
created public branch noise.

## Executive Summary

The next priority should shift from feature completion to security boundary
hardening and project governance:

- P0: Redact or suppress sensitive content in Postdoc audit snippets and diff
  summaries.
- P0: Harden Lab validation argument parsing for `--flag=path` and
  `--flag path` forms that can escape the workspace.
- P1: Make package-script validation in unknown workspaces require explicit
  approval or block by default.
- P1: Add a real LabRun role/stage policy overlay instead of treating
  `labrun` as only an `AutoLowRisk` preset name.
- P2: Add formal project governance files and security automation:
  `SECURITY.md`, `CONTRIBUTING.md`, `CODEOWNERS`, CodeQL, and a threat-model
  note.
- P2: Simplify the README architecture section so `docs/PROJECT_MAP.md` remains
  the canonical architecture map.

## Review Verdict

| Finding | Verdict | Repo evidence | Decision |
|---|---|---|---|
| Postdoc audit can persist secrets in audit files. | Correct and important. | `collect_postdoc_read_only_audit_proof()` writes `file_snippets` and `diff_summaries` to `.priority-agent/lab/runs/.../postdoc_audits/*.json`. | P0. Add Lab audit redaction and sensitive-path suppression. |
| Validation args may miss `--flag=../path` escapes. | Correct. | `suspicious_arg()` blocks direct absolute/parent traversal tokens, while `allow_cargo()` allows subcommands and relies on generic arg checks. | P0. Add structured flag/path validation for common tool flags. |
| `package_script` with `workspace_trust=unknown` is still allowed. | Correct. | `validation_policy_action()` blocks only `package_script + untrusted`; unknown remains allow. | P1. Change unknown to ask/block for LabRun verification paths. |
| LabRun role/stage permission overlay is deferred. | Correct. | `PermissionPreset::Labrun` maps to `PermissionMode::AutoLowRisk`; status docs explicitly say full overlay is future work. | P1. Add a policy overlay at the permission/action-review boundary. |
| README architecture is less canonical than `PROJECT_MAP.md`. | Correct but low risk. | README keeps a compact source tree that can drift from `docs/PROJECT_MAP.md`. | P2. Make README product-level and point to Project Map for current architecture. |
| Formal project governance is incomplete. | Correct. | The repo has CI workflows and quality gates, but no root `SECURITY.md`, `CONTRIBUTING.md`, `CODEOWNERS`, or CodeQL workflow. | P2. Add governance scaffolding and security automation. |

## Current Strengths To Preserve

- Lab validation already uses direct allowlisted command execution instead of
  `sh -lc`.
- Graduate scope checks and changed-file verification use LabRun path
  normalization.
- Provider policy and isolated-worktree requirements are now proof-backed.
- Postdoc audit is read-only and gate-visible.
- `/lab proof` surfaces validation, provider, isolation, semantic-gate, and
  audit events.
- CI already covers formatting, clippy, tests, docs validation, release artifact
  packaging, and checksum generation.

The next changes should preserve these hard boundaries. Do not make weak
providers look better by loosening validation, permissions, isolation, or proof
requirements.

## P0. Redact Sensitive Content In Postdoc Audit Evidence

### Problem

The Postdoc audit now stores:

- changed file snippets;
- git diff summaries;
- GraduateResult artifact ids and paths;
- validation event refs.

This is valuable for review, but snippets and diffs can contain secrets. The
audit files are durable local artifacts under `.priority-agent/lab/runs/...`.
If an agent modifies a provider config, dotenv file, fixture, test credential,
private key, or authorization header, raw secret-like content could be copied
into audit JSON.

### Target Shape

Add a Lab-specific redaction boundary before audit text is persisted:

```text
raw file/diff text
  -> sensitive path policy
  -> redact_lab_audit_text()
  -> bounded preview
  -> audit JSON
```

Sensitive paths should not include raw snippets or raw diffs. They should record
only safe metadata:

- normalized path;
- artifact id;
- content hash or diff hash;
- byte length / line count when useful;
- redaction reason;
- `snippet_redacted=true`;
- `diff_redacted=true`.

Suggested sensitive path patterns:

- `.env`, `.env.*`, `*.env`
- `*.pem`, `*.key`, `*.p12`, `*.pfx`
- `credentials*`, `*credentials*`
- `secrets*`, `*secret*`
- provider/local config files known to hold keys

Suggested content redaction patterns:

- `OPENAI_API_KEY=...`, `*_API_KEY=...`, `*_TOKEN=...`, `*_SECRET=...`
- `Authorization: Bearer ...`
- `Bearer <long-token>`
- `-----BEGIN ... PRIVATE KEY-----`
- SSH private key headers
- high-entropy strings above a conservative length threshold
- common cloud key prefixes where safe to detect

### Implementation Steps

1. Add a small Lab audit redaction module, for example
   `src/lab/audit_redaction.rs`, or a focused helper near
   `src/lab/orchestrator/runtime.rs` if the surface stays small.
2. Implement:
   - `is_sensitive_audit_path(path: &str) -> Option<reason>`
   - `redact_lab_audit_text(text: &str) -> RedactedAuditText`
   - optional `audit_text_hash(text: &str) -> String`
3. Apply the policy before writing `file_snippets` and `diff_summaries`.
4. Include redaction metadata in audit payloads:

```json
{
  "path": "src/config.rs",
  "snippet": "... [REDACTED: api_key] ...",
  "redaction_applied": true,
  "redaction_reasons": ["api_key"]
}
```

5. For sensitive paths, omit raw text entirely:

```json
{
  "path": ".env",
  "snippet_redacted": true,
  "redaction_reasons": ["sensitive_path:dotenv"],
  "content_hash": "sha256:..."
}
```

6. Add tests with raw values that must not appear in audit JSON:
   - `OPENAI_API_KEY=sk-...`
   - `Authorization: Bearer ...`
   - `-----BEGIN PRIVATE KEY-----`
   - dotenv-style `MINIMAX_API_KEY=...`
   - a long high-entropy token.

### Acceptance Criteria

- No audit JSON contains raw secret-like values from snippets or diffs.
- Sensitive paths record safe metadata only.
- Redaction is deterministic and covered by tests.
- Audit usefulness remains: reviewers can still see path, status, hash, and
  redaction reason.

## P0. Harden Validation Args With Key/Value Path Checks

### Problem

The current validation command runner blocks shell metacharacters and obvious
path escapes. However, many tools accept path-bearing options in forms such as:

```text
--manifest-path=../outside/Cargo.toml
--target-dir=../tmp
--rootdir=../outside
--workdir=../outside
--config=../config.toml
```

The generic `suspicious_arg()` catches direct `../x` tokens, but `--key=../x`
can hide a path value inside a flag.

### Target Shape

Validation command classification should understand path-bearing flags and
fail closed when their values are outside LabRun-relative scope.

Add a helper such as:

```rust
fn suspicious_key_value_path_arg(arg: &str) -> Option<String>
```

and, where needed, command-specific checks:

```rust
fn validate_cargo_args(args: &[String]) -> Result<(), String>
fn validate_pytest_args(args: &[String]) -> Result<(), String>
```

### Flag Policy

Always deny or strictly normalize values for:

- `--manifest-path`
- `--target-dir`
- `--config`
- `--rootdir`
- `--workdir`
- `--ignore`
- `--ignore-glob`
- `--path`
- `--workspace-root`

Handle both forms:

```text
--flag=value
--flag value
```

For commands where a flag value is allowed, the value must pass
`normalize_lab_relative_path()` or an explicitly narrower allowlist. For flags
that can change execution semantics too much, deny them outright in Lab
validation.

### Implementation Steps

1. Add a list of path-bearing validation flags.
2. Extend `suspicious_arg()` to inspect `--key=value` forms.
3. Add pairwise inspection for `--key value` forms before allowlisting a
   command.
4. Tighten cargo-specific validation:
   - either deny `--manifest-path`, `--target-dir`, and `--config`; or
   - allow only normalized in-workspace paths for a limited subset.
5. Tighten pytest/python validation:
   - deny or normalize `--rootdir`, `--confcutdir`, and similar path flags.
6. Add tests for allowed and blocked forms:
   - block `cargo test --manifest-path=../outside/Cargo.toml`
   - block `cargo check --target-dir=../tmp`
   - block `python3 -m pytest --rootdir=../outside`
   - allow normal focused commands already supported by the runner.

### Acceptance Criteria

- No `--flag=../...` or `--flag ../...` validation command is accepted.
- Direct validation commands used by current tests still pass.
- Blocked validation events still include `validation_kind`, `workspace_trust`,
  and `policy_action`.

## P1. Make Package-Script Unknown Trust Explicitly Ask Or Block

### Problem

Package scripts execute project-defined code. `npm test`, `pnpm test`, and
`yarn test` are not equivalent to direct `cargo check` or direct filesystem
validation. The current implementation distinguishes them as
`validation_kind=package_script`, but `workspace_trust=unknown` still maps to
`policy_action=allow`.

This is acceptable for dogfood in this known workspace, but too permissive for
formal release defaults.

### Target Shape

Use a stricter trust matrix:

| validation_kind | trusted | unknown | untrusted |
|---|---|---|---|
| cargo/python/filesystem/direct allowlist | allow | allow | allow or ask by risk |
| bash allowlisted scripts | allow | allow or ask | ask/block |
| package_script | allow | ask | block |

Because the current Lab validation runner is non-interactive in some paths, the
first implementation can map `unknown package_script` to block in verified
LabRun graduate validation, while leaving room for a future approval flow.

### Implementation Steps

1. Change `validation_policy_action("package_script", "unknown")` from
   `allow` to `ask` or `block`.
2. If the runner cannot ask interactively in the current path, block with a
   clear error:

```text
package-script validation requires trusted workspace approval
```

3. Add a trust override path:
   - environment: `PRIORITY_AGENT_LAB_WORKSPACE_TRUST=trusted`
   - future config: project-level trust record
   - future `/lab trust workspace` command if needed.
4. Make `/lab proof` highlight package-script validation distinctly:
   - `kind=package_script trust=unknown action=block`
   - `kind=package_script trust=trusted action=allow`
5. Add tests for unknown, trusted, and untrusted package scripts.

### Acceptance Criteria

- Unknown package-script validation no longer silently counts as verified proof.
- Trusted workspace package scripts remain usable when explicitly configured.
- `/lab proof` shows the reason and policy action.

## P1. Add LabRun Role/Stage Permission Overlay

### Problem

LabRun currently has strong workflow constraints on the graduate path, but the
named `labrun` permission preset is still just an `AutoLowRisk` preset with
stage/role guidance surfaced separately. For a formal LabRun mode, role/stage
permissions should be enforced as a deterministic overlay.

### Target Shape

Introduce a policy layer such as:

```text
LabRunPolicyOverlay
  Professor:
    allow read, artifact review, steering, final review
    deny mutation
  Postdoc:
    allow read, planning artifacts, audit artifacts, task creation/revision
    deny code mutation
  Graduate:
    allow mutation only inside allowed_scope
    require validation and evidence
  Runtime:
    allow validation, gate writing, proof events, scheduler state
```

The overlay should sit near permission/action-review decisions, not only in
prompt text.

### Implementation Steps

1. Add a small policy model:
   - `LabRunRole`
   - `LabRunStage`
   - `LabRunActionFamily`
   - `LabRunPolicyDecision`
2. Resolve current LabRun role/stage from Lab context when available.
3. Add overlay checks to mutation-capable tools:
   - file write/edit/patch
   - bash commands with mutation side effects
   - MCP/plugin mutation paths if applicable
4. Graduate mutations must prove the target path is inside `allowed_scope`.
5. Postdoc/professor modes should be read/artifact/audit only unless a specific
   runtime action is allowlisted.
6. Add proof/trace events:
   - `labrun_policy_allowed`
   - `labrun_policy_blocked`
   - include role, stage, action family, path, and reason.
7. Update `/permissions preset labrun` description after the overlay is real.

### Acceptance Criteria

- LabRun professor/postdoc turns cannot mutate project files through normal
  tool surfaces.
- Graduate turns cannot mutate outside `allowed_scope`.
- Policy decisions are visible in trace/proof output.
- Existing graduate runtime verification remains the final evidence gate; the
  overlay does not replace validation.

## P2. Add Formal Security And Governance Files

### Problem

The codebase has good CI and documentation, but formal project governance is
still thin for a public release. Large engineering organizations usually expect
security reporting, contribution rules, ownership, dependency automation, static
analysis, and a basic threat model.

### Target Shape

Add the following without over-bureaucratizing the project:

- `SECURITY.md`
- `CONTRIBUTING.md`
- `.github/CODEOWNERS`
- `.github/workflows/codeql.yml`
- `docs/THREAT_MODEL.md`
- optional `docs/SECURITY_RELEASE_CHECKLIST.md`

### Suggested Contents

`SECURITY.md`:

- supported versions / pre-release status;
- private vulnerability reporting path;
- do not include secrets in reports;
- current known security limitations:
  plaintext local dotenv credentials, best-effort Windows support, LabRun
  package-script trust still being hardened until P1 is complete.

`CONTRIBUTING.md`:

- setup commands;
- validation commands;
- coding style;
- documentation update expectations;
- security-sensitive change checklist.

`CODEOWNERS`:

- default owner for all files;
- stricter ownership for `src/lab/`, `src/permissions/`, `src/security/`,
  `.github/`, provider credentials, and release scripts.

CodeQL:

- Rust/C++ if supported by current GitHub CodeQL setup;
- JavaScript/TypeScript for desktop frontend;
- run on pull requests and pushes to `main`.

Threat model:

- local coding agent threat assumptions;
- untrusted repository/workspace risks;
- provider-generated command risks;
- secret handling risks;
- LabRun role/stage trust boundaries;
- MCP/tool/plugin boundary risks;
- release artifact and CI trust assumptions.

### Acceptance Criteria

- The repository has clear security reporting and contribution guidance.
- Ownership and security-sensitive paths are explicit.
- Static-analysis automation is present in `.github/`; dependency updates remain
  manual until automatic dependency PRs are useful enough to re-enable.
- `docs/README.md` links the threat model or security checklist.

## P2. Simplify README Architecture

### Problem

README should be stable and product-facing. Detailed source trees drift quickly.
`docs/PROJECT_MAP.md` is already the canonical architecture map and is more
accurate for current module boundaries.

### Target Shape

Keep README architecture to:

- product-level runtime summary;
- short runtime flow;
- direct link to `docs/PROJECT_MAP.md` for canonical source layout;
- remove or shrink the detailed source tree if it becomes stale.

### Acceptance Criteria

- README does not duplicate detailed architecture ownership that belongs in
  `PROJECT_MAP.md`.
- New contributors are routed to the canonical map.
- `docs/validate_docs.sh` still passes.

## Recommended Implementation Order

1. P0 audit redaction and sensitive-path suppression.
2. P0 validation key/value path hardening.
3. P1 package-script unknown trust ask/block behavior.
4. P1 LabRun role/stage permission overlay.
5. P2 governance docs and security automation.
6. P2 README architecture simplification.

## Validation Plan

Targeted tests:

```bash
cargo test -q lab::orchestrator --lib -- --test-threads=1
cargo test -q lab::validation --lib -- --test-threads=1
cargo test -q permissions --lib -- --test-threads=1
cargo test -q lab::commands --lib -- --test-threads=1
```

Shared gates:

```bash
cargo fmt --check
git diff --check
cargo check -q
bash scripts/validate_docs.sh
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Governance/CI validation:

```bash
ruby -e 'require "yaml"; ARGV.each { |path| YAML.load_file(path); puts "OK #{path}" }' .github/workflows/*.yml
```

If `cargo audit` or license/SBOM tools are added in this slice, record the exact
installation and invocation path in `QUALITY_GATES.md`.

## Definition Of Done

- Postdoc audit never persists raw secret-like snippets or diffs.
- Validation commands cannot hide workspace escapes inside key/value flags.
- Package-script validation in unknown workspaces is not silently treated as
  verified proof.
- LabRun professor/postdoc/graduate role boundaries are enforced by runtime
  policy, not only by workflow convention.
- The public repository has baseline security, contribution, ownership,
  dependency, static-analysis, and threat-model documentation.
- README remains a stable product entrypoint, while `docs/PROJECT_MAP.md`
  remains the canonical architecture map.

## Closeout Notes

Implemented files and surfaces:

- `src/lab/audit_redaction.rs`: deterministic audit redaction, sensitive-path
  classification, and SHA-256 audit hashes.
- `src/lab/orchestrator/runtime.rs`: Postdoc audit snippet/diff persistence now
  passes through redaction and sensitive-path suppression.
- `src/lab/validation.rs`: path-bearing validation flags are blocked, and
  unknown package-script validation is rejected unless the workspace is trusted.
- `src/lab/policy_overlay.rs` plus `src/engine/action_review.rs`: LabRun
  role/stage policy is enforced before normal tool execution proceeds.
- `src/lab/commands/view.rs`: `/lab proof` includes LabRun policy decisions.
- `SECURITY.md`, `CONTRIBUTING.md`, `.github/CODEOWNERS`,
  `.github/workflows/codeql.yml`, `docs/THREAT_MODEL.md`, and
  `docs/SECURITY_RELEASE_CHECKLIST.md`: baseline public security and governance
  posture.

Remaining future hardening:

- Add release signing, provenance attestations, SBOM generation, and license
  audit after the first formal release candidate path is stable.
- Add an interactive trust-management command such as `/lab trust workspace` if
  package-script validation needs a first-class user approval flow.
- Expand LabRun policy overlay to MCP/plugin mutation families as those
  surfaces gain more formal LabRun integration tests.
- Re-enable dependency-update automation only when the project is ready to
  triage automated PR branches.

Validation recorded for closeout:

```bash
cargo test -q lab:: --lib -- --test-threads=1
cargo test -q action_review --lib -- --test-threads=1
cargo fmt --check
git diff --check
ruby -e 'require "yaml"; ARGV.each { |path| YAML.load_file(path); puts "OK #{path}" }' .github/workflows/*.yml
cargo check -q
bash scripts/validate_docs.sh
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo doc --workspace --all-features --no-deps
```

`scripts/validate_docs.sh` passed with 3177 tests passed, 0 failed, and 1
ignored.
