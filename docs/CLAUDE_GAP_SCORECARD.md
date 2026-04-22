# Claude Code Gap Scorecard

> **Last updated**: 2026-04-22
> **Update frequency**: Weekly (every Monday)
> **Goal**: Track gap trends towards Claude Code parity

## Summary

| Dimension | Claude Code | Our Implementation | Gap | Trend |
|-----------|-------------|-------------------|-----|-------|
| **Tools** | 64 | ~64 | 0 | Complete |
| **Commands** | 101 | 101+ | 0 | Complete (entry-level) |
| **Agents** | 7 types | 7 types | 0 | Complete |
| **Transport** | 3 (stdio/http/ws) | 3 (stdio/http/ws) | 0 | Complete |
| **Auth** | OAuth/SSO | OAuth (MCP) | Partial | Improving |
| **Frontend** | CLI/TUI/Web/Desktop | CLI/TUI | -2 | Stable |
| **Hooks** | Full event system | Pre/Post/Tool/Provider/Config | Near complete | Improving |

Legend: Complete | Partial | Missing | — N/A

**Quick snapshot**: Run `/doctor gap` in TUI to see live metrics.

---

## 1. Tools Coverage

**Claude Code**: 64 tools
**Our implementation**: ~64 tool types (including Task variants)

| Tool Category | Claude Code | Our Implementation | Status |
|---------------|------------|-------------------|--------|
| **File Operations** | | | |
| FileRead | | file_read | Complete |
| FileWrite | | file_write | Complete |
| FileEdit | | file_edit | Complete |
| Glob | | glob | Complete |
| Grep | | grep | Complete |
| **Bash** | | | |
| Bash | | bash (local/restricted/external) | Complete |
| PowerShell | | powershell | Complete |
| REPL | | repl | Complete |
| **Web** | | | |
| WebFetch | | web_fetch | Complete |
| WebSearch | | web_search | Complete |
| Browser | | browser | Complete |
| **Agent** | | | |
| Agent | | agent | Complete |
| TaskCreate/List/Get | | task_create / task_list / task_get | Complete |
| **Context** | | | |
| MemorySave/Load | | memory_save / memory_load | Complete |
| Compact | | context_compress | Complete |
| ContextCollapse | | context_collapse | Complete |
| **MCP** | | | |
| MCPTool | | mcp | Complete |
| McpAuth | | mcp_auth | Complete |
| ListMcpResources | | list_mcp_resources | Complete |
| **LSP** | | | |
| LSP | | lsp | Complete |
| **Worktree** | | | |
| Worktree | | worktree | Complete |
| **Skills** | | | |
| SkillLoad/List | | skill_load / skill_list | Complete |
| **Plan Mode** | | plan | Complete |
| **Socratic** | | socratic | Complete |
| **Swarm** | | swarm | Complete |
| **Diff/Patch** | | diff | Complete |
| **Notebook** | | notebook | Complete |
| **Team** | | team | Complete |
| **Voice** | | voice | Partial (skeleton) |
| **Remote** | | remote_trigger / remote_dev | Complete |
| **Git** | | git | Complete |
| **GitHub** | | github | Complete |
| **Calculate/Encode/JSON/Datetime/Todo** | | calculate / encode / json_query / datetime / todo | Complete |
| **Telemetry** | | telemetry | Complete |
| **Plugin** | | plugin_list / plugin_manage | Partial (MVP) |
| **Ask/Symbol/ProjectList/Cron/Share/Sleep** | | ask_user / symbol / project_list / cron / share / sleep | Complete |
| **Refactor/Workbench** | | refactor / workbench | Partial |
| **ToolSearch/SendMessage** | | tool_search / send_message | Complete |
| **Exit/Enter PlanMode/Worktree** | | exit_plan_mode / enter_plan_mode / exit_worktree / enter_worktree | Complete |
| **Brief/Cost/Clear/Config/Copy/Desktop** | | brief / cost / clear / config / copy / desktop | Complete |
| **Resume/Rewind** | | resume / rewind | Partial |
| **MCP Server (standalone)** | | | Missing |

**Gap**: 1 missing (MCP Server standalone), 3 partial (Voice, Plugin, Refactor/Workbench, Resume/Rewind)
**Trend**: Improving

---

## 2. Commands Coverage

**Claude Code**: 101 commands
**Our implementation**: 101+ command entries (ALL_COMMANDS = 114)

Commands are categorized by maturity:
- **Production-ready**: Full implementation, stable
- **Usable**: Core flow works, edge cases limited
- **Scaffold**: Entry and help exist, implementation shallow

| Batch | Commands | Maturity |
|-------|----------|----------|
| Core | /help, /clear, /quit, /memory, /save, /cost, /model, /status, /tools | Production-ready |
| Task/Agent | /tasks, /agents, /doctor, /audit, /permissions | Production-ready |
| Git Workflow | /commit, /review-pr, /review, /security-review, /explain, /fix | Production-ready |
| Session Control | /session, /undo, /redo, /retry, /stop | Usable |
| Context | /history, /context, /mode, /compact, /diff | Usable |
| Dev Tools | /git, /lsp, /npm, /package, /hooks | Usable |
| Advanced Agents | /teammate, /critic, /assistant, /remote | Usable |
| Utility | /token, /share, /reload, /config, /copy | Scaffold/Usable |
| Extended | /benchmark, /test, /trace, /theme, /shortcuts, /profile, /wizard, /workspace, /focus, /pause, /install, /skeleton, /branch, /color | Scaffold |
| Integration | /slack, /webhook, /desktop, /chrome, /stealth, /shadow | Scaffold |
| Meta | /btw, /effort, /preamble, /untrap, /verbose, /write, /reject, /subscribe, /slots, /ticker | Scaffold |

**Gap**: Entry-level parity achieved. Depth gap remains for scaffold commands.
**Trend**: Improving

---

## 3. Agent Types

**Claude Code**: 7 agent types
**Our implementation**: 7+ types (complete!)

| Agent Type | Status |
|------------|--------|
| Task Agent | Complete |
| Teammate | Complete |
| Assistant | Complete |
| Critic | Complete |
| Verifier | Complete |
| Remote Specialist | Complete |
| Dream Task | Complete |
| Custom Agent | Complete |
| Orchestrator | Complete |

**Gap**: 0
**Trend**: Complete

---

## 4. Transport & Protocol

| Feature | Status |
|---------|--------|
| stdio transport | Complete |
| HTTP transport | Complete |
| WebSocket transport | Complete |
| MCP OAuth | Complete |
| MCP streaming | Partial |
| Bridge auth (multi-token) | Complete |
| Bridge tenant isolation | Complete |
| Bridge status/replay/sync | Complete |

**Gap**: MCP streaming polish
**Trend**: Improving

---

## 5. Frontends

| Frontend | Status |
|----------|--------|
| CLI | Complete |
| TUI | Complete |
| Web | Missing |
| Desktop (Electron) | Missing |

**Gap**: 2 missing frontends
**Trend**: Stable (Phase 8 workspace split in progress)

---

## 6. Hooks System

| Hook Event | Status |
|------------|--------|
| pre-tool | Complete |
| post-tool | Complete |
| pre-api | Complete |
| post-api | Complete |
| tool.execute.before | Complete |
| tool.execute.after | Complete |
| Provider hooks | Complete |
| Config hooks | Complete |

**Gap**: pre-agent / post-agent hooks missing
**Trend**: Improving

---

## 7. Performance & Observability

| Metric | Status |
|--------|--------|
| Cold start (no key) | ~23ms (Better than Claude) |
| Tool latency P95 | Tracked in /doctor |
| Tool success rate | Tracked in /doctor |
| Coding quality (first-pass rate) | Tracked in /doctor |
| Context compression | LLM summarization + micro_compress |
| Memory cache hit rate | Tracked in /doctor |
| Token usage | Tracked in /doctor |
| Model usage | Tracked in /doctor |
| Audit export | /audit + API endpoints |
| Benchmark | scripts/benchmark.sh |

**Trend**: Strong observability coverage

---

## 8. Security & Permissions

| Feature | Status |
|---------|--------|
| Glob pattern rules | Complete |
| Once mode | Complete |
| Auto-low-risk | Complete |
| Read-only mode | Complete |
| Rule explain (with confidence/warnings) | Complete |
| Rule import/export | Complete |
| Rule import merge mode | Complete |
| Dry-run (all registered tools) | Complete |
| LLM-based classifier | Missing |
| Bash sandbox (local/restricted/external) | Complete |
| Dangerous command detection | Complete |

**Gap**: 1 missing (LLM-based permission classifier)
**Trend**: Improving

---

## 9. Programming Capabilities (Claude Code Depth)

| Capability | Status |
|------------|--------|
| Smart Edit (old_string/new_string) | Complete |
| Diagnostic Tracking (before/after edit) | Complete |
| Verification Agent (adversarial) | Complete |
| Batch Refactor (parallel worktree agents) | Complete |
| Multi-Edit orchestration (read parallel/write serial) | Complete |
| Diff/Patch structured output | Complete |
| Streaming tool execution | Complete |
| Tool result disk cache | Complete |
| LLM memory extraction (forked agent) | Complete |
| Reactive Compact (413 recovery) | Complete |
| Fallback Model | Complete |
| Skill prefetch | Complete |
| Context Collapse | Complete |

**Gap**: 0
**Trend**: Complete

---

## Gap Trend (Weekly)

| Week | Tools Gap | Commands Gap | Agents Gap | Overall |
|------|-----------|--------------|------------|---------|
| Week 1 | -22 | -85 | -5 | |
| Week 5 | -16 | -73 | -3 | |
| Week 9 | -16 | 0 | -5 | |
| Week 11 | -6 | 0 | 0 | |
| Week 12 | -4 | 0 (entry) | 0 | |

**Current assessment**: Entry-level parity achieved across tools, commands, and agents. Remaining gaps are in depth (scaffold commands), frontends (Web/Desktop), and niche features (MCP Server standalone, LLM permission classifier, Voice full implementation).

---

## Verification

Run `/doctor gap` in TUI for a live snapshot.

Run `/doctor json` for full diagnostic JSON.

Check tool coverage:
```bash
grep -c "registry.register" src/tools/mod.rs
```

Check command coverage:
```bash
echo $(( $(grep "^pub const CMD_" src/tui/commands.rs | wc -l) ))
```
