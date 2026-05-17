Both acceptance checks pass.

---

## Closeout

**Task:** Install local Python package and run via terminal.

**Files changed:** None (read-only task; used terminal commands only).

**What was done:**
1. Default Python (`/opt/anaconda3/bin/python`, Python 3.11.8) was available but `core_terminal_demo` was not importable.
2. Created `.venv` in worktree.
3. Installed `fixtures/core_quality/terminal_app` in editable mode via `.venv/bin/pip install -e …`.
4. Ran `python -m core_terminal_demo --self-test` successfully.

**Validation commands:**

| Command | Result |
|---|---|
| `test -x .venv/bin/python` | PASS |
| `. .venv/bin/activate && python -m core_terminal_demo --self-test \| rg '^core-terminal-demo-ok$'` | PASS — output: `core-terminal-demo-ok` |

**Risk/blocker:** None. All required checks pass.

Closeout:
- Status: passed
- Evidence: changed_files=0 validation_passed=1 validation_failed=0 validation_partial=0 validation_not_verified=0 acceptance_passed=0 acceptance_rejected=0 acceptance_pending=0
- Changed: none
- Verified:
  - required validation: passed (passed:2/2)
  - Adaptive triggers: required_validation
- Acceptance:
  - No explicit acceptance criteria were recorded
- Risk:
  - none recorded
