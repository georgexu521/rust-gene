# Phase D: Visible Agent Profiles — Detailed Implementation Plan

Date: 2026-06-10
Status: Active
Parent: `docs/NEXT_PHASE_PRODUCT_ECOSYSTEM_GAP_PLAN_2026-06-09.md`

## Goal

Make Priority Agent's existing agent profile infrastructure visible and usable
as a daily primary-agent workflow, while preserving the separation between:

- primary agents that the user directly switches between for the main session;
- subagents that are launched for bounded parallel work;
- hidden/system agents that may exist later for summarization, title generation,
  compaction, or other internal jobs.

The immediate product target is close to OpenCode's visible Build/Plan workflow:
the user can switch the main session mode quickly, and the model can launch
focused subagents when parallel research, implementation, or verification is
useful. The implementation should not flatten every profile into one ambiguous
list.

## OpenCode Reference Design

Reference checked: 2026-06-10, [OpenCode agents docs](https://opencode.ai/docs/agents/).

OpenCode's current public agent model has two visible classes:

| Agent | Mode | Purpose |
|-------|------|---------|
| `build` | primary | default development agent with broad tool access |
| `plan` | primary | restricted planning/analysis agent; edits and bash are gated |
| `general` | subagent | parallel multi-step work, broad access except todo-style coordination |
| `explore` | subagent | fast read-only local codebase exploration |
| `scout` | subagent | read-only external docs/dependency research |

OpenCode also has hidden primary/system agents such as compaction, title, and
summary. Those are useful design references, but they should not become visible
Priority Agent scope in this phase.

Key design choices to mirror:

- Primary agents are a first-class session state and can be switched by the
  user during a session.
- Subagents are invoked by the primary agent or manually by the user; the task
  surface lists available subagents and their purpose.
- Agent permissions are runtime-enforced. Profile text is not enough.
- Agent configuration can include model, prompt, permissions, mode, and
  description; model-per-agent routing can remain a later phase for us.

## Current Priority Agent State

### Already Implemented

| Feature | Status | Location |
|---------|--------|----------|
| `AgentMode` enum (`Auto`, `Build`, `Plan`, `Explore`, `Review`) | Implemented | `src/engine/agent_mode.rs` |
| Main-session mode routing and tool-surface changes | Implemented | `src/engine/agent_mode.rs` |
| Product profiles (`build`, `plan`, `explore`, `review`, `verify`) | Implemented as data | `src/agent/profiles.rs` |
| Built-in subagent profiles (`default`, `explorer`, `planner`, `verifier`, `implementer`) | Implemented as data | `src/agent/profiles.rs` |
| `/agent list` | Implemented, product-profile only | `src/tui/slash_handler/agents/agent_listing.rs` |
| `/agent switch <mode>` | Implemented for switchable main modes | `src/tui/slash_handler/agents/agent_listing.rs` |
| `/agent run <profile> <prompt>` | Implemented through `AgentTool` | `src/tui/slash_handler/agents/agent_listing.rs` |
| Profile to `AgentMode` mapping | Implemented with hard-coded aliases | `src/tui/slash_handler/agents/agent_listing.rs` |
| Dynamic task-tool description builder | Implemented, but still constructed from static profiles at tool creation | `src/tools/agent_tool/description.rs`, `src/tools/agent_tool/mod.rs` |
| Project/user agent definitions | Implemented for subagent profiles | `src/agent/profiles.rs` |
| Agent task persistence | Implemented | `src/session_store/agent_store.rs` |
| Session shell jobs API | Implemented, but unrelated to agent tasks | `src/api/routes/mod.rs`, `src/api/dto/session_jobs.rs` |
| API prompt payload `agent_mode` | Implemented | `src/api/routes/mod.rs`, `src/api/state.rs` |

### Current Gaps

1. **Profile registry is split by accident, not by product semantics.**
   `builtin_profiles()` and `product_profiles()` currently represent different
   surfaces, but the code does not expose explicit registry filters like
   "primary profiles", "subagent profiles", and "runnable profiles". A flat
   `all_profiles()` would make this worse by mixing user-switchable modes,
   subagent workers, and possible hidden/system profiles.

2. **Primary-mode permissions are not sourced from product profiles.**
   Main-session behavior is currently enforced by `AgentMode::apply_to_route`.
   The `allowed_tools` and `disallowed_tools` fields on product profiles are
   descriptive data unless the call path explicitly consumes them. The next
   phase should either make product profiles the source of the route/tool
   policy or clearly keep `AgentMode` as the source and treat profiles as DTOs.

3. **`verify` is correctly run-only, but the UI should say that clearly.**
   It should not become an `AgentMode` just to make `/agent switch verify`
   succeed. Verification is a scoped task/run profile, not a main conversation
   mode.

4. **Profile prompts are incomplete for primary modes.**
   Product profiles have empty `system_prompt` strings. The runtime has mode
   context in `AgentMode::runtime_context`, but the profile registry should
   still carry concise, user-visible profile intent for listings, desktop UI,
   and future prompt composition.

5. **The subagent surface is missing a Scout-equivalent.**
   We have local exploration (`explorer`/`explore`) and verification, but no
   clear run-only profile for external dependency docs/source research.

6. **Desktop can submit `agent_mode`, but does not expose it as product UI.**
   The backend API already accepts `agent_mode` on prompt submission. The first
   desktop slice should pass this existing field through the runtime payload and
   UI state instead of creating independent global mode endpoints.

7. **Desktop "jobs" naming would collide with shell jobs.**
   `/api/sessions/:id/jobs` already means durable shell process jobs. Agent run
   projection should use `agent-tasks` or `agents/tasks` naming and map from
   `agent_task_states`.

8. **Task-tool description is not fully context-aware.**
   `AgentTool::new()` currently builds its description from `load_profiles(".")`
   at construction time, while execution resolves profiles against
   `context.working_dir`. Project-specific profiles can therefore be runnable
   without appearing in the advertised profile list.

## Target Architecture

### Registry Layer

Keep one registry entry type, but expose explicit views:

```rust
pub enum AgentProfileSurface {
    Primary,
    Subagent,
    Hidden,
}

pub fn primary_profiles(project_root: impl AsRef<Path>) -> Vec<AgentProfile>;
pub fn subagent_profiles(project_root: impl AsRef<Path>) -> Vec<AgentProfile>;
pub fn runnable_profiles(project_root: impl AsRef<Path>) -> Vec<AgentProfile>;
pub fn profile_surface(profile: &AgentProfile) -> AgentProfileSurface;
```

The first implementation can derive the surface conservatively:

- `build`, `plan`, `explore`, and `review` are primary/switchable;
- `verify`, `default`, `explorer`, `planner`, `verifier`, `implementer`, and
  the new `scout` profile are run-only subagents;
- hidden/system profiles are deferred until the product needs them.

This keeps OpenCode's primary/subagent split without requiring a risky profile
schema migration on day one. A later migration can add an explicit `mode` or
`surface` field to `AgentProfile`.

### Runtime Layer

The runtime must have a single source of truth for primary mode behavior.
Choose one of these approaches during implementation and document the choice in
code:

1. Keep `AgentMode` as the authoritative policy source, and use primary
   profiles as UI/config DTOs that map into `AgentMode`.
2. Move route/tool policy onto primary `AgentProfile` data and make
   `AgentMode::apply_to_route` derive from those profiles.

For this phase, option 1 is lower risk because `AgentMode::apply_to_route`
already guards the main-session tool surface. The work should still add tests
that product profile declarations and `AgentMode` behavior do not drift.

### Product Layer

Expose the same registry views consistently:

- TUI `/agent list`: group "Switchable primary modes" and "Runnable subagents".
- TUI `/agent switch`: accept only primary modes.
- TUI `/agent run`: accept runnable subagents, and optionally primary profiles
  only if they are explicitly marked runnable.
- Desktop composer/header: show current primary mode and let the user select a
  new mode for the next submitted prompt.
- Desktop agent panel: show durable agent task states, not shell jobs.

## Implementation Plan

### Step 1: Add Explicit Profile Registry Views

**Files**: `src/agent/profiles.rs`

Tasks:

- Add profile-surface helpers instead of a flat `all_profiles()`.
- Add `primary_profiles(project_root)`, `subagent_profiles(project_root)`, and
  `runnable_profiles(project_root)`.
- Preserve project/user profile overrides from `.priority-agent/agents/*.toml`
  and `~/.priority-agent/agents/*.toml`.
- Keep `verify` run-only. Do not add `AgentMode::Verify` unless we later define
  what a full main-session verification mode means.
- Add a `scout` run-only profile for external docs/dependency research. Its
  first version can be read-only and limited to search/fetch/read tools that
  already exist in this project.
- Add tests proving the surfaces stay separate.

Verification:

```bash
cargo test -q profiles -- --test-threads=1
cargo test -q agent_profiles -- --test-threads=1
```

If one of those filters runs zero tests, replace it with the exact test module
or test name shown by `cargo test -- --list`.

### Step 2: Make TUI Agent Commands Use Registry Views

**Files**: `src/tui/slash_handler/agents/agent_listing.rs`

Tasks:

- Change `/agent list` to show two groups:
  - switchable primary modes: `auto`, `build`, `plan`, `explore`, `review`;
  - runnable subagents: `default`, `explorer`, `planner`, `verifier`,
    `implementer`, `verify`, `scout`, plus project/user profiles.
- Make `/agent switch` consult `primary_profiles()` or an equivalent
  `agent_mode_for_primary_profile()` helper.
- Make `/agent run` consult `runnable_profiles(project_root)` instead of only
  `product_profiles()`.
- Ensure aliases such as `planner` -> `plan` and `explorer` -> `explore` are
  deliberate and tested.
- Update help text so run-only profiles are not presented as switchable modes.

Verification:

```bash
cargo test -q agent_listing -- --test-threads=1
```

### Step 3: Align Primary Profile Metadata With Runtime Policy

**Files**: `src/agent/profiles.rs`, `src/engine/agent_mode.rs`

Tasks:

- Add concise non-empty `system_prompt` or `runtime_context` text to product
  profiles. Keep it short; the prompt should describe intent and boundaries,
  not duplicate long AGENTS guidance.
- Add a test that every primary profile has a matching `AgentMode` mapping.
- Add a test that read-only primary profiles (`plan`, `explore`, `review`) do
  not drift from the read-only route/tool policy enforced by `AgentMode`.
- Document in code comments whether `AgentMode` or product profiles are the
  authoritative source for main-session permissions.

Suggested prompt snippets:

```text
build: You are in BUILD mode. Make focused code changes directly when asked,
then verify the changed behavior before finishing.

plan: You are in PLAN mode. Inspect and reason about the project, but do not
modify files unless the user explicitly asks to implement.

explore: You are in EXPLORE mode. Search, read, and map the codebase with
evidence. Avoid mutations unless the user changes the task.

review: You are in REVIEW mode. Lead with concrete findings grounded in diffs,
files, and command output. Avoid edits unless explicitly requested.
```

### Step 4: Make Agent Tool Description Context-Aware

**Files**: `src/tools/agent_tool/description.rs`, `src/tools/agent_tool/mod.rs`

Tasks:

- Keep the static base description for the tool schema.
- At execution time, resolve runnable profiles from `context.working_dir`.
- Ensure project/user profiles that are runnable are included in the model
  guidance for the current task.
- Filter by subagent/runnable surface rather than "everything except default".
- Keep the `profile` parameter as the selector; `role` should remain an
  advanced override only if still needed.

Verification:

```bash
cargo test -q agent_tool -- --test-threads=1
```

### Step 5: Desktop Primary Mode Selector

**Files**:

- `apps/desktop/src/app/components/Composer.tsx`
- `apps/desktop/src/runtime/desktopApi.ts`
- `apps/desktop/src-tauri/src/lib.rs`
- API/runtime files only if the existing payload is insufficient

Tasks:

- Add a compact primary-mode selector near the composer or status/header area.
- Thread the selected mode through the existing prompt submission payload using
  the already-supported `agent_mode` field.
- Prefer a session-local UI state first. Add separate `GET /api/agent/modes`
  or `POST /api/agent/mode` only if the desktop needs server-owned persistent
  mode state.
- Show the current mode with clear labels: Auto, Build, Plan, Explore, Review.
- Disable or omit `verify` and other run-only profiles from the mode selector.

Verification:

```bash
corepack pnpm --dir apps/desktop exec tsc --noEmit
corepack pnpm --dir apps/desktop build
```

Run the desktop smoke test if it exists and is stable in this checkout:

```bash
corepack pnpm --dir apps/desktop test:ui-smoke
```

### Step 6: Desktop Agent Task Projection

**Files**:

- `src/api/routes/mod.rs`
- `src/api/dto/`
- `src/session_store/agent_store.rs`
- `apps/desktop/src/app/components/WorkbenchPanel.tsx`
- `apps/desktop/src/runtime/desktopApi.ts`

Tasks:

- Add a dedicated agent-task endpoint, for example:
  - `GET /api/sessions/:id/agent-tasks`
  - optional later: `GET /api/sessions/:id/agent-tasks/:task_id`
- Do not reuse `/api/sessions/:id/jobs`; that namespace is already shell jobs.
- Map from durable `agent_task_states` rows into a desktop DTO with:
  - task id / agent id;
  - profile name;
  - status;
  - worktree and branch when present;
  - started/completed timestamps;
  - proof summary or artifact pointer when available.
- Add a collapsible desktop "Agents" panel that shows active and recent
  subagent runs.
- Prefer existing broadcast/progress events for refresh, with polling as a
  fallback if needed.

Verification:

```bash
cargo check --features experimental-api-server -q
corepack pnpm --dir apps/desktop exec tsc --noEmit
corepack pnpm --dir apps/desktop build
```

## Implementation Order

```text
Step 1 (registry views)
  -> Step 2 (TUI commands)
  -> Step 4 (context-aware task description)
  -> Step 3 (metadata/runtime drift tests)
  -> Step 5 (desktop primary mode selector)
  -> Step 6 (desktop agent task projection)
```

Steps 1-4 are Rust-first and should be completed before desktop work. Steps 5-6
are desktop/API slices and should stay small enough to validate independently.

## Acceptance Criteria

- [ ] The code exposes explicit primary/subagent/runnable profile views.
- [ ] A flat `all_profiles()` is not used as the product boundary.
- [ ] `/agent list` separates switchable primary modes from runnable subagents.
- [ ] `/agent switch build|plan|explore|review|auto` works and rejects run-only
      profiles with clear copy.
- [ ] `/agent run` can run project/user subagent profiles from the active
      working directory.
- [ ] `verify` remains run-only unless a real main-session verification mode is
      designed.
- [ ] A `scout` or equivalent external-research subagent exists, or the product
      docs explicitly defer it.
- [ ] Every primary profile has concise non-empty mode context.
- [ ] Tests guard against drift between primary product profiles and
      `AgentMode` route/tool policy.
- [ ] Task-tool profile guidance is generated from the current working
      directory's runnable subagent profiles.
- [ ] Desktop prompt submission can set `agent_mode` through visible UI.
- [ ] Desktop uses a dedicated agent-task projection instead of shell-job APIs.
- [ ] All targeted Rust and desktop validation commands pass.

## Validation

Use the narrowest relevant gates while developing, then run the broader set
before closing the phase:

```bash
cargo fmt --check
cargo check -q
cargo check --features experimental-api-server -q
cargo test -q profiles -- --test-threads=1
cargo test -q agent_mode -- --test-threads=1
cargo test -q agent_listing -- --test-threads=1
cargo test -q agent_tool -- --test-threads=1
cargo test -q session_prompt -- --test-threads=1
corepack pnpm --dir apps/desktop exec tsc --noEmit
corepack pnpm --dir apps/desktop build
bash scripts/doc_health_check.sh
bash scripts/daily-baseline.sh
```

Run `cargo clippy --all-targets --all-features -- -D warnings` before merging
if the implementation changes shared runtime policy, API DTOs, or permission
contracts.

## Non-Goals

- No new DAG/swarm orchestration; existing `AgentTool` remains the worker
  launch surface.
- No model-per-agent routing in this phase.
- No IDE extension or IDE handoff UI in this phase.
- No hidden compaction/title/summary agents unless a separate runtime need is
  designed.
- No broad autonomy expansion; subagents remain scoped workers with explicit
  task prompts and durable evidence.
