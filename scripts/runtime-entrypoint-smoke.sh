#!/usr/bin/env bash
# Smoke-test the real runtime entrypoints without mixing their signals into the
# product daily eval layer.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

MODE="all"
DRY_RUN=0
TIMEOUT_SECS="${PRIORITY_AGENT_ENTRYPOINT_SMOKE_TIMEOUT_SECS:-10}"
ARTIFACT_DIR="${PRIORITY_AGENT_ENTRYPOINT_SMOKE_ARTIFACT_DIR:-$ROOT_DIR/target/runtime-entrypoint-smoke/$(date +%Y%m%d-%H%M%S)}"

usage() {
  cat <<'EOF'
Usage: scripts/runtime-entrypoint-smoke.sh [options]

Options:
  --headless        Run the noninteractive shared-runtime dogfood.
  --cli            Launch priority-agent --cli in a real pseudo-terminal.
  --tui            Launch priority-agent --tui in a real pseudo-terminal.
  --desktop-quick  Run scripts/desktop-smoke.sh --quick.
  --desktop-native Run scripts/desktop-smoke.sh --bundle --native.
  --all            Run headless, CLI, TUI, and desktop quick checks.
  --dry-run        Print commands without executing.
  --timeout SECS   CLI/TUI launch timeout (default: 10).
  -h, --help       Show this help.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --headless) MODE="headless"; shift ;;
    --cli) MODE="cli"; shift ;;
    --tui) MODE="tui"; shift ;;
    --desktop-quick) MODE="desktop-quick"; shift ;;
    --desktop-native) MODE="desktop-native"; shift ;;
    --all) MODE="all"; shift ;;
    --dry-run) DRY_RUN=1; shift ;;
    --timeout) TIMEOUT_SECS="${2:-10}"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
done

mkdir -p "$ARTIFACT_DIR"

run_or_print() {
  if [[ "$DRY_RUN" -eq 1 ]]; then
    printf 'DRY RUN:'
    printf ' %q' "$@"
    printf '\n'
  else
    "$@"
  fi
}

ensure_expect() {
  command -v expect >/dev/null 2>&1 || {
    echo "expect is required for CLI/TUI pseudo-terminal smoke" >&2
    exit 1
  }
}

build_binary() {
  if [[ "$DRY_RUN" -eq 1 ]]; then
    echo "DRY RUN: cargo build -q"
  else
    cargo build -q
  fi
}

smoke_interactive_mode() {
  local mode="$1"
  local log_file="$ARTIFACT_DIR/${mode}.log"
  local smoke_home="$ARTIFACT_DIR/${mode}-home"
  mkdir -p "$smoke_home"

  ensure_expect
  build_binary

  if [[ "$DRY_RUN" -eq 1 ]]; then
    echo "DRY RUN: expect pseudo-terminal launch for priority-agent --${mode}"
    return 0
  fi

  expect <<EOF
set timeout $TIMEOUT_SECS
log_file -noappend $log_file
spawn env HOME=$smoke_home XDG_CONFIG_HOME=$smoke_home/xdg-config XDG_DATA_HOME=$smoke_home/xdg-data XDG_STATE_HOME=$smoke_home/xdg-state $ROOT_DIR/target/debug/priority-agent --$mode
expect {
  eof {
    set wait_status [wait]
    exit [lindex \$wait_status 3]
  }
  timeout {}
}
send "\003"
after 1000
catch { close }
catch { wait }
exit 0
EOF

  if [[ ! -s "$log_file" ]]; then
    echo "${mode} smoke did not capture terminal output: $log_file" >&2
    exit 1
  fi

  echo "${mode} entrypoint smoke passed"
  echo "log: $log_file"
}

smoke_headless() {
  run_or_print scripts/agent-runtime-dogfood.sh --out-dir "$ARTIFACT_DIR/headless"
}

smoke_desktop_quick() {
  run_or_print scripts/desktop-smoke.sh --quick
}

smoke_desktop_native() {
  run_or_print scripts/desktop-smoke.sh --bundle --native
}

case "$MODE" in
  headless) smoke_headless ;;
  cli) smoke_interactive_mode cli ;;
  tui) smoke_interactive_mode tui ;;
  desktop-quick) smoke_desktop_quick ;;
  desktop-native) smoke_desktop_native ;;
  all)
    smoke_headless
    smoke_interactive_mode cli
    smoke_interactive_mode tui
    smoke_desktop_quick
    ;;
  *) echo "unknown mode: $MODE" >&2; exit 2 ;;
esac

echo "runtime entrypoint smoke complete"
echo "artifacts: $ARTIFACT_DIR"
