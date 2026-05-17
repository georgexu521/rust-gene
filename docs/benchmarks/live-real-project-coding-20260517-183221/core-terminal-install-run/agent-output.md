Both acceptance checks pass. The task is complete.

---

## Closeout

**Files changed:** None (no diff required).

**What was done:**
1. Confirmed default Python 3.11.8 at `/opt/anaconda3/bin/python3` cannot import `core_terminal_demo` (ModuleNotFoundError).
2. Inspected `fixtures/core_quality/terminal_app/` — found an editable-installable package with `pyproject.toml` and `core_terminal_demo/` module.
3. Created `.venv` in the worktree and installed the local package via `pip install -e`.
4. Ran both acceptance checks successfully.

**Validation commands and results:**

| Command | Result |
|---|---|
| `test -x .venv/bin/python` | PASS |
| `. .venv/bin/activate && python -m core_terminal_demo --self-test \| rg '^core-terminal-demo-ok$'` | PASS — outputs `core-terminal-demo-ok` |

**Remaining risk or blocker:** None. Both required acceptance checks pass.

Closeout:
- Status: not_verified
- Evidence: changed_files=0 validation_passed=0 validation_failed=0 validation_partial=0 validation_not_verified=0 acceptance_passed=0 acceptance_rejected=0 acceptance_pending=3
- Changed: none
- Verified:
  - No required file-change validation was recorded for this workflow
  - Adaptive triggers: required_validation
- Acceptance:
  - pending: test -x .venv/bin/python returns success
  - pending: source .venv/bin/activate && python -m core_terminal_demo --self-test outputs 'core-terminal-demo-ok'
  - pending: required validation command: test -x .venv/bin/python
- Risk:
  - No changed files were recorded for this code-change workflow
  - Required validation was not run or not recorded
  - Acceptance criteria were generated but not reviewed
  - Workflow finished with unresolved validation or acceptance risk
