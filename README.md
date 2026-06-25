# Priority Agent

Priority Agent is a Rust-based agentic coding CLI inspired by Claude Code and
Codex, with an explicit priority/goal layer, observable runtime traces, tool
recovery, memory, MCP, and multi-provider LLM support.

The project is now focused on the programming-agent terminal CLI. The default
command is `priority-agent` or the `pa` shortcut; `--tui` remains as a
compatibility entry for the full-screen terminal interface.

## Naming And Repository

The public repository is `georgexu521/rust-gene`. The product name is Priority
Agent, and the released command/crate name remains `priority-agent` with the
`pa` shortcut. Older local paths or archived notes may still mention
`rust-agent`; treat those as historical workspace names, not the product name.

## Current Status

Current project status is tracked in [docs/PROJECT_STATUS.md](docs/PROJECT_STATUS.md).

Latest release-trust and priority-core cleanup baseline recorded on 2026-06-24:

- `cargo fmt --check`
- `git diff --check`
- `cargo check -q`
- `cargo check --features legacy-cli -q`
- `cargo check --features experimental-api-server -q`
- `cargo doc --workspace --all-features --no-deps`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-features -- --test-threads=1`
- `bash scripts/validate_docs.sh`

Full release readiness is tracked in
[docs/FRIEND_REVIEW_CODE_QUALITY_IMPROVEMENT_PLAN_2026-06-24.md](docs/FRIEND_REVIEW_CODE_QUALITY_IMPROVEMENT_PLAN_2026-06-24.md)
and
[docs/NEXT_PRIORITY_CORE_WEIGHT_REFINEMENT_PLAN_2026-06-24.md](docs/NEXT_PRIORITY_CORE_WEIGHT_REFINEMENT_PLAN_2026-06-24.md).
Before publishing a formal release, rerun the full gate set from
`QUALITY_GATES.md` on the exact release commit.

## Quick Start

```bash
git clone https://github.com/georgexu521/rust-gene.git
cd rust-gene

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
cd rust-gene
make install
```

## API Keys

Priority Agent chooses configured providers in this order unless
`PRIORITY_AGENT_DEFAULT_PROVIDER` is set:

```bash
export MINIMAX_API_KEY="..."
export MINIMAX_MODEL="MiniMax-M3"     # optional, default is M3

export KIMI_CODE_API_KEY="..."
export KIMI_CODE_MODEL="kimi-for-coding" # optional

export DEEPSEEK_API_KEY="..."
export DEEPSEEK_MODEL="deepseek-v4-pro"  # optional

export GLM_API_KEY="..."              # or ZAI_API_KEY / ZHIPUAI_API_KEY / BIGMODEL_API_KEY
export GLM_MODEL="glm-5.1"            # optional

export MOONSHOT_API_KEY="..."
export MOONSHOT_MODEL="kimi-k2.5"     # optional

export OPENAI_API_KEY="..."
export OPENAI_MODEL="gpt-4o"          # optional fallback

export PRIORITY_AGENT_DEFAULT_PROVIDER="minimax" # optional override
```

Default order: MiniMax, Kimi Code, DeepSeek, GLM/Z.AI, Moonshot/Kimi, OpenAI.

Keys saved through `/connect` are written to `~/.priority-agent/.env` as a
plaintext dotenv file. On Unix-like systems Priority Agent sets file permissions
to `0600`, but it does not currently use macOS Keychain, Secret Service, or
Windows Credential Manager. Do not save production or shared-machine secrets
with `/connect`.

## Platform Support

The current release target is macOS and Linux. Windows support is best-effort
until the installer, shell execution defaults, and credential-storage paths are
implemented and validated on Windows.

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
| `/trace` | Inspect the latest turn timeline, including workflow learning adjustments |
| `/goal` | Show or pin the active goal |
| `/goal drift` | Show recent goal drift events |
| `/recover` | Show recent recovery plans |
| `/learn` | Show recent learning events |
| `/improvements` | Scan, review, accept, reject, or apply controlled self-evolution proposals |
| `/skill-proposals` | Turn repeated successful workflows into reviewed, opt-in skill candidates |
| `/memory` | Show memory namespaces; supports `search`, `conflicts`, `review`, `explain`, and `doctor` |
| `/skills` | List bundled coding skills |
| `/karpathy <task>` | Apply careful coding guidelines to a task |
| `/permissions` | Inspect or edit permission rules |
| `/mcp status` | Show MCP server health and approvals |
| `/sessions` | List persisted conversations |
| `/resume` | Pick or search a prior conversation and continue it |
| `/cost` | Inspect token and cost usage |

## Architecture

Priority Agent is organized around a small product runtime:

- `engine` owns conversation flow, routing, traces, workflow state, and tool
  execution control.
- `tools`, `permissions`, `memory`, and `session_store` provide local execution,
  policy, retrieval, and persistence boundaries.
- `lab` adds the professor/postdoc/graduate LabRun orchestration layer for
  scoped implementation, validation, evidence, and review.
- `services/api`, `api`, and `platform` expose provider and integration
  surfaces without changing the terminal CLI as the main product path.

The canonical source layout and module-boundary map lives in
[docs/PROJECT_MAP.md](docs/PROJECT_MAP.md). Keep detailed architecture changes
there so this README remains a stable product entrypoint.

Core runtime flow:

```text
User prompt
  -> IntentRouter
  -> TurnTrace
  -> SessionGoal / goal drift checks
  -> Retrieval and memory prefetch
  -> Tool execution with action review, permissions, and LabRun policy overlay
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

## Product Boundaries

- Active memory is opt-in, local, bounded, and read-only retrieval context. It
  does not call an LLM, write memory, invoke tools, or act as a background
  planning agent.
- Skill evolution proposes reviewed candidates; candidates are not trusted or
  active until they pass gates and are explicitly applied.
- Subagents are scoped workers with profile/tool boundaries. Their claims are
  evidence inputs, not verified closeout proof unless the parent runtime
  verifies them.
- `verified` closeout means runtime evidence exists. `partial`, `failed`, and
  `not_verified` are valid honest outcomes when proof is incomplete or blocked.

## Development

```bash
cargo fmt
cargo check --quiet
cargo test --quiet
env PRIORITY_AGENT_WORKFLOW_ENABLED=1 cargo test --quiet -- --test-threads=1
cargo clippy --all-targets --all-features -- -D warnings
```

Some tests mutate process environment variables. Use `--test-threads=1` for the
full workflow-enabled suite to avoid cross-test environment interference.

## Documentation Map

- [docs/PROJECT_STATUS.md](docs/PROJECT_STATUS.md): current state and remaining
  priorities.
- [docs/PROJECT_MAP.md](docs/PROJECT_MAP.md): compact runtime and code
  navigation map.
- [docs/README.md](docs/README.md): docs index and reading order.
- [AGENTS.md](AGENTS.md): prompt-injected project runtime guidance.
- [CLAUDE.md](CLAUDE.md): compact Claude Code compatibility orientation.
- [QUICKSTART.md](QUICKSTART.md): setup-oriented guide.

## License

MIT
