All required tests pass and the requested behavior is already present.

## Analysis

The code in `src/engine/retrieval_context.rs` already implements all required behaviors:

1. **Generic words filtered**: `is_generic_conflict_token()` (lines 483-505) blocks tokens: `memory`, `project`, `value`, `key`, etc. from causing false conflicts.

2. **Structured conflict matching**: `memory_conflict_matches_item()` (lines 458-481) parses `key 'X' has conflicting values: Y | Z` format and requires both the key AND at least one value to appear in the snippet.

3. **Conflict demotion**: `score_recall()` in `recall.rs` caps conflict memories at 0.49 (below inject threshold of 0.70) and marks them `ConflictCapped`.

4. **Demotion reason in recall**: `memory_retrieval_reason()` (lines 435-456) returns "matched query but overlaps with a conflicting memory; confidence reduced".

5. **Tests exist**: Tests cover generic word false positives (`memory_conflict_matching_ignores_generic_key_conflicts`), structured key/value matching (`memory_conflict_matching_uses_structured_key_and_value`), and fallback token overlap (`memory_conflict_matching_requires_specific_fallback_overlap`).

---

**Closeout:**

| Validation Command | Result |
|---|---|
| `cargo test -q retrieval_context -- --test-threads=1` | ✅ PASS (9 tests) |
| `cargo test -q memory::recall::tests:: -- --test-threads=1` | ✅ PASS (1 test) |
| `cargo test -q -- --test-threads=1` | ✅ PASS (1195 tests) |

**Files changed**: None

**Remaining risk**: None — all acceptance criteria verified. The requested behavior is already implemented and tests pass.