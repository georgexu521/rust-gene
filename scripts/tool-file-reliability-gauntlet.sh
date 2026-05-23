#!/usr/bin/env bash
# Track G deterministic tool/file reliability gauntlet.
#
# This is intentionally local and non-LLM. It validates the replay fixtures and
# targeted gates that prove file edits, bash, permissions, desktop runtime facts,
# and compaction continuity keep their structured evidence.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

MODE="${1:-quick}"

usage() {
  cat <<'EOF'
Usage: scripts/tool-file-reliability-gauntlet.sh [quick|standard|full|help]

Modes:
  quick     Track G deterministic replay fixture plus focused runtime evidence tests.
  standard  quick + cargo check and desktop state/build gates.
  full      standard + clippy, UI smoke, and experimental API check.
EOF
}

run_step() {
  local label="$1"
  shift
  echo
  echo "=== $label ==="
  "$@"
}

quick_gate() {
  run_step "Track G deterministic replay fixture" \
    cargo test -q bundled_tool_file_reliability_gauntlet_passes -- --test-threads=1
  run_step "bash command classification" \
    cargo test -q command_classifier -- --test-threads=1
  run_step "file tool reliability" \
    cargo test -q file_tool -- --test-threads=1
  run_step "file patch reliability" \
    cargo test -q file_patch -- --test-threads=1
  run_step "permission evidence" \
    cargo test -q permission_controller -- --test-threads=1
  run_step "compaction runtime continuity" \
    cargo test -q runtime_continuity -- --test-threads=1
}

standard_gate() {
  quick_gate
  run_step "cargo fmt" cargo fmt --check
  run_step "cargo check" cargo check -q
  run_step "desktop run event state" \
    corepack pnpm --dir apps/desktop exec playwright test tests/run-event-state.spec.ts
  run_step "desktop build" corepack pnpm --dir apps/desktop build
  run_step "diff whitespace" git diff --check
}

full_gate() {
  standard_gate
  run_step "experimental API check" cargo check --features experimental-api-server -q
  run_step "clippy" cargo clippy --all-features -- -D warnings
  run_step "desktop UI smoke" corepack pnpm --dir apps/desktop test:ui-smoke
}

case "$MODE" in
  quick)
    quick_gate
    ;;
  standard)
    standard_gate
    ;;
  full)
    full_gate
    ;;
  help|-h|--help)
    usage
    ;;
  *)
    echo "Unknown mode: $MODE" >&2
    usage >&2
    exit 2
    ;;
esac
