# Quality Gates

> Last updated: 2026-06-09
> Purpose: Define acceptance criteria for Phase transitions and releases

---

## Quality Gate Overview

| Gate | Description | Threshold |
|------|-------------|-----------|
| **G0** | Build passes | `cargo build` succeeds |
| **G1** | Tests pass | `cargo test` all green |
| **G2** | Clippy clean | No new high-severity warnings |
| **G3** | Documentation valid | No doc/implementation conflicts |
| **G4** | Capability matrix current | Exports reflect implementation |
| **G5** | Regression suite stable | Failure rate not increasing |

---

## Gate Definitions

### G0: Build Gate

**Command**: `cargo build --all-features`

**Criteria**:
- Compilation succeeds with exit code 0
- No `error[E0xxx]` compiler errors
- Warnings allowed but must not indicate broken links

**Failure**:
- Do not proceed to next phase
- Fix compilation errors before continuing

---

### G1: Test Gate

**Command**: `cargo test`

**Criteria**:
- All tests pass (exit code 0)
- Test count must not decrease (unless explicitly documented removal)
- No test timeout failures

**Failure**:
- Do not proceed to next phase
- Fix failing tests or document expected failures

---

### G2: Clippy Gate

**Command**: `cargo clippy --all-targets --all-features -- -D warnings`

**Criteria**:
- No new `error` level clippy warnings
- New `warning` level warnings require justification
- Existing warnings may be suppressed with documented reason

**Failure**:
- Warnings must be fixed or explicitly allowed
- Use `#[allow(clippy::warning_name)]` with documentation

---

### G3: Documentation Gate

**Command**: `cargo doc --no-deps`

**Criteria**:
- All public API documentation renders
- No broken links in documentation
- Rustdoc warnings addressed

**Failure**:
- Fix doc comments or remove stale documentation

---

### G4: Capability Matrix Gate

**Command**: `cargo run --bin capability_check` (or manual verification)

**Criteria**:
- All commands in `CAPABILITY_MATRIX.md` have corresponding handler
- All tools in matrix have corresponding implementation
- Maturity levels match implementation status

**Failure**:
- Update matrix to reflect reality
- Or implement missing components

---

### G5: Regression Gate

**Command**: `cargo test --test regression -- --test-threads=1`

**Criteria**:
- Baseline regression tests pass
- New features have accompanying regression tests
- Failure rate does not increase vs. previous baseline

**Failure**:
- Do not proceed if regression rate increases
- Document acceptable regression

---

## Phase Transition Gates

| Phase | Required Gates |
|-------|----------------|
| Phase 0 → Phase 1 | G0, G1, G3, G4 |
| Phase 1 → Phase 2 | G0, G1, G2, G3, G4 |
| Phase 2 → Phase 3 | G0, G1, G2, G5 |
| Phase 3 → Phase 4 | G0, G1, G2, G5 |
| Phase 4 → Phase 5 | G0, G1, G2 |
| Phase 5 → Phase 6 | G0, G1, G2 |
| Phase 6 → Phase 7 | G0, G1, G2, G5 |

---

## Release Gates

Before any release (alpha/beta/stable), the following must pass:

1. **G0**: Build succeeds
2. **G1**: All tests pass (100%)
3. **G2**: Clippy clean (warnings as errors)
4. **G5**: Regression suite at baseline or better
5. **Documentation**: CHANGELOG.md updated
6. **Version**: Cargo.toml version updated

---

## Continuous Integration Gates

All CI runs must pass:

| Check | Command | Critical |
|--------|---------|----------|
| Build | `cargo build --all-features` | Yes |
| Test | `cargo test` | Yes |
| Clippy | `cargo clippy --all-targets --all-features -- -D warnings` | Yes |
| Format | `cargo fmt --check` | No |
| Docs | `cargo doc --no-deps` | No |

---

## Acceptance Criteria Summary

For Phase 0 completion:

- [ ] `PLAN.md` in place
- [ ] `CAPABILITY_MATRIX.md` in place
- [ ] `QUALITY_GATES.md` in place
- [ ] Documentation validation script runnable in CI
- [ ] Command/tool maturity statistics exportable

---

## Rollback Criteria

If quality gates fail repeatedly:

1. **Immediate**: Stop merge of affected code
2. **Investigation**: Identify root cause within 24 hours
3. **Resolution**: Fix or revert within 72 hours
4. **Documentation**: Log failure in issue tracker

---

## Appendix: Quality Metrics

| Metric | Target | Current |
|--------|--------|---------|
| Test coverage | >80% | varies |
| Compilation time | <60s | ~20s |
| Test suite time | <120s | varies by selected gate |
| Clippy warnings | 0 | clean as of 2026-06-09 with all targets/all features |
| Doc warnings | 0 | varies |
