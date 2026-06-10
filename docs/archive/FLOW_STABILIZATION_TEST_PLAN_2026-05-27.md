# Flow Stabilization Test Plan - 2026-05-27

## 1. Position

The next phase should not add more product surface. The goal is to make the
current Priority Agent loop steady:

1. route the task correctly, or recover when the route is wrong;
2. assemble compact, relevant context without turning memory or skills into
   prompt noise;
3. expose the right tools for the current route and risk;
4. turn tool, validation, and permission failures into observations for the
   next model step;
5. run bounded repair instead of looping forever;
6. close out only when proof is real, or report `not_verified`, `partial`,
   `failed`, or `blocked` honestly.

The product bar is not "the LLM never makes a mistake." The bar is:

- process bugs are visible and fixed in runtime/tool/eval code;
- model mistakes are caught by validation, fed back as structured evidence,
  given bounded repair opportunities, and never turned into false success.

## 2. Current Evidence Baseline

Use `docs/PROJECT_STATUS.md` as the canonical baseline, but treat live-eval
suite composition as time-sensitive because tasks keep being added.

Current useful anchors from the repo:

- deterministic local baseline in `docs/PROJECT_STATUS.md`: `1468 passed; 0
  failed`;
- latest recorded real-project coding gauntlet:
  `real-project-coding-20260517-192347`, `15/15` passed, `10` real code-change
  passes, `5` audit/no-diff passes, `failure_owner=none` for every case;
- current `real-project-coding` suite now lists `17` tasks, because
  `memory-failure-lesson-promotion` and
  `memory-stale-project-fact-demotion` are in the suite array after the latest
  recorded `15/15` baseline;
- latest MVA full weighted suite:
  `mva-followup-full-20260525-165257`, `6/7` passed, average
  `agent_score=87.1`;
- the important MVA lesson is that a failed model completion is acceptable only
  when required commands fail, closeout stays honest, and the report preserves
  the evidence instead of claiming success;
- `docs/LLM_FLOW_FAILURE_AUDIT_2026-05-25.md` is the current reference for
  separating flow failures from MiniMax/model variability.

Immediate baseline refresh target:

```bash
RUN_ID=flow-real-refresh-$(date +%Y%m%d-%H%M%S)
bash scripts/run_live_eval.sh \
  --case real-project-coding \
  --mode agent-run \
  --run-tests \
  --run-id "$RUN_ID" \
  --label flow-real-refresh
bash scripts/run_live_eval.sh --mode summary --run-id "$RUN_ID"
```

This refresh should be interpreted as the new stabilization baseline only after
checking the two newer memory cases, not by copying the old `15/15` number.

## 3. Test Projects To Use

### P0: Self-hosted Priority Agent Worktrees

Use the existing `priority-agent` fixture as the main test project. It is the
right default because the codebase is real, the failures are historically
meaningful, and the runner already isolates worktrees.

Primary suites:

- `core-coding-quality`: basic coding-agent behavior;
- `real-project-coding`: real read/edit/test/repair loop on current project
  history;
- `mvp-weighted-agent`: minimum agent loop, scoring, risk, memory boundary, and
  repair behavior;
- `runtime-spine-p0b`: route recovery, permission-required actions,
  validation repair, subagent proof boundaries, memory conflict, and skill
  guidance.

Do not start the stabilization phase with random large open-source repos. They
add setup noise before the local flow is stable.

### P1: Small Contained Product Fixtures

Keep using the small fixtures that live inside the live tasks:

- `backend-todo-api-crud`: backend CRUD implementation and validation;
- `frontend-book-notes-localstorage`: frontend state, persistence, and repair;
- `minimum-agent-verification-repair`: failing test -> observation -> repair ->
  passing test;
- `core-terminal-install-run`: terminal/package/runtime evidence;
- `core-long-output-artifact`: long output artifact discipline.

These are better than toy prompts because they force real tool use and
validation while keeping root cause readable.

### P2: Product Surface And Release Dogfood

Use this lane when changing CLI, desktop, terminal, permissions, or release
behavior:

- `release-dogfood`;
- `desktop-ui-smoke-polish`;
- `project-partner-demo`;
- `scripts/desktop-smoke.sh`, `scripts/desktop-native-smoke.sh`, and desktop
  build/test commands from `docs/PROJECT_STATUS.md` when desktop behavior is
  touched.

This lane should not block every code change. It is for product-surface changes
or release candidates.

### P3: External Baseline Calibration

Use Claude Code and Codex external baseline runs only as calibration, not as a
daily gate. The six scenarios in `docs/EXTERNAL_BASELINE_TEST_PLAN_2026-05-22.md`
are enough:

- `file_edit_rewind`;
- `bash_background_task`;
- `permission_denial_retry`;
- `compaction_boundary`;
- `subagent_worktree_worker`;
- `mcp_auth_repair`.

Do this monthly or before making parity claims. Do not let external baseline
work pull the project back into feature expansion.

## 4. Test Cadence

### Runtime Timeout Policy (Updated)

For stabilization runs, do not use a fixed hard timeout to kill an in-progress
agent task just because it is slow. A slow but advancing run is valid.

Policy:

- no hard stop based only on wall-clock duration;
- keep liveness monitoring always on;
- treat time thresholds as alert signals, not kill signals;
- only perform forced interruption when process death/lock is confirmed
  (`environment`) or when the evaluator explicitly decides to abort.

What counts as progress:

- new model output tokens/chunks;
- new tool events (`tool_execution_start/progress/complete`);
- phase movement in runtime spine (`context -> decision -> tool_execution ->
  verification -> closeout`);
- new validation evidence (command output, diff state, closeout proof status).

If progress is absent for a long window, mark as `stalled` and capture evidence
instead of auto-killing.

### Daily / Before Small Commits

Run only deterministic gates:

```bash
cargo fmt --check
cargo check -q
bash scripts/coding-workflow-gates.sh quick
```

If the change touches shared contracts, broaden to:

```bash
bash scripts/coding-workflow-gates.sh standard
cargo clippy --all-features -- -D warnings
```

### After Runtime, Tool, Validation, Repair, Or Closeout Changes

Run deterministic gates plus one targeted live smoke:

```bash
bash scripts/coding-workflow-gates.sh standard
LIVE_CASE=code-change-verification-repair-loop \
  bash scripts/coding-workflow-gates.sh live-smoke
```

If the change is about file tools, permissions, proof, or repair, prefer one of
these targeted live cases:

- `minimum-agent-verification-repair`;
- `core-simple-stale-edit`;
- `core-permission-rejection-recovery`;
- `live-eval-dashboard-summary`;
- `runtime-spine-p0b-test-failure-repair`;
- `runtime-spine-p0b-permission-required`.

### Weekly Stabilization Run

Run the two suites that best measure the existing loop:

```bash
RUN_ID=flow-mva-$(date +%Y%m%d-%H%M%S)
bash scripts/run_live_eval.sh \
  --case mvp-weighted-agent \
  --mode agent-run \
  --run-tests \
  --run-id "$RUN_ID" \
  --label flow-mva
bash scripts/run_live_eval.sh --mode summary --run-id "$RUN_ID"

RUN_ID=flow-real-$(date +%Y%m%d-%H%M%S)
bash scripts/run_live_eval.sh \
  --case real-project-coding \
  --mode agent-run \
  --run-tests \
  --run-id "$RUN_ID" \
  --label flow-real
bash scripts/run_live_eval.sh --mode summary --run-id "$RUN_ID"
```

Weekly success criteria:

- no false green closeout;
- all failed required commands are visible in reports;
- seeded code-change failures with no diff are classified explicitly;
- any `not_verified` or `failed` result has concrete evidence;
- failures are assigned to `agent_flow`, `llm_reasoning`, `harness`,
  `provider_protocol`, `environment`, or `eval_assertion`, not left ambiguous.

### Release Candidate Run

Before a release candidate:

```bash
bash scripts/coding-workflow-gates.sh full
cargo clippy --all-features -- -D warnings
cargo check --features experimental-api-server -q
bash scripts/workflow-production-gates.sh
```

Then run:

```bash
RUN_ID=flow-release-$(date +%Y%m%d-%H%M%S)
bash scripts/run_live_eval.sh \
  --case release-dogfood \
  --mode agent-run \
  --run-tests \
  --run-id "$RUN_ID" \
  --label flow-release
bash scripts/run_live_eval.sh --mode summary --run-id "$RUN_ID"
```

Only add desktop/native gates if desktop packaging or UI behavior changed.

## 5. Failure Triage Rules

Every failed live eval should be reviewed in this order.

### Step 1: Confirm The Harness

Check:

- did `prepare_commands` create the intended failing or incomplete state;
- is `eval_intent` correct;
- are required commands focused and runnable;
- did provider health fail before the task;
- did the summary parse agent events and trace data;
- is no-diff expected or a seeded-code-change failure.

If the fixture is stale, the assertion is brittle, or the summary parser
misclassifies evidence, mark it `harness` or `eval_assertion` and fix that
before changing runtime behavior.

### Step 1.5: Confirm Runtime Liveness State

Before assigning failure owner, classify run liveness from trace/event evidence:

- `running`: new events with meaningful forward progress;
- `waiting_model`: provider/model turn pending with recent protocol activity;
- `waiting_tool`: long-running command/tool still producing progress;
- `stalled`: no meaningful progress for a long interval;
- `looping`: repeated low-value actions with no new evidence/diff.

Do not classify `stalled`/`looping` as model reasoning failure by default.
First verify whether runtime observation/recovery/stop policy missed the state.

### Step 2: Confirm Runtime Flow

Mark as `agent_flow` when any of these is true:

- required validation failed but closeout claimed success;
- a tool failure was not turned into `ToolObservation` or equivalent evidence;
- the next model turn did not receive the failure evidence;
- route/tool exposure blocked the necessary safe tool with no recovery path;
- permission, checkpoint, or high-risk gate gave contradictory guidance;
- completion proof was missing, contradictory, or ignored;
- a no-diff audit was treated as a code-change pass without evidence;
- the runtime spun without bounded stop or honest blocker.

These are flow bugs and should lead to code, tool contract, parser, or report
changes.

### Step 3: Confirm Model Reasoning Failure

Mark as `llm_reasoning` when the runtime did its job but the model still failed:

- it had the relevant files, failed command output, and recovery hint;
- it chose the wrong edit, stale anchor, or no edit;
- it repeated low-value reads after being shown enough evidence;
- it produced a bad final explanation, but closeout stayed `not_verified` or
  `failed`;
- a stronger provider or rerun succeeds with the same runtime path.

This should not trigger broad prompt rules. The correct response is usually a
more structured repair observation, a narrower tool recovery hint, or no code
change if the failure is isolated and honestly reported.

### Step 4: Confirm Provider Or Environment Failure

Mark as `provider_protocol`, `provider_health`, or `environment` when:

- tool-result ordering or provider request conversion fails;
- provider health preflight fails;
- credentials, network, shell, dependency, or platform setup stops the run;
- the agent never receives a usable model turn.

Do not mix these with product behavior failures.

### Step 5: Timeout/Long-Run Classification

When a run is very long, classify by liveness evidence rather than elapsed time:

- long but progressing run: keep as normal execution (no failure by duration);
- no-progress run with missing/weak monitoring transition: `agent_flow`;
- provider call hangs or transport ordering failures: `provider_protocol`;
- shell/dependency/dead process blockers: `environment`;
- explicit evaluator abort without runtime fault: `harness` (manual abort).

Add explicit labels in run notes when applicable:

- `stalled_no_progress`
- `looping_no_new_evidence`
- `long_run_with_progress`

## 6. What "LLM Auto-correct" Should Mean

Auto-correction should be a bounded runtime loop, not magical model perfection.

Required loop:

```text
tool or validation failure
-> structured observation with command, error, source context, and next attention
-> next model turn sees the observation
-> model attempts a scoped repair
-> required validation reruns
-> closeout only if proof passes
```

Good auto-correct evidence:

- `recovered_failed` validation appears before the final pass;
- changed files stay within diff constraints;
- first repair is based on the failing command or diagnostic, not random
  rewriting;
- final closeout names both the initial failure and the passing proof.

Bad auto-correct evidence:

- the model edits without reading the relevant file;
- repair ignores the failing command;
- validation is skipped after repair;
- closeout claims success from intent or confidence;
- the runtime keeps trying after repeated no-progress signals.

The correct target is not `100% pass rate`. The correct target is:

- high pass rate on current suites;
- no false success;
- short, evidence-backed repair when the model can recover;
- clear failure owner when it cannot.

## 7. Stabilization Scorecard

Track these metrics per weekly run:

| Metric | Target |
| --- | --- |
| `real-project-coding` pass rate | trend toward current-suite green, starting from fresh 17-case baseline |
| `mvp-weighted-agent` pass rate | at least `6/7`, with any failure honestly closed |
| False green closeout | `0` |
| Required command evidence missing | `0` for scored tasks |
| Seeded no-diff ambiguous failures | `0` |
| `agent_flow` failures | must produce a follow-up fix or explicit accepted risk |
| `llm_reasoning` failures | must show evidence was available and closeout was honest |
| Repeated low-value actions | trend down; investigate when repeated across two runs |
| Prompt/tool policy contradictions | `0` |
| Long-running tasks killed by wall-clock only | `0` |
| `stalled` runs with missing evidence snapshot | `0` |
| `looping` runs without explicit classification | `0` |
| Behavior assertion coverage | increase only for existing flows; do not expand product scope |

Keep the scorecard in the generated `summary.md` first. Only refresh
`docs/PROJECT_STATUS.md` after a meaningful baseline change.

## 8. Execution Order

### Phase 0: Freeze The Real Baseline

1. Run deterministic daily gate.
2. Run current `mvp-weighted-agent`.
3. Run current `real-project-coding`.
4. Compare summaries with:
   - `docs/benchmarks/live-mva-followup-full-20260525-165257/summary.md`;
   - `docs/benchmarks/live-real-project-coding-20260517-192347/summary.md`.
5. Record only confirmed current numbers.

### Phase 1: Triage Failures Without Adding Features

For each failed case:

1. classify as `harness`, `eval_assertion`, `agent_flow`, `llm_reasoning`,
   `provider_protocol`, or `environment`;
2. if `agent_flow`, patch the narrow runtime/tool/report defect and add or
   update deterministic coverage;
3. if `llm_reasoning`, inspect whether structured repair feedback was adequate;
4. rerun only the failed case until the classification is proven;
5. rerun the suite after all flow fixes in that lane are closed.

### Phase 2: Tighten Existing Coverage

Only add new eval assertions when they cover existing product promises:

- honest closeout;
- required validation proof;
- route/tool exposure recovery;
- memory conflict demotion;
- skill guidance as background context;
- permission denial recovery;
- subagent result is not parent proof.

Do not add new product features as part of this phase.

### Phase 3: Release Confidence

When weekly runs are boring for two cycles:

1. run `release-dogfood`;
2. run desktop/native smoke only if relevant;
3. capture external baseline calibration if a parity claim is planned;
4. update `docs/PROJECT_STATUS.md` with the new validated baseline.

## 9. Decision Rules

Use these rules when deciding what to fix next:

- fix `agent_flow` before chasing pass rate;
- fix false green before fixing false negative;
- fix harness/eval assertions before changing product code;
- do not weaken validation, permissions, checkpoints, or high-risk gates to
  make MiniMax pass;
- do not add always-on prompt text for one-off model mistakes;
- prefer tool contracts, structured observations, targeted repair context, and
  report classification;
- if a failure cannot be classified in 15 minutes, preserve the run artifacts
  and write down the missing evidence field.
