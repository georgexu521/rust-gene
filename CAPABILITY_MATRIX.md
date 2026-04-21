# Capability Matrix

> Last updated: 2026-04-21
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

| Command | Handler | Description | Status |
|---------|---------|-------------|--------|
| `/config` | handle_config | Configuration | 🔴 Scaffold |
| `/copy` | handle_copy | Copy to clipboard | 🔴 Scaffold |
| `/desktop` | handle_desktop | Desktop integration | 🔴 Scaffold |
| `/branch` | handle_branch | Branch management | 🔴 Scaffold |
| `/chrome` | handle_chrome | Chrome integration | 🔴 Scaffold |
| `/color` | handle_color | Color theme | 🔴 Scaffold |
| `/effort` | handle_effort | Effort estimation | 🔴 Scaffold |
| `/focus` | handle_focus | Focus mode | 🔴 Scaffold |
| `/hooks` | handle_hooks | Hook management | 🔴 Scaffold |
| `/install` | handle_install | Install dependencies | 🔴 Scaffold |
| `/lsp` | handle_lsp | LSP management | 🔴 Scaffold |
| `/migrate` | handle_migrate | Migration tool | 🔴 Scaffold |
| `/npm` | handle_npm | NPM helper | 🔴 Scaffold |
| `/pause` | handle_pause | Pause agent | 🔴 Scaffold |
| `/preamble` | handle_preamble | Preamble edit | 🔴 Scaffold |
| `/profiling` | handle_profiling | Profiling tool | 🔴 Scaffold |
| `/prompt` | handle_prompt | Prompt management | 🔴 Scaffold |
| `/reload` | handle_reload | Reload config | 🔴 Scaffold |
| `/reject` | handle_reject | Reject suggestion | 🔴 Scaffold |
| `/rollback` | handle_rollback | Rollback changes | 🔴 Scaffold |
| `/shadow` | handle_shadow | Shadow mode | 🔴 Scaffold |
| `/skeleton` | handle_skeleton | Code skeleton | 🔴 Scaffold |
| `/slack` | handle_slack | Slack integration | 🔴 Scaffold |
| `/slots` | handle_slots | Slot management | 🔴 Scaffold |
| `/stealth` | handle_stealth | Stealth mode | 🔴 Scaffold |
| `/subscribe` | handle_subscribe | Subscribe updates | 🔴 Scaffold |
| `/ticker` | handle_ticker | Ticker tool | 🔴 Scaffold |
| `/token` | handle_token | Token info | 🔴 Scaffold |
| `/untrap` | handle_untrap | Untrap mouse | 🔴 Scaffold |
| `/verbose` | handle_verbose | Verbose output | 🔴 Scaffold |
| `/webhook` | handle_webhook | Webhook management | 🔴 Scaffold |
| `/wizard` | handle_wizard | Wizard mode | 🔴 Scaffold |
| `/workspace` | handle_workspace | Workspace management | 🔴 Scaffold |
| `/write` | handle_write | Write file | 🔴 Scaffold |

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
| Commands | 114 | 16 (14%) | 28 (25%) | 70 (61%) |
| Tools | ~58 | ~40 (69%) | ~16 (28%) | ~2 (3%) |

---

## Next Steps

1. **Phase 1 (W4-W6)**: Focus on raising Scaffold commands to Usable
2. Target: Convert 20+ Scaffold commands to Usable in Phase 1