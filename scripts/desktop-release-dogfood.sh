#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ARTIFACT_DIR="$ROOT_DIR/apps/desktop/test-artifacts"
BUILD_FIRST=true
NO_SCREENSHOT=true
DEEPSEEK_TIMEOUT=720
MINIMAX_TIMEOUT=720
LAB_TIMEOUT=180
REPEAT_COUNT=1

usage() {
  cat <<'USAGE'
Usage: scripts/desktop-release-dogfood.sh [options]

Run the desktop release dogfood suite against the packaged Tauri app:
  1. DeepSeek three-turn extended soak + restart.
  2. MiniMax three-turn extended soak + restart.
  3. LabRun paused recovery/report/artifact UI + restart.

Options:
  --skip-build       Reuse the existing .app bundle for every step.
  --screenshot      Allow the underlying smoke steps to capture screenshots.
  --timeout seconds Set the timeout for every step.
  --repeat count     Run the full dogfood sequence this many times.
  -h, --help        Show this help.
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --skip-build)
      BUILD_FIRST=false
      shift
      ;;
    --screenshot)
      NO_SCREENSHOT=false
      shift
      ;;
    --timeout)
      value="${2:-}"
      if ! [[ "$value" =~ ^[0-9]+$ ]] || [[ "$value" -lt 1 ]]; then
        echo "--timeout requires a positive integer" >&2
        exit 2
      fi
      DEEPSEEK_TIMEOUT="$value"
      MINIMAX_TIMEOUT="$value"
      LAB_TIMEOUT="$value"
      shift 2
      ;;
    --repeat)
      value="${2:-}"
      if ! [[ "$value" =~ ^[0-9]+$ ]] || [[ "$value" -lt 1 ]]; then
        echo "--repeat requires a positive integer" >&2
        exit 2
      fi
      REPEAT_COUNT="$value"
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
  echo "desktop release dogfood requires macOS" >&2
  exit 1
fi

if [[ -z "${DEEPSEEK_API_KEY:-}" ]]; then
  echo "desktop release dogfood requires DEEPSEEK_API_KEY" >&2
  exit 1
fi
if [[ -z "${MINIMAX_API_KEY:-}" ]]; then
  echo "desktop release dogfood requires MINIMAX_API_KEY" >&2
  exit 1
fi

mkdir -p "$ARTIFACT_DIR"
SUMMARY_LOG="$ARTIFACT_DIR/desktop-release-dogfood.log"
: >"$SUMMARY_LOG"

log() {
  printf '%s %s\n' "$(date -u '+%Y-%m-%dT%H:%M:%SZ')" "$*" | tee -a "$SUMMARY_LOG"
}

run_step() {
  local name="$1"
  shift
  log "START $name"
  log "COMMAND $*"
  set +e
  "$@" 2>&1 | tee -a "$SUMMARY_LOG"
  local status=${PIPESTATUS[0]}
  set -e
  if [[ "$status" -ne 0 ]]; then
    log "FAIL $name status=$status"
    exit "$status"
  fi
  log "PASS $name"
}

smoke_args=()
if [[ "$NO_SCREENSHOT" == true ]]; then
  smoke_args+=(--no-screenshot)
fi

first_build_args=()
if [[ "$BUILD_FIRST" != true ]]; then
  first_build_args+=(--skip-build)
fi

for ((iteration = 1; iteration <= REPEAT_COUNT; iteration += 1)); do
  log "START desktop_release_dogfood_iteration iteration=$iteration/$REPEAT_COUNT"

  iteration_build_args=(--skip-build)
  if [[ "$iteration" -eq 1 ]]; then
    iteration_build_args=("${first_build_args[@]}")
  fi

  run_step \
    "deepseek_extended_soak_restart iteration=$iteration/$REPEAT_COUNT" \
    "$ROOT_DIR/scripts/desktop-native-smoke.sh" \
    ${iteration_build_args[@]+"${iteration_build_args[@]}"} \
    --live-provider \
    --provider deepseek \
    --extended-soak-check \
    --restart-check \
    --timeout "$DEEPSEEK_TIMEOUT" \
    "${smoke_args[@]}"

  run_step \
    "minimax_extended_soak_restart iteration=$iteration/$REPEAT_COUNT" \
    "$ROOT_DIR/scripts/desktop-native-smoke.sh" \
    --skip-build \
    --live-provider \
    --provider minimax \
    --extended-soak-check \
    --restart-check \
    --timeout "$MINIMAX_TIMEOUT" \
    "${smoke_args[@]}"

  run_step \
    "lab_recovery_restart iteration=$iteration/$REPEAT_COUNT" \
    "$ROOT_DIR/scripts/desktop-native-smoke.sh" \
    --skip-build \
    --lab-recovery-check \
    --restart-check \
    --timeout "$LAB_TIMEOUT" \
    "${smoke_args[@]}"

  log "PASS desktop_release_dogfood_iteration iteration=$iteration/$REPEAT_COUNT"
done

log "PASS desktop_release_dogfood repeat=$REPEAT_COUNT"
log "SUMMARY $SUMMARY_LOG"
