# Completed Closure Plan

This plan tracked the final closure work after the trace, intent routing,
session goal, learning event, recovery plan, MCP status, and goal drift
foundations landed.

Overall status: complete as of 2026-04-25. Current project status is tracked in
`docs/PROJECT_STATUS.md`.

## Phase 1: Tool Failure Recovery And Tool Outcome Learning

Status: done in `fd74714`.

Goal: every failed tool call should produce structured recovery metadata and a
durable learning event.

Deliverables:

- Add tool recovery metadata with `recoverable`, `safe_retry`, `suggested_command`,
  and `user_note`.
- Attach metadata to failed `ToolResult.data` without changing each tool
  implementation.
- Persist `tool_outcome` learning events from `ConversationLoop`.
- Surface the metadata through `/learn` and future `/recover` views.

Acceptance:

- Failed tools have structured metadata in their result payload.
- `learning_events.kind = tool_outcome` is written for tool success/failure.
- Tests cover common failure classifications.

## Phase 2: Learning-Driven Tool Selection

Status: done in `44b4250`.

Goal: `IntentRouter` should use historical tool outcomes, not only failed turns,
to tune recommended tools.

Deliverables:

- Aggregate recent `tool_outcome` events by tool.
- Reduce confidence or avoid recommending tools with repeated failures.
- Prefer successful tools for the routed intent.
- Add trace reason text explaining the learning adjustment.

Acceptance:

- Router output changes when recent tool failures are present.
- The change is visible in `IntentRouted.reason`.

## Phase 3: Goal Drift Visibility

Status: done in this branch.

Goal: make medium/high goal drift easier to notice without reading raw trace data.

Deliverables:

- Add `/goal drift` to summarize recent drift events.
- Add a compact drift count to `/quick`.
- Keep medium drift advisory-only; high drift already requires approval.

Acceptance:

- Latest drift events are visible from CLI commands.
- Existing approval flow remains unchanged for high drift.

## Phase 4: Memory Namespace Search And Conflict Handling

Status: done in this branch.

Goal: unify project/user/agent memory at the search and conflict layer.

Deliverables:

- Search across `MEMORY.md`, `USER.md`, topic memory, and `memory/agents/*.json`.
- Show source namespace for each match.
- Add simple conflict detection for duplicate keys or contradictory entries.

Acceptance:

- `/memory` or memory tooling can show agent-memory matches with namespace labels.
- No migration is required and legacy paths remain readable.

## Phase 5: MCP Health-Aware Routing And Resource Traces

Status: done in this branch.

Goal: MCP health should affect routing/tool visibility and resource reads should
be traceable.

Deliverables:

- Feed `McpManager::health_diagnostics()` into routing or tool availability hints.
- Record MCP resource retrieval events in `TurnTrace`.
- Avoid recommending unhealthy or unapproved MCP servers.

Acceptance:

- `/mcp status` and routing agree on available servers.
- MCP resource reads are visible in `/trace last`.

## Execution Order

1. Phase 1
2. Phase 2
3. Phase 3
4. Phase 4
5. Phase 5

All phases above have been implemented and committed.
