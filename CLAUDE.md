# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Product Direction

Priority Agent should not be treated as a broad Claude Code clone. The project
goal is a narrow, deep, personalized, and verifiable programming assistant for
gex's local projects and workflows. See
`docs/PERSONAL_AGENT_PRODUCT_PRINCIPLES_2026-05-18.md`.

Current tactical plan: first reach Claude Code-like runtime parity, then
personalize and diverge. Use
`docs/CLAUDE_CODE_PARITY_IMPLEMENTATION_PLAN_2026-05-20.md` as the active
near-term implementation roadmap.

## Build and Development Commands

```bash
# Build default (TUI only)
cargo build

# Build with all features enabled (experimental-api-server is the main optional feature)
cargo build --features "experimental-api-server"

# Build release version
cargo build --release

# ⚠️ IMPORTANT: Use `cargo run` not `./target/debug/priority-agent`
# The pre-built binary is ad-hoc signed and gets blocked by macOS sandbox
# when accessing network. `cargo run` re-signs and works correctly.
# This affects: --eval-run, --provider-health, --tui, --cli, --api

# Run all tests
cargo test

# Run a specific test
cargo test test_analyze_critical_task

# Run the CLI (use cargo run, not ./target/debug/priority-agent — see note above)
cargo run -- --help

# Run legacy CLI mode (does NOT require API key)
cargo run -- --legacy init
./target/debug/priority-agent --legacy add "任务名称"
cargo run -- --legacy list
cargo run -- --legacy next
cargo run -- --legacy done <task_id>

# Run TUI mode (requires LLM API key)
export MOONSHOT_API_KEY="your-key"
cargo run -- --tui

# Run eval test (non-interactive)
bash scripts/test-m3.sh tests/fixtures/test-basic-read.txt

# Run API server mode (requires feature flag + API key)
cargo run --features experimental-api-server -- --api --port 8787
```

## Architecture Overview

Priority Agent is a Rust re-implementation of Claude Code's architecture. It has two distinct runtime paths:

1. **Legacy CLI** (`--legacy`): Local task management with hierarchical weight calculation. Does NOT require an LLM provider.
2. **TUI/API modes**: Full AI assistant with tool calling, streaming, agents, and memory. Requires `MOONSHOT_API_KEY` or `OPENAI_API_KEY`.

The `--legacy` flag is detected **before** `clap` parses arguments and bypasses all LLM initialization entirely (`main.rs:92-96`).

### Module Structure

**`tools/`** — Extensible tool system (Claude Code pattern)
- Core `Tool` trait in `mod.rs`: `name()`, `description()`, `parameters()`, `execute()`
- `ToolRegistry::default_registry()` registers 30+ tools including `file_read`, `file_write`, `file_edit`, `bash`, `glob`, `grep`, `agent`, `task_create`, `web_fetch`, `web_search`, `memory_save`, `memory_load`, `todo_write`, `calculate`, `json_query`, `encode`, `socratic_analyze`, `plan`, `mcp`, `swarm`, `project_list`, `skill_manage`, `ask_user`, `lsp`, `worktree`, `workbench`, `remote_trigger`
- `ToolContext` carries `working_dir`, `permissions`, `agent_manager`, `llm_provider`, `mcp_manager`, `lsp_manager`, `worktree_manager`

**`engine/`** — Query engine and conversation orchestration
- `query_engine.rs` — Non-streaming `QueryEngine`
- `streaming.rs` — `StreamingQueryEngine` used by TUI; produces `StreamEvent`s
- `conversation_loop.rs` — `ConversationLoop` shared builder between streaming and non-streaming engines
- `context_compressor.rs` — Token budget management and message summarization when context grows too large
- `plan_mode.rs` — Plan approval system with `PlanModeManager` and `PlanTool`
- `socratic.rs` / `socratic_executor.rs` — Socratic analysis tool for breaking down problems
- `mcp.rs` — MCP (Model Context Protocol) manager and `McpManageTool`
- `swarm.rs` — Swarm agent coordination
- `turn_state.rs` — Iteration limiting and diagnostic reporting
- `lsp.rs` — LSP manager and language server auto-detection
- `worktree.rs` — Git worktree manager
- `context_collapse.rs` — Context folding (persist history to disk, restore on load)

**`agent/`** — Sub-agent system
- `agent.rs` — `Agent`, `AgentConfig`, `AgentHandle`, `AgentStatus`
- `manager.rs` — `AgentManager` with `tokio::mpsc` channels for agent messaging; stores `AgentResult`s
- Advanced types: Teammate, Critic, Assistant, Remote, Verifier, Dream agents

**`tui/`** — Terminal UI
- `app.rs` — Main TUI loop (`TuiApp`) with `AppMode::Chat` / `AppMode::Settings` / `AppMode::VimNormal`
- `commands.rs` — Slash command registry (e.g., `/settings`, `/help`, `/commit`)
- `screens/` — Different UI screens
- `components/` — Input, messages, progress bars, file browser, settings panels, markdown renderer

**`ide/`** — IDE integration (VS Code / Cursor detection and CLI wrapper)

**`bridge/`** — Remote session bridge (`BridgeClient` for HTTP-based remote triggers)

**`state/`** — React-style state management (`AppState`, `StateStore`, `EventBus`)

**`services/`** — API and configuration
- `api/kimi.rs` — Kimi/Moonshot client (OpenAI-compatible)
- `api/openai.rs` — Generic OpenAI-compatible client
- `config.rs` — TOML-based `AppConfig`

**`session_store/`** — SQLite persistence for chat sessions and messages (rusqlite, FTS5 search, migration framework)

**`memory/`** — Working memory and LLM-driven extraction (`MemoryManager` with forked agent mode, trailing runs, keyword extraction)

**`permissions/`** — Permission system (`PermissionMode`: Default, AutoLowRisk, AutoAll, ReadOnly, Once; glob pattern rules)

**`skills/`** — Skill system (markdown parsing with frontmatter, external URL loading, prefetch)

**`security/`** — Security utilities (`is_dangerous_command` for bash tool)

**Legacy modules** (only compiled with `legacy-cli` feature)
- `weight_engine/`, `ai_analyzer/`, `cli/`, `context_manager/`

## Key Design Patterns

**Tool System Pattern:**
```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Value;
    async fn execute(&self, params: Value, context: ToolContext) -> ToolResult;
}
```

**Query Engine Flow:**
1. User input → `StreamingQueryEngine::query()`
2. LLM generates response (possibly tool calls)
3. `ConversationLoop` parses and executes tools via `ToolRegistry`
4. Tool results streamed back as `StreamEvent::ToolResult`
5. Final assistant message streamed as `StreamEvent::Message`
6. TUI appends events to `messages` list

**Agent Wiring:**
- `main.rs` creates `AgentManager` and wires it to both `QueryEngine` and `StreamingQueryEngine` via `with_agent_manager()`
- `AgentTool` uses `ToolContext.agent_manager` to spawn sub-agents

## Configuration

Environment variables:
```bash
# MiniMax (currently configured, used for eval testing)
export MINIMAX_API_KEY="your-key"
export MINIMAX_BASE_URL="https://api.minimaxi.com/v1"
export MINIMAX_MODEL="MiniMax-M3"  # M3 for best results, M2.7 for fallback

# Kimi/Moonshot
export MOONSHOT_API_KEY="your-api-key"
export MOONSHOT_BASE_URL="https://api.moonshot.cn/v1"  # optional
export MOONSHOT_MODEL="kimi-k2.5"  # optional

# OpenAI
export OPENAI_API_KEY="your-key"  # alternative
export OPENAI_BASE_URL="..."  # optional
export OPENAI_MODEL="gpt-4o"  # optional

# DeepSeek
export DEEPSEEK_API_KEY="your-key"
```

## Data Storage

- macOS: `~/Library/Application Support/priority-agent/`
- Linux: `~/.local/share/priority-agent/`
- SQLite session DB: `.../priority-agent/sessions.db`

## Testing

Unit tests are embedded under `#[cfg(test)]` in each module. Run with `cargo test`. There are 2100+ tests covering tools, engine, TUI components, permissions, and state management.

Pre-existing failures (not caused by recent changes):
- `runtime_spine_behavior_contract` — known pre-existing
- `test_minimax_client_defaults` — model name mismatch due to env MINIMAX_MODEL
- `grep_allows_runtime_tool_result_artifacts_read_only` — known pre-existing

### Eval Testing (non-interactive, with LLM)
```bash
# Requires MINIMAX_API_KEY to be set
bash scripts/test-m3.sh tests/fixtures/test-basic-read.txt

# TUI testing requires manual interaction:
cargo run -- --tui
# See tests/TUI_TEST_PLAN.md for 7 test scenarios
```

## Known Gaps vs Claude Code

Compared to the real Claude Code (`~/Desktop/claude/`), this reimplementation is architecturally aligned but missing substantial product-depth features. Do not assume parity exists unless verified.

Current gap source of truth: `docs/CLAUDE_CODE_GAP_MATRIX_2026-05-03.md`.

### What We Do Well
- Unified `ConversationLoop` + `StreamingQueryEngine` with context compression, reactive compact, and memory injection
- Core tool chain complete (~60 tool types, ~63 registered instances, recently de-bloated)
- Advanced agents: Teammate, Critic, Assistant, Remote, Dream, Verifier
- Plan Mode TUI integration (`PlanApprovalChannel` + `PlanModeManager`)
- Socratic analysis — unique deep-reasoning tool
- SQLite session persistence with FTS5 search and migration framework
- MCP: stdio/HTTP/WebSocket transport, OAuth, tool injection
- Hooks: Pre/Post tool hooks with fail-open/fail-closed, audit trail
- Sandbox: local/restricted/external backends with allowlist and fallback
- LLM memory: forked agent mode, trailing runs, throttle, mutual exclusion
- Context collapse: persist history to disk, restore on load
- Plugin system: manifest discovery, validation, enable/disable, tool injection
- 100+ slash commands (session, undo, redo, retry, audit, doctor, etc.)
- GitHub Action for Issue/PR auto-triggered agent workflows
- Benchmark script with markdown report output

### Current Gaps (Remaining)
- Plugin ecosystem productization (marketplace, signature trust, lifecycle governance)
- MCP Server (standalone server capability — partial)
- Voice mode
- Workspace crate split (priority-core/priority-cli — Phase 1-1 done)

### Recently Completed
- Static prefix caching (Reasonix-style): dynamic zones merged into user messages to preserve cache
- Tool de-bloat: removed task_* (6 tools), session/UI tools (4 tools)
- Config simplification: removed 4 unused fields, external provider 13→3 fields
- Lazy init: SessionStore, MemoryManager, AgentManager all use OnceLock
- TUI redesign: token themes, card messages, scroll indicator, context bar

### Key Environment Variables (Common)

| Variable | Purpose | Default |
|----------|---------|---------|
| `PRIORITY_AGENT_THINKING` | Enable/disable thinking | `1` |
| `PRIORITY_AGENT_THINKING_BUDGET` | Thinking token budget | adaptive |
| `PRIORITY_AGENT_LLM_MEMORY_EXTRACTION` | Enable LLM memory extraction | `0` |
| `PRIORITY_AGENT_LLM_MEMORY_FORKED` | Enable forked agent mode | `0` |
| `PRIORITY_AGENT_REACTIVE_COMPACT` | Enable reactive compaction | `0` |
| `PRIORITY_AGENT_CONTEXT_COLLAPSE` | Enable context collapse | `0` |
| `PRIORITY_AGENT_BASH_BACKEND` | Bash execution backend | `local` |
| `PRIORITY_AGENT_BASH_EXTERNAL_CMD` | External sandbox wrapper | — |
| `PRIORITY_AGENT_FALLBACK_MODEL` | Fallback LLM model | — |
| `PRIORITY_AGENT_HOOK_TIMEOUT_MS` | Hook execution timeout | 5000 |
| `PRIORITY_AGENT_HOOK_FAIL_CLOSED` | Hook fail strategy | `0` (fail-open) |
| `PRIORITY_AGENT_DIAGNOSTIC_TRACKING` | Enable diagnostic tracking | `0` |
| `PRIORITY_AGENT_ADVANCED_AGENTS` | Enable advanced agent types | `0` |
| `PRIORITY_AGENT_MCP_SERVERS_JSON` | MCP servers config | — |
| `PRIORITY_AGENT_SKILLS_PATH` | Extra skill directories | — |
| `PRIORITY_AGENT_SKILLS_URL` | Remote skill URLs | — |
| `PRIORITY_AGENT_PROVIDER_<NAME>` | Custom provider config | — |

## Historical Implementation Logs

Detailed implementation logs for Phases 1-11 are preserved in `docs/` for reference:
- Phase execution details: see commit history
- Gap analysis: `docs/CLAUDE_CODE_GAP_MATRIX_2026-05-03.md`
- Sprint plans: `docs/CLAUDE_CODE_PARITY_IMPLEMENTATION_PLAN_2026-05-20.md`
- Benchmark reports: `docs/benchmarks/`
