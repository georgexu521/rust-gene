# Quickstart

Last updated: 2026-06-24

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
git clone https://github.com/georgexu521/rust-gene.git
cd rust-gene
cargo build
cargo build --release
```

## Configure Provider

Set at least one provider key:

```bash
# MiniMax
export MINIMAX_API_KEY="..."
export MINIMAX_MODEL="MiniMax-M3"

# Kimi Code
export KIMI_CODE_API_KEY="..."
export KIMI_CODE_MODEL="kimi-for-coding"

# DeepSeek
export DEEPSEEK_API_KEY="..."
export DEEPSEEK_MODEL="deepseek-v4-pro"

# GLM / Z.AI
export GLM_API_KEY="..."   # or ZAI_API_KEY
export GLM_MODEL="glm-5.1"

# Kimi / Moonshot
export MOONSHOT_API_KEY="..."
export MOONSHOT_MODEL="kimi-k2.5"

# OpenAI-compatible fallback
export OPENAI_API_KEY="..."
export OPENAI_MODEL="gpt-4o"
```

Provider order is MiniMax, Kimi Code, DeepSeek, GLM/Z.AI, Moonshot/Kimi, then
OpenAI. Override with `PRIORITY_AGENT_DEFAULT_PROVIDER` when multiple keys are
configured.

Keys saved through `/connect` are stored in `~/.priority-agent/.env` as a
plaintext dotenv file. On Unix-like systems Priority Agent sets file permissions
to `0600`, but it does not currently use macOS Keychain, Secret Service, or
Windows Credential Manager. Prefer environment variables for shared machines or
production credentials.

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
cd rust-gene
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
cargo fmt --check
cargo check --quiet
cargo clippy --all-targets --all-features -- -D warnings
cargo test --quiet
```

Use `--test-threads=1` for workflow-enabled or older broad suites when a slice
mutates process environment variables.

## Desktop App

The desktop app is currently a macOS-first Tauri app. Use the CLI for the
primary macOS/Linux release path; use desktop packaging as a separate release
candidate flow.

```bash
# Development desktop app bound to this project
PRIORITY_AGENT_DESKTOP_PROJECT_DIR="$PWD" corepack pnpm --dir apps/desktop tauri dev

# Frontend build and browser smoke
corepack pnpm --dir apps/desktop build
corepack pnpm --dir apps/desktop test:ui-smoke

# Tauri backend tests
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -- --test-threads=1
```

On first launch, the desktop app guides project selection, provider status,
credential-storage acknowledgement, permission defaults, workspace trust, and
Direct Agent/LabRun mode. Saved desktop keys still use the local dotenv
fallback in this build; prefer environment variables for shared machines or
production credentials.

## Platform Support

Priority Agent currently targets macOS and Linux for the CLI. Windows is
best-effort until installer behavior, shell execution defaults, and credential
storage are tested and documented for Windows. Desktop packaging is macOS-first
until Windows/Linux Tauri packages are implemented and validated.

## Troubleshooting

### `make: *** No rule to make target 'install'. Stop.`

You are not in the repository root.

```bash
cd /path/to/rust-gene
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
- `docs/PROJECT_MAP.md`
- `docs/README.md`
- `AGENTS.md`
- `CLAUDE.md`
