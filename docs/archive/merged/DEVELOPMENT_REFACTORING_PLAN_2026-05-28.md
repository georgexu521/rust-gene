# Development Refactoring Plan

This document details the next steps for improving code quality, maintainability, and compilation performance in the priority-agent codebase. Generated from a detailed investigation of the codebase on 2026-05-28.

---

## Priority 1: Split Giant Files

### 1.1 `src/memory/manager.rs` — started at 6826 lines

The `impl MemoryManager` block alone is 2362 lines (L1287–L3649). The file contains 25+ type definitions, 50+ public methods, and free functions for ranking/scoring. Current structure makes navigation and code review painful.

Progress on 2026-05-29: the first low-coupling split moved report/outcome/flush/review data structures into `src/memory/reports.rs`. The second split moved standalone ranking/search helpers into `src/memory/ranking.rs`. `manager.rs` re-exports the moved report types for compatibility.

#### Proposed Module Split

| New Module | What moves there | Approx Lines | Dependencies |
|---|---|---|---|
| `memory/reports.rs` | Standalone report/outcome/flush/review data types and their impls: `MemoryTier`, `MemoryEntry`, `MemorySummary`, `MemorySnapshotReport`, `MemoryMigrationReport`, `MemoryWriteOutcome`, `MemoryFlushReason`, `MemoryFlushRecord`, `MemoryFlushSummary`, `MemoryRecordSummary`, `MemoryReviewItem`, `MemoryReviewReport`, `MemoryMaintenanceReport`; old `memory::manager::*` paths remain re-exported | 472 | Low; depends on provider migration report and typed records |
| `memory/ranking.rs` | Standalone ranking/search helpers: `rank_memory_records`, `rank_project_progress_records`, `rank_memory_paragraphs`, `rank_memory_files`, `record_source`, `extract_keywords`, `search_memory`, semantic scoring helpers | ~340 | Low; depends on typed records and two manager helper labels |
| `memory/extraction.rs` | `sync_turn`, `sync_turn_llm`, `sync_turn_llm_background`, `trailing_run`, `extract_memory_candidates_with_llm`, `parse_llm_memory_candidates`, `extract_learnings_from_turn`, `extract_session_learnings`, `rerank_memory_matches_with_llm` | ~500 | `MemoryManager` methods: `submit_candidate`, `add_auto_learning`, `provider_registry` |
| `memory/retrieval.rs` | `prefetch`, `prefetch_with_llm_rerank`, `preview_relevant_memories`, `preview_retrieval_context`, `search`, `search_tier`, `search_memory_index`, `rebuild_search_index` | ~300 | `MemoryManager` fields: `frozen_memory`, `frozen_memory_files`, `memory_records()`, `search_memory_index()` |
| `memory/persistence.rs` | `flush_session*`, `trailing_run`, `flush_session_with_reason*`, `maintain_memory`, `maintain_memory_records`, `append_flush_record`, `record_memory_decision_event`, `write_memory_file_atomically`, `MemoryFileLock` | ~400 (L2928–L3222, L3427–L3530, L3573–L3648) | `MemoryManager` fields: `ingest_learnings`, `pending_learnings`, `seen_hashes`, `session_flush_can_persist` |
| `memory/provider.rs` | Provider lifecycle: `register_external_memory_provider`, `configure_external_memory_provider_from_config`, `initialize_memory_providers`, `provider_system_prompt_blocks`, `provider_prefetch`, `provider_search`, `queue_memory_provider_prefetch`, `sync_memory_providers_turn`, `notify_memory_providers_*`, `shutdown_memory_providers` | ~140 (L1375–L1511) | `MemoryManager` fields: `provider_registry` |
| `memory/manager.rs` (kept) | `MemoryManager` struct definition, `Default` impl, remaining methods (state/telemetry, candidate submission, snapshot/freeze, record access, migration/repair) | ~3500 (remaining) | All sub-modules |

#### Implementation Strategy

The key challenge is that `MemoryManager` has 20+ fields and most methods read/write multiple fields. A clean split requires:

1. **Extract types first** (L65–L1234) — zero coupling, pure data, immediate win
2. **Extract free functions** (ranking/scoring) — they're already standalone in the file, no `self` parameter
3. **Extract provider lifecycle** — relatively self-contained, only touches `provider_registry`
4. **Extract extraction pipeline** — calls into `submit_candidate` and `add_auto_learning`, but those remain on `MemoryManager`
5. **Extract retrieval/persistence** — most coupled, do last

The sub-modules receive `&MemoryManager` or `&mut MemoryManager` as a parameter and access fields via public getters, OR we define trait-based extension methods. The simplest approach: keep `MemoryManager` struct and its core fields in `manager.rs`, and implement sub-module methods via `impl MemoryManager` blocks spread across the new files (Rust allows multiple `impl` blocks for the same type across different files within the same crate).

#### Migration Order

```
Step 1: memory/reports.rs        (done; report/outcome/flush/review data)
Step 2: memory/ranking.rs         (done; free functions, no self)
Step 3: memory/provider.rs        (isolated provider registry touch)
Step 4: memory/extraction.rs      (needs submit_candidate access)
Step 5: memory/persistence.rs     (needs pending_learnings access)
Step 6: memory/retrieval.rs       (needs frozen_memory access)
```

#### Test Migration

Tests in `memory/manager.rs` are `#[cfg(test)] mod tests` at the bottom. Move relevant tests to each new module's `#[cfg(test)]` section. The test at L67 (`BackgroundMemoryWriteDecision`) is already module-local.

---

### 1.2 `src/tools/file_tool/mod.rs` — 4978 lines

Contains 3 tool implementations (`FileReadTool`, `FileWriteTool`, `FileEditTool`), a `FileStateTracker`, path utilities, guardrails, diff computation, and 2056 lines of tests.

#### Proposed Module Split

| New Module | What moves there | Approx Lines |
|---|---|---|
| `file_tool/state.rs` | `FileReadRecord`, `FileState`, `FilePathIdentity`, `ReadBeforeEditStatus`, `FileStateTracker`, all `mark_*`/`is_*`/`clear_*` static functions | ~220 (L290–L507) |
| `file_tool/path_utils.rs` | `resolve_path`, `resolve_read_path`, `expand_home_path`, `normalize_path`, `canonicalize_or_normalize`, `realpath_deepest_existing`, `is_allowed_absolute_path`, `is_allowed_read_absolute_path`, `read_allowed_roots` | ~450 (L303–L310, L817–L860, L2714–L2921) |
| `file_tool/guardrails.rs` | `high_risk_file_target_result`, `high_risk_file_target_diagnostic`, `priority_agent_settings_validation_error`, `validate_permissions_toml`, `checkpoint_creation_failed_result`, env-flag functions | ~500 (L43–L86, L129–L271, L1640–L1777) |
| `file_tool/edit_engine.rs` | `find_occurrences`, `fuzzy_find_occurrences`, `find_occurrences_normalized`, `build_match_context`, `exact_replace_preflight_error`, `InsertMode`, `do_replace`, `do_insert`, `do_replace_lines` | ~550 (L1492–L1631, L1779–L1981, L2525–L2712) |
| `file_tool/diff.rs` | `edit_diff_summary`, `edit_diff_summary_json`, `edit_preview_json`, `EditDiffSummary` | ~250 (L312–L320, L660–L815) |
| `file_tool/mod.rs` (kept) | `FileReadTool`, `FileWriteTool`, `FileEditTool` Tool trait impls, re-exports | ~900 (remaining non-test) |

Existing sub-modules (`diagnostics`, `history`, `patch`, `text_codec`) already demonstrate this pattern works. The three Tool impls stay in `mod.rs` as the orchestration layer, calling into the new sub-modules.

#### Test Migration

The 2056 lines of tests should be distributed:
- `state.rs` tests: file tracking, read-before-edit
- `edit_engine.rs` tests: replace, insert, fuzzy matching
- `path_utils.rs` tests: path resolution, normalization
- `guardrails.rs` tests: high-risk file detection
- `mod.rs` tests: end-to-end tool execute tests

#### Migration Order

```
Step 1: file_tool/types.rs         (shared types: FilePathIdentity, EditDiffSummary)
Step 2: file_tool/state.rs         (FileStateTracker + tests)
Step 3: file_tool/path_utils.rs    (path functions + tests)
Step 4: file_tool/guardrails.rs    (guardrails + tests)
Step 5: file_tool/edit_engine.rs   (edit matching + tests)
Step 6: file_tool/diff.rs          (diff computation + tests)
```

---

## Priority 2: Integration Tests

### Current State

- 349 `#[cfg(test)]` modules, all unit tests
- `tests/` has started with shared fixtures and `streaming_query.rs`
- End-to-end coverage is still thin; the first priority is to make integration tests prove successful tool execution, not just stream completion
- Test-lane baseline: `docs/TEST_LANES_2026-05-29.md`
- Daily fast lane: `bash scripts/test-fast-lane.sh`
- Slow-lane profiler: `bash scripts/profile-test-lanes.sh`

### Proposed Integration Test Structure

```
tests/
├── streaming_query.rs          # streaming query → LLM response → message
├── tool_pipeline.rs            # tool registration → parse → execute → result
├── agent_spawn.rs              # agent manager → spawn → result collection
├── memory_roundtrip.rs         # write → flush → reload → search
├── context_compression.rs      # long conversation → compress → continue
├── mcp_connection.rs           # MCP server connect → call tool → disconnect
├── hook_execution.rs           # pre-hook → tool → post-hook
├── permission_flow.rs          # permission check → allow/deny/ask
├── session_persistence.rs      # create → save → load → continue
└── common/
    └── mod.rs                  # Test fixtures: mock LLM provider, test workspace
```

### Key Test Scenarios

**1. `streaming_query.rs` — Core Query Pipeline**
```rust
// Test: user message → LLM generates response with tool call → tool executes → response completes
#[tokio::test]
async fn streaming_query_with_tool_call() {
    // 1. Create mock LLM provider (returns tool_call in response)
    // 2. Create StreamingQueryEngine with mock provider + real ToolRegistry
    // 3. Register a mock tool (echo_tool)
    // 4. Call query_stream("use the echo tool with hello")
    // 5. Assert: StreamEvent::ToolStart received
    // 6. Assert: StreamEvent::ToolResult with "hello"
    // 7. Assert: StreamEvent::Message with final response
    // 8. Assert: message count in conversation is correct
}
```

**2. `tool_pipeline.rs` — Tool Execution Chain**
```rust
// Test: multiple tool calls in one response (parallel read + serial write)
#[tokio::test]
async fn parallel_read_serial_write() {
    // 1. Create tool registry with file_read + file_write
    // 2. Mock LLM returns [file_read(A), file_read(B), file_write(C)] in one response
    // 3. Execute via ConversationLoop
    // 4. Assert: file_read(A) and file_read(B) execute in parallel
    // 5. Assert: file_write(C) executes after both reads complete
    // 6. Assert: results are ordered correctly
}
```

**3. `agent_spawn.rs` — Sub-Agent Lifecycle**
```rust
#[tokio::test]
async fn agent_spawn_and_collect() {
    // 1. Create AgentManager
    // 2. Spawn a sub-agent with a simple task
    // 3. Assert: agent status transitions: Pending → Running → Completed
    // 4. Assert: AgentResult contains expected output
    // 5. Assert: main agent can query sub-agent status
}
```

**4. `memory_roundtrip.rs` — Memory Write/Read**
```rust
#[tokio::test]
async fn memory_write_flush_search() {
    // 1. Create MemoryManager with temp directory
    // 2. add_learning("Rust lifetimes are annotated with 'a")
    // 3. flush_session()
    // 4. search("lifetimes")
    // 5. Assert: search result contains the learning
    // 6. Assert: file exists on disk at expected path
}
```

### Mock Infrastructure

The biggest blocker for integration tests is the LLM provider. We need:

```rust
// tests/common/mod.rs
pub struct MockProvider {
    responses: VecDeque<ChatResponse>,  // pre-configured responses
    call_count: AtomicU32,
}

impl LlmProvider for MockProvider {
    async fn chat_stream(&self, request: ChatRequest) -> Result<...> {
        let resp = self.responses.lock().pop_front().unwrap();
        // Return pre-configured response
    }
}
```

Also needed:
- `temp_workspace()` — creates a temp directory with `Cargo.toml` (for project detection)
- `tool_registry_with_mocks()` — pre-populated with mock tools for testing
- `create_test_conversation_loop()` — wires mock provider + real tools + temp workspace

### Estimated Effort

- Mock provider + temp workspace: ~200 lines
- streaming_query tests: ~150 lines
- tool_pipeline tests: ~200 lines
- agent_spawn tests: ~100 lines
- memory_roundtrip tests: ~150 lines
- Other tests: ~300 lines
- **Total: ~1100 lines of new test code**

---

## Priority 3: Feature Gate Refinement

### Current State

```toml
[features]
default = []
experimental-api-server = []
experimental-priority = []
experimental-task-analyzer = []
experimental-platform = []
voice = []
```

All marker-only, no associated crate dependencies. Four are used in `lib.rs` to gate modules.

### Proposed Feature Gates

#### Tier 1 — Zero-Risk Gates (no internal consumers)

These modules have no downstream dependents within the codebase. Gate them with zero breakage.

| Feature | Modules Gated | Files | Why |
|---|---|---|---|
| `desktop` | `desktop_runtime` | 1 | No consumers |
| `ide` | `ide` | 2 | No consumers |
| `github-actions` | `github` | 1 | No consumers |
| `telemetry` | `telemetry` | 1 | No consumers |
| `cost-tracking` | `cost_tracker` | 1 | No consumers |
| `diagnostics` | `diagnostics` | 2 | No consumers |
| `bridge` | `bridge` | 1 | No consumers; pulls reqwest |

**lib.rs changes:**
```rust
#[cfg(feature = "desktop")]
pub mod desktop_runtime;

#[cfg(feature = "ide")]
pub mod ide;

#[cfg(feature = "github-actions")]
pub mod github;

// etc.
```

**Cargo.toml changes:**
```toml
[features]
desktop = []
ide = []
github-actions = []
telemetry = []
cost-tracking = []
diagnostics = []
bridge = []

default = ["tui", "telemetry", "cost-tracking"]
```

#### Tier 2 — Gates with Modest Refactoring

| Feature | Modules Gated | Files | Consumers to `#[cfg]` |
|---|---|---|---|
| `tui` | `tui` | 31 | `shell.rs` (wrap `tui::run_tui` call) |
| `plugins` | `plugins` | 2 | `diagnostics/mod.rs`, `tools/plugin_tool/mod.rs` |
| `team` | `team` | 1 | `tools/team_tool.rs` |
| `remote` | `remote` | 1 | `tools/remote_dev_tool.rs` |

For `tui` feature:
```rust
// In shell.rs:
#[cfg(feature = "tui")]
mod tui_runner {
    pub async fn run(...) {
        crate::tui::run_tui(...).await
    }
}

#[cfg(not(feature = "tui"))]
mod tui_runner {
    pub async fn run(...) -> anyhow::Result<()> {
        anyhow::bail!("TUI not enabled. Build with --features tui")
    }
}
```

#### Tier 3 — Do NOT Gate (too deeply coupled)

`services`, `memory`, `engine`, `agent`, `state`, `permissions`, `session_store`, `tools`, `bootstrap` — these form the core runtime. Gating any would cascade breaks across the entire codebase.

#### Tier 1+2 Combined: Cargo.toml Final Form

```toml
[features]
default = ["tui", "telemetry", "cost-tracking"]
tui = ["dep:ratatui", "dep:crossterm", "dep:syntect", "dep:rustyline", "dep:arboard"]
desktop = []
ide = []
github-actions = []
telemetry = []
cost-tracking = []
diagnostics = []
bridge = ["dep:reqwest"]
plugins = []
team = []
remote = []
experimental-api-server = []
experimental-priority = []
experimental-task-analyzer = []
experimental-platform = []
voice = []
full = ["tui", "desktop", "ide", "github-actions", "telemetry", "cost-tracking", "diagnostics", "bridge", "plugins", "team", "remote"]
```

#### Estimated Effort

- Tier 1 (7 features): ~70 lines of `#[cfg]` annotations + Cargo.toml changes
- Tier 2 (4 features): ~150 lines (mostly `#[cfg]` wrappers in consumers)
- Testing all feature combinations: `cargo check --no-default-features`, `cargo check --features full`
- **Total: ~220 lines**

---

## Priority 4: Bootstrap Deduplication

### Current Duplication

The same init sequence appears in 4 places in `main.rs`:

```
// Pattern repeated in Api (L405-423), Cli (L457-469), Tui (L491-502), EvalRun (L133-137)
let working_dir = std::env::current_dir()?;
let (provider, model) = bootstrap::init_provider()?;   // + error match
let tool_registry = bootstrap::init_tool_registry(&working_dir);
let components = bootstrap::init_components(provider, model, tool_registry, &working_dir).await?;
```

The `init_provider()` error-handling block (match Ok/Err with exit + hint) is copy-pasted identically in Api, Cli, and Tui.

### Proposed Refactoring

#### Step 1: Add centralized initialization helpers to `bootstrap.rs`

```rust
pub struct AppComponents {
    pub provider: Arc<dyn LlmProvider>,
    pub model: String,
    pub tool_registry: Arc<ToolRegistry>,
    pub streaming_engine: Arc<StreamingQueryEngine>,
    pub lsp_manager: Arc<LspManager>,
    pub worktree_manager: Arc<WorktreeManager>,
}

pub struct ApiComponents {
    pub provider: Arc<dyn LlmProvider>,
    pub model: String,
    pub tool_registry: Arc<ToolRegistry>,
    pub lsp_manager: Arc<LspManager>,
    pub worktree_manager: Arc<WorktreeManager>,
}

pub async fn init_app(working_dir: &Path) -> anyhow::Result<AppComponents> {
    let (provider, model) = init_provider()?;
    let tool_registry = init_tool_registry(working_dir);
    init_components(provider, model, tool_registry, working_dir).await
}

pub async fn init_api_components(working_dir: &Path) -> anyhow::Result<ApiComponents> {
    // Provider + tools + LSP/worktree only; do not create CLI sessions or memory snapshots.
}
```

#### Step 2: Simplify main.rs

**Before (Tui mode, 12 lines):**
```rust
StartupMode::Tui => {
    let working_dir = std::env::current_dir()?;
    let (provider, model) = match bootstrap::init_provider() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error: {}", e);
            eprintln!("Hint: ...");
            std::process::exit(1);
        }
    };
    let tool_registry = bootstrap::init_tool_registry(&working_dir);
    let components = match bootstrap::init_components(provider, model, tool_registry, &working_dir).await {
        Ok(c) => c,
        Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
    };
    tui::run_tui(components.streaming_engine, components.lsp_manager, components.worktree_manager).await?;
}
```

**After (4 lines):**
```rust
StartupMode::Tui => {
    let working_dir = std::env::current_dir()?;
    let components = bootstrap::init_app(&working_dir)
        .await
        .context("Failed to initialize components")?;
    tui::run_tui(components.streaming_engine, components.lsp_manager, components.worktree_manager).await?;
}
```

#### Step 3: Handle Api mode

Api mode is the outlier — it manually creates LspManager and WorktreeManager separately. Two options:

**Option A (recommended):** Make `init_components()` configurable
```rust
pub async fn init_components(
    provider, model, tool_registry, working_dir,
    skip_lsp: bool, skip_worktree: bool,
) -> anyhow::Result<Components> { ... }
```

**Option B:** Api mode calls a lightweight `init_api_components()` and destructures
```rust
StartupMode::Api => {
    let components = bootstrap::init_api_components(&working_dir).await?;
    api::start_server(components, cli_args.port).await?;
}
```

Option B is cleaner if it stays lightweight. API mode needs provider, tool registry, LSP, and worktree handles, but should not initialize CLI-only state such as memory snapshots, streaming engines, or unused "CLI Session" records.

#### Estimated Effort

- Add `CoreComponents` struct: ~20 lines
- Add `init_core_components()`: ~15 lines
- Refactor main.rs Api/Cli/Tui/EvalRun: remove ~40 lines of duplication, replace with ~16 lines of calls
- **Total: ~50 lines net change**

---

## Implementation Order & Dependencies

```
Phase A (Week 1): Feedback loop first
  1. bootstrap dedup (Priority 4)        — 50 lines, no test impact
  2. test-lane profiling + fast lane     — prove slow tests before splitting
  3. feature gate Tier 1 (Priority 3)    — 70 lines, compile-only check
  4. memory/types.rs extraction (P1.1)   — pure data, zero risk

Phase B (Week 2): Medium-risk extractions
  5. file_tool/state.rs + path_utils.rs  — self-contained, move tests
  6. memory/ranking.rs extraction         — free functions, no self
  7. feature gate Tier 2 (Priority 3)    — wrap consumers with #[cfg]

Phase C (Week 3): Integration test foundation
  8. Mock LLM provider + test fixtures   — tests/common/mod.rs
  9. streaming_query integration tests   — core pipeline
  10. tool_pipeline integration tests    — parallel/serial execution

Phase D (Week 4): High-effort splits
  11. memory/extraction.rs + provider.rs — needs MemoryManager access
  12. file_tool/edit_engine.rs + diff.rs — most complex extraction
  13. memory/persistence.rs + retrieval.rs — deepest coupling
```

## Verification Checklist

After each phase:
- [ ] `bash scripts/test-fast-lane.sh` — fast feedback gate passes
- [ ] `cargo test -q <touched-module>` — narrow module tests pass
- [ ] `cargo test -q` — all existing tests pass before merge
- [ ] `cargo clippy --all-features -- -D warnings` — no new warnings
- [ ] `cargo check --no-default-features` — compiles without default features
- [ ] `cargo check --features full` — compiles with all features
- [ ] No new public API breakage (all existing imports still work)

## Risk Assessment

| Task | Risk | Mitigation |
|---|---|---|
| memory/types.rs extraction | Low | Pure data, no logic changes |
| memory/extraction.rs | Medium | Methods access MemoryManager fields; ensure all field access paths work |
| file_tool split | Medium | Existing sub-modules prove pattern works; follow same approach |
| integration tests | Medium | Mock provider must accurately simulate LLM behavior |
| feature gating | Low | Tier 1 is zero-risk; Tier 2 needs consumer #[cfg] wrappers |
| bootstrap dedup | Low | Pure refactor, no behavior change |
