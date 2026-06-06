# Real Task Fixture Matrix

Each fixture is a self-contained project. Tasks should exercise the full agent
loop: read/search → edit/patch → test → closeout.

## Task Categories

| Fixture | Language | Type | Key Skill |
|----------|----------|------|-----------|
| `cli_tool/` | Rust | CLI tool | Add a subcommand, fix a flag parsing bug, add a config file reader |
| `db_pipeline/` | Python | Data pipeline | Fix import logic, add a transform step, fix DB schema mismatch |
| `auth_app/` | Python | Web backend | Add an endpoint, fix auth middleware, update session logic |

## Expected Tool Chain per Task Type

### Code Modification (fix / feature)
1. `file_read` or `grep` — understand current code
2. `file_edit` or `file_patch` — apply changes
3. `run_tests` — verify correctness
4. `closeout` — submit evidence

### Refactor (structural change)
1. `grep` + `glob` — scope impact
2. `file_read` — read current implementations
3. `file_edit` — apply changes across files
4. `run_tests` — verify no regression
5. `closeout` — report changed files and test results

### Bug Fix (diagnosis + repair)
1. `grep` + `file_read` — trace the bug
2. `bash` — reproduce the issue (test failure, log output)
3. `file_edit` — apply fix
4. `run_tests` — verify fix
5. `closeout` — confirm the bug is resolved

## Failure Owner Classification

For each live eval run, record the failure owner using one of these categories:

| Category | Scope | Examples |
|----------|-------|----------|
| `framework` | Agent framework defect | Tool not exposed, permission blocks valid action, closeout rejects valid proof, route recovery fails, context assembly error |
| `provider_model` | LLM provider/model weakness | Wrong edit, stale anchor, hallucinated path, fails to follow repair hints, misses required validation |
| `harness` | Test/eval infrastructure | Fixture broken, expected output wrong, timeout too tight, env var missing |
| `environment` | External conditions | Network error, filesystem permission, user intervention (non-interactive mode), disk full |

## Daily Baseline Recording

After each real-task run, append to `fixtures/baseline.jsonl`:

```json
{
  "fixture": "cli_tool_add_subcommand",
  "model": "kimi-k2.5",
  "status": "verified",
  "tool_chain": ["grep", "file_read", "file_edit", "run_tests", "closeout"],
  "tool_rounds": 4,
  "failure_owner": "none",
  "durations_ms": {"total": 45000, "api": 12000, "tools": 3000},
  "changed_files": 2,
  "repair_turns": 0,
  "timestamp": "2026-06-06T12:00:00Z"
}
```
