# Agent Runtime Contract Plan

This plan tracks the remaining work needed to move Priority Agent from broad feature parity toward mature coding-agent behavior.

## Priority Order

1. EvalSet full trajectory replay
   - Goal: evalsets should verify tool trajectories, verification gates, reflection repair loops, and final outcome criteria.
   - Acceptance:
     - Eval scenarios can declare tool calls and expected order.
     - Eval runner can replay deterministic trace events without an LLM.
     - Failed post-edit verification can assert a `ReflectionPass` repair gate.

2. Unified RetrievalContext
   - Goal: memory, MCP, project, web, and session retrieval should share one provenance object.
   - Acceptance:
     - All retrieval sources emit `RetrievalContextBuilt` trace events.
     - Prompt injection uses one `<retrieval-context>` format.
     - CLI can show source, trust, freshness, score, and token estimate.

3. Unified HumanReviewRequest
   - Goal: every approval path should use the same review contract.
   - Acceptance:
     - Tool, plan, goal drift, reflection, fallback, and risky workflow approvals share `HumanReviewRequest`.
     - Approval UI can render risk, reason, scope, and recommended action consistently.
     - Review decisions are traceable and reusable by evalsets.

4. A2A status/artifact/error protocol
   - Goal: `AgentTaskEnvelope` should cover the full agent handoff lifecycle, not just request wrapping.
   - Acceptance:
     - Agent/swarm/team handoffs produce durable status updates.
     - Returned artifacts and errors use a typed schema.
     - CLI can show live child-agent progress and final artifacts.

5. CLI dashboard contract observability
   - Goal: the CLI should expose live task, retrieval, review, reflection, resource, and A2A contract state.
   - Acceptance:
     - `/quick` or dashboard view shows current contract state without reading logs.
     - Reflection findings and repair status are first-class UI rows.
     - Resource budget and retrieval provenance are compact but inspectable.

## Current Sprint

First implementation pass completed:

- EvalSet scenarios can replay deterministic tool trajectories and assert post-edit reflection repair gates.
- Project retrieval now joins memory and MCP in the unified `RetrievalContext` prompt/trace contract.
- Reflection approvals use the unified `HumanReviewRequest` model through a dedicated `ReflectionGate` kind.
- `AgentTaskEnvelope` now includes status updates, produced artifacts, and typed errors.
- `/quick` shows live contract state for retrieval, reflection, verification, and resource policy.

Next pass should deepen these foundations rather than add new parallel abstractions:

Second implementation pass completed:

- Web tool results now emit unified `RetrievalContextBuilt` trace events.
- Session history search can build and inject `RetrievalContext` alongside project retrieval.
- Swarm/team handoffs append durable A2A transcript records to JSONL.
- `/quick` now shows the latest A2A transcript summary in the contract section.

Remaining depth work:

1. Add EvalSet replay for multi-turn tool repair trajectories.
2. Persist full A2A status transitions and returned artifacts, not just handoff summaries.
3. Add a dedicated CLI contract dashboard beyond the compact `/quick` panel.
