# LabRun Subagent Scope, Validation, And Profile Hardening Plan - 2026-06-26

Status: Implemented for P0/P1 runtime boundaries; P2 remains future hardening

Owner: Liz / gex

Source: external review notes on the latest LabRun evidence-lineage slice, checked
against the current repository on 2026-06-26.

## Implementation Update - 2026-06-26

Implemented in this slice:

- Added typed `LabExecutionBinding` metadata for Graduate dispatches and child
  agent execution.
- Propagated the binding through `ToolContext`, `QueryOptions`, `AgentConfig`,
  `AgentTool`, and durable child-agent task payloads.
- Enforced binding-based Graduate child mutation review before mutation-capable
  child tool actions can proceed.
- Required `lab-graduate` reserved profile execution to carry a valid
  `LabExecutionBinding`.
- Reserved `lab-professor`, `lab-postdoc`, and `lab-graduate` so project/user
  profile files cannot override system LabRun profiles.
- Recorded system profile origin/hash proof in subagent dispatch metadata.
- Made `lab-postdoc` read-only/audit-oriented by default.
- Replaced controlled validation `Command::output()` execution with a bounded
  direct-process runner that uses timeout, process cleanup, output caps,
  sanitized environment, and redacted persisted evidence.
- Promoted Postdoc audit redaction into shared runtime evidence redaction and
  applied it to validation stdout/stderr previews.
- Added non-noisy dependency-security CI and focused macOS CI.

Kept as explicit P2 future work:

- SQLite-authoritative LabStore transactions.
- Full property/fuzz harnesses for parsers and path normalization.
- SBOM, release provenance/signing, Actions SHA pinning, and automated secret
  scanning.

## Executive Judgment

These suggestions are useful and should be adopted. The previous hardening
slices made LabRun much stronger at the parent runtime, evidence, audit, and
policy layers, but this review identifies the next deeper boundary: the hard
contract must travel through the full chain from parent LabRun state into child
agent tool execution and validation subprocesses.

The highest-value next work is not another large role redesign. It is to make
three contracts explicit and enforceable:

1. Graduate subagents must inherit a concrete LabRun execution binding, and
   scope must be enforced before each mutation-capable child tool call.
2. Built-in LabRun profiles must be system-owned and non-overridable by project
   or user profile files.
3. Controlled validation must become a bounded process runner with timeout,
   process cleanup, output caps, sanitized environment, and redacted evidence.

This should be treated as the next release-trust hardening stream after
`LABRUN_EVIDENCE_LINEAGE_AND_CONCURRENCY_HARDENING_PLAN_2026-06-26.md`.

## Repo-Backed Observations

The review matches several current code facts:

- `src/lab/delegation.rs` passes `files`, `allowed_tools`, `profile =
  lab-graduate`, `context_mode = isolated_worktree_fork`, and LabRun metadata
  into `AgentTool`. This is good, but `files` is still primarily relevant-file
  context for subagents, not a typed filesystem allowlist enforced by every child
  tool before mutation.
- `src/lab/orchestrator/graduate.rs` and the recent policy work perform
  after-the-fact changed-file verification and parent-level policy review. That
  is necessary but not sufficient for shell side effects, network access, or
  writes that already happened inside the child worktree.
- `src/agent/profiles.rs` still has the general lookup shape
  `find_runnable_profile(project_root, name) = project/user profile first,
  product profile fallback`. That is correct for normal custom agents, but it is
  too permissive for reserved LabRun system roles such as `lab-graduate`.
- `src/agent/profiles.rs` currently defines `lab-postdoc` with mutation-capable
  tools and `IsolatedWrite`. That conflicts with the newer LabRun policy
  direction where Postdoc is primarily read/audit/integration and Graduate is
  the normal implementation role.
- `src/lab/validation.rs` no longer uses `sh -lc`, but it currently executes
  allowlisted commands via `std::process::Command::output()`. That still
  inherits the process environment, waits without an explicit LabRun timeout,
  collects complete stdout/stderr before compaction, and records validation
  previews without the same redaction guarantees used for Postdoc audit text.
- The current evidence-lineage work adds strong provenance fields, but it still
  needs end-to-end tests that prove cycle/task/dispatch evidence cannot be mixed
  across LabRun cycles or parallel GraduateTasks.

## Target Invariants

After this plan is complete, the following should be true:

- A LabRun Graduate mutation is authorized by one active task and one dispatch,
  never by all open GraduateTasks in the run.
- Every mutation-capable child tool call sees the same LabRun execution binding
  that the parent dispatch created.
- A scope violation is blocked before file write, file edit, format, bash, or
  similar mutation-capable child execution begins.
- LabRun internal profiles cannot be replaced or expanded by
  `.priority-agent/agents/*.toml` in the project or user home directory.
- Validation evidence is bounded, redacted, and honest about its security
  semantics: controlled direct process execution, not a sandbox.
- Postdoc and Professor cannot accidentally borrow evidence from an older
  cycle, different task, different dispatch, or different verification root.

## P0 - Execution-Boundary Hardening

### 1. Add `LabExecutionBinding`

Create a typed binding that can be serialized into `ToolContext` metadata and
child-agent state.

Suggested location:

- `src/lab/execution_binding.rs`, or
- `src/lab/provenance.rs` if we decide to group provenance types together.

Suggested fields:

```rust
pub struct LabExecutionBinding {
    pub project_root: PathBuf,
    pub lab_run_id: String,
    pub cycle_id: Option<String>,
    pub source_postdoc_plan_artifact_id: Option<String>,
    pub graduate_task_id: String,
    pub dispatch_id: String,
    pub agent_task_id: String,
    pub allowed_scope: Vec<String>,
    pub verification_root: PathBuf,
    pub lab_state_version: Option<String>,
}
```

Required behavior:

- `build_graduate_task_dispatch()` and
  `execute_graduate_task_with_agent_tool()` create the binding once from the
  durable GraduateTask and dispatch record.
- The binding is passed into `AgentTool` as structured metadata, not inferred
  from the isolated worktree's `.priority-agent/lab` directory.
- `ToolContext` exposes a parsed helper such as
  `ToolContext::lab_execution_binding()`.
- Missing or malformed LabRun binding in a LabRun Graduate execution fails
  closed.

### 2. Enforce Graduate Scope Before Child Mutations

Wire `LabExecutionBinding` into the child tool action-review path.

Required behavior:

- `file_write`, `file_edit`, `file_patch`, `format`, `bash`, and other
  mutation-capable child tool actions must be checked against the active binding
  before execution.
- Scope matching must use the existing LabRun path normalization helpers, not
  ad hoc string prefix checks.
- Bash should not be treated as safe merely because the command string is
  allowlisted at a high level. If we cannot prove the command writes only within
  the task scope, it should require a narrower validation runner, a sandbox, or
  explicit user approval.
- Parent-level post-execution scope verification stays in place as defense in
  depth, but it is no longer the first hard boundary.

Required tests:

- Graduate task scope is `src/lab`; child `file_write("README.md")` is rejected
  before `README.md` changes.
- Graduate task A scope is `src/api`; Graduate task B scope is `src/memory`;
  a child call under task A cannot use task B's scope.
- Child bash attempting an out-of-scope write is blocked before execution, or
  clearly downgraded to ask/block when the write target cannot be proven safe.
- A missing binding during `lab-graduate` execution blocks mutation and records
  a `labrun_policy_blocked` event.

### 3. Reserve Built-In LabRun Profiles

Protect these profile names:

```text
lab-professor
lab-postdoc
lab-graduate
```

Required behavior:

- Normal project/user profiles remain supported for ordinary agent work.
- LabRun internal dispatch must resolve reserved roles from product profiles
  only, or construct an internal `AgentDefinition` directly.
- A project file such as
  `.priority-agent/agents/lab-graduate.toml` must not override system prompt,
  permission mode, risk policy, MCP servers, memory policy, timeout, or allowed
  tool surface for LabRun execution.
- Agent proof should record:
  - `profile_name`
  - `profile_origin = system`
  - `profile_version`
  - `profile_hash`
  - `requested_allowed_tools`
  - effective allowed tools after disallowed-tool filtering

Required tests:

- A malicious project profile named `lab-graduate` with extra MCP servers does
  not affect LabRun dispatch.
- A malicious project profile named `lab-postdoc` does not change LabRun
  Postdoc policy.
- Normal non-reserved project profiles still work for `/agent run` and regular
  subagents.

### 4. Replace Validation `Command::output()` With A Bounded Runner

Implement a direct-process runner for controlled validation.

Suggested type:

```rust
pub struct ControlledProcessRunner {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub timeout: Duration,
    pub max_stdout_bytes: usize,
    pub max_stderr_bytes: usize,
    pub sanitized_env: BTreeMap<String, String>,
}
```

Required behavior:

- No shell fallback.
- Explicit timeout per validation command.
- Kill the process group or process tree on timeout where the platform supports
  it.
- Bound stdout and stderr before allocating unbounded strings.
- Record whether stdout/stderr was truncated.
- Use a sanitized environment that removes provider credentials and common
  secret-bearing variables.
- Keep a small allowlist of environment variables required for deterministic
  local builds, such as `PATH`, `HOME` if needed, `CARGO_HOME` if needed, and
  stable locale variables. The exact allowlist should be documented in code.
- Validation event payloads should record:
  - `timeout_secs`
  - `timed_out`
  - `terminated_process_tree`
  - `stdout_truncated`
  - `stderr_truncated`
  - `environment_policy`
  - `validation_security = controlled_not_sandboxed`

Required tests:

- A command that sleeps past the timeout is terminated and records timeout
  evidence.
- A command with very large stdout/stderr is capped without large memory growth.
- Environment variables such as `OPENAI_API_KEY`, `Authorization`, and provider
  keys are not visible to the child validation process.
- Existing allowlisted validation commands still pass classification tests.

### 5. Promote Redaction To Runtime Evidence

The current Postdoc audit redaction should become a shared runtime evidence
redactor.

Suggested type:

```rust
pub struct RuntimeEvidenceRedactor;
```

Apply it to:

- validation stdout/stderr previews,
- validation failure errors surfaced to LabRun,
- Postdoc file snippets and diff previews,
- provider error previews,
- future trace or proof exports that include tool output.

Required tests:

- `OPENAI_API_KEY=sk-...` is redacted.
- `Authorization: Bearer ...` is redacted.
- private-key blocks are redacted.
- high-entropy token-like strings are redacted.
- redaction happens before event persistence, not only at render time.

## P1 - Workflow Semantics And Evidence Tests

### 1. Add LabRun Provenance E2E Tests

Create a focused test module such as `src/lab/provenance_tests.rs` or expand
the orchestrator integration tests with named helpers.

Required scenario:

```text
Cycle 0:
  PostdocPlan A
  GraduateTask A
  Validation A passed
  GraduateResult A

Cycle 1:
  PostdocPlan B
  GraduateTask B
  Validation B failed
  GraduateResult B

Postdoc integrates Cycle 1
```

Assertions:

- Cycle 1 integration includes only B.
- Cycle 1 cannot borrow A's validation event.
- Cycle 1 cannot borrow A's GraduateResult.
- Cycle 1 cannot borrow A's diff/worktree proof.
- ProfessorReview cannot accept Cycle 1 using Cycle 0 evidence.

Also test parallel tasks:

```text
Task A scope = src/api
Task B scope = src/memory
```

Assertions:

- Task A cannot use Task B's validation event.
- Task A cannot use Task B's dispatch or worktree proof.
- Task A cannot inherit Task B's allowed scope.

### 2. Resolve `lab-postdoc` Role Semantics

Recommended model:

```text
Professor:
  strategy and final acceptance; no code writes

Postdoc:
  planning, review, integration, audit; default read-only

Graduate:
  normal scoped implementation role

Postdoc Repair:
  exceptional repair path, expressed as a separate scoped task/profile or routed
  back through a GraduateTask
```

Implementation options:

- Make `lab-postdoc` read-only and audit-oriented.
- Add an explicitly named `lab-postdoc-repair` profile only if we truly need a
  senior repair role.
- Prefer routing repair through a new GraduateTask so the existing scope,
  dispatch, validation, and Postdoc audit machinery remains consistent.

Required tests:

- `lab-postdoc` cannot get `file_edit`, `file_write`, or generic `bash` in
  normal LabRun policy.
- Any Postdoc repair path must produce a distinct artifact and explicit proof
  label.

### 3. Strengthen Workspace Trust Records

The current project-scoped trust file is a good foundation. Extend it gradually
from a single trust level into scoped approvals.

Suggested fields:

```json
{
  "canonical_path": "...",
  "repo_identity": "...",
  "repo_fingerprint": "...",
  "trusted_at": "...",
  "approved_by": "...",
  "trust_scopes": [
    "allow_tests",
    "allow_package_scripts"
  ]
}
```

Required behavior:

- Package-script validation checks `allow_package_scripts`, not just a broad
  trusted/untrusted value.
- `/lab proof` and `/permissions` show trust source and scope.
- Process-level environment trust remains an explicit override, not the normal
  persistent trust mechanism.

### 4. Add Non-Noisy Dependency Security CI

Dependabot branch noise was intentionally removed. That does not mean dependency
security should remain manual forever.

Recommended CI job:

```text
dependency-security:
  cargo install cargo-audit cargo-deny if absent or use cached tools
  bash scripts/security_dependency_audit.sh
```

Requirements:

- No dependency-update PR branches.
- No automatic manifest edits.
- Fails the CI job when audit or deny fails.
- Tool-missing behavior should be acceptable locally, but CI should provision
  the tools so the job can produce a real pass/fail.

### 5. Add macOS Focused CI

Because the public target is macOS and Linux, add a focused macOS job:

```text
macos-latest:
  cargo check --workspace --all-targets --all-features
  cargo test -q lab::validation --lib -- --test-threads=1
  cargo test -q lab::policy_overlay --lib -- --test-threads=1
  priority-agent --help smoke
```

Keep Ubuntu as the full gate. Treat Windows as experimental until installer,
shell defaults, and credential paths are validated.

## P2 - Longer-Term Concurrency And Supply-Chain Maturity

### 1. Make SQLite The Authoritative Transactional Lab Store

Current LabStore uses JSON files, JSONL events, SQLite indexes, active pointers,
lease files, artifacts, and task/dispatch files. This is workable, but long-term
multi-process LabRun needs stronger transaction boundaries.

Direction:

```sql
BEGIN IMMEDIATE;
  update task;
  update run;
  insert event;
  update artifact index;
COMMIT;
```

File artifacts can remain readable mirrors, but SQLite should become the
authoritative state transition log for concurrent scheduling.

Fault-injection tests to add:

- task saved, run not saved,
- artifact written, gate not written,
- dispatch succeeded, result binding not written,
- event append interrupted,
- two schedulers compete for the same lease,
- user command and daemon scheduler update the same run concurrently.

### 2. Add Property/Fuzz Tests For Security Boundaries

Best candidates:

- Lab path normalization,
- validation command parser,
- bash command classifier,
- runtime evidence redaction,
- artifact JSON parsing,
- tool parameter path extraction.

Start with property tests before introducing a full fuzzing harness. Move to
`cargo fuzz` once the target APIs are stable and small enough.

### 3. Supply Chain Release Hardening

Defer until the P0/P1 runtime boundaries are stable:

- SBOM generation,
- release provenance attestations,
- release signing,
- GitHub Actions SHA pinning,
- automated secret scanning,
- license allow/deny review beyond the baseline `deny.toml`.

## Suggested Implementation Order

1. Add `LabExecutionBinding` and propagate it through Graduate dispatch,
   `ToolContext`, and child-agent state.
2. Enforce binding-based scope review before child mutation-capable tool calls.
3. Protect reserved LabRun profiles and persist profile origin/hash proof.
4. Replace validation `Command::output()` with the bounded direct-process
   runner.
5. Promote audit redaction into shared runtime evidence redaction and apply it
   to validation events.
6. Add provenance E2E tests for cycle/task/dispatch isolation.
7. Resolve `lab-postdoc` into read-only default semantics, with explicit repair
   workflow if needed.
8. Add non-noisy dependency security CI and focused macOS CI.
9. Start longer-term store transaction and fuzz/property test work.

## Acceptance Criteria

This plan is complete when:

- a child Graduate tool cannot mutate outside its active task scope before the
  mutation happens;
- LabRun reserved profiles cannot be overridden by project or user profiles;
- validation process execution has timeout, kill, output cap, sanitized env, and
  redacted evidence;
- validation and Postdoc audit proofs remain tied to the exact cycle, plan,
  task, dispatch, result, and verification root;
- `lab-postdoc` semantics no longer conflict with LabRun policy;
- dependency security CI runs without creating dependency-update branches;
- docs and `PROJECT_STATUS.md` record the implemented behavior and validation
  evidence.

## Validation Plan

Use focused tests during implementation:

```bash
cargo test -q lab::delegation --lib -- --test-threads=1
cargo test -q lab::policy_overlay --lib -- --test-threads=1
cargo test -q lab::validation --lib -- --test-threads=1
cargo test -q lab::orchestrator --lib -- --test-threads=1
cargo test -q tools::agent_tool --lib -- --test-threads=1
cargo test -q agent::profiles --lib -- --test-threads=1
```

Before closing the workstream:

```bash
cargo fmt --check
git diff --check
cargo check -q
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features -- --test-threads=1
cargo doc --workspace --all-features --no-deps
bash scripts/validate_docs.sh
```

If dependency security CI is added in this slice, also verify:

```bash
bash scripts/security_dependency_audit.sh
```

## Non-Goals

- Do not weaken validation, permission, checkpoint, or evidence gates to make a
  weaker provider appear successful.
- Do not re-enable noisy Dependabot branch creation as part of this plan.
- Do not claim controlled validation is a sandbox. It is safer command
  execution with direct process control; untrusted repositories still need a
  real sandbox/container boundary.
- Do not perform a broad directory reorganization before the new boundary types
  are stable.
