# Claude Code Alignment Development Plan

Date: 2026-04-25

Status: substantially implemented through Phase 8 foundations. This document is
kept as the architectural alignment record; current project state is summarized
in `docs/PROJECT_STATUS.md`.

Scope: compare Priority Agent with the local Claude Code source at `/Users/georgexu/Desktop/claude` plus public Claude Code documentation. The goal is not to clone Claude Code, but to borrow mature architecture patterns for observability, routing, context, permissions, memory, and CLI ergonomics.

## Evidence Reviewed

### Local Claude Code Source

- `/Users/georgexu/Desktop/claude/src/QueryEngine.ts`
  - Builds system prompt parts, user context, MCP context, working directories, custom prompt, memory mechanics prompt, and mutable per-turn input context before entering the query loop.
  - Maintains a rich `ProcessUserInputContext`: messages, app state, abort controller, read file state, nested memory triggers, skill triggers, file history, attribution, and in-progress tool IDs.
- `/Users/georgexu/Desktop/claude/src/query.ts`
  - The main loop applies context collapse, autocompaction, token-budget tracking, runtime model selection, streaming tool execution, stop hooks, and error/retry paths.
  - Compaction is treated as a first-class runtime event with usage metrics, token counts, preserved tail, and post-compact message rebuilding.
- `/Users/georgexu/Desktop/claude/src/services/tools/toolOrchestration.ts`
  - Tool calls are partitioned into concurrency-safe batches and serial batches.
  - Read-only safe tools can run concurrently; mutating tools run serially.
  - Context modifiers from tools are applied after execution, which prevents concurrent tool results from corrupting shared state.
- `/Users/georgexu/Desktop/claude/src/state/AppStateStore.ts`
  - App state is a central coordination object for permission mode, tasks, agent registry, MCP clients/tools/resources, plugin state, elicitation queue, notifications, file history, todos, and remote/session bridge state.
- `/Users/georgexu/Desktop/claude/src/services/SessionMemory/sessionMemory.ts`
  - Session memory runs as a background/forked agent, gated by thresholds such as token growth and tool-call count.
  - It writes a session memory file and avoids interrupting the main conversation.
- `/Users/georgexu/Desktop/claude/src/services/extractMemories/extractMemories.ts`
  - Long-term memory extraction is asynchronous, best-effort, throttled, coalesced when already running, and uses a forked agent with a hard turn cap.
  - It records usage, duration, written paths, and skips extraction when the main agent already wrote memory.

### Public Claude Code Documentation

- Slash commands and skills: command/skill metadata includes description, arguments, allowed tools, model, effort, path matching, and dynamic context injection. Invoked skills remain in context and are carried through compaction within a budget.
- Hooks: Claude Code exposes a broad lifecycle: session start/end, prompt submit/expansion, pre/post tool use, permission request, tool batch, subagent start/stop, task created/completed, pre/post compact, config/file/cwd changes, and async hook completion.
- Settings: settings are layered across managed, user, project, and local project scopes. `/status` exposes active settings and origins.
- Permissions: permission rules are fine-grained, can be checked into projects, and follow explicit precedence. Public docs also describe retry after denial.
- Subagents: subagents have explicit descriptions/tool scopes and can be chained by returning results to the main agent.
- Status line: status line is configurable and refreshed from a command when conversation state changes.

Source URLs:

- <https://docs.claude.com/en/docs/claude-code/slash-commands>
- <https://docs.claude.com/en/docs/claude-code/subagents>
- <https://code.claude.com/docs/en/hooks>
- <https://docs.anthropic.com/en/docs/claude-code/settings>
- <https://docs.claude.com/en/docs/agent-sdk/permissions>
- <https://code.claude.com/docs/en/statusline>

## High-Level Diagnosis

Priority Agent already has many ingredients: tools, MCP, memory, permissions, plan mode, Socratic analysis, CLI components, session store, and sub-agent infrastructure. The gap is not raw feature count. The gap is that these capabilities are not yet governed by a single observable runtime model.

Claude Code's more mature shape is:

1. A rich per-turn context object.
2. A central app/session state object.
3. An explicit event lifecycle around prompts, tools, permissions, compaction, tasks, agents, and hooks.
4. Background agents for memory and summaries.
5. Settings/permissions as layered policy, not just local flags.
6. CLI panels that display runtime state, not just static decoration.

For Priority Agent, the next architecture should be:

```text
User input
  -> IntentRouter
  -> TurnTrace starts
  -> RetrievalContext built
  -> SessionGoal updated
  -> Workflow template selected
  -> Tool/agent execution emits trace events
  -> HumanReviewRequest handles approvals/questions
  -> Verification/reflection emits artifacts
  -> LearningEvent updates memory/routing stats
  -> CLI renders trace, status, and next actions
```

## Phase 1: TurnTrace And Runtime Event Spine

Status: complete.

Implemented highlights: `src/engine/trace.rs`, persisted turn traces,
`/trace`, tool/permission/context/memory/recovery/goal/MCP resource events.

### Problem

Current behavior is spread across conversation loop, tool orchestration, permissions, memory, context manager, and CLI state. When a turn goes wrong, the user cannot inspect a single timeline that explains:

- what intent was detected
- what context was retrieved
- why a mode/model/tool was chosen
- what tools ran and whether they were concurrent
- what permission prompts happened
- whether context was compressed
- what memory was used or saved
- what failed and how recovery happened

Claude Code's hook lifecycle and local analytics events show the value of a comprehensive event spine.

### Implementation

Add `src/engine/trace.rs`:

```rust
pub struct TurnTrace {
    pub trace_id: String,
    pub session_id: String,
    pub turn_index: u64,
    pub user_message_preview: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub status: TurnStatus,
    pub events: Vec<TraceEvent>,
}

pub enum TraceEvent {
    UserPromptSubmitted { chars: usize },
    IntentRouted { intent: String, confidence: f32, strategy: String, reason: String },
    RetrievalStarted { sources: Vec<String>, budget_tokens: usize },
    RetrievalCompleted { items: Vec<RetrievalItemSummary>, tokens: usize },
    PlanCreated { steps: usize, risk: String },
    ToolStarted { tool: String, call_id: String, concurrency_group: Option<String> },
    PermissionRequested { tool: String, risk: String, reason: String },
    PermissionResolved { decision: String, scope: String },
    ToolCompleted { tool: String, ok: bool, duration_ms: u64, output_chars: usize },
    ContextCompacted { before_tokens: usize, after_tokens: usize, strategy: String },
    MemoryUpdated { operation: String, paths: Vec<String> },
    RecoveryApplied { error_kind: String, action: String, success: bool },
    AssistantResponded { chars: usize, cost: Option<f64> },
}
```

Add `TraceCollector`:

- In-memory collector for the current turn.
- Cheap no-op implementation for tests and non-interactive paths.
- Emits JSONL-compatible records.
- Does not store full sensitive tool output by default; stores previews, hashes, paths, and structured metadata.

### Integration Points

- `src/engine/conversation_loop/mod.rs`
  - Start trace per user turn.
  - Record model/provider/mode, tool calls, permission events, final status.
- `src/engine/tool_orchestration.rs`
  - Record batch type: concurrent read-only vs serial mutating.
- `src/permissions/mod.rs`
  - Record permission classifier decision and matched rule source.
- `src/engine/context_manager.rs`
  - Record budget/snip/compress events.
- `src/memory/manager.rs`
  - Record memory prefetch/save/extract decisions.
- `src/session_store/mod.rs`
  - Add `turn_traces` and `trace_events` tables.
- `src/tui/commands.rs`
  - Add `/trace`, `/trace last`, `/trace errors`.
- `src/tui/screens/`
  - Render a compact timeline with expandable event details.

### Acceptance Criteria

- Every user turn has a trace ID.
- `/trace` shows the latest turn timeline.
- Tool calls, permission decisions, context compression, memory use, and errors are visible.
- Tests cover trace creation, event append, redaction, persistence, and CLI formatting.

## Phase 2: IntentRouter And Workflow Selection

Status: complete for rule-based routing and learning feedback.

Implemented highlights: `src/engine/intent_router.rs`, trace-visible routing,
workflow/retrieval/reasoning/risk policies, and recent `tool_outcome` feedback.

### Problem

Routing is currently implicit: the model decides tools; slash commands route commands; plan mode is separate; memory retrieval is broad; UI suggestions are local. Claude Code's architecture has multiple routing inputs: mode, settings, skills, hooks, permissions, MCP resources, model capability, and query source.

Priority Agent needs a central pre-turn router that makes decisions explicit and records them.

### Implementation

Add `src/engine/intent_router.rs`:

```rust
pub struct IntentRoute {
    pub intent: IntentKind,
    pub confidence: f32,
    pub workflow: WorkflowKind,
    pub retrieval_policy: RetrievalPolicy,
    pub reasoning_policy: ReasoningPolicy,
    pub tool_policy: ToolPolicy,
    pub model_policy: ModelPolicy,
    pub risk: RiskLevel,
    pub reason: String,
}
```

Start rule-based, then optionally add LLM-assisted routing:

- `question_answering`: direct answer, low retrieval.
- `code_change`: project retrieval, plan, edit/test workflow.
- `debugging`: inspect logs/files, reproduce, patch, verify.
- `research`: web/project retrieval, citation-aware final response.
- `memory`: memory commands or preference extraction.
- `configuration`: settings/model/permission/MCP commands.
- `delegation`: sub-agent/swarm workflows.

### Claude-Inspired Inputs

- Current permission mode.
- Active settings sources.
- Available tools/MCP servers.
- Workspace trust and working directories.
- Skill/command path match.
- Conversation depth and context pressure.
- User prompt shape.
- Recent failures from `TurnTrace`.

### Integration Points

- `ConversationLoop` calls router before provider request.
- Router output is appended to `TurnTrace`.
- CLI `/quick` and status bar show active intent/workflow.
- Slash commands can override router decisions.
- Router can activate a workflow template, not merely set a flag.

### Acceptance Criteria

- Router decision is visible in `/trace`.
- Direct answers avoid unnecessary project scans.
- Code-change tasks select code workflow and verification.
- High-risk tasks trigger stricter permission and reflection policy.
- Tests cover routing examples and override precedence.

## Phase 3: SessionGoal And Goal Monitoring

Status: complete for active goal tracking and drift visibility.

Implemented highlights: `src/engine/session_goal.rs`, goal drift detector,
high-drift approval, `/goal`, `/goal drift`, and `/quick` drift count.

### Problem

The project has plans, todos, and messages, but lacks a first-class active goal. Claude Code's CLI shows task/background state, supports task notifications, and has session-level state. Priority Agent should make the user's current objective explicit.

### Implementation

Add `src/engine/session_goal.rs`:

```rust
pub struct SessionGoal {
    pub goal_id: String,
    pub title: String,
    pub user_objective: String,
    pub acceptance_criteria: Vec<String>,
    pub subgoals: Vec<Subgoal>,
    pub risks: Vec<String>,
    pub status: GoalStatus,
    pub last_progress: Option<String>,
    pub updated_at: DateTime<Utc>,
}
```

Goal update sources:

- User prompt.
- Plan mode output.
- Todo tool.
- Tool results.
- Test results.
- User corrections.

### CLI Surface

- `/goal`: show or edit current goal.
- `/quick`: include active goal, progress, blocked items, next action.
- Status bar: compact goal/progress indicator when useful.
- Final responses: mention goal completion only when the goal is materially advanced.

### Acceptance Criteria

- A non-trivial coding request creates or updates `SessionGoal`.
- User can inspect goal with `/goal`.
- Goal drift is detected when current work no longer matches objective.
- Completion requires acceptance criteria or verification, not just a model response.

## Phase 4: TaskContextBundle For Coding Workflows And Subagents

Status: partially complete.

Implemented highlights: task/workflow routing, sub-agent infrastructure, swarm,
role memory, and trace-backed context handoff pieces. Remaining work is to make a
single `TaskContextBundle` object the only context handoff format across every
reflection/recovery/eval path.

### Problem

Claude Code builds rich per-turn context and uses forked agents for memory/subtasks. Priority Agent has sub-agent and swarm modules, but context handoff should become standardized.

### Implementation

Add `src/engine/task_context.rs`:

```rust
pub struct TaskContextBundle {
    pub bundle_id: String,
    pub goal: Option<SessionGoalSummary>,
    pub user_request: String,
    pub workflow: WorkflowKind,
    pub relevant_files: Vec<FileContext>,
    pub search_results: Vec<SearchHit>,
    pub memory_items: Vec<MemoryContextItem>,
    pub constraints: Vec<String>,
    pub permissions: PermissionSnapshot,
    pub recent_trace: Vec<TraceEventSummary>,
    pub verification_commands: Vec<String>,
    pub expected_outputs: Vec<String>,
}
```

Use it for:

- sub-agent prompts
- reflection/review passes
- recovery attempts
- evalset replays
- final summaries

### Workflow Templates

Add `src/engine/workflows/`:

- `code_change`
  - understand -> retrieve -> plan -> edit -> test -> reflect -> summarize
- `bug_fix`
  - reproduce -> locate -> patch -> regression test -> explain
- `refactor`
  - map dependencies -> staged edits -> compatibility checks -> migration notes
- `docs`
  - inspect API -> update docs -> verify examples
- `research`
  - retrieve -> compare -> synthesize -> cite

### Acceptance Criteria

- Agent tool can receive a `TaskContextBundle`.
- Workflow trace shows template stages.
- Reflection pass receives a bundle, not ad hoc text.
- Coding tasks produce verification commands or explicitly record why not.

## Phase 5: RetrievalContext, Memory Audit, And LearningEvent

Status: mostly complete.

Implemented highlights: memory snapshots, prefetch, LLM rerank support,
learning events, tool outcome learning, namespace-aware memory search, and
conflict hints. Remaining work is a single provenance-bearing
`RetrievalContext` struct used by all retrieval sources.

### Problem

Memory and retrieval are present but fragmented. Claude Code uses memory files, session memory, background extraction, skill retention through compaction, and context-aware prompt assembly. Priority Agent should unify all retrieval into a provenance-bearing object.

### Implementation

Add `src/engine/retrieval_context.rs`:

```rust
pub struct RetrievalContext {
    pub items: Vec<RetrievedItem>,
    pub total_tokens: usize,
    pub budget_tokens: usize,
    pub policy: RetrievalPolicy,
}

pub struct RetrievedItem {
    pub source: RetrievalSource,
    pub title: String,
    pub content_preview: String,
    pub score: f32,
    pub freshness: Freshness,
    pub trust: TrustLevel,
    pub provenance: String,
    pub token_count: usize,
}
```

Sources:

- project index
- open files
- session history
- persistent memory
- auto-extracted memory
- web/MCP resources
- skills/commands

Add `LearningEvent`:

```rust
pub struct LearningEvent {
    pub task_type: String,
    pub route: String,
    pub tools_used: Vec<String>,
    pub outcome: Outcome,
    pub duration_ms: u64,
    pub cost: Option<f64>,
    pub errors: Vec<String>,
    pub tests_run: Vec<String>,
    pub user_feedback: Option<String>,
}
```

### Acceptance Criteria

- Every memory item shown to the model has provenance and reason.
- `/memory` can show why an item was saved or retrieved.
- Router can use prior `LearningEvent` stats.
- Background memory extraction is throttled/coalesced, with trace events.

## Phase 6: HumanReviewRequest, Permissions, And RecoveryPlan

Status: partially complete.

Implemented highlights: permission approvals, high-drift approval,
source-aware permission rules, recovery plans, `/recover`, and trace events.
Remaining work is a unified `HumanReviewRequest` queue that also covers
ask-user and plan approvals.

### Problem

Approvals, user questions, permission prompts, and plan approvals are separate surfaces. Claude Code exposes permission request, pre/post tool hooks, retry after denial, and settings precedence. Priority Agent needs a unified human review queue.

### Implementation

Add `src/engine/human_review.rs`:

```rust
pub struct HumanReviewRequest {
    pub request_id: String,
    pub kind: ReviewKind,
    pub reason: String,
    pub risk: RiskLevel,
    pub options: Vec<ReviewOption>,
    pub default_option: Option<String>,
    pub persistence_scope: ReviewScope,
    pub related_tool_call: Option<String>,
    pub related_trace_id: String,
}
```

Add `RecoveryPlan`:

```rust
pub struct RecoveryPlan {
    pub error_kind: String,
    pub root_cause: Option<String>,
    pub selected_action: RecoveryAction,
    pub fallback_actions: Vec<RecoveryAction>,
    pub user_visible_note: String,
    pub status: RecoveryStatus,
}
```

Use cases:

- permission denial with retry
- context overflow
- API overload/rate limit
- tool failure
- failed tests
- invalid model/tool output

### CLI Surface

- Better approval panel backed by `HumanReviewRequest`.
- `/reviews`: pending/answered review requests.
- `/recover`: show recent recovery attempts.
- Denied tool can be marked retryable and sent back to the model.

### Acceptance Criteria

- Permission, ask-user, and plan approval share one request model.
- User choices can persist for session/project/global scope.
- Denials can optionally produce retry guidance.
- Recovery attempts are visible in `/trace` and `/recover`.

## Phase 7: EvalSet Framework

Status: partially complete.

Implemented highlights: workflow replay docs, acceptance checklists, trace-based
unit/integration coverage, and 820 passing tests. Remaining work is a formal
`evalsets/` runner with scenario fixtures and trace assertions.

### Problem

Mature agent behavior cannot be judged only by unit tests. Claude Code's public ecosystem and docs imply many behavior contracts: hook order, permission blocking, status/config visibility, subagent coordination, compaction behavior. Priority Agent needs scenario tests.

### Implementation

Add `evalsets/`:

```text
evalsets/
  cli/
    permission_denial_retry.toml
    code_change_verify.toml
    memory_retrieval.toml
    context_compaction.toml
    subagent_chain.toml
```

Each eval:

```toml
name = "permission_denial_retry"
prompt = "Edit a protected file and then recover safely"
workspace_fixture = "fixtures/protected-file"
expected_events = ["IntentRouted", "PermissionRequested", "PermissionResolved", "RecoveryApplied"]
forbidden_tools = ["bash:rm -rf"]
required_final_contains = ["permission", "not modified"]
```

Add:

- `cargo test --test evalsets`
- `/eval run <name>` for interactive debug later.

### Acceptance Criteria

- At least 10 scenario evals.
- Eval runner can replay deterministic providers/tools.
- TurnTrace is used as the main assertion surface.
- CI can run smoke evals quickly.

## Phase 8: CLI Experience Built On The Runtime Spine

Status: mostly complete.

Implemented highlights: `/trace`, `/goal`, `/goal drift`, `/quick`, `/learn`,
`/recover`, `/mcp status`, memory namespace visibility, permission explanations,
and status panels backed by runtime state. Remaining work is continued UX polish
and a configurable statusline.

### Problem

The CLI is improving visually, but maturity should come from exposing actual runtime state.

### Implementation

Add/upgrade:

- `/trace`: turn timeline.
- `/goal`: current objective and acceptance criteria.
- `/quick`: active workflow, pending review, next action, recent failures.
- `/settings`: layered settings with source labels.
- `/permissions`: rule source, matched patterns, recent decisions.
- `/memory`: provenance and retrieval audit.
- `/workflow`: current template stage.
- `/statusline`: simple configurable status command later, inspired by Claude Code.

### Acceptance Criteria

- CLI panels are backed by real trace/goal/retrieval/review data.
- No decorative status that cannot be traced back to state.
- Compact and dense status modes still work.

## Recommended Development Order

1. `TurnTrace` data model, collector, persistence, and `/trace`.
2. Instrument tool execution, permissions, context manager, and memory.
3. `IntentRouter` rule-based v1 and workflow selection.
4. `SessionGoal` and `/goal`/`/quick` integration.
5. `TaskContextBundle` and workflow templates for coding tasks.
6. `RetrievalContext` and memory audit.
7. `HumanReviewRequest` and `RecoveryPlan`.
8. Evalset framework.
9. CLI panels polish based on the new state models.

This order is deliberate: once `TurnTrace` exists, every later change becomes easier to debug, test, and compare against Claude-like behavior.

## Near-Term Milestone: Trace-First Refactor

Target files:

- Add `src/engine/trace.rs`
- Update `src/engine/mod.rs`
- Update `src/session_store/mod.rs`
- Update `src/engine/conversation_loop/mod.rs`
- Update `src/engine/tool_orchestration.rs`
- Update `src/permissions/mod.rs`
- Update `src/memory/manager.rs`
- Update `src/tui/commands.rs`
- Add CLI renderer under `src/tui/components/` or `src/tui/screens/`

Milestone deliverables:

- `TurnTrace` exists and is persisted.
- `/trace` works.
- Latest turn records at least: prompt submitted, tool started/completed, permission requested/resolved, context compression, memory prefetch/save, assistant response.
- Tests pass with trace disabled and enabled.
