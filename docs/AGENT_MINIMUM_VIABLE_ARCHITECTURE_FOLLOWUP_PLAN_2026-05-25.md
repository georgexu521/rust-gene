# Agent Minimum Viable Architecture Follow-up Plan - 2026-05-25

Source note: `/Users/georgexu/Downloads/11-agent_minimum_viable_architecture_notes.md`

Related implementation record:
`docs/AGENT_MINIMUM_VIABLE_ARCHITECTURE_ALIGNMENT_PLAN_2026-05-25.md`

## 1. What This Note Is

The note is a minimum viable Agent architecture checklist. Its core claim is
that a useful first-version Agent is not defined by having many advanced
features. It is defined by a stable closed loop:

```text
task route
-> state
-> context
-> candidate action
-> scoring
-> permission
-> tool execution
-> observation
-> state update
-> stop or continue
-> final response
```

For this project, the note is useful as an audit lens, not as a rewrite target.
Priority Agent already has richer versions of most modules: routing, task
state, context zones, action scoring, permission review, tool execution,
semantic observations, stop checks, memory, recovery, and completion proof.
The next useful work is to make the MVA contract provable end to end in live
evals and to tighten the few places where the first-version contract is still
only optional, implicit, or partially measured.

## 2. Current Project Fit

The current repo already has a first implementation pass for the note:

- `IntentRouter`, `TaskModeScore`, and `LightweightPlan` cover routing and
  direct/light/full/high-risk mode scoring.
- `AgentTaskState` records goal, mode, stage, observations, findings,
  hypotheses, verification, stop checks, action-score history, and stage
  transitions.
- Request preparation now records `ContextZonesMaterialized` and separates
  task state, relevant material, recent observations, and the current user
  request.
- `ActionDecision`, `ActionReview`, and `CandidateAction` provide runtime-owned
  scoring and action-boundary review.
- `ToolResultNormalizer`, `ContextLedger`, and `ToolObservationRecorded` turn
  raw tool results into semantic observations.
- `StopChecker` records terminal decisions and progress/no-progress reasons.
- `CompletionContractEvaluated` makes closeout mode-aware instead of relying
  only on model wording.
- `PRIORITY_AGENT_MVA_AUDIT_TOOLS=1` can narrow the tool surface for audit.
- `evalsets/live_tasks/minimum-agent-*.yaml` now defines a small MVA scenario
  set.

That means the immediate gap is not "add the missing modules." The gap is:
prove that these modules work together under the MVA contract, reduce optional
or implicit behavior, and add sharper regression gates around the loop.

## 3. Note-by-Note Comparison

| Note area | Current project state | Alignment | Remaining gap |
| --- | --- | --- | --- |
| Do not build a super Agent first | Product direction explicitly favors narrow, deep, personal, verifiable behavior. MVA work was added as a thin layer over the existing runtime spine. | Strong | Normal product surface still exposes many advanced tools; MVA audit is opt-in and not automatically tied to the MVA eval set. |
| Eight core modules | All eight exist in richer form: router, state, context, planner/scorer, permission, executor, observer, stop. | Strong | The modules are spread across many controllers; live reports need a single "MVA loop health" view instead of requiring trace expertise. |
| Task Router | `IntentRouter` and `TaskModeScore` classify workflow, retrieval, risk, reasoning, and direct/light/full/high-risk task mode. | Strong | The examples from the note are not locked as a small routing-matrix regression. |
| State Manager | `AgentTaskState` is richer than the note's JSON sketch and now records stage transitions. | Strong | There is no compact `MvaStateSnapshot` for eval/debug output that shows only the first-version fields. |
| Context Builder | Five named zones exist and `ContextZonesMaterialized` is recorded. | Medium-strong | Zone materialization is still partly inferred from request messages and tags. Per-zone budgets, overflow reasons, and zone-level eviction are not enforced as a first-class request object. |
| LLM Planner | The model can call tools directly. Candidate-action JSON is parsed and ranked only when `PRIORITY_AGENT_CANDIDATE_ACTIONS=shadow|gated`. | Medium | First-version candidate action planning is not default. Model-provided candidate scores are accepted structurally but not yet calibrated or compared against runtime scores in a useful report. |
| Weight Scorer | `ActionDecision` records value, risk, uncertainty reduction, cost, reversibility, scope fit, and action score. | Strong | Candidate-action ranking is still mostly a shadow/stuck-state feature; MVA evals should prove it can reduce repeated low-value actions without changing normal turns. |
| Permission Checker | `ActionReview` combines tool contract, side effects, permission, scope, budget, and checkpoint review. | Strong | High-risk "ask user / block / partial completion" semantics need stronger live-eval proof and exact completion-status assertions. |
| Tool Executor | Tool execution has resource policy, action review, observation metadata, and parallel read-only support. | Strong for product, medium for MVA | The note recommends one action per round and a tiny tool set. Current resource policy can allow multi-call rounds, and the MVA audit profile still permits broader tools such as `bash` and `file_patch`. |
| Observer + Stop Checker | Semantic tool observations and `StopChecker` are implemented and traced. | Strong | Observer output is still distributed across normalizer, ledger, task state, and trace. Reports should expose one compact observer outcome per tool round. |
| Simple first-version Memory | Current memory is much richer, with trust, stale demotion, provenance, and lifecycle. | Strong but more complex | The note's minimal contract, "read relevant preference at start; write useful summary at end," is not isolated as an MVA memory boundary eval. |
| Four stage model | Current stages are `Understand`, `Plan`, `Edit`, `Validate`, `Repair`, `Closeout`, `Done`. | Stronger than note | Debug/eval output should map these to the note's `diagnosis`, `implementation`, `verification`, `finalization` vocabulary so the first-version loop is easy to audit. |
| Stage transitions | Stage transition history exists with source, reason, and evidence counts. | Medium-strong | Transition rules are still distributed in state updates and stop handling. A small transition policy table would make allowed stage moves and fallback moves easier to test. |
| LLM output format | Candidate-action schema exists. | Medium | The runtime does not ask for or require at most three candidates in normal turns. This is good for prompt diet, but MVA shadow runs need a controlled request mode. |
| Execution strategy | Checkpoints, stop checks, validation proof, permissions, and budgets exist. | Strong | There is no strict MVA execution profile that enforces one scheduled action, max 10 steps, tiny tool surface, and candidate shadow telemetry together. |
| Completion conditions | `CompletionContractEvaluated` makes direct/light/full/high-risk status explicit. | Medium-strong | The MVA live tasks should assert exact contract status and proof status, not just presence of the event. |
| File structure | Project already has Rust module boundaries for all roles. | Strong | No new file-structure split is needed. Avoid creating a parallel `agent_loop` subsystem. |
| What first version validates | Trace and eval assertions now check many loop signals. | Medium | The minimum-agent eval set has been defined and parse/list checked, but it has not yet been baselined through real agent-run/collect reports in this review. |

## 4. Main Remaining Problems

### P0. The MVA suite is defined, but not yet baselined as a real live-eval gate

Six `minimum-agent-*` tasks exist, and the parser knows the new assertion
aliases. That is necessary but not sufficient. The project still needs a real
baseline run that shows which tasks pass, which fail, and whether failures are
product bugs, prompt/model behavior, or harness assertions.

Without this, MVA can regress silently because the code only proves unit-level
events, not the complete agent loop under live conditions.

### P0. MVA audit mode is manual instead of attached to MVA evals

`PRIORITY_AGENT_MVA_AUDIT_TOOLS=1` exists, but the live-eval runner does not
automatically enable it for `minimum-agent-*` cases. This means those evals can
accidentally run against the broader product tool surface and fail to test the
first-version architecture the note describes.

### P1. Candidate-action planning is still optional and weakly measured

This is intentionally conservative, but it leaves a gap against the note's
"LLM proposes candidates, scorer selects one" loop. Today:

- normal turns use provider tool calls directly;
- candidate mode is off by default;
- shadow/gated mode can rank candidates, but it is not part of the MVA suite
  by default;
- model-provided candidate scores are not yet surfaced as a calibration report
  against runtime scores.

The right next step is not to force JSON on every turn. The right next step is
to add MVA shadow-mode proof and score-delta diagnostics.

### P1. Context zones are observable, but not yet the single request assembly contract

`ContextZonesMaterialized` records token counts from request messages. This is
good, but the actual request path still treats zones partly as tagged blocks
inside messages. The first-version architecture would be easier to reason about
if the runtime produced a `ContextAssemblyPlan` and then rendered messages from
that plan.

### P1. The "one action per round / tiny tool set" first-version profile is not exact

The product has good reasons to support parallel read-only tools and broader
tools. The MVA audit profile should be stricter than the normal product
profile:

- one scheduled action per loop step unless explicitly marked read-only batch;
- max 10 loop steps for MVA tasks;
- tiny default tools: list/search/read/edit/run_tests/git_diff/final/ask_user;
- opt-in escape hatch for `bash` only when the eval declares it.

### P1. Completion contract needs exact-status assertions

The parser can see `CompletionContractEvaluated`, but the MVA tasks should
assert whether the expected status is `completed`, `partial`, `blocked`, or
`failed`. This matters most for high-risk and cannot-verify cases, where a
final answer can sound confident even when runtime proof is thin.

### P2. Observer output is strong internally but not yet compact in reports

Tool observations exist and update state, but reports still require stitching
together `ToolObservationRecorded`, context ledger entries, state deltas, and
stop checks. Add one compact "observer outcome" summary to the runtime report:
status, finding count, evidence count, uncertainty reduced, state updates,
recommended next action.

### P2. Minimal memory boundary is not separately proven

The memory system is now richer than the note recommends. That is fine, but
there should be a small MVA memory eval that proves only the boundary:

1. read a relevant project/user preference before planning;
2. do not inject stale or conflicting memory as trusted context;
3. write a concise, evidence-backed task summary only after closeout.

## 5. Recommended Implementation Plan

### Phase 0 - Baseline the current MVA suite

Purpose: turn the new YAML tasks into real evidence before adding more code.

Tasks:

1. Run the six `minimum-agent-*` tasks in `agent-run` or `collect` mode with
   current defaults.
2. Save the summary output under the normal live-eval report directory.
3. Classify each failure as one of:
   - product runtime gap;
   - model behavior gap;
   - harness/assertion gap;
   - fixture or acceptance-command bug.
4. Add a short baseline section to this document after the run.

Validation:

```bash
bash scripts/run_live_eval.sh --case minimum-agent-direct-answer --mode agent-run
bash scripts/run_live_eval.sh --case minimum-agent-light-inspection --mode agent-run
bash scripts/run_live_eval.sh --case minimum-agent-loop --mode agent-run
bash scripts/run_live_eval.sh --case minimum-agent-verification-repair --mode agent-run
bash scripts/run_live_eval.sh --case minimum-agent-high-risk-block --mode agent-run
bash scripts/run_live_eval.sh --case minimum-agent-low-value-replan --mode agent-run
bash scripts/run_live_eval.sh --mode summary --run-id <run-id>
```

If provider credentials are unavailable, run `--mode prepare` and `--list`
only, then explicitly mark live evidence as blocked by environment.

### Phase 1 - Attach MVA runtime profile to the MVA eval set

Purpose: make the evals test the first-version Agent surface by default.

Tasks:

1. Add a sample-level profile field, for example:

   ```yaml
   runtime_profile: minimum_viable_agent
   ```

2. Teach `scripts/run_live_eval.sh` to set profile env vars for that profile:
   - `PRIORITY_AGENT_MVA_AUDIT_TOOLS=1`
   - `PRIORITY_AGENT_CANDIDATE_ACTIONS=shadow`
   - a new MVA resource cap env if needed, such as
     `PRIORITY_AGENT_MVA_MAX_TOOL_CALLS=10`
3. Keep normal product evals unchanged.
4. Add parser/report fields that show whether the MVA profile was active.

Likely files:

- `scripts/run_live_eval.sh`
- `scripts/live_eval_report_parser.py`
- `scripts/live-eval-aggregate-summary.sh`
- `evalsets/live_tasks/minimum-agent-*.yaml`

Validation:

```bash
bash -n scripts/run_live_eval.sh scripts/live-eval-aggregate-summary.sh
python3 -m py_compile scripts/live_eval_report_parser.py
bash scripts/run_live_eval.sh --case minimum-agent-loop --mode prepare
```

### Phase 2 - Add candidate-action shadow calibration

Purpose: prove the note's planner/scorer loop without forcing JSON on normal
turns.

Tasks:

1. In shadow mode, record both:
   - model candidate scores when provided;
   - runtime scores from `ActionDecision`.
2. Add score-delta fields to `CandidateActionsEvaluated`:
   - selected model score;
   - selected runtime score;
   - disagreement reason;
   - whether runtime selection differs from model order.
3. Add a small candidate-action request hint only for MVA profile runs.
4. Keep `PRIORITY_AGENT_CANDIDATE_ACTIONS=off` as the normal default.

Likely files:

- `src/engine/candidate_action.rs`
- `src/engine/conversation_loop/turn_model_step_controller.rs`
- `src/engine/trace.rs`
- `scripts/live_eval_report_parser.py`

Validation:

```bash
cargo test -q candidate_action -- --test-threads=1
cargo test -q turn_model_step_controller -- --test-threads=1
cargo test -q trace -- --test-threads=1
```

### Phase 3 - Make context zones the actual request assembly object

Purpose: make the five zones first-class instead of mostly inferred from
rendered messages.

Tasks:

1. Route request preparation through `ContextAssemblyPlan` before rendering
   provider messages.
2. Add per-zone budget and overflow reason fields.
3. Record zone fingerprints in `ContextZonesMaterialized`.
4. Add an eval assertion for non-empty task state and current decision request.

Likely files:

- `src/engine/context_assembly.rs`
- `src/engine/conversation_loop/request_preparation_controller.rs`
- `src/engine/context_budget_controller.rs`
- `src/engine/trace.rs`

Validation:

```bash
cargo test -q context_assembly -- --test-threads=1
cargo test -q request_preparation_controller -- --test-threads=1
cargo test -q context_budget_controller -- --test-threads=1
```

### Phase 4 - Add a compact MVA state snapshot and stage transition policy

Purpose: make state and stage transitions easy to audit without reading the
full task state.

Tasks:

1. Add `MvaStateSnapshot` derived from `AgentTaskState`:
   goal, mode, MVA-stage label, internal stage, recent step, recent
   observation, relevant files, failure count, max tool calls, done status.
2. Map internal stages to note stages:
   - `Understand` / `Plan` -> `diagnosis`
   - `Edit` / `Repair` -> `implementation`
   - `Validate` -> `verification`
   - `Closeout` / `Done` -> `finalization`
3. Add a small transition policy table for expected moves and repair fallback.
4. Record transition policy verdict in trace when a stage changes.

Likely files:

- `src/engine/task_context.rs`
- `src/engine/stop_checker.rs`
- `src/engine/trace.rs`

Validation:

```bash
cargo test -q task_context -- --test-threads=1
cargo test -q stop_checker -- --test-threads=1
cargo test -q runtime_spine_behavior_tests -- --test-threads=1
```

### Phase 5 - Enforce exact MVA completion assertions

Purpose: prevent final-answer wording from hiding incomplete, blocked, or
unverified work.

Tasks:

1. Extend `runtime_spine_assertions` with:
   - `completion_status`
   - `terminal_status`
   - `verification_proof_status`
2. Update all `minimum-agent-*` tasks with expected values.
3. Make high-risk blocked cases assert a blocked/needs-user/partial outcome,
   not just event presence.

Likely files:

- `scripts/live_eval_report_parser.py`
- `evalsets/live_tasks/minimum-agent-*.yaml`
- `src/engine/conversation_loop/turn_completion_controller.rs` only if the
  existing contract cannot express a needed status.

Validation:

```bash
python3 -m py_compile scripts/live_eval_report_parser.py
bash scripts/run_live_eval.sh --case minimum-agent-high-risk-block --mode prepare
```

### Phase 6 - Add compact observer and memory boundary reporting

Purpose: make Observer and minimal Memory behavior visible as first-version
contracts.

Tasks:

1. Add a compact observer outcome summary to runtime reports:
   status, findings, evidence count, uncertainty reduced, state updates,
   recommended next action.
2. Add an optional `MemoryBoundaryEvaluated` trace event for MVA profile runs:
   memory read status, stale/conflict demotion status, closeout write
   candidate status.
3. Add one `minimum-agent-memory-boundary.yaml` eval once the report fields are
   available.

Likely files:

- `src/engine/conversation_loop/tool_execution_controller.rs`
- `src/engine/context_ledger.rs`
- `src/engine/task_context.rs`
- `src/memory/manager.rs`
- `scripts/live_eval_report_parser.py`
- `evalsets/live_tasks/minimum-agent-memory-boundary.yaml`

Validation:

```bash
cargo test -q context_ledger -- --test-threads=1
cargo test -q task_context -- --test-threads=1
cargo test -q memory -- --test-threads=1
python3 -m py_compile scripts/live_eval_report_parser.py
```

### Phase 7 - Refresh durable status and run gates

Purpose: keep docs tied to validated behavior.

Tasks:

1. Add implementation status to this file after Phases 0-6 are done.
2. Update `docs/PROJECT_STATUS.md` only with validated baseline facts.
3. Keep the old alignment plan as the first-pass implementation record.
4. Do not create a parallel agent-loop subsystem.

Validation:

```bash
cargo fmt --check
cargo check -q
cargo test -q candidate_action -- --test-threads=1
cargo test -q request_preparation_controller -- --test-threads=1
cargo test -q task_context -- --test-threads=1
cargo test -q stop_checker -- --test-threads=1
cargo test -q runtime_spine_behavior_tests -- --test-threads=1
python3 -m py_compile scripts/live_eval_report_parser.py
bash -n scripts/run_live_eval.sh scripts/live-eval-aggregate-summary.sh
git diff --check
```

## 6. Acceptance Criteria

This follow-up plan is complete when:

- The `minimum-agent-*` suite has a real current baseline report.
- MVA evals automatically activate the MVA runtime profile.
- Reports show whether MVA profile, context zones, candidate ranking, action
  scoring, permission review, observer output, stop check, and completion
  contract were present.
- Candidate-action shadow mode records model-score versus runtime-score
  calibration without changing normal turns.
- Context zones are assembled from a first-class plan and have budget/overflow
  reporting.
- Stage transitions can be reviewed through a compact MVA stage label and a
  transition-policy verdict.
- Completion assertions can require exact status/proof values for high-risk,
  unverified, and direct-answer tasks.
- Minimal memory read/write boundary behavior is covered by at least one MVA
  eval.

## 7. Non-Goals

- Do not replace the existing conversation loop.
- Do not force candidate JSON on every normal product turn.
- Do not remove advanced tools from normal Priority Agent.
- Do not simplify memory by deleting the current trust/provenance/lifecycle
  work.
- Do not add heavy always-on prompt instructions. Prefer runtime profile,
  trace, and eval contracts.

## 8. Suggested Priority

1. Phase 0: baseline the MVA eval suite.
2. Phase 1: attach MVA runtime profile to those evals.
3. Phase 5: exact completion assertions, because this catches false-positive
   success.
4. Phase 2: candidate-action shadow calibration.
5. Phase 3: first-class context assembly plan.
6. Phase 4: compact MVA state snapshot and transition policy.
7. Phase 6: observer and memory boundary reporting.
8. Phase 7: docs/status refresh after evidence exists.

Reason: the fastest way to keep momentum is to get live evidence first. Once
the MVA evals are truly running under the intended profile, the remaining
runtime hardening can be prioritized by observed failures instead of theory.

## 9. Implementation Status - 2026-05-25

Status: implemented.

The follow-up plan was implemented as a thin MVA contract over the existing
runtime spine, not as a parallel agent loop.

Completed implementation:

- Phase 0: baselined the `minimum-agent-*` live tasks with real `agent-run`
  evidence.
- Phase 1: added `runtime_profile: minimum_viable_agent` handling in
  `scripts/run_live_eval.sh`, including MVA audit tools, candidate-action
  shadow mode, max tool-call cap, and single-action parallelism cap.
- Phase 2: added candidate-action shadow calibration fields for selected
  model score, selected runtime score, runtime/model score delta,
  disagreement, and calibration reason.
- Phase 3: added `ContextAssemblyPlan` and per-zone budget, overflow,
  fingerprint, token, and empty-state reporting for request assembly.
- Phase 4: added `MvaStateSnapshot` and MVA stage labels/transition policy
  diagnostics for the note's diagnosis/implementation/verification/finalization
  vocabulary.
- Phase 5: extended runtime-spine assertions with exact completion,
  terminal, and verification-proof status checks; MVA tasks now carry expected
  status values where needed.
- Phase 6: added compact observer/memory-boundary reporting, including
  `MemoryBoundaryEvaluated` and a dedicated
  `minimum-agent-memory-boundary.yaml` eval.
- Phase 7: refreshed durable status docs and kept the implementation inside
  existing runtime modules.

Additional hardening found during implementation:

- Direct/light MVA turns now get deterministic structured closeout when there
  is runtime evidence but no code workflow closeout.
- Programming and required-validation tasks no longer terminate early because
  of duplicate read-only closeout before the required edit/validation path has
  a chance to run.
- Live-eval target worktrees under `target/live-evals/*/worktree` are treated
  as workspace paths for action policy and high-risk file mutation review.
- Required validation commands can run during the understand stage when an eval
  declares required validation.
- Python fixture test scripts are classified as validation commands.
- Configuration/direct read-only tasks now keep `glob`/`grep` visible so
  local evidence searches do not fall back to broad shell or unexposed tools.

Current MVA live-eval baseline:

| Task | Latest run id | Status | Key proof |
| --- | --- | --- | --- |
| `minimum-agent-direct-answer` | `live-eval-20260525-143650` | ok | direct answer completed with no tool requirement |
| `minimum-agent-light-inspection` | `live-eval-20260525-143713` | ok | read-only audit completed with closeout |
| `minimum-agent-loop` | `live-eval-20260525-143452` | ok | code diff, required command ok, closeout passed |
| `minimum-agent-verification-repair` | `live-eval-20260525-142853` | ok | code diff, required commands ok, closeout passed |
| `minimum-agent-high-risk-block` | `live-eval-20260525-143531` | ok | destructive task blocked with runtime-spine status passed |
| `minimum-agent-low-value-replan` | `live-eval-20260525-145541` | ok | repeated read-only path stopped, runtime spine passed |
| `minimum-agent-memory-boundary` | `live-eval-20260525-145541` | ok | memory boundary recorded, closeout passed |

Latest validation run during implementation:

```bash
cargo fmt --check
cargo check -q
cargo test -q
cargo clippy --all-features -- -D warnings
cargo test -q route_scoped_tools -- --test-threads=1
cargo test -q closeout_controller -- --test-threads=1
cargo test -q duplicate_read_only -- --test-threads=1
python3 -m py_compile scripts/live_eval_report_parser.py
bash -n scripts/run_live_eval.sh scripts/live-eval-aggregate-summary.sh
git diff --check
bash scripts/run_live_eval.sh --case minimum-agent-low-value-replan --mode agent-run --overlay-working-tree
bash scripts/run_live_eval.sh --case minimum-agent-memory-boundary --mode agent-run --overlay-working-tree
```

Remaining watch item:

- The low-value read-only eval now passes and proves the runtime stop/closeout
  path, but its human-facing answer quality is still model-dependent. If this
  case becomes an important release gate, add a sample-level output assertion
  for the missing target token so the harness checks answer content, not only
  runtime-spine completion.
