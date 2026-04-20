# Feature Comparison: Claude Code vs Priority-Agent

**Generated:** 2026-04-11
**Sources:**
- Claude Code: ~/Desktop/claude/ (TypeScript, ~5000+ line REPL.tsx, 30+ tools)
- Priority-Agent: ~/Desktop/rust-agent/ (Rust, ratatui TUI, 8 tools)

---

## Summary

| Category | Claude Code Features | Priority-Agent Equivalent | Coverage |
|----------|---------------------|--------------------------|----------|
| Tools (Core) | 15+ built-in | 8 implemented | ~53% |
| Tools (Advanced) | 15+ feature-gated | 0 | 0% |
| Core Engine | Full agentic loop | Full agentic loop | ~80% |
| TUI / UI | Rich React/Ink TUI | Basic ratatui TUI | ~30% |
| Permissions | Multi-mode + deny rules | 4-mode enum | ~40% |
| Cost Tracking | Per-model USD + caching | Basic token/cost | ~50% |
| MCP | Full client/server | Stub only | ~5% |
| Session/Storage | Project config + resume | SQLite + FTS5 | ~70% |
| Memory | MEMORY.md + auto-mem | None | 0% |
| Slash Commands | 40+ commands | 12 CLI commands | ~25% |
| Skills | Bundled + custom | Basic registry | ~20% |
| Sub-Agents | Full swarm/teams | Basic sub-agent | ~40% |
| Context Mgmt | Token budget + compression | Token budget + compression | ~75% |
| Streaming | Full SSE streaming | Full SSE streaming | ~85% |
| Config/Auth | Multi-provider + OAuth | Kimi-only + env vars | ~20% |

**Overall estimated feature parity: ~35-40%**

---

## 1. TOOLS

### 1.1 Core Tools (Always Available)

| Tool | Claude Code | Priority-Agent | Status | Gap |
|------|-------------|----------------|--------|-----|
| Bash/Shell Execute | BashTool - sandbox, security, destructive warnings, sed validation, read-only detection, PowerShell variant | BashTool - basic command execution, timeout, danger detection | ⚠️ Partial | Missing: sandbox mode, sed validation, read-only detection, PowerShell, destructive command warnings UI |
| File Read | FileReadTool - line offsets, image processing, size limits | FileReadTool - line offset/limit | ⚠️ Partial | Missing: image processing, size limits, binary detection |
| File Write | FileWriteTool (standalone) | FileWriteTool (in file_tool) | ✅ Full | Equivalent |
| File Edit | FileEditTool - diff-based, precise string replacement | FileEditTool (in file_tool) | ⚠️ Partial | Need to verify diff precision; Claude Code has separate Edit tool |
| Glob | GlobTool - file pattern matching | GlobTool | ✅ Full | Equivalent |
| Grep | GrepTool - content search with regex | GrepTool | ✅ Full | Equivalent |
| Web Fetch | WebFetchTool - URL content fetching | None | ❌ Missing | No web fetch capability |
| Web Search | WebSearchTool - search engine integration | None | ❌ Missing | No web search; FeatureFlags has web_search but no implementation |
| Notebook Edit | NotebookEditTool - Jupyter notebook cell editing | None | ❌ Missing | No notebook support |
| Agent (Sub-Agent) | AgentTool - spawn sub-agents with full tool access | AgentTool - spawn sub-agents with QueryEngine | ⚠️ Partial | Missing: async agents, team/swarm support, agent output streaming |
| Task Create | TaskCreateTool - structured task creation | TaskCreateTool | ⚠️ Partial | Rust version is simpler; Claude Code has full TaskCreate/Get/Update/List suite |
| Task List/Get/Update | TaskListTool, TaskGetTool, TaskUpdateTool | None (only TaskCreateTool) | ❌ Missing | Missing 3 of 4 task management tools |
| Task Stop | TaskStopTool - cancel running tasks | None | ❌ Missing | No task cancellation |
| Todo Write | TodoWriteTool - structured todo list for planning | None | ❌ Missing | No todo/planning tool |
| Config | ConfigTool (ant-only) | None (AppConfig struct) | ❌ Missing | No runtime config tool; config exists as struct only |
| Brief | BriefTool - file attachments + uploads | None | ❌ Missing | No file attachment/upload |

### 1.2 Planning & Mode Tools

| Tool | Claude Code | Priority-Agent | Status | Gap |
|------|-------------|----------------|--------|-----|
| Enter Plan Mode | EnterPlanModeTool - switch to planning-only mode | None | ❌ Missing | No plan mode concept |
| Exit Plan Mode | ExitPlanModeV2Tool - exit plan with approval flow | None | ❌ Missing | No plan approval flow |
| Ask User Question | AskUserQuestionTool - interactive clarification | None | ❌ Missing | No structured user Q&A tool |

### 1.3 Advanced / Feature-Gated Tools (Claude Code Only)

| Tool | Feature Flag | Priority-Agent Status |
|------|-------------|----------------------|
| REPLTool | ANT-only | ❌ Missing |
| SleepTool | PROACTIVE/KAIROS | ❌ Missing |
| CronCreateTool/CronDeleteTool/CronListTool | AGENT_TRIGGERS | ❌ Missing |
| RemoteTriggerTool | AGENT_TRIGGERS_REMOTE | ❌ Missing |
| MonitorTool | MONITOR_TOOL | ❌ Missing |
| WebBrowserTool | WEB_BROWSER_TOOL | ❌ Missing |
| PowerShellTool | PowerShell enabled | ❌ Missing |
| SendMessageTool | always | ❌ Missing |
| TeamCreateTool/TeamDeleteTool | Agent swarms | ❌ Missing |
| WorkflowTool | WORKFLOW_SCRIPTS | ❌ Missing |
| PushNotificationTool | KAIROS | ❌ Missing |
| SubscribePRTool | KAIROS_GITHUB_WEBHOOKS | ❌ Missing |
| SendUserFileTool | KAIROS | ❌ Missing |
| SnipTool | HISTORY_SNIP | ❌ Missing |
| ListPeersTool | UDS_INBOX | ❌ Missing |
| ToolSearchTool | tool search | ❌ Missing |
| LSPTool | ENABLE_LSP_TOOL | ❌ Missing |
| SuggestBackgroundPRTool | ANT-only | ❌ Missing |
| TungstenTool | ANT-only | ❌ Missing |
| ListMcpResourcesTool | always | ❌ Missing |
| ReadMcpResourceTool | always | ❌ Missing |

---

## 2. CORE ENGINE

| Feature | Claude Code | Priority-Agent | Status | Gap |
|---------|-------------|----------------|--------|-----|
| Query Engine (tool calling loop) | Full agentic loop with max iterations, tool dispatch | Full agentic loop with QueryEngine + QueryOptions | ✅ Full | Equivalent architecture |
| Streaming Responses | Full SSE streaming with delta accumulation | Full SSE streaming with StreamEvent enum | ✅ Full | Equivalent; both support TextChunk, ToolCall events |
| Message History | In-memory + context compression | In-memory Vec<Message> in StreamingQueryEngine | ⚠️ Partial | Missing: session-switch history management |
| Context Compression | Token budget, head/tail split, LLM-based summarization | Token budget, head/tail split, heuristic summarization | ⚠️ Partial | Uses heuristic summarization instead of LLM-based; missing iterative LLM summaries |
| Multi-Turn Conversation | Full conversation history with session switching | conversation_history in StreamingQueryEngine | ⚠️ Partial | No session switching in TUI; history is single-session |
| Error Classification | ErrorClassifier module | error_classifier.rs | ⚠️ Partial | Exists but unverified for completeness |
| Turn State Management | turn_state.ts with full state tracking | turn_state.rs | ⚠️ Partial | Basic implementation |

---

## 3. TUI / USER INTERFACE

| Feature | Claude Code | Priority-Agent | Status | Gap |
|---------|-------------|----------------|--------|-----|
| Chat REPL | React/Ink-based with virtual scrolling, message rendering, syntax highlighting | ratatui-based with message rendering, scroll offset | ⚠️ Partial | Missing: virtual scrolling, syntax highlighting, rich formatting |
| Input System | Multi-line input with cursor, paste handling, vim mode, autocomplete | Multi-line input with cursor, character insert/delete | ⚠️ Partial | Missing: paste, vim mode, autocomplete, history navigation (has basic history in TuiApp) |
| Spinner/Status | SpinnerWithVerb, BriefIdleStatus, animated status | Simple "Thinking..." in title bar | ❌ Missing | No spinner animations |
| Tool Execution Display | Rich UI with approval dialogs, diff previews, progress | Simple "[Executing: tool]" text | ❌ Missing | No approval UI, no diff preview |
| Permission Dialog | PermissionRequest component with Allow/Deny/Always options | None in TUI | ❌ Missing | No interactive permission prompts in TUI |
| Cost Display | formatTotalCost() with per-model breakdown, USD cost | CostTracker.generate_report() | ⚠️ Partial | No TUI integration; cost exists but not displayed in TUI |
| Keybindings | GlobalKeybindingHandlers, CommandKeybindingHandlers, shortcut display | Basic Ctrl+C exit | ❌ Missing | No keyboard shortcuts system |
| Slash Commands | 40+ /commands (see Section 7) | None in TUI | ❌ Missing | No slash command system in TUI |
| Search | useSearchInput hook, search highlighting | None | ❌ Missing | No search in TUI |
| Message Selection | MessageSelector component for browsing history | None | ❌ Missing | No message selection |
| Tab Completion | Tab status display, IDE integration | None | ❌ Missing | No tab completion |
| Theme Support | Theme system with color customization | Dark theme hardcoded | ❌ Missing | No theme system |
| Terminal Title | useTerminalTitle hook | None | ❌ Missing | No terminal title management |
| Doctor Screen | Doctor.tsx for diagnostics | None | ❌ Missing | No diagnostics screen |
| Resume Conversation | ResumeConversation.tsx | SessionStore.list_sessions() | ⚠️ Partial | Backend exists but no TUI to resume |

---

## 4. PERMISSIONS SYSTEM

| Feature | Claude Code | Priority-Agent | Status | Gap |
|---------|-------------|----------------|--------|-----|
| Permission Modes | Default, AutoLowRisk, AutoAll, ReadOnly (+ Plan Mode) | Default, AutoLowRisk, AutoAll, ReadOnly | ⚠️ Partial | Missing: Plan Mode as permission mode |
| Deny Rules | Fine-grained deny rules per tool with ruleContent matching | PermissionRules with always_allow/deny/ask sets | ⚠️ Partial | Missing: rule content matching, MCP server-prefix rules |
| Permission Context | ToolPermissionContext with working directory awareness | PermissionContext with working directory | ✅ Full | Equivalent concept |
| User Confirmation | Interactive PermissionRequest in REPL | Tool.requires_confirmation() trait method | ⚠️ Partial | No TUI confirmation flow |
| Sandbox Mode | shouldUseSandbox() for bash isolation | None | ❌ Missing | No sandbox execution |
| Session-Scoped Rules | Plan mode pushes temporary permission rules | None | ❌ Missing | No session-scoped permissions |

---

## 5. COST TRACKING

| Feature | Claude Code | Priority-Agent | Status | Gap |
|---------|-------------|----------------|--------|-----|
| Token Counting | Input, output, cache read, cache write tokens per model | Prompt + completion tokens | ⚠️ Partial | Missing: cache token tracking |
| USD Cost Calculation | calculateUSDCost() with per-model pricing tables | calculate_cost() with Kimi pricing | ⚠️ Partial | Only Kimi models; no cache cost calculation |
| Per-Model Usage | getModelUsage(), getUsageForModel() | model_usage HashMap | ✅ Full | Equivalent |
| Session Persistence | saveCurrentSessionCosts() to project config | In-memory only | ❌ Missing | Costs not persisted across sessions |
| Cost Display | formatTotalCost() with formatted output in REPL | generate_report() text output | ⚠️ Partial | Not integrated into TUI |
| Lines Changed Tracking | getTotalLinesAdded/Removed | None | ❌ Missing | No code change tracking |
| Duration Tracking | Total API duration, wall duration, tool duration | Session duration only | ⚠️ Partial | Missing: API duration, tool duration breakdown |
| Web Search Count | getTotalWebSearchRequests | None | ❌ Missing | No web search tracking |

---

## 6. MCP (Model Context Protocol)

| Feature | Claude Code | Priority-Agent | Status | Gap |
|---------|-------------|----------------|--------|-----|
| MCP Server Config | Full MCP server configuration, discovery | McpServerConfig struct | ⚠️ Partial | Config struct exists but no protocol implementation |
| MCP Tool Integration | assembleToolPool() merges MCP tools with built-in | None | ❌ Missing | No MCP tool integration |
| MCP Resource Tools | ListMcpResourcesTool, ReadMcpResourceTool | None | ❌ Missing | No MCP resource access |
| MCP Client | Full protocol client with approval flow | McpClient stub with TODO | ❌ Missing | Protocol handshake not implemented |
| MCP Server Approval | mcpServerApproval.tsx - interactive approval | None | ❌ Missing | No approval flow |

---

## 7. SLASH COMMANDS / CLI

### Claude Code Slash Commands (40+)

| Command | Priority-Agent Equivalent | Status |
|---------|--------------------------|--------|
| /add-dir | None | ❌ |
| /autofix-pr | None | ❌ |
| /bughunter | None | ❌ |
| /clear | None | ❌ |
| /color | None | ❌ |
| /commit | skills/commit (basic) | ⚠️ |
| /compact | context_compressor (auto) | ⚠️ |
| /config | None (AppConfig struct) | ❌ |
| /context | None | ❌ |
| /cost | CostTracker.generate_report() | ⚠️ |
| /diff | None | ❌ |
| /doctor | None | ❌ |
| /help | CLI print_help() | ⚠️ |
| /init | CLI init command | ⚠️ |
| /keybindings | None | ❌ |
| /login | None (env var only) | ❌ |
| /logout | None | ❌ |
| /mcp | None | ❌ |
| /memory | None | ❌ |
| /mobile | None | ❌ |
| /onboarding | None | ❌ |
| /pr_comments | None | ❌ |
| /release-notes | None | ❌ |
| /rename | None | ❌ |
| /resume | SessionStore.list_sessions() | ⚠️ |
| /review | None | ❌ |
| /session | SessionStore (backend) | ⚠️ |
| /share | None | ❌ |
| /skills | SkillRegistry | ⚠️ |
| /status | None | ❌ |
| /tasks | CLI list/next/done | ⚠️ |
| /teleport | None | ❌ |
| /theme | None | ❌ |
| /usage | CostTracker | ⚠️ |
| /vim | None | ❌ |
| /voice | None | ❌ |
| /workflows | None | ❌ |
| /feedback | None | ❌ |
| /security-review | None | ❌ |
| /ctx_viz | None | ❌ |

### Priority-Agent CLI Commands (12)

| Command | Description | Has equivalent in Claude Code? |
|---------|-------------|-------------------------------|
| init | Initialize project | /init ✅ |
| add <name> | Add task | /tasks (TaskCreateTool) ✅ |
| list | List tasks | /tasks list ✅ |
| next | Show next recommended | None (unique to priority-agent) |
| done <id> | Complete task | None (unique to priority-agent) |
| progress | Show progress | /status ⚠️ |
| analyze | Analyze project | None |
| ai-analyze | AI weight analysis | None (unique) |
| ai-suggest | AI weight suggestion | None (unique) |
| snapshot | Create snapshot | None |
| restore | Restore snapshot | None |
| interactive | Interactive mode | Default REPL mode ✅ |

---

## 8. MEMORY SYSTEM

| Feature | Claude Code | Priority-Agent | Status | Gap |
|---------|-------------|----------------|--------|-----|
| MEMORY.md | Entrypoint file loaded per-session, 200-line/25KB cap | None | ❌ Missing | No memory file system |
| Auto Memory | Auto-generated memories from conversations | None | ❌ Missing | No auto-memory |
| Memory Search | findRelevantMemories() | None | ❌ Missing | No memory search |
| CLAUDE.md | Project-level instructions file | CLAUDE.md exists (for Claude Code working IN the project) | ❌ Missing | Not a feature; just a project doc |
| Memory Age | memoryAge.ts for freshness tracking | None | ❌ Missing | No memory lifecycle |
| Team Memory | TeamMemPaths for shared team memories | None | ❌ Missing | No team memory |

---

## 9. SESSION MANAGEMENT

| Feature | Claude Code | Priority-Agent | Status | Gap |
|---------|-------------|----------------|--------|-----|
| Session Persistence | Project config with session ID, last costs | SQLite SessionStore with full CRUD | ✅ Full | Priority-agent has BETTER storage (SQLite vs JSON) |
| Session Resume | ResumeConversation.tsx, switchSession() | SessionStore.get_session() + get_messages() | ⚠️ Partial | Backend ready but no TUI flow for resume |
| Session Chaining | parent_session_id for context compression lineage | SessionRecord.parent_session_id field | ✅ Full | Equivalent |
| FTS Search | None (grep-based) | messages_fts FTS5 virtual table | ✅ Full | Priority-agent has BETTER search |
| Session List | /session command | SessionStore.list_sessions() | ⚠️ Partial | No TUI listing |

---

## 10. SUB-AGENT SYSTEM

| Feature | Claude Code | Priority-Agent | Status | Gap |
|---------|-------------|----------------|--------|-----|
| Sub-Agent Spawning | AgentTool with full tool access, async execution | AgentTool + AgentManager + Agent struct | ⚠️ Partial | Missing: async output streaming, tool filtering per agent |
| Agent Messaging | SendMessageTool for inter-agent communication | AgentMessage with mpsc channels | ⚠️ Partial | Rust has channel-based messaging; Claude Code has tool-based |
| Agent Status | Real-time status in UI | AgentStatus enum (Pending/Running/Completed/Failed) | ⚠️ Partial | No TUI display of agent status |
| Team Swarms | TeamCreateTool + TeamDeleteTool for agent teams | None | ❌ Missing | No team concept |
| Coordinator Mode | Coordinator mode with filtered tools | None | ❌ Missing | No coordinator pattern |
| Agent Output | TaskOutputTool for capturing sub-agent results | Result sent via AgentMessage::Result | ⚠️ Partial | No structured output capture |

---

## 11. CONTEXT MANAGEMENT

| Feature | Claude Code | Priority-Agent | Status | Gap |
|---------|-------------|----------------|--------|-----|
| Token Budget | getCurrentTurnTokenBudget(), dynamic budget | TokenBudget with max_context_tokens | ⚠️ Partial | Static budget; Claude Code has dynamic per-turn budgets |
| Context Compression | Two-stage: trim tool outputs + LLM summary | Two-stage: head/tail split + heuristic summary | ⚠️ Partial | Uses heuristic instead of LLM summarization |
| Structured Summary | Goal/Progress/Decisions/Files/NextSteps | StructuredSummary with same fields | ✅ Full | Equivalent structure |
| Iterative Summary | Accumulated summaries across compressions | accumulated_summary with merge() | ✅ Full | Equivalent |
| Tool Call Pair Integrity | Full sanitization | sanitize_tool_pairs() | ✅ Full | Equivalent |
| Budget Continuation | getBudgetContinuationCount() for multi-turn budgets | None | ❌ Missing | No budget continuation |

---

## 12. CONFIGURATION & AUTH

| Feature | Claude Code | Priority-Agent | Status | Gap |
|---------|-------------|----------------|--------|-----|
| Config File | Project-level .claude config, global settings | config.toml + env vars (PRIORITY_AGENT_ prefix) | ⚠️ Partial | Simpler config system |
| Auth | OAuth login/logout, API key management | MOONSHOT_API_KEY env var only | ❌ Missing | No auth flow, single provider |
| Model Switching | Multiple model support with pricing | Kimi model selection only | ❌ Missing | No multi-provider |
| Feature Flags | GrowthBook integration, feature() function | FeatureFlags struct with boolean fields | ⚠️ Partial | Static flags vs dynamic remote flags |
| Analytics | DataDog, first-party event logging, GrowthBook | None | ❌ Missing | No analytics |

---

## 13. SKILLS SYSTEM

| Feature | Claude Code | Priority-Agent | Status | Gap |
|---------|-------------|----------------|--------|-----|
| Skill Loading | loadSkillsDir() for bundled + custom skills | SkillRegistry with register/get/list | ⚠️ Partial | No directory loading; manual registration only |
| Bundled Skills | batch.ts and others bundled in app | CommitSkill only | ⚠️ Partial | Only 1 built-in skill |
| Skill Search | Experimental skill search | None | ❌ Missing | No skill discovery |
| Skill Improvement | SkillImprovementSurvey component | None | ❌ Missing | No feedback loop |

---

## 14. UNIQUE TO PRIORITY-AGENT (Not in Claude Code)

These features give priority-agent differentiation from Claude Code:

| Feature | Description |
|---------|-------------|
| Weight Engine | Hierarchical weight calculation for task prioritization |
| AI Analyzer | Heuristic + AI-based weight analysis |
| Task Analyzer | Task parsing and dependency graph construction |
| Priority Scheduler | Smart task allocation based on weights |
| GitHub Integration | gh CLI integration for issues/PRs/CI status as weight inputs |
| SQLite Session Store | Full SQLite persistence with FTS5 (Claude Code uses JSON config) |
| Context Compressor | Well-structured compression with StructuredSummary (implemented in Rust) |

---

## PRIORITY BUILD ORDER (What to Build Next)

Based on gap analysis, here's the recommended build order for maximum impact:

### Phase 1: Essential Missing Tools (High Impact)
1. **WebFetchTool** - URL content fetching
2. **WebSearchTool** - Search engine integration  
3. **NotebookEditTool** - Jupyter support
4. **TaskListTool/TaskGetTool/TaskUpdateTool** - Complete task management suite
5. **AskUserQuestionTool** - Interactive clarification

### Phase 2: TUI Polish (User Experience)
6. **Slash command system** in TUI (/help, /cost, /clear, /resume, /config)
7. **Permission dialog** in TUI (interactive allow/deny prompts)
8. **Spinner/status animations** (replace "Thinking...")
9. **Syntax highlighting** in message rendering
10. **Search** within conversation history

### Phase 3: Advanced Engine Features
11. **Plan Mode** (EnterPlanModeTool + ExitPlanModeTool)
12. **LLM-based context compression** (replace heuristic summarizer)
13. **Dynamic token budgets** (per-turn budget calculation)
14. **Session resume** TUI flow

### Phase 4: Ecosystem
15. **MCP client** (actual protocol implementation)
16. **Memory system** (MEMORY.md loading)
17. **Multi-provider auth** (not just Kimi)
18. **Analytics/telemetry**

---

## FILE REFERENCES

### Claude Code Key Files Analyzed:
- src/tools.ts - Tool registry and all tool imports
- src/cost-tracker.ts - Cost tracking system
- src/screens/REPL.tsx - Main TUI (5000+ lines)
- src/commands.ts - 40+ slash commands
- src/memdir/memdir.ts - Memory system
- src/tools/BashTool/*.ts - Bash tool with 15+ supporting files

### Priority-Agent Key Files Analyzed:
- src/tools/mod.rs - Tool trait and registry
- src/cost_tracker/mod.rs - Cost tracking
- src/permissions/mod.rs - Permission modes
- src/mcp/mod.rs - MCP stub
- src/engine/query_engine.rs - Query engine
- src/engine/streaming.rs - Streaming engine
- src/engine/context_compressor.rs - Context compression
- src/session_store/mod.rs - SQLite session store
- src/tui/app.rs - TUI application state
- src/agent/agent.rs - Agent system
- src/services/config.rs - Configuration
- src/skills/mod.rs - Skills registry
- src/github/mod.rs - GitHub integration
- src/cli/commands.rs - CLI commands
- src/main.rs - Entry point
