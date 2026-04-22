# Capability Matrix

> Last updated: 2026-04-22
> Purpose: Track maturity of commands and tools

## Command Maturity Levels

| Level | Description |
|-------|-------------|
| **Production-ready** | Fully implemented, tested, stable |
| **Usable** | Working implementation, may need polish |
| **Scaffold** | Placeholder only, needs significant work |

## Commands (114 total)

### Production-ready (Core)

| Command | Handler | Description | Status |
|---------|---------|-------------|--------|
| `/help` | handle_help | Show help information | ✅ Production |
| `/clear` | handle_clear | Clear conversation | ✅ Production |
| `/quit` / `/exit` | handle_quit | Exit application | ✅ Production |
| `/model` | handle_model | Switch model | ✅ Production |
| `/status` | handle_status | Show status | ✅ Production |
| `/tasks` | handle_tasks | Task management | ✅ Production |
| `/agents` | handle_agents | Agent management | ✅ Production |
| `/doctor` | handle_doctor | Diagnostic report | ✅ Production |
| `/audit` | handle_audit | Audit logs | ✅ Production |
| `/permissions` | handle_permissions | Permission management | ✅ Production |
| `/diff` | handle_diff | Show diff | ✅ Production |
| `/compact` | handle_compact | Compress context | ✅ Production |
| `/mcp` | handle_mcp | MCP management | ✅ Production |
| `/git` | handle_git | Git operations | ✅ Production |
| `/history` | handle_history | Session history | ✅ Production |
| `/context` | handle_context | Context status | ✅ Production |

### Usable (Working but needs polish)

| Command | Handler | Description | Status |
|---------|---------|-------------|--------|
| `/session` | handle_session | Session management | 🟡 Usable |
| `/merge` | handle_merge | Merge another session into current | 🟡 Usable |
| `/cleanup` | handle_cleanup | Cleanup sessions/cache/logs with confirmation | 🟡 Usable |
| `/snippet` | handle_snippet | Save/load/list local snippets | 🟡 Usable |
| `/bookmark` | handle_bookmark | Save/list/go named bookmarks | 🟡 Usable |
| `/tag` | handle_tag | Add/list/find item tags | 🟡 Usable |
| `/search` | handle_search | Search sessions by query | 🟡 Usable |
| `/filter` | handle_filter | Filter current session messages by role/query | 🟡 Usable |
| `/profile` | handle_profile | Persisted profile show/set/unset | 🟡 Usable |
| `/feedback` | handle_feedback | Persist feedback records | 🟡 Usable |
| `/theme` | handle_theme | Show/set persisted theme preset | 🟡 Usable |
| `/color` | handle_color | Theme color alias with persistence | 🟡 Usable |
| `/focus` | handle_focus | Toggle focused chat rendering mode | 🟡 Usable |
| `/pause` | handle_pause | Pause/resume message submission | 🟡 Usable |
| `/shortcuts` | handle_shortcuts | Show active keybindings | 🟡 Usable |
| `/quick` | handle_quick | Show contextual quick-action panel | 🟡 Usable |
| `/resume` | handle_resume | Resume session | 🟡 Usable |
| `/rewind` | handle_rewind | Rewind conversation | 🟡 Usable |
| `/retry` | handle_retry | Retry last operation | 🟡 Usable |
| `/stop` | handle_stop | Stop current operation | 🟡 Usable |
| `/undo` | handle_undo | Undo last operation | 🟡 Usable |
| `/redo` | handle_redo | Redo operation | 🟡 Usable |
| `/share` | handle_share | Share session | 🟡 Usable |
| `/cost` | handle_cost | Show cost breakdown | 🟡 Usable |
| `/btw` | handle_btw | Quick comment | 🟡 Usable |
| `/mode` | handle_mode | Switch mode | 🟡 Usable |
| `/package` | handle_package | Package info | 🟡 Usable |
| `/review` | handle_review | Code review | 🟡 Usable |
| `/security-review` | handle_security_review | Security review | 🟡 Usable |
| `/commit` | handle_commit | Git commit | 🟡 Usable |
| `/review-pr` | handle_review_pr | PR review | 🟡 Usable |
| `/explain` | handle_explain | Explain code | 🟡 Usable |
| `/fix` | handle_fix | Fix code | 🟡 Usable |
| `/settings` | handle_settings | Settings menu | 🟡 Usable |
| `/config` | handle_config | Configuration | 🟡 Usable |
| `/reload` | handle_reload | Reload config/plugins/skills | 🟡 Usable |
| `/prompt` | handle_prompt | Persisted prompt show/edit/append/apply/reset | 🟡 Usable |
| `/migrate` | handle_migrate | SQLx migration up/down/status helper | 🟡 Usable |
| `/webhook` | handle_webhook | Persisted webhook create/list/delete/test | 🟡 Usable |
| `/rollback` | handle_rollback | Rollback changes | 🟡 Usable |
| `/test` | handle_test | Run tests with correct exit status | 🟡 Usable |
| `/ping` | handle_ping | Local round-trip latency check | 🟡 Usable |
| `/workspace` | handle_workspace | Workspace info + git worktree listing | 🟡 Usable |
| `/stealth` | handle_stealth | Persisted stealth mode toggle/status | 🟡 Usable |
| `/shadow` | handle_shadow | Persisted shadow mode toggle/status | 🟡 Usable |
| `/reject` | handle_reject | Reject pending permission request | 🟡 Usable |
| `/subscribe` | handle_subscribe | Persisted subscription list/add/remove/clear | 🟡 Usable |
| `/slots` | handle_slots | Persisted slot list/get/set/unset/clear | 🟡 Usable |
| `/preamble` | handle_preamble | Persisted preamble show/set/reset | 🟡 Usable |
| `/verbose` | handle_verbose | Persisted verbose toggle + runtime log level | 🟡 Usable |
| `/backend` | handle_backend | Persisted backend mode switch/status | 🟡 Usable |
| `/sandbox` | handle_sandbox | Persisted sandbox toggle/status | 🟡 Usable |
| `/env` | handle_env | List/get/set/unset PRIORITY_AGENT_* vars | 🟡 Usable |
| `/cache` | handle_cache | Cache clear/stats | 🟡 Usable |
| `/trace` | handle_trace | Persisted trace toggle/status + log level | 🟡 Usable |
| `/import` | handle_import | Import messages from exported session JSON | 🟡 Usable |
| `/wizard` | handle_wizard | Guided setup entry + settings mode | 🟡 Usable |
| `/slack` | handle_slack | Webhook-based connect/status/send/disconnect | 🟡 Usable |
| `/ticker` | handle_ticker | Persisted ticker show/set/clear | 🟡 Usable |
| `/chrome` | handle_chrome | Open URL, list tabs, read bookmarks | 🟡 Usable |
| `/effort` | handle_effort | Persisted effort level | 🟡 Usable |
| `/untrap` | handle_untrap | Clear pending approvals/questions and recover UI mode | 🟡 Usable |
| `/project` | handle_project | Info/list/tree/init project helpers | 🟡 Usable |
| `/benchmark` | handle_benchmark | Script benchmark with synthetic fallback | 🟡 Usable |
| `/init` | handle_init | Bootstrap project directory/files | 🟡 Usable |
| `/login` | handle_login | Persisted local auth-session state | 🟡 Usable |
| `/logout` | handle_logout | Clear local auth-session state | 🟡 Usable |
| `/compact` | handle_compact | Perform context micro-compression and sync session | 🟡 Usable |
| `/plan` | handle_plan | Enter plan mode | 🟡 Usable |
| `/exit-plan` | handle_exit_plan | Exit plan mode | 🟡 Usable |
| `/teammate` | handle_teammate | Teammate agent | 🟡 Usable |
| `/critic` | handle_critic | Critic agent | 🟡 Usable |
| `/assistant` | handle_assistant | Assistant agent | 🟡 Usable |
| `/remote` | handle_remote | Remote agent | 🟡 Usable |
| `/dream` | handle_dream | Dream agent | 🟡 Usable |
| `/custom` | handle_custom | Custom agent | 🟡 Usable |
| `/orchestrate` | handle_orchestrate | Orchestrate agents | 🟡 Usable |

### Scaffold (Needs work)

No scaffold commands at the slash-command layer.

---

## Tools Maturity

### Production-ready

| Tool | Description | Status |
|------|-------------|--------|
| file_read | Read file contents | ✅ Production |
| file_write | Write file contents | ✅ Production |
| file_edit | Edit file contents | ✅ Production |
| glob | File pattern matching | ✅ Production |
| grep | Search in files | ✅ Production |
| bash | Execute bash commands | ✅ Production |
| web_fetch | Fetch web content | ✅ Production |
| web_search | Search the web | ✅ Production |
| agent | Spawn sub-agents | ✅ Production |
| task_create | Create tasks | ✅ Production |
| task_list | List tasks | ✅ Production |
| task_get | Get task details | ✅ Production |
| task_update | Update task | ✅ Production |
| task_stop | Stop task | ✅ Production |
| memory_save | Save to memory | ✅ Production |
| memory_load | Load from memory | ✅ Production |
| mcp | MCP tool invoker | ✅ Production |
| lsp | Language server | ✅ Production |
| worktree | Git worktree | ✅ Production |

### Usable

| Tool | Description | Status |
|------|-------------|--------|
| cost | Cost tracking | 🟡 Usable |
| clear | Clear history | 🟡 Usable |
| copy | Clipboard copy | 🟡 Usable |
| config | Configuration | 🟡 Usable |
| context | Context status | 🟡 Usable |
| context_vis | Context visualization | 🟡 Usable |
| desktop | Desktop integration | 🟡 Usable |
| resume | Resume session | 🟡 Usable |
| rewind | Rewind conversation | 🟡 Usable |
| datetime | Date/time utilities | 🟡 Usable |
| calculate | Calculator | 🟡 Usable |
| encode | Encoding utilities | 🟡 Usable |
| diff | Diff tool | 🟡 Usable |
| sleep | Sleep tool | 🟡 Usable |

### Scaffold

| Tool | Description | Status |
|------|-------------|--------|
| mcp_server | MCP server mode | 🔴 Scaffold |
| voice | Voice mode | 🔴 Scaffold |

---

## Statistics

| Category | Total | Production | Usable | Scaffold |
|----------|-------|------------|--------|----------|
| Commands | 114 | 16 (14%) | 98 (86%) | 0 (0%) |
| Tools | ~58 | ~40 (69%) | ~16 (28%) | ~2 (3%) |

---

## Next Steps

1. **Phase 1 (W4-W6)**: Focus on raising Scaffold commands to Usable
2. Target: Convert 20+ Scaffold commands to Usable in Phase 1
