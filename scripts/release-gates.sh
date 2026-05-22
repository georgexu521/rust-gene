#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

MODE="${1:-quick}"

usage() {
  cat <<'EOF'
Usage: scripts/release-gates.sh [quick|full|help]

Modes:
  quick  Release smoke gates: script syntax, install dry-run, focused tests, fmt/check/clippy.
  full   quick + full cargo test + experimental API feature check + package dry-run.
EOF
}

run_step() {
  local label="$1"
  shift
  echo
  echo "=== $label ==="
  "$@"
}

quick_gates() {
  run_step "install script syntax" bash -n scripts/install.sh
  run_step "package script syntax" bash -n scripts/package-release.sh
  run_step "install version" scripts/install.sh --version
  run_step "install dry-run" scripts/install.sh --dry-run --release --features experimental-api-server --prefix /tmp/priority-agent-install-smoke
  run_step "doctor diagnostics tests" cargo test -q diagnostics
  run_step "doctor slash tests" cargo test -q doctor
  run_step "config release tests" cargo test -q config
  run_step "mcp tool tests" cargo test -q mcp_tool
  run_step "fmt" cargo fmt --check
  run_step "check" cargo check -q
  run_step "clippy" cargo clippy --all-features -- -D warnings
  run_step "diff whitespace" git diff --check
}

full_gates() {
  quick_gates
  run_step "experimental API check" cargo check --features experimental-api-server -q
  run_step "package dry-run" scripts/package-release.sh --features experimental-api-server --dry-run
  run_step "full tests" cargo test -q
}

case "$MODE" in
  quick)
    quick_gates
    ;;
  full)
    full_gates
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
