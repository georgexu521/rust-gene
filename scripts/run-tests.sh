#!/usr/bin/env bash
# Priority Agent Test Runner
# Usage: ./scripts/run-tests.sh [quick|standard|full|strict|help]

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

print_header() {
    echo ""
    echo -e "${BLUE}════════════════════════════════════════════════${NC}"
    echo -e "${BLUE}  Priority Agent Test Runner${NC}"
    echo -e "${BLUE}════════════════════════════════════════════════${NC}"
    echo ""
}

print_success() {
    echo -e "${GREEN}✓ $1${NC}"
}

print_error() {
    echo -e "${RED}✗ $1${NC}"
}

print_info() {
    echo -e "${YELLOW}→ $1${NC}"
}

run_cmd() {
    local name="$1"
    local cmd="$2"
    echo ""
    print_info "Running: $name"
    echo "  Command: $cmd"
    echo ""

    if eval "$cmd"; then
        print_success "$name passed"
        return 0
    else
        print_error "$name failed"
        return 1
    fi
}

run_quick() {
    echo -e "${BLUE}═══ Quick Check (30 seconds) ═══${NC}"
    run_cmd "Format check" "cargo fmt --check"
    run_cmd "Basic compilation" "cargo check -q"
    run_cmd "Experimental API check" "cargo check --features experimental-api-server -q"
}

run_standard() {
    run_quick
    echo ""
    echo -e "${BLUE}═══ Standard Tests (2-5 minutes) ═══${NC}"
    run_cmd "Core unit tests" "cargo test -q"
}

run_full() {
    run_standard
    echo ""
    echo -e "${BLUE}═══ Full Test Suite (5-10 minutes) ═══${NC}"
    run_cmd "Workflow tests (single-threaded)" "env PRIORITY_AGENT_WORKFLOW_ENABLED=1 cargo test -q -- --test-threads=1"
}

run_strict() {
    run_full
    echo ""
    echo -e "${BLUE}═══ Strict Checks ═══${NC}"
    run_cmd "Clippy (all features)" "cargo clippy --all-features -- -D warnings"
    run_cmd "Workflow production gates" "bash scripts/workflow-production-gates.sh"
}

run_specific() {
    local pattern="$1"
    echo -e "${BLUE}═══ Running tests matching: $pattern ═══${NC}"
    run_cmd "Tests matching '$pattern'" "cargo test -q $pattern"
}

show_help() {
    cat << 'EOF'
Priority Agent Test Runner

Usage:
  ./scripts/run-tests.sh [MODE] [OPTIONS]

Modes:
  quick       - Fast checks: fmt, compilation (30s)
  standard    - Quick + core unit tests (2-5min)
  full        - Standard + workflow tests (5-10min)
  strict      - Full + clippy + production gates (10-15min)
  <pattern>   - Run specific tests (e.g., 'edit_match', 'file_tool')
  help        - Show this help

Examples:
  ./scripts/run-tests.sh quick                    # Fast sanity check
  ./scripts/run-tests.sh standard                 # Normal dev cycle
  ./scripts/run-tests.sh full                     # Before pushing
  ./scripts/run-tests.sh strict                   # Release candidate
  ./scripts/run-tests.sh edit_match              # Test file editing
  ./scripts/run-tests.sh "instructions"          # Test instruction parsing
  ./scripts/run-tests.sh "route_scoped_tools"    # Test tool routing

Environment Variables:
  PRIORITY_AGENT_WORKFLOW_ENABLED=1   - Enable workflow tests
  PRIORITY_AGENT_AUTO_APPROVE=1       - Auto-approve in tests

EOF
}

# Main
print_header

case "${1:-help}" in
    quick)
        run_quick
        ;;
    standard)
        run_standard
        ;;
    full)
        run_full
        ;;
    strict)
        run_strict
        ;;
    help|--help|-h)
        show_help
        exit 0
        ;;
    *)
        # If argument looks like a test pattern, run it
        if [[ "$1" =~ ^[a-zA-Z0-9_-]+$ ]]; then
            run_specific "$1"
        else
            echo "Unknown mode: $1"
            show_help
            exit 1
        fi
        ;;
esac

echo ""
echo -e "${GREEN}════════════════════════════════════════════════${NC}"
echo -e "${GREEN}  All tests passed!${NC}"
echo -e "${GREEN}════════════════════════════════════════════════${NC}"
echo ""
