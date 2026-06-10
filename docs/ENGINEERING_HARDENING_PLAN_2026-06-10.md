# Priority Agent Engineering Hardening Plan

Date: 2026-06-10
Status: **Active**
Previous Plan: `docs/LLM_RUNTIME_SIMPLIFICATION_PLAN_2026-05-08.md` (completed)

## Goal

Keep Priority Agent maintainable as it grows. The runtime simplification plan
fixed prompt bloat and workflow over-control. The opencode core alignment plan
fixed session-store correctness. This plan fixes the remaining engineering
debt that slows down future development.

Target: a codebase where:
- Files stay under the 1500-line limit the team already agreed on.
- Configuration is discoverable and typed, not scattered in `env::var` calls.
- Core paths (storage, permissions, tool execution) fail gracefully with
  context-rich errors instead of panicking.
- Tests cover complete user flows, not just unit-level behavior.
- Documentation is current; completed plans move to archive.

## Phase 0 — Code Size Stewardship

Status: **Not started**

Purpose: enforce the existing "source files under 1500 lines" rule.

### Current State

Five production files exceed 1500 lines:

| File | Lines | Nature | Split Strategy |
|---|---|---|---|
| `src/session_store/session_parts.rs` | 1743 | Projection logic + tests | Extract `projection.rs` submodule |
| `src/api/routes.rs` | 1726 | API route handlers | Group by domain: `routes/session.rs`, `routes/export.rs`, etc. |
| `src/tui/session_manager.rs` | 1580 | TUI session logic + export builder | Extract `export_builder.rs` |
| `src/engine/streaming.rs` | 1511 | Streaming engine + compaction events | Extract `compaction_events.rs` |
| `src/engine/scenario_matrix.rs` | 1488 | Close to limit | Monitor; split if grows further |

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

- All five listed files are <= 1500 lines.
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

Status: **Not started**

Purpose: replace 540 scattered `env::var` calls with a typed, validated `Config`
struct.

### Current State

`env::var` and `env::var_os` appear 540 times across the codebase. Examples:

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

1. **Audit existing `env::var` usage**
   - Generate a list of all env vars read by the application (not tests).
   - Categorize: required vs optional, numeric vs path vs boolean vs string.
   - Identify duplicates and conflicts.

2. **Design `AppConfig` struct**
   - Use `serde` + a config file format (TOML or JSON) for base configuration.
   - Allow env var overrides with consistent prefix `PRIORITY_AGENT_*`.
   - Group by domain: `provider`, `memory`, `session`, `tool`, `tui`, `debug`.
   - Include validation methods (e.g. `timeout >= 60`, `path exists`).

3. **Implement `src/config/mod.rs`**
   - Define `AppConfig` with typed fields.
   - Implement `load()` that merges file → env → defaults.
   - Add `validate()` returning `Result<(), ConfigError>`.

4. **Migrate core paths first**
   - Start with `engine/streaming.rs`, `session_store/`, `permissions/`.
   - Pass `&AppConfig` into constructors instead of reading env inline.
   - Keep backward compatibility: if config file is missing, fall back to
     current env-only behavior with a deprecation warning.

5. **Add config tests**
   - Default values match current behavior.
   - Env overrides work.
   - Invalid values produce clear errors.
   - Unknown env vars are warned, not silently ignored.

### Likely Files

- New: `src/config/mod.rs`, `src/config/tests.rs`
- Modified: `src/engine/streaming.rs`, `src/session_store/`, `src/permissions/`,
  `src/instructions/mod.rs`, `src/main.rs`

### Acceptance

- `grep -r 'env::var' src --include='*.rs' | grep -v 'tests.rs' | wc -l`
  drops from current ~300 production reads to < 50 (tests and bootstrap only).
- `cargo test -q config` passes with >= 10 tests.
- `cargo check -q` passes.
- Existing env var behavior unchanged unless config file is present.

### Validation

```bash
cargo fmt --check
cargo check -q
cargo test -q config
cargo test -q
cargo clippy --all-targets --all-features -- -D warnings
```

## Phase 2 — Core Path Error Handling Hardening

Status: **Not started**

Purpose: reduce `unwrap`/`expect` in paths that touch user data, storage, or
permissions.

### Current State

`grep` shows 2519 occurrences of `unwrap()`/`expect(`/`panic!` across all `.rs`
files. Many are in tests or truly invariant paths, but core modules have
remaining hard panics:

- `session_store/` — SQLite lock poisoning, query errors.
- `permissions/` — policy file parse errors.
- `engine/conversation_loop/` — channel send, mutex locks.
- `tools/file_tool/` — filesystem operations.

Risk: a transient SQLite lock or a malformed config file can crash the agent
mid-turn, losing user context.

### Tasks

1. **Inventory core-path panics**
   - List `unwrap`/`expect` in `session_store/`, `permissions/`,
     `engine/conversation_loop/`, `tools/file_tool/`.
   - Classify: truly invariant (keep), recoverable (convert to `Result`),
     test-only (ignore).

2. **Adopt `anyhow::Context` for operation context**
   - `db.query_row(...).context("load session parts")?` instead of `.unwrap()`.
   - `fs::read_to_string(path).with_context(|| format!("read {}", path))?`.

3. **Use `thiserror` for typed errors where callers branch**
   - `SessionStoreError`, `PermissionError`, `ToolExecutionError`.
   - Allow upstream code to match on `PermissionError::NotFound` vs
     `PermissionError::Denied`.

4. **Add graceful degradation paths**
   - If session part projection fails, log error and return empty parts rather
     than panic.
   - If permission policy file is unreadable, deny by default rather than crash.
   - If tool output write fails, return tool error to model so it can retry.

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

- `grep -r 'unwrap()\|expect(' src/session_store src/permissions src/engine/conversation_loop src/tools/file_tool --include='*.rs' | grep -v 'tests.rs' | wc -l`
  drops by >= 50%.
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

Status: **Not started**

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

Status: **Not started**

Purpose: reduce doc inflation; keep current plans visible and archive completed
ones.

### Current State

- 84 files in `docs/`
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

- `docs/*.md` count reduced from 84 to < 50.
- All remaining docs have standard status headers.
- No active plan is older than 60 days without an update.
- `docs/README.md` accurately reflects current documentation tree.

### Validation

```bash
bash scripts/doc_health_check.sh
ls docs/*.md | wc -l  # should print < 50
```

## Rollback Switches

- `PRIORITY_AGENT_LEGACY_CONFIG=1` — read env vars directly, skip config file
  (for Phase 1 migration period).
- `PRIORITY_AGENT_STRICT_UNWRAP=1` — panic on recoverable errors instead of
  returning `Err` (for Phase 2 debugging only).
- `PRIORITY_AGENT_SKIP_E2E=1` — skip end-to-end tests in CI if they become
  flaky.

## Success Criteria

The engineering hardening is successful when:

1. All source files (except tests and generated code) are <= 1500 lines.
2. Environment variable reads are centralized in a typed `Config` struct.
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
