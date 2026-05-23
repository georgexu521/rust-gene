#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DESKTOP_DIR="$ROOT_DIR/apps/desktop"
APP_BUNDLE="$DESKTOP_DIR/src-tauri/target/release/bundle/macos/Priority Agent.app"
BUNDLES="app"
SIGN_MODE="adhoc"
RUN_CHECKS=true

usage() {
  cat <<'USAGE'
Usage: scripts/package-macos-app.sh [options]

Build a local macOS Priority Agent desktop package.

Options:
  --app            Build only the .app bundle. This is the default.
  --dmg            Build both .app and .dmg bundles.
  --sign adhoc     Ad-hoc sign the .app after build. This is the default.
  --sign none      Leave the app unsigned.
  --skip-checks    Skip frontend build and Rust command checks before packaging.
  --preflight      Run release tool/credential preflight before packaging.
  -h, --help       Show this help.
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --app)
      BUNDLES="app"
      shift
      ;;
    --dmg)
      BUNDLES="app,dmg"
      shift
      ;;
    --sign)
      SIGN_MODE="${2:-}"
      if [[ "$SIGN_MODE" != "adhoc" && "$SIGN_MODE" != "none" ]]; then
        echo "--sign must be either 'adhoc' or 'none'" >&2
        exit 2
      fi
      shift 2
      ;;
    --skip-checks)
      RUN_CHECKS=false
      shift
      ;;
    --preflight)
      "$ROOT_DIR/scripts/macos-release-preflight.sh"
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if ! command -v corepack >/dev/null 2>&1; then
  echo "corepack is required to build the desktop frontend" >&2
  exit 1
fi

echo "==> Generating desktop icons"
"$ROOT_DIR/scripts/generate-desktop-icon.py"

if [[ "$RUN_CHECKS" == true ]]; then
  echo "==> Installing frontend dependencies"
  corepack pnpm --dir "$DESKTOP_DIR" install --frozen-lockfile

  echo "==> Building frontend"
  corepack pnpm --dir "$DESKTOP_DIR" build

  echo "==> Checking Tauri command bridge"
  cargo check --manifest-path "$DESKTOP_DIR/src-tauri/Cargo.toml" -q
  cargo test --manifest-path "$DESKTOP_DIR/src-tauri/Cargo.toml" -q desktop_smoke
fi

echo "==> Building macOS bundle: $BUNDLES"
corepack pnpm --dir "$DESKTOP_DIR" tauri build --bundles "$BUNDLES"

if [[ ! -d "$APP_BUNDLE" ]]; then
  echo "expected app bundle was not created: $APP_BUNDLE" >&2
  exit 1
fi

if [[ "$SIGN_MODE" == "adhoc" ]]; then
  if ! command -v codesign >/dev/null 2>&1; then
    echo "codesign is not available; use --sign none to skip signing" >&2
    exit 1
  fi
  echo "==> Ad-hoc signing app bundle"
  codesign --force --deep --sign - "$APP_BUNDLE"
  codesign --verify --deep --strict "$APP_BUNDLE"
fi

echo "packaged $APP_BUNDLE"
