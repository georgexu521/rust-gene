# Quickstart

Last updated: 2026-04-25

## Prerequisites

```bash
rustc --version
cargo --version
```

Install Rust with rustup if needed:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## Build

```bash
cd ~/Desktop/rust-agent
cargo build
cargo build --release
```

## Configure Provider

Set at least one provider key:

```bash
# MiniMax
export MINIMAX_API_KEY="..."
export MINIMAX_MODEL="MiniMax-M2.7"

# OpenAI
export OPENAI_API_KEY="..."
export OPENAI_MODEL="gpt-4o"

# Kimi / Moonshot
export MOONSHOT_API_KEY="..."
export MOONSHOT_MODEL="kimi-k2.5"
```

Priority order is MiniMax, then OpenAI, then Moonshot/Kimi.

## Run

```bash
# Development build
cargo run -- --cli

# Release binary
./target/release/priority-agent
./target/release/priority-agent --cli

# Deprecated compatibility alias
./target/release/priority-agent --tui
```

Inside the interactive CLI:

```text
/help
/quick
/trace
/goal
/goal drift
/memory
/mcp status
/permissions
/cost
```

## Install

```bash
cd ~/Desktop/rust-agent
make install
```

The install target writes to `~/.local/bin` by default. If `priority-agent` or
`pa` is not found after install, ensure `~/.local/bin` is in `PATH`.

```bash
export PATH="$HOME/.local/bin:$PATH"
```

## HTTP API

The API server is gated behind the experimental feature:

```bash
cargo run --features experimental-api-server -- --api --port 8787
```

## Verify

```bash
cargo fmt --all -- --check
cargo check --quiet
env PRIORITY_AGENT_WORKFLOW_ENABLED=1 cargo test --quiet -- --test-threads=1
```

The single-threaded test run avoids environment-variable interference between
workflow tests.

## Troubleshooting

### `make: *** No rule to make target 'install'. Stop.`

You are not in the repository root.

```bash
cd ~/Desktop/rust-agent
make install
```

### API key errors

Check the provider key:

```bash
echo "$MINIMAX_API_KEY"
echo "$OPENAI_API_KEY"
echo "$MOONSHOT_API_KEY"
```

### Terminal rendering issues

Use a UTF-8 terminal:

```bash
export LANG=en_US.UTF-8
```

### Reset local app data

Most project data lives under `~/.priority-agent` and application data dirs.
Back up anything important before deleting.

```bash
rm -rf ~/.priority-agent
rm -rf ~/.local/share/priority-agent
```

## More Docs

- `README.md`
- `docs/PROJECT_STATUS.md`
- `docs/CLAUDE_CODE_ALIGNMENT_PLAN.md`
- `docs/REMAINING_CLOSURE_PLAN.md`
- `AGENTS.md`
