# Agent Weight System Alignment Plan

Date: 2026-05-25

Source note: `/Users/georgexu/Downloads/10-agent_weight_system_notes.md`

Repo scope: `/Users/georgexu/Desktop/rust-agent`, current branch `claude`.

Related active plans:

- `docs/AGENT_OBSERVER_ALIGNMENT_PLAN_2026-05-24.md`
- `docs/AGENT_STOP_FAILURE_RECOVERY_ALIGNMENT_PLAN_2026-05-24.md`
- `docs/AGENT_MEMORY_SYSTEM_ALIGNMENT_PLAN_2026-05-25.md`
- `docs/WORKFLOW_CONTRACT_TARGETING_PLAN_2026-05-18.md`
- `docs/RISK_SIGNAL_CONTROLLER_PLAN_2026-05-18.md`
- `docs/PERSONAL_AGENT_PRODUCT_PRINCIPLES_2026-05-18.md`

## 0. Conclusion

Priority Agent already has several real "weight" surfaces:

- action-level scoring for tool calls;
- workflow-plan step weights;
- risk signals and permission gates;
- learning and memory adjustments to plan weights;
- Observer signals about uncertainty reduction;
- StopChecker counters for no progress and repeated failure;
- live-eval reporting for weighted planning.

So the project is not missing a weight system from scratch. The current problem
is fragmentation: scoring exists in several places, but there is not yet one
canonical next-action value contract that can answer:

```text
Given the current stage, evidence, memory, risk, cost, and user goal,
which next action is most worth taking now?
```

The note's strongest idea is:

```text
LLM understands and proposes; deterministic runtime calibrates, ranks, gates,
and records.
```

That fits this project well, but it should be adapted to Priority Agent's own
product direction: narrow, deep, personal, and verifiable. The weight system
should not become a generic workflow bureaucracy. It should become the local
runtime's "action economics" layer: a small, traceable system that uses gex's
workspace, memories, project status, validation habits, and past failures to
make the next tool/action safer and more useful.

Recommended next slice:

1. Unify the existing action score vocabulary.
2. Add a final calibrated `action_score` and missing `scope_fit`.
3. Run it first in shadow/audit mode for actual tool calls.
4. Feed Observer and Memory signals into bounded score modifiers.
5. Only then add gated LLM candidate-action proposals for the hard cases where
   the runtime has evidence that the model is stuck or about to choose a risky
   low-value action.

This avoids over-control while still giving the project its own distinctive
advantage: the runtime can learn what is worth doing on gex's machine.

## 0.1 Implementation Status

2026-05-25 implementation batch completed the planned weight-system alignment
slice across all eight phases, with one deliberate product decision:
candidate-action JSON is available as a gated runtime contract, but it is not
forced on ordinary turns. The default remains off, because this project should
preserve normal model reasoning and activate candidate ranking only when a
turn is stuck, repeatedly revised, or high-risk.

Implemented:

- Added canonical action scoring fields to `ActionDecision`:
  `scope_fit`, final `action_score`, formula stage, formula version, and
  per-source modifiers.
- Added deterministic stage-specific scoring formulas for diagnosis, planning,
  implementation, verification, recovery, and closeout.
- Kept existing value/risk/uncertainty/cost/reversibility metadata stable, and
  made new serialized fields backward-compatible with older traces/metadata.
- Extended `ActionDecisionEvaluated` trace events with score, scope fit,
  formula, alignment, mutation, broad-shell, and modifier data.
- Made `ActionReview` score-aware with explicit reasons for low scope fit, low
  action value, high-cost low-value, high-risk low-value, and repeated low-score
  actions, while preserving permission/checkpoint/destructive-scope precedence.
- Added score-aware model recovery and a `candidate_action_request` debug
  payload for revised score failures.
- Added bounded Observer modifiers for uncertainty not reduced, candidate focus
  matches, validation failures, validation success, active risks, key findings,
  and progress-to-validation transitions.
- Upgraded memory action modifiers to use retained memory trust/conflict/stale
  signals and record typed modifier kinds such as failure risk, success value,
  stale penalty, conflict uncertainty, and project fit.
- Added a gated candidate-action parser/ranker and
  `PRIORITY_AGENT_CANDIDATE_ACTIONS=off|shadow|gated` mode handling.
- Added a `CandidateActionsEvaluated` trace event type for future/flagged
  candidate ranking runs.
- Added action-score extraction into `ContextLedger` and action-score history
  into `AgentTaskState`.
- Fed action-score history summaries into `StopChecker`, including low-score
  loops, score-without-uncertainty-reduction loops, and repeated action
  revisions.
- Extended task context rendering with recent action-score evidence.
- Extended live-eval parser/reporting with action-scoring fields and behavior
  assertions: action score recorded, scope fit recorded, early edit demoted,
  observer modifier applied, memory modifier applied, low-score replan, and
  candidate ranking used.

Validated:

```bash
cargo check -q
cargo test -q action_decision -- --test-threads=1
cargo test -q action_review -- --test-threads=1
cargo test -q tool_execution_controller -- --test-threads=1
cargo test -q stop_checker -- --test-threads=1
cargo test -q candidate_action -- --test-threads=1
cargo test -q context_ledger -- --test-threads=1
cargo test -q task_context -- --test-threads=1
cargo test -q runtime_spine_behavior_tests -- --test-threads=1
cargo test -q memory -- --test-threads=1
cargo test -q retrieval_context -- --test-threads=1
python3 -m py_compile scripts/live_eval_report_parser.py
bash -n scripts/run_live_eval.sh scripts/live-eval-aggregate-summary.sh
cargo fmt --check
cargo clippy --all-features -- -D warnings
cargo check --features experimental-api-server -q
cargo test -q -- --test-threads=1
```

Note: one parallel `cargo test -q` run hit a transient
`test_file_edit_normalize_whitespace` `/tmp` checkpoint creation failure. The
same test passed immediately when rerun directly, and the full suite passed
with `--test-threads=1`.

## 1. Note Requirements Compared To Current Runtime

| Note requirement | Current project state | Alignment | Main gap |
| --- | --- | --- | --- |
| Use stable weight dimensions: value, risk, uncertainty reduction, cost, reversibility, scope fit | `ActionScores` has value, risk, uncertainty_reduction, cost, and reversibility. | Strong partial | Missing `scope_fit`; no final computed `action_score`; dimensions are not shared with workflow step weighting. |
| Keep LLM responsible for understanding and initial judgment | Workflow contract asks the model for task judgment and weighted plan steps. Tool actions still come from the model. | Strong | Good foundation; keep this model-led posture. |
| Let runtime calibrate and rank actions | Runtime reviews actual tool calls through `ActionDecision` and `ActionReview`. | Partial | It scores only the action already chosen by the model; it does not rank candidate alternatives. |
| Stage-specific formulas | `phase_allows_action` and workflow stages exist; `workflow_contract` computes plan weights separately. | Partial | No explicit formula per action stage such as diagnosis/implementation/verification/closeout. |
| Reject or revise low-value, high-risk, wrong-scope actions | `ActionReview` rejects premature mutation and enforces tool availability, exposure, permission, scope, budget, and checkpoint constraints. | Strong partial | The low-value rule is narrow; scope fit is not represented as a first-class score. |
| Risk above threshold should ask user | Permission and action review can ask user for risky/open-world/destructive actions. | Strong partial | The ask-user rule is permission-centric, not score-threshold-centric. |
| Low scope fit should revise or reject | Destructive scope and exposed-tool checks exist. | Partial | Scope is a gate, not a graded action-quality dimension. |
| High cost and low value should reject | Resource policy limits tool budgets; `ActionWorthVerdict.low_value` exists. | Weak partial | There is no direct `cost high + value low` action-review rule. |
| Diagnosis should prioritize uncertainty reduction | Inspection actions score high uncertainty reduction; premature edit is revised. | Partial | Repeated inspection is stopped later by counters, not dynamically demoted by action score history. |
| Observer should change the next round's weights | `ToolObservation` records `reduced_uncertainty`, key findings, candidate focus, and risk notes; task state tracks uncertainty-not-reduced steps. | Partial | These signals influence StopChecker, but not the next `ActionDecision` score directly. |
| Memory should bias action weights | Memory-to-planning and memory-to-action modifiers exist after the memory-system slice. | Partial | The action modifier is keyword-based and shallow; it does not use typed strategy/failure metadata deeply yet. |
| StopChecker should use repeated low scores | StopChecker uses no-progress, duplicate read-only, uncertainty-not-reduced, and failure counters. | Partial | It does not track last N action scores or stop based on low action value. |
| Trace should expose scoring decisions | `ActionDecisionEvaluated`, `ActionReviewed`, `WorkflowPlanProgress`, and stop traces exist. | Partial | Trace lacks final action score, formula, modifiers, candidate alternatives, and rejected candidate reasons. |

## 2. Evidence From Current Code

- `src/engine/action_decision.rs` defines `ActionDecision`, `ProposedAction`,
  and `ActionScores`. It already scores tool calls by value, risk,
  uncertainty_reduction, cost, and reversibility.
- `src/engine/action_decision.rs` also contains phase alignment logic. It raises
  risk and lowers value when the model tries to mutate during the wrong stage.
- `src/engine/action_review.rs` builds `ActionWorthVerdict` and can revise
  premature mutation before execution. It also enforces tool availability,
  exposed-tool policy, permission, destructive scope, budget, and checkpoint
  rules.
- `src/engine/workflow_contract.rs` defines a separate plan-step weight system:
  `WeightFactors`, `compute_importance`, `WeightComputation`, normalized weight
  shares, and feedback application.
- `src/engine/learning_planning.rs` applies learning and memory signals to
  workflow plan factors, then recomputes plan weights.
- `src/engine/conversation_loop/workflow_contract_controller.rs` records
  `WorkflowPlanProgress` with top priority, top importance score, weight share,
  and reweight status.
- `src/engine/conversation_loop/tool_execution_controller.rs` computes
  `ActionDecision` before tool execution, attaches it to tool metadata, records
  `ActionDecisionEvaluated`, and applies bounded memory action modifiers.
- `src/engine/conversation_loop/tool_result_controller.rs` builds
  `ToolObservation` with key findings, candidate focus, recommended next action,
  reduced-uncertainty state, and risk notes.
- `src/engine/task_context.rs` consumes tool-observation ledger entries and
  increments or resets `uncertainty_not_reduced_steps`.
- `src/engine/stop_checker.rs` evaluates stop/replan conditions from repeated
  tool failures, no code progress, duplicate read-only calls, uncertainty not
  reduced, validation failures, edit failures, command failures, permission
  blocks, model-output failures, rollback candidates, and validation readiness.
- `src/engine/trace.rs` exposes action decision fields, but the event does not
  yet include `scope_fit`, `action_score`, formula stage, candidate count, or
  score modifiers.

## 3. Remaining Problems

### P0. There are multiple scoring vocabularies, but no canonical action-value contract

Current scoring is split between:

- `ActionScores` for individual tool calls;
- `WeightFactors` for workflow-plan steps;
- risk signals for workflow activation;
- memory/retrieval confidence scores;
- stop counters for no progress or failure;
- permission and destructive-scope gates.

Each piece is useful, but there is no single object that says:

```text
This action is worth doing now because the final score is X,
computed from these factors and these bounded modifiers.
```

This makes the behavior harder to calibrate and harder to evaluate.

### P0. The runtime scores the chosen action, not the candidate set

The model chooses a tool call first. The runtime then scores/reviews that tool
call. That catches unsafe or low-value actions, but it cannot directly choose
between:

- read a target file;
- grep for a symbol;
- edit immediately;
- run a focused test;
- ask user;
- stop and report a blocker.

The note's candidate-action idea is not fully present. This is the biggest
missing piece, but it should be introduced carefully because forcing every turn
through candidate JSON would make the runtime heavier.

### P0. `scope_fit` and final `action_score` are missing

`phase_aligned` is a boolean and destructive scope is a gate. They do not
replace a graded scope-fit dimension. Priority Agent needs to know whether an
action is not only allowed, but locally relevant to the user's goal and current
evidence.

The final action formula is also missing. Today the fields are recorded
individually; there is no calibrated score such as:

```text
action_score =
  value
  + uncertainty_reduction
  + reversibility * 0.5
  + scope_fit
  - risk
  - cost
```

The exact coefficients should be project-owned, not copied blindly from the
note. But the runtime needs one explicit formula per stage.

### P1. Observer signals stop the loop, but do not tune the next action

Observer output currently updates task state and stop counters. That is useful,
but it is a delayed correction. A stronger loop is:

```text
tool result -> ToolObservation -> task state -> next action modifiers
```

Examples:

- repeated reads with no findings should lower uncertainty-reduction score for
  more broad reads;
- a key finding in a file should raise scope fit for targeted edits/tests in
  that file;
- a failed validation should raise value for focused repair or source reads
  around the failing error;
- a successful validation should raise closeout value and lower further-edit
  value.

### P1. Memory action modifiers are present but shallow

The current memory modifier is intentionally bounded, which is good. The gap is
that it is mostly text/keyword based. After the memory-system plan, the project
now has a better path: typed memory records with kind, evidence, confidence,
importance, strategy metadata, stale state, and conflict state.

The weight system should use those typed fields instead of treating memory as
generic text.

### P1. StopChecker does not see action score history

StopChecker can see no-progress and uncertainty-not-reduced counters. It cannot
yet see:

- last selected action scores;
- repeated low `action_score`;
- repeated high risk with low value;
- candidate alternatives rejected by the runtime;
- whether the model keeps proposing the same revised action.

This limits the runtime's ability to stop because "continuing is not worth it"
instead of only because a specific counter fired.

### P1. ActionReview can revise, but cannot ask for a better scored candidate

When an action is revised, the recovery message gives a direction such as
"inspect first." That is already useful. The next step is to make revisions
score-aware:

```text
Rejected because scope_fit=2 and risk=8.
Choose a narrower inspection or validation action with scope_fit >= 6.
```

That gives the model an actionable target without stuffing long behavioral
rules into the prompt.

### P2. Evaluation does not yet assert weight-system behavior

Live reports expose weighted planning and runtime-spine signals, but there are
no dedicated assertions for:

- early edit demotion;
- candidate ranking;
- memory-driven risk increase;
- Observer-driven targeted action boost;
- stop after repeated low-value actions;
- score formula breakdown in trace.

Without this, tuning coefficients will become opinion-driven instead of
evidence-driven.

## 4. Design Direction For This Project

### 4.1 Keep the model free; make the runtime auditable

The weight system should not turn Priority Agent into a rigid step executor.
The model should still reason normally and propose actions. The runtime should:

- score;
- calibrate;
- reject/revise unsafe choices;
- record why;
- ask for candidate alternatives only when needed.

This matches the current runtime-diet direction: fewer always-on prompt rules,
more typed runtime contracts.

### 4.2 Treat weights as local action economics

For this project, weights should answer questions like:

- Does this action reduce uncertainty in this repo, not in the abstract?
- Is it consistent with gex's past successful workflow?
- Is it risky on this machine or only generally risky?
- Does memory say this strategy failed before?
- Is the validation command known and cheap?
- Is the edit reversible through checkpointed tools?
- Does the action move toward a verified finish?

This is the unique part. The formula is less important than the feedback loop
that learns the local value of actions.

### 4.3 Shadow first, gate second

Do not immediately block more actions just because a formula exists. First
record the formula and compare it with existing behavior. Promote the score into
hard gates only after tests and traces show it improves behavior.

### 4.4 Use a 1-10 scale, not the note's 1-5 scale

The current runtime already uses 1-10 scores in `ActionScores`. Keep that scale
to avoid churn. Map note thresholds approximately:

| Note 1-5 threshold | Runtime 1-10 equivalent |
| --- | --- |
| risk >= 5 | risk >= 9 or explicit high-risk permission gate |
| scope_fit <= 2 | scope_fit <= 4 |
| cost >= 5 | cost >= 8 |
| value <= 3 | value <= 5 |

The first implementation should make these thresholds configurable constants in
one module, not scattered literals.

## 5. Implementation Plan

### Phase 1: Canonical action scoring contract

Goal: one runtime-owned action scoring object for tool calls.

Tasks:

1. Extend or wrap `ActionScores` with:
   - `scope_fit: u8`;
   - `action_score: i16`;
   - `formula_stage: ActionScoreStage`;
   - `formula_version: String`;
   - `modifiers: Vec<ActionScoreModifier>`.
2. Keep existing fields stable for trace/backward compatibility.
3. Define stages:
   - `diagnosis`;
   - `planning`;
   - `implementation`;
   - `verification`;
   - `recovery`;
   - `closeout`.
4. Add formula helpers in or near `src/engine/action_decision.rs`:
   - `score_action_dimensions`;
   - `compute_action_score`;
   - `stage_formula_coefficients`.
5. Map existing `AgentTaskStage` into formula stages.
6. Include coefficient version in trace metadata.

Acceptance criteria:

- Existing `ActionDecision` tests pass.
- New tests prove `scope_fit` is lower for phase-misaligned and broad-scope
  actions.
- New tests prove `action_score` is computed deterministically.
- Existing serialized metadata remains compatible for old consumers.

Suggested validation:

```bash
cargo test -q action_decision -- --test-threads=1
cargo test -q action_review -- --test-threads=1
cargo fmt --check
```

### Phase 2: Shadow scoring for actual tool calls

Goal: record calibrated action scores without changing behavior yet.

Tasks:

1. Attach `action_score`, `scope_fit`, formula stage, formula version, and
   modifier breakdown to tool result metadata.
2. Extend `TraceEvent::ActionDecisionEvaluated` with the new fields.
3. Keep `ActionReview` gates unchanged except for metadata propagation.
4. Add live-report parser fields:
   - `action_scoring_active`;
   - `selected_action_score_min`;
   - `selected_action_score_avg`;
   - `low_score_actions`;
   - `phase_misaligned_actions`;
   - `action_score_formula_version`.

Acceptance criteria:

- Trace shows score breakdown for risky/mutating/broad actions.
- Report parser remains backward compatible with older trace files.
- No additional tool call is required in normal turns.

Suggested validation:

```bash
cargo test -q tool_execution_controller -- --test-threads=1
cargo test -q turn_recording -- --test-threads=1
python3 -m py_compile scripts/live_eval_report_parser.py
```

### Phase 3: Score-aware ActionReview calibration

Goal: convert the note's simple rules into bounded runtime gates.

Tasks:

1. Add review reasons:
   - `LowScopeFit`;
   - `LowActionValue`;
   - `HighCostLowValue`;
   - `HighRiskLowValue`;
   - `RepeatedLowScoreAction`.
2. Add soft revision rules:
   - `scope_fit <= 4` -> revise unless action is a required stop/ask-user path;
   - `cost >= 8 && value <= 5` -> revise;
   - `risk >= 8 && value <= 5` -> ask user or revise based on permission;
   - diagnosis-stage mutation without evidence remains revise.
3. Preserve existing permission/checkpoint/destructive-scope precedence.
4. Make recovery messages include the dominant score reason and a safer target.

Acceptance criteria:

- Premature edit behavior remains blocked.
- Low-value expensive actions produce `Revise`, not silent execution.
- High-risk low-value actions do not bypass permission.
- Existing checkpoint and destructive-scope tests still pass.

Suggested validation:

```bash
cargo test -q action_review -- --test-threads=1
cargo test -q tool_execution_controller -- --test-threads=1
cargo check -q
```

### Phase 4: Observer-to-weight feedback

Goal: use the Observer loop to adjust the next action score.

Tasks:

1. Add bounded `ObserverActionSignal` derived from recent task state and
   context-ledger observations:
   - reduced uncertainty;
   - repeated no-finding observations;
   - candidate focus;
   - key findings;
   - validation failure;
   - validation success;
   - risk note.
2. Feed this signal into `ActionDecision` as score modifiers:
   - focused read/edit/test around candidate focus raises `scope_fit`;
   - repeated broad inspection lowers `uncertainty_reduction`;
   - validation failure raises value for focused repair;
   - validation success raises closeout value and lowers further mutation value;
   - risk note raises risk for similar mutating actions.
3. Record modifier source as `observer`.
4. Keep modifier deltas small and capped.

Acceptance criteria:

- A key finding in file A raises scope fit for targeted actions on file A.
- Repeated generic reads with no findings reduce the next generic read score.
- Validation success makes closeout more valuable than another edit.
- Trace shows each observer modifier separately.

Suggested validation:

```bash
cargo test -q task_context -- --test-threads=1
cargo test -q tool_result_controller -- --test-threads=1
cargo test -q action_decision -- --test-threads=1
```

### Phase 5: Typed memory strategy modifiers v2

Goal: make memory influence weights through typed evidence, not keyword text.

Tasks:

1. Extend memory-to-action scoring to use typed memory records:
   - memory kind;
   - confidence;
   - importance;
   - evidence refs;
   - strategy success/failure metadata;
   - stale or conflict status;
   - project scope.
2. Convert text-only modifiers into a compatibility fallback.
3. Add modifier types:
   - `memory_failure_risk`;
   - `memory_success_value`;
   - `memory_stale_penalty`;
   - `memory_conflict_uncertainty`;
   - `memory_project_fit`.
4. Record memory ids and deltas in trace metadata.
5. Never let memory override permission/checkpoint/destructive-scope gates.

Acceptance criteria:

- A verified failure memory raises risk for matching mutation strategies.
- A verified successful diagnostic pattern raises value for matching
  inspection/validation.
- Stale project facts do not strongly boost action value.
- Conflicting memory raises uncertainty/verification pressure instead of
  directly choosing a path.

Suggested validation:

```bash
cargo test -q memory -- --test-threads=1
cargo test -q retrieval_context -- --test-threads=1
cargo test -q tool_execution_controller -- --test-threads=1
```

### Phase 6: Gated LLM candidate-action proposals

Goal: introduce candidate ranking only where it pays for itself.

This should not be always-on. Ask the model for 2-3 candidate next actions only
when one of these triggers fires:

- ActionReview revises a low-value action.
- StopChecker sees no progress or uncertainty not reduced.
- The selected action score is below threshold for two consecutive rounds.
- A high-risk action is about to ask the user and a safer alternative may exist.
- Debugging/recovery has repeated failures.

Candidate schema:

```json
{
  "candidate_actions": [
    {
      "id": "read_target_file",
      "action_type": "tool_call",
      "tool": "file_read",
      "arguments": { "path": "src/example.rs" },
      "reason": "Need current implementation before editing",
      "expected_observation": "Relevant function body",
      "model_scores": {
        "value": 7,
        "risk": 1,
        "uncertainty_reduction": 8,
        "cost": 2,
        "reversibility": 10,
        "scope_fit": 8
      }
    }
  ]
}
```

Tasks:

1. Add a small parser and validator for candidate-action JSON.
2. Score every candidate with the same runtime formula.
3. Reject candidates that use unavailable/unexposed tools or invalid arguments.
4. Choose the top allowed candidate only if it clearly beats the current
   proposed action, or return a score-aware revision message.
5. Start behind an env/config gate:
   - `PRIORITY_AGENT_CANDIDATE_ACTIONS=off|shadow|gated`;
   - default `shadow` or `off` until eval evidence is good.
6. Record candidate set in trace:
   - count;
   - selected id;
   - selected score;
   - rejected reasons;
   - model-vs-runtime score delta.

Acceptance criteria:

- Normal simple tasks do not require candidate JSON.
- Repeated stuck debugging can trigger candidate proposal.
- Candidate ranking chooses targeted inspection over another broad read when
  the Observer shows no uncertainty reduction.
- Candidate ranking never bypasses permission or checkpoint review.

Suggested validation:

```bash
cargo test -q action_decision -- --test-threads=1
cargo test -q action_review -- --test-threads=1
cargo test -q route_scoped_tools -- --test-threads=1
```

### Phase 7: StopChecker action-score history

Goal: let the runtime stop or replan when continuing is not worth it.

Tasks:

1. Store recent action score records in `AgentTaskState`:
   - tool;
   - stage;
   - action_score;
   - value;
   - risk;
   - uncertainty_reduction;
   - cost;
   - reversibility;
   - scope_fit;
   - review decision;
   - reduced uncertainty after result.
2. Feed summary fields into `StopCheckInput`:
   - `consecutive_low_action_scores`;
   - `consecutive_high_risk_low_value_actions`;
   - `score_without_uncertainty_reduction_rounds`;
   - `repeated_revised_action_count`.
3. Add stop/replan reasons:
   - `LowActionValueLoop`;
   - `ScoreNotReducingUncertainty`;
   - `RepeatedActionRevision`.
4. Make closeout report include the relevant score evidence when stopping.

Acceptance criteria:

- StopChecker can replan after repeated low-score actions even if tools
  technically succeeded.
- Repeated revised action proposals stop with a clearer model-recovery note.
- Validation-ready closeout still wins over low-score continuation.

Suggested validation:

```bash
cargo test -q stop_checker -- --test-threads=1
cargo test -q turn_iteration_controller -- --test-threads=1
cargo test -q closeout -- --test-threads=1
```

### Phase 8: Live-eval and reporting coverage

Goal: make weight-system behavior measurable.

Tasks:

1. Extend `scripts/live_eval_report_parser.py` with:
   - `action_scoring_active`;
   - `candidate_action_count`;
   - `selected_action_score`;
   - `low_action_score_count`;
   - `score_driven_replan`;
   - `score_driven_stop`;
   - `observer_modifier_applied`;
   - `memory_modifier_applied`;
   - `scope_fit_revision`;
   - `early_edit_demoted`;
   - `candidate_selected_by_runtime`.
2. Add live-eval behavior assertions:
   - `action_score_recorded`;
   - `scope_fit_recorded`;
   - `early_edit_demoted`;
   - `observer_modified_next_action`;
   - `memory_modified_action_score`;
   - `low_score_replan_triggered`;
   - `candidate_ranking_used`.
3. Add or update eval tasks:
   - early edit before evidence should revise;
   - repeated broad inspection should replan;
   - memory failure lesson should raise mutation risk;
   - validation success should prefer closeout over more edits;
   - high-risk low-value bash action should ask/revise;
   - stuck debugging should request candidate alternatives.

Acceptance criteria:

- New report fields are present without breaking old benchmark reports.
- At least one deterministic runtime-spine test covers scoring end to end.
- At least two live tasks assert behavior-level scoring outcomes.

Suggested validation:

```bash
python3 -m py_compile scripts/live_eval_report_parser.py
bash -n scripts/run_live_eval.sh scripts/live-eval-aggregate-summary.sh
cargo test -q runtime_spine_behavior_tests -- --test-threads=1
```

## 6. Recommended First Implementation Slice

Do not start with full LLM candidate-action JSON. That is the most visible
feature, but it is also the easiest way to over-control the model.

Start with this slice:

1. Phase 1: canonical scoring contract with `scope_fit` and `action_score`.
2. Phase 2: shadow trace/reporting for actual tool calls.
3. A small part of Phase 3: score-aware metadata and tests, but no broad new
   blocking except already-covered premature mutation.

Then review traces from real tasks. If the formula is sane, continue with
Observer and Memory modifiers before enabling candidate alternatives.

This ordering gives the project durable instrumentation before it changes
behavior.

## 7. Open Design Decisions

### Decision 1: Should action scoring and workflow plan weighting share one formula?

Recommendation: no. Share vocabulary and trace conventions, but keep formulas
separate.

Reason: plan-step weighting answers "which objective matters most?" Action
scoring answers "which concrete next move is worth taking now?" They overlap,
but forcing one formula will make both worse.

### Decision 2: Should the model be forced to emit candidate actions every turn?

Recommendation: no. Use gated candidate proposals.

Reason: the project direction is runtime-diet and tool-contract enforcement,
not heavy prompt protocol. Candidate JSON is valuable during stuck/risky
moments, but wasteful on simple tasks.

### Decision 3: Should weights directly execute the highest-ranked action?

Recommendation: only after normal tool review.

Reason: scoring is about usefulness. Permission, checkpoint, destructive-scope,
and user-intent gates are about safety and authority. The latter must remain
harder than the former.

### Decision 4: Should memory be allowed to dominate action selection?

Recommendation: no. Memory should be a bounded modifier.

Reason: memories can be stale, project-specific, or overfit to prior failures.
They should bias risk/value/uncertainty, but never replace current evidence.

### Decision 5: Should weights be visible to the user?

Recommendation: mostly in trace/debug panels, not normal final answers.

Reason: gex needs concise user-facing answers. The score details should be
available when debugging agent behavior, not spam every response.

## 8. Target Architecture

```text
Model proposed tool call
        |
        v
ActionDecision
  base tool profile
  task stage
  route risk
  checkpoint state
  Observer modifiers
  Memory modifiers
  score formula
        |
        v
ActionReview
  tool availability
  tool exposure
  arguments
  permission
  destructive scope
  budget
  checkpoint
  score-aware revision
        |
        v
Execute / revise / ask user / deny
        |
        v
ToolObservation
  key findings
  focus
  uncertainty reduced
  validation result
  risk note
        |
        v
TaskState + StopChecker + Memory/Learning
        |
        v
Next action score modifiers
```

Gated candidate-action path:

```text
Low score / stuck / high risk / repeated revision
        |
        v
Ask model for 2-3 candidates
        |
        v
Runtime scores all candidates
        |
        v
Top candidate still goes through ActionReview
```

## 9. Success Criteria

The weight system should be considered working when:

- every non-trivial tool call has a traceable action score;
- score dimensions include value, risk, uncertainty reduction, cost,
  reversibility, and scope fit;
- score formula and modifier sources are visible in traces;
- Observer findings change the next action's score in predictable ways;
- typed memory records can raise or lower value/risk without overriding safety;
- StopChecker can replan after repeated low-value actions;
- candidate-action ranking is available for stuck/risky cases, but not required
  for ordinary turns;
- live-eval reports can prove weight-system behavior instead of relying on
  subjective review.

## 10. Suggested Validation Gate For The Full Plan

```bash
cargo test -q action_decision -- --test-threads=1
cargo test -q action_review -- --test-threads=1
cargo test -q tool_execution_controller -- --test-threads=1
cargo test -q tool_result_controller -- --test-threads=1
cargo test -q task_context -- --test-threads=1
cargo test -q stop_checker -- --test-threads=1
cargo test -q runtime_spine_behavior_tests -- --test-threads=1
python3 -m py_compile scripts/live_eval_report_parser.py
bash -n scripts/run_live_eval.sh scripts/live-eval-aggregate-summary.sh
cargo fmt --check
cargo check -q
```

For implementation PRs that touch scoring, review one real trace before
promoting a shadow score into a blocking gate.
