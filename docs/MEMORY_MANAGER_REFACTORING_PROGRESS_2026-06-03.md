# Memory Manager Refactoring Progress

Date: 2026-06-03

## Summary

`memory/manager.rs` (2011 lines) has been split into a `memory/manager/` directory
with 7 submodules. The main compilation passes, but test compilation needs fixes.

## Current Status

### Compilation: ✅ Pass (with warnings)

```
warning: unused import: `MemoryProvenance`
warning: function `cleanup_session_denial_counters` is never used
```

### Test Compilation: ❌ Needs Fix

There is one remaining test compilation error:

```
error[E0308]: mismatched types
  --> src/memory/manager/mod.rs:96:17
   |
96 |         status: outcome.status,
   |                 ^^^^^^^^^^^^^^ expected `MemoryStatus`, found `MemoryWriteOutcomeStatus`
```

This is in the `#[cfg(test)]` helper function `write_background_memory_candidate`.

## Completed Work

### Directory Structure

```
src/memory/manager/
├── mod.rs          # Core MemoryManager struct, constructor, basic methods (~900 lines)
├── helpers.rs      # Pure helper functions (~330 lines)
├── learning.rs     # add_learning, add_topic_learning, add_auto_learning (~110 lines)
├── snapshot.rs     # freeze_snapshot, get_snapshot, memory_snapshot_report (~130 lines)
├── migration.rs    # memory_migration_dry_run, backup, rollback, import_legacy (~200 lines)
├── review.rs       # memory_record_summary, memory_review_report, projection_repair (~260 lines)
└── submit.rs       # candidate_from_content, submit_candidate, lifecycle (~380 lines)
```

### File Responsibilities

#### `mod.rs` (~900 lines)
- `MemoryManager` struct definition
- `new()` and `with_base_dir()` constructors
- Basic accessor methods (records_path, search_index_path, active_scope, etc.)
- `memory_records()`, `memory_operation_journal()`
- `search_index_documents()`
- `memory_conflicts()`
- `save_workflow_decision()`, `save_workflow_decision_async()`
- Turn management (reset_turn, increment_turn, llm_extraction_interval)
- Stats (extraction_stats, cache_stats, memory_decision_counts, memory_flush_summary)
- Mode checks (is_forked_mode, is_trailing_mode, is_trailing_completed)
- `load_tier()`, `memory_summary()`
- Learning ingestion (push_learning, ingest_learnings, passes_quality_gate)
- Re-exports from submodules

#### `helpers.rs` (~330 lines)
- Constants: MAX_LEARNINGS_PER_TURN, MEMORY_DIR_NAME, etc.
- `memory_llm_timeout()` - timeout configuration
- `log_preview()` - content preview
- `kind_label()`, `status_label()` - type labels
- `normalized_contains()`, `normalize_for_duplicate()` - dedup helpers
- `default_candidate_evidence()`, `evidence_status()` - evidence helpers
- `has_required_evidence()`, `requires_verified_evidence()` - validation
- `infer_memory_importance()` - importance scoring
- `memory_scope_label()` - scope display
- `record_has_verified_evidence()`, `memory_lifecycle_key()` - lifecycle
- `is_safe_memory_backup_id()` - security
- `record_needs_revalidation()` - staleness check
- `memory_messages_hash()` - hashing
- `MemoryDecisionEvent` struct and `memory_decision_event()` constructor

#### `learning.rs` (~110 lines)
- `add_learning()` - sync version
- `add_topic_learning()` - sync version with topic
- `add_auto_learning()` - auto-select target
- `add_learning_async()` - async version
- `add_topic_learning_async()` - async version with topic
- `add_auto_learning_async()` - async auto-select

#### `snapshot.rs` (~130 lines)
- `freeze_snapshot()` - sync snapshot freeze
- `freeze_snapshot_async()` - async snapshot freeze
- `get_snapshot()` - retrieve frozen snapshot for system prompt
- `memory_snapshot_report()` - detailed snapshot report
- `memory_snapshot_skip_report()` - skip analysis

#### `migration.rs` (~200 lines)
- `memory_migration_dry_run()` - dry run migration
- `memory_migration_backup()` - backup memory files
- `memory_migration_rollback()` - rollback from backup
- `import_legacy_markdown_records()` - import from legacy format

#### `review.rs` (~260 lines)
- `memory_record_summary()` - summary of all records
- `memory_review_report()` - detailed review with categories
- `projection_repair_proposals()` - find projection drift
- `upsert_projection_repair_proposals()` - persist repair proposals
- `apply_projection_repair_proposal()` - apply a repair

#### `submit.rs` (~380 lines)
- `candidate_from_content()` - create candidate from text
- `submit_candidate()` - full submission pipeline with quality gates
- `submit_candidate_with_provider_notifications()` - async with provider hooks
- `apply_record_lifecycle_before_append()` - supersede old records
- `path_for_candidate()` - resolve write target
- `projection_path()`, `projection_contains_record()` - projection helpers

## Remaining Issues

### 1. Test Compilation Error

The `#[cfg(test)]` helper in `mod.rs` has a type mismatch:

```rust
#[cfg(test)]
fn write_background_memory_candidate(...) -> BackgroundMemoryWriteDecision {
    // ...
    BackgroundMemoryWriteDecision {
        status: outcome.status,  // Error: MemoryWriteOutcomeStatus vs MemoryStatus
        // ...
    }
}
```

**Fix:** Change the `BackgroundMemoryWriteDecision.status` field type or map the status value.

### 2. Old File Cleanup

`src/memory/manager_old.rs` (the original 2011-line file) still exists and should be removed
after all tests pass.

### 3. Test File Updates

`src/memory/manager/tests.rs` needs import updates to work with the new module structure.
Currently uses `use super::*;` which should work, but some types need explicit imports.

## Next Steps

### Immediate (This Session)

1. Fix the `BackgroundMemoryWriteDecision` type mismatch
2. Fix test imports in `tests.rs`
3. Remove `manager_old.rs`
4. Run full test suite: `cargo test -q memory -- --test-threads=1`

### Future Sessions

Continue with the refactoring plan from `CODEBASE_REFACTORING_PLAN_2026-06-03.md`:

| Priority | File | Lines | Target |
|----------|------|-------|--------|
| High | `engine/task_contract.rs` | 2897 | 7 files |
| Medium | `engine/trace.rs` | 2805 | 5 files |
| Medium | `session_store/mod.rs` | 1974 | 6 files |
| Medium | `memory/provider.rs` | 1968 | 6 files |
| Medium | `tool_execution_controller.rs` | 1995 | 5 files |
| Low | `agent_tool/mod.rs` | 1995 | 6 files |
| Low | `command_classifier.rs` | 1974 | 6 files |

## Lessons Learned

1. **Start with helpers** - Pure helper functions are safest to extract first
2. **Keep `pub(super)` consistent** - Methods called from sibling modules need `pub(super)` or `pub`
3. **Check field types** - Struct field types must match across module boundaries
4. **Test imports separately** - `use super::*;` may not pull in everything after splitting
5. **One module at a time** - Don't try to split multiple files simultaneously
