# Claude Code Gap Scorecard

> Last updated: 2026-05-03
> Current detailed matrix: `docs/CLAUDE_CODE_GAP_MATRIX_2026-05-03.md`

This scorecard tracks maturity against Claude Code. It intentionally does not
use raw tool or slash-command counts as parity. A command entry, tool registry
entry, or skeleton module only counts when it is connected to real runtime
behavior, traceability, validation, tests, and user-facing recovery.

## Summary

`rust-agent` has crossed the line from prototype to working coding-agent
runtime. It can inspect code, edit files, run validations, use memory, emit
traces, route workflows, and recover from some failures.

It is still not a Claude Code replacement. The biggest gaps are depth and
product maturity: per-tool semantics, decisive broad-task implementation,
subagent isolation, typed lifecycle hooks, polished CLI panels, remote/session
control, and broad replay evidence.

| Dimension | Current rating | Trend | Main gap |
|-----------|----------------|-------|----------|
| Controlled coding loop | Partial/strong | Improving | Broad-task first-pass success and stable closeout |
| Core tool surface | Broad/partial depth | Improving | Product-grade semantics for file/bash/search/git |
| Slash commands | Broad/mixed maturity | Needs discipline | Many entries need maturity labels and smoke tests |
| Subagents | Partial | Improving | Fork/resume semantics, per-agent permissions, status UI |
| Hooks | Partial | Stable | One typed lifecycle runtime |
| Memory/context | Strong foundation | Improving | Smooth product behavior and compaction carryover |
| MCP/integrations | Partial | Improving | Auth/repair/prompt UX and server mode |
| CLI/TUI UX | Partial | Stable | Diff/approval/status/history/context polish |
| Remote/product delivery | Partial | Stable | Packaging, desktop/remote handoff, onboarding |
| Evaluation | Partial/strong foundation | Improving | 20-30 task replay matrix and external baselines |

Approximate maturity:

| Measure | Estimate |
|---------|----------|
| Controlled repo coding tasks | 65-75% Claude Code-like |
| Daily dependable replacement | 55-60% |
| Product maturity | 40-50% |

## Current Strengths

- Unified conversation loop with tool execution, memory injection, context
  compression, traces, permissions, recovery plans, and required validation.
- Broad local coding tool surface: file, bash, grep/glob/project search, git,
  LSP, worktree, memory, MCP, web, task, and agent tools.
- Interactive CLI with slash commands and trace-backed runtime panels.
- Live eval history for memory, planning, repair loops, validation, and
  closeout behavior.
- Unique experimental direction around weighted planning, Socratic analysis,
  memory-driven workflow adjustment, and learning events.

## Current Gaps

### P0: Coding Loop Reliability

The project can complete controlled tasks, but broad tasks still need better
first-pass implementation intent, lower repair counts, stronger validation
classification, and more deterministic final evidence.

Watch metrics:

- First-pass edit success.
- Number of repair rounds.
- Required validation command recognition.
- Stale validation evidence rejection.
- Final output/closeout completeness.

### P0: Core Tool Semantics

Tool count is no longer the right measure. The next gap is depth:

- Bash command classification and recovery.
- File edit stale-read protection.
- Search no-result recovery.
- Git workflow summaries.
- Per-tool trace summaries that can feed final answers.

### P1: Subagents

Agent infrastructure exists, but Claude-level behavior needs explicit agent
profiles, forked/inherited context semantics, per-agent permissions, resumable
results, status UI, and lifecycle traces.

### P1: Hooks

Hook behavior should be consolidated into one typed lifecycle runtime covering
prompt submit, pre/post tool, permission requests, validation, subagents, file
changes, compaction, and session close.

### P1: CLI Product UX

The CLI needs fewer implied promises and more polished high-frequency surfaces:
diff review, approval panels, status line, history search, context
visualization, tool expansion, and settings visibility.

### P2: Product Delivery

Remaining ecosystem work includes MCP server mode, plugin trust/lifecycle,
remote bridge hardening, IDE/desktop onboarding, updater/packaging, and optional
web/voice surfaces.

## Update Rules

When this scorecard is updated:

1. Verify claims against the current checkout.
2. Keep feature count separate from maturity.
3. Mark command/tool surfaces as `production`, `usable`, or `placeholder`.
4. Prefer replay/eval evidence over manual inspection.
5. Do not claim Claude Code parity without broad real-task evidence.
