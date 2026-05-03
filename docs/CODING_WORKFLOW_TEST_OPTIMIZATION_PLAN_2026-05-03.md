# Coding Workflow Test Optimization Plan

Date: 2026-05-03

This plan turns the current coding-agent workflow checks into a layered,
low-waste test strategy. The goal is to prove the writing-code loop works
without running expensive live evals on every small change.

## What Must Be Proven

The coding workflow is considered healthy when these contracts hold:

- Code-change routing records implementation intent before edits.
- Tool execution records start/done events and structured summaries.
- File edits, bash validation, git summaries, hooks, subagents, and permission
  decisions keep their deterministic contracts.
- Failed validation triggers repair/blocked closeout instead of a false success.
- Passed validation plus acceptance review produces deterministic closeout.
- Final closeout reports real evidence, not stale or opportunistic commands.
- TUI progress and panels show useful state for validation and long-running
  commands.

## Test Tiers

### Tier 0: Focused Fast Gate

Use after small changes to closeout, progress labels, command classification,
tool summaries, or eval reporting.

- [x] Add one script entry point for focused workflow tests.
- [x] Include closeout, progress-label, command-classifier, git summary, eval
  report, and coding replay matrix tests.
- [x] Keep expected runtime low enough for frequent use.

### Tier 1: Standard Coding Workflow Gate

Use before committing workflow/tool changes.

- [x] Add one script entry point that runs Tier 0 plus `cargo check -q`.
- [x] Include the bundled deterministic coding replay matrix.
- [x] Avoid live LLM/API work.

### Tier 2: Full Local Gate

Use before merging or after a multi-file batch.

- [x] Reuse `scripts/validate_docs.sh` for docs/build/full-test consistency.
- [x] Keep it local and deterministic.

### Tier 3: Live Smoke Gate

Use only after changing the actual agent loop or repair/closeout behavior.

- [x] Add a documented wrapper for one representative live eval case.
- [x] Default to `code-change-verification-repair-loop`.
- [x] Do not run `--case all` by default.
- [x] Make live smoke opt-in and clearly warn that it uses the real agent path.
- [x] Execute the representative live smoke once after the deterministic gates.

Latest live smoke result:

- Run id: `live-eval-20260503-152320`
- Case: `code-change-verification-repair-loop`
- Status: `ok`
- Report:
  `docs/benchmarks/live-live-eval-20260503-152320/code-change-verification-repair-loop/report.md`
- Full suite inside the live run: `1053 passed; 0 failed`
- Key signals: `verification_passed=true`, `stage_validation_passed=true`,
  `acceptance_accepted=True`, `closeout_status=passed`,
  `failure_owner=none`
- Runtime visibility: long-running validation emitted progress notices after
  30s/60s/90s as expected.

## Implementation Tasks

- [x] Create `scripts/coding-workflow-gates.sh`.
- [x] Document the script in `scripts/README.md`.
- [x] Add `--help`, `quick`, `standard`, `full`, and `live-smoke` modes.
- [x] Run the new quick and standard gates.
- [x] Run the full local gate.
- [x] Run the opt-in live smoke gate.
- [x] Update this plan with completed checks.
- [x] Commit the script and docs.

## Recommended Daily Usage

Small code change:

```bash
scripts/coding-workflow-gates.sh quick
```

Workflow/tool/eval change before commit:

```bash
scripts/coding-workflow-gates.sh standard
```

Stage closeout:

```bash
scripts/coding-workflow-gates.sh full
```

Agent-loop behavior change:

```bash
scripts/coding-workflow-gates.sh live-smoke
```

## Stop Conditions

- Do not run live smoke automatically inside `quick`, `standard`, or `full`.
- Do not make `--case all` the default live behavior.
- If a focused test fails, stop there instead of running broader gates.
