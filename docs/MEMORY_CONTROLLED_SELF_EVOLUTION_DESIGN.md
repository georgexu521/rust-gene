# Memory-Controlled Self-Evolution Design

Date: 2026-04-28

This document reviews the proposed memory and self-evolution direction, compares
it with the current Priority Agent codebase, and turns it into an implementation
plan.

## Short Answer

The proposal is directionally right. Memory and self-evolution are a better fit
for mathematical scoring and control theory than ordinary task planning.

The reason is simple: task planning is mostly semantic judgment, while memory and
self-evolution are persistent state updates. Persistent state needs gates,
decay, confidence, audit trails, rollback, and proof that changes improve future
behavior.

The product principle should be:

```text
AI extracts candidates and explains semantic meaning.
Math gates value, risk, priority, and stability.
Validation proves whether an update helped.
The controller limits update size and prevents drift.
Humans approve high-risk behavior changes.
```

## External Reference Summary

Hermes Agent's public docs show three useful patterns:

- Persistent memory separates agent/project learning from user preferences, with
  short memory files injected into future sessions.
- Memory providers add a prefetch/sync/session-end lifecycle and can externalize
  semantic memory.
- Skills are procedural memory: reusable instructions loaded on demand, with
  agent-created skills treated as possible but requiring trust controls.

The Hermes self-evolution repo goes further: it treats skills, prompts, tool
descriptions, and code as evolvable artifacts, but routes changes through
execution traces, eval data, constraints, benchmark checks, and review.

DSPy's GEPA direction is relevant later because it optimizes text components
from execution traces and textual feedback, not only scalar scores. That is
useful only after we have enough clean experience data and eval cases.

Reference links:

- [Hermes persistent memory](https://hermes-agent.nousresearch.com/docs/user-guide/features/memory)
- [Hermes memory providers](https://hermes-agent.nousresearch.com/docs/user-guide/features/memory-providers)
- [Hermes skills](https://hermes-agent.nousresearch.com/docs/user-guide/features/skills)
- [Hermes self-evolution repo](https://github.com/NousResearch/hermes-agent-self-evolution)
- [DSPy GEPA overview](https://dspy.ai/api/optimizers/GEPA/overview/)

## Current Priority Agent State

Priority Agent already has important foundations:

- `src/memory/quality.rs`
  - Scores memory candidates using stable fact, future utility, specificity,
    relevance, volatility, sensitivity risk, and duplication.
  - Applies thresholds: accepted, proposed, rejected.
- `src/memory/safety.rs`
  - Blocks prompt injection, secret-like strings, invisible unicode, and unsafe
    shell/secret combinations.
- `src/engine/retrieval_context.rs`
  - Uses provenance-bearing retrieval items.
  - Memory ranking uses lexical match, scope, confidence, recency, and a semantic
    proxy.
  - Conflicts lower confidence and appear in traces.
- `src/session_store/mod.rs`
  - Persists `LearningEventRecord` with kind, source, summary, confidence, and
    JSON payload.
- `src/engine/improvement.rs`
  - Creates controlled `ImprovementProposal`s from learning events.
  - Requires explicit user accept/apply.
- `src/engine/skill_evolution.rs`
  - Converts repeated successful procedures into `SkillProposal`s.
  - Adds quality checks, safety scan, trust state, and explicit apply.
- `src/engine/learning_planning.rs`
  - Feeds recent learning events and retrieved memory into workflow planning
    factor adjustments.
  - Records before/after planning audit.
- `src/engine/evalset.rs`
  - Provides deterministic behavior eval foundations.

That means the project is no longer just "saving memory"; it has the skeleton of
a closed loop.

## Implementation Audit: 2026-04-28

The first math-hardening pass has now addressed the highest-risk gaps found in
code review:

- `memory_save` now uses the normal quality gate instead of treating model tool
  calls as an unconditional explicit save.
- `explicit` memory saves no longer bypass scoring for low-quality or duplicate
  candidates; explicit intent can lower friction, but not override unsafe,
  duplicate, or very weak signals.
- Memory retrieval scoring now separates lexical strength, match density,
  semantic proxy, scope, trust, recency, and token cost. Conflicting memories are
  capped below high-confidence injection range.
- Learning-to-planning feedback now requires token-level relevance before old
  success patterns or high-confidence memories can adjust a plan step.
- Skill mining now records `creation_score`, `creation_factors`,
  `evidence_count`, and `scope_confidence`, and exposes a first-pass
  `SkillFitness` formula.

The first implementation pass against this design has also landed:

- `src/memory/scoring.rs` now owns `MemoryWriteScore` and `MemoryKeepScore`
  contracts.
- `src/memory/recall.rs` now owns `RecallScore`, `RecallFactors`, and
  `RecallDecision`.
- `src/engine/experience_ledger.rs` adds a typed `ExperienceRecord` and embeds
  it inside learning event payloads without breaking existing fields.
- `src/engine/skill_evolution.rs` now persists skill usage events, aggregates
  `SkillFitnessSnapshot`, and exposes a promotion gate comparator.
- `src/engine/evolution_controller.rs` implements `EvolutionTriggerScore`,
  target risk policy, cooldown checks, and the "no auto-apply for high-risk
  targets" rule.
- `/experience` now exposes structured Experience Ledger records from the
  interactive CLI.
- Skill invocation now writes a lightweight provisional usage event, and
  the interactive CLI infers a conservative confirmed outcome when that response
  finishes. `/skill-proposals record <skill> <success|fail> [version]` can
  manually attach outcome feedback to a skill fitness history.

Remaining implementation gaps:

- Memory write scoring is now formalized, but thresholds still need calibration
  with real rejected/accepted samples.
- Recall scoring is improved but still uses a semantic proxy, not embeddings or
  provider-backed semantic similarity.
- Memory maintenance scoring is visible in doctor/review output, but automatic
  archive/delete is intentionally not enabled yet.
- Skill fitness events, version comparison, manual outcome recording, and CLI
  inspection exist. Provisional invocation events count toward reuse but not
  success/failure, so they do not inflate Fitness before a confirmed outcome.
  Automatic final-outcome attribution now covers ordinary interactive CLI skill
  responses with conservative heuristics; deeper acceptance-review attribution,
  automatic EvalSet binding, rollback pointer materialization, and full
  promotion UX are not fully productized yet.
- Evolution trigger scoring and cooldown exist, but controller decisions are not
  yet routed through all improvement/skill proposal commands.

## Main Gap

The current implementation has pieces of a control system, but not yet one
unified controller.

The biggest gaps are:

1. Memory scoring now has write/recall/keep contracts, but needs telemetry
   calibration and deeper semantic similarity.
2. Retrieval scoring exists, but prior usefulness, task criticality, and token
   cost are not strong first-class inputs.
3. Skill proposals exist, but `SkillCreationScore` and long-term `Fitness` are
   not yet part of promotion.
4. Improvement proposals exist, but prompt/tool/workflow changes do not yet have
   a quantified trigger score, cooldown, regression gate, and rollback contract.
5. Learning events are useful but still broad JSON payloads. We need a cleaner
   Experience Ledger schema for long-term learning.

## Control-Theory Framing

The agent's state can be viewed as:

```text
S(t) = [
  user_preferences,
  project_facts,
  environment_facts,
  workflow_patterns,
  failure_patterns,
  skill_library,
  trust_scores,
  uncertainty
]
```

Each completed turn produces observations:

```text
O(t) = [
  user_feedback,
  tool_results,
  tests_passed,
  errors_seen,
  acceptance_status,
  repair_attempts,
  final_outcome,
  cost
]
```

The memory system is the state estimator:

```text
S(t+1) = Update(S(t), O(t))
```

The self-evolution system is the controller:

```text
Policy(t+1) = Policy(t) + bounded_update(error(t), evidence(t))
```

Where:

```text
error(t) = target_performance - observed_performance(t)
```

The update must be bounded. Otherwise the system will oscillate: one failure
changes a prompt, the next failure changes it back, and skills become bloated or
unstable.

## Proposed Scoring Contracts

### Scoring Convention

All factors should be normalized to `[0.0, 1.0]` before entering a formula.
Formulas use additive positive evidence and subtractive penalties, then clamp the
final score to `[0.0, 1.0]`.

This means the positive weights usually sum to `1.0`, while penalties are extra
negative control terms. The score is not a probability; it is a bounded control
signal for gating, ordering, and auditing.

```text
score = clamp(positive_weighted_sum - penalty_weighted_sum, 0.0, 1.0)
```

Thresholds should be treated as product policy, not mathematical truth. They
must be calibrated with eval cases and manual review samples over time.

### 1. Memory Write Score

Current `MemoryQualityAssessment` is close, but should become explicit about
reuse, trust, novelty, risk reduction, and token cost.

```text
MemoryWriteScore =
  0.25 * Relevance
+ 0.20 * ReuseProbability
+ 0.15 * Stability
+ 0.15 * Trust
+ 0.10 * Novelty
+ 0.10 * RiskReduction
- 0.15 * TokenCost
- 0.20 * SensitivityRisk
```

Decision:

```text
score >= 0.65  -> accepted long-term memory
0.45..0.65     -> proposed/session learning only
score < 0.45   -> rejected
unsafe/secret  -> rejected regardless of score
duplicate      -> reject or no-op even if explicit
```

Mapping to current code:

| Desired factor | Current equivalent | Gap |
|---|---|---|
| Relevance | `relevance` | present |
| ReuseProbability | `future_utility` | present but should be renamed/split |
| Stability | `stable_fact - volatility` | present but implicit |
| Trust | sensitivity/provenance indirectly | needs explicit evidence trust |
| Novelty | `1 - duplication` | present but implicit |
| RiskReduction | absent | add from category/evidence/test failure |
| TokenCost | absent | add char/token normalized penalty |
| SensitivityRisk | `sensitivity_risk` | present |

First code target:

```text
src/memory/scoring.rs
  MemoryWriteFactors
  MemoryWriteDecision
  score_memory_write()
```

Then `src/memory/quality.rs` can wrap the scorer instead of carrying all logic
inline.

### 2. Recall Score

Current `RetrievalContext` scores memory using lexical, scope, confidence,
recency, and semantic proxy. It should be expanded:

```text
RecallScore =
  0.30 * SemanticSimilarity
+ 0.20 * ScopeMatch
+ 0.15 * Recency
+ 0.15 * Trust
+ 0.10 * PriorUsefulness
+ 0.10 * TaskCriticality
- 0.15 * TokenCost
```

Decision:

```text
score >= 0.70 -> inject
0.50..0.70    -> keep available but not always injected
score < 0.50  -> omit
conflict      -> cap below high-confidence range and surface conflict reason
```

Mapping to current code:

| Desired factor | Current equivalent | Gap |
|---|---|---|
| SemanticSimilarity | semantic proxy | needs real provider/vector hook later |
| ScopeMatch | scope heuristic | present |
| Recency | fixed/source heuristic | needs real last_used/updated |
| Trust | USER.md/conflict heuristic | needs record-level trust |
| PriorUsefulness | absent | add from experience ledger |
| TaskCriticality | absent | add from intent/workflow risk |
| TokenCost | token estimate exists | not yet penalized in score |

Current implementation note:

```text
RecallScore =
  0.30 * SemanticProxy
+ 0.20 * LexicalMatch
+ 0.18 * ScopeMatch
+ 0.15 * Trust
+ 0.12 * Recency
+ 0.10 * MatchDensity
- 0.15 * TokenCost

if conflict:
  score = min(score * 0.55, 0.49)
```

This is still a proxy, but it is now mathematically cleaner than the old
implementation because `SemanticProxy` is no longer the same raw score repeated
under a different name.

First code target:

```text
src/memory/recall.rs
  RecallFactors
  RecallDecision
  score_recall()
```

`RetrievalItem` should keep the full factor breakdown for trace explainability.

### 3. Memory Keep Score

The project has maintenance and capacity controls, but no clear keep/archive
score yet.

```text
MemoryKeepScore =
  0.25 * RecentUse
+ 0.25 * HistoricalUsefulness
+ 0.20 * Trust
+ 0.15 * Stability
+ 0.15 * ScopeImportance
- 0.20 * ContradictionRisk
- 0.15 * Redundancy
```

Decision:

```text
score >= 0.65 -> keep active
0.40..0.65    -> compress or demote
score < 0.40  -> archive or delete candidate
contradiction -> review/supersede candidate
```

First code target:

```text
src/memory/maintenance.rs
  MemoryKeepFactors
  MemoryMaintenanceDecision
  score_memory_keep()
```

This should feed `/memory doctor` and future `/memory review`.

## Experience Ledger

`LearningEventRecord` is good for general persistence, but self-evolution needs a
more structured ledger.

Suggested normalized event payload:

```json
{
  "task_type": "bug_fix",
  "risk": "medium",
  "workflow": "code_change",
  "plan_before": [],
  "plan_after": [],
  "tools_used": [],
  "tool_failures": [],
  "tests": [],
  "acceptance_status": "passed",
  "repair_attempts": 1,
  "cost": {
    "tokens": 0,
    "duration_ms": 0,
    "tool_calls": 0
  },
  "user_feedback": null,
  "candidate_memories": [],
  "candidate_skills": [],
  "final_outcome": "completed"
}
```

First code target:

```text
src/engine/experience_ledger.rs
  ExperienceRecord
  ToolUseSummary
  ValidationSummary
  AcceptanceSummary
  CandidateMemoryRef
  CandidateSkillRef
```

Keep storing as `LearningEventRecord` initially, but use a typed serializer so
future scorers can rely on stable fields.

## Skill Mining And Fitness

Current `SkillProposal` generation checks repeated successful procedures and
quality. That is a good Phase 1.

The missing piece is numeric creation and promotion.

### Skill Creation Score

```text
SkillCreationScore =
  0.25 * Repeatability
+ 0.25 * Complexity
+ 0.20 * SuccessEvidence
+ 0.15 * FutureUtility
+ 0.15 * UserCorrectionValue
- 0.20 * OverSpecificity
```

Decision:

```text
score >= 0.70 -> create SkillProposal
0.50..0.70    -> keep as improvement proposal / memory candidate
score < 0.50  -> no skill
```

Add to `SkillProposal`:

```text
creation_score
creation_factors
evidence_count
scope_confidence
```

Current implementation note:

```text
creation_score = clamp(
  0.25 * repeatability
+ 0.25 * complexity
+ 0.20 * success_evidence
+ 0.15 * future_utility
+ 0.15 * user_correction_value
- 0.20 * over_specificity,
  0.0,
  1.0
)
```

The current implementation gates proposal creation at `0.70`. It also reduces
scores when repeated events lack observed workflow steps or observed tool usage,
so trivial repeated interactions do not become skills.

### Skill Fitness

Once a skill is active, every invocation should update usage stats.

```text
SkillFitness =
  0.30 * TaskSuccess
+ 0.20 * AcceptancePassRate
+ 0.15 * TestPassRate
+ 0.10 * UserSatisfaction
+ 0.10 * ReuseRate
+ 0.10 * TimeSaved
+ 0.05 * ToolEfficiency
- 0.15 * FailureRate
- 0.10 * Cost
- 0.20 * RiskPenalty
```

Current implementation note:

```text
fitness = clamp(
  0.30 * task_success
+ 0.20 * acceptance_pass_rate
+ 0.15 * test_pass_rate
+ 0.10 * user_satisfaction
+ 0.10 * reuse_rate
+ 0.10 * time_saved
+ 0.05 * tool_efficiency
- 0.15 * failure_rate
- 0.10 * cost
- 0.20 * risk_penalty,
  0.0,
  1.0
)
```

This function exists as a scoring primitive. It is not yet backed by persistent
skill usage telemetry or automatic promotion.

Promotion gate:

```text
NewFitness - OldFitness > 0.05
AND RegressionRate == 0
AND EvalCount >= N
AND RiskPenalty < threshold
AND SemanticDrift < threshold
```

First code target:

```text
src/engine/skill_fitness.rs
  SkillUsageEvent
  SkillFitnessStats
  compute_skill_fitness()
  compare_skill_versions()
```

CLI targets:

```text
/skill-proposals fitness <name>
/skill-proposals gate <name> [old-fitness]
/skill-proposals bind-eval <id> <evalset>
/skill-proposals versions <name>
/skill-proposals rollback <name> --yes
```

## Evolution Controller

Self-evolution should be a controller, not a collection of commands.

It should support targets in increasing risk order:

```text
Memory
Skill
Prompt Section
Workflow Policy
Tool Description
Core Code
```

### Evolution Trigger Score

```text
EvolutionTriggerScore =
  0.30 * RepeatedFailure
+ 0.25 * ReuseFrequency
+ 0.20 * UserCorrectionFrequency
+ 0.15 * TaskImpact
+ 0.10 * OptimizationPotential
- 0.20 * EvolutionCost
- 0.20 * Risk
```

Decision:

```text
score >= 0.70 -> propose evolution
0.50..0.70    -> monitor, collect more evidence
score < 0.50  -> do nothing
```

### Risk Policy

| Target | Default mode |
|---|---|
| Memory | automatic accept if safe and high score, otherwise proposed |
| Skill draft | automatic proposal only |
| Skill apply | explicit user accept/apply |
| Prompt section | explicit user review plus eval |
| Workflow policy | explicit user review plus eval and rollback |
| Tool description | explicit review plus eval |
| Core code | PR-style workflow only |

### Stability Controls

Add these controls before allowing any prompt/workflow/tool evolution:

- cooldown per target
- max update size
- rollback pointer
- eval count minimum
- fitness improvement threshold
- semantic drift threshold
- no auto-apply for high-risk changes

First code target:

```text
src/engine/evolution_controller.rs
  EvolutionTarget
  EvolutionTriggerFactors
  EvolutionGateDecision
  EvolutionController
```

This controller should consume:

- `ExperienceRecord`
- `ImprovementProposal`
- `SkillProposal`
- `EvalSet` results
- user accept/reject events

## Suggested Implementation Plan

### Phase A: Formalize Memory Scoring

Goal: replace ad hoc memory quality logic with explicit factors.

Status: implemented as a first pass.

Tasks:

1. Add `src/memory/scoring.rs`.
2. Implement `MemoryWriteFactors`, `MemoryWriteScore`, and decision thresholds.
3. Keep `assess_memory_candidate()` as compatibility wrapper.
4. Add token-cost and risk-reduction factors.
5. Store score breakdown in memory decision logs.

Acceptance:

- Memory write decisions explain every factor.
- Secrets and prompt injection are still rejected regardless of score.
- Existing memory tests still pass.

### Phase B: Recall Scoring And Usefulness Feedback

Goal: make retrieval ranking learn from usefulness.

Status: partially implemented. `RecallFactors` and token-cost penalties exist;
last-used/usefulness feedback is still pending.

Tasks:

1. Add `RecallFactors` and `RecallDecision`.
2. Penalize token cost.
3. Add task criticality from `IntentRoute` / workflow risk.
4. Track memory last_used and usefulness events.
5. Show factor breakdown in `/memory explain`.

Acceptance:

- Trace explains why memory was injected or omitted.
- Unused or conflicting memory loses rank over time.

### Phase C: Experience Ledger

Goal: make learning events reliable enough for scoring.

Status: mostly implemented. Turn and tool learning events now include a typed
`experience` payload while preserving old fields. `/experience last|list|show`
exposes recent records in the interactive CLI. Candidate memory/skill links are
still pending.

Tasks:

1. Add typed `ExperienceRecord`.
2. Persist workflow outcome with tools, validation, acceptance, cost, and repair
   count.
3. Link candidate memories and skills to the originating experience.
4. Add `/learn show <id>` or `/experience last`.

Acceptance:

- A finished coding workflow creates one structured experience record.
- Skill/memory proposals can point to experience IDs.

### Phase D: Skill Creation Score

Goal: prevent weak or over-specific skills from being proposed.

Tasks:

1. Add creation factors to `SkillProposal`.
2. Gate proposal creation by `SkillCreationScore`.
3. Add over-specificity detection.
4. Preserve current quality checklist.

Acceptance:

- Repeated trivial tasks do not become skills.
- Repeated non-trivial successful workflows do become candidates.

### Phase E: Skill Fitness And Versioning

Goal: prove skills improve behavior before promotion.

Status: mostly implemented. `SkillUsageEvent`, `SkillFitnessSnapshot`, a
promotion comparator, provisional skill invocation telemetry, conservative
automatic outcome attribution, manual `/skill-proposals record` outcome
feedback, `/skill-proposals gate`, `/skill-proposals bind-eval`, and
`/skill-proposals versions` exist. Provisional events count toward reuse but not
success/failure. Bound evalsets block apply when they fail, and apply records
version metadata plus a rollback pointer. `/skill-proposals rollback <name>
--yes` safely disables an active generated skill by moving it aside instead of
deleting it. Acceptance-review attribution and richer rollback restore UX are
still pending.

Tasks:

1. Add `SkillUsageEvent`.
2. Compute `SkillFitness`.
3. Add skill version metadata and rollback pointer.
4. Connect selected skills to EvalSet.
5. Add `/skills fitness`.

Acceptance:

- Skill promotion requires evidence.
- Regression blocks promotion.
- User can inspect old/new fitness.

### Phase F: Evolution Controller

Goal: unify memory, skill, prompt, and workflow evolution gates.

Status: partially implemented. Trigger scoring, cooldown, target risk policy,
and high-risk review gating exist. Full routing through proposal commands and
learning-event audit persistence is still pending.

Tasks:

1. Add `EvolutionController`.
2. Implement `EvolutionTriggerScore`.
3. Add cooldown and max-change constraints.
4. Route high-risk proposals to explicit review.
5. Record all decisions as learning events.

Acceptance:

- Prompt/workflow/tool changes cannot auto-apply.
- Every evolution decision has score, evidence, and rollback story.

### Phase G: GEPA/DSPy Adapter Later

Goal: optimize text artifacts using traces only after enough eval data exists.

Do not start here. GEPA is useful after:

- enough ExperienceRecords exist
- eval cases exist for target skills
- rollback and promotion gates are implemented

Then add:

```text
EvolutionJob:
  input: skill/prompt + success/failure cases
  mutation: generate variants
  eval: run EvalSet
  selection: fitness + drift + cost + risk
  output: proposal, not automatic apply
```

## Priority Recommendation

Next best implementation order:

1. Phase A: formal memory scoring
2. Phase C: typed experience ledger
3. Phase B: recall usefulness feedback
4. Phase D: skill creation score
5. Phase E: skill fitness
6. Phase F: evolution controller

The reason is dependency order. Skill fitness and evolution control require
stable experience data. Recall usefulness requires memory usage telemetry.
Prompt/workflow evolution should wait until memory and skill evolution are
measurable.

## Final View

This direction is worth doing.

The current project already has the hard foundations: memory quality gates,
safety scanning, retrieval provenance, learning events, improvement proposals,
skill proposals, and learning-to-planning feedback.

The next level is to turn those foundations into an explicit control system:

```text
Memory = state estimator
LearningEvent / ExperienceRecord = observation stream
Skill = procedural memory
Fitness = reward signal
EvolutionController = bounded adaptive controller
EvalSet = verification environment
HumanReview = high-risk safety valve
```

That is the difference between "an agent with memory" and "an agent that can
accumulate engineering experience without losing control."
