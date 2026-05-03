# Next Product Maturity Plan

Date: 2026-05-03

This plan continues from `docs/PROJECT_STATUS.md` and
`docs/CLAUDE_CODE_GAP_MATRIX_2026-05-03.md`. The P0/P1 checklist from the gap
matrix is largely closed; this stage focuses on product maturity items that can
be implemented and verified locally.

## Execution Rules

- Keep each batch small enough to validate independently.
- Prefer rendered UI smoke tests for user-visible CLI behavior.
- Keep docs synchronized with the verified test baseline after each batch.
- Commit each completed batch separately.

## Checklist

### 1. Rendered Diff Review Coverage

Goal: `/diff` and permission diff review should have actual rendering
regression coverage, not only string-generation tests.

- [x] Add a `ratatui::backend::TestBackend` smoke test for
  `components::diff_viewer::render_diff_viewer`.
- [x] Assert title, file headers, hunk headers, additions, deletions, and footer
  controls render into the buffer.
- [x] Add an empty/no-diff render smoke test.
- [x] Run targeted diff viewer tests, `cargo check -q`, and full tests.

### 2. Eval Trend External Baseline Metadata

Goal: persisted eval trends should be ready to compare local runs against
Claude/Codex-style baselines when those reports are available.

- [ ] Extend persisted eval report bundles with optional baseline metadata.
- [ ] Add a loader/formatter path that reports local-vs-baseline deltas when a
  baseline entry is present.
- [ ] Keep existing JSON backward compatible.
- [ ] Add unit tests for old JSON and baseline-aware JSON.

### 3. Git Tool Semantics And Closeout Evidence

Goal: git-related tool output should be easier to trust in final closeout and
trace views.

- [ ] Audit `src/tools/git_tool/mod.rs` summaries and errors.
- [ ] Add tests for `status`, `diff`, `add`, `commit`, and failure summaries
  where they can run deterministically.
- [ ] Ensure unsafe or invalid git inputs return actionable recovery text.

### 4. Documentation And Verification

Goal: keep project status accurate and avoid stale claims.

- [ ] Update `docs/PROJECT_STATUS.md` with the latest completed batch and test
  baseline.
- [ ] Update `docs/CLAUDE_CODE_GAP_MATRIX_2026-05-03.md` if a gap item changes.
- [ ] Run `bash scripts/validate_docs.sh`.
- [ ] Commit the docs with the implementation batch.

## Stop Conditions

- Stop and report if a change requires product direction rather than local
  implementation, such as choosing official external Claude/Codex eval report
  formats.
- Stop and report if a validation failure is unrelated to the current batch and
  cannot be isolated quickly.
