#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DESKTOP_DIR="$ROOT_DIR/apps/desktop"
APP_BUNDLE="$DESKTOP_DIR/src-tauri/target/release/bundle/macos/Priority Agent.app"
ARTIFACT_DIR="$DESKTOP_DIR/test-artifacts"
BUILD_APP=true
CAPTURE_SCREEN=true
KEEP_ARTIFACT_HOME=false
TIMEOUT_SECONDS=20

usage() {
  cat <<'USAGE'
Usage: scripts/desktop-native-smoke.sh [options]

Launch the built macOS Tauri app, verify the native process stays alive, capture
a screenshot artifact, and then stop the app.

Options:
  --skip-build       Reuse the existing .app bundle instead of building it.
  --no-screenshot   Skip screencapture and only verify launch/process health.
  --keep-home       Keep the temporary HOME used for isolated app data.
  --timeout seconds Native launch timeout. Default: 20.
  -h, --help        Show this help.
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --)
      shift
      ;;
    --skip-build)
      BUILD_APP=false
      shift
      ;;
    --no-screenshot)
      CAPTURE_SCREEN=false
      shift
      ;;
    --keep-home)
      KEEP_ARTIFACT_HOME=true
      shift
      ;;
    --timeout)
      TIMEOUT_SECONDS="${2:-}"
      if ! [[ "$TIMEOUT_SECONDS" =~ ^[0-9]+$ ]] || [[ "$TIMEOUT_SECONDS" -lt 1 ]]; then
        echo "--timeout requires a positive integer" >&2
        exit 2
      fi
      shift 2
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

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "desktop native smoke requires macOS" >&2
  exit 1
fi

if ! command -v corepack >/dev/null 2>&1; then
  echo "corepack is required to build the desktop app" >&2
  exit 1
fi

if [[ "$BUILD_APP" == true ]]; then
  echo "==> Building macOS .app bundle"
  corepack pnpm --dir "$DESKTOP_DIR" tauri build --bundles app
fi

if [[ ! -d "$APP_BUNDLE" ]]; then
  echo "expected app bundle does not exist: $APP_BUNDLE" >&2
  echo "run without --skip-build or build the app first" >&2
  exit 1
fi

APP_EXECUTABLE="$(
  find "$APP_BUNDLE/Contents/MacOS" -maxdepth 1 -type f -perm +111 | sort | head -n 1
)"
if [[ -z "$APP_EXECUTABLE" ]]; then
  echo "could not find app executable in $APP_BUNDLE/Contents/MacOS" >&2
  exit 1
fi

mkdir -p "$ARTIFACT_DIR"
SMOKE_HOME="$(mktemp -d "${TMPDIR:-/tmp}/priority-agent-native-smoke-home.XXXXXX")"
LOG_PATH="$ARTIFACT_DIR/native-smoke.log"
SCREENSHOT_PATH="$ARTIFACT_DIR/native-smoke.png"

APP_PID=""
cleanup() {
  if [[ -n "$APP_PID" ]] && kill -0 "$APP_PID" >/dev/null 2>&1; then
    kill "$APP_PID" >/dev/null 2>&1 || true
    wait "$APP_PID" >/dev/null 2>&1 || true
  fi
  if [[ "$KEEP_ARTIFACT_HOME" != true ]]; then
    rm -rf "$SMOKE_HOME"
  else
    echo "kept native smoke HOME at $SMOKE_HOME"
  fi
}
trap cleanup EXIT

echo "==> Launching native app"
(
  cd "$ROOT_DIR"
  HOME="$SMOKE_HOME" \
    PRIORITY_AGENT_DESKTOP_PROJECT_DIR="$ROOT_DIR" \
    "$APP_EXECUTABLE"
) >"$LOG_PATH" 2>&1 &
APP_PID="$!"

deadline=$((SECONDS + TIMEOUT_SECONDS))
while true; do
  if ! kill -0 "$APP_PID" >/dev/null 2>&1; then
    echo "native app exited before smoke completed" >&2
    tail -n 80 "$LOG_PATH" >&2 || true
    exit 1
  fi

  if [[ "$SECONDS" -ge "$deadline" ]]; then
    break
  fi

  sleep 1
  if [[ "$SECONDS" -ge 4 ]]; then
    break
  fi
done

if command -v osascript >/dev/null 2>&1; then
  echo "==> Activating native app"
  for _ in 1 2 3 4 5; do
    if osascript -e 'tell application "Priority Agent" to activate' >/dev/null 2>&1; then
      sleep 1
      break
    fi
    sleep 1
  done

  ACTIVE_APP="$(
    osascript -e 'tell application "System Events" to name of first application process whose frontmost is true' 2>/dev/null || true
  )"
  if [[ "$ACTIVE_APP" != "Priority Agent" && "$ACTIVE_APP" != "priority-agent-desktop" ]]; then
    echo "expected Priority Agent to be the frontmost app, got: ${ACTIVE_APP:-unknown}" >&2
    exit 1
  fi
fi

if [[ "$CAPTURE_SCREEN" == true ]]; then
  if ! command -v screencapture >/dev/null 2>&1; then
    echo "screencapture is not available" >&2
    exit 1
  fi

  echo "==> Capturing native screenshot"
  screencapture -x "$SCREENSHOT_PATH"
  if [[ ! -s "$SCREENSHOT_PATH" ]]; then
    echo "native screenshot was not created: $SCREENSHOT_PATH" >&2
    exit 1
  fi
fi

echo "native desktop smoke passed"
echo "log: $LOG_PATH"
if [[ "$CAPTURE_SCREEN" == true ]]; then
  echo "screenshot: $SCREENSHOT_PATH"
fi
