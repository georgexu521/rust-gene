# Codebase Refactoring Plan

Date: 2026-06-03

## Executive Summary

Priority Agent has grown to ~249k Rust lines across ~464 files. While the
architecture is generally sound, several files have grown beyond comfortable
review size and accumulate multiple responsibilities. This plan identifies the
worst offenders and proposes concrete splitting strategies.

## Files Requiring Refactoring

| File | Lines | Priority | Proposed Split |
|------|-------|----------|----------------|
| `engine/task_contract.rs` | 2897 | High | 7 files |
| `engine/trace.rs` | 2805 | Medium | 5 files |
| `memory/manager.rs` | 2011 | High | 7 files |
| `conversation_loop/tool_execution_controller.rs` | 1995 | Medium | 5 files |
| `tools/agent_tool/mod.rs` | 1995 | Low | 6 files |
| `tools/bash_tool/command_classifier.rs` | 1974 | Low | 6 files |
| `session_store/mod.rs` | 1974 | Medium | 6 files |
| `memory/provider.rs` | 1968 | Medium | 6 files |

## Refactoring Principles

1. **One concern per file** — each file should have a single clear responsibility
2. **Preserve public API** — re-export from `mod.rs` to maintain backward compatibility
3. **Move tests with code** — tests should live next to the code they test
4. **Minimize cross-module imports** — prefer passing data over reaching into internals
5. **Keep `mod.rs` thin** — `mod.rs` should only contain re-exports and module declarations

---

## Phase 1: High Priority (Next Sprint)

### 1.1 Split `memory/manager.rs` (2011 lines → 7 files)

**Current responsibilities:**
- Memory manager core (construction, configuration)
- Snapshot management (freeze, get, report)
- Candidate submission (quality gates, provider notification)
- Learning management (add_learning, topic_learning, auto_learning)
- Migration/backup (dry_run, backup, rollback, import_legacy)
- Review/reporting (review_report, record_summary, projection_repair)
- Lifecycle management (apply_record_lifecycle, record_needs_revalidation)
- Helper functions (log_preview, normalized_contains, kind_label)

**Proposed structure:**

```
src/memory/
├── manager.rs                  # MemoryManager struct, new(), with_base_dir(), core config
├── manager_snapshot.rs         # freeze_snapshot, get_snapshot, memory_snapshot_report
├── manager_submit.rs           # submit_candidate, submit_candidate_with_provider_notifications
├── manager_learning.rs         # add_learning, add_topic_learning, add_auto_learning_async
├── manager_migration.rs        # memory_migration_dry_run, backup, rollback, import_legacy
├── manager_review.rs           # memory_review_report, memory_record_summary, projection_repair
├── manager_lifecycle.rs        # apply_record_lifecycle_before_append, record_needs_revalidation
└── manager_helpers.rs          # log_preview, normalized_contains, kind_label, etc.
```

**Files to move:**

| Function/Type | From | To |
|---------------|------|-----|
| `freeze_snapshot*`, `get_snapshot`, `memory_snapshot_report` | manager.rs | manager_snapshot.rs |
| `submit_candidate*` | manager.rs | manager_submit.rs |
| `add_learning*`, `add_topic_learning*`, `add_auto_learning*` | manager.rs | manager_learning.rs |
| `memory_migration_*`, `import_legacy_markdown_records` | manager.rs | manager_migration.rs |
| `memory_review_report`, `memory_record_summary`, `projection_repair_*` | manager.rs | manager_review.rs |
| `apply_record_lifecycle_before_append` | manager.rs | manager_lifecycle.rs |
| All `pub(super)` helper functions | manager.rs | manager_helpers.rs |

**Acceptance criteria:**

```bash
cargo fmt --check
cargo check -q
cargo test -q memory
cargo test -q memory_manager
```

### 1.2 Split `engine/task_contract.rs` (2897 lines → 7 files)

**Current responsibilities:**
- Type definitions (TaskContractType, AssumptionSource, ConfidenceLevel, etc.)
- Task contract (TaskContract, ContextPack, ExecutionReport)
- Memory proposal (MemoryProposal, MemoryProposalCandidate, MemoryProposalStatus)
- Proposal store (MemoryProposalReviewStore — CRUD, conflict resolution, batch ops)
- Proposal gates (sensitivity, evidence, scope_identity validation)
- Conflict detection (memory_proposal_conflict_groups)
- Background review (BackgroundReviewPacket, BackgroundMemoryReviewWorker)

**Proposed structure:**

```
src/engine/
├── task_contract/
│   ├── mod.rs                  # re-exports
│   ├── types.rs                # TaskContractType, AssumptionSource, ConfidenceLevel, etc.
│   ├── contract.rs             # TaskContract, ContextPack, ExecutionReport
│   ├── memory_proposal.rs      # MemoryProposal, MemoryProposalCandidate, MemoryProposalStatus
│   ├── proposal_store.rs       # MemoryProposalReviewStore (CRUD)
│   ├── proposal_gates.rs       # Gate logic: sensitivity, evidence, scope_identity
│   ├── proposal_conflict.rs    # memory_proposal_conflict_groups, conflict resolution
│   └── background_review.rs    # BackgroundReviewPacket, BackgroundMemoryReviewWorker
```

**Files to move:**

| Function/Type | From | To |
|---------------|------|-----|
| 6 enums + `TaskAssumption`, `TaskContractScope`, etc. | task_contract.rs | types.rs |
| `TaskContract`, `ContextPack`, `ExecutionReport` + methods | task_contract.rs | contract.rs |
| `MemoryProposal*` types + `from_execution_report` | task_contract.rs | memory_proposal.rs |
| `MemoryProposalReviewStore` all methods | task_contract.rs | proposal_store.rs |
| `proposal_gate_report`, `proposal_sensitivity_findings` | task_contract.rs | proposal_gates.rs |
| `memory_proposal_conflict_groups` | task_contract.rs | proposal_conflict.rs |
| `BackgroundReviewPacket`, `BackgroundMemoryReviewWorker` | task_contract.rs | background_review.rs |

**Acceptance criteria:**

```bash
cargo fmt --check
cargo check -q
cargo test -q task_contract
cargo test -q memory_proposals
```

---

## Phase 2: Medium Priority

### 2.1 Split `engine/trace.rs` (2805 lines → 5 files)

**Current responsibilities:**
- TraceEvent definition (60+ variants)
- TraceEvent::label() and summary() implementations
- TurnTrace container
- TraceCollector (thread-safe event collector)
- TraceStore (in-memory recent traces)
- Formatting/diagnostics (format_trace_summary, control_loop_diagnostic)
- Helper queries (latest_runtime_diet_summary, latest_memory_proposal_summary)

**Proposed structure:**

```
src/engine/
├── trace/
│   ├── mod.rs              # re-exports
│   ├── types.rs            # TurnStatus, TurnTrace, TraceEvent definition
│   ├── event_summary.rs    # TraceEvent::label() and summary() implementation
│   ├── collector.rs        # TraceCollector, TraceStore
│   ├── diagnostic.rs       # ControlLoopDiagnostic, ActionReviewTraceSummary
│   └── format.rs           # format_trace_summary, format_trace_recent_line
```

**Acceptance criteria:**

```bash
cargo fmt --check
cargo check -q
cargo test -q trace
```

### 2.2 Split `conversation_loop/tool_execution_controller.rs` (1995 lines → 5 files)

**Current responsibilities:**
- Tool execution orchestration (execute_tools_parallel)
- Execution gate (ToolExecutionGate — permissions, scope, budget, destructive checks)
- Read-only tool parallelization
- Read-write tool serial execution
- Runtime context (ToolRuntimeContext — route, policy, stage info)
- Action decision (ActionDecision evaluation, Observer/Memory signals)
- Action review (ActionReview construction and recording)
- Lifecycle tracking (ToolCallLifecycle state management)
- Storm circuit breaker (repeated call suppression)

**Proposed structure:**

```
src/engine/conversation_loop/
├── tool_execution_controller.rs    # ToolExecutionController main logic
├── tool_execution_gate.rs          # ToolExecutionGate, ToolExecutionGateOutcome
├── tool_runtime_context.rs         # ToolRuntimeContext, ObserverActionSignal, MemoryActionSignal
├── tool_action_decision.rs         # apply_observer_action_signal, apply_memory_action_signal
└── tool_execution_batch.rs         # ToolExecutionBatch, ToolExecutionRequest
```

**Files to move:**

| Function/Type | From | To |
|---------------|------|-----|
| `ToolExecutionGate` + `evaluate` + `deny_with_trace` | tool_execution_controller.rs | tool_execution_gate.rs |
| `ToolRuntimeContext` + `ObserverActionSignal` + `MemoryActionSignal` | tool_execution_controller.rs | tool_runtime_context.rs |
| `apply_observer_action_signal`, `apply_memory_action_signal` | tool_execution_controller.rs | tool_action_decision.rs |
| `ToolExecutionBatch`, `ToolExecutionRequest` | tool_execution_controller.rs | tool_execution_batch.rs |

**Acceptance criteria:**

```bash
cargo fmt --check
cargo check -q
cargo test -q tool_execution
cargo test -q conversation_loop
```

### 2.3 Split `session_store/mod.rs` (1974 lines → 6 files)

**Current responsibilities:**
- Session CRUD (create, get, list, delete sessions)
- Message CRUD (add, get, delete, rewrite messages)
- Full-text search (FTS5 search messages and sessions)
- Statistics (stats() for database stats)
- Trace persistence (add_turn_trace, latest_turn_trace)
- Learning event persistence (add_learning_event, recent_learning_events)
- Compact boundary persistence (add_compact_boundary, list_compact_boundaries)
- Agent artifact persistence (add_agent_artifact, recent_agent_artifacts)
- Agent task state persistence (upsert_agent_task_state, recent_agent_task_states)
- Database migrations (open() registers and runs migrations)

**Proposed structure:**

```
src/session_store/
├── mod.rs              # SessionStore struct, session/message CRUD, search, stats
├── records.rs          # MessageRecord, SessionRecord, LearningEventRecord, etc.
├── trace_store.rs      # add_turn_trace, latest_turn_trace, recent_turn_traces
├── learning_store.rs   # add_learning_event, recent_learning_events
├── compact_store.rs    # add_compact_boundary, list_compact_boundaries
├── agent_store.rs      # add_agent_artifact, upsert_agent_task_state
└── helpers.rs          # session_from_row, learning_event_from_row, fts_phrase_terms
```

**Files to move:**

| Function/Type | From | To |
|---------------|------|-----|
| All Record/Upsert structs | mod.rs | records.rs |
| Trace-related methods | mod.rs | trace_store.rs |
| Learning Event methods | mod.rs | learning_store.rs |
| Compact Boundary methods | mod.rs | compact_store.rs |
| Agent Artifact/TaskState methods | mod.rs | agent_store.rs |
| Row mapping functions | mod.rs | helpers.rs |

**Acceptance criteria:**

```bash
cargo fmt --check
cargo check -q
cargo test -q session_store
```

### 2.4 Split `memory/provider.rs` (1968 lines → 6 files)

**Current responsibilities:**
- Provider trait (MemoryProvider — lifecycle hooks)
- Provider registry (MemoryProviderRegistry — local + external provider management)
- Local provider (LocalMemoryProvider — local filesystem implementation)
- No-network provider (NoNetworkMemoryProvider — for testing)
- Operation journal (MemoryOperationJournalEntry)
- Migration/backup (migration_file_reports, backup, rollback)
- Search index (rebuild_search_index, search_index)
- Projection repair (projection_contains_record, append_record_to_projection_with_backup)

**Proposed structure:**

```
src/memory/
├── provider/
│   ├── mod.rs                  # re-exports
│   ├── traits.rs               # MemoryProvider trait, MemoryProviderCapabilities
│   ├── registry.rs             # MemoryProviderRegistry, fanout logic
│   ├── local_provider.rs       # LocalMemoryProvider all implementations
│   ├── no_network_provider.rs  # NoNetworkMemoryProvider
│   ├── migration.rs            # migration_file_reports, backup, rollback
│   └── types.rs                # MemoryTurn, MemoryProviderCallStatus, MemoryOperationJournalEntry, etc.
```

**Files to move:**

| Function/Type | From | To |
|---------------|------|-----|
| `MemoryProvider` trait + `MemoryProviderCapabilities` | provider.rs | traits.rs |
| `MemoryProviderRegistry` all methods | provider.rs | registry.rs |
| `LocalMemoryProvider` all methods | provider.rs | local_provider.rs |
| `NoNetworkMemoryProvider` all methods | provider.rs | no_network_provider.rs |
| `migration_*` functions | provider.rs | migration.rs |
| `MemoryTurn`, `MemoryProviderCallStatus`, `MemoryOperationJournalEntry`, etc. | provider.rs | types.rs |

**Acceptance criteria:**

```bash
cargo fmt --check
cargo check -q
cargo test -q memory_provider
```

---

## Phase 3: Low Priority

### 3.1 Split `tools/agent_tool/mod.rs` (1995 lines → 6 files)

**Proposed structure:**

```
src/tools/agent_tool/
├── mod.rs              # AgentTool, Tool trait impl, main entry
├── templates.rs        # AgentTemplate and system prompt construction
├── spawn.rs            # spawn_single_agent, create_isolated_agent_worktree
├── synthesize.rs       # synthesize_results, attach_subagent_proof_metadata
├── persistence.rs      # persist_agent_task_state, persist_agent_artifact
└── handlers.rs         # handle_resume, handle_cancel, handle_fork_branches, handle_subtasks
```

**Acceptance criteria:**

```bash
cargo fmt --check
cargo check -q
cargo test -q agent_tool
```

### 3.2 Split `tools/bash_tool/command_classifier.rs` (1974 lines → 6 files)

**Proposed structure:**

```
src/tools/bash_tool/
├── command_classifier.rs       # classify_command main entry + CommandClassification
├── classification_types.rs     # CommandKind, ShellCommandCategory, ValidationFamily, etc.
├── validation_family.rs        # validation_family, is_safe_rg_assertion, is_safe_shell_assertion
├── shell_category.rs           # shell_command_category, is_git_mutation_command, etc.
├── bash_plan.rs                # bash_command_plan, BashCommandPlan
└── shell_tokens.rs             # shell_tokens, extract_path_patterns, shell_redirection_facts
```

**Acceptance criteria:**

```bash
cargo fmt --check
cargo check -q
cargo test -q command_classifier
```

---

## Implementation Order

### Sprint 1 (High Priority)
1. `memory/manager.rs` → 7 files
2. `engine/task_contract.rs` → 7 files

### Sprint 2 (Medium Priority)
3. `engine/trace.rs` → 5 files
4. `session_store/mod.rs` → 6 files
5. `memory/provider.rs` → 6 files

### Sprint 3 (Medium-Low Priority)
6. `conversation_loop/tool_execution_controller.rs` → 5 files
7. `tools/agent_tool/mod.rs` → 6 files
8. `tools/bash_tool/command_classifier.rs` → 6 files

---

## Migration Checklist

For each file split:

- [ ] Create new module directory (if needed)
- [ ] Move types/functions to new files
- [ ] Add `mod` declarations to parent `mod.rs`
- [ ] Add `pub use` re-exports to maintain public API
- [ ] Update imports in dependent files
- [ ] Run `cargo fmt --check`
- [ ] Run `cargo check -q`
- [ ] Run relevant tests
- [ ] Verify no new warnings
- [ ] Commit with descriptive message

---

## Risk Mitigation

### 1. Breaking Public API

**Risk:** Moving types/functions breaks downstream imports.

**Mitigation:** Always re-export from the original `mod.rs`:
```rust
// memory/mod.rs
pub use manager::MemoryManager;
pub use manager_snapshot::*;
pub use manager_submit::*;
// etc.
```

### 2. Merge Conflicts

**Risk:** Large refactoring causes merge conflicts with parallel work.

**Mitigation:**
- Coordinate with team before starting
- Do refactoring in dedicated branches
- Merge to main quickly after completion

### 3. Test Breakage

**Risk:** Moving code breaks test imports.

**Mitigation:**
- Move tests with the code they test
- Update test imports in the same commit
- Run full test suite after each file split

### 4. Circular Dependencies

**Risk:** Splitting creates circular module dependencies.

**Mitigation:**
- Analyze dependencies before splitting
- Use trait objects or dependency injection if needed
- Keep related code together to minimize cross-module calls

---

## Done Definition

This refactoring is complete when:

- No source file exceeds 1500 lines (excluding tests)
- Each module has a single clear responsibility
- All tests pass after each split
- No new compiler warnings introduced
- Public API remains backward compatible
- Documentation updated to reflect new structure
