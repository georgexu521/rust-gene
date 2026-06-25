# LabRun Validation Security And Review Hardening Plan
Status: Implemented

Date: 2026-06-25

Implementation closeout:

- Controlled Lab validation runner implemented; provider-authored validation
  strings no longer run through `sh -lc`.
- LabRun relative path normalization implemented for graduate allowed scope and
  changed-file verification.
- Semantic gates implemented for critical stage artifacts.
- Provider execution policy implemented with certified/unverified/known
  unsupported proof labels and override behavior.
- First-phase Postdoc read-only audit evidence implemented and attached to
  `PostdocIntegrationSummary`.
- `/lab proof` now surfaces safety/proof events, including validation policy,
  provider policy, semantic gate, postdoc audit, and Lab context maintenance
  events.
- Validation passed: targeted LabRun tests, request-preparation regression,
  `cargo fmt --check`, `git diff --check`, `cargo check -q`,
  `bash scripts/validate_docs.sh`, `bash -n scripts/run_live_eval.sh`,
  `python3 -m py_compile scripts/live_eval_report_parser.py`, all-features
  clippy, and all-features rustdoc.

This plan triages a fresh external review of the LabRun professor/postdoc/graduate
workflow against the current repository. The useful direction is clear: keep the
LLM/provider layer responsible for semantic judgment, but move shell execution,
path scope checks, artifact gate eligibility, and proof labeling into explicit
runtime contracts.

## Executive Summary

The review is largely correct. LabRun is now a real bounded workflow, but several
edges still need hardening before treating it as a release-trust surface:

- P0: required validation commands currently execute through `sh -lc` and can
  originate from provider-backed `PostdocPlan.validation_plan`. This must become
  a controlled Lab validation runner before more autonomous LabRun dogfood.
- P0: graduate scope checks compare raw strings. They should normalize and reject
  unsafe relative paths before comparing allowed scope and changed files.
- P0/P1: artifact gates are structurally grounded, but key stage artifacts need
  semantic eligibility checks before a gate can advance the LabRun stage.
- P1: postdoc integration currently aggregates evidence well, but it is not yet
  a true read-only code audit of the graduate diff.
- P1: provider certification is intentionally provider-neutral, but unverified
  and historically failed providers should affect proof labels and required
  safeguards.
- P2: Lab context maintenance has been made explicit in code/trace in the current
  working tree; the remaining work is to ensure proof/status surfaces make those
  maintenance artifacts obvious.

## Repo-Backed Findings

| Finding | Current code evidence | Decision |
|---|---|---|
| Required validation bypasses the normal permission/action-review path. | `runtime_verify_graduate_task_result()` calls `run_required_validation_commands()`, and that function runs each string with `Command::new("sh").arg("-lc").arg(command)` in `src/lab/orchestrator/runtime.rs`. | Accept as P0. This is the highest-risk issue. |
| Artifact gates are structural more than semantic. | `ArtifactGate::is_satisfied()` checks artifact id, next action, evidence/status, blockers, and `needs_revision`; `LabStore::validate_artifact_gate()` checks artifact/run/stage/type/owner consistency. | Accept. Keep structural checks, add semantic eligibility by artifact type. |
| Postdoc integration is evidence-aware but not fully code-aware. | `create_postdoc_integration_summary_for_latest()` aggregates GraduateResult, worktree proof, workspace proof, risks, and evidence refs, but does not perform a fresh read-only diff/file audit. | Accept as P1. This is the next major quality upgrade after safety. |
| Provider certification is diagnostic, not a runtime gate. | `LabGraduateProviderCertification::allows_graduate_execution()` returns `true`; `validate_graduate_provider_for_execution()` returns `Ok(())`. | Partially accept. Stay provider-neutral, but make unverified/failed status affect safeguards and proof labels. |
| Scope matching can be bypassed by non-normalized paths. | `path_matches_any_scope()` trims `./` and compares raw `file == scope` or `file.starts_with(scope + "/")`. | Accept as P0. Normalize both allowed scope and changed files before comparison. |
| Lab context injection has persisted maintenance side effects. | Current working tree moved this into a named `maybe_record_lab_context_maintenance()` trace path. | Mostly addressed. Add proof/status visibility as a follow-up. |

## Design Principles

- Do not weaken existing proof, permission, validation, or closeout gates to make
  weaker providers look better.
- Do not rely on prompt instructions for safety-critical behavior. Shell command
  eligibility and path scope rules belong in deterministic runtime code.
- Preserve provider-neutral execution where possible, but mark uncertainty in
  proof and require stricter safeguards for unverified providers.
- Keep LabRun steps bounded and inspectable: every blocked validation command,
  semantic gate failure, postdoc audit finding, or provider-policy downgrade
  should produce a run event/evidence ref that `/lab proof`, `/lab trace`, and
  status docs can surface.

## P0.1 Controlled Lab Validation Runner

### Problem

Graduate runtime verification currently executes required validation strings
through a shell. Because those strings can be copied from a provider-generated
`PostdocPlan.validation_plan`, the runtime can execute provider-authored shell
without the normal permission controller, action review, or user approval.

### Target Shape

Add a dedicated runner, for example:

```rust
LabValidationRunner
LabValidationCommand
LabValidationDecision
LabValidationPolicy
```

The runner should replace `run_required_validation_commands()` and should not use
`sh -lc` for normal validation commands.

### Policy

Allowed by default:

- `cargo check`, `cargo test`, `cargo clippy`, `cargo fmt --check`
- `pnpm test`, `npm test`, `yarn test`
- `pytest`, `python -m pytest`
- Selected repository validation scripts when invoked directly, not through
  shell command strings, such as `bash scripts/validate_docs.sh` and
  `bash -n scripts/run_live_eval.sh`

Denied by default:

- shell pipelines and command chaining: `|`, `;`, `&&`, `||`
- command substitution and expansion: backticks, `$(`, `${...}` where execution
  would depend on shell expansion
- destructive or privilege-changing commands: `rm`, `sudo`, `chmod`, `chown`
- network/bootstrap commands: `curl`, `wget`, `ssh`, `scp`
- filesystem redirection to arbitrary paths: `>`, `>>`, `<`, `2>`
- absolute paths, parent traversal, or execution outside the repository

Unknown commands:

- Do not execute in background graduate verification.
- Mark validation as blocked/not verified with a `lab_validation_command_blocked`
  event and a proof entry.
- Allow an explicit future user-approved override path, but keep that separate
  from unattended graduate runtime verification.

### Implementation Steps

1. Add a Lab validation runner module near the LabRun runtime boundary.
2. Parse validation strings into a command plan. Prefer direct executable + args
   execution; treat parse ambiguity as blocked.
3. Add denylist checks before allowlist checks so obviously dangerous commands
   never reach execution.
4. Replace `run_required_validation_commands()` in graduate runtime verification.
5. Record allowed/blocked/failed decisions as LabRun events and include command
   policy reason in runtime evidence.
6. Update tests that currently assume arbitrary shell validation is valid.

### Acceptance Criteria

- `cargo check -q`, `cargo test -q some_filter`, and `cargo fmt --check` style
  validations still run.
- `curl ... | sh`, `rm -rf`, `sudo`, `chmod`, `chown`, redirection, and chained
  commands are blocked before execution.
- A blocked validation command prevents verified GraduateResult binding.
- The blocked reason is visible in LabRun proof/trace output.

## P0.2 Lab Path Scope Normalization

### Problem

Graduate scope matching currently compares mostly raw path strings. This is
adequate for normal `git status` output, but unsafe for model-provided or
manually bound paths such as `src/../other`, absolute paths, backslashes, or
internal runtime directories.

### Target Shape

Add one normalization boundary:

```rust
normalize_lab_relative_path(path) -> Result<String, LabPathScopeError>
```

Use it for both `GraduateTask.allowed_scope` and runtime-observed
`changed_files`.

### Rules

- Convert `\` to `/`.
- Trim whitespace and a leading `./`.
- Reject empty paths and `.`.
- Reject absolute paths.
- Reject `..` path segments.
- Reject `.git`, `.priority-agent`, and internal LabRun runtime paths such as
  `target/lab-live-validation`.
- Preserve case rather than lowercasing; let Git provide canonical path casing.
- Compare normalized file paths to normalized scope entries.

### Acceptance Criteria

- `src/main.rs` matches `src` and `src/main.rs`.
- `src/../secrets.rs`, `/tmp/file`, `../file`, `.git/config`, and
  `.priority-agent/state.json` are rejected.
- `src\lab\mod.rs` normalizes to `src/lab/mod.rs`.
- Scope validation errors identify the offending path and do not bind a
  verified GraduateResult.

## P0.3 Semantic Gate For PostdocPlan Before Graduate Work

### Problem

Accepted `PostdocPlan` artifacts can satisfy the structural gate, then create
blocked graduate tasks if `files_expected` or `validation_plan` is missing. That
is safer than widening permissions, but it still lets a weak plan advance too
far before asking for revision.

### Target Shape

Add semantic artifact validation before accepting or advancing a gate:

```rust
validate_stage_artifact_semantics(artifact, gate_context) -> ArtifactSemanticReport
```

For `PostdocPlan`, require:

- non-empty `implementation_summary`
- non-empty `slices`
- non-empty `files_expected`
- non-empty `validation_plan`
- every `files_expected` value passes Lab path normalization
- every validation command is accepted by `LabValidationRunner` policy
- non-empty `graduate_handoff`

### Acceptance Criteria

- A provider/deterministic `PostdocPlan` missing files or validation stays in
  `needs_revision` or blocked gate state instead of advancing into executable
  graduate work.
- The revision reason names the missing/unsafe field.
- Existing tests for blocked graduate tasks are updated so the block occurs at
  the semantic gate when appropriate.

## P1.1 Semantic Gates For Remaining Critical Artifacts

### ProfessorPlan

Require:

- non-empty `problem_statement`
- non-empty `strategic_direction`
- at least one `success_criteria`
- at least one `risk`
- non-empty `handoff_to_postdoc`

### GraduateResult

Require:

- non-empty `task_summary`
- changed files normalized and within allowed scope
- validation attempts from the controlled runner, unless blockers explicitly
  explain why validation is incomplete
- non-empty `handoff_to_postdoc`

### PostdocIntegrationSummary

Require when not `needs_revision`:

- at least one accepted result
- evidence refs include GraduateResult/runtime validation proof
- no pending parent-verification marker for accepted graduate claims
- non-empty `handoff_to_professor`

### ProfessorReview

Require when `accepted == true`:

- accepted PostdocIntegrationSummary evidence
- no open needs-revision blocker
- non-empty `user_report`
- explicit reference to validation/proof evidence

## P1.2 Postdoc Read-Only Code Audit

### Problem

Postdoc integration now aggregates evidence, which is useful, but it still acts
more like a report collector than a read-only code reviewer.

### Target Shape

Introduce a postdoc audit step after GraduateResult binding and before
PostdocIntegrationSummary acceptance.

Inputs:

- GraduateResult artifact
- changed files
- allowed scope
- validation attempts and controlled-runner decisions
- git diff/worktree proof
- relevant file snippets or file-read evidence

Allowed tools:

- read-only file inspection
- grep/search
- git diff/status
- controlled validation runner only

Forbidden:

- file writes
- arbitrary shell
- mutation tools

Output options:

- Phase 1: write an audit evidence JSON/Markdown file and attach it to
  `PostdocIntegrationSummary.evidence_refs`.
- Phase 2: add a first-class `PostdocAuditReport` artifact if the schema needs a
  durable standalone audit type.

### Acceptance Criteria

- A PostdocIntegrationSummary cannot be marked ready for professor review unless
  it references either a postdoc audit proof or an explicit deferred-audit reason.
- Audit output identifies changed files inspected, validation evidence reviewed,
  and remaining risks.
- The audit path is read-only by construction.

## P1.3 Provider Certification As Safeguard Policy

### Problem

Provider certification is currently honest diagnostics, but the name suggests it
may gate execution. It does not. Fully blocking by provider name would be too
blunt, but certification should affect safeguards and proof labels.

### Target Shape

Replace a boolean "allowed" idea with an execution policy:

```rust
LabGraduateProviderExecutionPolicy {
    certification: Certified | Unverified | KnownUnsupported,
    execution_allowed: bool,
    isolated_worktree_required: bool,
    controlled_validation_required: bool,
    postdoc_audit_required: bool,
    proof_labels: Vec<String>,
    user_override_required: bool,
}
```

Policy:

- Certified: allow execution with normal LabRun gates.
- Unverified: allow execution only with isolated worktree, controlled validation,
  postdoc audit, and `provider_unverified` proof label.
- KnownUnsupported: require explicit user override or block by default; if
  overridden, require the same safeguards plus `provider_known_unsupported`
  proof label.

### Acceptance Criteria

- Provider-neutral dogfood remains possible.
- Proof clearly distinguishes certified, unverified, and known-unsupported
  graduate execution.
- Known unsupported provider execution cannot silently look like a normal
  verified run.

## P2 Lab Context Maintenance Proof Visibility

The current working tree has already separated Lab context assembly from named
Lab context maintenance and records a trace event when compression decisions or
summaries are persisted.

Remaining follow-up:

- Surface `lab_context_maintenance` in `/lab proof` or `/lab trace` summaries.
- Include compression summary artifact IDs in proof output when created during a
  normal full-agent turn.
- Keep idempotency tests so repeated request preparation does not create
  duplicate compression summaries for the same cycle.

## Suggested Implementation Order

1. Add path normalization and tests.
2. Add the controlled Lab validation runner and tests.
3. Replace `run_required_validation_commands()` and update graduate runtime
   verification tests.
4. Add PostdocPlan semantic gate checks before graduate task queueing/stage
   advance.
5. Extend semantic checks to ProfessorPlan, GraduateResult,
   PostdocIntegrationSummary, and accepted ProfessorReview.
6. Add provider execution policy labels and safeguards.
7. Add Postdoc read-only audit evidence.
8. Update `/lab proof`, `/lab trace`, `PROJECT_STATUS.md`, and docs index after
   implementation.

## Validation Plan

Run narrow tests first:

```bash
cargo test -q lab::orchestrator --lib -- --test-threads=1
cargo test -q lab::store --lib -- --test-threads=1
cargo test -q lab::draft --lib -- --test-threads=1
```

Then run shared gates:

```bash
cargo fmt --check
git diff --check
cargo check -q
cargo test -q
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

For release-trust slices, also rerun:

```bash
bash scripts/validate_docs.sh
bash -n scripts/run_live_eval.sh
python3 -m py_compile scripts/live_eval_report_parser.py
```

## Definition Of Done

- Provider-authored validation strings can no longer execute arbitrary shell
  through graduate runtime verification.
- GraduateResult verification uses normalized scope paths and rejects traversal,
  absolute paths, and internal runtime paths.
- Weak or placeholder critical artifacts produce semantic blockers before stage
  advancement.
- Postdoc review has a read-only code-audit evidence path.
- Provider certification state appears in execution policy and proof labels.
- LabRun proof/trace clearly explains validation blocks, semantic gate failures,
  provider uncertainty, postdoc audit status, and Lab context maintenance
  artifacts.
