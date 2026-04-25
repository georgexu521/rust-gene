# Agentic Design Patterns Review

Source: `/Users/georgexu/Downloads/agentic-design-patterns-main`

Purpose: use the Agentic Design Patterns handbook as a structured audit checklist for Priority Agent. This is a living review log. Each pass reads a bounded set of chapters, maps the patterns to current code, and records concrete gaps.

## Reading Plan

| Pass | Material | Why It Matters | Status |
| --- | --- | --- | --- |
| 0 | README, SUMMARY, chapter index | Establish scope and chapter order | Done |
| 1 | Ch.1 Prompt Chaining, Ch.2 Routing, Ch.3 Parallelization, Ch.4 Reflection, Ch.5 Tool Use, Ch.6 Planning | Core agent execution loop and tool behavior | Done |
| 2 | Ch.7 Multi-Agent Collaboration, Ch.8 Memory, Ch.9 Learning, Ch.10 MCP, Ch.11 Goal Monitoring, Ch.12 Recovery | Long-running agent reliability and adaptation | Done |
| 3 | Ch.13 Human-in-the-Loop, Ch.14 RAG, Ch.15 A2A, Ch.16 Resource-Aware Optimization | Coordination, retrieval, budget control | Done |
| 4 | Ch.17 Reasoning, Ch.18 Guardrails, Ch.19 Evaluation, Ch.20 Prioritization, Ch.21 Exploration | Safety, observability, decision quality | Done |
| 5 | Appendix E CLI Agents, Appendix F Reasoning Engines, Appendix G Coding Agents | Directly relevant CLI/coding-agent maturity | Done |

## Current Project Map

Priority Agent already has a broad implementation surface:

- Execution loop: `src/engine/conversation_loop/`, `src/engine/streaming.rs`
- Planning and Socratic analysis: `src/engine/plan_mode.rs`, `src/engine/socratic.rs`, `src/engine/socratic_executor.rs`
- Tools: `src/tools/`, including bash, file, project, git, web, memory, todo, agent, swarm, MCP, cron
- Memory: `src/memory/manager.rs`, `src/tools/memory_tool/`
- Permissions and guardrails: `src/permissions/`, tool confirmation hooks, CLI approval UI
- CLI experience: `src/tui/`, command palette, picker panels, approval panels, tool output viewer
- Session persistence: `src/session_store/`, `src/tui/session_manager.rs`

The project is not missing “features” in a raw checklist sense. The larger gap is maturity: explicit orchestration contracts, measurable evaluation, tighter feedback loops, and runtime observability.

## 2026-04-25 Coverage Recheck

This recheck compares the full handbook against the current codebase after the recent orchestration, learning, recovery, memory, MCP, and CLI-observability work.

### Direct Answer

The recent changes were comprehensive for the highest-risk gaps from the first review: turn tracing, intent routing, goal monitoring, tool recovery metadata, learning feedback, memory selection, and MCP visibility are now implemented and connected to the conversation loop.

They are not yet comprehensive for the entire handbook. The remaining work is mostly productizing implicit behavior into first-class contracts and user-visible workflows: task context bundles, unified retrieval provenance, unified human review requests, evaluation sets, structured reflection artifacts, A2A-style envelopes, and resource policies.

### Pattern Coverage Matrix

| Handbook Pattern | Current Coverage | Evidence | Remaining Gap |
| --- | --- | --- | --- |
| Prompt chaining | Mostly covered | `ConversationLoop`, `WorkflowEngine`, `PlanMode`, `TurnTrace` | No reusable chain-template library for common coding workflows |
| Routing | Covered | `src/engine/intent_router.rs`, route events in `TurnTrace`, learning-aware routing | Needs more calibration data from real outcomes |
| Parallelization | Mostly covered | read-only parallel tool execution, `swarm`, `agent`, `team` tools | No first-class `ParallelGroup` merge contract or branch UI |
| Reflection | Mostly covered | `ReflectionPass` v1, Socratic analysis, review/debug agent templates, verification trace events | Reflection is not yet mandatory after every risky edit |
| Tool use | Mostly covered | tool registry, permissions, structured `ToolResult`, recovery metadata, tool viewer | Tool reliability score is not yet shown in routing/UI |
| Planning | Mostly covered | `TaskContextBundle` v1, `PlanMode`, workflow gates, approval UI, trace events | Plan dependencies/checkpoints are not fully unified with the task bundle |
| Multi-agent collaboration | Mostly covered | `agent`, `swarm`, `team`, role memory | Missing A2A-compatible task envelope and protocol boundary |
| Memory management | Covered | frozen snapshot, prefetch, topic/project/user/session memory, namespace search, conflict hints | Needs stronger UI for conflicts and memory provenance |
| Learning/adaptation | Mostly covered | `LearningEventRecord`, turn/tool outcome persistence, `route_with_learning` | Priority weights and workflow choice still need broader outcome feedback |
| MCP | Mostly covered | MCP manager, stdio/ws/http, OAuth, resource list/read trace, health diagnostics | Not yet a polished standalone MCP server surface |
| Goal monitoring | Covered | `SessionGoalManager`, `GoalDriftDetector`, `/goal drift`, approval integration | Needs richer goal history and acceptance criteria UI |
| Recovery | Mostly covered | `RecoveryPlan`, `/recover`, API/tool recovery trace, tool metadata | Workflow-level recovery is not uniform across every failure mode |
| Human-in-the-loop | Mostly covered | `HumanReviewRequest` v1, permissions, ask-user tool, plan approval, goal-drift approval | Plan approval and fallback decisions still need full migration into the unified model |
| RAG/retrieval | Mostly covered | `RetrievalContext` v1, project index, memory prefetch, web tools, MCP resource access | Project/web/MCP still need to migrate fully into the unified context object |
| A2A/inter-agent communication | Mostly covered | `AgentTaskEnvelope` v1, team messaging, swarm, child agents | Existing agent/swarm/team paths still need full migration to the envelope |
| Resource-aware optimization | Mostly covered | `ResourcePolicy` v1, cost tracker, model fallback, context compression, tool budgets, `/resource` | Policy is visible but not yet enforced across every executor |
| Reasoning techniques | Mostly covered | Socratic engine, `ReasoningPolicy`, workflow questioning | Missing explicit strategy selection artifact beyond router policy |
| Guardrails/safety | Mostly covered | permissions, bash danger checks, path protections, SSRF guards, approvals | Needs eval coverage for guardrail regressions |
| Evaluation/monitoring | Mostly covered | EvalSet v1, 820+ tests, traces, workflow reports, CLI observability | EvalSet currently covers deterministic routing/trace checks; full tool trajectory replay is still pending |
| Prioritization | Mostly covered | weight engine, priority scheduler, todo priority, learning signals | Learned outcomes do not yet recalibrate weights deeply |
| Exploration/discovery | Mostly covered | project scanner, fuzzy search, web search/fetch, agents | Needs exploration workflow templates and a discovery dashboard |
| CLI/coding-agent appendices | Mostly covered | mature CLI shell, command palette, tool views, approvals, status line | Missing prompt library, configurable statusline, and mature workflow dashboard |

### What The Recent Work Closed

The latest implementation rounds closed the review's original P0/P2/P3 spine:

- `TurnTrace` now records routing, goal updates, memory injection, compaction, API calls, tool calls, approvals, MCP resources, recovery, verification, and final response events.
- `IntentRouter` now chooses intent, workflow, retrieval depth, and reasoning policy, and it consumes persisted learning events.
- `SessionGoalManager` and `GoalDriftDetector` make goal drift visible and approval-aware.
- `RecoveryPlan` is now a structured artifact attached to API and tool failures, with CLI visibility through `/recover`.
- `LearningEventRecord` persists turn and tool outcomes, including recovery metadata, and feeds routing.
- Memory selection now supports project/user/session/topic search and semantic prefetch.
- MCP has health-aware status, resource access tracing, and approval-aware tool execution.

### Still Missing As First-Class Architecture

These are the main gaps to close before claiming the handbook is fully represented:

1. Prompt/workflow template library and configurable statusline for mature CLI ergonomics.
2. EvalSet full replay support for tool trajectories, artifacts, and final answer criteria.
3. Full migration of project/web/MCP/session retrieval into `RetrievalContext`.
4. Full migration of plan approval and fallback decisions into `HumanReviewRequest`.
5. Runtime enforcement of `TaskContextBundle`, `ReflectionPass`, and `ResourcePolicy` for risky coding tasks.
6. Full migration of agent/swarm/team handoffs into `AgentTaskEnvelope`.

### Recommended Next Priority

The next best work is not to add another isolated feature. Build the missing contracts in this order:

1. Expand EvalSet from deterministic routing/trace checks into full tool-trajectory replay.
2. Migrate project/web/MCP/session retrieval into `RetrievalContext`.
3. Expand CLI dashboard and prompt/workflow templates.
4. Complete runtime enforcement of `TaskContextBundle`, `ReflectionPass`, `ResourcePolicy`, and `HumanReviewRequest`.
5. Migrate agent/swarm/team handoffs into `AgentTaskEnvelope`.

### 2026-04-25 Implementation Update

- Added `src/engine/evalset.rs` with deterministic EvalSet loading, routing assertions, trace-event assertions, reports, and unit tests.
- Added `evalsets/smoke.yaml` covering direct answer, debugging, code change, memory, and research routing.
- Added `/eval list` and `/eval run <name|all>` to the CLI.
- Added `src/engine/retrieval_context.rs` with source, score, provenance, trust, freshness, and token-estimate fields.
- Migrated memory prefetch prompt injection to `<retrieval-context>` and added `retrieval.context` trace events.
- Added `src/engine/human_review.rs` with a unified review request contract, and wired tool/goal-drift approvals through it.
- Added `src/engine/task_context.rs` and `src/engine/reflection_pass.rs` as first-class task and self-review artifacts.
- Added `src/engine/resource_policy.rs`, trace-visible `resource.policy` events, `/resource`, and evalset assertions for resource policy selection.
- Added `src/agent/envelope.rs` with an A2A-inspired `AgentTaskEnvelope` for normalized sub-agent handoffs.

## Pass 0: Handbook Scope

The handbook covers 21 core patterns:

1. Prompt chaining
2. Routing
3. Parallelization
4. Reflection
5. Tool use
6. Planning
7. Multi-agent collaboration
8. Memory management
9. Learning and adaptation
10. MCP
11. Goal setting and monitoring
12. Exception handling and recovery
13. Human-in-the-loop
14. RAG
15. A2A
16. Resource-aware optimization
17. Reasoning techniques
18. Guardrails/safety
19. Evaluation and monitoring
20. Prioritization
21. Exploration and discovery

This aligns well with the project direction. Priority Agent already has modules for most topics, but several are partially connected rather than systemically governed.

## Pass 1: Core Execution Patterns

### Chapter 1: Prompt Chaining

Pattern summary:

- Break complex work into ordered LLM/tool steps.
- Each step should have a clear input/output contract.
- Intermediate outputs should be inspectable and reusable.

Current coverage:

- Conversation loop supports multi-turn tool use and streaming.
- Plan mode has structured `Plan` and `PlanStep`.
- Socratic executor can generate per-step analysis and reports.
- Tool outputs are now visible in the CLI and can be expanded.

Gaps:

- Prompt chains are implicit in the conversation loop. There is no first-class “chain graph” or durable step artifact tying prompts, tool calls, outputs, and decisions together.
- Intermediate LLM reasoning artifacts are not consistently typed. Plans, tool calls, memory extraction, and final responses each store different shapes.
- There is no reusable chain template mechanism for common workflows such as “inspect → plan → edit → test → summarize”.

Recommended improvements:

- Add a `WorkflowTrace` or `TurnTrace` model that records step type, input, output, tool calls, approvals, costs, and status.
- Define workflow templates for coding loops: diagnose, implement, verify, review, document.
- Surface workflow steps in CLI as collapsible timeline blocks.

### Chapter 2: Routing

Pattern summary:

- Route tasks to specialized handlers/agents/tools based on intent.
- Routing should be explainable and recoverable when wrong.

Current coverage:

- Command registry routes slash commands.
- Tool registry exposes available tools.
- Provider/model pickers exist.
- Agent tool supports templates such as explore, verify, plan, review, debug.

Gaps:

- LLM/tool routing is mostly model-driven. There is no centralized router that explains why a task should use plan mode, Socratic mode, swarm, project search, or direct answer.
- Current `PriorityScheduler` is task-oriented but not deeply integrated with the conversation loop.
- Contextual CLI recommendations exist, but they are UI-level, not a general routing policy.

Recommended improvements:

- Introduce `IntentRouter` before each user turn. It should output `intent`, `confidence`, `recommended_mode`, `recommended_tools`, and `reason`.
- Use router output to decide when to auto-enable plan mode, memory retrieval depth, project indexing, or sub-agent delegation.
- Log router decisions into the proposed `TurnTrace`.

### Chapter 3: Parallelization

Pattern summary:

- Run independent subtasks concurrently.
- Merge outputs through a synthesis step.
- Parallel branches need explicit ownership and failure handling.

Current coverage:

- `agent_tool` supports subtasks and branch forking.
- `swarm` exists for multiple agents.
- The development process already benefits from parallel shell reads.

Gaps:

- Parallelization is available as a tool, but the main agent does not appear to have a strong planner that automatically identifies independent branches.
- There is limited typed merge behavior: synthesis is mainly natural-language output rather than structured result comparison.
- There is no visible “parallel branch status” UI in the CLI.

Recommended improvements:

- Extend plan mode with `ParallelGroup { branches, merge_strategy }`.
- Add merge strategies: concatenate, vote, compare-diffs, risk-review, choose-best.
- Show parallel branch progress in the tool timeline.

### Chapter 4: Reflection

Pattern summary:

- Add review/critique loops after generation.
- Reflection should catch quality gaps before output reaches the user.

Current coverage:

- Socratic analysis asks deeper questions before execution.
- Agent templates include review/debug roles.
- Tests and tool output viewer provide manual verification support.

Gaps:

- Reflection is not systematically attached to risky actions. For example, after editing code, there is no mandatory self-review checklist before final response.
- There is no structured “critique result” artifact with severity, confidence, and proposed fix.
- Reflection and evaluation are not clearly separated: critique, tests, and user-facing summary currently blend together.

Recommended improvements:

- Add `ReflectionPass` after code edits and before final answers for high-risk or multi-file changes.
- Represent reflection output as findings: `issue`, `evidence`, `severity`, `fix_status`.
- Connect reflection findings to CLI UI and final summaries.

### Chapter 5: Tool Use

Pattern summary:

- Tools should have clear schemas, stable results, errors, logging, and safety controls.
- Agents need tool selection discipline, not just tool availability.

Current coverage:

- `Tool` trait defines parameters, execution, confirmation, and classifier input.
- Tool registry is broad.
- Permission system is rule-based with wildcard rules and approval UI.
- Tool output rendering has summaries, details, and full output viewer.

Gaps:

- Tool result schemas are mostly text-first. Some tools have data, but downstream synthesis cannot depend on a uniform structured result model.
- Error taxonomy exists in `ToolErrorCode`, but many tools still return free-form errors.
- There is no tool quality score or historical reliability tracking used by routing.

Recommended improvements:

- Standardize tool results: `summary`, `structured_data`, `diagnostics`, `artifacts`, `risk`, `suggested_next_action`.
- Add per-tool reliability metrics: success rate, average duration, recent failures.
- Feed tool metrics into the router and `/quick` dashboard.

### Chapter 6: Planning

Pattern summary:

- Planning decomposes a goal into steps.
- Good plans include dependencies, progress, approvals, and intermediate checks.
- Plans should update as execution changes.

Current coverage:

- `Plan`, `PlanStep`, `PlanApprovalChannel`, and `PlanModeManager` exist.
- Socratic executor can analyze steps and recalculate weights.
- CLI supports plan approval.

Gaps:

- Plans are linear. Dependencies, parallel groups, checkpoints, and validation gates are not first-class.
- Plan execution state is not fully unified with tool timeline, session persistence, and final reporting.
- Plan revisions are not explicitly versioned.

Recommended improvements:

- Extend `PlanStep` with `depends_on`, `acceptance_criteria`, `verification`, `risk`, `artifacts`, and `status_reason`.
- Add plan versioning and revision history.
- Show active plan progress in status bar or `/quick`.

## First-Pass Priority Recommendations

1. Add a first-class `TurnTrace` / `WorkflowTrace`.
   - Highest leverage because it supports chaining, routing, reflection, tool observability, evaluation, and future UI timelines.

2. Add an `IntentRouter`.
   - This should be explicit and inspectable, not hidden inside the LLM response.

3. Upgrade plan mode from linear list to execution graph.
   - Dependencies, validation gates, parallel groups, and revisions are the main missing maturity layer.

4. Normalize tool result contracts.
   - Current tool outputs are useful to humans but not structured enough for reliable orchestration.

5. Add reflection artifacts for code changes.
   - This is a direct quality upgrade for coding-agent behavior.

## Open Questions For Later Passes

- How much should be persisted in SQLite vs in-memory only?
- Should traces be user-visible by default or hidden behind a viewer?
- Should routing be deterministic rules first, LLM-classifier second, or hybrid?
- How should Socratic reasoning coexist with plan mode without making every task heavy?
- Which evaluation metrics should gate “production-ready” maturity?

## Pass 2: Reliability, Memory, Collaboration

Material read:

- Chapter 7: Multi-Agent Collaboration
- Chapter 8: Memory Management
- Chapter 9: Learning and Adaptation
- Chapter 10: Model Context Protocol
- Chapter 11: Goal Setting and Monitoring
- Chapter 12: Exception Handling and Recovery

### Chapter 7: Multi-Agent Collaboration

Pattern summary:

- Multi-agent systems need clear roles, parent/child delegation, shared state, and explicit communication structures.
- Collaboration is not merely spawning agents; it requires coordination, state passing, stopping conditions, and result synthesis.

Current coverage:

- `src/agent/manager.rs` has `AgentManager`, `AgentDag`, `ResultFusion`, and auditing.
- `src/tools/agent_tool/` supports templates, subtasks, branch forking, and result waiting.
- `src/engine/swarm.rs` and `swarm` tool provide broader orchestration.

Gaps:

- Role contracts are mostly prompt templates. There is no strongly typed role capability model.
- Result fusion exists, but main workflows do not consistently use structured fusion decisions.
- Parent/child state passing is not yet unified with session store, trace, memory, and CLI observability.
- Stopping conditions for multi-agent loops are primitive compared with the handbook’s emphasis on loop agents and state checks.

Recommended improvements:

- Add `AgentRoleSpec` with capabilities, allowed tools, memory scope, output schema, and verification requirements.
- Make `ResultFusion` produce structured fields: consensus, disagreements, confidence, chosen_result, follow_up_tasks.
- Show multi-agent DAG and statuses in CLI, not just final text.

### Chapter 8: Memory Management

Pattern summary:

- Memory has layers: session history, state scratchpad, and long-term searchable memory.
- Production memory needs namespaces, retrieval, update policy, and persistence.

Current coverage:

- `MemoryManager` has session/project/user tiers, frozen snapshots, prefetch, sync, session extraction, maintenance, topic files, and cache stats.
- `SessionStore` persists conversations with SQLite and FTS search.
- Agent role memory exists in `src/agent/memory.rs`.

Gaps:

- Memory namespaces are not yet a first-class routing concept. Project/user/session memories exist, but retrieval policy is not visibly governed by intent.
- Memory quality is not evaluated. The system can save memories, but it does not score usefulness, staleness, contradiction, or confidence.
- Memory write policy is still partly heuristic and LLM extraction based. There is no user-visible “why this was saved” audit.
- Agent memory and main memory are separate systems without a clear reconciliation protocol.

Recommended improvements:

- Add memory metadata: `source_turn`, `confidence`, `category`, `namespace`, `ttl/staleness`, `last_used`, `supersedes`.
- Add `/memory audit` to show why memories were retrieved or saved.
- Add a memory conflict detector and compaction policy.
- Integrate memory retrieval with the proposed `IntentRouter`.

### Chapter 9: Learning and Adaptation

Pattern summary:

- Agents can improve by analyzing outcomes, feedback, failures, and successful strategies.
- Learning needs controlled update mechanisms rather than unrestricted self-modification.

Current coverage:

- Memory extraction can store learnings.
- Workflow modules include feedback/metrics concepts.
- Verification agent and tests provide outcome signals.

Gaps:

- There is no closed-loop learning cycle from “task outcome → policy update → future behavior change”.
- Feedback is not connected to routing, tool selection, or plan templates.
- No experiment registry exists for comparing strategies.

Recommended improvements:

- Add `LearningEvent`: task type, chosen strategy, tools used, outcome, duration, failures, user feedback.
- Add strategy profiles, e.g. “small code edit”, “large refactor”, “research task”, each with success/failure stats.
- Use learning events to adjust router defaults and suggested workflows.

### Chapter 10: Model Context Protocol

Pattern summary:

- MCP externalizes tools/resources and standardizes discovery/calls.
- Production MCP needs transport handling, security boundaries, approval, health, and fallback.

Current coverage:

- `src/engine/mcp.rs` is stronger than a stub: stdio/websocket/http transports, OAuth token storage, health diagnostics, circuit breaker, approvals, tool/resource discovery, tool adapter.
- TUI has `/mcp` management and approval hooks.

Gaps:

- MCP server capabilities are not exposed as a rich CLI dashboard.
- Server health is available programmatically but not deeply integrated into routing or `/quick`.
- MCP resource reading exists, but resource-context injection policy is not clearly linked to memory/RAG/context budget.
- MCP approvals are server-level plus tool calls; there is room for finer scoped trust policies.

Recommended improvements:

- Add `/mcp status` panel with servers, transport, health, approved state, tool counts, resource counts, circuit status.
- Feed MCP health into tool routing so unhealthy servers are avoided.
- Add MCP resource retrieval traces to `TurnTrace`.

### Chapter 11: Goal Setting and Monitoring

Pattern summary:

- Agents need explicit goals, subgoals, progress monitoring, and completion criteria.
- Monitoring should detect drift and decide when to revise plans.

Current coverage:

- Plan mode has goal/title/steps/progress.
- Turn state tracks iterations, transitions, tool calls, retries, compression.
- Workflow gate and questioning modules exist.

Gaps:

- Goals are not first-class across the entire conversation. They exist in plans, but not as a session-level objective model.
- Progress monitoring is mostly local to plan mode, not reflected in `/quick`, status bar, memory, or final report.
- No drift detector compares current actions against the active goal.

Recommended improvements:

- Add `SessionGoal`: user goal, active subgoals, acceptance criteria, status, risks, last_progress.
- Add goal progress to status bar and `/quick`.
- Add a “goal drift” check before tool execution when the conversation is long or the task is complex.

### Chapter 12: Exception Handling and Recovery

Pattern summary:

- Recovery should classify failures, choose fallback paths, and preserve state.
- Reliable agents separate primary action, fallback handler, and final response rendering.

Current coverage:

- `ErrorClassifier` has categories and recovery actions.
- `TurnState` records retry/compression/fallback transitions.
- MCP has circuit breaker and health diagnostics.
- Tool errors have a code taxonomy.

Gaps:

- Recovery policies are not consistently expressed as executable fallback workflows.
- Tool-level failures do not always produce structured recovery suggestions.
- User-visible recovery is fragmented across error messages, slash handlers, and final summaries.
- There is no central “recovery trace” that shows what failed, what was retried, what fallback was used, and why.

Recommended improvements:

- Add `RecoveryPlan` with primary failure, chosen action, fallback candidates, user-visible note, and final status.
- Standardize tool failure output with `recoverable`, `suggested_command`, and `safe_retry`.
- Surface recovery events in CLI tool timeline.

## Second-Pass Priority Recommendations

1. Add `SessionGoal` and connect it to plan mode, `/quick`, status bar, and final summaries.
2. Add `LearningEvent` persistence and use it to tune routing/tool selection over time.
3. Unify agent memory and main memory through namespaces and conflict handling.
4. Upgrade MCP observability and feed server health into tool routing.
5. Implement `RecoveryPlan` as a structured artifact rather than ad hoc retry logic.

## Updated Assessment After Pass 2

Priority Agent is architecturally broad and already covers many handbook chapters at the module level. The repeated weakness is that many capabilities are available but not governed by a shared orchestration model. The best next architectural move is still the same: introduce shared trace/goal/router artifacts that every subsystem can write to and read from.

## Pass 3: Human Oversight, Retrieval, Safety, Evaluation

Material read:

- Chapter 13: Human-in-the-Loop
- Chapter 14: Knowledge Retrieval (RAG)
- Chapter 15: Inter-Agent Communication (A2A)
- Chapter 16: Resource-Aware Optimization
- Chapter 17: Reasoning Techniques
- Chapter 18: Guardrails/Safety Patterns
- Chapter 19: Evaluation and Monitoring
- Chapter 20: Prioritization
- Chapter 21: Exploration and Discovery

### Chapter 13: Human-in-the-Loop

Pattern summary:

- HITL is best used for high-risk, ambiguous, irreversible, or expertise-heavy decisions.
- The key tradeoff is scalability vs accuracy.

Current coverage:

- Permission approval panel for tool calls.
- Ask-user tool and pending question UI.
- Plan approval channel.
- `/reject`, `/permissions`, contextual command palette actions.

Gaps:

- HITL triggers are scattered: permission, ask-user, plan approval. There is no unified `HumanReviewRequest`.
- The UI does not yet show why the system chose human review instead of automatic execution.
- Human feedback is not consistently converted into reusable policy/learning signals.

Recommended improvements:

- Add `HumanReviewRequest { reason, risk, options, default, persistence_scope, impact }`.
- Use a single review queue for plan approvals, tool approvals, and questions.
- Record human decisions as `LearningEvent` and permission policy evidence.

### Chapter 14: Knowledge Retrieval (RAG)

Pattern summary:

- Agentic RAG is active retrieval: query decomposition, source evaluation, conflict resolution, and iterative refinement.
- Retrieval quality must be managed against latency/cost.

Current coverage:

- Project scanner and fuzzy file search.
- Memory prefetch and session FTS.
- Web fetch/search tools.
- MCP resource read support.

Gaps:

- Retrieval is fragmented across project search, memory search, web search, MCP resources, and session search.
- There is no source ranking object that normalizes file/memory/web/MCP/session sources.
- No explicit conflict-resolution step when retrieved sources disagree.

Recommended improvements:

- Add `RetrievalContext` with sources, score, provenance, freshness, trust level, and token cost.
- Add `RetrievalRouter` to decide whether to search project, memory, session, web, or MCP.
- Add a source conflict detector and cite retrieved context in trace artifacts.

### Chapter 15: Inter-Agent Communication (A2A)

Pattern summary:

- A2A needs standard envelopes: sender, recipient, task, status, artifacts, streaming updates, and errors.
- A2A and MCP differ: MCP is tool/resource access; A2A is agent-to-agent task exchange.

Current coverage:

- `AgentMessage`, `AgentManager::send_message`, and agent statuses.
- Platform adapters and API/WebSocket surfaces.
- Agent DAG and audit records.

Gaps:

- Agent messages are internal and not a protocol boundary.
- No A2A-compatible task envelope or streaming status protocol.
- Artifacts and result schemas are not standardized across agents.

Recommended improvements:

- Add an A2A-inspired `AgentTaskEnvelope`.
- Standardize `AgentArtifact`: text, file patch, diff, test report, trace, decision.
- Expose agent task/status events over API/WebSocket.

### Chapter 16: Resource-Aware Optimization

Pattern summary:

- Agents should choose models, tools, depth, and parallelism based on budget, latency, risk, and desired quality.
- Fallback and graceful degradation are resource-aware patterns.

Current coverage:

- Cost tracker, token summaries, model/provider switching.
- Context compression and token budget management.
- Workflow questioning budget.
- Error fallback actions include fallback model/reduce tokens.

Gaps:

- Model selection is manual or provider-default. The agent does not dynamically choose cheap/fast/deep modes per task.
- Tool and sub-agent parallelism are not budget-aware.
- `/quick` exposes status but not actionable resource budgets.

Recommended improvements:

- Add `ResourcePolicy { latency_target, cost_ceiling, reasoning_depth, parallelism_limit }`.
- Route simple tasks to cheaper/faster model settings and complex tasks to deeper workflows.
- Show cost/risk/depth in active plan and status bar.

### Chapter 17: Reasoning Techniques

Pattern summary:

- Complex tasks benefit from explicit reasoning structures: decomposition, reflection, debate, graph exploration, and iterative verification.
- More reasoning compute should be allocated intentionally.

Current coverage:

- Socratic questioning engine and active questioning workflow.
- Plan mode and workflow engine.
- Verification agent.

Gaps:

- Reasoning strategy selection is not explicit. Socratic, plan, reflection, and swarm are separate capabilities.
- Debate/graph-of-thought style reasoning is not represented.
- There is no “reasoning budget” artifact visible to user.

Recommended improvements:

- Add `ReasoningStrategy`: direct, chain, plan, socratic, reflect, debate, parallel-research.
- Have `IntentRouter` choose strategy based on task complexity/risk.
- Add a visible reasoning/depth indicator in CLI for non-trivial workflows.

### Chapter 18: Guardrails/Safety Patterns

Pattern summary:

- Safety needs input filtering, output validation, least privilege, policy enforcement, and observability.
- Guardrails should be layered rather than a single check.

Current coverage:

- Permission modes, rule sources, wildcard matching, LLM classifier.
- Bash/file safety checks and confirmation prompts.
- MCP server approval and OAuth token handling.
- Approval UI and persistent rules.

Gaps:

- Output validation guardrails are underdeveloped. The system protects tool use better than final answer quality.
- Least-privilege tool profiles are not tied to agent roles.
- No red-team/eval suite specifically targets prompt injection, unsafe tool routing, or memory poisoning at workflow level.

Recommended improvements:

- Add role-based tool allowlists.
- Add final-output validators for code edits, claims, citations, and unsafe recommendations.
- Add security eval sets for tool injection, memory poisoning, MCP compromise, and path attacks.

### Chapter 19: Evaluation and Monitoring

Pattern summary:

- Evaluation should include tool trajectories, expected intermediate states, final answers, quality scoring, and monitoring.

Current coverage:

- Large test suite.
- `CostTracker` stores tool execution stats, quality scores, recent tool events, latency percentiles.
- `VerificationAgent` can run build/test/lint/concurrency/boundary/idempotency probes.
- Workflow metrics and calibration summaries exist.

Gaps:

- No canonical evalset format for agent tasks.
- No automated regression suite for full tool trajectories.
- Monitoring data is not consistently persisted into a unified dashboard.

Recommended improvements:

- Add `evalsets/` with JSON/YAML scenarios: input, expected tools, forbidden tools, expected artifacts, final criteria.
- Add CLI command `/eval run <set>` and `/eval report`.
- Persist trace + metrics per eval run.

### Chapter 20: Prioritization

Pattern summary:

- Agents should prioritize tasks based on urgency, importance, dependencies, risk, and impact.

Current coverage:

- `PriorityScheduler`, `AiWeightAnalyzer`, heuristics, priority module.
- User’s original project concept is explicitly priority-driven.

Gaps:

- Priority scoring is not central in active CLI workflows.
- Plan step weights and task priorities are not consistently visible or used to gate execution order.
- Prioritization lacks feedback calibration from outcomes.

Recommended improvements:

- Make priorities visible in plan UI and `/quick`.
- Add dependency-aware and risk-aware priority scoring.
- Connect `LearningEvent` outcomes to priority weight tuning.

### Chapter 21: Exploration and Discovery

Pattern summary:

- Discovery agents generate hypotheses, explore search spaces, and iteratively refine candidates.
- Useful in research, science, codebase exploration, and unknown-problem diagnosis.

Current coverage:

- Project scanner, search, web tools, Socratic questions, agent exploration template.

Gaps:

- Exploration is not a named workflow with hypotheses, evidence, and candidate ranking.
- Findings from exploration do not become structured artifacts.
- No novelty/coverage metric exists.

Recommended improvements:

- Add `ExplorationWorkflow`: hypotheses, probes, evidence, confidence, next probes.
- Use it for codebase reconnaissance, debugging, and research tasks.
- Store exploration artifacts in session and optionally memory.

## Third-Pass Priority Recommendations

1. Build a unified `ReviewRequest` / HITL queue.
2. Build `RetrievalContext` and `RetrievalRouter`.
3. Add `ReasoningStrategy` and `ResourcePolicy` selection before execution.
4. Add role-based tool profiles and output validators.
5. Add evalset-based agent regression testing.

## Overall Gap Pattern

The handbook repeatedly distinguishes between “having a capability” and “operating it as a pattern”. Priority Agent has many capabilities, but needs shared artifacts and policy layers:

- `TurnTrace` for observability
- `SessionGoal` for monitoring
- `IntentRouter` for mode/tool selection
- `RetrievalContext` for RAG
- `ReasoningStrategy` for deliberate thinking depth
- `ResourcePolicy` for cost/latency/quality tradeoffs
- `HumanReviewRequest` for HITL
- `LearningEvent` for adaptation
- `RecoveryPlan` for failure handling
- `EvalSet` for regression quality

## Pass 4: CLI and Coding-Agent Appendices

Material read:

- Appendix E: AI Agents on the CLI
- Appendix F: Under the Hood: Reasoning Engines
- Appendix G: Coding Agents

### Appendix E: AI Agents on the CLI

Pattern summary:

- Modern CLI agents are collaborative workspaces, not command wrappers.
- Claude Code is positioned around architecture understanding and conversational multi-step coding.
- Gemini CLI emphasizes broad context, multimodal input, sandboxing, memory, and MCP.
- Aider emphasizes direct file edits, tests, Git transparency, and auto-committed increments.
- Copilot CLI emphasizes GitHub-native issue/PR workflows.
- Terminal-Bench frames CLI agent maturity around reproducible task benchmarks.

Current coverage:

- Priority Agent has a rich CLI surface: command palette, status bar, approvals, tool timeline, model/provider pickers, session persistence, `/quick`.
- It has broad tools and MCP support.
- Git, diff, checkpoint, verification, and session tools exist.

Gaps:

- Claude-like architecture understanding needs a stronger codebase model than file index and grep. The system needs symbols, dependencies, recent edits, test map, and ownership signals.
- Aider-like Git transparency is partial. We can inspect diffs, but there is not yet a disciplined edit/test/commit workflow mode.
- Copilot-like issue/PR workflow is basic. GitHub tool exists, but not a first-class “take issue → branch → implement → PR” workflow.
- Terminal-Bench style reproducible CLI benchmark/eval is missing.

Recommended improvements:

- Add “coding workflow modes”: inspect, edit, verify, review, commit/PR.
- Add a project intelligence cache: files, symbols, dependencies, test commands, package scripts, recent changes.
- Add an eval harness for terminal tasks.

### Appendix F: Reasoning Engines

Pattern summary:

- Leading models describe reasoning as prompt decomposition, context activation/retrieval, method selection, option evaluation, response construction, and iterative refinement.
- Reliable agents should make these stages operational, even if internal chain-of-thought remains hidden.

Current coverage:

- Socratic questions, workflow gate, active questioning, plan mode, reflection-like agent templates.
- Context management and compression.

Gaps:

- The stages are not unified in a lifecycle. The current loop can do them, but does not consistently label or persist them.
- “Method selection” is missing as a first-class step.
- Iterative refinement is not consistently triggered by verification failures or reflection findings.

Recommended improvements:

- Define a standard turn lifecycle:
  1. Understand
  2. Retrieve context
  3. Select strategy
  4. Plan
  5. Act
  6. Verify
  7. Reflect
  8. Summarize/update memory
- Each stage should emit a trace event.
- Add strategy-specific budgets and stopping conditions.

### Appendix G: Coding Agents

Pattern summary:

- Coding agents should operate as specialized team members: scaffolders, testers, documenters, optimizers, reviewers, quality supervisors.
- Good coding-agent workflows use task context folders, versioned prompts, explicit test responsibilities, and humans as final quality gate.

Current coverage:

- Agent templates cover explore, verify, plan, review, debug.
- Verification agent supports build/test/lint/concurrency/boundary/idempotency probes.
- Skills exist as file-driven instructions.
- CLI has tool output viewer, diff viewer, permission UI, model/provider config.

Gaps:

- Prompt libraries are not versioned as project assets in a dedicated `/prompts` or `.priority-agent/prompts` system.
- Specialized coding roles are not deeply integrated into workflow execution.
- Test generation/review/documentation are not first-class workflow steps.
- There is no task-context bundle that captures goal, relevant files, docs, diffs, tests, and constraints for sub-agents.

Recommended improvements:

- Add `.priority-agent/prompts/` role prompts and `/prompts` management command.
- Add `TaskContextBundle` for each non-trivial coding task.
- Add workflow templates:
  - `code-change`: inspect → plan → edit → test → review → summarize
  - `bug-fix`: reproduce → isolate → patch → regression test → review
  - `refactor`: map dependencies → plan phases → edit → test matrix → migration notes
  - `docs`: inspect API → generate docs → verify examples

## Final Roadmap From The Handbook

### P0: Orchestration Spine

1. `TurnTrace`
   - Central trace for every user turn.
   - Events: intent, retrieval, plan, tool call, approval, error, recovery, verification, reflection, memory update.
   - Store in SQLite and render in CLI timeline.

2. `IntentRouter`
   - Inputs: user message, workspace state, memory summary, pending prompts, risk, history.
   - Output: intent, strategy, tools, retrieval sources, model/resource policy, explanation.

3. `SessionGoal`
   - Tracks active user objective, subgoals, acceptance criteria, progress, risks, and drift.
   - Visible in `/quick`, status bar, and final reports.

### P1: Coding-Agent Maturity

4. `TaskContextBundle`
   - Captures goal, files, search results, constraints, diffs, test commands, and relevant memory.
   - Passed to sub-agents and persisted with session.

5. Workflow templates
   - Implement standard modes for code-change, bug-fix, refactor, docs, research.
   - Each template defines stages, tools, verification, and reflection criteria.

6. Reflection artifacts
   - Structured review findings after edits.
   - Severity, evidence, fix status, and whether it blocks final response.

### P2: Retrieval, Memory, Learning

7. `RetrievalContext`
   - Normalizes project/session/memory/web/MCP retrieval with provenance, score, trust, freshness, and token cost.

8. Memory audit and conflict handling
   - Explain why memory was retrieved or saved.
   - Detect stale or conflicting memory.

9. `LearningEvent`
   - Records task type, strategy, tools, cost, outcome, tests, failures, and user feedback.
   - Feeds router and priority calibration.

### P3: Safety, Recovery, Evaluation

10. Unified `HumanReviewRequest`
    - One queue/model for permission approval, plan approval, and user questions.

11. `RecoveryPlan`
    - Structured fallback path for API, tool, context, and workflow failures.

12. Evalset framework
    - Terminal/coding-agent scenario tests with expected tool trajectories, forbidden actions, final criteria, and artifacts.

### P4: CLI Experience

13. Trace/timeline viewer
    - A mature CLI should let users inspect what happened and why.

14. Workflow dashboard
    - `/quick` should evolve into current goal, active workflow, pending review, recent failures, next best actions.

15. Project intelligence dashboard
    - Symbols, tests, dependencies, recent changes, detected commands, and risk hotspots.

## Recommended Next Implementation Sequence

The best next step is not another isolated UI tweak. It should be the orchestration spine:

1. Add `TurnTrace` data model and in-memory collector.
2. Emit trace events from conversation loop, tool execution, permissions, context compression, memory prefetch, and errors.
3. Persist trace events to SQLite.
4. Add `/trace` viewer in CLI.
5. Then add `IntentRouter` and record its decisions into the same trace.

This sequence turns existing broad functionality into an observable, debuggable, improvable agent system. It also directly supports later improvements: evalsets, learning events, better routing, human review, and workflow dashboards.
