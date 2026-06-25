# LabRun Post-Hardening Follow-Up Plan
Status: Implemented

Date: 2026-06-25

This plan reviews a follow-up external assessment after the LabRun validation,
path-scope, semantic-gate, provider-policy, and proof-surface hardening slice.
The assessment is directionally correct: the core professor/postdoc/graduate
workflow is now much stronger, but several boundary details should be tightened
before treating LabRun as a release-trust automation surface.

## Executive Summary

The previous hardening slice fixed the main safety holes:

- LabRun graduate validation no longer executes provider-authored commands
  through `sh -lc`.
- Graduate allowed scope and changed-file verification now use shared path
  normalization.
- Critical artifacts now have semantic gates.
- Failed provider diagnostics can block graduate execution.
- Postdoc integration writes first-phase read-only audit evidence.
- `/lab proof` surfaces safety/proof events.

The remaining issues are smaller but important:

- P0: `/lab plan` currently reports "Gate satisfied" even when semantic blockers
  keep the gate blocked.
- P1: Postdoc audit is still a metadata/evidence audit, not a real diff/file
  inspection audit.
- P1: provider policy records `isolated_worktree_required`, but runtime
  verification can still fall back to the parent workspace.
- P2: package-script validation commands such as `npm test`, `pnpm test`, and
  `yarn test` are allowlisted without a workspace-trust distinction.

## Review Verdict

| Finding | Verdict | Repo evidence | Decision |
|---|---|---|---|
| `/lab plan` output can mislead users. | Correct. | `src/lab/commands.rs` formats `Gate satisfied for stage` unconditionally, while semantic gates can return `Ok(gate)` with blockers. | P0 fix. Show actual gate status and blockers. |
| Postdoc audit does not yet read code. | Correct. | `collect_postdoc_read_only_audit_proof()` checks changed-file metadata, parent existence, and validation attempt presence, then writes JSON evidence. | P1 upgrade. Add diff/file-read audit evidence. |
| Provider isolated-worktree policy is not a hard runtime requirement. | Correct. | `graduate_provider_execution_policy()` records `isolated_worktree_required`, but `runtime_verify_graduate_task_result()` falls back to `context.working_dir` when no agent worktree is found. | P1 hardening. Missing isolation proof should block verified graduate binding when policy requires isolation. |
| Package-script validation should consider workspace trust. | Correct as future hardening. | `classify_lab_validation_command()` allowlists `npm`, `pnpm`, and `yarn` test commands. | P2. Add validation kind and workspace-trust labeling first, then gate untrusted package scripts. |

## P0. Fix `/lab plan` Gate Status Output

### Problem

The `/lab plan` branch currently renders:

```text
Gate satisfied for stage '...'
```

This is no longer always true. Semantic gate validation can create an artifact,
return a gate, and mark that gate as blocked or `needs_revision`. Returning
`Ok(created)` is intentional because the artifact was created and the user needs
to see the revision blockers.

### Target Shape

Render gate status the same way `/lab integrate` and `/lab professor-review`
already do:

```text
Created PostdocPlan artifact: artifact_postdocplan_...
Gate: postdoc_plan (blocked)
Blockers: PostdocPlan.files_expected must contain at least one item; ...
Artifact: ...
Report: ...
```

### Implementation Steps

1. Add a small formatter near the Lab command surface, for example
   `format_created_stage_artifact(created)`.
2. Use `created.gate.is_satisfied()` to choose `satisfied` or `blocked`.
3. Include blockers when `created.gate.blockers` is non-empty.
4. Include validation status when present, because `needs_revision` is useful
   user-facing evidence.
5. Update `/lab plan` tests to cover both a satisfied gate and a semantic-blocked
   gate.

### Acceptance Criteria

- `/lab plan` never says "Gate satisfied" when `created.gate.is_satisfied()` is
  false.
- Blocked semantic gate output includes the blocker text.
- Existing happy-path plan output remains compact and readable.

## P1. Upgrade Postdoc Audit To Diff/File Inspection

### Current State

The current postdoc audit is a useful first phase. It is read-only and durable,
but it mainly records:

- GraduateResult artifact ids.
- `changed_files` metadata.
- whether each changed file exists in the parent workspace.
- whether validation attempts are present.
- remaining audit risks.

It does not yet inspect:

- actual git diff hunks;
- changed file content snippets;
- validation event payloads;
- the GraduateResult JSON body as audit input.

### Target Shape

Add a `PostdocDiffAudit` evidence path without immediately creating a new
first-class artifact type. The audit can stay as JSON/Markdown evidence under:

```text
.priority-agent/lab/runs/<run>/postdoc_audits/
```

Suggested fields:

```json
{
  "audit_id": "...",
  "status": "postdoc_audit_verified | postdoc_audit_not_verified | postdoc_audit_needs_revision",
  "changed_files_inspected": [],
  "diff_summaries": [],
  "file_snippets": [],
  "graduate_result_artifact_ids": [],
  "validation_event_refs": [],
  "risks": [],
  "forbidden_actions": ["file_write", "file_edit", "file_patch", "arbitrary_shell"]
}
```

### Implementation Steps

1. Split the current metadata audit into an explicit helper such as
   `collect_postdoc_metadata_audit_proof()`.
2. Add `collect_postdoc_diff_audit_proof()` that:
   - reads normalized changed files from GraduateResult artifacts;
   - reads bounded file snippets from the relevant verification root when
     available;
   - collects `git diff -- <changed_files>` or direct diff evidence through a
     non-shell `Command::new("git")` path;
   - links recent `lab_validation_command_*` events;
   - links the GraduateResult artifact JSON path.
3. Keep the audit read-only by construction. Do not expose file write/edit/patch
   or arbitrary shell.
4. Attach the audit evidence ref to `PostdocIntegrationSummary.evidence_refs`.
5. Add `/lab proof` display support for the audit status and audit path.

### Acceptance Criteria

- Postdoc integration evidence distinguishes metadata-only audit from diff/file
  audit.
- A missing file, missing diff, missing validation event, or out-of-scope changed
  file produces `postdoc_audit_needs_revision` or a clearly named remaining risk.
- The audit does not mutate files or run arbitrary shell.

## P1. Enforce Isolated Worktree Requirement For Graduate Verification

### Current State

The provider execution policy records:

- `isolated_worktree_required`
- `controlled_validation_required`
- `postdoc_audit_required`
- provider proof labels

However, `runtime_verify_graduate_task_result()` currently resolves a
verification root by trying `agent_id`, then `agent_task_id`, then falling back to
`context.working_dir`.

That fallback is useful for local diagnostics, but it is too permissive for
provider policies that require isolated graduate execution.

### Target Shape

When a policy requires isolation, verified graduate binding should require hard
isolation proof:

- an agent task state with `isolated_worktree.path`;
- the path exists;
- the path is not the parent workspace;
- runtime changed-file verification runs against that worktree;
- proof is recorded in run events and GraduateResult evidence refs.

### Implementation Steps

1. Pass `LabGraduateProviderExecutionPolicy` or an explicit
   `GraduateVerificationPolicy` into `runtime_verify_graduate_task_result()`.
2. Add a helper such as `resolve_graduate_verification_root(context, agent_id,
   agent_task_id, policy)`.
3. If `policy.isolated_worktree_required` is true and no distinct isolated
   worktree exists:
   - block runtime verification;
   - record `lab_graduate_isolation_missing`;
   - mark the dispatch failed or the task blocked;
   - do not create a verified GraduateResult.
4. Apply the same rule to durable sync paths, not only the immediate
   `execute_graduate_task_latest_with_context()` path.
5. Decide whether certified providers also keep `isolated_worktree_required =
   true`. If yes, enforce it uniformly. If no, change the policy value so output
   and behavior match.

### Acceptance Criteria

- Unverified and override-enabled known-unsupported provider execution cannot
  produce a verified GraduateResult without isolated worktree proof.
- Missing isolation proof appears in `/lab proof` and `/lab trace`.
- Existing durable isolated-worktree success tests continue to pass.
- Parent-workspace fallback is either removed from verified LabRun graduate
  paths or explicitly limited to non-verified diagnostics.

## P2. Add Workspace Trust For Package-Script Validation

### Current State

The controlled Lab validation runner blocks shell metacharacters and dangerous
commands, but `npm test`, `pnpm test`, and `yarn test` still execute package
scripts. That is reasonable in this trusted repository, but it is not a safe
default for arbitrary workspaces.

### Target Shape

Add explicit validation metadata:

- `validation_kind=cargo | pytest | python_py_compile | package_script |
  bash_allowlisted_script | filesystem_test`
- `workspace_trust=trusted | unknown | untrusted`
- `policy_action=allow | ask | block`

Initial behavior can stay permissive for the current trusted workspace, but
proof should show that package scripts are different from direct cargo/python
validation.

### Implementation Steps

1. Extend `LabValidationCommandPlan` with `validation_kind`.
2. Add a workspace trust resolver:
   - explicit project config/env can mark the workspace as trusted;
   - default can be `unknown`;
   - future user approval can upgrade trust for the current run.
3. Add `validation_kind` and `workspace_trust` to
   `lab_validation_command_passed`, `failed`, and `blocked` events.
4. In unknown/untrusted workspaces, make package scripts either:
   - require explicit user approval; or
   - remain allowed but mark the GraduateResult as not fully verified.
5. Update `/lab proof` to show package-script validation separately.

### Acceptance Criteria

- `/lab proof` can show when validation came from a package script.
- The runner can distinguish direct validation from script-driven validation.
- Untrusted package-script execution has an explicit policy decision instead of
  being silently treated like `cargo test`.

## Implementation Closeout

Implemented on 2026-06-25:

- `/lab plan` now reports the actual gate state, validation status, and semantic
  blockers instead of unconditionally saying the gate was satisfied.
- Graduate runtime verification now enforces isolated-worktree proof when the
  provider execution policy requires it. Missing or parent-workspace isolation
  records `lab_graduate_isolation_missing` and cannot bind a verified
  GraduateResult.
- Postdoc read-only audit now records code-aware evidence: changed-file
  snippets, `git diff -- <path>` summaries, GraduateResult artifact links, and
  recent Lab validation event refs. Missing file, missing diff, invalid path, or
  missing validation events become explicit `postdoc_audit_needs_revision`
  risks that can block the postdoc gate.
- Lab validation events now include `validation_kind`, `workspace_trust`, and
  `policy_action`; package-script validation is visible as
  `validation_kind=package_script`, and explicitly untrusted package-script
  validation is blocked.
- `/lab proof` now displays validation kind/trust/action, isolated-worktree
  proof or isolation gaps, and postdoc audit status.

Remaining future hardening:

- Add an interactive trust approval flow for unknown package-script validation
  instead of relying only on proof metadata and explicit untrusted blocking.
- Turn postdoc audit evidence into a first-class `PostdocDiffAudit` artifact if
  the evidence surface grows beyond lightweight JSON proof files.

## Recommended Implementation Order

1. P0 `/lab plan` truthful gate output.
2. P1 isolated-worktree hard requirement for provider policies.
3. P1 Postdoc diff/file audit.
4. P2 package-script validation kind and workspace trust proof labels.
5. Update `docs/PROJECT_STATUS.md` after implementation and validation.

## Validation Plan

Run targeted tests first:

```bash
cargo test -q lab::commands --lib -- --test-threads=1
cargo test -q lab::orchestrator --lib -- --test-threads=1
cargo test -q lab::validation --lib -- --test-threads=1
```

Then run shared gates:

```bash
cargo fmt --check
git diff --check
cargo check -q
bash scripts/validate_docs.sh
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

## Definition Of Done

- `/lab plan` output reflects the true gate state.
- Postdoc audit evidence includes actual diff/file inspection or explicitly says
  why code inspection was not possible.
- Provider policy isolation requirements are enforced before verified graduate
  result binding.
- Package-script validation is visible as a distinct trust-sensitive validation
  category.
- `/lab proof` and `/lab trace` explain all four boundaries without requiring
  users to inspect raw JSON files.
