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

- [x] `memory-recall-conflict-precision`
  - Type: bug_fix.
  - Why: tests memory conflict precision and retrieval relevance.
  - Expected pressure: memory reasoning without over-demotion.
  - Result: failed in `capability-memory-conflict-20260503-182641`.
  - Key learning: tests were green on unchanged code, but agent produced no
    code diff and hidden deterministic patch synthesis failed; this is a
    workflow-boundary issue, not a product pass.

- [x] `permission-default-open-dangerous-guard`
  - Type: bug_fix.
  - Why: tests safety boundaries while preserving smooth developer flow.
  - Expected pressure: avoid both over-blocking and dangerous permissiveness.
  - Result: failed/stale in `capability-permission-guard-20260503-205410`.
  - Key learning: unchanged baseline already passes required permission and bash
    tests with default-open safety coverage; the quality gate correctly rejected
    a no-diff code-change run.

### Batch C: Product Surface

- [x] `live-eval-dashboard-summary`
  - Type: feature.
  - Result: failed in `capability-dashboard-summary-20260503-213148`.
  - Key learning: seeded code-change intent worked as a gate; the agent
    inspected but produced no diff, action checkpoint prevented false success,
    and required commands exposed a PyYAML dependency in isolated eval
    worktrees.
  - Follow-up implemented: `scripts/run_live_eval.sh --list` no longer requires
    PyYAML, `--mode summary --run-id <id>` writes `summary.md`, and the live
    task now seeds the summary gap with `prepare_commands`.
  - Rerun after follow-up: failed in
    `capability-dashboard-summary-rerun-20260503-235256`; the seeded fixture
    worked and required `--mode summary` failed, but the model again produced no
    `file_edit`.
  - Harness follow-up: `run_live_eval.sh` now uses Ruby YAML/JSON stdlib for
    live task parsing instead of requiring PyYAML in prepare/collect paths.
    The dashboard summary fixture now keeps the `--mode summary` entrypoint and
    replaces only `summary_task()` with an explicit not-implemented stub, so the
    seeded edit target is narrower and less likely to require reconstructing a
    large script block from scratch.
  - Harness rerun after dependency cleanup:
    `capability-dashboard-summary-pyyaml-free-20260505-225124` failed without
    PyYAML traceback. Failure owner remained `llm_reasoning`: no code diff,
    required commands failed on the seeded `summary_task()` stub, and closeout
    stayed `not_verified`.

- [x] `resume-session-picker`
  - Type: feature.
  - Why: tests Claude-like daily CLI workflow.
  - Local implementation: completed on 2026-05-05.
  - Local validation:
    `cargo test -q resume -- --test-threads=1`,
    `cargo test -q session -- --test-threads=1`, and
    `cargo test -q -- --test-threads=1` passed.
  - Key change: `/resume` resolves number/id/search selections and restored
    sessions show recent conversation preview.

- [x] `cli-scrollback-polish`
  - Type: ux.
  - Why: tests interactive CLI polish without overfitting to tests.
  - Local implementation: completed on 2026-05-05.
  - Local validation:
    `cargo test -q shell -- --test-threads=1`,
    `cargo test -q tui -- --test-threads=1`, and
    `cargo test -q -- --test-threads=1` passed with `1060 passed; 0 failed`.
  - Key change: scrollback shell now surfaces concise long-running tool
    progress lines while keeping assistant text and tool status readable.

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

### `memory-recall-conflict-precision`

- Report path:
  `docs/benchmarks/live-capability-memory-conflict-20260503-182641/memory-recall-conflict-precision/report.md`
- Status: failed.
- Failure owner: agent_flow.
- Required commands: ok on unchanged baseline.
- Files changed: none.
- Specialty signals: 4/6 active.
  - `memory_active=true`
  - `automation_active=true`
  - `guided_debugging_active=false`
  - `guided_reasoning_active=true`
  - `weighted_planning_active=true`
  - `closeout_active=false`
- Acceptance gaps: no acceptance review ran because there was no code diff and
  no recorded validation event from the agent workflow.
- False-success behavior: good; the quality gate rejected the run despite all
  required commands passing, because the requested code-change task produced no
  change and closeout was `not_verified`.
- Improvement type: runtime boundary reduction. Disable deterministic
  task-specific patch synthesis by default and keep it only as explicit opt-in
  research behavior.

### `memory-recall-conflict-precision` no-synth rerun

- Report path:
  `docs/benchmarks/live-capability-memory-conflict-nosynth-20260503-183836/memory-recall-conflict-precision/report.md`
- Status: failed.
- Failure owner: agent_flow.
- Required commands: ok on unchanged baseline.
- Files changed: none.
- Specialty signals: 5/6 active.
  - `memory_active=true`
  - `automation_active=true`
  - `guided_debugging_active=true`
  - `guided_reasoning_active=true`
  - `weighted_planning_active=true`
  - `closeout_active=false`
- Acceptance gaps: no acceptance review ran because no code diff or validation
  event was recorded by the agent workflow.
- False-success behavior: good; quality gates again rejected an unchanged
  code-change run.
- Improvement type: generic workflow/tool-surface fix. In focused repair mode,
  hide `bash` until a file change exists so the model is steered toward
  `file_edit`/`file_write` first and then validation.

### `memory-recall-conflict-precision` focused-repair rerun

- Report path:
  `docs/benchmarks/live-capability-memory-conflict-focused-20260503-185437/memory-recall-conflict-precision/report.md`
- Status: failed.
- Failure owner: agent_flow.
- Required commands: ok on unchanged baseline.
- Files changed: none.
- Specialty signals: 4/6 active.
  - `memory_active=true`
  - `automation_active=true`
  - `guided_debugging_active=false`
  - `guided_reasoning_active=true`
  - `weighted_planning_active=true`
  - `closeout_active=false`
- Acceptance gaps: no acceptance review ran because no code diff or validation
  event was recorded by the agent workflow.
- False-success behavior: good; quality gates rejected another unchanged
  code-change run.
- Improvement type: runtime boundary reduction. Generic patch synthesis still
  activated after focused repair attempts, so disable all patch synthesis by
  default behind explicit opt-in.

### `memory-recall-conflict-precision` model-edit rerun

- Report path:
  `docs/benchmarks/live-capability-memory-conflict-modelledit-20260503-191431/memory-recall-conflict-precision/report.md`
- Status: failed.
- Failure owner: agent_flow.
- Required commands: ok on unchanged baseline.
- Files changed: none.
- Specialty signals: 5/6 active.
  - `memory_active=true`
  - `automation_active=true`
  - `guided_debugging_active=true`
  - `guided_reasoning_active=true`
  - `weighted_planning_active=true`
  - `closeout_active=false`
- Acceptance gaps: no acceptance review ran because no code diff or validation
  event was recorded by the agent workflow.
- False-success behavior: good; quality gates rejected an unchanged run.
- Improvement type: tool-feedback observability. With patch synthesis disabled,
  the model eventually attempted `file_edit`, but report-visible tool output did
  not include the concrete failure reason. Failed tool results should surface
  error text in visible content so the next model turn can repair from evidence.

### `memory-recall-conflict-precision` visible-error rerun

- Report path:
  `docs/benchmarks/live-capability-memory-conflict-visible-errors-20260503-204111/memory-recall-conflict-precision/report.md`
- Status: failed.
- Failure owner: agent_flow / stale_eval.
- Required commands: ok on unchanged baseline.
- Files changed: none.
- Specialty signals: 4/6 active.
  - `memory_active=true`
  - `automation_active=true`
  - `guided_debugging_active=false`
  - `guided_reasoning_active=true`
  - `weighted_planning_active=true`
  - `closeout_active=false`
- Acceptance gaps: no acceptance review ran because no code diff or validation
  event was recorded by the agent workflow.
- False-success behavior: good; quality gates rejected an unchanged code-change
  run.
- Improvement type: eval maintenance. The unchanged branch already contains
  generic conflict-token guards and related tests, so this case is no longer a
  clean editing-capability test. Keep it as evidence that stale eval cases need
  a baseline freshness check before repeated reruns.

### `permission-default-open-dangerous-guard`

- Report path:
  `docs/benchmarks/live-capability-permission-guard-20260503-205410/permission-default-open-dangerous-guard/report.md`
- Status: failed/stale.
- Failure owner: eval_harness / stale_eval after report recollect.
- Required commands: ok on unchanged baseline.
- Files changed: none.
- Specialty signals: 4/6 active.
  - `memory_active=true`
  - `automation_active=true`
  - `guided_debugging_active=false`
  - `guided_reasoning_active=true`
  - `weighted_planning_active=true`
  - `closeout_active=false`
- Acceptance gaps: no acceptance review ran because no code diff or validation
  event was recorded by the agent workflow.
- False-success behavior: good; quality gates rejected an unchanged code-change
  run.
- Improvement type: eval maintenance. The branch already has AutoAll
  default-open safety tests for safe operations versus dangerous bash, external
  network, unsafe writes, git push, and memory_clear. Refresh this case before
  using it again as an editing-capability signal.

### Top-level capability review

- Review doc:
  `docs/TOP_LEVEL_AGENT_CAPABILITY_REVIEW_2026-05-03.md`
- Core diagnosis: current small real coding tasks pass, while two recent
  specialty failures are stale/current-HEAD baselines rather than clean
  editing-capability failures.
- Runner change: `scripts/run_live_eval.sh` now reports
  `current_head_no_fixture_already_satisfied` and attributes no-diff,
  required-commands-passing current-HEAD cases without `prepare_commands` to
  `eval_harness`.
- Eval taxonomy change: every live task now declares `eval_intent`.
  `seeded_code_change` tasks require a diff; `audit_or_regression_check` tasks
  can legitimately close out with no diff if they prove current behavior and
  required commands pass.
- Validation:
  - `bash -n scripts/run_live_eval.sh`
  - `scripts/run_live_eval.sh --list`
  - recollected
    `permission-default-open-dangerous-guard`, which now reports
    `failure_owner=eval_harness` while keeping status failed because closeout
    was `not_verified`.
- Boundary decision: keep patch synthesis disabled by default and fix stale eval
  design instead of forcing hidden runtime edits.

## Stop Conditions

- Stop and fix the harness if reports are missing trace, output, or required
  command status.
- Stop and review before changing runtime if two consecutive runs fail for
  different task-specific reasons.
- Do not add file-specific hidden heuristics for a single missed acceptance
  target.
- Do not mark a task successful if acceptance review or required commands fail.
