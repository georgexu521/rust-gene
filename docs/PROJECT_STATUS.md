# Project Status

Last updated: 2026-05-03

## Summary

Priority Agent is now an interactive coding CLI with a stateful runtime spine:
intent routing, turn traces, session goals, memory, permissions, recovery plans,
MCP health, CLI observability panels, and required validation closeout.

The recent closure plan is complete:

| Area | Status | Commit |
|------|--------|--------|
| Tool failure recovery and tool outcome learning | Complete | `fd74714` |
| Learning-driven tool selection | Complete | `44b4250` |
| Goal drift visibility | Complete | `bd12f64` |
| Memory namespace search and conflict hints | Complete | `934f7fe` |
| MCP health-aware visibility and resource traces | Complete | `f0f4a95` |

Latest verified baseline observed after the 2026-05-03 Claude-gap implementation
batch:

```text
1035 passed; 0 failed
```

Verified with:

```bash
cargo check --quiet
cargo test --quiet -- --test-threads=1
```

## Completed Runtime Spine

- `TurnTrace` records prompt, routing, memory, context, tool, permission,
  recovery, goal drift, assistant, and MCP resource events.
- `IntentRouter` chooses workflow/retrieval/reasoning policy and now consumes
  learning events from tool outcomes.
- `SessionGoal` tracks the active goal; high drift requires approval and
  medium drift is visible in `/goal drift` and `/quick`.
- Tool failures attach recovery metadata and persist `tool_outcome` learning
  events.
- Code-change turns record implementation intent before edits, and env-prefixed
  validation commands are classified as validation evidence.
- Core coding tools now attach structured execution summaries; file edits refuse
  stale-read writes by default; bash command classification covers shell/env
  wrappers and common validation families.
- Memory search spans project, user, topic, and agent namespaces with simple
  conflict detection.
- MCP status and tool/resource visibility are health-aware and approval-aware.
- Hook execution uses typed lifecycle events, records structured run results,
  and is visible through TurnTrace and `/hooks`.
- Subagents have explicit profile contracts, independent tool scopes, lifecycle
  trace events, durable result artifacts, and manager tests for timeout, failure,
  cancellation, and resumable results.
- Slash commands are labeled as `production`, `usable`, or `placeholder` in help
  and command-palette surfaces, with rendered command-palette smoke tests for
  placeholder, usable, and contextual permission actions.
- Evalsets include a 20-scenario deterministic coding replay matrix, JSON
  report output, and `/eval record <name|all>` persisted report files for
  pass/fail trend collection.
- CLI panels are increasingly backed by actual runtime state, not decoration.
- `karpathy-guidelines` is bundled as a coding behavior skill and exposed
  through `/skills`, `/karpathy <task>`, and code-change reflection checks.
- Repeated successful workflows can now become reviewed skill candidates through
  `/skill-proposals`; accepted candidates are untrusted until explicitly
  applied into the user skill path.
- Learning and high-confidence retrieved memory now feed back into workflow
  planning weights with traceable before/after audit records.

## Product Surface

Primary interface:

- `priority-agent`
- `priority-agent --cli`

Compatibility:

- `priority-agent --tui` starts the compatibility full-screen terminal interface.

Secondary interfaces:

- HTTP API with REST/WebSocket/SSE behind `experimental-api-server`.
- Platform adapter framework with Telegram implemented.
- MCP client over stdio, WebSocket, and HTTP.

## Documentation Status

Canonical current docs:

- `README.md`
- `docs/PROJECT_STATUS.md`
- `docs/CLAUDE_CODE_GAP_MATRIX_2026-05-03.md`
- `docs/CLAUDE_CODE_ALIGNMENT_PLAN.md`
- `docs/REMAINING_CLOSURE_PLAN.md`
- `AGENTS.md`

Historical docs kept for reference:

- `PLAN.md`
- `CAPABILITY_MATRIX.md`
- `docs/CLAUDE_GAP_SCORECARD.md`
- `docs/workflow/*`

Removed as obsolete:

- `FEATURE_COMPARISON_CLAUDE_CODE.md`
- `FEATURE_COMPLETENESS_REPORT.md`

Both removed reports described an early state with very few tools, no memory,
and MCP as a stub. That no longer matches the codebase and was more misleading
than useful.

## Remaining Work

The latest 5-item closure plan is complete, and the first Claude-gap P0/P1
implementation batch is now landed. The remaining work is now product maturity,
not missing foundations:

1. Continue measuring broad code-change first-pass success and repair count
   against the replay matrix and live eval tasks.
2. Deepen remaining high-use tool semantics for git, validation closeout, and
   long-running command progress.
3. Expand rendered command-level smoke tests from command-palette coverage into
   approval-panel behavior.
4. Compare persisted eval trend history across baseline runs.
5. Continue CLI polish based on trace-backed state: command palette, statusline,
   approval panels, tool expansion, and settings visibility.
6. Harden ecosystem integrations: MCP server mode, plugins, remote workflows,
   Discord/Slack adapters if they become product priorities.
7. Keep docs synchronized with tests and current behavior.
