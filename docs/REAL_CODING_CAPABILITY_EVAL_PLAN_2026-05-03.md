# Real Coding Capability Eval Plan

Date: 2026-05-03

This plan evaluates whether `rust-agent` can act as a real programming agent,
not just pass deterministic replay tests.

## Principles

- Prefer fresh `agent-run` evidence over opinions.
- Do not tune hidden runtime rules after one failure.
- First classify failures: `llm_reasoning`, `agent_flow`, `tooling`,
  `eval_harness`, or `environment`.
- Improve by telling the AI how to work, improving evidence, or tightening
  false-success guards before adding new auto-repair behavior.
- Keep the runtime's role to structure, safety, evidence, and review. The model
  should still decide the implementation.

## Scorecard

Each run gets a compact score:

| Dimension | Question |
|-----------|----------|
| Task success | Did the final implementation satisfy the task? |
| Required validation | Did every required command pass? |
| Acceptance coverage | Did it close every named file/path/behavior target? |
| Diff discipline | Did it modify only relevant files? |
| Repair behavior | If validation failed, did guided debugging or repair help? |
| Specialty signals | Did memory, automation, guided reasoning, weighted planning, and closeout appear when relevant? |
| False-success guard | Did it avoid claiming success when incomplete? |

## Task Matrix

### Batch A: Productive Baseline

- [x] `backend-todo-api-crud`
  - Type: feature.
  - Why: tests basic implementation across a small real API surface.
  - Expected pressure: file editing, test running, endpoint coverage.
  - Result: passed in `capability-backend-20260503-175233`.
  - Key learning: agent completed a real stdlib CRUD API and recovered through
    validation/acceptance iterations; tool errors were visible but did not cause
    false success.

- [x] `frontend-book-notes-localstorage`
  - Type: feature.
  - Why: tests frontend/product behavior and persistence.
  - Expected pressure: UX completeness, browser-like user flow, validation.
  - Result: passed in `capability-frontend-20260503-180633`.
  - Key learning: agent completed the product behavior in one relevant frontend
    file with clean validation; repeated acceptance rejections show useful
    false-success resistance but measurable rework cost.

### Batch B: Core Agent Features

- [x] `persistent-memory-planning-context`
  - Type: bug_fix.
  - Result: passed in `realflow-memory-20260503-163910`.
  - Key learning: memory/planning path worked; weight floor needed follow-up.

- [x] `memory-save-quality-gate`
  - Type: bug_fix.
  - Result: failed in `realflow-guided-20260503-170614`.
  - Key learning: guided debugging fired and false success was prevented; model
    missed an explicit `src/tui/app.rs` acceptance target.

- [ ] `memory-recall-conflict-precision`
  - Type: bug_fix.
  - Why: tests memory conflict precision and retrieval relevance.
  - Expected pressure: memory reasoning without over-demotion.

- [ ] `permission-default-open-dangerous-guard`
  - Type: bug_fix.
  - Why: tests safety boundaries while preserving smooth developer flow.
  - Expected pressure: avoid both over-blocking and dangerous permissiveness.

### Batch C: Product Surface

- [ ] `resume-session-picker`
  - Type: feature.
  - Why: tests Claude-like daily CLI workflow.

- [ ] `cli-scrollback-polish`
  - Type: ux.
  - Why: tests interactive CLI polish without overfitting to tests.

## Run Commands

Use fresh agent runs:

```bash
scripts/run_live_eval.sh --case <case-id> --mode agent-run --run-tests --timeout 1800 --idle-timeout 300 --label capability
```

For lightweight deterministic regression after code changes:

```bash
scripts/coding-workflow-gates.sh quick
scripts/coding-workflow-gates.sh standard
```

## Review Template

For each run, record:

- Report path:
- Status:
- Failure owner:
- Required commands:
- Files changed:
- Specialty signals:
- Acceptance gaps:
- False-success behavior:
- Improvement type:
  - prompt/evidence/review
  - runtime guard
  - tool fix
  - eval harness fix
  - no change

## Run Log

### `backend-todo-api-crud`

- Report path:
  `docs/benchmarks/live-capability-backend-20260503-175233/backend-todo-api-crud/report.md`
- Status: passed.
- Failure owner: none.
- Required commands: ok.
- Files changed: `fixtures/live_backend/todo_api/todo_api.py`.
- Specialty signals: 5/6 active.
  - `memory_active=true`
  - `automation_active=true`
  - `guided_debugging_active=true`
  - `guided_reasoning_active=false`
  - `weighted_planning_active=true`
  - `closeout_active=true`
- Acceptance gaps: earlier acceptance reviews rejected the result, then final
  acceptance passed after repair.
- False-success behavior: good; earlier failures were retained as warnings and
  the final closeout only passed after required commands succeeded.
- Improvement type: no immediate runtime change. Keep watching tool errors and
  whether medium feature tasks should expose more useful plan priorities.

### `frontend-book-notes-localstorage`

- Report path:
  `docs/benchmarks/live-capability-frontend-20260503-180633/frontend-book-notes-localstorage/report.md`
- Status: passed.
- Failure owner: none.
- Required commands: ok.
- Files changed: `fixtures/live_frontend/book_notes/app.js`.
- Specialty signals: 5/6 active.
  - `memory_active=true`
  - `automation_active=true`
  - `guided_debugging_active=true`
  - `guided_reasoning_active=false`
  - `weighted_planning_active=true`
  - `closeout_active=true`
- Acceptance gaps: five earlier acceptance reviews rejected the result, then
  final acceptance passed with zero unresolved items.
- False-success behavior: good; required commands and acceptance were both
  green before closeout.
- Improvement type: no immediate runtime change. Track repeated acceptance
  loops as a prompt/review calibration signal before considering runtime logic.

## Stop Conditions

- Stop and fix the harness if reports are missing trace, output, or required
  command status.
- Stop and review before changing runtime if two consecutive runs fail for
  different task-specific reasons.
- Do not add file-specific hidden heuristics for a single missed acceptance
  target.
- Do not mark a task successful if acceptance review or required commands fail.
