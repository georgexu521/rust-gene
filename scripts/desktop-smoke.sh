#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DESKTOP_DIR="$ROOT_DIR/apps/desktop"
TAURI_MANIFEST="$DESKTOP_DIR/src-tauri/Cargo.toml"
BUILD_APP=false
RUN_NATIVE=false

for arg in "$@"; do
  case "$arg" in
    --bundle)
      BUILD_APP=true
      ;;
    --native)
      RUN_NATIVE=true
      ;;
    --quick)
      BUILD_APP=false
      ;;
    -h|--help)
      cat <<'USAGE'
Usage: scripts/desktop-smoke.sh [--quick|--bundle] [--native]

Runs the macOS desktop app smoke checks.

  --quick   Build frontend and run Rust command/runtime smoke tests.
  --bundle  Also build the local macOS .app bundle.
  --native  Launch the native .app and capture a screenshot artifact.

For complex agent-flow dogfood, run scripts/agent-runtime-dogfood.sh first.
Desktop smoke is intentionally limited to frontend, bridge, and package checks.
USAGE
      exit 0
      ;;
    *)
      echo "unknown argument: $arg" >&2
      exit 2
      ;;
  esac
done

if ! command -v pnpm >/dev/null 2>&1; then
  echo "pnpm is required to run the desktop frontend checks" >&2
  exit 1
fi

echo "==> Installing desktop frontend dependencies"
pnpm --dir "$DESKTOP_DIR" install --frozen-lockfile

echo "==> Building desktop frontend"
pnpm --dir "$DESKTOP_DIR" build

echo "==> Checking Rust formatting"
cargo fmt --check
cargo fmt --manifest-path "$TAURI_MANIFEST" --check

echo "==> Checking root runtime"
cargo check -q
cargo test -q desktop_runtime

echo "==> Checking Tauri command bridge"
cargo check --manifest-path "$TAURI_MANIFEST" -q
cargo test --manifest-path "$TAURI_MANIFEST" -q desktop_smoke

if [[ "$BUILD_APP" == true ]]; then
  echo "==> Building macOS .app bundle"
  pnpm --dir "$DESKTOP_DIR" tauri build --bundles app
  test -d "$DESKTOP_DIR/src-tauri/target/release/bundle/macos/Priority Agent.app"
fi

if [[ "$RUN_NATIVE" == true ]]; then
  if [[ "$BUILD_APP" == true ]]; then
    "$ROOT_DIR/scripts/desktop-native-smoke.sh" --skip-build
  else
    "$ROOT_DIR/scripts/desktop-native-smoke.sh"
  fi
fi

echo "desktop smoke passed"
