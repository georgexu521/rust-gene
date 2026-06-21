# Lab Graduate Execution Policy Discussion

Date: 2026-06-21

Status: discussion record and implementation guidance

Related plan: `docs/LAB_AGENT_WORKFLOW_PLAN_2026-06-18.md`

## Context

The recent LabRun work added a strict Lab graduate provider certification path.
That path was useful for exposing a real problem: some provider/model
combinations can claim they wrote files or ran validation even when runtime
evidence shows no tool use, no file change, and no validation proof.

The design question is whether Lab graduate execution should be blocked before
execution based on provider certification. The current conclusion is no.

LabRun should not treat providers differently by name. DeepSeek, Kimi, MiniMax,
OpenAI-compatible providers, local providers, and future custom providers should
all enter the same Lab graduate execution policy. The runtime should judge
mechanical evidence and boundaries, not provider identity.

## Discussion Summary

The user model is closer to a real lab:

- The professor owns direction and high-level judgment.
- The postdoc owns implementation quality and review.
- The graduate agent performs scoped implementation work and may make mistakes.

In that model, a graduate student is not pre-blocked because their skill is
uncertain. They are given a narrow task, their work is observed, and the postdoc
reviews the result. If the result is bad, the postdoc creates a follow-up task,
narrows the scope, changes instructions, or rejects the output.

This maps well to coding agents. A graduate subagent should be allowed to try a
scoped task. The runtime should record what actually happened. The postdoc
should then review code, diffs, validation, and evidence before anything is
integrated.

## Opencode Comparison

The opencode task/subagent model supports this direction.

The relevant opencode design uses child sessions with parent linkage, inherited
or derived permissions, task IDs, background execution, synthetic result
injection into the parent session, and worktree/snapshot support. It does not
appear to use a provider-name or provider-certification gate before a subagent
is allowed to execute.

Important takeaways:

- Subagents are operationally constrained by permissions and task scope.
- Parent/child session linkage is durable.
- Background completion can be injected back into the parent session.
- Worktree or snapshot evidence can support review and rollback.
- The parent or caller still has to decide whether the returned work is good.

For this project, that means Lab graduate should be closer to the existing
generic subagent path: dispatch the task, capture durable evidence, then let
postdoc review decide the next step.

## Policy Decision

Do not use provider-level graduate certification as a hard pre-execution gate.

Use one provider-neutral graduate execution policy:

1. Convert an accepted postdoc task into a narrow `GraduateTask`.
2. Dispatch it through the existing subagent execution path.
3. Run it in an isolated worktree or equivalent bounded workspace.
4. Capture runtime-observed evidence.
5. Bind a `GraduateResult` only from observed output and evidence.
6. Require postdoc review before merge, integration, or stage advancement.
7. If review fails, create another task or blocker report instead of pretending
   the LabRun succeeded.

The runtime should not decide that a provider is intelligent enough to work.
The runtime should decide only whether the execution produced evidence, stayed
inside boundaries, and satisfied deterministic checks.

## Evidence Policy

For a code-writing `GraduateTask`, runtime evidence should include:

- subagent session ID and task ID;
- tools used, if available;
- workspace snapshot before and after execution;
- changed files;
- changed files versus `allowed_scope`;
- validation commands requested by the task;
- validation commands actually run;
- validation exit status and output summary;
- structured graduate result, if the model produced one;
- errors, timeouts, missing proof, or scope violations.

Recommended task-level states:

- `ExecutedWithEvidence`: task produced observed output and evidence.
- `NoEffectiveOutput`: task finished but produced no meaningful file, diff, or
  validation evidence for a code-writing task.
- `ValidationFailed`: required validation ran and failed.
- `ScopeViolation`: task changed files outside allowed scope.
- `ReviewRejected`: postdoc reviewed the output and rejected it.
- `ReviewAccepted`: postdoc accepted the output for integration.

These states should be task/result states, not provider allowlist states.

## Handling Weak Or Hallucinating Providers

If a provider says "I created the file" but the runtime sees no file, no diff,
and no tool evidence, that should be recorded as task failure or
`NoEffectiveOutput`.

The provider should not be banned by name. The failed result becomes evidence
for the postdoc. The postdoc can then:

- retry with clearer instructions;
- narrow the task;
- create a smaller validation target;
- ask the professor for a meeting if repeated failures suggest the project plan
  needs adjustment;
- report that the current provider/model is not practically effective for this
  task class.

This keeps the product honest without hardcoding provider prejudice into the
runtime.

## Unattended Auto LabRun Decision

Do not add a separate long-running unattended auto LabRun policy for now.

The earlier idea was to make long unattended runs stricter, with stronger
task-level evidence and failure-count limits. That sounds reasonable in
principle, but it creates another execution mode that is hard to define and hard
to validate:

- "long unattended" is ambiguous;
- different projects need different risk tolerance;
- extra policy branches increase scheduler and recovery complexity;
- stricter mode can silently diverge from normal LabRun behavior;
- it may push the runtime back toward over-controlling the agents.

Current decision: keep one LabRun execution policy. Use the same evidence,
failure accounting, pause/resume, postdoc review, and professor escalation rules
for all LabRun execution.

If a future product need appears, unattended behavior should be added as an
operator setting or project policy only after the basic LabRun loop is stable.
It should still not block by provider name.

## Graduate Branch Lifecycle

If graduate agents create isolated branches or worktrees, cleanup must be part
of the normal LabRun lifecycle. Otherwise long-running LabRuns will accumulate
stale branches, stale worktrees, and stale task artifacts until the repository
becomes hard to inspect.

Recommended policy:

1. Each graduate task gets a deterministic branch/worktree identity derived from
   LabRun ID, task ID, and dispatch ID.
2. The graduate branch is never merged directly by the graduate agent.
3. Postdoc review decides one of three outcomes: accept, reject, or revise.
4. On accept, the postdoc/runtime integration path merges or applies the
   reviewed changes, records the merge/apply evidence, then deletes the graduate
   branch and removes the isolated worktree.
5. On reject, the branch/worktree is retained only long enough to support
   inspection, report generation, and possible rollback evidence. After that it
   should be deleted or marked for cleanup.
6. On revise, a new graduate task should usually get a new branch/worktree
   instead of repeatedly mutating an old failed branch. The old branch can be
   retained as evidence until the revision chain is closed.

The runtime should expose cleanup status explicitly:

- `cleanup_pending`: review is complete but branch/worktree still exists;
- `cleanup_done`: branch/worktree was removed after merge, reject, or archive;
- `cleanup_blocked`: deletion failed or branch still contains unmerged evidence
  that a reviewer chose to keep.

This should not become a hidden background best effort only. The LabRun review,
dashboard, and recovery views should show stale graduate branches/worktrees so
the user can see when the system needs cleanup.

## What Should Change From Current Direction

The existing certification machinery should be reframed.

Keep provider diagnostics:

- `/lab provider compare`;
- live validation scripts;
- tool-use diagnostics;
- durable evidence records;
- provider/model behavior reports.

But do not let a "graduate passed" or "known unsupported" provider record be
the hard condition for launching a Lab graduate task.

Better naming:

- `provider diagnostics` instead of `provider certification`;
- `graduate evidence check` instead of `graduate provider gate`;
- `task evidence status` instead of `provider execution allowed`;
- `model behavior note` instead of `known unsupported`.

If old certification records remain for compatibility, they should be advisory
and diagnostic only.

## Runtime Boundary

The runtime should enforce:

- task scope;
- workspace isolation;
- permission boundaries;
- required validation command execution;
- evidence capture;
- failure accounting;
- no merge without review;
- no LabRun completion from graduate output alone.

The runtime should not enforce:

- provider allowlists;
- provider denylists;
- semantic code-quality approval;
- "this model is smart enough" judgments;
- direct promotion of graduate claims without observed proof.

## Postdoc Responsibility

The postdoc is the quality gate.

The postdoc should review:

- the actual diff;
- required validation results;
- changed file scope;
- whether the implementation matches the task;
- whether the result advances the professor plan;
- whether more graduate tasks are needed.

If the graduate result is weak, the postdoc should write a concrete follow-up
task. If repeated weak results show the plan is wrong, the postdoc should
escalate to professor review or a Lab meeting.

## Professor Escalation Logic

The professor agent should not micromanage code, but it should prevent the
postdoc/graduate loop from digging deeper into a bad direction.

Professor intervention should be event-triggered, not continuous supervision.
The professor should not constantly watch the LabRun or interrupt normal
postdoc/graduate work. The default path remains:

```text
Graduate executes task
-> Postdoc reviews result
-> accepted: integrate
-> rejected: revise, retry, or write blocker
-> strategic concern: escalate to professor
```

There are four escalation entry points:

1. Postdoc-initiated escalation.
2. Runtime signal reminder.
3. Mandatory professor checkpoint.
4. User-initiated professor review or Lab meeting.

Postdoc-initiated escalation is the primary path. The postdoc has the semantic
responsibility to decide whether repeated implementation trouble is still a
normal coding/debugging problem or whether it suggests a wrong direction,
unclear goal, bad architecture, or poor task decomposition. That judgment should
come from the postdoc prompt and review contract, not from framework code trying
to understand software quality.

The runtime can still record factual signals and surface reminders. These are
not strategic judgments and must not be treated as proof that the professor must
intervene. They are counters and state facts that help the postdoc notice
patterns:

- repeated graduate failures on the same task family;
- repeated postdoc rejections with similar reasons;
- validation failures that survive multiple repair tasks;
- rising LabRun failure count without meaningful artifact progress;
- multiple cycles spent in the same stage without accepted integration;
- a blocker report that says the current plan or architecture may be wrong;
- user sponsor feedback that changes the project goal or priority;
- cost/context growth that is high relative to progress.

The runtime can present these as `escalation_signals` or
`professor_review_suggested`, but it should not automatically conclude
`professor_review_required` unless there is a hard product rule such as user
request, exhausted failure budget, or explicit postdoc blocker report.

Postdoc-initiated escalation is not enough by itself. A postdoc can also get
path-dependent: it may keep accepting work that is locally coherent while the
overall direction is wrong. To handle that without making the framework pretend
to understand strategy, LabRun should add mandatory professor checkpoints.

Mandatory checkpoint triggers are institutional process rules, not framework
semantic judgments:

- before every user-facing closeout;
- at the end of each LabRun cycle before continuing into the next cycle;
- after a configured number of graduate tasks, such as every 3 completed or
  rejected graduate tasks;
- after a configured number of postdoc accepted results, such as every 2
  accepted integrations;
- after repeated context compression or high cost growth relative to user-visible
  progress;
- when failure budget is exhausted and the LabRun enters `NeedsUser`.

The important distinction:

```text
Do not: runtime decides "this direction is wrong."
Do: runtime decides "this is a required professor checkpoint."
```

This gives the professor a regular chance to catch strategic drift even when the
postdoc does not ask for help.

User-initiated escalation should always remain available. If the user clicks a
Lab meeting or professor review button, the professor receives the current
evidence packet regardless of runtime counters.

Recommended flow:

1. Runtime records factual signals and makes them visible to the postdoc.
2. Postdoc reviews task results and decides whether this is ordinary repair work
   or a strategic concern.
3. If strategic, postdoc writes a concise `ProfessorEscalationRequest` or
   blocker/drift report with concrete evidence.
4. Professor reviews the report against the original proposal, professor plan,
   current artifacts, and project direction.
5. Professor chooses one of these steering decisions:
   - continue current plan;
   - narrow scope;
   - change implementation strategy;
   - open a Lab meeting;
   - ask the user for a sponsor decision;
   - pause or close the LabRun as not worth continuing.
6. If the decision changes work, postdoc converts it into new tasks or a revised
   plan. Graduate agents still receive only narrow task envelopes.

This keeps the professor useful at the right level: direction, tradeoffs,
architecture, and project viability. The professor should not replace postdoc
code review or graduate implementation.

Professor guidance must be targeted to the postdoc's actual situation. It should
not give broad, disconnected advice or switch domains without evidence. For
example, if the postdoc is blocked on a backend architecture problem, the
professor should not respond with generic frontend guidance. The professor must
anchor its steering decision to:

- the current LabRun stage;
- the original proposal and professor plan;
- the postdoc's concrete blocker, integration summary, or review notes;
- graduate task evidence, changed files, validation results, and remaining
  risks;
- the specific tradeoff the postdoc is facing.

Good professor guidance should say, in effect:

- what current direction appears risky or stale;
- what evidence supports that concern;
- what alternative direction or narrower scope should be tried next;
- what the postdoc should convert into tasks;
- what should not be changed because it is outside the current problem.

Bad professor guidance is broad, generic, or orthogonal to the current blocker.
The professor prompt should explicitly reject that style.

This point is important enough to treat as a hard review contract for professor
outputs. The professor does not need to inspect every code detail like the
postdoc, but it must read the postdoc's actual trouble report before steering.
The professor should answer the problem the postdoc is facing, not a generic
project-management question.

Examples:

- If the postdoc is blocked on backend architecture, the professor should judge
  backend direction, scope, boundary placement, data flow, or risk tradeoffs.
- If the postdoc is stuck because validation keeps failing, the professor should
  decide whether to narrow the task, change the implementation strategy, request
  a smaller experiment, or stop that line of work.
- If the postdoc has local progress but no project-level progress, the professor
  should judge whether the loop is optimizing the wrong subproblem.
- If the postdoc reports uncertainty about product direction, the professor can
  ask the user for a sponsor decision.
- If the current blocker is not about frontend, UX, or presentation, professor
  output should not drift into frontend guidance unless the evidence packet
  explains why that domain is now relevant.

The runtime should help by assembling a professor checkpoint packet with:

- the current stage and owner;
- the last professor plan;
- the latest postdoc blocker, integration summary, or rejection reason;
- recent graduate task outcomes and validation evidence;
- changed-file scope;
- cost/context/compression signals;
- the exact question the professor is being asked to decide.

The professor prompt should require a direct answer to that question before any
optional broader observations. This keeps professor guidance directional but not
empty: it should be high-level relative to code, yet specific to the postdoc's
current bottleneck.

Lab meetings can be the heavier form of professor intervention. They should be
opened when the issue is not just "one task failed" but "the current direction
may be wrong." In a Lab meeting, professor and postdoc can run in parallel:
professor evaluates direction and risk, while postdoc evaluates implementation
facts and evidence. The result should be a steering artifact, not direct edits.

## Current Code Audit Notes

The current LabRun implementation still has several places where framework code
does too much role judgment. These appear to be historical scaffolding from
building the LabRun skeleton and later provider-certification experiments.

These should be treated as cleanup targets before LabRun is considered aligned
with the professor-postdoc-graduate boundary.

### 1. Provider-level graduate execution gate

Current code:

- `src/lab/provider_certification.rs` still classifies some provider/model
  combinations as `known_unsupported`.
- `src/lab/orchestrator.rs` still calls
  `validate_graduate_provider_for_execution()` before launching a graduate task.
- `/lab provider` and `/lab provider compare` still expose
  "Graduate certification" and "Graduate execution allowed" as if provider
  identity determines whether graduate execution may run.

Problem:

This is provider-level judgment. It conflicts with the new policy that all
providers should enter the same task-level evidence path. A weak provider should
produce weak or missing evidence, not be blocked by name before execution.

Desired direction:

- Keep provider compare, live validation, and tool diagnostics.
- Rename certification surfaces to diagnostics/evidence surfaces.
- Remove provider-name hard blocking from graduate dispatch.
- Let task evidence, scope checks, validation proof, and postdoc review decide
  whether the graduate result is useful.

### 2. Runtime tick writes role artifacts and advances stages

Current code:

- `LabOrchestrator::tick_latest()` creates a "Runtime tick artifact" for the
  current stage and then advances the LabRun.
- `create_current_stage_artifact_for_latest()` builds placeholder
  `ProfessorPlan`, `PostdocPlan`, `ProfessorReview`, and related artifacts.
- `write_satisfied_gate_for_latest()` immediately validates and satisfies the
  current stage gate.

Problem:

This lets framework code impersonate professor/postdoc output. It was useful as
an early deterministic skeleton, but it is not correct for the real LabRun
workflow. A runtime placeholder should not satisfy a professor or postdoc gate
as if an agent made the judgment.

Desired direction:

- Keep deterministic artifact creation only as a dev/test/debug path, or mark
  artifacts as `runtime_placeholder_not_agent_reviewed`.
- Do not let runtime placeholder artifacts satisfy strategic or postdoc gates in
  production LabRun flow.
- The main path should require provider-backed professor/postdoc artifacts or
  explicit human/user-authored artifacts before stage advancement.

### 3. Scheduler auto-generates postdoc/professor review artifacts

Current code:

- `run_scheduler_step_latest_with_context()` automatically calls
  `create_postdoc_integration_summary_for_latest()` in `postdoc_review`.
- The same scheduler path automatically calls
  `create_professor_review_for_latest()` in `professor_review`.
- If the generated gate is satisfied, the scheduler advances the LabRun.

Problem:

This makes the scheduler act like the postdoc or professor. The scheduler should
orchestrate steps, not decide implementation quality or strategic readiness.

Desired direction:

- Scheduler may stop and request the relevant role artifact.
- Scheduler may call provider-backed role steps when explicitly running in
  provider/hybrid mode.
- Scheduler should not silently create accepted postdoc/professor artifacts from
  deterministic templates.

### 4. Deterministic professor review auto-accepts based on fields

Current code:

- `create_professor_review_for_latest()` sets `accepted = true` when the postdoc
  integration summary is not `needs_revision` and has accepted results.

Problem:

That is a semantic professor decision encoded in framework logic. The runtime can
verify that fields exist, but it cannot decide that the project is strategically
ready for closeout.

Desired direction:

- Provider professor review should make the strategic acceptance decision.
- Runtime can enforce evidence boundaries after the professor decision, such as
  rejecting overclaims when postdoc evidence is missing.
- Deterministic professor review should be renamed or limited to a
  `not_verified` placeholder that cannot close out the LabRun.

### 5. Meeting recommendation can look like professor judgment

Current code:

- `meeting_recommendation_for_latest()` turns blocked tasks, repeated dispatch
  failures, and failure-budget signals into `recommended=true`.
- `create_meeting_request_for_latest()` writes a "Professor-triggered meeting
  request" from those runtime signals.

Problem:

The signals are useful, but the wording and state shape make runtime counters
look like professor intent. This risks turning advisory telemetry into
authority.

Desired direction:

- Rename this surface to `runtime_escalation_signals` or equivalent.
- Present signals to the postdoc and user as advisory context.
- Let the postdoc create `ProfessorEscalationRequest`, or let the user manually
  open a Lab meeting.
- Keep hard stops such as exhausted failure budget as safety states, but do not
  describe them as professor judgment.

## What Is Still Correct

The following parts are deterministic boundaries and should remain hard runtime
checks:

- graduate changed-file detection;
- `allowed_scope` enforcement;
- required validation command execution and exit-status capture;
- worktree/session evidence capture;
- artifact schema, stage, owner, and LabRun ID validation;
- failure budget safety stop to `NeedsUser`;
- no merge or closeout from graduate claims alone.

These are not framework "smart judgment." They are mechanical evidence and
safety contracts.

## Implementation Guidance

Recommended next implementation slice:

1. Replace provider-certification hard blocks in Lab graduate dispatch with a
   provider-neutral task evidence path.
2. Keep `/lab provider compare` and live validation as diagnostics.
3. Change user-facing labels from certification/unsupported language to
   evidence/diagnostic language.
4. Ensure failed or empty graduate runs produce durable task evidence rather
   than disappearing behind a pre-execution block.
5. Route every graduate result through postdoc review before integration.
6. Add graduate branch/worktree cleanup states to review, dashboard, and
   recovery surfaces.
7. Add postdoc prompt/review-contract language requiring the postdoc to consider
   professor escalation when repeated failures, stalled stages, blocker reports,
   sponsor feedback, or poor progress-to-cost ratio suggest strategic drift.
8. Add runtime-visible escalation signals as advisory context only, not as a
   semantic decision engine.
9. Demote deterministic `tick_latest()` placeholder artifacts so they cannot
   satisfy professor/postdoc gates in production LabRun flow.
10. Make scheduler stop at postdoc/professor review boundaries unless a
   provider-backed role step or explicit user-authored artifact is available.
11. Remove deterministic professor auto-acceptance from closeout-capable paths.
12. Rename meeting recommendation surfaces so runtime counters are presented as
   advisory escalation signals, not professor intent.
13. Add mandatory professor checkpoints at cycle boundaries, closeout,
   configured graduate-task/postdoc-acceptance intervals, high cost/context
   growth, and failure-budget exhaustion.
14. Update professor prompts so checkpoint guidance must stay anchored to the
   current stage, postdoc blocker, evidence, changed files, validation results,
   and specific tradeoff under review.
15. Update the main LabRun plan and project status after the code behavior
   changes.

Current implementation status:

- Done: provider-name hard blocks have been demoted to diagnostics; graduate
  dispatch follows the same task-evidence path for every provider.
- Done: provider-facing command text now uses diagnostics/evidence language
  rather than certification-as-execution-policy language.
- Done: scheduler and tick paths stop at professor/postdoc role boundaries
  unless an explicit role artifact is available; they no longer silently create
  accepted role artifacts from runtime placeholders.
- Done: deterministic professor review cannot auto-accept closeout and instead
  produces revision work when used as a placeholder path.
- Done: runtime meeting recommendations are surfaced as runtime escalation
  signals, not professor intent.
- Done: mandatory professor-checkpoint signals cover closeout, cycle boundary,
  graduate task interval, postdoc acceptance interval, high cost/context growth,
  repeated compression, and failure-budget exhaustion.
- Done: professor prompts require guidance to stay anchored to the current
  blocker, evidence, changed files, validation results, and exact tradeoff.
- Done: postdoc prompt/handoff language requires the postdoc to consider
  professor escalation when repeated failures, stalled stages, blocker reports,
  sponsor feedback, or poor progress-to-cost ratio suggest strategic drift.
- Done: graduate dispatch records persist cleanup status
  (`cleanup_pending`, `cleanup_done`, `cleanup_blocked`) and review, dashboard,
  and recovery surfaces expose that cleanup state.
- Done: `docs/LAB_AGENT_WORKFLOW_PLAN_2026-06-18.md` and
  `docs/PROJECT_STATUS.md` have been updated to reflect the current behavior.
- Still future polish: rename the remaining compatibility storage/API names that
  still contain "provider certification" internally. These are historical
  diagnostic record names now, not execution gates.

This keeps the LabRun idea closer to the original professor-postdoc-graduate
workflow while preserving the project's hard runtime boundaries.
