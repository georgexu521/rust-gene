# Top-Level Agent Capability Review

Date: 2026-05-03

## Executive Diagnosis

The project can already complete real small programming tasks. The latest live
capability batch passed a backend CRUD implementation and a frontend
localStorage UI task through model-led inspection, edit, validation, and
closeout. The current failures are not all the same kind of failure.

The main top-level issue is measurement drift:

- Some live eval cases are true seeded coding tasks. They create a gap in an
  isolated worktree, then require the agent to inspect, edit, validate, and
  close out.
- Some current high-value specialty cases use `base_ref: HEAD` with no fixture
  mutation. Once the real project already contains the target behavior, a fresh
  agent run can pass every required command without producing a diff.
- The quality gate correctly rejects no-diff code-change runs, but the report
  currently tends to blame `agent_flow` even when the baseline is already
  satisfied.

That creates a bad feedback loop: we may over-optimize the runtime to force an
edit where no edit is needed, or reintroduce hidden patch synthesis to satisfy a
stale benchmark. This would violate the desired product boundary: the system
should tell the LLM how to work, not secretly make implementation decisions for
it.

## Current Strengths

- Real coding loop exists: read, edit, validate, repair, acceptance review, and
  closeout are wired and observable.
- The distinctive systems are visible in reports: memory/retrieval, validation
  automation, guided debugging, weighted planning, and closeout signals.
- False success is mostly blocked: failed validation and rejected acceptance now
  keep closeout from claiming success.
- Patch synthesis is behind explicit opt-in flags and is no longer default
  runtime behavior.

## Current Weaknesses

- Eval taxonomy is under-specified. `bug_fix` currently mixes seeded missing
  behavior, stale already-fixed behavior, and audit-style safety verification.
- `base_ref: HEAD` plus no `prepare_commands` is unsafe for a code-change eval:
  it may stop being a code-change task after normal development catches up.
- Failure ownership is too coarse for stale evals. A no-diff, tests-pass,
  current-HEAD task should be classified as eval baseline/freshness risk, not
  automatically as agent flow.
- Some workflow guidance still over-emphasizes "make an edit" even when the
  evidence says the requested behavior is already present. For true coding
  benchmarks, the fixture should force the gap; for audit tasks, no diff should
  be allowed when evidence is strong.
- Deterministic task-specific repair helpers are still present in code, though
  gated. They should not expand. The long-term solution is better prompts,
  evidence, tools, and eval fixtures.

## Design Boundary

The right direction is:

- Software provides structure, safety, evidence, and review.
- The LLM owns diagnosis, implementation choices, and code edits.
- The harness owns whether a case is a seeded code-change task, an audit task,
  or stale.
- Hidden runtime patching remains opt-in only.

The wrong direction is:

- More benchmark-specific deterministic patch rules.
- Forcing arbitrary edits to satisfy stale no-diff cases.
- Treating every failed report as a model coding failure without checking the
  baseline.

## Proposed Solution

### 1. Split Eval Case Intent

Every live eval should be treated as one of these:

- `seeded_code_change`: fixture or base ref is known to lack required behavior;
  no diff is a failure.
- `audit_or_regression_check`: current behavior may already be correct; no diff
  can pass if required commands and evidence pass.
- `stale_or_already_satisfied`: a historical code-change case whose current
  baseline already satisfies required behavior; do not count it as coding
  failure until refreshed.

### 2. Add Baseline Freshness Signals

The runner should identify the risky shape:

- task type is `bug_fix`, `feature`, or `refactor`;
- `repo.base_ref` resolves from `HEAD`;
- no `repo.prepare_commands` are present;
- the agent produced no diff;
- required commands pass.

That should be reported as baseline freshness risk and attributed to
`eval_harness`, even if the agent closeout is imperfect. The run can still be
`failed` for no successful closeout, but it should not be used as evidence that
the agent cannot code.

### 3. Refresh Or Reclassify Stale Cases

For `memory-recall-conflict-precision` and
`permission-default-open-dangerous-guard`, choose one path:

- Convert to `seeded_code_change` by adding fixture mutation that removes the
  target behavior before the run.
- Convert to `audit_or_regression_check` and accept no-diff success when the
  required commands and static evidence pass.
- Retire them from the coding-capability score and keep them only as regression
  audits.

### 4. Keep Runtime Changes Prompt-First

Runtime should keep:

- action checkpoints,
- tool failure visibility,
- validation and acceptance gates,
- stale read protection,
- memory/retrieval provenance,
- workflow trace reporting.

Runtime should avoid:

- hidden auto-implementation,
- task-specific deterministic code patches,
- expanding patch synthesis as normal coding behavior.

### 5. Next Work Order

1. Record this top-level review and fix stale-eval failure ownership in
   `scripts/run_live_eval.sh`.
2. Add explicit eval taxonomy fields to the live task YAML files.
3. Refresh the two stale specialty cases with real fixture mutations or move
   them to audit mode.
4. Run the remaining product-surface tasks:
   `resume-session-picker` and `cli-scrollback-polish`.
5. Build a compact dashboard summary so pass/fail/stale/environment/LLM
   ownership is visible across runs.

## Immediate Acceptance

- A stale current-HEAD no-diff run with passing required commands is not
  reported as a pure `agent_flow` coding failure.
- Seeded tasks still require code diff and validation.
- Patch synthesis remains disabled by default.
- The evaluation plan clearly separates coding ability from benchmark freshness.

## Implementation Update

The live eval matrix now uses an explicit `eval_intent` field:

- `seeded_code_change`: a real code-change task where a diff is required.
- `audit_or_regression_check`: an audit/regression task where no diff can be
  acceptable if the agent proves the current behavior and required commands pass.
- `stale_or_already_satisfied`: reserved for historical cases that should not
  count as fresh coding failures until their baseline is refreshed.

Current classification:

- `seeded_code_change`: backend/frontend fixture tasks, verification repair,
  memory quality gate, persistent memory planning, skill promotion, resume
  picker, CLI scrollback, and dashboard summary.
- `audit_or_regression_check`: memory conflict precision, duplicate memory
  demotion, sensitive memory hard block, and default-open permission guard.

The runner now prints `eval_intent`, includes it in generated agent prompts, and
uses it for no-diff quality decisions instead of relying only on `base_ref`.

## Dashboard Summary Run

Run:

`docs/benchmarks/live-capability-dashboard-summary-20260503-213148/live-eval-dashboard-summary/report.md`

Result:

- Status: failed.
- Failure owner: `llm_reasoning`.
- Eval intent: `seeded_code_change`.
- Files changed: none.
- Closeout: `not_verified`.
- Specialty signals: 4/6 active.
- Required commands: failed because `scripts/run_live_eval.sh --list` could not
  import PyYAML inside the isolated worktree; full Rust tests still passed.

Interpretation:

- The eval taxonomy did its job: this was a seeded code-change case, so no diff
  was correctly treated as failure.
- The action checkpoint did its job: it blocked false success instead of
  claiming the dashboard feature was complete.
- The model did not do the implementation. It repeatedly inspected
  `scripts/run_live_eval.sh` and task metadata, then stalled without a
  `file_edit`.
- The next concrete project improvement should be a human-led implementation of
  summary generation plus removal or vendoring of the PyYAML dependency for
  `--list`, not a hidden runtime patch.

Follow-up implemented:

- `scripts/run_live_eval.sh --list` now uses a lightweight stdlib parser for
  top-level task metadata and no longer requires PyYAML.
- `scripts/run_live_eval.sh --mode summary --run-id <id>` writes
  `docs/benchmarks/live-<run-id>/summary.md`.
- Summary rows include pass/fail status, `eval_intent`, `failure_owner`,
  required-command status, plan/tool boundary, verification status, closeout,
  first write index, diff presence, and warnings.
- The dashboard summary live task now has `prepare_commands` that remove summary
  support from the fixture worktree, so it remains a true seeded code-change
  eval after the feature lands.

Rerun:

`docs/benchmarks/live-capability-dashboard-summary-rerun-20260503-235256/live-eval-dashboard-summary/report.md`

- Status: failed.
- Failure owner: `llm_reasoning`.
- The seeded fixture worked: `--mode summary` was absent in the worktree and the
  required summary command failed.
- The model again produced no `file_edit`.
- This confirms the current weakness is not stale eval design for this case; it
  is the model-led edit transition on a medium script feature.
