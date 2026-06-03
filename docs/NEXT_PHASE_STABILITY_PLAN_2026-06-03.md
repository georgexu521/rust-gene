# Next Phase Stability Plan

Date: 2026-06-03

Status: active implementation plan

## Summary

Priority Agent's next phase should be a stabilization phase, not a feature
expansion phase.

The product is already able to run realistic engineering-agent tasks: core
tooling, memory recall, multi-file edits, Rust refactors, and verification
repair paths are covered by deterministic tests and live evals. The current
weak spots are narrower and more operational:

1. complex validation-repair loops still have a slow and unreliable tail;
2. TUI and desktop real-device paths are not yet as proven as `--eval-run`;
3. provider timeout and weak-model behavior need clearer early-stop and owner
   classification;
4. the daily baseline needs to become a stable product-health signal.

The target for this phase is:

> make the main coding-agent path reliable, observable, and honestly
> classifiable before adding more product surface area.

## Current Baseline

The current working baseline is stronger than the older daily report suggests.
After follow-up fixes, the deterministic gates passed in this tree, including
the full Rust test suite before build artifacts were cleaned.

Recent live daily eval status:

- product daily live eval: 7/8 passing;
- passing cases included inspection, stale edit, multi-file edit, Rust
  refactor, project memory resume, memory conflict recall, and minimum
  verification repair;
- remaining failing case: `code-change-verification-repair-loop`;
- observed failure mode: wall timeout after repeated weak-model repair attempts
  without an effective code edit;
- current interpretation: this is a complex repair-loop / weak-provider
  reliability problem, not evidence that core tools or validation gates are
  broadly broken.

This plan should preserve the important invariant: a failed or partial run is
acceptable when the evidence is honest. False green closeout is not acceptable.

## Non-Goals

- Do not add broad new tool categories.
- Do not add always-on prompt rules for one-off weak-model mistakes.
- Do not weaken validation, permissions, checkpoints, high-risk gates, or proof
  requirements to make a weak provider pass.
- Do not treat TUI, desktop, and eval-run as separate products. They should keep
  sharing the same runtime path.
- Do not make daily evals so heavy that they stop being usable as a routine
  health check.

## Phase 0: Freeze The Current Baseline

Goal: preserve the known-good stabilization work before starting deeper repair
loop changes.

Implementation status: done in baseline commit `5ca87926`.

### Work

- Commit the current source and documentation fixes as one scoped baseline
  commit.
- Keep generated benchmark and evidence directories out of the source commit
  unless they are intentionally needed as permanent documentation.
- Record the current live eval state clearly: daily is mostly green, with the
  complex repair-loop case still failing under the current weak provider.

### Acceptance

```bash
cargo fmt --check
cargo test -q -- --test-threads=1
bash -n scripts/product-daily-gate.sh scripts/run_live_eval.sh scripts/active-memory-baseline.sh
python3 -m py_compile scripts/live_eval_report_parser.py
bash scripts/product-daily-gate.sh --dry-run --include-desktop
```

Expected result:

- deterministic tests pass;
- daily dry-run lists the intended product cases;
- no generated build cache or live-eval worktree is accidentally committed.

## Phase 1: Stabilize Complex Repair Loops

Goal: make `code-change-verification-repair-loop` reliable with capable
providers and honest, bounded, diagnosable with weak providers.

Implementation status: in progress. The daily runner now supports single-case
execution, and `code-change-verification-repair-loop` runs with an explicit
no-effective-worktree-progress timeout so weak-provider slow tails close out
with structured evidence instead of only hitting the wall timeout.

The repair loop should be treated as a product workflow:

1. validation fails;
2. failure evidence is reduced to a precise repair context;
3. the model chooses and applies a repair;
4. validation is rerun;
5. the runtime either verifies success or closes as failed / partial /
   not-verified with evidence.

### Work

- Inspect the repair context passed back to the model after failed validation:
  command, exit status, compiler/test error, target file, recent diff, and prior
  repair attempts.
- Reduce noisy repair context while preserving the actual actionable failure.
- Add or tighten no-effective-repair detection:
  - no code diff after repair attempt;
  - repeated identical failed command without new action;
  - repeated reads of the same unchanged context without narrowing;
  - tool run without closeout.
- Add an early-stop policy for repair-loop slow tails. If the model has not
  produced a meaningful new action after a bounded number of turns, close out
  as failed or partial with explicit evidence instead of burning the full wall
  timeout.
- Keep validation strict. A compile error, missing required command, missing
  diff, or missing proof must still block verified success.

### Acceptance

- A capable provider can pass `code-change-verification-repair-loop`.
- A weak provider that cannot repair the fixture fails earlier and more clearly
  than the current 1200 second wall timeout.
- The report distinguishes:
  - model failed to make a useful edit;
  - provider timed out;
  - runtime failed to feed evidence back;
  - harness setup failed.
- No false green is introduced.

### Validation

```bash
bash scripts/product-daily-gate.sh --case code-change-verification-repair-loop
PRIORITY_AGENT_DAILY_REPAIR_NO_EFFECTIVE_PROGRESS_SECS=360 \
  bash scripts/product-daily-gate.sh --case code-change-verification-repair-loop
cargo test -q reflection_pass -- --test-threads=1
cargo test -q evalset -- --test-threads=1
cargo test -q closeout
```

Single-case execution is now available through `--case`.

## Phase 2: Prove TUI And Desktop Real Paths

Goal: prove the user-facing entrypoints, not only the noninteractive eval-run
path.

Implementation status: initial entrypoint smoke is available through
`scripts/runtime-entrypoint-smoke.sh`. It separates headless runtime dogfood,
real pseudo-terminal CLI/TUI launch smoke, desktop quick smoke, and packaged
native desktop smoke. Full automated TUI task submission remains a later stretch
item because the current full-screen interface still requires terminal
interaction.

The current daily runner validates the runtime through `--eval-run`. That is
useful, but it does not fully prove the real TUI and desktop experience. The
next phase should add small, explicit smoke gates for the real launch paths.

### TUI Work

- Add a minimal TUI smoke path that starts the app, submits one simple coding or
  inspection task, observes streamed output, and exits cleanly.
- Verify that tool events, validation status, and closeout state are visible in
  the transcript.
- Keep this smoke small enough to run manually or as a slower scheduled gate.

### Desktop Work

- Add a desktop smoke path for the Tauri app:
  - app starts;
  - project is bound to the expected workspace;
  - a task is submitted;
  - `run_submit` and `run_completed` are visible in logs or persisted state;
  - transcript cards render without obvious stuck or blank states.
- Use the established desktop diagnosis path:
  - inspect process tree and local port;
  - inspect `desktop.log`;
  - snapshot `sessions.db` plus WAL/SHM before querying;
  - avoid direct reads against a locked live SQLite DB.

### Acceptance

- `priority-agent --cli`, `priority-agent --tui`, and desktop can each run one
  simple real task.
- A failed TUI or desktop run has enough logs to decide whether the issue is
  runtime, UI shell, provider, or environment.
- Daily docs clearly distinguish `--eval-run` baseline from TUI/desktop smoke.

### Validation

Recommended commands should be finalized after checking the current app launch
scripts. The expected shape is:

```bash
scripts/runtime-entrypoint-smoke.sh --dry-run --all
scripts/runtime-entrypoint-smoke.sh --headless
scripts/runtime-entrypoint-smoke.sh --cli
scripts/runtime-entrypoint-smoke.sh --tui
scripts/runtime-entrypoint-smoke.sh --desktop-quick
scripts/runtime-entrypoint-smoke.sh --desktop-native
```

`--desktop-native` builds and launches the packaged macOS app through the
existing native smoke path. Use it as a scheduled or pre-release check, not as
the default fast daily gate.

## Phase 3: Provider Timeout And Slow-Tail Control

Goal: make provider weakness, latency, and timeout behavior visible and bounded.

Implementation status: in progress. Agent-run now writes
`agent-run-metrics.json` with elapsed time, first activity, first worktree diff,
no-effective-progress duration, termination reason, provider family/model, and
streaming tool mode. Product daily summaries include these slow-tail fields.

The runtime should not assume every provider has the same latency, tool-call
behavior, streaming support, or repair ability. It should also avoid mixing
provider failures with agent-flow defects.

### Work

- Normalize provider failure classification:
  - API timeout;
  - stream interruption;
  - empty content;
  - malformed tool call;
  - tool result without closeout;
  - model made wrong edit;
  - model made no effective edit;
  - harness or environment setup failure.
- Add slow-tail fields to live eval summaries:
  - first token time;
  - total wall time;
  - tool round count;
  - last effective action time;
  - provider family and model;
  - streaming vs non-streaming tool mode.
- Define a weak-provider task matrix:
  - smoke tasks that weak providers must pass;
  - product tasks used for main quality;
  - stretch tasks that measure capability but do not alone define product
    health.
- Add early stop for long periods with no effective progress.

### Acceptance

- Provider API timeout is not reported as `agent_flow`.
- Slow model behavior does not hide whether the runtime fed back the right
  validation evidence.
- Long-tail failures produce a useful report before the user has to wait for a
  full wall-time timeout.
- Weak providers can fail honestly without weakening product gates.

## Phase 4: Formalize The Daily Baseline

Goal: turn daily evals from ad hoc evidence into a stable health dashboard.

Implementation status: daily layers are now script-level concepts:

```bash
bash scripts/product-daily-gate.sh --layer smoke --dry-run
bash scripts/product-daily-gate.sh --layer product --dry-run
bash scripts/product-daily-gate.sh --layer stretch --dry-run
bash scripts/product-daily-gate.sh --case code-change-verification-repair-loop --dry-run
```

The daily baseline should have three layers.

### Layer 1: Daily Smoke

Fast, stable, must-pass checks for every normal development day.

Recommended coverage:

- read-only inspection;
- simple file edit;
- basic tool validation;
- memory recall smoke;
- closeout proof smoke.

Expected behavior:

- runs quickly;
- failures are release-blocking unless clearly environmental;
- no desktop dependency.

### Layer 2: Daily Product

The main product health line.

Recommended coverage:

- `core-inspection-grounding`;
- `core-simple-stale-edit`;
- `core-multi-file-edit`;
- `core-rust-multi-file-refactor`;
- `project-partner-resume-with-memory`;
- `memory-recall-conflict-precision`;
- `minimum-agent-verification-repair`;
- `code-change-verification-repair-loop`.

Expected behavior:

- reports capability level, proof status, closeout status, failure owner, and
  root cause;
- can be used to decide whether the current tree is fit for normal dogfooding;
- complex repair-loop failure should be clearly separated from broad product
  failure.

### Layer 3: Daily Stretch / Scheduled

Slower checks that should run manually, nightly, or before larger releases.

Recommended coverage:

- full desktop smoke;
- full TUI smoke;
- weak-provider stress tasks;
- long repair-loop tasks;
- full-suite validation inside live eval worktrees.

Expected behavior:

- failures are triaged, not automatically treated as mainline breakage;
- reports preserve enough artifacts for debugging;
- generated worktrees and build caches are cleaned or capped.

### Acceptance

- One command can run each layer.
- Reports include:
  - pass rate;
  - failed cases;
  - capability level;
  - failure owner;
  - provider/model;
  - proof/closeout status;
  - whether the failure blocks daily use.
- Every daily task declares `capability_level`.
- Every known expected weakness has an owner classification rule.

## Suggested Execution Order

1. Commit the current stabilization baseline.
2. Harden `code-change-verification-repair-loop` as a focused slice.
3. Add TUI and desktop smoke gates as a separate slice.
4. Add provider slow-tail reporting and early-stop behavior.
5. Split daily baseline into smoke, product, and stretch layers.
6. Update `docs/PROJECT_STATUS.md` only after the new baseline is repeatable.

## Success Criteria For This Phase

This phase is done when Priority Agent can answer these operational questions
clearly:

- Can the core agent path run normal coding tasks today?
- If a task fails, is the failure from provider behavior, model judgment,
  runtime flow, harness setup, or environment?
- Did the agent actually change code when repair was required?
- Did validation proof really pass?
- Did TUI and desktop run through the same runtime path successfully?
- Is the daily baseline stable enough to trust as the product health signal?

The goal is not a perfect score on every weak-provider stretch task. The goal
is a stable, verifiable product loop where success is real and failure is
actionable.
