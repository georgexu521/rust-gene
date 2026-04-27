# Coding Agent Workflow Discussion

Last updated: 2026-04-26

This document captures the current discussion about improving Priority Agent as a programming-focused agent. It is intentionally a planning and discussion document, not an implementation checklist yet.

## Source Idea

The whiteboard notes describe a stronger coding-agent loop:

1. Build a plan before acting, with AI-driven task decomposition, priority, and weight.
2. When the agent encounters a problem, start guided reasoning instead of guessing.
3. Validate at each stage, detect hidden problems, and prevent fix loops.

The overall direction is: a coding agent should not simply receive a user request and immediately edit files. It should behave more like a senior engineer who plans, investigates, checks assumptions, validates changes, and stops when the work becomes unsafe or unclear.

## Current Assessment

Priority Agent already has several foundations for this direction:

- Plan Mode and workflow planner.
- Weight engine and priority scheduling.
- IntentRouter and learning-aware routing.
- Socratic engine and Socratic executor.
- Goal drift detector.
- ReflectionPass.
- EvalSet and regression test concepts.
- HumanReviewRequest.
- ResourcePolicy.
- Tool permission and approval system.
- Session, memory, retrieval, and context management.

However, these pieces are not yet fully unified into the default programming workflow. The current state is closer to "the project has many relevant capabilities" than "every coding task reliably follows a mature engineering loop."

## 1. Planning And Weighting

### Target Behavior

For programming tasks, the agent should default to a structured planning process:

```text
User request
-> classify task type: bug fix / feature / refactor / investigation / test / review
-> generate plan
-> assign weights to steps:
   - risk
   - dependency
   - user value
   - uncertainty
   - blocking factor
-> execute the highest-priority step first
-> re-evaluate remaining steps after each meaningful result
```

### Current Gap

The project has planning and weighting modules, but they are not yet a hard requirement for all meaningful code-change workflows. In some paths, the agent can still move directly from request to edits without a clearly recorded plan, risk list, or acceptance criteria.

### Opinion

This is worth implementing, especially for medium and high-risk programming tasks. It should not be too heavy for simple edits. A risk-sensitive strategy is better:

```text
Low risk:
  direct edit + minimal verification

Medium risk:
  lightweight plan + targeted tests

High risk:
  full plan + risk checklist + staged validation + reflection
```

## 2. Guided Reasoning When Problems Appear

### Target Behavior

When the agent hits uncertainty, a failing command, ambiguous code, or conflicting evidence, it should not immediately guess or repeatedly retry. It should enter a guided reasoning loop:

```text
Problem detected
-> decide whether this is a true blocker
-> generate key questions
-> answer questions using local code/context/tools first
-> ask the user only when a decision truly requires human judgment
-> continue when enough evidence exists
```

The agent should usually ask itself questions before asking the user.

### Current Gap

Priority Agent has Socratic components, but they are not yet reliably triggered by workflow events such as:

- test failure
- tool failure
- ambiguous requirement
- unexpected diff
- goal drift
- repeated failed fix attempt
- missing acceptance criteria

### Opinion

This is important because it improves autonomy without making the agent reckless. The ideal behavior is not "ask the user more often"; it is "reason more carefully and only ask the user for genuinely human choices."

## 3. Stage Validation And Hidden Problem Checks

### Target Behavior

The agent should maintain an explicit validation loop:

```text
Before edits:
  define acceptance criteria
  identify risks
  identify expected files/modules

During edits:
  check whether the work is still aligned with the goal
  detect scope creep
  keep changes bounded

After edits:
  run relevant tests/checks
  inspect diff
  verify acceptance criteria
  check for regressions and hidden risks
  decide whether to continue, repair, ask, or stop
```

The workflow should prevent endless repair loops:

```text
failure detected
-> diagnose
-> repair once or a bounded number of times
-> re-run focused validation
-> if still failing, stop and report clearly
```

### Current Gap

ReflectionPass, EvalSet, ResourcePolicy, and HumanReviewRequest exist, but the project still needs a stronger "default closeout contract" for code-change workflows:

- Every code-change task should end with a validation summary.
- ReflectionPass should be mandatory for high-risk workflows.
- Unresolved high-risk reflection findings should block completion or request confirmation.
- Test failure recovery should be bounded and explainable.
- The final answer should reflect actual verification, not just intent.

### Opinion

This is the most important of the three points. It is the difference between a coding assistant that can write code and a coding agent that can be trusted with real software work.

## Proposed Unified Workflow

The desired programming workflow could be:

```text
1. Intake
   - classify the request
   - determine risk level
   - identify likely files/modules

2. Plan
   - generate steps
   - assign weights
   - define acceptance criteria
   - define risk checklist

3. Investigate
   - read relevant files
   - inspect tests and existing patterns
   - gather missing context

4. Execute
   - make bounded edits
   - update plan as evidence changes
   - avoid unrelated refactors

5. Validate
   - run focused checks
   - inspect diff
   - verify acceptance criteria

6. Reflect
   - detect hidden risks
   - check goal drift
   - decide whether more work is needed

7. Close
   - summarize changes
   - report tests run
   - report residual risks
```

## AI-Led Judgment, Not Hard-Coded Judgment

An important design principle: Priority Agent should not replace the model's judgment with a rigid set of hand-written rules.

For example, the software should not rely only on hard-coded logic such as:

```text
if request contains "website" then complexity = high
if request contains "button" then complexity = low
if missing "database" then ask question
```

That kind of rule can be useful as a fallback or guardrail, but it should not be the main intelligence of the agent.

Instead, Priority Agent should give the model a structured thinking contract:

```text
Before coding, analyze:
1. What type of programming task is this?
2. How complete is the user's requirement?
3. What important information is missing?
4. Is the missing information blocking, or can you make a conservative default?
5. What is the complexity and risk level?
6. Should you ask the user questions, or proceed with assumptions?
7. If proceeding, what assumptions are you making?
8. What plan should be followed?
9. Which plan steps are most important and why?
10. What validation is required before completion?
```

In this design, the software provides:

- the prompt contract
- available context
- output schema
- persistence
- validation hooks
- retry limits
- human review points

The AI provides:

- requirement interpretation
- complexity judgment
- severity judgment
- risk analysis
- question generation
- planning
- prioritization
- tradeoff reasoning

This distinction matters. The goal is not to make the software "think instead of the AI." The goal is to make the software reliably ask the AI to think one step deeper before acting.

## Programming Strategy

When a user gives a programming request, such as building a website, the agent should not immediately start writing files. It should first run a short internal analysis:

```text
1. Understand the request.
2. Judge task type and risk.
3. Judge whether the requirement is complete enough.
4. If key information is missing, ask focused questions.
5. If enough information exists, produce a weighted plan.
6. Execute according to the plan.
7. Validate each stage.
8. Use guided reasoning when problems appear.
9. Run final validation and reflection.
```

The important nuance is that asking questions should be conditional. The agent should not ask questions just to appear careful. It should ask when the missing information changes architecture, user experience, data model, permissions, deployment, or acceptance criteria.

### Example: Website Request

If the user says:

```text
Build a website for recording book notes.
```

The agent should identify missing high-impact details and ask focused questions:

```text
I need to confirm a few points before building:
1. Is this for local single-user use, or should it support accounts/login?
2. What fields should each note have? For example: book title, author, quote, tags, rating.
3. Do you need search, filtering, export, or sync?
4. Should I build a simple usable first version or a more polished product-style version?
```

If the user says:

```text
Build a local single-page book notes app. No backend. Support create/delete/search/tag filtering. Store data in localStorage. Use a clean minimal style.
```

The agent should not ask unnecessary questions. It should proceed with a plan and explicitly record assumptions.

### When The AI Should Ask Questions

Rather than hard-coding this as fixed software rules, Priority Agent should prompt the AI to decide whether questions are necessary. The model should consider:

- Is the user goal unclear?
- Are there multiple reasonable implementations with very different costs?
- Is a key architecture choice missing?
- Are data model, permissions, deployment, or persistence unclear?
- Would proceeding likely cause rework?
- Can a conservative default safely handle the ambiguity?
- Did the user explicitly ask the agent to decide?

The output should include a decision:

```json
{
  "needs_user_questions": true,
  "reason": "The persistence and authentication requirements change the architecture.",
  "questions": [
    "Should this be local-only or multi-user with login?",
    "Should notes sync across devices?"
  ],
  "safe_assumptions_if_no_answer": [
    "Build a local-only version using localStorage."
  ]
}
```

### When The AI Should Proceed Without Questions

The AI should proceed when:

- the task is small and local
- the missing information does not affect the first useful version
- the user gave enough constraints
- a conservative default is obvious
- the user explicitly says to decide autonomously

The model should still record assumptions:

```json
{
  "needs_user_questions": false,
  "assumptions": [
    "Use localStorage because no backend was requested.",
    "Use a single-page layout because no routing requirements were specified."
  ]
}
```

## Weighted Planning

Plans should not be simple ordered lists. Each step should include a reasoned priority.

The user should not be asked to fill in numeric weights. Weight is an internal model judgment, not a user-facing configuration burden. It can be represented as a percentage, score, rank, or priority label. The representation is secondary. The real purpose is to make the AI understand which plan step matters more and why.

The model should assign priority by thinking through factors such as:

- dependency
- user value
- risk reduction
- uncertainty reduction
- effort
- reversibility
- validation importance

Acceptable output forms include:

```text
High / Medium / Low
```

```text
P0 / P1 / P2
```

```text
weight: 0.86
```

```text
priority: 86%
```

Priority Agent should not force one representation too early. A numeric score is useful for sorting and recording, but the explanation is more important than the number.

An example scoring shape:

```json
{
  "step": "Define the data model for notes",
  "weight": 0.86,
  "factors": {
    "dependency": 0.95,
    "user_value": 0.80,
    "risk_reduction": 0.75,
    "uncertainty_reduction": 0.70,
    "effort": 0.30
  },
  "reason": "Most features depend on the note schema, and changing it later would cause rework."
}
```

The exact formula should not be the main point. The important requirement is that the model must justify why a step comes before another. Priority Agent can provide a recommended formula as guidance, but the AI should be allowed to reason and adjust based on context.

Example guidance:

```text
Consider:
score =
  dependency * 0.30
+ user_value * 0.25
+ risk_reduction * 0.20
+ uncertainty_reduction * 0.15
- effort * 0.10

You may adjust if the task context justifies it.
```

This formula should be treated as a thinking aid, not a hard-coded product rule. The AI should be asked to produce both:

```json
{
  "priority": "high",
  "weight": 0.86,
  "reason": "This step unlocks all later implementation and reduces rework risk."
}
```

If the model provides a number, the software can sort by it. If the model provides a label, the software can map it to ordering. In both cases, the AI is making the judgment.

## Model-Led Weighted Control

The preferred weight design is not "AI freely invents a number" and not
"a formula replaces model judgment." The better design is:

```text
AI judgment + mathematical constraint + feedback correction
```

In this model:

```text
AI understands semantics and context.
Math turns that judgment into stable, sortable, traceable scores.
Tools and validation results feed back into the next weighting decision.
```

The weight is therefore a control signal, not just a percentage on a task
list. It helps the agent decide the next control action:

```text
read more code
ask the user
edit a file
run focused validation
repair a failure
stop and report risk
```

### Responsibility Split

```text
Weight source: AI
Weight calculation: math
Weight execution: software
Weight correction: feedback
Weight explanation: AI
```

Expanded:

- AI judges task type, risk, dependencies, user value, uncertainty, rework risk,
  and why one step matters before another.
- Math converts those judgments into normalized scores, sorts steps, clamps
  extremes, records changes, and detects whether reweighting occurred.
- Software stores plans, selects the active step, triggers validation and guided
  debugging, limits repair attempts, and exposes workflow state.
- Feedback changes the next weights: test failures raise repair/verification
  priority, goal drift raises clarification priority, tool failures trigger
  guided debugging, and acceptance gaps raise validation priority.

### Factor-Based Weighting

The agent should avoid asking the model "what is the weight?" directly. A better
prompt is:

```text
How important is this step right now for safely completing the task?
```

The model can answer that through factors:

```text
D = dependency
V = user_value
R = risk_reduction
U = uncertainty_reduction
B = blocking
C = cost
```

A first useful formula:

```text
raw_score =
  D * 0.25
+ V * 0.25
+ R * 0.20
+ U * 0.15
+ B * 0.15
- C * 0.10
```

Then:

```text
weight = raw_score / sum(raw_score for all open steps)
```

Priority mapping can be:

```text
0.80 - 1.00 = P0
0.60 - 0.79 = P1
0.40 - 0.59 = P2
0.00 - 0.39 = P3
```

This formula is a control model and a thinking aid, not a claim of truth. The AI
still owns the semantic judgment. The formula stabilizes the output so the
system can sort, replay, compare, and learn from it.

### Concrete Weight Computation

Implementation should keep two numbers separate:

```text
importance_score:
  Absolute importance of one step, in [0.0, 1.0].
  Use this for P0/P1/P2/P3 mapping.

weight_share:
  Relative share among currently open steps, sum ~= 1.0.
  Use this for scheduling, dashboard percentages, and reweight history.
```

This distinction matters. If `weight_share` is used for P0/P1/P2/P3, then a
plan with many steps will make every step look low priority because each
normalized share is small. Priority should come from absolute importance, not
from the normalized share.

#### Input Factor Bounds

Every factor is a model judgment in `[0.0, 1.0]`:

```text
D = dependency
V = user_value
R = risk_reduction
U = uncertainty_reduction
B = blocking
C = cost
```

The software must sanitize all numeric inputs:

```text
sanitize(x):
  if x is missing, NaN, infinite, or non-numeric -> None
  otherwise clamp(x, 0.0, 1.0)
```

For high-risk tasks, missing factors should be treated as a contract problem:
ask the model to regenerate or fall back to a conservative P1/P2 plan. For
medium-risk tasks, missing factors can be inferred from the priority label. For
low-risk tasks, factors can be skipped entirely.

#### Base Formula

Use this as the first implementation formula:

```text
positive_score =
  D * 0.25
+ V * 0.25
+ R * 0.20
+ U * 0.15
+ B * 0.15

cost_penalty = C * 0.10

importance_score = clamp(positive_score - cost_penalty, 0.05, 1.00)
```

Notes:

- Positive factors sum to `1.00`.
- Cost is a penalty, but it should not erase a step completely.
- The `0.05` floor prevents divide-by-zero and avoids losing low-priority but
  still valid work.
- Completed steps should not be scheduled even if their score remains high.

Then normalize across open steps:

```text
open_steps = all steps where status != completed
sum_score = sum(step.importance_score for open_steps)

if sum_score <= 0:
  weight_share = 1.0 / open_steps.len()
else:
  weight_share = step.importance_score / sum_score
```

Priority label comes from `importance_score`:

```text
importance_score >= 0.80 -> P0
importance_score >= 0.60 -> P1
importance_score >= 0.40 -> P2
else                     -> P3
```

#### Priority Label Fallback

If the AI gives only a label and no factors, map it to an initial
`importance_score`:

```text
P0 -> 0.90
P1 -> 0.70
P2 -> 0.50
P3 -> 0.25

high   -> 0.75
medium -> 0.55
low    -> 0.30
```

This fallback should be marked in trace as:

```json
{
  "weight_source": "priority_label_fallback"
}
```

#### Sorting Rule

The active step should be selected with a stable comparator:

```text
sort open steps by:
1. priority rank: P0 before P1 before P2 before P3
2. adjusted_importance_score descending
3. blocking descending
4. dependency descending
5. cost ascending
6. original plan order ascending
```

The last tie-breaker keeps behavior stable across runs.

### Feedback Reweighting

Reweighting should update factor scores first, then recompute the formula. It
should not directly mutate the final weight unless the model provides an
explicit override.

Represent feedback as an event:

```json
{
  "event": "test_failure | tool_failure | goal_drift | acceptance_gap | step_completed | user_feedback",
  "severity": "low | medium | high",
  "confidence": 0.0,
  "affected_steps": ["step-id"]
}
```

Map severity to numeric intensity:

```text
low    -> 0.25
medium -> 0.50
high   -> 0.80

event_strength = severity_value * confidence
```

Then apply factor deltas:

```text
factor' = clamp(factor + delta * event_strength, 0.0, 1.0)
```

Recommended first deltas:

| Feedback event | Affected step updates |
| --- | --- |
| `test_failure` | `R += 0.25`, `B += 0.20`, `U += 0.10` |
| `tool_failure` | create/raise diagnostic step: `U += 0.25`, `B += 0.15`; lower repeated failing action `C += 0.20` |
| `goal_drift` | create/raise clarification step: `U += 0.30`, `R += 0.20`, `B += 0.20` |
| `acceptance_gap` | raise validation/repair step: `V += 0.20`, `R += 0.25`, `B += 0.20` |
| `step_completed` | mark step completed; exclude it from `open_steps` |
| `user_feedback` | let AI propose factor deltas, then clamp and record |

Reweighting should be recorded when any of these are true:

```text
top active step changed
abs(old_importance_score - new_importance_score) >= 0.15 for any open step
validation failed
acceptance review returned continue_repair or stop
goal drift was medium/high
```

Trace payload should include:

```json
{
  "reweighted": true,
  "reason": "test_failure",
  "old_active_step": "Implement storage",
  "new_active_step": "Repair persistence failure",
  "changed_scores": [
    {
      "step_id": "storage",
      "old_importance_score": 0.72,
      "new_importance_score": 0.91,
      "old_weight_share": 0.22,
      "new_weight_share": 0.41
    }
  ]
}
```

### Override Formula

AI overrides should be represented as a separate adjustment after the formula:

```text
formula_score = computed importance_score
override_delta = adjusted_score - formula_score
adjusted_importance_score = clamp(adjusted_score, 0.05, 1.00)
```

Default guardrail:

```text
if abs(override_delta) > 0.25:
  require override.reason
  require override.confidence >= 0.70
```

If the override does not meet the guardrail, keep the formula score and record:

```json
{
  "override_accepted": false,
  "reason": "override delta too large without enough confidence"
}
```

The stored step should keep both values:

```json
{
  "formula_importance_score": 0.55,
  "adjusted_importance_score": 0.78,
  "weight_share": 0.31,
  "override": {
    "accepted": true,
    "reason": "The user explicitly prioritized visual demo quality."
  }
}
```

### Suggested Rust Shape

The implementation can start with these structs:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightFactors {
    pub dependency: f32,
    pub user_value: f32,
    pub risk_reduction: f32,
    pub uncertainty_reduction: f32,
    pub blocking: f32,
    pub cost: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightOverride {
    pub adjusted_importance_score: f32,
    pub reason: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightComputation {
    pub formula_importance_score: f32,
    pub adjusted_importance_score: f32,
    pub weight_share: f32,
    pub priority: PriorityLabel,
    pub source: WeightSource,
    pub override_accepted: bool,
}
```

Core functions:

```rust
fn compute_importance(factors: WeightFactors) -> f32;
fn priority_from_importance(score: f32) -> PriorityLabel;
fn normalize_weight_shares(steps: &mut [WorkflowPlanStep]);
fn normalize_open_weight_shares(steps: &mut [WorkflowPlanStep], completed_step_ids: &[String]);
fn apply_weight_feedback(step: &mut WorkflowPlanStep, event: &WeightFeedbackEvent);
fn should_record_reweight(old: &[WorkflowPlanStep], new: &[WorkflowPlanStep]) -> bool;
```

The first pass does not need a complicated optimizer. Deterministic factor
scoring plus event-driven feedback is enough.

### Risk-Sensitive Weight Depth

Low-risk tasks should stay fast:

```json
{
  "risk": "low",
  "planning_mode": "minimal",
  "priority": "direct_edit"
}
```

Medium-risk tasks can use priority plus reason:

```json
{
  "risk": "medium",
  "plan": [
    {
      "step": "Inspect the existing note schema",
      "priority": "high",
      "reason": "Tag filtering depends on the note data structure."
    }
  ]
}
```

High-risk tasks should use full factor scoring:

```json
{
  "risk": "high",
  "plan": [
    {
      "step": "Identify all workflow entry points",
      "weight": 0.91,
      "factors": {
        "dependency": 0.95,
        "user_value": 0.75,
        "risk_reduction": 0.90,
        "uncertainty_reduction": 0.85,
        "blocking": 0.90,
        "cost": 0.40
      },
      "reason": "Missing an entry point would leave part of the coding workflow outside the new contract."
    }
  ]
}
```

### AI Override With Accountability

The AI should be allowed to adjust the formula's result when task context
justifies it, but overrides must be explicit:

```json
{
  "override": true,
  "original_weight": 0.55,
  "adjusted_weight": 0.78,
  "reason": "The user asked for a visual demo first, so UI completeness has higher acceptance value than the default formula assigned."
}
```

This prevents both failure modes:

- math becoming rigid and blind to context
- AI changing weights without accountability

### User Visibility

Detailed numeric weights should be internal by default. Normal users should see
the explanation:

```text
I will handle the data model and persistence first because later features depend
on them; UI polish comes after the core flow is verified.
```

Advanced/debug views can expose the full factor scores, raw weights, adjusted
weights, and reweight history.

### Minimal Schema Direction

A practical next schema for weighted planning is:

```json
{
  "task_type": "feature | bug_fix | refactor | investigation | test | review",
  "risk": "low | medium | high",
  "needs_user_questions": true,
  "assumptions": [],
  "plan": [
    {
      "id": "stable-step-id",
      "step": "string",
      "priority": "P0 | P1 | P2 | P3",
      "importance_score": 0.0,
      "weight_share": 0.0,
      "factors": {
        "dependency": 0.0,
        "user_value": 0.0,
        "risk_reduction": 0.0,
        "uncertainty_reduction": 0.0,
        "blocking": 0.0,
        "cost": 0.0
      },
      "override": {
        "adjusted_importance_score": 0.0,
        "reason": "string",
        "confidence": 0.0
      },
      "reason": "string",
      "acceptance_criteria": []
    }
  ]
}
```

The core product rule is:

```text
AI judges why a step matters.
Math calculates how much it matters.
Software chooses what to do next.
Feedback decides whether to change the weights.
```

## When To Use Guided Reasoning

Guided reasoning should be selective. If the agent uses a deep Socratic process for every small task, the product will feel slow and overly formal. If it never uses guided reasoning, it will guess too much and fail on complex work.

The desired behavior is:

```text
Simple and clear:
  proceed directly

Medium complexity:
  use lightweight planning and targeted validation

Complex, risky, ambiguous, or failing:
  trigger guided reasoning
```

The software should not decide all of this with rigid rules. It should prompt the AI to judge whether guided reasoning is needed, and it should provide workflow events that make the judgment easier.

### Places Where Guided Reasoning Can Appear

Guided reasoning can be used at several points, but not all points need it every time.

| Workflow Point | Default Behavior | Trigger Guided Reasoning When |
| --- | --- | --- |
| Intake | Understand request quickly | Requirement is ambiguous, broad, or architecture-changing |
| Planning | Generate direct plan | Plan has competing approaches or unclear dependencies |
| Investigation | Read relevant files | Codebase structure is unfamiliar or evidence conflicts |
| Execution | Make scoped edits | Edit affects shared contracts, permissions, data, or architecture |
| Validation | Run focused checks | Tests fail, output is unexpected, or verification is incomplete |
| Reflection | Check hidden risks | High-risk change, repeated failure, or possible goal drift |
| Closeout | Summarize work | Acceptance criteria are partially met or residual risk remains |

### Trigger Conditions

The AI should consider guided reasoning when it sees:

- unclear user goal
- missing information that affects architecture
- multiple reasonable implementation paths with different tradeoffs
- high-risk area: permissions, data loss, memory, workflow, agent handoff, security, migrations
- unfamiliar code path
- tool failure
- test failure
- unexpected diff
- repeated failed repair
- possible goal drift
- context conflict
- user asks for a broad product, website, app, or system

The AI should usually skip guided reasoning when:

- the task is a small local edit
- the requirement is explicit
- the missing information does not block a useful first version
- the change is easy to verify
- the user asked for a quick direct change

### Prompt Contract For Guided Reasoning

When guided reasoning is triggered, the model should answer a compact set of questions:

```text
1. What is the exact uncertainty or failure?
2. Why does it matter?
3. What are the likely explanations or options?
4. What evidence can resolve it fastest?
5. Can I proceed with a conservative assumption?
6. Do I need to ask the user, or can I continue?
7. What is the next safest action?
```

The output should be compact and operational. It is not meant to be a long essay. It should directly decide whether to continue, inspect more, ask the user, or stop.

## Stage Validation

Each plan step should have acceptance criteria.

Example:

```text
Step: Implement note creation.

Acceptance criteria:
- User can enter a title and note content.
- Saving adds the note to the list.
- Empty content is rejected.
- Refreshing the page preserves saved notes.
```

After each meaningful step, the agent should ask itself:

```text
1. Did this step satisfy its acceptance criteria?
2. Did the implementation stay within scope?
3. Did this introduce new risks?
4. Do weights or next steps need to change?
5. Is more validation needed now?
```

Again, Priority Agent should prompt the AI to perform this judgment. The software can store the result and enforce retry limits, but the reasoning should come from the AI.

## Acceptance System

The third whiteboard point suggests a dedicated acceptance system. This is worth considering because a coding agent can easily "finish" a task while quietly drifting from the user's real goal.

Claude Code and Codex already do a useful version of this informally: after work is done, they return to the user with a summary of changes, tests run, and remaining risks. That is helpful because it forces the agent to reconnect the implementation to the original request.

Priority Agent can go further by adding an explicit acceptance loop.

### Core Idea

The acceptance system should not be a static checklist written entirely by the software. It should be an AI-generated acceptance process guided by a prompt contract.

```text
Before implementation:
  AI proposes acceptance questions and criteria.

During implementation:
  AI checks progress against the current criteria.

After implementation:
  AI performs a final acceptance review:
    - Did we satisfy the original request?
    - Did we satisfy each acceptance criterion?
    - Did we introduce hidden problems?
    - What was not verified?
    - Should we continue, ask the user, or stop?
```

The software's job is to record the criteria, call the acceptance prompt at the right time, enforce retry limits, and make the result visible. The AI's job is to decide what should be accepted and why.

### Acceptance Questions

The model should generate project-specific acceptance questions. For a website task, examples might be:

```text
1. Does the page implement every user-requested feature?
2. Is the main user flow usable from start to finish?
3. Does the layout work on common screen sizes?
4. Are empty, loading, and error states handled?
5. Does data persist or submit according to the requirement?
6. Are visual choices consistent with the requested style?
7. Is there any feature the user likely expected but we skipped?
```

For a bug fix task:

```text
1. Is the reported bug fixed?
2. Is there a regression test or focused verification?
3. Could the fix break neighboring behavior?
4. Did the fix address the root cause or only the symptom?
5. Are there similar code paths that need checking?
```

For a refactor:

```text
1. Is behavior preserved?
2. Are public interfaces unchanged or intentionally changed?
3. Are tests still passing?
4. Did complexity actually go down?
5. Did the refactor expand beyond the requested scope?
```

### Acceptance Timing

Acceptance should happen at multiple levels, but not with the same weight every time.

| Timing | Purpose | Depth |
| --- | --- | --- |
| Before work | Define what "done" means | Lightweight for simple tasks, detailed for complex tasks |
| After each major step | Prevent drift early | Short self-check |
| After tool/test failure | Re-evaluate whether the plan still works | Guided reasoning |
| Before final response | Confirm completion and residual risk | Required for code changes |
| After user feedback | Compare feedback with acceptance criteria | Update plan if needed |

### Acceptance Result

The final acceptance result should be structured:

```json
{
  "accepted": true,
  "confidence": "medium",
  "criteria": [
    {
      "criterion": "User can create and delete notes",
      "status": "passed",
      "evidence": "Manual flow verified in browser"
    },
    {
      "criterion": "Notes persist after refresh",
      "status": "passed",
      "evidence": "localStorage behavior verified"
    },
    {
      "criterion": "Responsive layout works on mobile",
      "status": "not_verified",
      "evidence": "No mobile viewport check was run"
    }
  ],
  "residual_risks": [
    "No automated browser test was added"
  ],
  "next_action": "ask_user_or_finish"
}
```

This does not mean the user must see all JSON. The software can use the structure internally and show a concise final summary.

### User Acceptance Versus AI Acceptance

There are two different kinds of acceptance:

1. AI self-acceptance.
2. User acceptance.

AI self-acceptance answers:

```text
Based on the request and available evidence, do I think this is complete?
```

User acceptance answers:

```text
Does the user agree that this matches what they wanted?
```

The agent should not ask the user to approve every small change. But for larger deliverables, it can return with a clear acceptance summary and ask a focused question:

```text
I believe the first version is complete against the criteria above.
The only unverified item is mobile visual polish.
Would you like me to refine that next, or is this acceptable for now?
```

### Avoiding Overhead

Acceptance should be risk-sensitive:

```text
Low-risk task:
  final self-check only

Medium-risk task:
  acceptance criteria + final validation summary

High-risk task:
  acceptance criteria + step checks + ReflectionPass + possible user confirmation
```

This keeps simple work fast while giving complex work a real quality gate.

### Relationship To ReflectionPass

Acceptance and reflection are related but not identical.

Acceptance asks:

```text
Did we satisfy the intended outcome?
```

Reflection asks:

```text
Did we miss hidden risks, regressions, or reasoning gaps?
```

Both are needed. Acceptance is goal-oriented. Reflection is risk-oriented.

### Suggested Product Behavior

For non-trivial programming tasks, the agent should maintain an internal `AcceptanceContract`:

```text
AcceptanceContract
  - original_user_goal
  - assumptions
  - acceptance_criteria
  - validation_evidence
  - unresolved_items
  - final_acceptance_status
```

The contract should be created by the AI and stored by the software. During closeout, the final answer should be grounded in this contract.

## Guided Debugging

When a failure appears, the agent should enter a guided debugging loop:

```text
Failure detected
-> summarize exact symptom
-> identify recent changes
-> propose likely causes
-> choose the easiest cause to verify
-> run focused check
-> repair if justified
-> re-run validation
```

The model should be prompted to answer questions like:

```text
1. What exactly failed?
2. What changed recently?
3. What are the top three likely causes?
4. Which cause can be verified fastest?
5. What is the smallest safe fix?
6. Could this fix break anything else?
```

The software should enforce a bounded repair policy:

```text
Normal failure:
  allow up to 2 automatic repair attempts

High-risk failure:
  allow 1 repair attempt, then reassess

Repeated failure:
  stop and report diagnosis, attempted fixes, and next recommendation
```

This prevents infinite loops while still allowing useful autonomous repair.

## Final Closeout

For coding tasks, the final answer should include:

```text
What changed:
- files/modules touched
- behavior added or fixed

Validation:
- tests/checks run
- manual checks performed
- acceptance criteria status

Residual risk:
- what was not verified
- assumptions made
- follow-up opportunities
```

This closeout should be generated from actual workflow records where possible, not from memory or optimism.

## Risk-Sensitive Policy

Not every request should pay the full cost of the full workflow.

| Risk | Example | Required Process |
| --- | --- | --- |
| Low | Rename text, small UI wording, docs update | Direct edit, basic check |
| Medium | Add small feature, fix localized bug | Lightweight plan, targeted tests, diff check |
| High | Refactor, permissions, memory, workflow, agent handoff, data loss risk | Full plan, acceptance criteria, risk checklist, ReflectionPass, bounded repair |

## Candidate Implementation Direction

Potential future implementation areas:

1. Make `CodeChangeWorkflow` the default path for programming tasks.
2. Add a model-facing workflow prompt contract that asks the AI to judge complexity, risk, missing information, and whether questions are needed.
3. Attach `Plan + Acceptance Criteria + Risk Checklist` to each non-trivial code-change task.
4. Let the AI assign plan priority/weight, while the software records and sorts the result.
5. Trigger Socratic reasoning on failures, ambiguity, drift, or repeated repair.
6. Require ReflectionPass for high-risk code changes.
7. Add bounded repair attempts for failed tests/checks.
8. Emit a structured validation report before final response.
9. Store workflow outcomes as learning events for future routing.

## Implementation Progress

### 2026-04-26

The first implementation slice has started.

Completed:

- Added a model-led workflow contract module.
  - The runtime can ask the model to judge task type, complexity, risk, missing information, whether user questions are needed, assumptions, plan priority/weight, guided reasoning triggers, and acceptance criteria.
  - The result is stored in `TaskContextBundle` and surfaced in trace events.
- Injected workflow judgment into real conversation turns for programming workflows.
  - The extra preflight model call is enabled by default for non-mock providers.
  - It can be disabled with `PRIORITY_AGENT_WORKFLOW_CONTRACT=0`.
  - If parsing fails, the turn continues and records a workflow fallback event.
- Added a model-led acceptance review contract after code edits.
  - The model reviews acceptance criteria using verification evidence, changed files, and the original acceptance contract.
  - The result records accepted/not accepted, confidence, unresolved items, residual risks, and the next action.
- Added a guided debugging contract for failed tool rounds.
  - When a tool round fails, the model can identify the symptom, likely causes, evidence to collect, smallest safe action, whether to ask the user, and whether to inspect, repair, ask, or stop.
- Added first enforcement and observability hooks.
  - High-risk workflows can be stopped before final closeout when acceptance review remains unresolved after bounded repair attempts or explicitly returns `stop`.
  - Acceptance review and guided debugging outcomes are persisted as learning events.
  - `/quick` now shows latest acceptance and guided-debug state alongside task/retrieval/reflection contract status.
- Removed the external environment dependency from workflow engine unit tests.
  - Default `cargo test` now passes without requiring `PRIORITY_AGENT_WORKFLOW_ENABLED=1`.

Still to implement:

- None in the current implementation slice.

Completed after the initial slice:

- Made plan progress visible through trace events and `/quick`.
  - `workflow.plan` records total steps, completed steps, active step, top priority, and whether the plan was reweighted.
  - `/quick` surfaces the latest plan progress under Contracts.
- Persisted workflow judgment into learning events.
  - This complements the existing acceptance review and guided debugging learning events.
- Added EvalSet coverage for workflow contract behavior.
  - Eval replay can now include workflow judgment, plan progress, acceptance review, and guided debugging events.
  - Tests verify these events are present and can affect expected repair/verification outcomes.

Completed weighted-control refinement:

- Upgrade weighted planning from "model-provided priority/weight" to
  "model-provided factor scores + mathematical normalization + feedback
  reweighting."
  - This keeps the model responsible for semantic judgment.
  - It gives the software a stable, auditable control signal.
  - It makes reweighting explainable after validation failures, tool failures,
    goal drift, or acceptance gaps.

### 2026-04-27

Completed:

- Implemented the first Model-Led Weighted Control code path.
  - `WorkflowPlanStep` now supports stable step ids, `importance_score`,
    `weight_share`, factor scores, guarded AI overrides, and persisted
    computation metadata.
  - The runtime computes absolute importance from AI-provided factors using the
    documented formula, maps importance to P0/P1/P2/P3, and normalizes relative
    weight share across the open plan.
  - Legacy `weight` and priority-only plans still work through explicit fallback
    sources, so older prompt responses and tests remain compatible.
- Added deterministic weight feedback primitives.
  - `WeightFeedbackEvent` covers test failures, tool failures, acceptance gaps,
    goal drift, user corrections, and completed steps.
  - Feedback updates factors first, then recomputes importance, priority, and
    traceable computation metadata.
  - Open-step normalization can exclude completed step ids, and the workflow
    contract exposes a stable `should_record_reweight` predicate for later
    executor integration.
- Improved workflow observability.
  - `workflow.plan` trace events now include top importance score, normalized
    weight share, and weight source.
  - `/quick` surfaces those values in the contract panel.
  - Workflow learning events now include a weighted plan summary.
  - Guided debugging tool failures and failed acceptance reviews now feed back
    into the active workflow plan and emit reweighted plan progress when the
    active step or score changes materially.
- Added coverage for factor-based weighting, override guardrails, feedback
  reweighting, EvalSet replay, trace formatting, and CLI quick-panel display.

Completed second workflow integration slice:

- Added a `CodeChangeWorkflowRunner` boundary for programming turns.
  - It centralizes risk-sensitive workflow policy, stage validation records,
    acceptance review history, changed-file tracking, and closeout generation.
  - The direct conversation loop still performs provider/tool orchestration, but
    code-change workflow state no longer lives only as ad hoc local variables.
- Solidified risk-sensitive policy.
  - Low-risk programming work stays minimal and quiet.
  - Medium-risk programming work uses workflow judgment, stage validation,
    final closeout, and two bounded repair attempts.
  - High-risk programming work uses strict validation, visible weight details,
    ReflectionPass blocking, and one bounded repair attempt before reassessment.
- Added lightweight stage validation.
  - After a meaningful file-change round, the runner records the active plan
    step, changed files, validation status, and compact evidence.
  - Failed stage validation produces weight feedback, so validation failures can
    change the next active step instead of only appending text to the prompt.
- Added structured final closeout.
  - Coding turns can now append a closeout derived from workflow records:
    changed files, validation status, acceptance status, and residual risk.
  - The trace records `closeout` separately so `/trace` and `/quick` can show
    whether the final answer was grounded in workflow records.
- Improved CLI observability.
  - `/quick` now includes stage validation and closeout state in the Contracts
    block, while detailed weight values remain limited to plan/debug surfaces.

## Design Conclusion

The preferred design is a model-led engineering workflow:

```text
Software provides structure.
AI provides judgment.
Tools provide evidence.
Validation provides accountability.
User confirmation is reserved for meaningful choices.
```

Priority Agent should not become a pile of hard-coded heuristics that tries to decide everything itself. Instead, it should reliably ask the model to produce structured judgments at the right time:

- requirement completeness
- task risk
- whether user questions are needed
- plan priority
- acceptance criteria
- validation result
- hidden risk assessment
- whether to continue, repair, ask, or stop

The product should stay fast for simple tasks and become rigorous only when the task is complex, ambiguous, risky, or failing.

## Open Questions

- What should count as low, medium, and high risk in this project?
- Which checks are mandatory for high-risk changes?
- When should the agent ask the user instead of continuing autonomously?
- Should Plan Mode be visible in the CLI, or mostly internal unless the user asks?
- How strict should ReflectionPass blocking be?
- Should failed validation automatically trigger one repair attempt, or should that depend on task risk?
- What prompt format best encourages model judgment without producing verbose analysis?
- Which parts of weighted planning should be shown in normal mode versus debug mode?
- Should low-risk tasks bypass weighted planning entirely?

## Discussion Notes

- The main goal is to improve programming reliability, not to add process for its own sake.
- The workflow should be quiet for simple tasks and rigorous for risky work.
- The agent should avoid endless loops by using explicit retry budgets.
- The most valuable next step is likely to unify existing capabilities into a real code-change workflow contract.
- Weight is a model-generated prioritization signal, not a user input requirement.
- Guided reasoning should be event-triggered and risk-triggered, not always-on.
