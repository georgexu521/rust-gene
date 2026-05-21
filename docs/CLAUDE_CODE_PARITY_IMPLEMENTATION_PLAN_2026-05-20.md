# Claude Code Parity Implementation Plan

Date: 2026-05-20

Status: new active implementation plan.

Scope: tactical shift from "mostly inspired by Claude Code" to "first reach
Claude Code-like runtime parity, then personalize and diverge." This plan is
based on the current `rust-agent` checkout and the local Claude Code source at
`/Users/georgexu/Desktop/claude`.

This supersedes the older roadmap as the routing document for near-term code
work. It does not delete the long-term product principle that Priority Agent
should become narrow, deep, personal, and verifiable for gex. The order changes:
first build a strong mainstream coding-agent harness, then make it more personal
and differentiated.

## Position

gex's current direction is right as an engineering tactic.

The project should stop treating Claude Code as only a loose conceptual
reference. The current codebase already has many pieces with similar names, but
several are shallower than Claude Code's actual implementation. Continuing to
invent behavior from first principles risks building plausible but brittle
substitutes. The better path is:

1. Use the local Claude source as a concrete behavioral and architectural
   reference.
2. Translate the runtime semantics into original Rust modules.
3. Keep compatibility with the existing Priority Agent code and data stores.
4. Do not copy long prompt strings, UI text, or code bodies verbatim; copy the
   product semantics and state-machine shape.
5. Delay broad optimization and large test-matrix work until the core parity
   surfaces are in place.

In short: not "feature count parity"; not "prompt imitation"; first target
Claude Code's harness completeness.

## Evidence Reviewed

### Current Priority Agent

- Main runtime: `src/engine/conversation_loop/mod.rs` plus the controller files
  under `src/engine/conversation_loop/`.
- Tool system: `src/tools/mod.rs` and 63 Rust tool files under `src/tools/`.
- CLI/TUI: `src/tui/`, command registry, slash handlers, tool views, approval
  panels.
- State/session: `src/state/`, `src/session_store/`, `src/task_manager/`.
- Memory/context: `src/memory/`, `src/engine/retrieval_context.rs`,
  `src/engine/context_compressor.rs`, `src/engine/context_collapse.rs`.
- Subagents: `src/agent/`, `src/tools/agent_tool/`, `src/tools/task_tool/`.
- Permissions/hooks: `src/permissions/`, `src/engine/hooks.rs`,
  `src/engine/destructive_scope.rs`.
- Current status source: `docs/PROJECT_STATUS.md`.
- Existing alignment docs:
  - `docs/CLAUDE_CODE_ALIGNMENT_PLAN.md`
  - `docs/CLAUDE_CODE_GAP_MATRIX_2026-05-03.md`
  - `docs/AGENT_PRODUCTIZATION_REFERENCE_AUDIT_2026-05-10.md`
  - `docs/LLM_RUNTIME_SIMPLIFICATION_PLAN_2026-05-08.md`
  - `docs/PERSONAL_AGENT_PRODUCT_PRINCIPLES_2026-05-18.md`

Current repo shape from this pass:

```text
rust-agent Rust files by top module:
engine 144
tools 63
tui 29
services 10
agent 10
memory 9
skills 7
migrations 7
```

The project is no longer missing the obvious skeleton. The risk is that the
skeleton is not yet the same product-grade runtime as Claude Code.

### Local Claude Code Source

The local Claude source is much wider and deeper than the previous gap lists
captured:

```text
Claude source files by top module:
utils 564
components 389
commands 189
tools 184
services 130
hooks 104
ink 96
bridge 31
constants 21
skills 20
cli 19
```

Key reference areas:

- Query/session lifecycle:
  - `/Users/georgexu/Desktop/claude/src/QueryEngine.ts`
  - `/Users/georgexu/Desktop/claude/src/query.ts`
- Tool contract:
  - `/Users/georgexu/Desktop/claude/src/Tool.ts`
  - `/Users/georgexu/Desktop/claude/src/tools.ts`
  - `/Users/georgexu/Desktop/claude/src/services/tools/toolExecution.ts`
  - `/Users/georgexu/Desktop/claude/src/services/tools/toolHooks.ts`
  - `/Users/georgexu/Desktop/claude/src/services/tools/toolOrchestration.ts`
- Core tools:
  - `/Users/georgexu/Desktop/claude/src/tools/BashTool/`
  - `/Users/georgexu/Desktop/claude/src/tools/FileReadTool/`
  - `/Users/georgexu/Desktop/claude/src/tools/FileEditTool/`
  - `/Users/georgexu/Desktop/claude/src/tools/FileWriteTool/`
  - `/Users/georgexu/Desktop/claude/src/tools/GrepTool/`
  - `/Users/georgexu/Desktop/claude/src/tools/GlobTool/`
  - `/Users/georgexu/Desktop/claude/src/tools/TodoWriteTool/`
- State and UI:
  - `/Users/georgexu/Desktop/claude/src/state/AppStateStore.ts`
  - `/Users/georgexu/Desktop/claude/src/components/`
  - `/Users/georgexu/Desktop/claude/src/ink/`
  - `/Users/georgexu/Desktop/claude/src/commands/`
- Context and memory:
  - `/Users/georgexu/Desktop/claude/src/services/compact/`
  - `/Users/georgexu/Desktop/claude/src/services/SessionMemory/`
  - `/Users/georgexu/Desktop/claude/src/services/extractMemories/`
  - `/Users/georgexu/Desktop/claude/src/utils/context.ts`
  - `/Users/georgexu/Desktop/claude/src/utils/fileHistory.ts`
- Subagents and tasks:
  - `/Users/georgexu/Desktop/claude/src/tools/AgentTool/`
  - `/Users/georgexu/Desktop/claude/src/tasks/`
  - `/Users/georgexu/Desktop/claude/src/utils/forkedAgent.ts`
  - `/Users/georgexu/Desktop/claude/src/tools/shared/spawnMultiAgent.ts`
- Permissions, plugins, MCP, bridge:
  - `/Users/georgexu/Desktop/claude/src/utils/permissions/`
  - `/Users/georgexu/Desktop/claude/src/types/hooks.ts`
  - `/Users/georgexu/Desktop/claude/src/plugins/`
  - `/Users/georgexu/Desktop/claude/src/services/mcp/`
  - `/Users/georgexu/Desktop/claude/src/bridge/`
  - `/Users/georgexu/Desktop/claude/src/remote/`

## Core Diagnosis

Priority Agent has many Claude-like nouns: `ConversationLoop`, `Tool`,
`AppState`, `Agent`, `PermissionContext`, `HookManager`, `SessionStore`,
`MemoryManager`, `MCP`, `PlanMode`, `Trace`.

Claude Code's advantage is that those nouns are wired as a coherent runtime:

1. A `QueryEngine` owns conversation state across turns.
2. A mutable `ToolUseContext` carries app state, file cache, permissions, MCP,
   agents, notifications, hooks, progress, compact callbacks, and per-turn
   tracking.
3. A central `AppState` is the live product state for permissions, tasks,
   agents, MCP, plugins, file history, todos, bridge, notifications, and UI.
4. Each tool is a product object, not just a function with JSON params.
5. Tool orchestration separates concurrency-safe read batches from serial
   mutating batches and applies context mutations in order.
6. Hooks and permissions are part of tool execution, not afterthoughts.
7. Compaction, session memory, and memory extraction are runtime events with
   state, metrics, and boundaries.
8. Subagents inherit or fork context with explicit tools, permissions, MCP,
   memory, transcript, and cleanup behavior.
9. UI panels render runtime state directly rather than relying on the model to
   explain what happened.

The implementation target is therefore:

```text
Priority Agent vNext
  SessionRuntime
    AppRuntimeState
    QuerySessionProcessor
    TurnRuntimeContext
    ToolUseContext
    ToolExecutionRecord
    PermissionReviewQueue
    HookRuntime
    ContextCompactionRuntime
    AgentTaskRuntime
    EvidenceLedger
    TuiRuntimeViews
```

## Non-Goals For This Phase

- Do not redesign every module from scratch.
- Do not rewrite the whole project in one pass.
- Do not add a large new planning framework into normal turns.
- Do not make tests and eval optimization the main work right now.
- Do not claim Claude Code parity from tool count or command count.
- Do not copy Claude Code source text line-for-line. Implement original Rust
  with equivalent behavior.

## Success Definition

This plan is complete enough when Priority Agent can do a broad real coding task
with Claude Code-like behavior:

1. It understands the repo, reads targeted files, edits correctly, and runs
   relevant validation without being over-prompted.
2. Core tools expose rich semantics: validation, permission, read-only status,
   concurrency safety, summaries, activity labels, result schemas, evidence,
   and recovery.
3. Permission approvals, denied tools, hidden tools, hooks, file snapshots,
   tool failures, validation evidence, and final closeout are all runtime-owned.
4. Subagents can be used for real parallel work with context isolation, scoped
   tools, visible progress, durable results, and cleanup.
5. Long sessions compact and preserve working memory without losing the active
   task.
6. TUI views show what matters: status, tool progress, diffs, approvals,
   context pressure, tasks, agents, and trace.

## Workstreams

The work should run as staged code batches. Each batch can include minimal
targeted verification, but broad test/performance tuning is deferred until the
core code surfaces exist.

### Stream A: Session Runtime And Query Loop

Goal: make the main runtime structurally closer to Claude `QueryEngine` and
`query.ts`.

Current landing files:

- `src/engine/conversation_loop/mod.rs`
- `src/engine/conversation_loop/session_processor.rs`
- `src/engine/query_engine.rs`
- `src/engine/streaming.rs`
- `src/engine/turn_state.rs`
- `src/engine/prompt_context.rs`
- `src/engine/retrieval_context.rs`

Target design:

```rust
SessionRuntime
  owns SessionRuntimeState
  owns QuerySessionProcessor
  creates TurnRuntimeContext per user turn
  streams RuntimeEvent
  persists ToolExecutionRecord and TurnTrace
```

Implementation tasks:

1. Introduce `SessionRuntimeState` as the Rust equivalent of Claude's long-lived
   query/session state:
   - session id
   - mutable messages
   - active cwd
   - app/runtime state handle
   - permission context
   - file history/cache
   - MCP state snapshot
   - loaded memory paths
   - discovered skills
   - active tool ids
   - active task/agent ids
   - usage/cost totals
2. Introduce `TurnRuntimeContext` as the per-turn object passed through
   controllers:
   - user prompt
   - route/resource policy
   - exposed tools
   - tool context
   - trace/evidence ledger
   - memory/retrieval context
   - hook runtime
   - approval queue
   - compaction callbacks
3. Move remaining `run_inner` ad hoc state into those objects in small batches.
4. Make `SessionProcessor` the owner of one model step:
   - prepare provider request
   - stream assistant content
   - collect tool calls
   - normalize provider protocol edge cases
   - return `SessionStepResult`
5. Keep `ToolExecutionController` as the owner of tool execution, but feed it
   `ToolUseContext` instead of scattered arguments.
6. Add a `RuntimeEvent` enum that can feed both TUI and trace.
7. Preserve current provider request shape while moving ownership.

Acceptance:

- A normal code-change turn still works.
- The main loop has fewer raw local variables and fewer cross-controller
  argument lists.
- Tool execution, permission, memory, and closeout receive the same
  `TurnRuntimeContext` facts.
- `/trace last` still shows route, tools, permissions, memory, and validation.

### Stream B: Tool Contract V2

Goal: make Priority Agent tools product-grade like Claude's `Tool` interface.

Current landing files:

- `src/tools/mod.rs`
- `src/engine/conversation_loop/tool_metadata.rs`
- `src/engine/conversation_loop/tool_result_controller.rs`
- `src/engine/conversation_loop/tool_execution_controller.rs`
- `src/session_store/mod.rs`
- provider adapters under `src/services/api/`

Claude reference semantics:

- schema and optional output schema
- validate input before permissions
- tool-specific permission check
- read-only and concurrency-safe predicates
- destructive predicate
- search/read/list classification
- max result size policy
- user-facing name
- compact summary
- current activity description
- classifier input
- provider payload mapping
- UI rendering metadata

Implementation tasks:

1. Extend the Rust `Tool` trait in a backwards-compatible way:
   - `output_schema()`
   - `is_read_only(params)`
   - `is_concurrency_safe(params)`
   - `is_destructive(params)`
   - `is_search_or_read(params)`
   - `max_result_size_chars()`
   - `user_facing_name(params)`
   - `tool_use_summary(params)`
   - `activity_description(params)`
   - `validate_input(params, context)`
   - `check_permissions(params, context)`
   - `provider_payload(result)`
2. Do not force all tools to implement everything immediately. Add defaults
   first, then deepen core tools.
3. Introduce `ToolExecutionRecord` as the normalized result object:
   - call id
   - tool name
   - input
   - status: pending, running, completed, failed, denied, blocked, cancelled
   - user-visible output
   - model-visible payload
   - machine metadata
   - evidence kind
   - permission decision
   - read/write classification
   - started/ended timestamps
   - duration
   - artifact paths
4. Make `ToolResultNormalizer` produce provider-safe payloads from
   `ToolExecutionRecord`.
5. Persist records in the session store or a new migration-backed table.
6. Route final closeout and trace through these records instead of rescanning
   raw strings.

Core-tool upgrade order:

1. `file_read`, `file_edit`, `file_write`, `file_patch`
2. `bash`, `bash_output`, `bash_cancel`, `bash_tasks`
3. `grep`, `glob`
4. `todo_write`
5. `agent`, `task_create`, `task_output`, `task_stop`
6. `mcp`, `mcp_auth`, `list_mcp_resources`, `read_mcp_resource`
7. `web_fetch`, `web_search`
8. `worktree`, `git`, `lsp`

Acceptance:

- Tool execution state is durable and queryable.
- TUI and final closeout can show a tool summary without parsing text.
- Read-only tools can safely run in concurrent batches.
- Mutating tools remain serial.
- Provider tool-result payload is generated in one place.

### Stream C: App Runtime State

Goal: replace the small current `AppState` with a central runtime state closer
to Claude's `AppStateStore`.

Current landing files:

- `src/state/app_state.rs`
- `src/state/store.rs`
- `src/state/events.rs`
- `src/tui/app.rs`
- `src/session_store/mod.rs`
- `src/task_manager/mod.rs`

Target state fields:

- settings snapshot and origins
- main model and fallback model
- status line text
- current expanded view
- permission context
- active cwd and allowed directories
- tasks and agent registry
- foregrounded/background task ids
- MCP clients/tools/resources/prompts/status
- plugin enabled/disabled/errors/refresh state
- file history and checkpoints
- todos per agent
- notifications queue
- elicitation/user-question queue
- active hooks
- bridge/remote state
- tool ids in progress
- context pressure and compact state

Implementation tasks:

1. Add `RuntimeAppState` without deleting the current `AppState` immediately.
2. Add a thin store API:
   - read snapshot
   - update with closure
   - subscribe to events
   - emit TUI diff events
3. Move permission mode and session rules into `RuntimeAppState`.
4. Move MCP status/tool/resource snapshots into `RuntimeAppState`.
5. Move task and agent registry into `RuntimeAppState`.
6. Move file history/checkpoint summary into `RuntimeAppState`.
7. Replace direct TUI reads from scattered managers with state selectors.
8. Add `RuntimeStateEvent` records for UI updates and trace links.

Acceptance:

- `/status` reads from one state model.
- Tool view can show active tool ids from state.
- Approval panels, tasks, agents, MCP, and file history share state selectors.
- Background/subagent tasks can update root state safely.

### Stream D: Core Tool Productization

Goal: deepen the core tools rather than adding new names.

#### D1: File Tools And File History

Current landing files:

- `src/tools/file_tool/mod.rs`
- `src/tools/file_tool/patch.rs`
- `src/tools/file_tool/history.rs`
- `src/engine/checkpoint.rs`
- `src/tools/rewind_tool/`
- `src/tools/resume_tool/`

Claude reference areas:

- `FileReadTool`
- `FileEditTool`
- `FileWriteTool`
- `fileHistory.ts`

Tasks:

1. Make file read state part of `ToolUseContext`, not only tool-local cache.
2. Track read coverage, content hash, mtime, encoding, line endings, and
   selected line ranges.
3. Require edit tools to reference current file identity and stale-read state
   when applicable.
4. Ensure every write/edit/patch creates a checkpoint before mutation.
5. Store file-change records in `ToolExecutionRecord`.
6. Make `/diff`, `/rewind`, and closeout read from the same file history.
7. Add a real "restore checkpoint" path for the common cases.

Acceptance:

- File edits have runtime-owned before/after evidence.
- Stale edits are blocked or clearly warned.
- Rewind/restore is not a placeholder path.
- Final answers can cite changed paths and validation from evidence records.

#### D2: Bash And Terminal Tasks

Current landing files:

- `src/tools/bash_tool/mod.rs`
- `src/tools/bash_tool/background.rs`
- `src/tools/bash_tool/command_classifier.rs`
- `src/tui/tool_view.rs`
- `src/tui/screens/main_screen.rs`

Claude reference areas:

- `BashTool`
- `LocalShellTask`
- bash permission helpers
- command semantics
- read-only validation

Tasks:

1. Keep current foreground/background/PTV foundation, but make task state a
   first-class runtime object.
2. Extend command classification from broad categories to structured facts:
   - reads/list/search
   - validation/test
   - dev server
   - package install
   - git mutation
   - file mutation
   - destructive
   - interactive/PTY required
   - network access
   - external path access
3. Add parsed path facts to permission and evidence.
4. Make background command handles visible in `/status`, `/tasks`, and trace.
5. Make `bash_output` and `bash_cancel` operate on durable task ids.
6. Treat validation commands as evidence records with exit status and command
   family.
7. Improve failure feedback so the model knows whether to retry, inspect
   output, switch PTY, or ask permission.

Acceptance:

- Interactive commands produce PTY recovery guidance instead of hanging.
- Background tasks are not lost after a turn.
- Validation evidence is machine-readable.
- Permission prompt can explain shell risk with command-specific facts.

#### D3: Search Tools

Current landing files:

- `src/tools/grep_tool/mod.rs`
- `src/tools/glob_tool/mod.rs`
- `src/engine/conversation_loop/retrieval_context_builder.rs`

Tasks:

1. Standardize search/read/list metadata.
2. Add no-result recovery hints into structured metadata.
3. Add result truncation and artifact rules comparable to core tool policy.
4. Connect grep/glob records to retrieval provenance.
5. Make route-scoped tool exposure prefer search tools before broad reads on
   repo-analysis tasks.

Acceptance:

- A no-result grep does not become a blind loop.
- Search evidence is visible in trace and closeout.
- Result format is stable for model and TUI.

### Stream E: Permission, Hook, And Human Review Runtime

Goal: make permission and hooks one product path.

Current landing files:

- `src/permissions/mod.rs`
- `src/permissions/llm_classifier.rs`
- `src/engine/hooks.rs`
- `src/engine/conversation_loop/permission_controller.rs`
- `src/engine/conversation_loop/approval.rs`
- `src/engine/destructive_scope.rs`
- `src/tui/slash_handler/permissions.rs`
- `src/tui/screens/main_screen.rs`

Claude reference areas:

- `types/hooks.ts`
- `services/tools/toolHooks.ts`
- `utils/hooks/`
- `utils/permissions/`
- permission UI components

Tasks:

1. Define one `HumanReviewRequest` model for:
   - tool permission
   - plan approval
   - ask-user questions
   - hook-originated questions
   - destructive scope escalation
   - subagent permission bubbling
2. Define one `PermissionReview` record:
   - tool call id
   - tool name
   - input summary
   - risk facts
   - matched rules
   - classifier result
   - hook decision
   - user reply: once, always, reject
   - scope/pattern persisted
3. Upgrade hooks from env-only command hooks to typed lifecycle hooks:
   - session start/end
   - user prompt submit
   - pre tool use
   - post tool use
   - post tool failure
   - permission request
   - permission denied
   - subagent start/end
   - pre/post compact
   - cwd/file changes
   - notification
4. Allow hooks to:
   - add context
   - update input
   - ask
   - allow
   - deny
   - block continuation
   - run async with timeout
5. Preserve current simple env hooks as one hook provider.
6. Add project/user hook config after the runtime type is stable.
7. Surface hook records in `/trace`, `/status`, and permission explanation.

Acceptance:

- A pre-tool hook can block a risky tool and produce a structured denial.
- A hook allow does not bypass explicit deny rules.
- User approvals can be once/always/reject.
- Subagents can bubble approval requests or auto-deny depending on profile.

### Stream F: Context, Compaction, Memory, And Skills

Goal: make long-session behavior Claude-like without prompt bloat.

Current landing files:

- `src/engine/context_compressor.rs`
- `src/engine/context_collapse.rs`
- `src/engine/conversation_loop/context_budget_controller.rs`
- `src/engine/conversation_loop/preflight_compression_controller.rs`
- `src/engine/conversation_loop/memory_snapshot_controller.rs`
- `src/engine/conversation_loop/memory_sync_controller.rs`
- `src/memory/manager.rs`
- `src/engine/retrieval_context.rs`
- `src/skills/`

Claude reference areas:

- `services/compact/`
- `services/SessionMemory/`
- `services/extractMemories/`
- `skills/`
- `loadSkillsDir.ts`
- memory file utilities

Tasks:

1. Define compaction as a first-class runtime state:
   - current token pressure
   - strategy used
   - messages preserved
   - summary messages produced
   - boundary event
   - post-compact tool/memory carryover
2. Add explicit compaction strategies:
   - no-op under budget
   - snip/read-search collapse
   - microcompact for tool-result bloat
   - auto compact for long conversation
   - reactive compact after provider context errors
   - session-memory compact for durable long tasks
3. Make compaction boundaries visible to trace and session storage.
4. Move memory extraction into a background/forked agent mode:
   - throttled
   - best effort
   - skipped if main agent already wrote memory
   - records paths and duration
   - never blocks user turn completion
5. Make nested project memory and skill triggers explicit in `ToolUseContext`.
6. Preserve active skills through compaction within a budget.
7. Add memory provenance to the prompt only when useful to the current route.

Acceptance:

- Long conversations do not lose the active task after compaction.
- Memory extraction cannot pollute context with low-quality saves.
- `/trace last` shows why memory/skills were included.
- Session memory and long-term memory are separate runtime concerns.

### Stream G: Subagents, Tasks, Worktrees, And Parallel Work

Goal: make subagents dependable enough for real code work.

Current landing files:

- `src/agent/`
- `src/tools/agent_tool/`
- `src/tools/task_tool/`
- `src/engine/swarm.rs`
- `src/engine/worktree.rs`
- `src/tools/worktree_tool/`
- `src/session_store/mod.rs`

Claude reference areas:

- `tools/AgentTool/`
- `tasks/`
- `utils/forkedAgent.ts`
- `tools/shared/spawnMultiAgent.ts`

Tasks:

1. Define `AgentDefinition` records:
   - name/type
   - when to use
   - tools
   - permission mode
   - model policy
   - max turns
   - MCP servers
   - memory policy
   - output contract
2. Implement context modes:
   - minimal
   - inherited summary
   - full fork
   - isolated worktree fork
3. Implement agent task state:
   - pending/running/completed/failed/cancelled
   - transcript path
   - tool ids in progress
   - permission requests
   - result artifact
   - cleanup hooks
4. Add forked context building:
   - preserve relevant parent history
   - insert deterministic placeholder results where needed
   - add child directive
   - prevent recursive uncontrolled forking
5. Add agent-specific MCP and tool resolution.
6. Add worktree-isolated execution for mutating parallel workers.
7. Add result fusion only after durable agent result artifacts exist.
8. Surface agents in TUI task/status panels.

Acceptance:

- Parent can spawn explorer/verifier/worker agents with scoped tools.
- Mutating workers can use isolated worktrees.
- Agent outputs are durable and trace-linked.
- Permission handling is explicit for background agents.

### Stream H: TUI, Slash Commands, And Product Surfaces

Goal: move from "commands exist" to Claude-like daily-use surfaces.

Current landing files:

- `src/tui/app.rs`
- `src/tui/commands.rs`
- `src/tui/tool_view.rs`
- `src/tui/screens/`
- `src/tui/components/`
- `src/tui/slash_handler/`

Claude reference areas:

- `components/`
- `commands/`
- `ink/`
- `hooks/use*`
- `components/permissions/`
- `components/diff/`
- `components/mcp/`
- `components/tasks/`
- `components/memory/`
- `components/skills/`

Priority UI surfaces:

1. Status line and `/status`
2. Tool progress and expandable tool output
3. Diff viewer
4. Permission approval panel
5. Context pressure visualization
6. Trace viewer
7. Task/agent panel
8. MCP status/auth/repair panel
9. Session resume/rewind panel
10. Command palette/help with maturity labels

Tasks:

1. Make TUI read from `RuntimeAppState` selectors.
2. Use `ToolExecutionRecord` for tool rows.
3. Add stable diff panel backed by file history/checkpoints.
4. Make permission panel show risk facts and once/always/reject choices.
5. Add context/memory/skill panel using runtime provenance.
6. Convert placeholder slash commands into honest states:
   - production
   - usable
   - scaffold
   - hidden unless enabled
7. Add Claude-like command families in implementation order:
   - `/status`, `/doctor`, `/trace`
   - `/permissions`
   - `/diff`, `/rewind`, `/resume`
   - `/mcp`
   - `/agents`, `/tasks`
   - `/memory`, `/skills`
   - `/hooks`
   - `/config`, `/model`, `/output-style`
   - `/export`, `/share`
   - `/bridge`, `/remote`
8. Keep command text concise. The UI should show runtime facts, not explain the
   whole architecture.

Acceptance:

- During a coding task, gex can see what the agent is doing without reading raw
  logs.
- Permission and diff review feel like product flows, not debug output.
- Placeholder commands do not pretend to be implemented.

### Stream I: MCP, Plugins, Bridge, And External Integration

Goal: close integration depth after the local harness is stable.

Current landing files:

- `src/engine/mcp.rs`
- `src/tools/mcp_tool/`
- `src/plugins/`
- `src/bridge/`
- `src/remote/`
- `src/skills/`

Claude reference areas:

- `services/mcp/`
- `plugins/`
- `bridge/`
- `remote/`
- `commands/mcp/`
- `commands/plugin/`
- `commands/bridge/`

Tasks:

1. MCP:
   - tools/resources/prompts/commands as first-class runtime facts
   - approval and auth repair flows
   - prompt/command exposure through slash commands
   - health status and reconnect
   - per-agent MCP servers
2. Plugins:
   - plugin metadata
   - tool/command/skill contributions
   - enable/disable state
   - reload
   - error reporting
   - trusted source policy
3. Bridge/remote:
   - remote session state in `RuntimeAppState`
   - inbound/outbound message queue
   - permission callbacks
   - status UI
   - durable session ids and reconnect

Acceptance:

- MCP failures are diagnosable and repairable from CLI.
- Plugin state changes can be reloaded without restarting.
- Bridge state is visible and not a hidden background process.

### Stream J: Provider Protocol And Model Runtime

Goal: keep provider support stable while changing runtime internals.

Current landing files:

- `src/services/api/`
- `src/engine/conversation_loop/api_request_controller.rs`
- `src/engine/conversation_loop/session_processor.rs`
- `src/engine/error_classifier.rs`

Tasks:

1. Keep OpenAI-compatible, MiniMax, Kimi, and future provider conversion behind
   normalized message/tool-result records.
2. Add explicit provider capability records:
   - tool calling
   - parallel tool calls
   - reasoning/thinking blocks
   - image/pdf support
   - max context
   - fallback model
3. Make fallback model selection a runtime policy, not prompt text.
4. Treat provider protocol errors as typed failures:
   - orphan tool result
   - missing tool result
   - prompt too long
   - rate limit
   - auth
   - unsupported schema
5. Use reactive compaction or model fallback based on typed failures.

Acceptance:

- Provider adapters consume normalized messages.
- Tool-result ordering bugs are caught at the protocol boundary.
- Context-too-long recovery does not require the model to guess the fix.

## Detailed Phase Order

### Phase 0: Freeze Current Parity Map

Purpose: prevent another vague roadmap.

Tasks:

1. Build a Claude-to-Rust map for:
   - query loop
   - tool contract
   - app state
   - core tools
   - hooks
   - permissions
   - compaction
   - memory
   - agents
   - tasks
   - UI panels
   - MCP/plugins/bridge
2. For each Claude module, record:
   - product semantics
   - current Rust equivalent
   - missing runtime state
   - target Rust file
   - first code slice
3. Add a small `docs/CLAUDE_CODE_PARITY_MAP_2026-05-20.md` if this file becomes
   too large.

Deliverable:

- This plan plus a follow-up map when implementation starts.

### Phase 1: Tool Contract V2 Foundation

Why first: everything else depends on tool semantics.

Tasks:

1. Add default methods to `Tool`.
2. Add `ToolExecutionRecord`.
3. Teach `ToolExecutionController` to emit records.
4. Teach `ToolResultNormalizer` to build model/UI payloads from records.
5. Update `file_read`, `file_edit`, `bash`, `grep`, `glob` to fill the first
   meaningful fields.

Implementation notes:

- Keep the old `ToolResult` during migration.
- Do not break existing provider requests.
- Avoid a broad refactor of every tool in one commit.

Expected files:

- `src/tools/mod.rs`
- `src/engine/conversation_loop/tool_execution_record.rs`
- `src/engine/conversation_loop/tool_execution_controller.rs`
- `src/engine/conversation_loop/tool_result_controller.rs`
- `src/engine/conversation_loop/tool_metadata.rs`
- core tool files.

Progress, 2026-05-20:

- Done: added Tool Contract V2 defaults to `Tool`: operation kind, read-only
  status, concurrency safety, destructive flag, output schema hook, result-size
  hint, invocation summary, activity text, and provider-payload hook.
- Done: wired `ToolSchema::from_tool` to expose tool output schema when a tool
  provides one.
- Done: upgraded `bash`, `file_read`, `file_write`, `file_edit`, `file_patch`,
  `grep`, and `glob` with first-pass operation/concurrency semantics.
- Done: changed `ToolExecutionController` and streaming read-only pre-execution
  to use the tool contract instead of only the static tool-name allowlist.
- Decision: do not create a second top-level `ToolExecutionRecord` yet. The repo
  already has the rich record type in `src/engine/evidence_ledger.rs`; the
  smaller record in `tool_metadata.rs` is a provider-payload helper. The next
  batch should bridge the execution controller to the evidence-ledger record and
  then retire or rename the helper shape if it still creates ambiguity.
- Done: `ToolExecutionController` now attaches Tool Contract V2 facts to
  `tool_contract` and `tool_summary`; the evidence-ledger `ToolExecutionRecord`
  now preserves operation kind, read-only status, concurrency safety, and
  destructive flag.
- Done: renamed the provider-payload helper in `tool_metadata.rs` to
  `ProviderToolResultRecord` so it no longer looks like the evidence-ledger
  `ToolExecutionRecord`.
- Validation: `cargo test -q tool_contract`,
  `cargo test -q provider_tool_result`,
  `cargo test -q test_tool_call_concurrency_uses_tool_contract`,
  `cargo fmt --check`, and `cargo check -q`.

### Phase 2: Runtime App State Skeleton

Why second: tool execution, permissions, tasks, and UI need a shared product
state.

Tasks:

1. Add `RuntimeAppState`.
2. Add selectors for status/tool/tasks/permission/MCP.
3. Wire TUI status and tool-view reads to selectors.
4. Move active tool ids and permission context into runtime state.

Expected files:

- `src/state/runtime_state.rs`
- `src/state/store.rs`
- `src/tui/app.rs`
- `src/tui/tool_view.rs`
- `src/tui/slash_handler/observability.rs`
- `src/engine/conversation_loop/mod.rs`

Progress, 2026-05-20:

- Done: added `RuntimeAppState` with runtime tool, permission, and MCP
  snapshots plus selectors for status, tools, permission, MCP, and tool-viewer
  selection.
- Done: attached `RuntimeAppState` to the existing `AppState`/`StateStore`
  rather than creating a parallel state system.
- Done: wired TUI stream tool events into a runtime-state snapshot during
  response refresh/finalization.
- Done: status bar active-tool helpers now prefer runtime-state facts, with the
  older `ToolRunView` snapshot as fallback.
- Done: `/status` now includes runtime active-tool counts, current active tool,
  pending permission, runtime MCP summary, and runtime task summary.
- Done: tool viewer selection now uses the runtime-state selector before
  falling back to the older visible-tool lookup.
- Validation: `cargo test -q runtime_state`, `cargo test -q open_tool_viewer`,
  `cargo test -q status_bar`, `cargo fmt --check`, and `cargo check -q`.

### Phase 3: TurnRuntimeContext And SessionRuntimeState

Why third: after tools and app state have target shapes, reduce loop coupling.

Tasks:

1. Introduce `SessionRuntimeState`.
2. Introduce `TurnRuntimeContext`.
3. Move route/resource/exposed-tools/tool-context/trace/evidence inputs into
   the new context.
4. Shrink long function argument lists in controllers.
5. Keep behavior identical.

Expected files:

- `src/engine/conversation_loop/runtime_state.rs`
- `src/engine/conversation_loop/turn_runtime_context.rs`
- `src/engine/conversation_loop/mod.rs`
- controller files under `src/engine/conversation_loop/`.

Progress, 2026-05-20:

- Done: added `turn_runtime_context.rs` with `TurnRuntimeContext` for
  per-turn immutable runtime inputs and an initial `SessionRuntimeState`
  placeholder.
- Done: migrated the tool-round chain (`TurnIterationController` ->
  `TurnToolRoundStepController` -> `ToolRoundController`) to pass
  `TurnRuntimeContext` instead of repeatedly threading route/resource/trace/tx,
  working-directory, validation-command, destructive-scope, and git-baseline
  fields.
- Decision: keep `TurnRuntimeState` as the active per-turn mutable state file
  instead of adding another `runtime_state.rs` with overlapping ownership.
- Validation: `cargo test -q turn_runtime_context`,
  `cargo test -q empty_round_returns_empty_round_state`, `cargo fmt --check`,
  and `cargo check -q`.

### Phase 4: File History And Rewind Become Real

Why fourth: Claude-like autonomy depends on reliable rollback.

Tasks:

1. Connect file read state, file changes, checkpoints, and session store.
2. Ensure every mutation has a checkpoint record.
3. Make `/diff` read from file history.
4. Make `/rewind` restore known checkpoints.
5. Make closeout cite file change evidence from records.

Progress, 2026-05-20:

- Current baseline found: file tools already create checkpoints and write
  `FileChangeRecord` entries through `src/tools/file_tool/history.rs`; `/rollback
  last-file --yes`, `/rollback <file_change_id> --yes`, `/checkpoints`, and
  `/restore <checkpoint_id>` already use `CheckpointManager`.
- Done: converted `/rewind` to an async checkpoint-backed command. It now lists
  recent checkpoint-backed file changes, restores `last-file`, restores explicit
  `fc_*` file-change IDs, and restores the latest tracked change for a path
  before falling back to the legacy session edit snapshot.
- Done: converted `/diff` to prefer checkpoint-backed file-change diffs for the
  latest change, `last-file`, explicit `fc_*` IDs, and tracked file paths; git
  ranges such as `HEAD~3..HEAD` still use the existing git diff path.
- Decision: keep legacy session-manager rewind as a fallback only, because the
  checkpoint/file-change history is now the authoritative file rollback path.
- Validation: `cargo test -q rewind`, `cargo test -q diff`,
  `cargo fmt --check`, and `cargo check -q`.

Expected files:

- `src/tools/file_tool/`
- `src/engine/checkpoint.rs`
- `src/session_store/mod.rs`
- `src/tools/diff_tool/`
- `src/tools/rewind_tool/`
- `src/tui/slash_handler/session.rs`

### Phase 5: Bash And Terminal Task Parity

Why fifth: coding agents live or die on shell execution.

Tasks:

1. Make terminal task records durable.
2. Extend command facts and path facts.
3. Improve risk/permission integration.
4. Make background/PTY task status visible in `/status` and TUI.
5. Normalize validation command records.

Expected files:

- `src/tools/bash_tool/`
- `src/permissions/mod.rs`
- `src/tui/tool_view.rs`
- `src/tui/slash_handler/runtime.rs`
- `src/engine/conversation_loop/closeout_controller.rs`

Progress, 2026-05-20:

- Done: extended bash tool summaries and the evidence ledger with
  `path_patterns`, so command scope is now preserved from classification through
  `ToolExecutionRecord`, `CommandEvidence`, repair evidence, and provider
  machine metadata.
- Done: tightened bash path extraction so command subcommands like
  `cargo test` are not misclassified as path facts, while real path arguments
  such as `tests`, `src/lib.rs`, and `src/tools` remain visible.
- Done: normalized command identity is now persisted on command and tool
  execution records instead of being recomputed only at closeout time.
- Done: stream `ToolExecutionComplete` events now carry compact
  `tool_summary` metadata. TUI tool runs keep that metadata and prefer
  `terminal_task` status for background shell, PTY, timeout, cancel, and
  completion state.
- Done: `RuntimeAppState` now includes runtime terminal tasks, and the runtime
  status snapshot reports backgrounded tools, running terminal tasks, and PTY
  task counts. The status bar and `/status` can surface those counts from
  runtime state.
- Done: permission approval metadata for bash now carries command
  classification (`command_kind`, `command_category`, validation family,
  path patterns, closeout safety, and PTY requirement), aligning risk review
  records with shell semantics.
- Validation: `cargo test -q evidence_ledger`, `cargo test -q
  command_classifier`, `cargo test -q
  tool_execution_summary_includes_bash_path_patterns`, `cargo test -q
  runtime_state`, `cargo test -q shell_lifecycle`, `cargo test -q
  test_runtime_snapshot_keeps_terminal_task_metadata`, `cargo test -q
  bash_permission_metadata_includes_command_classification`, and `cargo check
  -q` all passed.

### Phase 6: Permission And Hook Runtime

Why sixth: after core tool semantics are durable, approvals and hooks can become
typed product flows.

Tasks:

1. Add `HumanReviewRequest`.
2. Add `PermissionReview`.
3. Upgrade hook event model.
4. Preserve env hooks as a provider.
5. Add once/always/reject flow.
6. Wire permission/hook records into trace and TUI.

Expected files:

- `src/engine/human_review.rs`
- `src/engine/hooks.rs`
- `src/permissions/mod.rs`
- `src/engine/conversation_loop/permission_controller.rs`
- `src/tui/screens/main_screen.rs`
- `src/tui/slash_handler/permissions.rs`

Progress, 2026-05-20:

- Done: confirmed the existing `HumanReviewRequest` layer is already active for
  tool permission, goal drift, plan approval, and reflection gate approvals.
- Done: added typed `PermissionReview`, `PermissionReviewDecision`, and
  `PermissionReviewOption` records. Permission reviews now expose the
  once/session/project/global/reject-global decision set as data instead of
  only hard-coded UI text.
- Done: moved permission rule-pattern calculation into the engine review layer
  and reused it from TUI permission handling, keeping MCP scoped rules like
  `mcp/<server>/<tool>` consistent.
- Done: `ToolApprovalRequest` can now produce a `PermissionReview`, and the
  permission approval renderer uses that review's rule pattern.
- Done: hook records now include a typed `HookProviderKind`; current env hooks
  are explicitly recorded as the `env` provider, and hook trace/TUI summaries
  carry that provider through.
- Done, 2026-05-21: upgraded the approval channel response from a bare boolean
  to `ToolApprovalResponse`, preserving the selected review decision
  (`approve_once`, `approve_session`, `approve_project`, `approve_global`,
  `reject_once`, `reject_always`), rule decision, persistence scope, rule
  pattern, saved config path, and note. TUI, shell, eval approval, and
  reflection-gate callers now share that response shape, and
  `permission.resolve` trace events expose the decision/scope/rule instead of
  only approved/denied.
- Done, 2026-05-21: added `HookLifecycleSnapshot` and `HookRegistration` so
  env-backed hooks have a structured lifecycle surface: configured hooks,
  provider, event, scope, timeout, fail-open/fail-closed policy, command
  preview, and recent success/failure/blocked statistics. `/hooks` now reuses
  the shared hook panel renderer, and `/panel hooks` plus `/panel all` expose
  the same product surface beside approvals, tasks, MCP, and bridge state.
- Done, 2026-05-21: connected hook failure, hook blocking, and permission
  denial into the shared recovery spine. Failed/blocked hook records now emit
  `hook_failed`/`hook_blocked` recovery plans with `/hooks` guidance, pre-tool
  hook blocks are treated as hook-runtime failures instead of generic dangerous
  tool blocks, and non-remote permission denials now emit trace-backed recovery
  plans pointing to `/permissions explain`.
- Validation: `cargo test -q human_review`, `cargo test -q
  bash_permission_metadata_includes_command_classification`, `cargo test -q
  test_pre_tool_hook_can_deny_execution`, and `cargo check -q` all passed.
- Validation, 2026-05-21: after this response-contract slice,
  `cargo fmt --check`, `cargo check -q`, `cargo test -q human_review`,
  `cargo test -q permission_controller`, `cargo test -q
  test_session_permission_rule_is_added_when_approving_for_session`, `cargo
  test -q test_respond_to_permission`, and `cargo test -q trace_summary` all
  passed.
- Validation, 2026-05-21: after the hook lifecycle surface slice,
  `cargo fmt --check`, `cargo check -q`, `cargo test -q hooks`, `cargo test -q
  runtime_panels`, `cargo test -q trace_summary`, and `cargo test -q commands`
  all passed.
- Validation, 2026-05-21: after the hook/permission recovery slice,
  `cargo fmt --check`, `cargo check -q`, `cargo test -q recovery_plan`,
  `cargo test -q turn_recording`, `cargo test -q hooks`, and `cargo test -q
  trace_summary` all passed.

### Phase 7: Context Compaction And Memory Runtime

Why seventh: once tool records are normalized, compaction can work on structured
message/tool evidence instead of raw text.

Tasks:

1. Make compaction boundary records durable.
2. Split strategies: snip, microcompact, auto compact, reactive compact,
   session-memory compact.
3. Add background/forked memory extraction.
4. Make skill and nested memory retention explicit.
5. Surface provenance in trace.

Expected files:

- `src/engine/context_compressor.rs`
- `src/engine/context_collapse.rs`
- `src/engine/conversation_loop/context_budget_controller.rs`
- `src/engine/conversation_loop/preflight_compression_controller.rs`
- `src/memory/manager.rs`
- `src/skills/`

Progress, 2026-05-20:

- Done: `ContextCompacted` trace events now carry compact-boundary metadata
  when a compression pass emits a boundary: boundary id, sequence, messages
  before/after, and preserved tail count.
- Done: preflight compression and reactive API-error compression both capture
  only the boundary created by that compression pass, avoiding stale metadata
  from earlier compactions.
- Done: trace rendering now includes the compact boundary id/sequence and
  message counts, giving `/trace` durable provenance for context compaction.
- Done: added runtime compaction records for `snip`, `microcompact`,
  `auto_compact`, and `reactive_compact`, including level, token/message
  deltas, compact-boundary ids, preserved tail counts, and provenance tags.
- Done: preflight and reactive compression now consume those runtime records
  directly, and add trigger provenance (`preflight`, `api_context_error`)
  instead of reconstructing trace fields from stale compact metadata.
- Done: session-memory compaction now keeps explicit user preferences in the
  injected summary and emits provenance tags for hot files, pending tasks,
  tool patterns, and preferences.
- Done: moved compact-boundary metadata, boundary extraction, compaction
  strategy labels, and runtime compaction records into `context_collapse`,
  leaving `context_compressor` as the compaction pipeline and compatibility
  re-export point.
- Done: fixed `context_collapse` persisted commit restore by writing collapsed
  messages under the same id stored in the collapse entry; added restore
  coverage for the persisted window prefix.
- Done: wired forked/background LLM memory extraction through
  `MemorySyncController` when `PRIORITY_AGENT_LLM_MEMORY_FORKED=1`, while the
  non-forked LLM path now marks extraction attempts for throttle and telemetry.
- Done: added explicit retained-context metadata to `ToolContext`, carrying
  per-turn retrieval provenance and skill triggers into tools, hooks,
  permissions, and future subagent context builders without injecting large
  prompt bodies into tools.
- Done: `TurnContextBootstrapController` now builds the retained context from
  retrieval results and `SkillRuntime` search, `TurnRuntimeContext` carries it
  through the tool round, and tool-result runtime metadata records retained
  retrieval/skill counts plus provenance.
- Validation: `cargo test -q test_compact_boundary_embedded_in_compression`,
  `cargo test -q records_preflight_budget_when_compressor_is_available`, and
  `cargo check -q` all passed.
- Validation: `cargo test -q context_compressor`,
  `cargo test -q records_preflight_budget_when_compressor_is_available`, and
  `cargo check -q` all passed after the runtime-record update.
- Validation: `cargo test -q context_collapse`,
  `cargo test -q context_compressor`, `cargo test -q memory_sync_controller`,
  `cargo test -q test_extraction_stats`, and `cargo check -q` all passed after
  the `context_collapse`/background-memory slice.
- Validation: `cargo test -q retained_context`,
  `cargo test -q test_unexposed_tool_call_is_denied_before_execution`,
  `cargo test -q turn_runtime_context`,
  `cargo test -q empty_round_returns_empty_round_state`,
  `cargo test -q plain_model_response_breaks_iteration`, and `cargo check -q`
  all passed after the retained-context slice.
- Phase 7 code status: complete enough to move into Phase 8. Remaining work is
  product hardening and real-task validation rather than another Phase 7 code
  prerequisite.

### Phase 8: Subagent And Task Runtime

Why eighth: parallel work should build on stable tool, permission, and state
records.

Tasks:

1. Add `AgentDefinition`.
2. Add context modes.
3. Add durable task/agent state.
4. Add forked context builder.
5. Add worktree-isolated workers.
6. Add agent-specific MCP/tool scopes.
7. Add visible agent panel/status.

Expected files:

- `src/agent/`
- `src/tools/agent_tool/`
- `src/tools/task_tool/`
- `src/engine/worktree.rs`
- `src/tools/worktree_tool/`
- `src/tui/slash_handler/agents.rs`

2026-05-20 code status:

- `AgentDefinition` now normalizes profile data into a Claude-like runtime
  record: type/name, when-to-use text, role, tools, disallowed tools,
  permission mode, model policy, max turns, MCP servers, memory policy,
  context mode, risk policy, and output contract.
- Context modes are explicit and backward-compatible with existing profile
  config: `minimal`, `inherited_summary` (legacy `inherit`), `full_fork`
  (legacy `fork`), and `isolated_worktree_fork`.
- The `agent` tool now threads definitions into subagent prompt contracts,
  A2A-style envelope constraints, persisted artifact profile metadata, trace
  profile labels, and tool result JSON.
- `/agents` and resource inventory now surface normalized agent definitions
  instead of only raw profile names.
- Durable subagent task state now has a session DB table and API for task id,
  agent id, status, transcript path, in-progress tool ids, permission request
  placeholders, result artifact id, cleanup hooks, and structured payload.
- The `agent` tool writes durable task state on subagent start and completion,
  and completion rows link back to persisted agent artifacts when available.
- `/agents` now shows durable task states alongside live agent handles and
  completed artifacts.
- `agent::forked_context` now implements the Claude-like fork prefix builder:
  parent assistant tool calls are preserved, each parent tool call receives the
  same placeholder tool result, the child directive is wrapped in a
  `fork-boilerplate` guard, and recursive fork attempts are rejected when that
  guard is already present.
- Tool execution now attaches the parent assistant tool-call round to
  `ToolContext`; `AgentConfig` can carry inherited context messages; and the
  `agent` tool uses those messages for `full_fork` definitions and explicit
  `fork_branches`.
- `isolated_worktree_fork` now creates a dedicated git worktree/branch before
  spawning the child agent, injects a worktree path-translation notice into
  the forked context, loads file context from the isolated checkout, and runs
  the child `ConversationLoop` with a per-agent working-directory override
  instead of switching the parent session's shared worktree state.
- Completed isolated agent worktrees can now be handled through reusable
  `worktree` tool actions and `/agents worktree ...` slash commands:
  `agent_review` summarizes status/diff/branch-ahead state, `agent_merge`
  merges committed agent branches or applies tracked uncommitted diffs back to
  the target worktree with clean-target safeguards, and `agent_cleanup` removes
  the isolated worktree with optional safe `codex/agent-*` branch deletion.
- Agent definitions now drive runtime tool/MCP resolution instead of only
  prompt metadata: profile `disallowed_tools` are removed from the child
  whitelist, configured `mcp_servers` automatically expose the MCP resource/tool
  commands, and child `ToolContext` metadata constrains MCP tools to those
  declared servers.
- `/agents` now renders richer durable task state details from runtime
  producers, including in-progress tool and permission counts, cleanup hooks,
  isolated worktree path/branch, and fork-context placeholder status.
- Latest targeted validation: `cargo test -q profiles`,
  `cargo test -q agent_tool`, `cargo test -q query_options`,
  `cargo test -q forked_context`, `cargo test -q worktree_tool`,
  `cargo test -q session_store`, `cargo test -q commands`,
  `cargo test -q mcp_tool`, `cargo test -q agents`, `cargo fmt --check`,
  `git diff --check`, and `cargo check -q`.

Remaining Phase 8 work:

- No known Phase 8 implementation items remain in this plan snapshot.

### Phase 9: TUI Product Surface Pass

Why ninth: the UI should now have real runtime data to render.

Current code status:

- Status bar now renders from a synchronous `RuntimeStatusSnapshot` selector
  projection, including active tool count, failed/backgrounded tools, terminal
  task state, MCP availability/repair counts, pending approval labels, and
  message count.
- Tool output viewer now has a slash command surface:
  `/tool-output [list|latest|<tool_id>]` and alias `/tool`, backed by visible
  tool run records with full output details.
- Runtime panels now have an initial slash surface:
  `/panel [all|diff|approval|context|tasks|mcp]` plus `/runtime`, with shared
  formatters for cached diff state, pending approvals, context budget facts,
  tracked/runtime/terminal tasks, and MCP health/repair status.
- Command maturity is now driven by explicit usable/placeholder command lists,
  with `/help maturity` reporting production/usable/placeholder buckets and
  initial runtime surfaces such as `/panel` and `/tool-output` marked usable
  instead of silently appearing production-grade.
- `/tasks` now reuses the task runtime panel, so the high-use task command
  reports tracked tasks, runtime task counts, terminal handles, backgrounded
  tools, and recent runtime tool status from one product surface.
- `/mcp status|health` now reuses the MCP runtime panel, and `/permissions`
  includes the shared approval panel whenever a tool approval is pending.
- `/context` now starts with the shared context runtime panel and then appends
  the detailed request-budget, memory-preview, and compression sections.

Tasks:

1. Status line from runtime state. (done)
2. Tool output viewer from execution records. (initial slash surface done)
3. Diff/approval/context/task/MCP panels. (initial slash/runtime panel surface
   done)
4. Command maturity cleanup. (explicit registry lists and `/help maturity`
   report done)
5. High-use command flows made production-grade. (`/tasks`, `/mcp status`,
   `/context`, and pending `/permissions` runtime panel flows done)

Expected files:

- `src/tui/app.rs`
- `src/tui/commands.rs`
- `src/tui/tool_view.rs`
- `src/tui/screens/`
- `src/tui/components/`
- `src/tui/slash_handler/`

### Phase 10: MCP, Plugins, Bridge

Why tenth: external surfaces are easier after local runtime parity.

Current code status:

- `/mcp repair` now prints a repair plan from MCP health diagnostics, separating
  explicit approval, explicit OAuth auth, and circuit-breaker repair actions.
- `/mcp repair --all` applies only safe circuit-breaker resets and leaves
  approval/OAuth steps explicit.
- The runtime `mcp` management tool now covers MCP resources and server repair:
  `list_resources`, `read_resource`, and `repair_server` are part of the tool
  schema and respect per-agent MCP server scopes.
- `/mcp resources [server]` and `/mcp read <server> <uri>` now reuse the
  runtime MCP resource tools, so CLI resource discovery follows the same
  approval, health, and scoped-server rules as agent tool use.
- Plugin reload now has a registration lifecycle report shared by startup
  injection, `plugin_manage reload`, and `/reload plugins`: discovered,
  enabled, injected tool names, skipped disabled/missing-entry/unsigned/name
  collision counts, and trust mode are reported from one path.
- Bridge/remote runtime status now has a first TUI surface:
  `/panel bridge` and `/remote status` show bridge URL source, auth-token
  presence, tenant id, replay cursor file/count/session ids, remote environment
  detection, saved SSH sessions, and whether `remote_trigger`/`remote_dev` are
  exposed in the active tool registry. The same facts are now represented in
  `RuntimeAppState`/`RuntimeStatusSnapshot` selectors for future status/trace
  consumers.
- Bridge configuration resolution is now shared by the remote trigger tool and
  TUI status: `PRIORITY_AGENT_BRIDGE_URL`/`BRIDGE_URL`,
  `PRIORITY_AGENT_BRIDGE_TOKEN`/`BRIDGE_TOKEN`, and
  `PRIORITY_AGENT_BRIDGE_TENANT_ID`/`BRIDGE_TENANT_ID` are handled through the
  bridge module instead of one-off slash/tool parsing.
- Bridge/remote permission callbacks now carry structured risk facts for
  `remote_trigger` and `remote_dev`: remote execution, remote session creation,
  cursor-persisting sync, and SSH exec are classified for permission prompts,
  permission request metadata, recovery guidance, and `/trace` via
  `remote.bridge` events. Remote execution failures now prefer `/remote status`
  and are not marked as safe automatic retries unless the user/agent has checked
  remote side effects.

Tasks:

1. MCP prompt/resource/tool/command parity. (initial MCP manage-tool and slash
   resource/read parity done)
2. MCP auth repair and approval flows. (initial repair-plan and safe
   circuit-reset flow done)
3. Plugin lifecycle and reload. (initial reload diagnostics and registration
   report done)
4. Bridge/remote state and permission callbacks. (runtime state panel, shared
   config resolution, remote permission facts, recovery hints, and trace events
   done)

Expected files:

- `src/engine/mcp.rs`
- `src/tools/mcp_tool/`
- `src/plugins/`
- `src/bridge/`
- `src/remote/`

### Phase 11: Provider Protocol Hardening

Why eleventh: provider correctness must hold after the runtime message model
changes.

Current code status:

- `ProviderCapabilities` now records protocol family, streaming/tool-call
  support, reasoning-token support, MiniMax non-streaming tool-call
  requirements, system-message merging, and tool-result adjacency requirements.
- Provider config/type resolution can derive capability records from explicit
  provider type, base URL, and model name.
- Provider message normalization can run through capability records, preserving
  the existing provider-family normalizer while making the capability table the
  routing surface.
- Conversation-loop streaming fallback now uses provider capabilities for
  MiniMax-style non-streaming tool-call routing instead of ad hoc string checks
  in the loop.
- Terminal API failures now emit a classified recovery plan. Provider protocol
  and request-schema failures are marked non-safe-retry and point to
  `/trace last` instead of collapsing into a generic `api_error`.
- Provider request recovery now treats fallback model use as runtime policy:
  `ResourcePolicy` decides whether fallback is allowed, `/resource` and
  `resource.policy` traces expose that decision, transient provider failures can
  retry once with `PRIORITY_AGENT_FALLBACK_MODEL`, and context-size errors prefer
  reactive compaction before fallback instead of repeating the same oversized
  request when no compressor is available.
- API start traces now include provider protocol facts: detected provider
  family, whether non-streaming tool requests are required, and whether strict
  tool-result adjacency is required. This makes provider incompatibility visible
  in `/trace last` before looking at serialized payloads.

Tasks:

1. Normalize provider message generation. (initial capability-driven
   normalization entrypoint done)
2. Add provider capability records. (initial provider/type/config capability
   records done)
3. Add typed provider error recovery. (initial terminal API failure recovery
   plans done)
4. Add context-too-long reactive compaction and fallback model policy. (initial
   reactive compaction plus policy-gated fallback model retry done)

Expected files:

- `src/services/api/`
- `src/engine/conversation_loop/api_request_controller.rs`
- `src/engine/conversation_loop/session_processor.rs`
- `src/engine/error_classifier.rs`

### Phase 12: Verification And External Baseline

Why last in this tactical push: gex asked to pause testing and optimization
first. We still keep narrow compile/test checks during implementation, but the
large replay matrix comes after core parity code exists.

Tasks:

1. Update deterministic scenario matrix around the new runtime records.
2. Add real-task replay cases for:
   - file edit with rewind
   - bash background task
   - permission denial and retry
   - compaction boundary
   - subagent worktree worker
   - MCP auth repair
3. Compare the same task set against Claude Code and Codex when available.
4. Update `docs/PROJECT_STATUS.md` only after the evidence is current.

Progress, 2026-05-21:

- Added the first Phase 12 deterministic scenario matrix skeleton in
  `src/engine/scenario_matrix.rs`.
- The matrix declares the six required product scenarios and maps each one to
  concrete local evidence surfaces: trace events, runtime panels, recovery
  plans, tool metadata, slash commands, or session-store records.
- Added `/eval matrix` as a lightweight readout so the runtime can show the
  current scenario coverage without starting a full replay or live benchmark.
- External Claude/Codex baselines remain deferred until these six mapped cases
  become deterministic replay fixtures.

## First Ten Code Batches

These are the concrete next implementation batches after this planning pass:

1. Add `ToolExecutionRecord` and default Tool V2 methods.
2. Convert `ToolExecutionController` to emit records while preserving current
   `ToolResult`.
3. Upgrade file tools to fill file identity, read coverage, checkpoint, diff,
   and stale-state facts through records.
4. Upgrade bash tools to fill terminal task, command facts, path facts,
   validation facts, and permission risk facts through records.
5. Add `RuntimeAppState` skeleton and wire `/status` plus tool-view active tool
   ids to it.
6. Add `TurnRuntimeContext` and migrate controller argument groups into it.
7. Implement durable permission review objects with once/always/reject.
8. Upgrade hook runtime events and preserve env hooks as one backend.
9. Implement compaction boundary records and split compaction strategy state.
10. Implement `AgentDefinition` plus context modes for minimal/inherited/fork.

## Development Rules For This Plan

1. Read the Claude reference for the specific subsystem immediately before
   implementing that subsystem.
2. Translate semantics, not source text.
3. Keep each batch scoped to one ownership boundary.
4. Preserve user and prior-agent changes in the dirty tree.
5. Prefer runtime state/contracts over prompt instructions.
6. Avoid broad eval/test work until the relevant code surface exists.
7. Still run the narrowest compile or targeted check needed to avoid leaving the
   repo obviously broken.
8. Update docs only when implementation changes the actual current state.
9. Do not claim parity until behavior is proven on real tasks.

## Main Risks

1. **Big-bang rewrite risk.**
   The current code is large but working. Avoid replacing `ConversationLoop` in
   one move. Migrate ownership one controller at a time.

2. **Fake parity risk.**
   Adding method names like Claude is not enough. Each method must feed runtime
   state, UI, permission, evidence, or provider payload.

3. **Prompt bloat risk.**
   The answer is not to copy Claude-like guidance into prompts. Runtime should
   own state and hard constraints.

4. **UI-before-data risk.**
   Do not build big panels before `ToolExecutionRecord`, `RuntimeAppState`, and
   file history are stable.

5. **Subagent chaos risk.**
   Do not let subagents mutate the same working tree by default. Use scoped
   tools and worktrees for mutating parallel workers.

6. **Stale roadmap risk.**
   Re-check the local Claude source before each major stream. The folder is the
   reference for this plan, not older docs.

## Current Recommendation

Start with Stream B, not with UI or agents.

The fastest path to real Claude Code-like behavior is to make tools rich product
objects and make tool execution produce durable structured records. Once that is
done, file history, shell tasks, permissions, hooks, TUI, closeout, evals, and
subagents can all use the same data. Without that foundation, every later
feature will keep parsing raw strings and adding special cases.
