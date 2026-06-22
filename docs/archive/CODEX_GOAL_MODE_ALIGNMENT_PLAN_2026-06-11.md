# Codex Goal Mode Alignment Plan
Status: Implemented MVP, hardening in progress

Last updated: 2026-06-11

## Goal

Bring Priority Agent's current lightweight goal tracking up to a Codex-style
Goal mode: a persistent objective that can drive multiple full-agent turns until
the objective is completed, paused, blocked, failed, or requires user input.

This plan is intentionally product-facing and implementation-facing. The next
phase should not add a second semantic agent loop. It should add a deterministic
outer runner that repeatedly invokes the existing full-agent runtime, reads
runtime evidence, and decides whether another bounded turn is justified.

## 2026-06-11 Review Adjustments

The first implementation was scoped around a small Phase 0-2 slice because
`AGENTS.md` says not to force heavyweight planning. The current implementation
has moved beyond that slice: Phases 0-7 now exist in code. The remaining work is
hardening, dogfood, integration coverage, and product polish rather than a
second goal-system design.

Review findings from the current repository:

- `apps/desktop/` does exist in this checkout (`apps/desktop/src`,
  `apps/desktop/src-tauri`, and `apps/desktop/tests`). Desktop work should not
  be deleted from the roadmap, but it should be deferred until the TUI/runtime
  path proves the model.
- Existing goal code is flat under `src/engine/session_goal.rs` and
  `src/engine/goal_drift.rs`. Do not move those files just to make a new
  hierarchy. Keep Phase 0 flat; create `src/engine/goal/` only when durable
  goal models, store helpers, decision logic, and runner logic would otherwise
  create mixed-responsibility files.
- `features` already exists in `src/services/config.rs` with `mcp_enabled`,
  `skills_enabled`, `web_search`, `llm_memory_extraction`, and
  `plugin_trust_mode`. Adding `features.goals` is a straightforward config
  extension, not a new configuration system.
- The scored improvement loop is implemented as an optional stop-rule path. Keep
  it optional until dogfood runs prove which score parsers and thresholds are
  useful.

## Public Codex Reference Points

The Codex manual helper could not fetch `codex-manual.md` in this environment
because the official endpoint returned HTTP 403. The reference points below are
from public OpenAI Codex documentation on `developers.openai.com`, checked on
2026-06-11.

- [Follow a goal](https://developers.openai.com/codex/use-cases/follow-goals):
  `/goal` is for a durable objective that keeps Codex working across turns
  toward a verifiable stopping condition. The intended time horizon is
  long-running work with clear success criteria and validation loops.
- [Prompting - Goal mode](https://developers.openai.com/codex/prompting):
  Goal mode gives Codex a persistent objective. The goal text acts as both the
  starting prompt and the completion criteria. Good goals include a specific
  outcome, measurable target, or test criteria. The app exposes progress above
  the composer with pause/resume/edit/clear controls.
- [CLI slash commands](https://developers.openai.com/codex/cli/slash-commands):
  `/goal <objective>` sets the goal, `/goal` views it, and `/goal pause`,
  `/goal resume`, and `/goal clear` manage it. Objectives must be non-empty and
  at most 4,000 characters; longer instructions should live in a file that the
  goal references.
- [Codex app commands](https://developers.openai.com/codex/app/commands):
  app Goal mode is a persistent objective that runs until Codex finishes,
  pauses, or needs more input. The app shows progress controls above the
  composer, and follow-up messages can steer the active goal.
- [Iterate on difficult problems](https://developers.openai.com/codex/use-cases/iterate-on-difficult-problems):
  hard tasks work best when Codex has a scoring/evaluation system and can keep
  improving until the score is good enough. This maps directly to our existing
  verification proof, eval, and closeout surfaces.
- [Sandboxing](https://developers.openai.com/codex/concepts/sandboxing):
  autonomy is safe only inside explicit sandbox and approval boundaries. When a
  task stays inside the boundary, the agent can keep moving; when it crosses
  the boundary, it must fall back to approval.

## Current Priority Agent State

### Already Implemented

- `SessionGoal` exists in `src/engine/session_goal.rs`, with `id`, `title`,
  `status`, `intent`, `workflow`, generated acceptance criteria, last user
  message, and updated time.
- `/goal` exists in TUI command handling. It can show, set, clear, and inspect
  drift through `src/tui/slash_handler/learning.rs`.
- Goal drift exists in `src/engine/goal_drift.rs` and is wired through
  tool-execution gating and trace events.
- `RuntimeController` is the shared full-agent turn boundary in
  `src/engine/runtime_controller.rs`. CLI, TUI, desktop, and API full-agent
  paths should converge through it or the underlying `StreamingQueryEngine`.
- `ConversationLoop` already runs the single-turn agent loop with tool calls,
  post-edit validation, repair, closeout, evidence ledger, and force-summary
  containment.
- `SessionRunCoordinator` exists in `src/engine/run_coordinator.rs`. It
  prevents concurrent runs per session and supports queued or steering inputs.
- `SessionStore` already has durable sessions, messages, session events,
  session inputs, session jobs, session parts, compact boundaries, trace store,
  todos, reverts, and export payloads.
- `ActiveTaskPlan` can derive a user-facing progress view from the current goal,
  trace, closeout, memory proposal, and project progress.
- `apps/desktop/` exists and desktop already renders session parts, closeout
  events, trace details, and a current session panel. The desktop bridge is
  close enough to host the goal progress row without inventing a new runtime.
- `features.goals` exists in the normal config feature block.
- Durable `GoalRun`/`GoalStep` storage exists through the v17 migration and
  `SessionStore` goal helpers.
- `GoalDecisionEngine` exists and screens closeout/proof/blocker/budget/score
  evidence deterministically.
- `GoalRunner` exists and drives start, pause, resume, clear, status, step
  recording, and automatic continuation prompts.
- `/goal <objective>`, `/goal status`, `/goal pause`, `/goal resume`,
  `/goal clear`, `/goal edit`, and `/goal log` are wired in TUI.
- Desktop exposes a compact goal progress row and goal commands over the Tauri
  bridge.
- Export/readiness include goal information.

### Gaps

- Goal mode still needs broader integration coverage across real TUI and desktop
  runs, especially user steering while an automatic continuation is queued.
- Desktop start/resume should keep using the existing run event/watchdog path;
  avoid adding a parallel desktop-only agent loop.
- Restart behavior should remain conservative: active goals are paused or made
  explicit, never silently auto-resumed.
- The scored improvement loop needs dogfood data before adding more parser types
  or product promises.
- Export/readiness should stay factual and evidence-backed; do not report a goal
  as ready unless the durable store and latest step evidence are queryable.

## Target Product Semantics

Priority Agent should implement the same user-facing shape as Codex:

```text
/goal <objective>      Start or replace the active goal.
/goal                  Show active goal, status, budget, latest step, and blocker.
/goal pause            Pause automatic continuation.
/goal resume           Resume automatic continuation.
/goal clear            Clear the active goal.
/goal edit <text>      Replace the objective while preserving run history.
/goal log [limit]      Show recent goal steps.
```

`/goal set <text>` should remain as a compatibility alias for `/goal <text>`.

Goal objectives should be:

- non-empty;
- capped at 4,000 characters for Codex compatibility;
- allowed to reference files for longer instructions;
- stored as both the first turn objective and the completion criteria;
- rendered in TUI, desktop, trace, export, and readiness surfaces.

## Target Architecture

### Deterministic Outer Runner

Module boundary decision:

```text
Phase 0:
  keep current flat files:
    src/engine/session_goal.rs
    src/engine/goal_drift.rs

Phase 1+ if responsibilities grow:
  src/engine/goal/
    mod.rs
    model.rs
    store.rs
    decision.rs
    runner.rs
    prompt.rs
    events.rs
```

Do not perform a cosmetic move before the durable model exists. If the
subdirectory is introduced, keep compatibility exports so existing callers do
not churn in the same change.

The runner owns scheduling and state transitions only. It must not own semantic
engineering judgment. The LLM still decides approach, code reasoning, repair
strategy, and whether the work appears done. The deterministic runner decides
whether the latest turn has enough evidence to stop, continue, pause, or ask.

### Core Types

```rust
pub enum GoalRunStatus {
    Active,
    Paused,
    Completed,
    Blocked,
    Failed,
    NeedsUser,
    Cancelled,
}

pub struct GoalRun {
    pub id: String,
    pub session_id: String,
    pub objective: String,
    pub status: GoalRunStatus,
    pub stop_rules: GoalStopRules,
    pub budget: GoalBudget,
    pub turn_count: u32,
    pub created_at: String,
    pub updated_at: String,
    pub last_closeout_status: Option<String>,
    pub last_blocker: Option<String>,
}

pub struct GoalStep {
    pub id: String,
    pub goal_id: String,
    pub session_id: String,
    pub turn_index: u32,
    pub prompt: String,
    pub closeout_status: Option<String>,
    pub verification_status: Option<String>,
    pub changed_files: usize,
    pub validation_items: usize,
    pub decision: GoalDecision,
    pub summary: String,
    pub created_at: String,
}

pub struct GoalBudget {
    pub max_turns: u32,
    pub max_minutes: u32,
    pub max_tokens: Option<u64>,
    pub max_repeated_blockers: u32,
}

pub struct GoalStopRules {
    pub validation_commands: Vec<String>,
    pub success_markers: Vec<String>,
    pub require_clean_worktree: bool,
    pub require_verified_closeout: bool,
}

pub enum GoalDecision {
    Continue,
    Complete,
    Pause,
    NeedsUser,
    Blocked,
    Failed,
}
```

### Persistence

Add a new migration after the current session-store migrations:

```sql
CREATE TABLE IF NOT EXISTS goal_runs (
  id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL,
  objective TEXT NOT NULL,
  status TEXT NOT NULL,
  stop_rules_json TEXT,
  budget_json TEXT,
  turn_count INTEGER NOT NULL DEFAULT 0,
  last_closeout_status TEXT,
  last_blocker TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now')),
  FOREIGN KEY (session_id) REFERENCES sessions(id)
);

CREATE TABLE IF NOT EXISTS goal_steps (
  id TEXT PRIMARY KEY,
  goal_id TEXT NOT NULL,
  session_id TEXT NOT NULL,
  turn_index INTEGER NOT NULL,
  prompt TEXT NOT NULL,
  closeout_status TEXT,
  verification_status TEXT,
  changed_files INTEGER NOT NULL DEFAULT 0,
  validation_items INTEGER NOT NULL DEFAULT 0,
  decision TEXT NOT NULL,
  summary TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  FOREIGN KEY (goal_id) REFERENCES goal_runs(id),
  FOREIGN KEY (session_id) REFERENCES sessions(id)
);
```

The first implementation can keep goal tables independent from `session_parts`.
Later, goal events can be projected into session parts for export and desktop
reload parity.

### Runner Flow

```text
User enters /goal <objective>
  -> validate objective length and feature flag
  -> persist GoalRun(status=Active)
  -> submit first full-agent turn through RuntimeController
  -> collect TurnEvent stream and mirrored session events
  -> derive closeout/verification/changed-files evidence
  -> record GoalStep
  -> decide:
       Complete  if stop rules are satisfied
       Continue  if useful progress remains and budget allows
       NeedsUser if permission/ambiguity/high-risk gate blocks
       Blocked   if the same blocker repeats beyond limit
       Failed    if validation/proof is terminal and repair budget is exhausted
       Pause     if user paused or connectivity/session state requires stop
  -> if Continue, enqueue a bounded continuation prompt
```

Continuation prompts should be compact and evidence-backed:

```text
Goal: <objective>
Stop criteria: <rules>
Previous step:
- closeout=<status>
- verification=<status>
- changed_files=<n>
- blocker=<blocker or none>

Continue the goal by taking the smallest useful next step.
Do not repeat completed work. Stop with a clear blocker if the next step
requires user input, approval, credentials, network access, or a risk boundary.
```

Do not add a large always-on prompt rule. The goal runner should pass only the
current objective, stop rules, and last-step evidence.

## Development Plan

Execution rule: the MVP is now implemented. Further work should be hardening
and dogfood-driven, not a second design pass.

### Phase 0 — Feature Flag And Compatibility Contract

Status: Implemented

Tasks:

- Add `features.goals` to the existing `FeatureFlags` config block, matching
  Codex naming.
- Keep the feature off by default until durable persistence and pause/resume are
  implemented.
- Update `/goal` parsing:
  - `/goal` shows status.
  - `/goal <objective>` becomes the preferred start command.
  - `/goal set <objective>` remains a compatibility alias.
  - `/goal pause`, `/goal resume`, `/goal clear`, `/goal log` parse but can
    initially return clear "not implemented yet" text behind the feature flag.
- Add tests for empty objective, 4,000 character cap, slash-subcommand parsing,
  and alias behavior.

Likely files:

- `src/services/config.rs`
- `src/engine/session_goal.rs`
- `src/tui/slash_handler/learning.rs`
- `src/tui/commands/catalog.rs`
- `src/tui/app/slash_commands.rs`

Validation:

```bash
cargo test -q session_goal --lib
cargo test -q learning --lib
cargo fmt --check
```

### Phase 1 — Durable Goal Store

Status: Implemented

Tasks:

- Add `GoalRun`, `GoalStep`, `GoalBudget`, `GoalStopRules`, and
  `GoalRunStatus`.
- Add a session-store migration for `goal_runs` and `goal_steps`.
- Add CRUD methods to `SessionStore`:
  - `create_goal_run`
  - `get_active_goal_run`
  - `update_goal_run_status`
  - `record_goal_step`
  - `list_goal_steps`
- Keep `SessionGoalManager` as the in-memory current-goal projection, but hydrate
  it from the active durable `GoalRun` when a session starts.
- Add export fields later; do not expand export in this phase.

Likely files:

- `src/engine/session_goal.rs` for the first small extension, or
  `src/engine/goal/model.rs` and `src/engine/goal/store.rs` if the durable
  model/store would make the flat file too broad.
- `src/session_store/mod.rs`
- `src/session_store/records.rs`
- `src/migrations/mod.rs`
- `src/migrations/v17_add_goal_runs.rs`

Validation:

```bash
cargo test -q goal --lib
cargo test -q session_store --lib
cargo check -q
```

### Phase 2 — Goal Decision Engine

Status: Implemented

Tasks:

- Implement a deterministic `GoalDecisionEngine`.
- Inputs:
  - latest `TurnEvent` or `TurnTrace`;
  - final closeout status;
  - verification proof status;
  - changed files count;
  - validation item count;
  - permission or approval blockers;
  - repeated blocker history;
  - current budget.
- Decisions:
  - `Complete`: verified closeout or explicit no-diff success satisfying stop
    rules.
  - `Continue`: progress was made, budget remains, no hard blocker.
  - `NeedsUser`: permission, credentials, ambiguous requirement, network, or
    high-risk approval is needed.
  - `Blocked`: same blocker repeats beyond the configured threshold.
  - `Failed`: validation/proof is terminal and repair budget is exhausted.
  - `Pause`: user paused or runtime was cancelled.
- Add tests that prevent false completion when closeout is `partial`,
  `not_verified`, or has unsettled tool gaps.

Likely files:

- `src/engine/session_goal.rs` for a small first pass, or
  `src/engine/goal/decision.rs` if the decision engine is introduced as a
  separate module with its own focused tests.
- `src/engine/trace/event.rs`
- `src/engine/trace/event_summary.rs`
- `src/engine/active_task_plan.rs`

Validation:

```bash
cargo test -q goal_decision --lib
cargo test -q closeout --lib
cargo test -q active_task_plan --lib
```

### Phase 3 — Single-Session GoalRunner

Status: Implemented, hardening in progress

Tasks:

- Add `GoalRunner` as an outer orchestrator around `RuntimeController`.
- Reuse `SessionRunCoordinator` so one session never runs two full-agent turns
  concurrently.
- Implement start, pause, resume, stop, clear, and current status.
- First milestone: one automatic continuation after the initial turn.
- Second milestone: multiple continuations up to `GoalBudget.max_turns`.
- Each continuation must persist a `GoalStep` before starting the next turn.
- If the process restarts, the runner must not auto-resume blindly. It should
  restore the active goal as `Paused` or `NeedsUser` and require explicit
  `/goal resume`.

Likely files:

- `src/engine/goal/runner.rs`
- `src/engine/runtime_controller.rs`
- `src/engine/run_coordinator.rs`
- `src/desktop_runtime/mod.rs`
- `src/tui/app.rs`

Validation:

```bash
cargo test -q goal_runner --lib
cargo test -q run_coordinator --lib
cargo test -q runtime_controller --lib
cargo check -q
```

## Implemented Follow-Up Phases

These items moved from backlog into the implementation. Keep the scope honest:
the product surface exists, but it still needs dogfood and smoke coverage before
being described as mature.

- Phase 4 TUI product surface: `/quick`, `/active-task`, and `/goal log` expose
  turn count, budget, latest decision, blocker, and verification status.
- Phase 5 desktop progress row: compact pause/resume/edit/clear controls are
  mounted above the composer and use the same desktop run event path for
  start/resume continuation prompts.
- Phase 6 scored improvement loop: optional scored-eval stop rules feed the
  deterministic decision engine.
- Phase 7 export/readiness: durable goal state and latest step evidence are
  included in readiness/export surfaces.

Hardening checklist:

- Add real desktop smoke coverage for `/goal <objective>` and Resume.
- Dogfood steering behavior: user follow-up during active goal should take
  priority over automatic continuation.
- Keep score parsing conservative until real runs need parser formats beyond
  the current verification-derived score.
- Keep restart resume explicit.

## Safety And Runtime Boundaries

Goal mode increases autonomy. These boundaries must stay hard:

- No goal continuation can bypass existing permission, checkpoint, destructive
  scope, high-risk, LSP/diagnostic, or validation gates.
- A summary from the model is not proof. Goal completion must be backed by
  closeout, validation proof, or explicit no-diff audit evidence.
- The runner may generate compact continuation prompts, but it must not insert a
  large hidden rule block that fights the normal prompt.
- On restart, do not auto-resume active goals without explicit user action.
- If network, credentials, external paths, or broad filesystem writes are
  required, mark the goal `NeedsUser`.
- If the same blocker repeats beyond budget, mark the goal `Blocked` instead of
  burning turns.
- If a user pauses or sends a side question, do not continue mutating the
  workspace until the goal is resumed.

## MVP Acceptance Criteria

The first slice is good enough for dogfood when:

- `/goal <objective>` starts a durable goal and persists it in `SessionStore`.
- `/goal pause`, `/goal resume`, and `/goal clear` work in TUI.
- Restart restores the goal as paused or needs-user, never as silently running.
- The decision engine refuses to mark `partial`, `not_verified`, or unsettled
  closeout as complete.
- Repeated blockers can be represented as `Blocked` with a concrete reason.
- `/goal status` and `/goal log` expose durable state without needing desktop.
- MVP gates pass:

```bash
cargo fmt --check
cargo check -q
cargo test -q goal --lib
cargo test -q session_store --lib
cargo test -q closeout --lib
cargo test -q active_task_plan --lib
cargo check --features experimental-api-server -q
git diff --check
```

Full product acceptance additionally requires safe automatic continuation,
desktop progress controls, export/readiness coverage, and optional scored-loop
support after dogfood evidence.

## Suggested First Slice

Start small:

1. Add feature flag, parser compatibility, and objective length checks.
2. Add durable `GoalRun` and `GoalStep` store.
3. Add read-only `/goal status` and `/goal log`.
4. Add `GoalDecisionEngine` tests using synthetic closeout traces.
5. Stop and review before automatic continuation.

This keeps the riskiest part, autonomous cross-turn scheduling, behind a durable
state model and deterministic stop decisions.
