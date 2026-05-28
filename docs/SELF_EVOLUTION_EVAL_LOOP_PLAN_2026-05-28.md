# Self-evolution Eval Loop Plan

Date: 2026-05-28

Owner context: Priority Agent has just landed the Hermes-inspired memory
lifecycle work. The next phase should shift from building more memory
architecture to proving that the agent can improve its own coding behavior
through evidence, evals, explicit review, and rollback.

The goal is not to let the model rewrite its own behavior freely. The goal is
to make runtime learning produce inspectable proposals, bind those proposals to
real evalsets, apply only when the evidence is strong, and measure whether the
change helped later runs.

## Executive Decision

The main product direction should shift from memory architecture expansion to a
self-evolution eval loop:

```text
learning event -> improvement proposal -> evalset binding -> eval -> accept/apply -> rollback -> effect tracking
```

The first vertical slice should focus on `tool_guidance` and `workflow_guidance`
rather than general prompt rewrites or autonomous skill marketplace behavior.
These are the safest and highest-value targets because they map directly to the
agent's known failure modes:

- wrong or missing tool use;
- weak validation;
- poor closeout evidence;
- repeated permission/recovery mistakes;
- memory/retrieval misuse during coding tasks.

## Current Baseline

Already implemented:

- `ImprovementProposal` and `ImprovementStore`;
- proposal lifecycle: proposed, accepted, rejected, applied, rolled back;
- eval status: pending, passed, failed;
- evalset bindings on improvement proposals;
- apply blocked until eval passes;
- rollback state and audit refs;
- `/improvements list|scan|show|bind-eval|eval|accept|reject|apply|rollback`;
- `SkillProposalStore`, skill quality reports, fitness snapshots, promotion
  gate, version records, and skill rollback paths;
- `/skill-proposals` review/apply/rollback surfaces;
- memory eval suite and memory lifecycle observability.

First batch changes:

- first implementation batch now makes eval binding a hard apply gate;
- `apply` writes an explicit applied-guidance artifact instead of only changing
  proposal state;
- active guidance has a local registry, runtime injection surface, and trace
  event;
- effect records can now attach positive/neutral/negative outcomes to applied
  proposals and recommend rollback after repeated regressions;
- `/improvements` now exposes active guidance, effect summaries, manual effect
  recording, deactivation, and doctor output;
- `/evolution status` now summarizes improvement and skill evolution in one
  lifecycle panel.

Remaining weakness:

- eval command metadata is still lightweight (`run_id` plus formatted evalset
  result), not a full persisted eval artifact bundle;
- effect tracking is manual/command-driven first; automatic live-eval import is
  still a follow-up;
- skill evolution and improvement evolution share lifecycle concepts in UX, but
  their storage models are still separate.

## Implementation Status: 2026-05-28

Implemented in the first self-evolution eval-loop batch:

- `AppliedGuidanceStore` backed by local JSONL next to `improvements.jsonl`;
- `AppliedGuidanceRecord`, `GuidanceScope`, `GuidanceActivation`, and
  `AppliedGuidanceStatus`;
- `ImprovementEffectStore`, `ImprovementEffectRecord`, and effect summaries;
- apply gate requiring both passed eval and at least one bound evalset;
- apply/rollback integration that activates and deactivates guidance records;
- bounded `<self-evolution-guidance>` runtime context for matching active
  guidance;
- trace event `self_evolution.guidance` with record count, character count, and
  provenance;
- `/improvements active|doctor|effect|record-effect|deactivate`;
- `/improvements show` includes applied guidance and effect state;
- `/evolution status` includes active guidance, missing evalset blockers,
  failed evals, rollback recommendations, and skill proposal status;
- tests for evalset gating, active guidance rendering, duplicate apply
  idempotency, corrupted registry input, rollback deactivation, effect
  rollback recommendation, and slash-panel formatting.

## Design Principles

1. Proposal-first, never direct mutation.
   Runtime learning may propose behavior changes, but it must not silently alter
   prompts, tool contracts, memory rules, or skills.

2. Eval-bound apply.
   A proposal cannot be applied unless it has at least one bound evalset and the
   bound eval passes. For high-risk targets, require multiple evalsets.

3. Applied guidance is explicit state.
   Applying an improvement should write to a small, typed, inspectable registry,
   not to a hidden prompt string.

4. Rollback must restore previous behavior.
   Rollback must remove or deactivate the applied guidance artifact and preserve
   audit history.

5. Effect tracking is part of the feature.
   A successful apply is not the end. The system must track whether later tasks
   improve, regress, or stay neutral.

6. Framework versus LLM failure stays explicit.
   If an eval fails because the proposal bypassed gates, that is framework
   failure. If the proposal is bad but blocked by eval/gate, the framework did
   its job.

## Target User Experience

The user-facing loop should feel like this:

```text
/improvements scan
/improvements show imp_...
/improvements bind-eval imp_... runtime-spine-p0b-skill-guidance
/improvements eval imp_...
/improvements accept imp_...
/improvements apply imp_...
/improvements effect imp_...
/improvements rollback imp_...
```

The user should be able to answer:

- What behavior is the agent trying to improve?
- What evidence caused the proposal?
- Which evalsets prove it is safe?
- What artifact will be applied?
- What runtime surface does it affect?
- How do we roll it back?
- Did later tasks improve after applying it?

## MVP Scope

### In Scope

- `ImprovementTarget::ToolGuidance`;
- `ImprovementTarget::Routing`;
- narrow workflow guidance for validation/closeout/tool selection;
- an applied guidance registry under `.priority-agent` or the existing local
  runtime state root;
- binding existing evalsets to improvement proposals;
- deterministic tests for proposal/eval/apply/rollback/effect tracking;
- doctor/status output for active self-evolution state.

### Out Of Scope For The First Slice

- autonomous prompt rewriting;
- external provider write-mirroring;
- social/idle heartbeat;
- unreviewed skill activation;
- broad skill marketplace behavior;
- gateway/cron multi-platform self-evolution.

## Applied Guidance Registry

Add a typed registry for active self-evolution changes.

Suggested record:

```rust
pub struct AppliedGuidanceRecord {
    pub id: String,
    pub proposal_id: String,
    pub target: ImprovementTarget,
    pub scope: GuidanceScope,
    pub content: String,
    pub activation: GuidanceActivation,
    pub evalsets: Vec<String>,
    pub applied_at: String,
    pub rollback_ref: Option<String>,
    pub status: AppliedGuidanceStatus,
}
```

Initial `GuidanceScope`:

- `global_runtime`;
- `project`;
- `route`;
- `tool`;
- `workflow`.

Initial `GuidanceActivation`:

- `diagnostic_only`;
- `prompt_context`;
- `tool_contract_hint`;
- `route_policy_hint`.

First implementation should keep activation conservative:

- `diagnostic_only` for most proposals;
- `tool_contract_hint` only for tool-guidance proposals with passing evals;
- no broad prompt rewrite in the MVP.

## First Vertical Slice

Build one complete self-evolution path:

1. Generate a proposal from learning events.
   Source examples:
   - repeated tool failures;
   - recovery plans;
   - user correction;
   - failed closeout verification;
   - memory eval failure ownership.

2. Bind to existing evalsets.
   Start with:
   - `runtime-spine-p0b-skill-guidance`;
   - `minimum-agent-verification-repair`;
   - `code-change-verification-repair-loop`;
   - `core-permission-rejection-recovery`;
   - `memory-recall-conflict-precision` for memory-related improvements.

3. Run eval.
   `eval` must produce:
   - passed/failed;
   - evalset names;
   - command/run id;
   - failure owner;
   - reason summary;
   - regression risk.

4. Accept and apply.
   Apply writes `AppliedGuidanceRecord`.
   It does not directly rewrite system prompts.

5. Inject active guidance through one explicit runtime surface.
   First target:
   - route/tool guidance context for the relevant target.

6. Rollback.
   Rollback marks the guidance inactive and records a rollback event.

7. Effect tracking.
   Later eval/live-eval summaries can attach before/after outcomes to the
   proposal.

## Milestones

### Milestone A: Applied Guidance Registry

Deliverables:

- `AppliedGuidanceRecord`;
- local JSONL store;
- list/get/apply/rollback APIs;
- `/improvements active`;
- doctor section for active applied guidance.

Definition of Done:

- applying an evaluated proposal writes one active guidance record;
- rollback marks it inactive;
- duplicate apply is idempotent;
- active guidance records are visible to doctor/status;
- tests cover apply, duplicate apply, rollback, and corrupted registry input.

Status: completed in the first implementation batch.

### Milestone B: Real Eval Binding

Deliverables:

- require at least one evalset binding before apply;
- store eval command/run metadata;
- show per-evalset pass/fail in `/improvements show`;
- classify failures as framework, LLM, test harness, or none.

Definition of Done:

- apply without evalset fails;
- apply with missing evalset fails;
- passing bound eval allows accept/apply;
- failed bound eval blocks apply;
- eval failure owner appears in review output.

Status: mostly completed. Apply now requires a bound evalset and a passed eval;
missing or failing evalsets block apply and expose failure owner. Full persisted
eval artifact bundles remain a follow-up.

### Milestone C: Tool-guidance Activation

Deliverables:

- convert applied `ToolGuidance` records into a bounded runtime hint;
- inject only for matching tool/route/workflow;
- include provenance and rollback id in trace/debug output.

Definition of Done:

- unrelated tool guidance is not injected;
- matching tool guidance appears in runtime trace;
- guidance has a strict char/token budget;
- guidance cannot override permissions, validation gates, or tool schemas.

Status: completed for the conservative first slice. Tool/workflow guidance is
bounded, matching-only, trace-visible, and explicitly marked as unable to
override user intent, permissions, validation gates, tool schemas, or safety
policy.

### Milestone D: Effect Tracking

Deliverables:

- `ImprovementEffectRecord`;
- attach later eval/live-eval outcomes to a proposal;
- show before/after trend in `/improvements effect <id>`;
- downgrade or recommend rollback after repeated regression.

Definition of Done:

- effect record includes proposal id, evalset, run id, outcome, failure owner,
  and timestamp;
- positive/negative/neutral effect summary is computed;
- repeated regression marks proposal as rollback recommended;
- doctor shows rollback-recommended improvements.

Status: completed for manual/local effect records. Automatic import from
live-eval summaries remains a follow-up.

### Milestone E: Skill Evolution Alignment

Deliverables:

- align improvement proposals and skill proposals around one lifecycle view;
- reuse fitness/eval concepts where possible;
- make `/self-evolution` or `/learning` panel summarize both.

Definition of Done:

- user can see improvements and skill proposals in one lifecycle panel;
- skill apply requires quality and promotion gate;
- improvement apply requires eval and guidance gate;
- rollback UX is consistent across both.

Status: partially completed. `/evolution status` now gives a shared lifecycle
panel for improvements and skill proposals. Storage and deeper fitness model
unification remain future work.

## Evalsets To Reuse First

Use existing evalsets before creating new ones:

- `evalsets/live_tasks/runtime-spine-p0b-skill-guidance.yaml`
- `evalsets/live_tasks/skill-promotion-gate.yaml`
- `evalsets/live_tasks/minimum-agent-verification-repair.yaml`
- `evalsets/live_tasks/code-change-verification-repair-loop.yaml`
- `evalsets/live_tasks/core-permission-rejection-recovery.yaml`
- `evalsets/live_tasks/memory-recall-conflict-precision.yaml`
- `evalsets/live_tasks/project-partner-resume-with-memory.yaml`
- `evalsets/tool_file_reliability_gauntlet.yaml`

Only add new evalsets when these cannot express the required assertion.

## Failure Ownership Policy

Framework failure:

- proposal applies without eval;
- failed eval does not block apply;
- applied guidance bypasses permissions or tool contracts;
- rollback does not deactivate guidance;
- effect tracking misses regression evidence;
- prompt/runtime trace cannot explain active guidance.

LLM failure handled correctly:

- model proposes a bad improvement but eval rejects it;
- model proposes broad prompt rewrite but scope/risk gate blocks it;
- model ignores active guidance, but trace proves it was injected correctly.

Test harness failure:

- evalset missing or malformed;
- live-eval artifact cannot be parsed;
- expected assertion is stale relative to product contract.

## Commands And Surfaces

Extend `/improvements`:

```text
/improvements active
/improvements effect <id>
/improvements deactivate <id>
/improvements doctor
```

Keep existing:

```text
/improvements scan
/improvements show <id>
/improvements bind-eval <id> <evalset>
/improvements eval <id>
/improvements accept <id>
/improvements apply <id>
/improvements rollback <id>
```

Doctor/status should show:

- active applied guidance count;
- rollback-recommended count;
- last eval result;
- proposals blocked by missing evalsets;
- proposals blocked by regression.

## Suggested First Implementation Batch

1. Add `AppliedGuidanceStore`.
2. Make improvement apply write an applied guidance record.
3. Make rollback deactivate that record.
4. Require at least one evalset binding before apply.
5. Add `/improvements active`.
6. Add tests for apply/rollback/idempotency.
7. Run:

```bash
cargo fmt --check
cargo check -q
cargo test -q improvement
cargo test -q skill_evolution
cargo test -q learning
cargo clippy --all-features -- -D warnings
```

Batch status: implemented. Current validation for the batch:

```bash
cargo fmt --check
cargo check -q
cargo test -q improvement
cargo test -q learning
```

Additional validation completed:

```bash
cargo test -q skill_evolution
cargo check --features experimental-api-server -q
cargo clippy --all-features -- -D warnings
git diff --check
```

Full `cargo test -q` reached 2067/2069 passing; the two failures were
`run_tests_tool` tests that passed when rerun individually and as a filtered
module (`cargo test -q run_tests_tool`), so the remaining issue appears to be
full-suite test isolation/flakiness outside this self-evolution batch.

## Success Criteria

The phase is successful when Priority Agent can demonstrate this with local
evidence:

1. A repeated failure creates an improvement proposal.
2. The proposal is bound to a real evalset.
3. The eval blocks bad changes and allows good changes.
4. Apply creates explicit active guidance.
5. Runtime traces show when guidance affected a turn.
6. Later evals show whether the change helped.
7. Rollback deactivates the guidance cleanly.

This is the Hermes lesson for self-evolution: self-improvement should be a
reviewed product loop, not a model silently rewriting its own behavior.
