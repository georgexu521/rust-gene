All three acceptance checks pass. The code already implements the requested behavior.

## Closeout

**Changed files:** none

**Requested behavior is already present:**

1. **Generic words filtering** (`src/engine/retrieval_context.rs:483-505`): `is_generic_conflict_token()` excludes `memory`, `project`, `value`, `key`, `user`, `conflicting`, `conflicts`, `source`, etc. from conflict matching.

2. **Structured conflict matching** (`src/engine/retrieval_context.rs:507-526`): `parse_memory_conflict()` parses key/value/source from structured format like `- key 'language' has conflicting values: chinese | english`.

3. **High keyword hit conflicts are capped/demoted** (`src/memory/recall.rs:55-58`): When `conflict=true`, score is `(base * 0.55).min(0.49)` — below the 0.70 inject threshold — and decision is `ConflictCapped`.

4. **Recall reason shows conflict/demotion cause** (`src/engine/retrieval_context.rs:435-456`): `memory_retrieval_reason()` returns conflict-specific messaging; `score_recall()` includes decision, match_quality, scope, recency, trust, usefulness, criticality, token_cost in its reason string.

**Tests covering the required behaviors:**

| Test | Verifies |
|------|----------|
| `memory_conflict_matching_ignores_generic_key_conflicts` | `project` key conflicts don't match unrelated snippets |
| `memory_conflict_matching_uses_structured_key_and_value` | Structured key/value matches only relevant items |
| `memory_conflict_matching_requires_specific_fallback_overlap` | Generic word tokens insufficient; needs concrete value overlap |
| `conflict_caps_recall_below_inject_range` | Conflicts cap score below inject threshold |

**Validation results:**

| Command | Status |
|---------|--------|
| `cargo test -q retrieval_context -- --test-threads=1` | ✅ 16 passed |
| `cargo test -q memory::recall::tests:: -- --test-threads=1` | ✅ 1 passed |
| `cargo test -q -- --test-threads=1` | ✅ 1440 passed |

**Remaining risk or blocker:** none. The code is already fixed and all acceptance criteria are satisfied.

Closeout:
- Status: passed
- Evidence: changed_files=0 validation_passed=1 validation_failed=0 validation_partial=0 validation_not_verified=0 acceptance_passed=1 acceptance_rejected=0 acceptance_pending=0
- Changed: none
- Verified:
  - required validation: passed (passed:3/3)
  - Adaptive triggers: required_validation
- Acceptance:
  - accepted=true confidence=High unresolved=0 (required validation passed; code diff optional for audit/regression task)
- Risk:
  - none recorded
