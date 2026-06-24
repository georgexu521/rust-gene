# Friend Review Code Quality Improvement Plan
Status: Implemented
Created: 2026-06-24
Implemented: 2026-06-24

## Purpose

This document turns a friend's external static review into a repo-backed quality
improvement plan for Priority Agent.

The review is useful because it points at release-trust problems rather than
only style problems: project identity, CI truthfulness, `priority-core`
correctness, tool-cache safety, bash/path safety, credential storage disclosure,
dependency-cycle handling, and platform support boundaries.

The review was based on GitHub static reading, not a local checkout. I
therefore re-checked the claims against the current local repository before
turning them into a plan.

## Executive Conclusion

The review is directionally right and worth acting on.

Priority Agent is not a toy project anymore. The current codebase already has
real runtime boundaries, tools, permissions, memory, MCP, desktop workbench,
provider support, live-eval evidence, docs maps, and release cleanup work. The
next quality step is not "add more features"; it is to make the public release
surface harder to distrust.

The biggest confirmed issues are:

- `priority-core` has a real stack-overflow bug in `ExecutableTask` ordering.
- The repository/product naming story is still confusing across `rust-gene`,
  `rust-agent`, and `priority-agent`.
- GitHub Actions quality gates still contain placeholder reporting that can
  make the repo look more release-ready than the actual CI proves.
- `ToolResultCache` is fail-open for unlisted tools, which is the wrong default
  for a coding agent even if the cached executor is not currently the main
  runtime path.
- Credential storage and platform support need clearer public disclosure before
  a broad release claim.

The review also needs refinement:

- Dependency-cycle handling already exists in `src/internal/task_analyzer`, but
  `priority-core::WeightCalculator` still lacks cycle guards.
- Bash analysis has improved: there is command classification and a
  tree-sitter-bash shadow parser. The remaining issue is that final block/allow
  safety still partly depends on string-pattern checks.
- The cache risk is currently latent because `CachedToolExecutor` does not
  appear to be wired into the main tool execution path. It is still a real
  library/API risk and should be fixed before future reuse.

## Evidence Checked

| Area | Current evidence | Assessment |
|------|------------------|------------|
| Project identity | `Cargo.toml` package is `priority-agent`, repository URL is `georgexu521/rust-agent`; README/QUICKSTART still use `~/Desktop/rust-agent`; pushed remote target is `georgexu521/rust-gene`. | Confirmed. Needs a documented naming contract and metadata cleanup. |
| `priority-core` ordering | `priority-core/src/weight_engine/calculator.rs` has `PartialOrd::partial_cmp -> self.cmp` and `Ord::cmp -> self.partial_cmp`. | Confirmed. This is a P0 correctness bug. |
| Runtime proof | `cargo test -q -p priority-core test_priority_queue` aborts with stack overflow. | Confirmed runtime failure, not just theoretical review feedback. |
| `priority-core` architecture | `priority-core/src/lib.rs` still exposes placeholder `engine`, `tools`, `agent`, and `services` modules with TODO comments. | Confirmed. It should either become a real core crate or be labeled experimental. |
| CI quality gates | `.github/workflows/ci.yml` has real check/test jobs, but `quality-gates` has placeholder TODO/FIXME checking and echo-only G0-G5 reporting; release job only echoes a version. | Confirmed. CI truthfulness needs hardening. |
| Docs validation | `scripts/validate_docs.sh` is stronger than the GitHub quality-gate job: it checks required docs, file-size ceiling, advisory rustdoc audit, `cargo check --all-features`, and workflow-enabled tests. | Important nuance. The fix is to wire real scripts into CI, not invent a separate gate. |
| Tool cache | `ToolCacheConfig` has a default 60-second TTL for tools not explicitly configured; tests currently cache `bash` per working dir. | Confirmed. Should become explicit allowlist only. |
| Cache runtime use | `CachedToolExecutor` is exported but only referenced in comments/public API; main runtime appears to use direct registry/tool execution. | Latent risk, still worth fixing before someone wires it into production. |
| Bash safety | `validate_command_safety` still scans string patterns like `rm -rf /`; command classifier has richer parsing and tree-sitter shadow observations. | Partially confirmed. Improve final policy to use structured analysis where possible. |
| Credentials | `credentials.rs` and `auth_store.rs` persist provider keys to `~/.priority-agent/.env`; Unix permissions are set to `0600`; Keychain status remains `from_keychain: false`. | Confirmed. Needs public disclosure and a secret-store roadmap. |
| Dependency cycles | `src/internal/task_analyzer/dependency_graph.rs` has cycle detection, but `priority-core::WeightCalculator::calculate_dependency_depth` recursively follows deps without a visited set. | Partially confirmed. Fix the core crate surface. |
| Platform boundary | `src/main.rs` falls back to `/dev/null` for CLI/TUI logs; `scripts/install.sh` assumes bash and `/usr/bin/install`. | Confirmed. Document macOS/Linux support or add portable branches. |

## Recommendation

Borrow the review's direction, but make it Priority Agent-specific:

- Do not rename the product casually. The product/binary name can remain
  `Priority Agent` / `priority-agent` / `pa`. The repository name and release
  metadata must explain that clearly.
- Do not weaken runtime safety to make CI or evals greener. Fix gates and
  contracts.
- Treat `priority-core` as the first hardening target because it has a proven
  failing test and affects trust in the workspace split.
- Treat CI truthfulness as a release blocker. A weak CI badge is worse than no
  badge because it encourages false confidence.
- Fix latent safety APIs even if they are not yet on the main path. Coding-agent
  safety bugs become serious the moment a future integration reuses them.

## P0: Release-Trust Blockers

These should be fixed before another broad "release-ready" claim.

### P0.1 Fix `ExecutableTask` Ordering

Problem:

- `ExecutableTask`'s `Ord` and `PartialOrd` implementations recurse into each
  other.
- `cargo test -q -p priority-core test_priority_queue` currently aborts with a
  stack overflow.

Plan:

- Implement `Ord::cmp` directly with `f64::total_cmp`.
- Use deterministic tie-breakers:
  - `priority_score`
  - `absolute_weight.value()`
  - `blocking_count`
  - `dependency_depth`
  - `task_id.0`
- Make `PartialOrd::partial_cmp` return `Some(self.cmp(other))`.
- Make `PartialEq`/`Eq` consistent with the ordering. Prefer identity equality
  on `task_id` only if the ordering also ends with `task_id`, or compare the
  same fields used in `cmp`.
- Add tests for:
  - `BinaryHeap` pop order.
  - equal priority deterministic tie-break.
  - `NaN` priority does not panic or recurse.

Validation:

```bash
cargo test -q -p priority-core test_priority_queue
cargo test -q -p priority-core weight_engine
cargo test -q -p priority-core
```

Acceptance:

- No stack overflow.
- Priority queue order is deterministic.
- `priority-core` tests pass independently.

### P0.2 Add Cycle Guards To `priority-core`

Problem:

- The internal dependency graph already detects cycles, but
  `priority-core::WeightCalculator::calculate_dependency_depth` still recurses
  without `visiting` / `visited` guards.

Plan:

- Add a depth helper with:
  - `visiting: HashSet<TaskId>` for active recursion.
  - `memo: HashMap<TaskId, usize>` for already-computed depths.
- Decide the public behavior for cycles:
  - Short term: return bounded depth and mark the task as blocked.
  - Better: expose a `Result` path for cycle-aware APIs.
- Add tests for:
  - direct cycle `a -> a`.
  - two-node cycle `a -> b -> a`.
  - normal chain `a -> b -> c`.
  - missing dependency stays bounded.

Validation:

```bash
cargo test -q -p priority-core dependency
cargo test -q -p priority-core weight_engine
```

Acceptance:

- No dependency traversal can infinite recurse.
- Cycle behavior is documented in rustdoc or module docs.

### P0.3 Make Tool Result Cache Fail Closed

Problem:

- `ToolResultCache` currently applies the default TTL to tools not explicitly
  listed in `tool_ttls`.
- Existing tests prove `bash` can be cached per working directory.
- The main runtime does not appear to use `CachedToolExecutor` today, but the
  exported wrapper is unsafe-by-default for future wiring.

Plan:

- Add an explicit cacheability function:

```rust
fn is_cacheable_tool(tool_name: &str) -> bool {
    matches!(tool_name, "calculate" | "glob" | "grep" | "file_read" | "project_list")
}
```

- Make unlisted tools non-cacheable by default.
- Keep `datetime` non-cacheable unless a caller explicitly asks for a stable
  time fixture in tests.
- Ensure mutation-capable tools never hit cache:
  - `bash`
  - `file_write`
  - `file_edit`
  - `file_patch`
  - `memory_save`
  - `git_*`
  - `install_dependencies`
  - `agent`
- Add tests named around the contract:
  - `bash_is_not_cached_by_default`
  - `file_edit_is_not_cached_by_default`
  - `read_only_tools_can_be_cached_when_allowlisted`

Validation:

```bash
cargo test -q tools::cache --lib
cargo test -q registry --lib
```

Acceptance:

- Default policy is no-cache unless explicitly allowlisted.
- Cache cannot bypass mutation permissions or side effects.

### P0.4 Make CI Quality Gates Real

Problem:

- `.github/workflows/ci.yml` has real check/test jobs, but the
  `quality-gates` job still contains placeholder checks and echo-only gate
  reports.
- The release job does not build/upload artifacts.

Plan:

- Replace placeholder quality-gate steps with real commands:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo test --workspace --all-features
cargo doc --workspace --all-features --no-deps
cargo clippy --workspace --all-targets --all-features -- -D warnings
bash scripts/validate_docs.sh
bash scripts/check_source_file_sizes.sh
git diff --check
```

- If full `--workspace --all-features` is too slow for every PR, split CI into:
  - PR required: fmt, check, clippy, focused tests, docs validation, diff check.
  - nightly/manual release: full workspace tests, full docs, package artifacts.
- Generate the gate report from command exit status, not static echo.
- Keep the existing `scripts/validate_docs.sh` as the single source for docs
  consistency instead of duplicating logic in YAML.

Validation:

```bash
git diff --check
cargo fmt --check
bash scripts/validate_docs.sh
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Acceptance:

- CI cannot print G0-G5 PASS unless the corresponding commands actually ran.
- The README release baseline and GitHub Actions enforce the same gate family.

### P0.5 Normalize Project Identity For Release Metadata

Problem:

- Product: `Priority Agent`.
- Binary: `priority-agent` / `pa`.
- Local historical folder/docs: `rust-agent`.
- GitHub target repo currently pushed as `georgexu521/rust-gene`.
- `Cargo.toml` still points `repository` at `georgexu521/rust-agent`.

Recommended naming contract:

- Product name: `Priority Agent`.
- Binary name: `priority-agent`; shortcut `pa`.
- Crate package name: keep `priority-agent` unless there is a deliberate
  crates.io rename decision.
- Repository URL: update to the actual release repository, currently
  `https://github.com/georgexu521/rust-gene`, or explicitly decide that the
  release repo should be renamed back to `rust-agent`.
- Local path examples: avoid `cd ~/Desktop/rust-agent` in public docs; use
  `cd priority-agent` or `cd rust-gene` depending on final repository decision.

Plan:

- Update `Cargo.toml` `repository`.
- Update README and QUICKSTART path examples.
- Add a short "Naming" note:
  - "The repository is `rust-gene`; the product and binary are
    `Priority Agent` / `priority-agent`."
- Check docs for stale public references, excluding archived historical docs.

Validation:

```bash
rg -n "rust-agent|rust-gene|priority-agent|~/Desktop/rust-agent" README.md QUICKSTART.md Cargo.toml docs scripts .github
cargo metadata --no-deps
```

Acceptance:

- A new user can understand the repo/product/binary distinction in under one
  minute.
- Release metadata no longer points at the wrong repository.

### P0.6 Be Honest About Credentials And Platform Support

Problem:

- Credentials are saved to `~/.priority-agent/.env`.
- Unix mode is set to `0600`, but the file is still plaintext.
- Keychain support is not implemented (`from_keychain: false`).
- CLI/TUI log fallback uses `/dev/null`; install script assumes Unix tools.

Plan:

- Update README/QUICKSTART/provider setup docs:
  - "Current provider keys are stored in a local plaintext env file with 0600
    permissions on Unix."
  - "Do not use this storage mode on shared machines."
  - "Keychain/Secret Service/Windows Credential Manager is not implemented yet."
- State support target:
  - Short term: macOS/Linux.
  - Windows: best-effort unless portable logging and installer path are fixed.
- Replace `/dev/null` fallback with a portable sink or platform branch when
  practical.

Validation:

```bash
cargo test -q credentials --lib
cargo test -q auth_store --lib
rg -n "/dev/null|Credential Manager|Keychain|plaintext|0600" README.md QUICKSTART.md docs src
```

Acceptance:

- Public docs no longer imply stronger credential security than the code
  provides.
- Platform support is explicit rather than accidental.

## P1: Safety And Architecture Hardening

### P1.1 Move Bash Blocking Toward Structured Policy

Current state:

- Command classifier has categories, mutation paths, redirection facts, arity
  suggestions, and a tree-sitter-bash shadow parser.
- Final dangerous-command blocking still relies partly on substring patterns.

Plan:

- Make the structured classifier the primary safety input for bash permission
  and block/ask decisions.
- Keep the old dangerous string patterns as fallback defense, not the only
  policy.
- Add regression tests for:
  - `rm -rf /`, `rm -rf /*`, `sudo rm -rf /`.
  - `rm -rf ./target` remains allowed/ask-scoped, not globally blocked.
  - bare `~`, `~/`, `$HOME`, and quoted home paths.
  - command substitution, eval, curl/wget pipe to shell.
  - write redirections and `tee`.
  - Windows path-like tokens if Windows support is claimed.

Validation:

```bash
cargo test -q bash_tool --lib
cargo test -q command_classifier --lib
```

Acceptance:

- Known destructive commands are blocked.
- Normal workspace cleanup is not blocked as a false positive.
- Mutation and high-risk commands remain gated through permissions.

### P1.2 Clarify `priority-core` Ownership

Problem:

- `priority-core` looks like a stable core crate but still has TODO placeholder
  modules.

Plan options:

1. Core-library path:
   - Move stable, reusable priority/weight/task types into `priority-core`.
   - Keep runtime/provider/tools in the root crate until interfaces stabilize.
   - Add `priority-core` public API docs.

2. Experimental-crate path:
   - Rename docs wording to "experimental priority model crate".
   - Remove or privatize placeholder `engine`, `tools`, `agent`, and `services`
     modules.
   - Do not present it as the core runtime crate.

Recommendation:

- Take option 2 first. It is honest and low-risk.
- Revisit option 1 after the release candidate is stable.

Validation:

```bash
cargo doc -p priority-core --no-deps
cargo test -q -p priority-core
```

Acceptance:

- `priority-core` no longer advertises APIs it does not implement.
- New contributors know whether to modify root `src/` or `priority-core/`.

### P1.3 Add Safety-Focused Property/Fuzz Tests

Targets:

- Path policy and workspace containment.
- Bash command classification.
- Tool cache allowlist.
- File mutation read-before-write guard.
- Credential env parsing/quoting.

Plan:

- Start with table-driven tests before adding a fuzz harness.
- Add `proptest` only when the table-driven suite stabilizes.
- Keep tests deterministic and cheap enough for PR CI.

Acceptance:

- Safety regressions become test failures, not review comments.

## P2: Release Maturity

### P2.1 Make Release Job Produce Artifacts

Plan:

- Build release binaries for supported platforms.
- Upload artifacts from GitHub Actions.
- Generate checksums.
- Create a draft GitHub Release for tagged commits.
- Run installer smoke tests against the built artifact.

Acceptance:

- "Release candidate" means there is a downloadable candidate, not just a log
  line.

### P2.2 Align `QUALITY_GATES.md` With Reality

Problem:

- `QUALITY_GATES.md` is useful but not fully aligned with current scripts and
  CI.

Plan:

- Update gate commands to match actual CI and local release gates.
- Distinguish:
  - PR gate.
  - release candidate gate.
  - live-eval/dogfood gate.
  - desktop packaged gate.
- Remove vague claims like "if benchmarks pass" unless the benchmark command is
  listed.

Acceptance:

- A maintainer can run the exact gate set from one doc.

### P2.3 Keep Runtime Evidence As The Quality Standard

The project should continue using the stronger pattern we have been building:

- required commands
- diff state
- runtime-spine trace
- verification proof
- failure owner
- closeout status
- live-eval report artifacts

This is more valuable than generic "AI agent best practice" prose. CI and docs
should point back to real evidence, not just model claims.

## Suggested Execution Order

1. **Commit 1: `priority-core` correctness**
   - Fix `ExecutableTask` ordering.
   - Add dependency-depth cycle guard.
   - Add deterministic tests.

2. **Commit 2: cache fail-closed**
   - Make tool-result cache allowlist-only.
   - Remove or update tests that cache `bash`.
   - Add mutation-tool no-cache tests.

3. **Commit 3: CI gate truthfulness**
   - Replace placeholder quality gates with real script/command execution.
   - Wire `scripts/validate_docs.sh` and `git diff --check`.

4. **Commit 4: identity and public honesty**
   - Normalize repository URL and docs examples.
   - Add naming note.
   - Add credential plaintext and platform support disclosure.

5. **Commit 5: bash safety hardening**
   - Promote structured command analysis into final bash safety decisions.
   - Add false-positive/false-negative tests.

6. **Commit 6: release artifact path**
   - Add release artifacts/checksums/draft release automation.

## Release-Ready Acceptance Criteria For This Plan

Before treating this plan as complete:

- `cargo test -q -p priority-core` passes.
- `cargo test -q tools::cache --lib` proves mutation tools are not cached.
- `cargo test -q bash_tool --lib` covers the new command-safety cases.
- `.github/workflows/ci.yml` has no placeholder quality-gate pass messages.
- README/QUICKSTART/Cargo metadata explain the repo/product/binary naming.
- Credential docs clearly say plaintext env file + Unix `0600`, not Keychain.
- Platform docs say macOS/Linux only or code has portable fallbacks.
- `git diff --check` passes.
- `cargo fmt --check` passes.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  passes, or any exception is documented with a scoped follow-up.

## Open Decisions For gex

1. Should the public GitHub repository name stay `rust-gene`, or should it be
   renamed to match the product/repo history?
2. Should crates.io package name stay `priority-agent`?
3. Is Windows support a release goal now, or should the first public release
   explicitly target macOS/Linux only?
4. Should Keychain/Secret Service support block the first public release, or is
   clear plaintext disclosure acceptable for an early release?

## My Recommendation

Keep the product name `Priority Agent` and binary `priority-agent` / `pa`.
Treat `rust-gene` as the repository name only if that name is intentional and
public-facing. If it is not intentional, rename the repository before release.

Do the P0 work before adding more product surface. The project already has
enough product breadth; the next visible quality jump will come from making the
hard gates and core contracts trustworthy.

## Implementation Closeout

Completed on 2026-06-24.

Implemented changes:

- Fixed `priority-core` task ordering so `Ord`, `PartialOrd`, and `PartialEq`
  no longer recurse and use deterministic tie-breakers.
- Added bounded dependency-depth traversal in `priority-core` with cycle guards
  and regression tests.
- Changed `ToolResultCache` to fail closed: only explicitly allowlisted tools
  with positive TTLs are cached.
- Updated bash safety preflight to use the shared structured dangerous-command
  detector and added home-directory destructive-command regressions.
- Split `ShellCommandView` into `src/tools/bash_tool/command_classifier/view.rs`
  to keep the main classifier below the release file-size ceiling.
- Clarified `priority-core` as an experimental priority model crate and removed
  placeholder public modules.
- Replaced CI placeholder quality gates with executable checks, docs
  validation, whitespace validation, all-features clippy, serialized
  all-features tests, release artifact packaging, checksums, and draft GitHub
  release creation for `v*` tags.
- Normalized public release identity around repository `georgexu521/rust-gene`,
  product `Priority Agent`, command/crate `priority-agent`, and shortcut `pa`.
- Documented plaintext credential storage, Unix `0600` permissions, lack of
  Keychain/Secret Service/Windows Credential Manager support, and macOS/Linux
  release target.
- Updated `QUALITY_GATES.md` so maintainers can run PR, release candidate,
  runtime dogfood, desktop/TUI, and CI gates from one current document.
- Fixed all-features provider timeout DTO tests so they verify legacy env
  isolation without assuming stale default timeout values.
- Refactored `record_stop_check` inputs to avoid a broad internal function
  signature and keep clippy all-targets/all-features clean.

Validation evidence:

```bash
cargo test -q -p priority-core
cargo test -q tools::cache --lib
cargo test -q bash_tool --lib
cargo test -q command_classifier --lib
cargo test -q turn_iteration_controller --lib
cargo test --workspace --all-features api::dto::provider::tests -- --test-threads=1
cargo check -q
cargo check --features experimental-api-server -q
cargo check --features legacy-cli -q
cargo fmt --check
git diff --check
cargo doc --workspace --all-features --no-deps
cargo clippy --workspace --all-targets --all-features -- -D warnings
bash scripts/validate_docs.sh
CARGO_INCREMENTAL=0 cargo test --workspace --all-features -- --test-threads=1
```

Notable verification details:

- Final `bash scripts/validate_docs.sh` passed after the code/docs updates,
  confirmed all required docs, 72 registered tools, 148 registered commands,
  no production Rust file above the 1500-line ceiling, successful
  `cargo check --all-features`, and 3143 workflow-script tests with 0 failures
  and 1 ignored test.
- `CARGO_INCREMENTAL=0 cargo test --workspace --all-features -- --test-threads=1`
  passed with 3199 library tests, 0 failures, and 1 ignored doc-related test,
  plus integration tests and `priority-core` tests.
- The first broad all-features test run exposed a stale provider timeout test
  assumption; it was fixed and the broad gate was rerun successfully.
