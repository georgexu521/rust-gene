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
SMOKE_MODE="fixture"
PROVIDER_OVERRIDE=""
MODEL_OVERRIDE=""
RESTART_CHECK=false
MULTI_TOOL_CHECK=false
SOAK_CHECK=false
EXTENDED_SOAK_CHECK=false
LAB_RECOVERY_CHECK=false

usage() {
  cat <<'USAGE'
Usage: scripts/desktop-native-smoke.sh [options]

Launch the built macOS Tauri app, verify the native process stays alive, capture
a screenshot artifact, and then stop the app.

Options:
  --skip-build       Reuse the existing .app bundle instead of building it.
  --no-screenshot   Skip screencapture and only verify launch/process health.
  --live-provider   Run a small real-provider desktop request instead of the fixture.
  --provider id     Select provider for --live-provider, e.g. minimax or deepseek.
  --model id        Select model for --live-provider. Defaults to provider config.
  --multi-tool-check
                   Run a real provider tool/file-edit smoke in an isolated project.
  --soak-check     Run two real provider tool/file-edit turns in one desktop session.
  --extended-soak-check
                   Run three real provider tool/file-edit turns in one desktop session.
  --lab-recovery-check
                   Prepare a file-backed paused LabRun and verify desktop recovery/report UI.
  --restart-check   After a live-provider or LabRun recovery run, restart the app and verify recovery.
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
    --live-provider)
      SMOKE_MODE="live-provider"
      if [[ "$TIMEOUT_SECONDS" == "20" ]]; then
        TIMEOUT_SECONDS=180
      fi
      shift
      ;;
    --provider)
      PROVIDER_OVERRIDE="${2:-}"
      if [[ -z "$PROVIDER_OVERRIDE" ]]; then
        echo "--provider requires a provider id" >&2
        exit 2
      fi
      shift 2
      ;;
    --model)
      MODEL_OVERRIDE="${2:-}"
      if [[ -z "$MODEL_OVERRIDE" ]]; then
        echo "--model requires a model id" >&2
        exit 2
      fi
      shift 2
      ;;
    --restart-check)
      RESTART_CHECK=true
      shift
      ;;
    --multi-tool-check)
      MULTI_TOOL_CHECK=true
      SMOKE_MODE="live-provider"
      if [[ "$TIMEOUT_SECONDS" == "20" ]]; then
        TIMEOUT_SECONDS=240
      fi
      shift
      ;;
    --soak-check)
      SOAK_CHECK=true
      SMOKE_MODE="live-provider"
      if [[ "$TIMEOUT_SECONDS" == "20" ]]; then
        TIMEOUT_SECONDS=360
      fi
      shift
      ;;
    --extended-soak-check)
      SOAK_CHECK=true
      EXTENDED_SOAK_CHECK=true
      SMOKE_MODE="live-provider"
      if [[ "$TIMEOUT_SECONDS" == "20" ]]; then
        TIMEOUT_SECONDS=540
      fi
      shift
      ;;
    --lab-recovery-check)
      LAB_RECOVERY_CHECK=true
      SMOKE_MODE="lab-recovery"
      if [[ "$TIMEOUT_SECONDS" == "20" ]]; then
        TIMEOUT_SECONDS=90
      fi
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

if [[ "$RESTART_CHECK" == true && "$SMOKE_MODE" != "live-provider" && "$LAB_RECOVERY_CHECK" != true ]]; then
  echo "--restart-check requires --live-provider or --lab-recovery-check" >&2
  exit 2
fi
if [[ "$MULTI_TOOL_CHECK" == true && "$RESTART_CHECK" == true ]]; then
  echo "--multi-tool-check and --restart-check are separate smoke modes" >&2
  exit 2
fi
if [[ "$SOAK_CHECK" == true && "$MULTI_TOOL_CHECK" == true ]]; then
  echo "--soak-check and --multi-tool-check are separate smoke modes" >&2
  exit 2
fi
if [[ "$LAB_RECOVERY_CHECK" == true && ( "$MULTI_TOOL_CHECK" == true || "$SOAK_CHECK" == true ) ]]; then
  echo "--lab-recovery-check is separate from --multi-tool-check and --soak-check" >&2
  exit 2
fi
if [[ "$LAB_RECOVERY_CHECK" == true && ( -n "$PROVIDER_OVERRIDE" || -n "$MODEL_OVERRIDE" ) ]]; then
  echo "--lab-recovery-check does not use provider/model overrides" >&2
  exit 2
fi

if ! command -v corepack >/dev/null 2>&1; then
  echo "corepack is required to build the desktop app" >&2
  exit 1
fi

if [[ "$SMOKE_MODE" == "live-provider" ]]; then
  HAS_PROVIDER_KEY=false
  PROVIDER_KEY_NAMES=(MINIMAX_API_KEY KIMI_CODE_API_KEY DEEPSEEK_API_KEY GLM_API_KEY ZAI_API_KEY ZHIPUAI_API_KEY BIGMODEL_API_KEY MOONSHOT_API_KEY OPENAI_API_KEY)
  case "$PROVIDER_OVERRIDE" in
    minimax) PROVIDER_KEY_NAMES=(MINIMAX_API_KEY) ;;
    kimi) PROVIDER_KEY_NAMES=(KIMI_CODE_API_KEY MOONSHOT_API_KEY) ;;
    deepseek) PROVIDER_KEY_NAMES=(DEEPSEEK_API_KEY) ;;
    glm|zai|zhipu|zhipuai) PROVIDER_KEY_NAMES=(GLM_API_KEY ZAI_API_KEY ZHIPUAI_API_KEY BIGMODEL_API_KEY) ;;
    openai) PROVIDER_KEY_NAMES=(OPENAI_API_KEY) ;;
    "") ;;
    *)
      echo "--provider is not recognized by the smoke script: $PROVIDER_OVERRIDE" >&2
      exit 2
      ;;
  esac
  for key in "${PROVIDER_KEY_NAMES[@]}"; do
    if [[ -n "${!key:-}" ]]; then
      HAS_PROVIDER_KEY=true
      break
    fi
  done
  if [[ "$HAS_PROVIDER_KEY" != true ]]; then
    echo "--live-provider requires at least one configured provider API key" >&2
    exit 1
  fi
fi

if [[ "$BUILD_APP" == true ]]; then
  echo "==> Building macOS .app bundle"
  pnpm --dir "$DESKTOP_DIR" tauri build --bundles app
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

if pgrep -x "priority-agent-desktop" >/dev/null 2>&1; then
  echo "==> Stopping existing Priority Agent processes"
  pkill -x "priority-agent-desktop" >/dev/null 2>&1 || true
  for _ in 1 2 3 4 5; do
    if ! pgrep -x "priority-agent-desktop" >/dev/null 2>&1; then
      break
    fi
    sleep 1
  done
fi

mkdir -p "$ARTIFACT_DIR"
SMOKE_HOME="$(mktemp -d "${TMPDIR:-/tmp}/priority-agent-native-smoke-home.XXXXXX")"
SMOKE_PROJECT=""
if [[ "$SMOKE_MODE" == "live-provider" ]]; then
  ARTIFACT_PROVIDER_SUFFIX=""
  if [[ -n "$PROVIDER_OVERRIDE" ]]; then
    ARTIFACT_PROVIDER_SUFFIX="-${PROVIDER_OVERRIDE//[^[:alnum:]_-]/-}"
  fi
  ARTIFACT_MODE_LABEL="live-provider"
  SUCCESS_PATTERN="native_live_provider_smoke ok=true"
  FAILURE_PATTERN="native_live_provider_smoke ok=false"
  if [[ "$MULTI_TOOL_CHECK" == true || "$SOAK_CHECK" == true ]]; then
    ARTIFACT_MODE_LABEL="multitool"
    if [[ "$SOAK_CHECK" == true ]]; then
      ARTIFACT_MODE_LABEL="soak"
      SUCCESS_PATTERN="native_soak_smoke ok=true"
      FAILURE_PATTERN="native_soak_smoke ok=false"
      if [[ "$EXTENDED_SOAK_CHECK" == true ]]; then
        ARTIFACT_MODE_LABEL="extended-soak"
        SUCCESS_PATTERN="native_extended_soak_smoke ok=true"
        FAILURE_PATTERN="native_extended_soak_smoke ok=false"
      fi
    else
      SUCCESS_PATTERN="native_multitool_smoke ok=true"
      FAILURE_PATTERN="native_multitool_smoke ok=false"
    fi
    SMOKE_PROJECT="$(mktemp -d "${TMPDIR:-/tmp}/priority-agent-${ARTIFACT_MODE_LABEL}-project.XXXXXX")"
    printf 'initial\n' >"$SMOKE_PROJECT/qa_target.txt"
    if [[ "$SOAK_CHECK" == true ]]; then
      printf 'pending\n' >"$SMOKE_PROJECT/qa_followup.txt"
      if [[ "$EXTENDED_SOAK_CHECK" == true ]]; then
        printf 'waiting\n' >"$SMOKE_PROJECT/qa_third.txt"
      fi
    fi
    cat >"$SMOKE_PROJECT/AGENTS.md" <<'EOF'
# Native Multi-Tool Smoke Project

Use tools for file changes and validation. Keep edits inside this directory.

For QA tasks in this project, success requires runtime evidence:

- Read the target file before changing it.
- Write the requested exact content with a file tool.
- Run the requested validation command.
- Do not claim completion from text-only reasoning.
EOF
  fi
  LOG_PATH="$ARTIFACT_DIR/native-${ARTIFACT_MODE_LABEL}${ARTIFACT_PROVIDER_SUFFIX}-smoke.log"
  APP_LOG_ARTIFACT_PATH="$ARTIFACT_DIR/native-${ARTIFACT_MODE_LABEL}${ARTIFACT_PROVIDER_SUFFIX}-app-desktop.log"
  SCREENSHOT_PATH="$ARTIFACT_DIR/native-${ARTIFACT_MODE_LABEL}${ARTIFACT_PROVIDER_SUFFIX}-smoke.png"
  RESTART_LOG_PATH="$ARTIFACT_DIR/native-live-provider${ARTIFACT_PROVIDER_SUFFIX}-restart-smoke.log"
  RESTART_APP_LOG_ARTIFACT_PATH="$ARTIFACT_DIR/native-live-provider${ARTIFACT_PROVIDER_SUFFIX}-restart-app-desktop.log"
  if [[ "$SOAK_CHECK" == true ]]; then
    RESTART_LOG_PATH="$ARTIFACT_DIR/native-${ARTIFACT_MODE_LABEL}${ARTIFACT_PROVIDER_SUFFIX}-restart-smoke.log"
    RESTART_APP_LOG_ARTIFACT_PATH="$ARTIFACT_DIR/native-${ARTIFACT_MODE_LABEL}${ARTIFACT_PROVIDER_SUFFIX}-restart-app-desktop.log"
  fi
elif [[ "$LAB_RECOVERY_CHECK" == true ]]; then
  SUCCESS_PATTERN="native_lab_recovery_smoke ok=true"
  FAILURE_PATTERN="native_lab_recovery_smoke ok=false"
  SMOKE_PROJECT="$(mktemp -d "${TMPDIR:-/tmp}/priority-agent-lab-recovery-project.XXXXXX")"
  cat >"$SMOKE_PROJECT/AGENTS.md" <<'EOF'
# Native LabRun Recovery Smoke Project

This project is used only for desktop LabRun recovery/report UI validation.
EOF
  LOG_PATH="$ARTIFACT_DIR/native-lab-recovery-smoke.log"
  APP_LOG_ARTIFACT_PATH="$ARTIFACT_DIR/native-lab-recovery-app-desktop.log"
  SCREENSHOT_PATH="$ARTIFACT_DIR/native-lab-recovery-smoke.png"
  RESTART_LOG_PATH="$ARTIFACT_DIR/native-lab-recovery-restart-smoke.log"
  RESTART_APP_LOG_ARTIFACT_PATH="$ARTIFACT_DIR/native-lab-recovery-restart-app-desktop.log"
else
  LOG_PATH="$ARTIFACT_DIR/native-smoke.log"
  APP_LOG_ARTIFACT_PATH="$ARTIFACT_DIR/native-app-desktop.log"
  SCREENSHOT_PATH="$ARTIFACT_DIR/native-smoke.png"
  SUCCESS_PATTERN="native_interaction_smoke ok=true"
  FAILURE_PATTERN="native_interaction_smoke ok=false"
fi
APP_DATA_LOG_PATH="$SMOKE_HOME/Library/Application Support/com.priorityagent.desktop/logs/desktop.log"

APP_PID=""
SMOKE_APP_PID=""
append_smoke_summary() {
  local summary_path="$1"
  local app_log_path="$2"
  local title="$3"

  {
    echo "==> $title"
    echo "mode: $SMOKE_MODE"
    echo "project: $PROJECT_DIR"
    echo "app log: $app_log_path"
    if [[ -s "$app_log_path" ]]; then
      grep -E \
        "desktop_start|native_.*_smoke ok=true|native_.*_smoke ok=false|native_lab_recovery_project prepared=true|stream_event closeout status=verified" \
        "$app_log_path" || true
    else
      echo "app log missing or empty"
    fi
  } >>"$summary_path"
}
cleanup() {
  local status=$?
  if [[ -n "$SMOKE_APP_PID" ]] && [[ "$SMOKE_APP_PID" != "$APP_PID" ]] && kill -0 "$SMOKE_APP_PID" >/dev/null 2>&1; then
    kill "$SMOKE_APP_PID" >/dev/null 2>&1 || true
    wait "$SMOKE_APP_PID" >/dev/null 2>&1 || true
  fi
  if [[ -n "$APP_PID" ]] && kill -0 "$APP_PID" >/dev/null 2>&1; then
    kill "$APP_PID" >/dev/null 2>&1 || true
    wait "$APP_PID" >/dev/null 2>&1 || true
  fi
  if [[ "$KEEP_ARTIFACT_HOME" != true && "$status" -eq 0 ]]; then
    rm -rf "$SMOKE_HOME"
    if [[ -n "$SMOKE_PROJECT" ]]; then
      rm -rf "$SMOKE_PROJECT"
    fi
  else
    echo "kept native smoke HOME at $SMOKE_HOME"
    if [[ -n "$SMOKE_PROJECT" ]]; then
      echo "kept native smoke project at $SMOKE_PROJECT"
    fi
  fi
  return "$status"
}
trap cleanup EXIT

echo "==> Launching native app"
PROJECT_DIR="$ROOT_DIR"
if [[ -n "$SMOKE_PROJECT" ]]; then
  PROJECT_DIR="$SMOKE_PROJECT"
fi
APP_ENV=(
  "HOME=$SMOKE_HOME"
  "PRIORITY_AGENT_DESKTOP_PROJECT_DIR=$PROJECT_DIR"
)
if [[ "$SMOKE_MODE" == "live-provider" ]]; then
  if [[ "$MULTI_TOOL_CHECK" == true ]]; then
    APP_ENV+=("PRIORITY_AGENT_DESKTOP_MULTI_TOOL_SMOKE=1")
    APP_ENV+=("PRIORITY_AGENT_DESKTOP_SMOKE_AGENT_MODE=build")
  elif [[ "$SOAK_CHECK" == true ]]; then
    if [[ "$EXTENDED_SOAK_CHECK" == true ]]; then
      APP_ENV+=("PRIORITY_AGENT_DESKTOP_EXTENDED_SOAK_SMOKE=1")
    else
      APP_ENV+=("PRIORITY_AGENT_DESKTOP_SOAK_SMOKE=1")
    fi
    APP_ENV+=("PRIORITY_AGENT_DESKTOP_SMOKE_AGENT_MODE=build")
  else
    APP_ENV+=("PRIORITY_AGENT_DESKTOP_LIVE_PROVIDER_SMOKE=1")
  fi
  if [[ -n "$PROVIDER_OVERRIDE" ]]; then
    APP_ENV+=("PRIORITY_AGENT_DESKTOP_SMOKE_PROVIDER=$PROVIDER_OVERRIDE")
  fi
  if [[ -n "$MODEL_OVERRIDE" ]]; then
    APP_ENV+=("PRIORITY_AGENT_DESKTOP_SMOKE_MODEL=$MODEL_OVERRIDE")
  fi
else
  if [[ "$LAB_RECOVERY_CHECK" == true ]]; then
    APP_ENV+=("PRIORITY_AGENT_DESKTOP_LAB_RECOVERY_SMOKE=1")
  else
    APP_ENV+=("PRIORITY_AGENT_DESKTOP_NATIVE_SMOKE=1")
  fi
fi
(
  cd "$ROOT_DIR"
  env "${APP_ENV[@]}" "$APP_EXECUTABLE"
) >"$LOG_PATH" 2>&1 &
APP_PID="$!"
SMOKE_APP_PID="$APP_PID"

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
CHILD_APP_PID="$(pgrep -P "$APP_PID" -x "priority-agent-desktop" | sort | head -n 1 || true)"
if [[ -n "$CHILD_APP_PID" ]]; then
  SMOKE_APP_PID="$CHILD_APP_PID"
fi

if command -v osascript >/dev/null 2>&1; then
  echo "==> Activating native app"
  for _ in 1 2 3 4 5; do
    if osascript -e "tell application \"System Events\" to set frontmost of first application process whose unix id is $SMOKE_APP_PID to true" >/dev/null 2>&1; then
      sleep 1
      break
    fi
    sleep 1
  done

  ACTIVE_APP_PID="$(
    osascript -e 'tell application "System Events" to unix id of first application process whose frontmost is true' 2>/dev/null || true
  )"
  ACTIVE_APP="$(
    osascript -e 'tell application "System Events" to name of first application process whose frontmost is true' 2>/dev/null || true
  )"
  if [[ "$ACTIVE_APP_PID" != "$SMOKE_APP_PID" ]]; then
    echo "expected smoke app pid $SMOKE_APP_PID to be frontmost, got ${ACTIVE_APP:-unknown} pid ${ACTIVE_APP_PID:-unknown}" >&2
    exit 1
  fi
fi

if [[ ! -s "$APP_DATA_LOG_PATH" ]]; then
  echo "expected app diagnostic log was not created: $APP_DATA_LOG_PATH" >&2
  exit 1
fi
if ! grep -q "desktop_start" "$APP_DATA_LOG_PATH"; then
  echo "app diagnostic log does not include desktop_start: $APP_DATA_LOG_PATH" >&2
  exit 1
fi
deadline=$((SECONDS + TIMEOUT_SECONDS))
while ! grep -q "$SUCCESS_PATTERN" "$APP_DATA_LOG_PATH" 2>/dev/null; do
  if grep -q "$FAILURE_PATTERN" "$APP_DATA_LOG_PATH" 2>/dev/null; then
    echo "native interaction smoke failed" >&2
    cp "$APP_DATA_LOG_PATH" "$APP_LOG_ARTIFACT_PATH" 2>/dev/null || true
    tail -n 80 "$APP_DATA_LOG_PATH" >&2 || true
    exit 1
  fi
  if [[ "$SMOKE_MODE" == "live-provider" ]] \
    && grep -q "stream_event tool_call_start .* name=ask_user" "$APP_DATA_LOG_PATH" 2>/dev/null; then
    echo "native live-provider smoke asked for user input during unattended validation" >&2
    cp "$APP_DATA_LOG_PATH" "$APP_LOG_ARTIFACT_PATH" 2>/dev/null || true
    tail -n 120 "$APP_DATA_LOG_PATH" >&2 || true
    exit 1
  fi
  if [[ "$SECONDS" -ge "$deadline" ]]; then
    echo "timed out waiting for native interaction smoke" >&2
    cp "$APP_DATA_LOG_PATH" "$APP_LOG_ARTIFACT_PATH" 2>/dev/null || true
    tail -n 80 "$APP_DATA_LOG_PATH" >&2 || true
    exit 1
  fi
  sleep 1
done
cp "$APP_DATA_LOG_PATH" "$APP_LOG_ARTIFACT_PATH"

if [[ "$LAB_RECOVERY_CHECK" == true ]]; then
  if ! grep -q "native_lab_recovery_project prepared=true" "$APP_DATA_LOG_PATH"; then
    echo "native LabRun recovery smoke did not prepare a LabRun project" >&2
    tail -n 120 "$APP_DATA_LOG_PATH" >&2 || true
    exit 1
  fi
  LAB_REPORT_COUNT="$(find "$SMOKE_PROJECT/.priority-agent/lab/runs" -path "*/reports/*.md" -type f 2>/dev/null | wc -l | tr -d '[:space:]')"
  if [[ "${LAB_REPORT_COUNT:-0}" -lt 2 ]]; then
    echo "native LabRun recovery smoke expected at least two report markdown files" >&2
    find "$SMOKE_PROJECT/.priority-agent/lab" -maxdepth 5 -type f 2>/dev/null >&2 || true
    exit 1
  fi
  if ! grep -R "Desktop LabRun recovery smoke" "$SMOKE_PROJECT/.priority-agent/lab/runs" >/dev/null 2>&1; then
    echo "native LabRun recovery smoke reports do not include the expected recovery topic" >&2
    find "$SMOKE_PROJECT/.priority-agent/lab/runs" -path "*/reports/*.md" -type f -print >&2 || true
    exit 1
  fi
  cp "$APP_DATA_LOG_PATH" "$APP_LOG_ARTIFACT_PATH"
fi

if [[ "$LAB_RECOVERY_CHECK" == true && "$RESTART_CHECK" == true ]]; then
  echo "==> Restarting native app for LabRun recovery check"
  INITIAL_LAB_RECOVERY_OK_COUNT="$(grep -c "native_lab_recovery_smoke ok=true" "$APP_DATA_LOG_PATH" 2>/dev/null || true)"
  if [[ -n "$SMOKE_APP_PID" ]] && [[ "$SMOKE_APP_PID" != "$APP_PID" ]] && kill -0 "$SMOKE_APP_PID" >/dev/null 2>&1; then
    kill "$SMOKE_APP_PID" >/dev/null 2>&1 || true
    wait "$SMOKE_APP_PID" >/dev/null 2>&1 || true
  fi
  if [[ -n "$APP_PID" ]] && kill -0 "$APP_PID" >/dev/null 2>&1; then
    kill "$APP_PID" >/dev/null 2>&1 || true
    wait "$APP_PID" >/dev/null 2>&1 || true
  fi

  (
    cd "$ROOT_DIR"
    env \
      "HOME=$SMOKE_HOME" \
      "PRIORITY_AGENT_DESKTOP_PROJECT_DIR=$PROJECT_DIR" \
      "PRIORITY_AGENT_DESKTOP_LAB_RECOVERY_RESTART_SMOKE=1" \
      "$APP_EXECUTABLE"
  ) >"$RESTART_LOG_PATH" 2>&1 &
  APP_PID="$!"
  SMOKE_APP_PID="$APP_PID"

  deadline=$((SECONDS + TIMEOUT_SECONDS))
  while true; do
    if ! kill -0 "$APP_PID" >/dev/null 2>&1; then
      echo "native app exited before LabRun recovery restart smoke completed" >&2
      tail -n 80 "$RESTART_LOG_PATH" >&2 || true
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
  CHILD_APP_PID="$(pgrep -P "$APP_PID" -x "priority-agent-desktop" | sort | head -n 1 || true)"
  if [[ -n "$CHILD_APP_PID" ]]; then
    SMOKE_APP_PID="$CHILD_APP_PID"
  fi

  if command -v osascript >/dev/null 2>&1; then
    echo "==> Activating restarted native app"
    for _ in 1 2 3 4 5; do
      if osascript -e "tell application \"System Events\" to set frontmost of first application process whose unix id is $SMOKE_APP_PID to true" >/dev/null 2>&1; then
        sleep 1
        break
      fi
      sleep 1
    done
  fi

  deadline=$((SECONDS + TIMEOUT_SECONDS))
  while true; do
    LAB_RECOVERY_OK_COUNT="$(grep -c "native_lab_recovery_smoke ok=true" "$APP_DATA_LOG_PATH" 2>/dev/null || true)"
    if [[ "${LAB_RECOVERY_OK_COUNT:-0}" -gt "${INITIAL_LAB_RECOVERY_OK_COUNT:-0}" ]]; then
      break
    fi
    if grep -q "native_lab_recovery_smoke ok=false" "$APP_DATA_LOG_PATH" 2>/dev/null; then
      echo "native LabRun recovery restart smoke failed" >&2
      tail -n 120 "$APP_DATA_LOG_PATH" >&2 || true
      exit 1
    fi
    if [[ "$SECONDS" -ge "$deadline" ]]; then
      echo "timed out waiting for native LabRun recovery restart smoke" >&2
      tail -n 120 "$APP_DATA_LOG_PATH" >&2 || true
      exit 1
    fi
    sleep 1
  done
  cp "$APP_DATA_LOG_PATH" "$RESTART_APP_LOG_ARTIFACT_PATH"
fi

if [[ "$MULTI_TOOL_CHECK" == true ]]; then
  deadline=$((SECONDS + TIMEOUT_SECONDS))
  while true; do
    if grep -q "stream_event tool_execution_start" "$APP_DATA_LOG_PATH" \
      && grep -q "stream_event tool_execution_complete" "$APP_DATA_LOG_PATH" \
      && grep -q "stream_event closeout status=verified" "$APP_DATA_LOG_PATH" \
      && [[ -f "$SMOKE_PROJECT/qa_target.txt" ]] \
      && [[ "$(tr -d '\r' <"$SMOKE_PROJECT/qa_target.txt")" == "desktop tool qa ok" ]]; then
      break
    fi
    if [[ "$SECONDS" -ge "$deadline" ]]; then
      break
    fi
    sleep 1
  done
  if ! grep -q "stream_event tool_execution_start" "$APP_DATA_LOG_PATH"; then
    echo "native multitool smoke did not record tool execution start" >&2
    tail -n 120 "$APP_DATA_LOG_PATH" >&2 || true
    exit 1
  fi
  if ! grep -q "stream_event tool_execution_complete" "$APP_DATA_LOG_PATH"; then
    echo "native multitool smoke did not record tool execution completion" >&2
    tail -n 120 "$APP_DATA_LOG_PATH" >&2 || true
    exit 1
  fi
  if ! grep -q "stream_event closeout status=verified" "$APP_DATA_LOG_PATH"; then
    echo "native multitool smoke did not record verified closeout" >&2
    tail -n 120 "$APP_DATA_LOG_PATH" >&2 || true
    exit 1
  fi
  if [[ ! -f "$SMOKE_PROJECT/qa_target.txt" ]]; then
    echo "native multitool smoke target file is missing" >&2
    exit 1
  fi
  if [[ "$(tr -d '\r' <"$SMOKE_PROJECT/qa_target.txt")" != "desktop tool qa ok" ]]; then
    echo "native multitool smoke target file content mismatch" >&2
    echo "expected: desktop tool qa ok" >&2
    echo "actual:" >&2
    cat "$SMOKE_PROJECT/qa_target.txt" >&2 || true
    exit 1
  fi
  cp "$APP_DATA_LOG_PATH" "$APP_LOG_ARTIFACT_PATH"
fi

if [[ "$SOAK_CHECK" == true ]]; then
  EXPECTED_SOAK_TURNS=2
  if [[ "$EXTENDED_SOAK_CHECK" == true ]]; then
    EXPECTED_SOAK_TURNS=3
  fi
  deadline=$((SECONDS + TIMEOUT_SECONDS))
  while true; do
    TOOL_START_COUNT="$(grep -c "stream_event tool_execution_start" "$APP_DATA_LOG_PATH" 2>/dev/null || true)"
    TOOL_COMPLETE_COUNT="$(grep -c "stream_event tool_execution_complete" "$APP_DATA_LOG_PATH" 2>/dev/null || true)"
    CLOSEOUT_COUNT="$(grep -c "stream_event closeout status=verified" "$APP_DATA_LOG_PATH" 2>/dev/null || true)"
    if [[ "$TOOL_START_COUNT" -ge "$EXPECTED_SOAK_TURNS" ]] \
      && [[ "$TOOL_COMPLETE_COUNT" -ge "$EXPECTED_SOAK_TURNS" ]] \
      && [[ "$CLOSEOUT_COUNT" -ge "$EXPECTED_SOAK_TURNS" ]] \
      && [[ -f "$SMOKE_PROJECT/qa_target.txt" ]] \
      && [[ -f "$SMOKE_PROJECT/qa_followup.txt" ]] \
      && [[ "$(tr -d '\r' <"$SMOKE_PROJECT/qa_target.txt")" == "desktop tool qa ok" ]] \
      && [[ "$(tr -d '\r' <"$SMOKE_PROJECT/qa_followup.txt")" == "desktop soak qa ok" ]] \
      && { [[ "$EXTENDED_SOAK_CHECK" != true ]] || { [[ -f "$SMOKE_PROJECT/qa_third.txt" ]] && [[ "$(tr -d '\r' <"$SMOKE_PROJECT/qa_third.txt")" == "desktop extended soak qa ok" ]]; }; }; then
      break
    fi
    RUN_COMPLETED_COUNT="$(grep -c "run_completed" "$APP_DATA_LOG_PATH" 2>/dev/null || true)"
    if grep -q "$SUCCESS_PATTERN" "$APP_DATA_LOG_PATH" 2>/dev/null \
      && [[ "${RUN_COMPLETED_COUNT:-0}" -ge "$EXPECTED_SOAK_TURNS" ]]; then
      break
    fi
    if [[ "$SECONDS" -ge "$deadline" ]]; then
      break
    fi
    sleep 1
  done
  if [[ "${TOOL_START_COUNT:-0}" -lt "$EXPECTED_SOAK_TURNS" ]]; then
    echo "native soak smoke did not record ${EXPECTED_SOAK_TURNS} tool execution starts" >&2
    tail -n 160 "$APP_DATA_LOG_PATH" >&2 || true
    exit 1
  fi
  if [[ "${TOOL_COMPLETE_COUNT:-0}" -lt "$EXPECTED_SOAK_TURNS" ]]; then
    echo "native soak smoke did not record ${EXPECTED_SOAK_TURNS} tool execution completions" >&2
    tail -n 160 "$APP_DATA_LOG_PATH" >&2 || true
    exit 1
  fi
  if [[ "${CLOSEOUT_COUNT:-0}" -lt "$EXPECTED_SOAK_TURNS" ]]; then
    echo "native soak smoke did not record ${EXPECTED_SOAK_TURNS} verified closeouts" >&2
    tail -n 160 "$APP_DATA_LOG_PATH" >&2 || true
    exit 1
  fi
  if [[ ! -f "$SMOKE_PROJECT/qa_target.txt" || ! -f "$SMOKE_PROJECT/qa_followup.txt" ]]; then
    echo "native soak smoke target files are missing" >&2
    exit 1
  fi
  if [[ "$(tr -d '\r' <"$SMOKE_PROJECT/qa_target.txt")" != "desktop tool qa ok" ]]; then
    echo "native soak smoke first target file content mismatch" >&2
    cat "$SMOKE_PROJECT/qa_target.txt" >&2 || true
    exit 1
  fi
  if [[ "$(tr -d '\r' <"$SMOKE_PROJECT/qa_followup.txt")" != "desktop soak qa ok" ]]; then
    echo "native soak smoke second target file content mismatch" >&2
    cat "$SMOKE_PROJECT/qa_followup.txt" >&2 || true
    exit 1
  fi
  if [[ "$EXTENDED_SOAK_CHECK" == true ]]; then
    if [[ ! -f "$SMOKE_PROJECT/qa_third.txt" ]]; then
      echo "native extended soak smoke third target file is missing" >&2
      exit 1
    fi
    if [[ "$(tr -d '\r' <"$SMOKE_PROJECT/qa_third.txt")" != "desktop extended soak qa ok" ]]; then
      echo "native extended soak smoke third target file content mismatch" >&2
      cat "$SMOKE_PROJECT/qa_third.txt" >&2 || true
      exit 1
    fi
  fi
  cp "$APP_DATA_LOG_PATH" "$APP_LOG_ARTIFACT_PATH"
fi

if [[ "$SOAK_CHECK" == true && "$RESTART_CHECK" == true ]]; then
  echo "==> Restarting native app for soak recovery check"
  if [[ -n "$SMOKE_APP_PID" ]] && [[ "$SMOKE_APP_PID" != "$APP_PID" ]] && kill -0 "$SMOKE_APP_PID" >/dev/null 2>&1; then
    kill "$SMOKE_APP_PID" >/dev/null 2>&1 || true
    wait "$SMOKE_APP_PID" >/dev/null 2>&1 || true
  fi
  if [[ -n "$APP_PID" ]] && kill -0 "$APP_PID" >/dev/null 2>&1; then
    kill "$APP_PID" >/dev/null 2>&1 || true
    wait "$APP_PID" >/dev/null 2>&1 || true
  fi

  (
    cd "$ROOT_DIR"
    env \
      "HOME=$SMOKE_HOME" \
      "PRIORITY_AGENT_DESKTOP_PROJECT_DIR=$PROJECT_DIR" \
      "$([[ "$EXTENDED_SOAK_CHECK" == true ]] && printf 'PRIORITY_AGENT_DESKTOP_EXTENDED_SOAK_RESTART_SMOKE=1' || printf 'PRIORITY_AGENT_DESKTOP_SOAK_RESTART_SMOKE=1')" \
      "$APP_EXECUTABLE"
  ) >"$RESTART_LOG_PATH" 2>&1 &
  APP_PID="$!"
  SMOKE_APP_PID="$APP_PID"

  deadline=$((SECONDS + TIMEOUT_SECONDS))
  while true; do
    if ! kill -0 "$APP_PID" >/dev/null 2>&1; then
      echo "native app exited before soak restart smoke completed" >&2
      tail -n 80 "$RESTART_LOG_PATH" >&2 || true
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
  CHILD_APP_PID="$(pgrep -P "$APP_PID" -x "priority-agent-desktop" | sort | head -n 1 || true)"
  if [[ -n "$CHILD_APP_PID" ]]; then
    SMOKE_APP_PID="$CHILD_APP_PID"
  fi

  if command -v osascript >/dev/null 2>&1; then
    echo "==> Activating restarted native app"
    for _ in 1 2 3 4 5; do
      if osascript -e "tell application \"System Events\" to set frontmost of first application process whose unix id is $SMOKE_APP_PID to true" >/dev/null 2>&1; then
        sleep 1
        break
      fi
      sleep 1
    done
  fi

  deadline=$((SECONDS + TIMEOUT_SECONDS))
  RESTART_SUCCESS_PATTERN="native_soak_restart_smoke ok=true"
  RESTART_FAILURE_PATTERN="native_soak_restart_smoke ok=false"
  if [[ "$EXTENDED_SOAK_CHECK" == true ]]; then
    RESTART_SUCCESS_PATTERN="native_extended_soak_restart_smoke ok=true"
    RESTART_FAILURE_PATTERN="native_extended_soak_restart_smoke ok=false"
  fi
  while ! grep -q "$RESTART_SUCCESS_PATTERN" "$APP_DATA_LOG_PATH" 2>/dev/null; do
    if grep -q "$RESTART_FAILURE_PATTERN" "$APP_DATA_LOG_PATH" 2>/dev/null; then
      echo "native soak restart smoke failed" >&2
      tail -n 140 "$APP_DATA_LOG_PATH" >&2 || true
      exit 1
    fi
    if [[ "$SECONDS" -ge "$deadline" ]]; then
      echo "timed out waiting for native soak restart smoke" >&2
      tail -n 140 "$APP_DATA_LOG_PATH" >&2 || true
      exit 1
    fi
    sleep 1
  done
  cp "$APP_DATA_LOG_PATH" "$RESTART_APP_LOG_ARTIFACT_PATH"
fi

if [[ "$RESTART_CHECK" == true && "$SMOKE_MODE" == "live-provider" && "$SOAK_CHECK" != true ]]; then
  echo "==> Restarting native app for recovery check"
  if [[ -n "$SMOKE_APP_PID" ]] && [[ "$SMOKE_APP_PID" != "$APP_PID" ]] && kill -0 "$SMOKE_APP_PID" >/dev/null 2>&1; then
    kill "$SMOKE_APP_PID" >/dev/null 2>&1 || true
    wait "$SMOKE_APP_PID" >/dev/null 2>&1 || true
  fi
  if [[ -n "$APP_PID" ]] && kill -0 "$APP_PID" >/dev/null 2>&1; then
    kill "$APP_PID" >/dev/null 2>&1 || true
    wait "$APP_PID" >/dev/null 2>&1 || true
  fi

  (
    cd "$ROOT_DIR"
    env \
      "HOME=$SMOKE_HOME" \
      "PRIORITY_AGENT_DESKTOP_PROJECT_DIR=$ROOT_DIR" \
      "PRIORITY_AGENT_DESKTOP_RESTART_SMOKE=1" \
      "$APP_EXECUTABLE"
  ) >"$RESTART_LOG_PATH" 2>&1 &
  APP_PID="$!"
  SMOKE_APP_PID="$APP_PID"

  deadline=$((SECONDS + TIMEOUT_SECONDS))
  while true; do
    if ! kill -0 "$APP_PID" >/dev/null 2>&1; then
      echo "native app exited before restart smoke completed" >&2
      tail -n 80 "$RESTART_LOG_PATH" >&2 || true
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
  CHILD_APP_PID="$(pgrep -P "$APP_PID" -x "priority-agent-desktop" | sort | head -n 1 || true)"
  if [[ -n "$CHILD_APP_PID" ]]; then
    SMOKE_APP_PID="$CHILD_APP_PID"
  fi

  if command -v osascript >/dev/null 2>&1; then
    echo "==> Activating restarted native app"
    for _ in 1 2 3 4 5; do
      if osascript -e "tell application \"System Events\" to set frontmost of first application process whose unix id is $SMOKE_APP_PID to true" >/dev/null 2>&1; then
        sleep 1
        break
      fi
      sleep 1
    done
  fi

  deadline=$((SECONDS + TIMEOUT_SECONDS))
  while ! grep -q "native_restart_smoke ok=true" "$APP_DATA_LOG_PATH" 2>/dev/null; do
    if grep -q "native_restart_smoke ok=false" "$APP_DATA_LOG_PATH" 2>/dev/null; then
      echo "native restart smoke failed" >&2
      tail -n 100 "$APP_DATA_LOG_PATH" >&2 || true
      exit 1
    fi
    if [[ "$SECONDS" -ge "$deadline" ]]; then
      echo "timed out waiting for native restart smoke" >&2
      tail -n 100 "$APP_DATA_LOG_PATH" >&2 || true
      exit 1
    fi
    sleep 1
  done
  cp "$APP_DATA_LOG_PATH" "$RESTART_APP_LOG_ARTIFACT_PATH"
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
  SCREENSHOT_BYTES="$(stat -f%z "$SCREENSHOT_PATH")"
  if [[ "$SCREENSHOT_BYTES" -lt 50000 ]]; then
    echo "native screenshot is unexpectedly small: ${SCREENSHOT_BYTES} bytes" >&2
    exit 1
  fi
  if command -v sips >/dev/null 2>&1; then
    SCREENSHOT_WIDTH="$(sips -g pixelWidth "$SCREENSHOT_PATH" 2>/dev/null | awk '/pixelWidth:/ {print $2}')"
    SCREENSHOT_HEIGHT="$(sips -g pixelHeight "$SCREENSHOT_PATH" 2>/dev/null | awk '/pixelHeight:/ {print $2}')"
    if [[ "${SCREENSHOT_WIDTH:-0}" -lt 800 || "${SCREENSHOT_HEIGHT:-0}" -lt 500 ]]; then
      echo "native screenshot dimensions are unexpectedly small: ${SCREENSHOT_WIDTH:-unknown}x${SCREENSHOT_HEIGHT:-unknown}" >&2
      exit 1
    fi
  fi
fi

append_smoke_summary "$LOG_PATH" "$APP_LOG_ARTIFACT_PATH" "Native desktop smoke summary"
if [[ "$RESTART_CHECK" == true && -n "${RESTART_APP_LOG_ARTIFACT_PATH:-}" && -s "$RESTART_APP_LOG_ARTIFACT_PATH" ]]; then
  append_smoke_summary "$RESTART_LOG_PATH" "$RESTART_APP_LOG_ARTIFACT_PATH" "Native desktop restart smoke summary"
fi

echo "native desktop smoke passed"
echo "log: $LOG_PATH"
echo "app log: $APP_LOG_ARTIFACT_PATH"
if [[ "$CAPTURE_SCREEN" == true ]]; then
  echo "screenshot: $SCREENSHOT_PATH"
fi
