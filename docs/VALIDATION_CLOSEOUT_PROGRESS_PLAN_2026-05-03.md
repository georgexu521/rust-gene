# Validation Closeout And Progress Plan

Date: 2026-05-03

This plan follows `docs/NEXT_PRODUCT_MATURITY_PLAN_2026-05-03.md`. The focus is
the remaining product-maturity item in `docs/PROJECT_STATUS.md`: validation
closeout semantics and long-running command progress.

## Execution Rules

- Keep changes small and independently validated.
- Prefer deterministic unit tests over live long-running commands.
- Update `docs/PROJECT_STATUS.md` and the gap matrix when the verified baseline
  changes.
- Commit each completed batch.

## Checklist

Maintenance update, 2026-05-05:

- The closeout/progress-focused standard gate still passes after lint cleanup.
- The full workflow-enabled suite is `1059 passed; 0 failed`.
- `cargo clippy --all-features -- -D warnings` is clean.

### 1. Closeout Evidence Quality

Goal: final closeout should distinguish required validation, opportunistic
checks, acceptance review state, and residual risk more clearly.

- [x] Audit `WorkflowCloseout::format_for_final_response` and closeout status
  construction.
- [x] Add validation summary text that includes passed/failed/not-verified
  counts and changed-file count.
- [x] Add tests for mixed validation records and missing acceptance review.
- [x] Run targeted closeout tests, `cargo check -q`, and full tests.

### 2. Long-Running Tool Progress Labels

Goal: TUI progress should be clearer than generic `Executing bash...` for
validation and long-running shell commands.

- [x] Add a shared progress-label helper for tool execution start events.
- [x] Use command classifier metadata for bash validation/long-running labels.
- [x] Add tests for cargo test/check/clippy and generic bash commands.
- [x] Ensure existing `ToolRunView` expanded progress rendering remains stable.

### 3. Documentation And Verification

Goal: keep status, gap matrix, and this plan synchronized with the code.

- [x] Update `docs/PROJECT_STATUS.md`.
- [x] Update `docs/CLAUDE_CODE_GAP_MATRIX_2026-05-03.md`.
- [x] Run `bash scripts/validate_docs.sh`.
- [x] Commit the completed batch.

## Stop Conditions

- Stop and report if closeout changes require a new user-facing final-answer
  contract beyond summarizing existing evidence.
- Stop and report if long-running progress requires async cancellation or PTY
  behavior changes outside the current event stream.
