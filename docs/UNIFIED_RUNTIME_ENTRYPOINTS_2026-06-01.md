# Unified Runtime Entrypoints

Status: active guidance as of 2026-06-09.

## Decision

`StreamingQueryEngine` is the canonical full agent runtime. CLI, TUI,
headless dogfood, and desktop full turns must all route through that runtime
instead of maintaining separate tool-loop, retry, validation, or closeout
logic.

Persisted chat history restore also belongs to the canonical streaming runtime
boundary. `ConversationLoop` receives the already-prepared turn history and
must not independently reload SQLite session messages, otherwise resumed
sessions can duplicate prior messages in the prompt.

The desktop app is a thin entrypoint:

- Tauri commands gather UI context and emit `DesktopRunEvent` values.
- `DesktopRuntime` owns desktop-facing runtime setup and delegates full turns
  to `StreamingQueryEngine`.
- React renders run events and diagnostics; it does not decide agent flow.

## Test Order

Use this order when debugging complex agent behavior:

1. Deterministic Rust tests for the touched runtime modules.
2. Headless or TUI full-runtime dogfood for one real full-runtime turn without
   building or launching the desktop app.
3. TUI/CLI interactive testing only when human interaction matters.
4. Desktop smoke for bridge/UI behavior.
5. Full Tauri packaging only for release or package-specific failures.

For runtime-loop changes, do not start by rebuilding the desktop app. Desktop
should prove bridge and rendering behavior after the shared runtime is already
stable.

## Commands

Fast complex-runtime dogfood:

```bash
scripts/agent-runtime-dogfood.sh
```

Custom dogfood prompt:

```bash
scripts/agent-runtime-dogfood.sh --prompt-file path/to/prompt.md
```

Desktop bridge smoke:

```bash
scripts/desktop-smoke.sh --quick
```

Packaged desktop verification:

```bash
scripts/desktop-smoke.sh --bundle --native
```

## Boundary

Headless/TUI runtime dogfood can prove tool-loop, retry, validation, closeout,
and model-flow behavior. It cannot prove desktop-specific behavior such as
Tauri command wiring, event delivery to React, packaged-app environment,
window state, or visual rendering. Those stay as small desktop smoke tests.

## Entrypoint Smoke

Use the entrypoint smoke wrapper when the question is whether the real launch
paths still start and route into the shared runtime shell:

```bash
scripts/runtime-entrypoint-smoke.sh --dry-run --all
scripts/runtime-entrypoint-smoke.sh --headless
scripts/runtime-entrypoint-smoke.sh --cli
scripts/runtime-entrypoint-smoke.sh --tui
scripts/runtime-entrypoint-smoke.sh --desktop-quick
scripts/runtime-entrypoint-smoke.sh --desktop-native
```

The CLI and TUI checks run in a pseudo-terminal and verify startup output. The
desktop checks delegate to `scripts/desktop-smoke.sh`; `--desktop-native`
launches the packaged macOS app and captures native artifacts.

## Runtime Diet Boundary

The shared runtime should not infer semantic intent from natural-language model
text such as "I will read X next". If the provider returns no valid tool calls,
the turn is complete unless the response is empty. Loop safety belongs to the
iteration budget, force-summary, and exact duplicate storm guard; tool safety
belongs to tool contracts, permissions, destructive scope, and validation
proof.
