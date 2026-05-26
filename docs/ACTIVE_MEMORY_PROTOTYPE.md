# Active Memory Prototype

Last updated: 2026-05-26

The active-memory prototype is an opt-in, gated, read-only memory worker. It is
intended for long interactive persistent sessions where extra local recall may
help, without making memory another hidden planner.

## Enablement

Active memory is off by default.

```bash
PRIORITY_AGENT_ACTIVE_MEMORY=1
```

Optional controls:

```bash
PRIORITY_AGENT_ACTIVE_MEMORY_TIMEOUT_MS=250
PRIORITY_AGENT_ACTIVE_MEMORY_MAX_RESULTS=4
PRIORITY_AGENT_ACTIVE_MEMORY_MAX_CHARS=1800
```

## Eligibility Gates

The worker only runs when all gates pass:

- active memory is enabled;
- memory is available;
- the route allows memory retrieval;
- the turn is user-facing;
- the session has a persistent session id;
- a timeout budget is available;
- the process is not an eval, headless run, automation, or internal agent.

The runtime also checks process markers such as `--eval-run`,
`PRIORITY_AGENT_EVAL`, `PRIORITY_AGENT_EVAL_EVENTS`,
`PRIORITY_AGENT_HEADLESS`, `PRIORITY_AGENT_AUTOMATION`,
`CODEX_AUTOMATION`, and `PRIORITY_AGENT_INTERNAL_AGENT`.

## Behavior

When eligible, the worker performs a bounded local SQLite FTS memory search and
returns at most one retrieval-context item containing compact hits. It does not
call an LLM, write memory, invoke tools, or make independent decisions.

The item is fenced as `<active-memory-context>` and marked as untrusted
background evidence. It enters the same `<relevant_material>` zone as other
retrieval context, below the stable prompt cache boundary.

Failures, empty results, and timeouts are isolated. They produce a trace event
and do not block the main turn.

Trace event:

- `memory.active`

Statuses:

- `skipped`
- `empty`
- `returned`
- `timed_out`
- `failed`

## Boundary

The active-memory worker retrieves and summarizes context only. The main LLM
still owns semantic judgment, repair decisions, and final user-facing behavior.
