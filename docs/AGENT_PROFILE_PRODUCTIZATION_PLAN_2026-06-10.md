# Phase D: Visible Agent Profiles — Detailed Implementation Plan

Date: 2026-06-10
Status: Active
Parent: `docs/NEXT_PHASE_PRODUCT_ECOSYSTEM_GAP_PLAN_2026-06-09.md`

## Goal

Make Priority Agent's existing agent profile infrastructure visible and usable
as a daily primary-agent workflow, matching opencode's model where users switch
between Build and Plan modes with Tab, and subagents are launched for parallel
work.

## opencode Reference Design

opencode's agent surface (`packages/opencode/src/agent/agent.ts`):

| Agent | Mode | Permissions |
|-------|------|-------------|
| `build` | primary | full access (default) |
| `plan` | primary | read-only, denies edit tools |
| `general` | subagent | parallel research, no todowrite |
| `explore` | subagent | read-only exploration |

Key design choices from opencode:
- Primary agents (build/plan) are switchable by the user with Tab; each has
  distinct permission rulesets applied by the tool registry at call time
- Subagents (general/explore) are invoked by the LLM via the `task` tool;
  the tool description dynamically lists available subagents and their purposes
- The permission system merges: default rules → agent-specific rules → user
  config rules, with deny taking priority
- Each agent has its own model config (modelID/providerID), enabling routing
  different agents to different models

## Current Priority Agent State

### Already Implemented ✅

| Feature | Status | Location |
|---------|--------|----------|
| `AgentMode` enum (Auto/Build/Plan/Explore/Review) | ✅ | `src/engine/agent_mode.rs` |
| Product profiles (build, plan, explore, review, verify) | ✅ | `src/agent/profiles.rs:561` |
| Built-in subagent profiles (default, explorer, planner, verifier, implementer) | ✅ | `src/agent/profiles.rs:425` |
| `/agent list` — shows product profiles with permissions | ✅ | `src/tui/slash_handler/agents/agent_listing.rs:225` |
| `/agent switch <mode>` — sets primary session mode | ✅ | `src/tui/slash_handler/agents/agent_listing.rs:267` |
| `/agent run <profile> <prompt>` — delegates to subagent tool | ✅ | `src/tui/slash_handler/agents/agent_listing.rs:338` |
| Profile → AgentMode mapping (auto/plan/build/explore/review) | ✅ | `src/tui/slash_handler/agents/agent_listing.rs:327` |
| Dynamic agent description injection in task tool prompt | ✅ | `src/tools/agent_tool/description.rs` |
| Agent definitions from `.priority-agent/agents/*.toml` | ✅ | `src/agent/profiles.rs:342` |
| Tool permission enforcement per profile | ✅ | `src/agent/profiles.rs` |
| Worktree isolation for mutating profiles | ✅ | `src/engine/worktree.rs` |
| Durable agent results in SQLite | ✅ | `src/session_store/agent_store.rs` |

### Gaps ❌

1. **No desktop agent picker** — Desktop app has no mode selector like
   opencode's Tab-switchable Build/Plan toggle. The backend supports it, but
   the desktop UI doesn't expose it.

2. **No desktop job projection** — When subagents run (agent tool), desktop
   has no visible job panel showing agent id, worktree/branch, status,
   and proof. All information is recorded in session store but not rendered.

3. **`verify` profile is run-only** — `/agent switch verify` doesn't work
   because `verify` maps to `AgentMode::???` (doesn't exist). It's only
   usable via `/agent run verify <prompt>`.

4. **Profiles lack descriptive context** — Product profiles have minimal
   `system_prompt` (empty string). They don't tell the model what mode it's
   in or how its permissions differ.

5. **Built-in subagent profiles not in product_profiles()** — The built-in
   `explorer`/`verifier` profiles are separate from the product-facing
   `explore`/`verify` profiles. Unclear which one the agent tool should use.

## Implementation Plan

### Step 1: Unify Profile Definitions

Clean up the profile registry so there's one canonical list of available
profiles, matching opencode's structure.

**Files**: `src/agent/profiles.rs`

Tasks:
- Add `verify` to `AgentMode` enum (or map it to Review)
- Merge `builtin_profiles()` and `product_profiles()` into a single
  `all_profiles()` function that returns a unified list
- Give each profile a meaningful `system_prompt` snippet that tells the
  model what capabilities it has (e.g., "You are in BUILD mode. You have
  full access to read, edit, write, and run shell commands.")
- Ensure subagent-only profiles (verify, implementer) are not switchable
  as primary modes

**Verification**:
```bash
cargo test -q agent_profiles
cargo test -q agent_mode
```

### Step 2: Make /agent switch Cover All Switchable Profiles

Ensure `/agent switch` accepts all profiles designed as primary modes.

**Files**: `src/tui/slash_handler/agents/agent_listing.rs`

Tasks:
- Ensure `agent_mode_for_profile()` covers: auto, build, plan, explore,
  review
- Keep verify/implementer as run-only (not switchable)
- Update help text to show switchable vs run-only profiles clearly

**Verification**:
```bash
cargo test -q agent_listing
```

### Step 3: Add System Prompts to Product Profiles

Give each profile a default system prompt so the model understands its
current mode and permission boundaries.

**Files**: `src/agent/profiles.rs`

Template (following opencode's explore.txt pattern):

```
build:   "You are in BUILD mode. You have full access to read, write,
         edit, and execute shell commands. Make changes directly."

plan:    "You are in PLAN mode. You can read, search, and explore code,
         but cannot make any changes. Ask the user before shell commands.
         Output structured plans for the user to review."

explore: "You are in EXPLORE mode. You specialize in navigating
         codebases quickly. Use glob, grep, and file_read. Do not create
         or modify files. Return clear file paths and findings."

review:  "You are in REVIEW mode. Analyze code changes, diffs, and
         patterns. Provide findings and recommendations. Do not edit
         files."

verify:  "You are in VERIFY mode. Run validation commands, check
         correctness, and summarize proof status. Report pass/fail
         clearly."
```

### Step 4: Desktop Agent Mode Selector

Add a mode selector to the desktop app's header/status bar, mirroring
opencode's Tab-switchable Build/Plan.

**Files**: `apps/desktop/src/app/components/Composer.tsx`,
`apps/desktop/src/runtime/desktopApi.ts`,
`apps/desktop/src-tauri/src/lib.rs`

Tasks:
- Add `GET /api/agent/modes` endpoint returning list of switchable profiles
  with name, description, and current status
- Add `POST /api/agent/mode` endpoint to switch active agent mode
- Add a mode selector component (dropdown or pill buttons) in the desktop
  header/composer area
- Show current mode label and color indicator

**Verification**:
```bash
corepack pnpm --dir apps/desktop test:ui-smoke
```

### Step 5: Desktop Subagent Job Panel

Show running/completed subagent runs in a visible panel so users can see
parallel agent work.

**Files**: `apps/desktop/src/app/components/WorkbenchPanel.tsx`,
`apps/desktop/src/runtime/desktopApi.ts`,
`apps/desktop/src-tauri/src/lib.rs`

Tasks:
- Add `GET /api/agent/jobs` endpoint returning recent agent runs with
  status, worktree/branch, duration, and proof summary
- Add a collapsible "Agents" panel in the desktop workbench showing
  active/completed subagent jobs
- Each entry shows: agent name, status (running/done/failed), duration,
  output summary
- Panel auto-refreshes when agent status changes (use existing
  AgentProgressEvent broadcast)

**Verification**:
```bash
corepack pnpm --dir apps/desktop build
corepack pnpm --dir apps/desktop test:ui-smoke
```

### Step 6: Clean Up Agent Description Builder

Ensure the dynamic agent description injected into the task tool prompt
includes the correct, unified list of subagent profiles.

**Files**: `src/tools/agent_tool/description.rs`,
`src/tools/agent_tool/mod.rs`

Tasks:
- Update `build_tool_description()` to use `all_profiles()` instead of
  `load_profiles(".")`
- Filter to only include profiles suitable as subagents (not just
  filtering out "default")
- Use profile descriptions consistently (product profiles already have
  good descriptions)

**Verification**:
```bash
cargo test -q agent_tool
```

## Implementation Order

```
Step 1 (unify profiles) → Step 2 (switch command) → Step 6 (description builder)
     ↓
Step 3 (system prompts) — independent, can parallel with 1/2/6
     ↓
Step 4 (desktop mode selector) → Step 5 (desktop job panel)
```

Steps 1-3 and 6 are pure Rust, can be done in one session.
Steps 4-5 are desktop (TypeScript/React/Tauri), separate session.

## Acceptance Criteria

- [ ] `all_profiles()` returns a unified list of all available agent profiles
- [ ] `/agent list` shows both switchable primary modes and run-only subagent profiles
- [ ] `/agent switch build|plan|explore|review` works and changes session behavior
- [ ] Each profile has a non-empty `system_prompt` describing its mode and permissions
- [ ] Desktop app shows a visible agent mode selector in the header
- [ ] Desktop app shows a "Jobs" or "Agents" panel with subagent run status
- [ ] Dynamic agent description includes correct subagent list
- [ ] All agent tool tests pass
- [ ] Profile definitions align with opencode's Build/Plan/Explore/Review model

## Validation

```bash
cargo fmt --check
cargo check -q
cargo clippy --all-targets --all-features -- -D warnings
cargo test -q agent_tool -- --test-threads=1
cargo test -q agent_profiles
cargo test -q session_store
cargo test -q permissions
corepack pnpm --dir apps/desktop exec tsc --noEmit
corepack pnpm --dir apps/desktop build
bash scripts/daily-baseline.sh
```

## Non-Goals

- No new orchestration (DAG, auto-split) — existing AgentTool is sufficient
- No model-per-agent routing — use same model for all agents for now
- No IDE extension — Phase E is separate
- No broader agent autonomy — subagents remain scoped workers
