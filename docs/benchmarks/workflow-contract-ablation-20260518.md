# Workflow Contract Ablation - 2026-05-18

## Question

Does Priority Agent's workflow contract layer, including weighted planning,
guided debugging, and guided reasoning, create measurable coding-agent value, or
is it only prompt/process overhead?

## Method

- Base commit: `0cddc7a`.
- Provider: live provider run through `scripts/run_live_eval.sh`.
- Binary: one shared `cargo build --release -q` before both groups.
- Contract-on run id: `ablation-contract-on-20260518-143305`.
- Contract-off run id: `ablation-contract-off-20260518-143305`.
- Toggle:
  - on: `PRIORITY_AGENT_WORKFLOW_CONTRACT=1`
  - off: `PRIORITY_AGENT_WORKFLOW_CONTRACT=0`
- Cases:
  - `backend-todo-api-crud`
  - `frontend-book-notes-localstorage`
  - `code-change-verification-repair-loop`
  - `core-permission-rejection-recovery`

This is a focused ablation, not a statistically strong benchmark. It is useful
for detecting whether the feature has any real signal on hard repair paths.

## Outcome

| case | contract on | contract off | key signal |
|------|-------------|--------------|------------|
| `backend-todo-api-crud` | failed | passed | on introduced a `_TODO_PATH` / `_TODOS_PATH` typo and failed required commands |
| `frontend-book-notes-localstorage` | passed | passed | no material success-rate difference |
| `code-change-verification-repair-loop` | passed | failed | on removed the bad retry format pattern; off left it in place while claiming success |
| `core-permission-rejection-recovery` | passed | passed | no material success-rate difference |

Summary:

| metric | contract on | contract off |
|--------|-------------|--------------|
| pass rate | 3/4 | 3/4 |
| coding gauntlet repaired passes | 1 | 0 |
| repair signals | 8 | 1 |
| changed-plan tasks | 4 | 0 |
| guided debugging active in repair-loop | yes | no |
| weighted planning active in repair-loop | yes | no |
| repair-loop required commands | ok | failed |

## Interpretation

The result does not prove broad superiority. Overall pass rate was tied at 3/4,
and the contract-on run lost a normal backend CRUD task that the contract-off run
completed. That failure was a real model coding error, not a harness mistake:
`do_POST`, `do_PATCH`, and `do_DELETE` used `self._TODO_PATH` while the class
defined `_TODOS_PATH`.

The strongest positive signal is the repair-loop case. With the workflow
contract enabled, the agent handled failed validation, acceptance rejection, and
repair closeout correctly. It removed the targeted bad pattern and passed all
five required commands, including the full `1461 passed; 0 failed` suite. With
the contract disabled, the agent fixed the compile-level argument count problem
but preserved the behavior-bad pattern:

```rust
&format!("retry: {}", verification_command),
```

The off run then claimed no remaining blocker even though the required `! rg`
assertion failed. This is exactly the kind of failure weighted planning and
guided repair are supposed to reduce: confusing "tests pass" with "the actual
acceptance contract is satisfied."

## Judgment

Weighted planning and guided questioning/debugging are not a magic coding
advantage. They can add overhead and can still fail on ordinary implementation
quality. But they are not decorative either: this ablation produced one concrete
case where the contract layer converted a subtle repair/acceptance failure into
a passing run, while the disabled run made a plausible but wrong closeout.

The practical conclusion is to keep this layer, but keep it targeted:

- Keep it active for high-risk code changes, repair loops, validation failures,
  and ambiguous acceptance contracts.
- Avoid treating it as proof that every task improves.
- Continue measuring with paired runs, especially where baseline coding already
  passes without guidance.

## Evidence

- Contract-on summary:
  `docs/benchmarks/live-ablation-contract-on-20260518-143305/summary.md`
- Contract-off summary:
  `docs/benchmarks/live-ablation-contract-off-20260518-143305/summary.md`
- Contract-on repair-loop report:
  `docs/benchmarks/live-ablation-contract-on-20260518-143305/code-change-verification-repair-loop/report.md`
- Contract-off repair-loop report:
  `docs/benchmarks/live-ablation-contract-off-20260518-143305/code-change-verification-repair-loop/report.md`
- Contract-on backend report:
  `docs/benchmarks/live-ablation-contract-on-20260518-143305/backend-todo-api-crud/report.md`
- Contract-off backend report:
  `docs/benchmarks/live-ablation-contract-off-20260518-143305/backend-todo-api-crud/report.md`
