# Priority Agent

Priority Agent is a Rust-based agentic coding CLI inspired by Claude Code and
Codex, with an explicit priority/goal layer, observable runtime traces, tool
recovery, memory, MCP, and multi-provider LLM support.

The project started as a weighted-priority desktop agent. It has since evolved
into a programming-agent terminal CLI. The default command is `priority-agent`
or the `pa` shortcut; `--tui` remains as a compatibility entry for the
full-screen terminal interface.

## Current Status

Current project status is tracked in [docs/PROJECT_STATUS.md](docs/PROJECT_STATUS.md).

Latest verified baseline:

- `cargo check --quiet`
- `env PRIORITY_AGENT_WORKFLOW_ENABLED=1 cargo test --quiet -- --test-threads=1`
- Result: `899 passed; 0 failed`

## Quick Start

```bash
cd ~/Desktop/rust-agent

# Development run
cargo run -- --cli

# Release build
cargo build --release
./target/release/priority-agent

# Install to ~/.local/bin
make install
```

If `make install` fails with `No rule to make target 'install'`, run it from the
repository root:

```bash
cd ~/Desktop/rust-agent
make install
```

## API Keys

Priority Agent chooses providers in this order when configured:

```bash
export MINIMAX_API_KEY="..."
export MINIMAX_MODEL="MiniMax-M2.7"   # optional

export OPENAI_API_KEY="..."
export OPENAI_MODEL="gpt-4o"          # optional

export MOONSHOT_API_KEY="..."
export MOONSHOT_MODEL="kimi-k2.5"     # optional
```

## Usage

```bash
# Interactive coding CLI
priority-agent
priority-agent --cli

# Deprecated compatibility alias
priority-agent --tui

# HTTP API server, when built with the experimental API feature
cargo run --features experimental-api-server -- --api --port 8787
```

Common interactive commands:

| Command | Purpose |
|---------|---------|
| `/help` | Show command help |
| `/quick` | Show compact runtime panel: goal, drift, provider, permission state |
| `/trace` | Inspect the latest turn timeline |
| `/goal` | Show or pin the active goal |
| `/goal drift` | Show recent goal drift events |
| `/recover` | Show recent recovery plans |
| `/learn` | Show recent learning events |
| `/improvements` | Scan, review, accept, reject, or apply controlled self-evolution proposals |
| `/memory` | Show memory namespaces; supports `search`, `conflicts`, `review`, `explain`, and `doctor` |
| `/skills` | List bundled coding skills |
| `/karpathy <task>` | Apply careful coding guidelines to a task |
| `/permissions` | Inspect or edit permission rules |
| `/mcp status` | Show MCP server health and approvals |
| `/sessions` | List persisted conversations |
| `/resume` | Pick or search a prior conversation and continue it |
| `/cost` | Inspect token and cost usage |

## Architecture

```text
src/
├── engine/               # Conversation loop, routing, trace, workflow, MCP
├── tools/                # Local tools, MCP tools, memory, project index, web
├── memory/               # Snapshot, prefetch, extraction, maintenance
├── agent/                # Sub-agent lifecycle, role memory, swarm support
├── tui/                  # Interactive CLI UI and slash commands
├── session_store/        # SQLite persistence, traces, learning events
├── permissions/          # Permission rules, sources, decisions
├── services/api/         # LLM provider adapters
├── api/                  # HTTP/WebSocket/SSE API
└── platform/             # Platform adapters such as Telegram
```

Core runtime flow:

```text
User prompt
  -> IntentRouter
  -> TurnTrace
  -> SessionGoal / goal drift checks
  -> Retrieval and memory prefetch
  -> Tool execution with permissions and recovery metadata
  -> LearningEvent persistence
  -> CLI panels backed by trace/state
```

## Implemented Capabilities

- Interactive Claude/Codex-style coding CLI.
- Turn tracing with `/trace`.
- Intent routing with learning feedback from recent tool outcomes.
- Session goal tracking and goal drift visibility.
- Tool failure recovery metadata and `/recover`.
- Persistent memory across `MEMORY.md`, `USER.md`, topic files, and agent JSON
  memory with namespace search and conflict hints.
- MCP client support over stdio, WebSocket, and HTTP, with approval and health
  diagnostics.
- MCP resource listing/reading with trace events.
- SQLite session persistence and learning events.
- Tool orchestration with read-only parallel execution and mutating serial
  execution.
- Permissions with project/global/user rule sources.
- HTTP API, WebSocket, SSE, and platform adapter framework.

## Development

```bash
cargo fmt
cargo check --quiet
cargo test --quiet
env PRIORITY_AGENT_WORKFLOW_ENABLED=1 cargo test --quiet -- --test-threads=1
```

Some tests mutate process environment variables. Use `--test-threads=1` for the
full workflow-enabled suite to avoid cross-test environment interference.

## Documentation Map

- [docs/PROJECT_STATUS.md](docs/PROJECT_STATUS.md): current state and remaining
  priorities.
- [docs/CLAUDE_CODE_ALIGNMENT_PLAN.md](docs/CLAUDE_CODE_ALIGNMENT_PLAN.md):
  Claude Code alignment plan and phase status.
- [docs/REMAINING_CLOSURE_PLAN.md](docs/REMAINING_CLOSURE_PLAN.md): completed
  closure plan for recovery, learning, drift, memory, and MCP health.
- [AGENTS.md](AGENTS.md): detailed development guide and architecture notes.
- [QUICKSTART.md](QUICKSTART.md): setup-oriented guide.

## License

MIT
