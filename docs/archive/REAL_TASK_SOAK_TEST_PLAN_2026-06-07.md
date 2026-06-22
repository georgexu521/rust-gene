# Real Task Soak Test Plan

Date: 2026-06-07

Status: proposed release-readiness soak plan after runtime/API/provider/UI
hardening

## 1. Purpose

Priority Agent has enough core runtime and programming-chain infrastructure to
start real usage testing. The next goal is not to add more features. The goal is
to prove that normal programming work can run end to end through the same
runtime paths users will actually use.

This plan answers:

- Can the agent complete real coding tasks with correct edits and validation?
- Can CLI, TUI, desktop, and HTTP full-agent API all use the same runtime
  behavior?
- Can the product explain failures with session events, usage ledger, provider
  status, checkpoints, diagnostics, and closeout evidence?
- Can long or repeated tasks avoid hangs, false verified closeout, runaway token
  use, unsafe shell mutation, and unrecoverable state?

## 2. Current Test Baseline

### 2.1 Deterministic Gates Already Available

`scripts/daily-baseline.sh` is the no-provider baseline. It covers compilation,
formatting, API feature compilation, runtime-controller tests, route-scoped
tools, closeout, permissions, checkpoint, file tools, desktop runtime,
usage/cost accounting, edit matching, full lib tests, file-size stewardship, and
script syntax.

Use it before any paid/provider soak:

```bash
bash scripts/daily-baseline.sh
```

Recent targeted gates also covered the newest API/runtime slice:

```bash
cargo check --features experimental-api-server -q
cargo fmt --check
git diff --check
bash -n scripts/programming-soak-suite.sh
cargo test -q api::session_runner --features experimental-api-server
cargo test -q api::routes --features experimental-api-server
cargo test -q persist_session_job_if_shell
cargo test -q file_tool
```

These prove the low-level wiring is testable. They do not prove real user
stability.

### 2.2 Live Eval / Daily Gate Coverage Already Built

`scripts/product-daily-gate.sh` has three layers:

- `smoke`: `core-inspection-grounding`, `core-simple-stale-edit`,
  `project-partner-resume-with-memory`, `minimum-agent-verification-repair`.
- `product`: smoke plus multi-file edit/refactor, verification repair,
  memory-recall precision.
- `stretch`: desktop UI, long output, provider roundtrip, verification repair.

The 2026-06-03 report recorded 8 daily product cases: 4 passed and 4 failed.
Three failures were provider timeout/environment, and one was an agent-flow /
behavior-assertion issue that later had deterministic follow-up fixes. The
important lesson is that live results must classify failure owner before
changing agent logic.

### 2.3 Real Programming Plans Already Written

Existing docs already proposed useful real tasks:

- `docs/archive/LIVE_CODING_TEST_PLAN_2026-06-01.md`: analysis, bug fix, adding tests,
  multi-file edit, validation repair loop, force-summary pressure.
- `docs/archive/REAL_WORLD_TEST_PLAN_2026-06-01.md`: refactor, bug repair, cross-module
  API change, new feature, CLI flag, larger memory-module audit.
- `docs/RELEASE_CANDIDATE_STABILITY_TEST_PLAN_2026-06-05.md`: clean checkout,
  deterministic daily baseline, CLI/TUI/desktop real paths, provider/cost
  review, recovery behavior.
- `docs/archive/OPENCODE_PROGRAMMING_PARITY_NEXT_PLAN_2026-06-07.md`: remaining gap is
  product maturity under long real programming tasks, not missing basic agent
  functions.

This document consolidates those into one executable soak plan.

### 2.4 New Soak Scripts

`scripts/programming-soak-suite.sh` is now the focused programming soak runner.
It writes prompt, stdout/stderr, agent output, events, `run.json`, and a
baseline markdown bundle under `target/soak-bundles/`.

Dry-run:

```bash
PRIORITY_AGENT_SOAK_DRY_RUN=1 bash scripts/programming-soak-suite.sh single-file-bug-fix
```

Real run:

```bash
bash scripts/programming-soak-suite.sh all
```

`scripts/api-full-agent-soak.sh` verifies the HTTP full-agent path:

```bash
bash scripts/api-full-agent-soak.sh
```

## 3. Soak Principles

- Use real tasks, real file edits, real validation commands, and saved
  artifacts.
- Run deterministic gates first; do not spend provider tokens when the local
  tree is already broken.
- Keep weak-model mistakes separate from framework bugs.
- Never weaken validation, permissions, checkpoints, or proof closeout to make
  a live run pass.
- Prefer tasks that can be undone or run in isolated fixture/worktree folders.
- Capture exact session id, provider/model, prompt, events, diff, validation,
  usage, cache, and failure-owner classification.
- Treat false verified closeout, unsafe shell mutation, data loss, and
  unrecoverable session state as release blockers.

## 4. Execution Lanes

### Lane 0: Preflight

Purpose: confirm the tree is healthy before live provider work.

Commands:

```bash
git status --short --branch
bash scripts/daily-baseline.sh
cargo check --features experimental-api-server -q
```

Pass criteria:

- Deterministic gates pass.
- Any dirty files are intentional and recorded.
- There is at least enough disk space for `target/live-evals` and
  `target/soak-bundles`.

### Lane 1: CLI / Eval-Run Real Programming

Purpose: test the core programming loop in the most controllable full-agent
path.

Command:

```bash
bash scripts/programming-soak-suite.sh all
```

Run at least these 8 scenarios:

| Scenario | Main Risk Covered | Pass Signal |
|---|---|---|
| `single-file-bug-fix` | read-before-edit, precise mutation | one source edit, targeted validation, verified closeout |
| `multi-file-refactor` | cross-file coherence | 2+ coherent edits, cargo check/test passes |
| `frontend-component-change` | desktop/TUI/UI surface editing | UI-facing code compiles or lint/build gate passes |
| `failing-test-repair` | edit-test-repair loop | first failure becomes tool evidence, repair passes |
| `permission-deny-safe-retry` | shell write guardrail | bash mutation is avoided/blocked, file tool path succeeds |
| `long-shell-output-paging` | output flood control | long output is paged/bounded, not pasted into final answer |
| `revert-last-assistant-turn` | checkpoint and revert | changed file restored, revert result is visible |
| `provider-slow-tail-classification` | timeout/slow-tail productization | timeout/latency is classified without false agent-flow blame |

Artifacts:

- `target/soak-bundles/<run-id>/<task>/prompt.txt`
- `agent-output.md`
- `events.jsonl`
- `stdout.log`
- `stderr.log`
- `run.json`
- `baseline.md`

### Lane 2: Product Daily Live Baseline

Purpose: run the existing representative eval set and compare trend against
the 2026-06-03 baseline.

Smoke run:

```bash
bash scripts/product-daily-gate.sh --layer smoke --skip-provider-health
```

Product run:

```bash
bash scripts/product-daily-gate.sh --layer product --skip-provider-health
```

Optional stretch:

```bash
bash scripts/product-daily-gate.sh --layer stretch --include-desktop --skip-provider-health
```

Pass criteria for release candidate:

- Smoke: 4/4 pass or failures are provider/environment with clear evidence.
- Product: at least 7/8 pass, or every failure has a non-agent-flow
  classification and no release-blocker symptoms.
- Stretch: no false verified closeout, no data loss, no unsafe permission
  bypass.

### Lane 3: TUI Real Task

Purpose: test the actual interactive terminal path, not only eval-run.

Command:

```bash
cargo run -- --tui
```

Manual tasks:

1. Ask for a small single-file bug fix and targeted test.
2. Ask for a 2-file refactor and validation.
3. Trigger or observe a long shell output and confirm it is inspectable without
   flooding the main view.
4. Reload or resume the session and confirm completed parts remain visible.
5. Run a revert after a tiny edit and confirm the UI shows the reverted state.

Pass criteria:

- TUI uses the same full-agent runtime path.
- Tool cards render useful status for file mutations, shell jobs, permission,
  usage, and closeout.
- Failed validation blocks verified closeout.
- Long output and background/foreground shell status are inspectable.

### Lane 4: Desktop Real Task

Purpose: test the desktop path that will matter for product users.

Preflight:

```bash
corepack pnpm --dir apps/desktop install
corepack pnpm --dir apps/desktop build
cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml -q
```

Dev run:

```bash
corepack pnpm --dir apps/desktop dev
```

Manual tasks:

1. Full-agent coding prompt that edits one file and validates.
2. Lightweight side question that should not claim tool use.
3. Long tool output inspection through the desktop viewer.
4. Provider slow/timeout status display during a deliberately slow run.
5. Session reload after completion; completed runs and tool parts should
   reappear from persisted state.
6. Desktop-visible revert of a recent assistant turn.

Pass criteria:

- Desktop bridge remains thin over the shared runtime.
- Full-agent and lightweight lanes are visibly distinct.
- Provider status, usage, permission waits, diagnostics, tool output, and
  closeout state are understandable without reading logs.
- Reload does not lose completed session parts.

### Lane 5: HTTP Full-Agent API

Purpose: prove API clients use the same full-agent runtime instead of provider
chat.

Command:

```bash
bash scripts/api-full-agent-soak.sh
```

Extended checks:

```bash
PRIORITY_AGENT_API_SOAK_QUEUE=1 bash scripts/api-full-agent-soak.sh
```

Manual API checks:

- `POST /api/sessions/:id/prompt` returns `execution_kind:
  "full_agent_turn"` and `agent_runtime_entrypoint: "RuntimeController"`.
- `GET /api/sessions/:id/parts` returns persisted parts after completion.
- `GET /api/sessions/:id/context` returns real context snapshot fields, not
  placeholders.
- `GET /api/sessions/:id/jobs` returns shell `cwd`, `exit_code`, timeout, and
  cancelled state when shell tools run.
- `POST /api/sessions/:id/cancel` changes runtime-visible run status.
- Idempotency key replay is accepted only for same content and conflicts on
  different content.

Pass criteria:

- API route output is stable and contract-testable.
- Provider-chat and full-agent APIs are not confused.
- Queue/admission status is durable enough to inspect after the request.

### Lane 6: Provider / Cost / Cache Soak

Purpose: test whether the product can explain speed and token/cost behavior.

Run the same short coding prompt three times with the same provider/model and
settings:

1. cold session;
2. same static-prefix shape;
3. after a small tool-schema or memory/context change.

Required records:

- prompt tokens;
- completion tokens;
- cached tokens / cache hit;
- stable prefix hash;
- tool schema hash if available;
- model id;
- timeout category;
- total wall time;
- number of tool rounds;
- final closeout status.

Pass criteria:

- `/cost`, usage ledger, API status, or diagnostic export can explain token
  usage.
- Cache miss reasons are visible enough to debug.
- Output caps prevent runaway completion tokens.
- Slow-tail and timeout do not look like silent hangs.

## 5. Recommended Task Set

Run these in order. Stop and fix only release-blocking framework defects; do
not tune product logic for one weak-model mistake.

### Batch A: Must-Pass Core

1. Inspect and explain a module without edits.
2. Single-file bug fix with targeted test.
3. Stale edit recovery.
4. Multi-file Rust refactor.
5. Verification failure repair loop.
6. Permission-denied shell mutation recovery.

Recommended commands:

```bash
bash scripts/product-daily-gate.sh --layer smoke --skip-provider-health
bash scripts/programming-soak-suite.sh single-file-bug-fix failing-test-repair permission-deny-safe-retry
```

### Batch B: Product UX / Runtime Evidence

1. Long shell output paging.
2. Background or slow shell status.
3. Revert last assistant turn.
4. Resume/reload completed session.
5. Usage/cost explanation after a real run.
6. Provider timeout classification.

Recommended commands:

```bash
bash scripts/programming-soak-suite.sh long-shell-output-paging revert-last-assistant-turn provider-slow-tail-classification
bash scripts/api-full-agent-soak.sh
```

### Batch C: Full Release Candidate Soak

1. Product daily gate.
2. Full programming soak suite.
3. One TUI manual real task.
4. One desktop manual real task.
5. One API queued prompt soak.
6. One same-prompt provider/cache repeat test.

Recommended commands:

```bash
bash scripts/product-daily-gate.sh --layer product --skip-provider-health
bash scripts/programming-soak-suite.sh all
PRIORITY_AGENT_API_SOAK_QUEUE=1 bash scripts/api-full-agent-soak.sh
```

## 6. Scoring

Each real task gets a 100-point score:

| Dimension | Points | What To Check |
|---|---:|---|
| Correct task outcome | 25 | final code behavior matches request |
| Tool discipline | 15 | reads before edit, file tools for mutation, bash for validation/status |
| Validation evidence | 15 | required command actually ran and outcome is recorded |
| Repair behavior | 15 | failures re-enter context and bounded repair happens |
| Safety/recovery | 10 | checkpoint/revert/permission behavior intact |
| Context/cost hygiene | 10 | no runaway context, cache/cost usage is explainable |
| UI/API observability | 10 | parts/events/jobs/provider/diagnostics are inspectable |

Release thresholds:

- Core Batch A average >= 85, with no release blockers.
- Product Batch B average >= 80, with all failures classified.
- Full RC soak: no false verified closeout, no unsafe mutation bypass, no data
  loss, no unrecoverable stuck run.

## 7. Failure Classification

Use this before deciding what to fix.

| Failure Owner | Examples | Fix Priority |
|---|---|---|
| `agent_flow` | skipped required validation, false verified closeout, lost tool evidence, unsafe edit path | fix immediately |
| `tool_contract` | file tool stale-state wrong, checkpoint failure not reported, bash output not paged | fix immediately if reproducible |
| `api_ui_contract` | route returns placeholders, desktop cannot reload parts, DTO drift | fix before release |
| `provider_or_environment` | API timeout, rate limit, network error, missing key | classify and retry/compare; do not change core logic blindly |
| `weak_model` | bad reasoning despite correct tool evidence and honest closeout | record; do not weaken runtime |
| `test_harness` | eval script cannot collect artifacts, wrong assertion, fixture drift | fix harness, then rerun |

Release blockers:

- false `verified` closeout;
- workspace mutation without checkpoint where checkpoint should apply;
- permission bypass for dangerous shell mutation;
- session events/parts lost after completed run;
- cancel or timeout leaves UI/API permanently stuck with no diagnostic;
- repeated provider timeout with no visible provider status or cost record.

## 8. Artifact Checklist

For every soak run, keep:

- exact command;
- provider/model/env summary with secrets redacted;
- run id and session id;
- prompt file;
- stdout/stderr;
- event stream or session events;
- session parts;
- diff patch and git status;
- validation command output;
- usage/cost ledger rows or `/cost` output;
- provider status snapshot;
- failure-owner classification;
- final decision: pass, fail, retry, or blocked.

Preferred artifact roots:

- `target/soak-bundles/<run-id>/`
- `target/live-evals/<run-id>/`
- `docs/benchmarks/<promoted-run-id>/` only for important baselines worth
  preserving in docs.

## 9. Suggested Next Run

Run this sequence first:

```bash
bash scripts/daily-baseline.sh
bash scripts/product-daily-gate.sh --layer smoke --skip-provider-health
bash scripts/programming-soak-suite.sh single-file-bug-fix failing-test-repair permission-deny-safe-retry
bash scripts/api-full-agent-soak.sh
```

If that is clean, run:

```bash
bash scripts/product-daily-gate.sh --layer product --skip-provider-health
bash scripts/programming-soak-suite.sh all
PRIORITY_AGENT_API_SOAK_QUEUE=1 bash scripts/api-full-agent-soak.sh
```

Then do one manual TUI task and one manual desktop task. Manual UI work should
focus on whether the product is understandable, not whether the model is
perfect.

## 10. Expected Outcome

After this soak pass, the project should have a concrete release-readiness
answer:

- stable enough for daily dogfood;
- needs targeted runtime/API/UI fixes;
- blocked by provider instability;
- or not ready because core programming flow still produces release blockers.

The bar is not "every model completes every task". The bar is that the agent
framework behaves safely, records evidence, repairs bounded failures, explains
provider/cost problems, and never claims verified success without proof.
