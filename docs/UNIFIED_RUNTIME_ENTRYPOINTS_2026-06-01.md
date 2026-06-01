# Unified Runtime Entrypoints

Status: active guidance as of 2026-06-01.

## Decision

`StreamingQueryEngine` is the canonical full agent runtime. CLI, TUI,
headless dogfood, and desktop full turns must all route through that runtime
instead of maintaining separate tool-loop, retry, validation, or closeout
logic.

The desktop app is a thin entrypoint:

- Tauri commands gather UI context and emit `DesktopRunEvent` values.
- `DesktopRuntime` owns desktop-facing runtime setup and delegates full turns
  to `StreamingQueryEngine`.
- React renders run events and diagnostics; it does not decide agent flow.

## Test Order

Use this order when debugging complex agent behavior:

1. Deterministic Rust tests for the touched runtime modules.
2. `scripts/agent-runtime-dogfood.sh` for one real full-runtime turn without
   building or launching the desktop app.
3. TUI/CLI interactive testing only when human interaction matters.
4. Desktop smoke for bridge/UI behavior.
5. Full Tauri packaging only for release or package-specific failures.

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
