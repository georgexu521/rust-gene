# Live Validation Round 3 - Strict Plan Gate

Run id: `validation-round3-strict-final`
Date: 2026-04-29
Mode: `api-plan`
Provider: MiniMax (`MINIMAX_MODEL`, default `MiniMax-M2.7`)

## Purpose

This round validates the three-layer regression strategy for coding-agent quality:

1. Re-run the existing live regressions to keep prior fixes from drifting.
2. Add near-neighbor variants for memory, recall, resume, skill, and CLI behavior.
3. Add broader product-level tasks that represent real coding-agent workflows.

The `api-plan` mode does not edit files. It validates prompt routing, task framing,
LLM planning behavior, tool-boundary discipline, hidden-reasoning sanitization, and
plan-only linting through the real API server path.

## Result

All 11 tasks generated MiniMax plans and passed the strict plan lint.

| Task | Layer | Risk | Result |
| --- | --- | --- | --- |
| `cli-scrollback-polish` | baseline | medium | pass |
| `memory-save-quality-gate` | baseline | high | pass |
| `persistent-memory-planning-context` | baseline | high | pass |
| `resume-session-picker` | baseline | medium | pass |
| `skill-promotion-gate` | baseline | medium | pass |
| `memory-save-sensitive-hard-block` | variant | high | pass |
| `memory-save-duplicate-demotion` | variant | medium | pass |
| `memory-recall-conflict-precision` | variant | high | pass |
| `code-change-verification-repair-loop` | broad workflow | high | pass |
| `permission-default-open-dangerous-guard` | broad workflow | high | pass |
| `live-eval-dashboard-summary` | broad workflow | medium | pass |

## What Changed In The Gate

The first strict run exposed a useful failure: one plan ended with an action-like
sentence (`let me run...`) even though `api-plan` cannot execute tools. The runner
now rejects action-text phrases such as:

- `let me start`
- `let me inspect`
- `let me run`
- `let me edit`
- `ready to proceed with implementation`

The runner also now propagates individual task failures correctly when running
`--case all`, instead of continuing and exiting successfully after a failed plan.

## Commands Run

```bash
scripts/run_live_eval.sh --case all --mode api-plan --run-id validation-round3-baseline
scripts/run_live_eval.sh --case all --mode api-plan --run-id validation-round3-expanded2 --skip-build
bash -n scripts/run_live_eval.sh
scripts/run_live_eval.sh --case persistent-memory-planning-context --mode api-plan --run-id validation-round3-strict-fix --skip-build
scripts/run_live_eval.sh --case all --mode api-plan --run-id validation-round3-strict-final --skip-build
```

## Remaining Gap

This is still a planning/API regression gate, not a full code-change completion
benchmark. The next step is to add a true non-interactive code-change runner that
can:

1. Execute the task in an isolated worktree.
2. Allow the agent to edit files and run validation commands.
3. Capture diff size, tool calls, repair loops, and final test results.
4. Compare the same tasks against Claude Code/Codex-style baselines when available.
