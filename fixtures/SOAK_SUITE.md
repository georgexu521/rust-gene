# Product Soak Suite

Phase 5 product soak tasks for release confidence.  Each task exercises a
product surface: CLI streaming, TUI diagnostics, desktop reload, revert,
permissions, tool-output paging, provider timeouts, and API contracts.

Status: draft — task definitions ready, results pending real runs.

---

## Tasks

| # | Name | Product Surface | Expected |
|---|------|-----------------|----------|
| 1 | `soak-basic-backend-fix` | CLI: code-change + validation | passed, failure_owner=none |
| 2 | `soak-frontend-ui-tweak` | CLI: code-change + lint | passed, failure_owner=none |
| 3 | `soak-rust-multi-file-refactor` | CLI: multi-file file_edit + cargo check | passed |
| 4 | `soak-failing-test-repair` | CLI: diagnostics + repair | passed or partial with valid evidence |
| 5 | `soak-long-output-handle` | tool-output paging, /tool-output list | output stored, page API works |
| 6 | `soak-provider-slow-tail` | provider timeout diagnostics, /diagnostic | timeout classified, profile visible |
| 7 | `soak-desktop-reload` | desktop: reload after tool activity | session parts reloaded without loss |
| 8 | `soak-revert-unrevert` | /revert + /unrevert + checkpoint | revert succeeds, unrevert restores |
| 9 | `soak-permission-deny-recover` | permission block + explain + recovery | blocked, explained, recovered |
| 10 | `soak-session-events-cursor` | API: GET /api/sessions/:id/events | paged events returned via cursor |

---

## Run Commands

```bash
# Individual soak runs
cargo run -- --tui --eval live-eval --case soak-basic-backend-fix
cargo run -- --tui --eval live-eval --case soak-frontend-ui-tweak

# Full soak baseline
bash scripts/soak-suite.sh

# API route smoke (requires server running)
curl -s http://localhost:8080/api/provider/status | jq .
curl -s "http://localhost:8080/api/sessions/{id}/parts?limit=5" | jq .
curl -s "http://localhost:8080/api/diagnostics/latest?session_id={id}" | jq .
```

---

## Baseline Report Template

| Task | Status | Failure Owner | Model | Turns | Cost | Notes |
|------|--------|---------------|-------|-------|------|-------|
| 1 | | | | | | |
| 2 | | | | | | |
| ... | | | | | | |
| 10 | | | | | | |

---

## Release Blockers

The following conditions block a release candidate:

- Data loss (session parts, events, checkpoints not recoverable after crash)
- False verified closeout (verification passed but required commands actually failed)
- Unrecoverable desktop reload (parts missing after TUI quit + desktop open)
- Provider timeout with no visible diagnosis
- Permission hard gate bypass (allow rule incorrectly permits destructive operation)
- API schema drift (route output shape changes without version bump)
