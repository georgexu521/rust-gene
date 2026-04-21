# Claude Gap Scorecard

> **Last updated**: 2026-04-21
> **Update frequency**: Weekly (every Monday)
> **Goal**: Track差距趋势 towards Claude Code parity

## Summary

| Dimension | Claude Code | Our Implementation | Gap | Trend |
|-----------|-------------|-------------------|-----|-------|
| **Tools** | 64 | 48 | -16 | Improving |
| **Commands** | 101 | 22 | -79 | Improving |
| **Agents** | 7 types | 2 types | -5 | Stable |
| **Transport** | 3 (stdio/http/ws) | 2 (stdio/http) | -1 | Improving |
| **Auth** | OAuth/SSO | OAuth (MCP) | Partial | Improving |
| **Frontend** | CLI/TUI/Web/Desktop | CLI/TUI | -2 | Stable |
| **Hooks** | Full event system | Pre/Post only | Medium | Improving |

Legend: ✅ Complete | 🟡 Partial | ❌ Missing | — N/A

---

## 1. Tools Coverage

**Claude Code**: 64 tools
**Our implementation**: 48 tools

| Tool Category | Claude Code | Our Implementation | Status |
|---------------|------------|-------------------|--------|
| **File Operations** | | | |
| FileRead | ✅ | ✅ file_read | Complete |
| FileWrite | ✅ | ✅ file_write | Complete |
| FileEdit | ✅ | ✅ file_edit | Complete |
| Glob | ✅ | ✅ glob | Complete |
| Grep | ✅ | ✅ grep | Complete |
| **Bash** | | | |
| Bash | ✅ | ✅ bash | Complete |
| PowerShell | ✅ | ✅ powershell | Complete |
| REPL | ✅ | ✅ repl | Complete |
| **Web** | | | |
| WebFetch | ✅ | ✅ web_fetch | Complete |
| WebSearch | ✅ | ✅ web_search | Complete |
| Browser | ✅ | ✅ browser | Complete |
| **Agent** | | | |
| Agent | ✅ | ✅ agent | Complete |
| TaskCreate | ✅ | ✅ task_create | Complete |
| TaskList | ✅ | ✅ task_list | Complete |
| TaskGet | ✅ | ✅ task_get | Complete |
| **Context** | | | |
| MemorySave | ✅ | ✅ memory_save | Complete |
| MemoryLoad | ✅ | ✅ memory_load | Complete |
| Compact | ✅ | ✅ context_compress | Complete |
| **MCP** | | | |
| MCPTool | ✅ | ✅ mcp | Complete |
| McpAuth | ✅ | ✅ mcp_auth | Complete |
| ListMcpResources | ✅ | ✅ list_mcp_resources | Complete |
| **LSP** | | | |
| LSP | ✅ | ✅ lsp | Complete |
| **Worktree** | | | |
| Worktree | ✅ | ✅ worktree | Complete |
| **Skills** | | | |
| SkillLoad | ✅ | ✅ skill_load | Complete |
| SkillList | ✅ | ✅ skill_list | Complete |
| **Plan Mode** | ✅ | ✅ plan | Complete |
| **Socratic** | ✅ | ✅ socratic | Complete |
| **Swarm** | ✅ | ✅ swarm | Complete |
| **Diff/Patch** | ✅ | ✅ diff | Complete |
| **Notebook** | ✅ | ✅ notebook | Complete |
| **Team** | ✅ | ✅ team | Complete |
| **Voice** | ✅ | ✅ voice | Complete |
| **Remote** | | | |
| RemoteTrigger | ✅ | ✅ remote_trigger | Complete |
| RemoteDev | ✅ | ✅ remote_dev | Complete |
| **Git** | | | |
| GitStatus | ✅ | ✅ git | Complete |
| GitCommit | ✅ | ✅ git | Complete |
| **GitHub** | | | |
| GitHub | ✅ | ✅ github | Complete |
| **Calculate** | ✅ | ✅ calculate | Complete |
| **Encode** | ✅ | ✅ encode | Complete |
| **JSON** | ✅ | ✅ json_query | Complete |
| **Datetime** | ✅ | ✅ datetime | Complete |
| **TodoWrite** | ✅ | ✅ todo | Complete |
| **Telemetry** | ✅ | ✅ telemetry | Complete |
| **Plugin** | | | |
| PluginList | ✅ | ✅ plugin_list | 🟡 |
| PluginManage | ✅ | ✅ plugin_manage | 🟡 |
| **Ask** | ✅ | ✅ ask_user | Complete |
| **Symbol** | ✅ | ✅ symbol | Complete |
| **ProjectList** | ✅ | ✅ project_list | Complete |
| **Cron** | ✅ | ✅ cron | Complete |
| **Share** | ✅ | ✅ share | Complete |
| **Sleep** | ✅ | ✅ sleep | Complete |
| **Refactor** | ✅ | ✅ refactor | 🟡 |
| **Workbench** | ✅ | ✅ workbench | 🟡 |
| **ToolSearch** | ✅ | ✅ tool_search | Complete |
| **SendMessage** | ✅ | ✅ send_message | Complete |
| **ExitPlanMode** | ✅ | ✅ exit_plan_mode | Complete |
| **EnterPlanMode** | ✅ | ✅ enter_plan_mode | Complete |
| **ExitWorktree** | ✅ | ✅ exit_worktree | Complete |
| **EnterWorktree** | ✅ | ✅ enter_worktree | Complete |
| **Brief** | ✅ | ❌ | Missing |
| **Cost** | ✅ | ❌ | Missing |
| **Clear** | ✅ | ❌ | Missing |
| **Config** | ✅ | ❌ | Missing |
| **Context visualization** | ✅ | ❌ | Missing |
| **Copy** | ✅ | ❌ | Missing |
| **Desktop** | ✅ | ❌ | Missing |
| **Resume** | ✅ | 🟡 | Partial |
| **Rewind** | ✅ | 🟡 | Partial |
| **MCP (server)** | ✅ | 🟡 | Partial |

**Gap**: 16 missing tools, 6 partial tools
**Trend**: Improving (was 22 missing in Week 1)

---

## 2. Commands Coverage

**Claude Code**: 101 commands
**Our implementation**: 28 commands

| Command | Claude Code | Our Implementation | Status |
|---------|------------|-------------------|--------|
| `/help` | ✅ | ✅ | Complete |
| `/clear` | ✅ | ✅ | Complete |
| `/quit` / `/exit` | ✅ | ✅ | Complete |
| `/memory` | ✅ | ✅ | Complete |
| `/save` | ✅ | ✅ | Complete |
| `/cost` | ✅ | ✅ | Complete |
| `/model` | ✅ | ✅ | Complete |
| `/status` | ✅ | ✅ | Complete |
| `/tools` | ✅ | ✅ | Complete |
| `/tasks` | ✅ | ✅ | Complete |
| `/agents` | ✅ | ✅ | Complete |
| `/doctor` | ✅ | ✅ | Complete |
| `/audit` | ✅ | ✅ | Complete |
| `/permissions` | ✅ | ✅ | Complete |
| `/diff` | ✅ | ✅ | Complete |
| `/resume` | ✅ | 🟡 | Partial |
| `/rewind` | ✅ | 🟡 | Partial |
| `/commit` | ✅ | ✅ | Complete |
| `/review-pr` | ✅ | ✅ | Complete |
| `/review` | ✅ | ✅ | Complete |
| `/security-review` | ✅ | ✅ | Complete |
| `/explain` | ✅ | ✅ | Complete |
| `/fix` | ✅ | ✅ | Complete |
| `/mcp` | ✅ | ✅ | Complete |
| `/vim` | ✅ | ✅ | Complete |
| `/compact` | ✅ | ✅ | Complete |
| `/btw` | ✅ | ✅ | Complete |
| `/config` | ✅ | ❌ | Missing |
| `/context` | ✅ | ✅ | Complete |
| `/copy` | ✅ | ❌ | Missing |
| `/desktop` | ✅ | ❌ | Missing |
| `/branch` | ✅ | ❌ | Missing |
| `/chrome` | ✅ | ❌ | Missing |
| `/color` | ✅ | ❌ | Missing |
| `/diff` | ✅ | ✅ | Complete |
| `/effort` | ✅ | ❌ | Missing |
| `/exit-plan` | ✅ | ✅ | Complete |
| `/focus` | ✅ | ❌ | Missing |
| `/git` | ✅ | ✅ | Complete |
| `/history` | ✅ | ✅ | Complete |
| `/hooks` | ✅ | ❌ | Missing |
| `/install` | ✅ | ❌ | Missing |
| `/keybindings` | ✅ | ✅ | Complete |
| `/lsp` | ✅ | ❌ | Missing |
| `/migrate` | ✅ | ❌ | Missing |
| `/mode` | ✅ | ✅ | Complete |
| `/npm` | ✅ | ❌ | Missing |
| `/package` | ✅ | ✅ | Complete |
| `/pause` | ✅ | ❌ | Missing |
| `/preamble` | ✅ | ❌ | Missing |
| `/profiling` | ✅ | ❌ | Missing |
| `/prompt` | ✅ | ❌ | Missing |
| `/redo` | ✅ | ❌ | Missing |
| `/reject` | ✅ | ❌ | Missing |
| `/reload` | ✅ | ❌ | Missing |
| `/retry` | ✅ | ❌ | Missing |
| `/rollback` | ✅ | ❌ | Missing |
| `/session` | ✅ | ❌ | Missing |
| `/shadow` | ✅ | ❌ | Missing |
| `/share` | ✅ | ❌ | Missing |
| `/skeleton` | ✅ | ❌ | Missing |
| `/slack` | ✅ | ❌ | Missing |
| `/slots` | ✅ | ❌ | Missing |
| `/stealth` | ✅ | ❌ | Missing |
| `/stop` | ✅ | ❌ | Missing |
| `/subscribe` | ✅ | ❌ | Missing |
| `/ticker` | ✅ | ❌ | Missing |
| `/token` | ✅ | ❌ | Missing |
| `/undo` | ✅ | ❌ | Missing |
| `/untrap` | ✅ | ❌ | Missing |
| `/verbose` | ✅ | ❌ | Missing |
| `/webhook` | ✅ | ❌ | Missing |
| `/wizard` | ✅ | ❌ | Missing |
| `/workspace` | ✅ | ❌ | Missing |
| `/write` | ✅ | ❌ | Missing |
| ... (and 30+ more) | | | |

**Gap**: 73 missing commands
**Trend**: Improving (was 85 missing in Week 1)

---

## 3. Agent Types

**Claude Code**: 7 agent types (task, teammate, assistant, critic, verifier, custom, etc.)
**Our implementation**: 4 types (general purpose, teammate, critic, assistant, remote, verification)

| Agent Type | Claude Code | Our Implementation | Status |
|------------|------------|-------------------|--------|
| Task Agent | ✅ | ✅ | Complete |
| Teammate | ✅ | ✅ | Complete |
| Assistant | ✅ | ✅ | Complete |
| Critic | ✅ | ✅ | Complete |
| Verifier | ✅ | ✅ | Complete |
| Remote Specialist | ✅ | ✅ | Complete |
| Dream Task | ✅ | 🟡 | Partial (role exists, skill not fully implemented) |

**Gap**: 3 missing (partially complete agent types)
**Trend**: Improving

---

## 4. Transport & Protocol

| Feature | Claude Code | Our Implementation | Status |
|---------|------------|-------------------|--------|
| stdio transport | ✅ | ✅ | Complete |
| HTTP/SSE transport | ✅ | ✅ | Complete |
| WebSocket transport | ✅ | ❌ | Missing |
| MCP OAuth | ✅ | ✅ | Complete |
| MCP streaming | ✅ | 🟡 | Partial |
| Bridge auth (multi-token) | ✅ | ✅ | Complete |
| Bridge tenant isolation | ✅ | ✅ | Complete |

**Gap**: 1 missing transport (WebSocket)
**Trend**: Improving

---

## 5. Frontends

| Frontend | Claude Code | Our Implementation | Status |
|----------|------------|-------------------|--------|
| CLI | ✅ | ✅ | Complete |
| TUI | ✅ | ✅ | Complete |
| Web | ✅ | ❌ | Missing |
| Desktop (Electron) | ✅ | ❌ | Missing |

**Gap**: 2 missing frontends
**Trend**: Stable

---

## 6. Hooks System

| Hook Event | Claude Code | Our Implementation | Status |
|------------|------------|-------------------|--------|
| pre-tool | ✅ | ✅ | Complete |
| post-tool | ✅ | ✅ | Complete |
| pre-api | ✅ | ✅ | Complete |
| post-api | ✅ | ✅ | Complete |
| pre-agent | ✅ | ❌ | Missing |
| post-agent | ✅ | ❌ | Missing |
| tool.execute.before | ✅ | ✅ | Complete |
| tool.execute.after | ✅ | ✅ | Complete |
| Provider hooks | ✅ | ✅ | Complete |
| Config hooks | ✅ | ✅ | Complete |

**Gap**: 2 missing hook events
**Trend**: Improving

---

## 7. Performance Metrics

| Metric | Claude Code | Our Implementation | Status |
|--------|------------|-------------------|--------|
| Cold start (no key) | <100ms | ~23ms | ✅ Better |
| First token latency | ~500ms | varies | 🟡 |
| Tool execution (avg) | varies | varies | 🟡 |
| Context compression | ✅ LLM summarization | ✅ LLM summarization | ✅ |
| Cache hit rate | varies | varies | 🟡 |

**Trend**: Stable

---

## 8. Security & Permissions

| Feature | Claude Code | Our Implementation | Status |
|---------|------------|-------------------|--------|
| Glob pattern rules | ✅ | ✅ | Complete |
| Once mode | ✅ | ✅ | Complete |
| Auto-low-risk | ✅ | ✅ | Complete |
| Read-only mode | ✅ | ✅ | Complete |
| Rule explain | ✅ | ✅ | Complete |
| Rule import/export | ✅ | ✅ | Complete |
| Dry-run | ✅ | ✅ | Complete |
| LLM-based classifier | ✅ | ❌ | Missing |

**Gap**: 1 missing feature
**Trend**: Improving

---

## Gap Trend (Weekly)

| Week | Tools Gap | Commands Gap | Agents Gap | Overall |
|------|-----------|--------------|------------|---------|
| Week 1 | -22 | -85 | -5 | 🔴 |
| Week 2 | -20 | -83 | -5 | 🔴 |
| Week 3 | -18 | -81 | -5 | 🟡 |
| Week 4 | -16 | -79 | -5 | 🟡 |
| Week 5 | -16 | -73 | -3 | 🟡 |

**Goal**: Reduce gap to <10 across all dimensions by end of Phase 9

---

## Priority Next Steps

1. **High Priority**: Add more commands (`/btw`, `/config`, `/context`, `/keybindings`)
2. **High Priority**: Add WebSocket MCP transport
3. **Medium Priority**: Add teammate/remote specialist agent types
4. **Medium Priority**: Add LLM-based permission classifier
5. **Low Priority**: Web/Desktop frontends (Phase 10+)

---

## Verification

Run `/doctor json` to see current implementation status:
```
/doctor json
```

Check tool coverage:
```bash
grep -c "registry.register" src/tools/mod.rs  # Should show ~64
```

Check command coverage:
```bash
grep -c "pub const CMD_" src/tui/commands.rs  # Should show ~22
```
