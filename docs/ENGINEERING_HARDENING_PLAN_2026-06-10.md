# Priority Agent Engineering Hardening Plan

Date: 2026-06-10
Last Updated: 2026-06-10
Status: **Active**
Previous Plan: `docs/LLM_RUNTIME_SIMPLIFICATION_PLAN_2026-05-08.md` (completed)

## Goal

Keep Priority Agent maintainable as it grows. The runtime simplification plan
fixed prompt bloat and workflow over-control. The opencode core alignment plan
defined the next session-store reliability work. This plan fixes the remaining
engineering debt that slows down future development.

Target: a codebase where:
- Files stay under the 1500-line limit the team already agreed on.
- Configuration is discoverable and typed through the existing
  `services::config::AppConfig`, not scattered in `env::var` calls.
- Core paths (storage, permissions, tool execution) fail gracefully with
  context-rich errors instead of panicking.
- Tests cover complete user flows, not just unit-level behavior.
- Documentation is current; completed plans move to archive.

## Phase 0 — Code Size Stewardship

Status: **Completed**

Purpose: enforce the existing "source files under 1500 lines" rule.

### Current State

Four production files currently exceed 1500 lines. One more file is close
enough to treat as a warning:

| File | Lines | Status | Nature | Split Strategy |
|---|---:|---|---|---|
| `src/session_store/session_parts.rs` | 1743 | Over limit | Projection logic + tests | Extract `projection.rs` and `tests.rs` submodules |
| `src/api/routes.rs` | 1726 | Over limit | API route handlers | Group by domain: `routes/session.rs`, `routes/export.rs`, `routes/tool_output.rs`, etc. |
| `src/tui/session_manager.rs` | 1580 | Over limit | TUI session logic + export builder | Extract `export_builder.rs` and narrow reload/export helpers |
| `src/engine/streaming.rs` | 1511 | Over limit | Streaming engine + compaction events | Extract `compaction_events.rs` without changing streaming semantics |
| `src/engine/scenario_matrix.rs` | 1488 | Warning | Close to limit | Monitor; split only if new work pushes it over 1500 |

### Tasks

1. **Extract `session_parts` projection logic**
   - Move `project_session_parts()` and related types into
     `src/session_store/session_parts/projection.rs`.
   - Keep `session_parts.rs` as the public module re-export.
   - Preserve all existing tests; move integration tests to
     `src/session_store/session_parts/tests.rs`.

2. **Split `api/routes.rs` by domain**
   - `src/api/routes/mod.rs` — router setup and shared middleware.
   - `src/api/routes/session.rs` — session CRUD.
   - `src/api/routes/export.rs` — export/download endpoints.
   - `src/api/routes/tool_output.rs` — tool output pagination.

3. **Extract TUI export builder**
   - Move `build_session_export()` and helper types into
     `src/tui/session_manager/export_builder.rs`.
   - Keep `TuiSessionManager` struct and high-level methods in
     `session_manager.rs`.

4. **Extract streaming compaction events**
   - Move compaction event write helpers into
     `src/engine/streaming/compaction_events.rs`.
   - Keep provider request/response orchestration in `streaming.rs`.

5. **Add file-size regression gate to daily baseline**
   - Update `scripts/daily-baseline.sh` to fail if any non-test `.rs` file
     exceeds 1500 lines.
   - Document exceptions (e.g. generated files, `tests.rs` integration suites).

### Likely Files

- `src/session_store/session_parts.rs`
- `src/api/routes.rs`
- `src/tui/session_manager.rs`
- `src/engine/streaming.rs`
- `scripts/daily-baseline.sh`

### Acceptance

- All four over-limit files are <= 1500 lines.
- `src/engine/scenario_matrix.rs` remains <= 1500 lines or gets a focused
  split if it grows.
- `scripts/daily-baseline.sh` file-size-report gate passes.
- `cargo test -q` still passes (2453+ tests).
- No public API breakage for downstream consumers.

### Validation

```bash
cargo fmt --check
cargo check -q
cargo clippy --all-targets --all-features -- -D warnings
cargo test -q
bash scripts/daily-baseline.sh
```

## Phase 1 — Configuration Centralization

Status: **Completed**

Purpose: extend the existing typed config system so most
`PRIORITY_AGENT_*` reads go through one discoverable, validated registry.

### Current State

The project already has a config system in `src/services/config.rs`:

- `AppConfig::load()` merges defaults, config file, and
  `PRIORITY_AGENT_*` environment overrides.
- `CONFIG_KEY_SPECS` documents some mutable config keys.
- `validate_config()` already exists for several config domains.

The remaining problem is not "no config system"; it is incomplete adoption.
There are still many direct environment reads:

- About 288 production-ish `std::env::var` / `var_os` / `vars` reads in
  `src/` and `apps/` after excluding standalone test files.
- About 505 production-ish `std::env::*` references if `current_dir`,
  `temp_dir`, `set_var`, and platform environment inspection are included.
- Direct reads are valid for OS/platform facts (`HOME`, `USER`, `SHELL`,
  `current_dir`, test env guards), but `PRIORITY_AGENT_*` runtime knobs should
  be centralized unless explicitly allowlisted.

Examples:

- `PRIORITY_AGENT_TURN_TIMEOUT_SECS` parsed in `src/engine/streaming.rs:33`
- `PRIORITY_AGENT_AGENTS_MD_FULL` read in `src/instructions/mod.rs:108`
- `PRIORITY_AGENT_LEGACY_WORKFLOW_ENABLED` referenced in multiple modules
- Provider keys, memory paths, debug flags all read ad-hoc.

Problems:
- Typos in env var names are compile-time invisible.
- Default values are duplicated or inconsistent.
- No centralized validation (e.g. numeric ranges, path existence).
- New developers cannot discover available options.

### Tasks

1. **Audit existing env usage**
   - Generate a list of env vars read by the application, excluding tests and
     OS facts.
   - Categorize: config knob, provider secret, platform fact, test helper,
     debug-only escape hatch.
   - Identify duplicates, conflicting defaults, and stale names.

2. **Extend the existing `services::config` module**
   - Do not create a parallel `src/config/mod.rs`.
   - Add an `EnvKeySpec` registry for every supported `PRIORITY_AGENT_*` key:
     name, type, default, config path, secret flag, deprecated flag,
     owner/domain, and description.
   - Expand `CONFIG_KEY_SPECS` or link it to the env registry so `/config`,
     diagnostics, and docs all use the same source of truth.
   - Extend `validate_config()` to cover numeric ranges, path policy, mutually
     exclusive options, and provider-secret diagnostics.

3. **Add typed accessors for high-risk domains**
   - `AppConfig::turn_timeout()`
   - `AppConfig::tool_output_policy()`
   - `AppConfig::memory_paths()`
   - `AppConfig::permission_policy()`
   - `AppConfig::runtime_flags()`

4. **Migrate core paths first**
   - Start with `engine/streaming.rs`, `session_store/`, `permissions/`.
   - Pass `&AppConfig` into constructors instead of reading env inline.
   - Keep backward compatibility: existing env vars continue to override config
     file values, but reads happen through `AppConfig`.

5. **Add config tests**
   - Default values match current behavior.
   - Env overrides work.
   - Invalid values produce clear errors.
   - Deprecated aliases warn and still map correctly during the migration
     window.
   - Unknown `PRIORITY_AGENT_*` env vars are surfaced in diagnostics, not
     silently ignored.

6. **Add a no-new-raw-env guard**
   - Add a script or test that fails on new raw
     `std::env::var("PRIORITY_AGENT_...")` outside `src/services/config.rs`
     and an explicit allowlist.
   - Allow platform facts and test env guards.

### Likely Files

- Modified: `src/services/config.rs`
- Modified: `src/engine/streaming.rs`, `src/session_store/`, `src/permissions/`,
  `src/instructions/mod.rs`, `src/main.rs`, `src/bootstrap.rs`
- New or modified: config/env audit script or test

### Acceptance

- No new raw `std::env::var("PRIORITY_AGENT_...")` calls outside
  `src/services/config.rs` and the allowlist.
- Core runtime modules no longer parse `PRIORITY_AGENT_*` values inline.
- Direct production-ish `PRIORITY_AGENT_*` reads drop by >= 70% in the first
  migration pass; remaining reads are documented allowlist entries.
- `cargo test -q config` passes with >= 10 tests.
- `cargo check -q` passes.
- Existing env var behavior stays backward compatible.

### Validation

```bash
cargo fmt --check
cargo check -q
cargo test -q config
cargo test -q
cargo clippy --all-targets --all-features -- -D warnings
```

## Phase 2 — Core Path Error Handling Hardening

Status: **Completed**

Purpose: remove recoverable panics from paths that touch user data, storage,
permissions, and tool execution.

### Current State

`rg` currently finds roughly 2500 `unwrap()` / `expect(` / `panic!` occurrences
under `src/`, and about 1500 after excluding standalone `tests.rs` files.
Those totals are useful for tracking, but they are not the main risk metric:
many occurrences are inside inline `#[cfg(test)]` modules or assert-style test
helpers.

The real hardening target is runtime code that can affect user data or agent
continuity:

- `session_store/` — SQLite lock poisoning, event writer lock handling,
  query/projection errors.
- `permissions/` — policy file parse errors and classifier parsing.
- `engine/conversation_loop/` — channel send, mutex locks.
- `tools/file_tool/` — filesystem operations.

Risk: a transient SQLite lock or a malformed config file can crash the agent
mid-turn, losing user context.

### Tasks

1. **Inventory runtime panics, not test assertions**
   - List `unwrap`/`expect` in `session_store/`, `permissions/`,
     `engine/conversation_loop/`, `tools/file_tool/`.
   - Exclude `#[cfg(test)]` and standalone test files from the primary count.
   - Classify: truly invariant, recoverable runtime error, poison/coordination
     error, test-only assertion.

2. **Adopt `anyhow::Context` for operation context**
   - `db.query_row(...).context("load session parts")?` instead of `.unwrap()`.
   - `fs::read_to_string(path).with_context(|| format!("read {}", path))?`.

3. **Use `thiserror` only where callers branch**
   - `SessionStoreError`, `PermissionError`, `ToolExecutionError`.
   - Allow upstream code to match on `PermissionError::NotFound` vs
     `PermissionError::Denied`.
   - Prefer `anyhow` at orchestration boundaries where no caller branches on
     variants.

4. **Add graceful degradation paths**
   - If session part projection fails, return a degraded/partial result with a
     diagnostic. Do not silently return empty parts, because that hides data
     corruption.
   - If permission policy file is unreadable, deny by default rather than crash.
   - If tool output write fails, return tool error to model so it can retry.
   - If a lock is poisoned, convert to a contextual error where possible; only
     panic for impossible invariants inside tests.

5. **Add error-injection tests**
   - Simulate SQLite locked, disk full, malformed JSON.
   - Assert runtime returns `Err` rather than panicking.

### Likely Files

- `src/session_store/*.rs`
- `src/permissions/*.rs`
- `src/engine/conversation_loop/*.rs`
- `src/tools/file_tool/*.rs`
- `src/engine/trace.rs`

### Acceptance

- Runtime panic inventory exists and distinguishes tests from production paths.
- Recoverable panics in storage, permissions, event writing, and file tools are
  converted to contextual `Result` paths.
- No session data, permission decision, or file mutation path can panic on
  malformed input, missing files, SQLite busy/locked, or invalid JSON.
- Targeted runtime `unwrap`/`expect` count in the audited modules drops by
  >= 50%, excluding tests and documented invariants.
- `cargo test -q error_handling` passes with >= 8 tests covering failure modes.
- `cargo test -q` still passes (2453+ tests).
- No behavioral change for happy path.

### Validation

```bash
cargo fmt --check
cargo check -q
cargo test -q error_handling
cargo test -q
cargo clippy --all-targets --all-features -- -D warnings
```

## Phase 3 — End-to-End Deterministic Tests

Status: **Completed**

Purpose: cover complete user flows from prompt to tool execution to closeout.

### Current State

2453 tests pass, but most are unit tests. There is no deterministic test that
verifies: "user asks to create a Python file → agent writes file → validation
runs → closeout says done." Live evals catch these, but they are slow and
non-deterministic (depend on provider, model, timing).

### Tasks

1. **Design E2E test harness**
   - `tests/e2e/` directory.
   - Mock LLM provider that returns scripted responses.
   - In-memory or temp-dir SQLite session store.
   - No network calls, no real provider API keys.

2. **Implement mock provider**
   - `tests/e2e/mock_provider.rs` — implements `LlmProvider` trait.
   - Reads response scripts from JSON fixtures.
   - Supports multi-turn (each turn reads next response from script).

3. **Write scenario tests**
   - `test_file_read_flow`: user asks "read src/main.rs" → mock returns
     `file_read` tool call → harness asserts file content appears in trace.
   - `test_file_create_and_validate`: user asks "create hello.py" → mock
     returns `file_write` → harness asserts file exists and `py_compile` was
     invoked in closeout.
   - `test_edit_and_verify`: user asks "change foo to bar" → mock returns
     `file_edit` → harness asserts diff is correct and closeout is concise.
   - `test_tool_failure_recovery`: mock returns invalid tool call → harness
     asserts error observation is recorded and model gets a retry.

4. **Add to CI baseline**
   - `cargo test -q e2e` runs in CI.
   - Target runtime < 30 seconds for full suite.

### Likely Files

- New: `tests/e2e/mod.rs`, `tests/e2e/mock_provider.rs`, `tests/e2e/scenarios.rs`
- New fixtures: `tests/e2e/fixtures/read_scenario.json`, etc.

### Acceptance

- `cargo test -q e2e` passes with >= 6 scenario tests.
- Each scenario covers prompt → tool call → observation → closeout.
- No real provider calls; fully deterministic.
- Runs in < 30 seconds.

### Validation

```bash
cargo fmt --check
cargo check -q
cargo test -q e2e
cargo test -q
cargo clippy --all-targets --all-features -- -D warnings
```

## Phase 4 — Documentation Cleanup

Status: **Completed**

Purpose: reduce doc inflation; keep current plans visible and archive completed
ones.

### Current State

- 85 files in `docs/`
- 60 files in `docs/archive/`
- Many files have similar names: `TUI_OPTIMIZATION_PLAN_2026-06-09.md`,
  `TUI_DEEP_OPTIMIZATION_PLAN_2026-06-09.md`, `NEXT_PHASE_PRODUCT_DEVELOPMENT_PLAN_2026-06-02.md`,
  `NEXT_PHASE_PRODUCT_ECOSYSTEM_GAP_PLAN_2026-06-09.md`.
- Some files lack a clear status header (completed vs active vs draft).

### Tasks

1. **Audit all `docs/*.md` files**
   - For each file: last updated date, status, relationship to other files.
   - Identify duplicates, subsets, and outdated content.

2. **Archive completed plans older than 30 days**
   - Move to `docs/archive/` if status is "completed" and last updated before
     2026-05-10.
   - Update `docs/README.md` index to point to archive for historical plans.

3. **Merge duplicate/overlapping plans**
   - Example: consolidate TUI optimization plans into one active document.
   - Keep the most recent as canonical; archive others with a redirect note.

4. **Standardize headers**
   - Every current doc must have:
     ```markdown
     # Title
     Date: YYYY-MM-DD
     Status: Draft | Active | Completed | Archived
     Last Updated: YYYY-MM-DD
     ```

5. **Add doc-health test**
   - `cargo test -q docs` or a shell script that checks:
     - No plan doc older than 60 days without a status.
     - `docs/*.md` count < 50.
     - Every `.md` has required header fields.

### Likely Files

- `docs/README.md`
- Many `docs/*.md` moves/updates
- New: `scripts/doc_health_check.sh`

### Acceptance

- `docs/*.md` count reduced from 85 to < 50.
- All remaining docs have standard status headers.
- No active plan is older than 60 days without an update.
- `docs/README.md` accurately reflects current documentation tree.

### Validation

```bash
bash scripts/doc_health_check.sh
ls docs/*.md | wc -l  # should print < 50
```

## Migration Switches

- `PRIORITY_AGENT_LEGACY_CONFIG=1` — temporary compatibility switch during
  Phase 1 only. It may keep existing env override behavior, but it must not
  create a second long-term config path.
- Do not add a "strict unwrap" switch. Recoverable runtime errors should stay
  recoverable once hardened.
- Do not add a broad CI skip for E2E tests. If an E2E scenario is flaky,
  quarantine that named scenario with a tracked issue instead of disabling the
  suite.

## Success Criteria

The engineering hardening is successful when:

1. All source files (except tests and generated code) are <= 1500 lines.
2. `PRIORITY_AGENT_*` environment reads are centralized through the existing
   `services::config::AppConfig` registry and typed accessors.
3. Core paths fail gracefully with context-rich errors; no user-data loss on
   transient failures.
4. End-to-end tests verify complete flows deterministically without provider
   calls.
5. Documentation is current: completed plans are archived, active plans have
   clear status, and the doc count is manageable.
6. All existing tests continue to pass (2453+ baseline).
7. `cargo clippy --all-targets --all-features -- -D warnings` stays clean.

## Execution Order

```
Phase 0 → Phase 1 → Phase 2 → Phase 3 → Phase 4
```

Rationale:
- Phase 0 (file splitting) makes later phases easier (smaller files to modify).
- Phase 1 (config) reduces magic strings before Phase 2 touches error paths.
- Phase 2 (error handling) should be done before Phase 3 adds new test paths.
- Phase 4 (docs) is lowest risk and can run in parallel once earlier phases
  stabilize.

Target timeline: 2–3 weeks for full plan, assuming focused daily work.
