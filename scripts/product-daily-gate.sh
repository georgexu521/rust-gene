#!/usr/bin/env bash
# Product daily gate: compact daily scoreboard for Priority Agent.
#
# Runs a small representative eval set and produces a summary report.
# Designed to be fast enough for daily development feedback.
#
# Usage:
#   scripts/product-daily-gate.sh                    # full run
#   scripts/product-daily-gate.sh --dry-run           # show cases, skip agent run
#   scripts/product-daily-gate.sh --layer smoke       # fast daily smoke layer
#   scripts/product-daily-gate.sh --case <id>         # run one case
#   scripts/product-daily-gate.sh --skip-provider-health
#   scripts/product-daily-gate.sh --timeout 600
#   scripts/product-daily-gate.sh --report-only RUN_ID

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

# ── Daily eval case lists ──
# smoke: fast must-pass health; product: main daily line; stretch: slower
# entrypoint/provider stress that should not mask the product layer signal.
DAILY_SMOKE_CASES=(
  core-inspection-grounding
  core-simple-stale-edit
  project-partner-resume-with-memory
  minimum-agent-verification-repair
)

DAILY_PRODUCT_CASES=(
  core-inspection-grounding
  core-simple-stale-edit
  core-multi-file-edit
  core-rust-multi-file-refactor
  code-change-verification-repair-loop
  project-partner-resume-with-memory
  memory-recall-conflict-precision
  minimum-agent-verification-repair
)

DAILY_STRETCH_CASES=(
  desktop-ui-smoke-polish
  code-change-verification-repair-loop
  core-long-output-artifact
  core-provider-roundtrip
)

DESKTOP_CASES=(
  desktop-ui-smoke-polish
)

# Cases that require special environment (desktop, pnpm, playwright)
SKIP_DESKTOP_CASES="${PRIORITY_AGENT_SKIP_DESKTOP_CASES:-1}"

# ── Defaults ──
DRY_RUN=0
SKIP_PROVIDER_HEALTH=0
TIMEOUT_SECS="${PRIORITY_AGENT_DAILY_TIMEOUT_SECS:-1200}"
REPAIR_NO_EFFECTIVE_PROGRESS_SECS="${PRIORITY_AGENT_DAILY_REPAIR_NO_EFFECTIVE_PROGRESS_SECS:-360}"
REPORT_ONLY=""
LABEL=""
RUN_ID=""
LAYER="${PRIORITY_AGENT_DAILY_LAYER:-product}"
CASE_OVERRIDE=""

# ── Parse args ──
while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run) DRY_RUN=1; shift ;;
    --layer) LAYER="${2:-product}"; shift 2 ;;
    --case) CASE_OVERRIDE="${2:-}"; shift 2 ;;
    --skip-provider-health) SKIP_PROVIDER_HEALTH=1; shift ;;
    --include-desktop) SKIP_DESKTOP_CASES=0; shift ;;
    --timeout) TIMEOUT_SECS="${2:-1200}"; shift 2 ;;
    --report-only) REPORT_ONLY="${2:-}"; shift 2 ;;
    --label) LABEL="${2:-product-daily}"; shift 2 ;;
    --run-id) RUN_ID="${2:-}"; shift 2 ;;
    -h|--help)
      cat <<'EOF'
Usage: scripts/product-daily-gate.sh [options]

Options:
  --dry-run              Show case list and exit without running agent
  --layer LAYER          daily layer: smoke, product, stretch, all (default: product)
  --case ID              Run a single live task case
  --skip-provider-health Skip provider health preflight
  --include-desktop      Include desktop UI tests (requires pnpm/playwright)
  --timeout SECS         Wall-clock timeout per agent run (default: 1200)
  --report-only RUN_ID   Generate report from existing run data
  --label LABEL          Report label (default: product-daily)
  --run-id ID            Stable run id (default: timestamp)
  -h, --help             Show this help
EOF
      exit 0
      ;;
    *) echo "Unknown option: $1" >&2; exit 1 ;;
  esac
done

case "$LAYER" in
  smoke) DAILY_CASES=("${DAILY_SMOKE_CASES[@]}") ;;
  product) DAILY_CASES=("${DAILY_PRODUCT_CASES[@]}") ;;
  stretch) DAILY_CASES=("${DAILY_STRETCH_CASES[@]}") ;;
  all) DAILY_CASES=("${DAILY_SMOKE_CASES[@]}" "${DAILY_PRODUCT_CASES[@]}" "${DAILY_STRETCH_CASES[@]}") ;;
  *) echo "Unknown layer: $LAYER" >&2; exit 1 ;;
esac

UNIQUE_CASES=()
for candidate_case in "${DAILY_CASES[@]}"; do
  found=0
  if [[ "${#UNIQUE_CASES[@]}" -gt 0 ]]; then
    for id in "${UNIQUE_CASES[@]}"; do
      if [[ "$id" == "$candidate_case" ]]; then
        found=1
        break
      fi
    done
  fi
  if [[ "$found" == "0" ]]; then
    UNIQUE_CASES+=("$candidate_case")
  fi
done
DAILY_CASES=("${UNIQUE_CASES[@]}")

if [[ "$SKIP_DESKTOP_CASES" == "0" ]]; then
  for desktop_case in "${DESKTOP_CASES[@]}"; do
    found=0
    for id in "${DAILY_CASES[@]}"; do
      if [[ "$id" == "$desktop_case" ]]; then
        found=1
        break
      fi
    done
    if [[ "$found" == "0" ]]; then
      DAILY_CASES+=("$desktop_case")
    fi
  done
fi

if [[ -n "$CASE_OVERRIDE" ]]; then
  DAILY_CASES=("$CASE_OVERRIDE")
fi

if [[ -z "$LABEL" ]]; then
  if [[ "$LAYER" == "product" ]]; then
    LABEL="product-daily"
  else
    LABEL="daily-${LAYER}"
  fi
fi

DESKTOP_INCLUDED=0
for id in "${DAILY_CASES[@]}"; do
  for desktop_case in "${DESKTOP_CASES[@]}"; do
    if [[ "$id" == "$desktop_case" ]]; then
      DESKTOP_INCLUDED=1
    fi
  done
done

if [[ -z "$RUN_ID" && -z "$REPORT_ONLY" ]]; then
  RUN_ID="${LABEL}-$(date +%Y%m%d-%H%M%S)"
fi

REPORT_DIR="docs/benchmarks"
WORK_ROOT="target/live-evals"

# ── Helpers ──

print_header() {
  echo
  echo "╔══════════════════════════════════════════════════════════╗"
  echo "║          Priority Agent — Product Daily Gate            ║"
  echo "╚══════════════════════════════════════════════════════════╝"
  echo
  echo "Run id: ${RUN_ID:-$REPORT_ONLY}"
  echo "Layer: $LAYER"
  if [[ -n "$CASE_OVERRIDE" ]]; then
    echo "Single case: $CASE_OVERRIDE"
  fi
  echo "Cases: ${#DAILY_CASES[@]}"
  echo "Timeout: ${TIMEOUT_SECS}s per case"
  echo "Repair no-effective-progress timeout: ${REPAIR_NO_EFFECTIVE_PROGRESS_SECS}s"
  if [[ "$DESKTOP_INCLUDED" == "1" ]]; then
    echo "Desktop tests: included"
  else
    echo "Desktop tests: skipped (use --include-desktop to enable)"
  fi
  echo
}

print_case_list() {
  printf '%-40s %s\n' "Case ID" "Status"
  printf '%-40s %s\n' "-------" "------"
  for id in "${DAILY_CASES[@]}"; do
    printf '%-40s %s\n' "$id" "pending"
  done
}

check_live_tasks() {
  local missing=0
  for id in "${DAILY_CASES[@]}"; do
    if ! ls evalsets/live_tasks/"${id}".yaml >/dev/null 2>&1; then
      echo "Missing live task: $id" >&2
      missing=1
    fi
  done
  return "$missing"
}

# ── Report generation ──

generate_report() {
  local run_id="$1"
  local run_dir="$REPORT_DIR/live-${run_id}"
  local report_file="${run_dir}/product-daily-summary.md"
  local json_file="${run_dir}/product-daily-summary.json"

  if [[ ! -d "$run_dir" ]]; then
    echo "Run directory not found: $run_dir" >&2
    return 1
  fi

  mkdir -p "$run_dir"

  python3 scripts/product_daily_summary.py \
    --run-dir "$run_dir" \
    --cases "${DAILY_CASES[*]}" \
    --run-id "$run_id" \
    --layer "$LAYER" \
    --report "$report_file" \
    --json "$json_file"

  echo
  echo "Report written to: $report_file"
  echo "JSON written to:   $json_file"
}

# ── Main ──

print_header

if [[ -n "$REPORT_ONLY" ]]; then
  generate_report "$REPORT_ONLY"
  exit 0
fi

if [[ "$DRY_RUN" -eq 1 ]]; then
  echo "=== Dry Run — Case List ==="
  echo
  print_case_list
  echo
  echo "Run without --dry-run to execute."
  exit 0
fi

# Preflight checks
echo "=== Preflight ==="
check_live_tasks || { echo "Missing live tasks. Aborting." >&2; exit 1; }
echo "All ${#DAILY_CASES[@]} live tasks found."
echo
echo "=== File Size Guard ==="
scripts/file-size-report.sh --threshold 3000 --fail-over 3000
echo
echo "=== File Size Watchlist ==="
scripts/file-size-report.sh --threshold 1500 --top 20

# Build binary
echo
echo "=== Build ==="
cargo build --release --features experimental-api-server >/dev/null
echo "Binary built: target/release/priority-agent"

# Run evals
echo
echo "=== Running Daily Cases ==="
echo

PASSED=0
FAILED=0
FAILED_CASES=()
RUN_LIVE_EVAL_ARGS=()
if [[ "$SKIP_PROVIDER_HEALTH" == "1" ]]; then
  RUN_LIVE_EVAL_ARGS+=(--skip-provider-health)
fi

for case_id in "${DAILY_CASES[@]}"; do
  echo "--- $case_id ---"

  # Use run_live_eval.sh in agent-run mode
  RUN_CMD=(
    scripts/run_live_eval.sh
    --case "$case_id" \
    --mode agent-run \
    --run-id "$RUN_ID" \
    --label "$LABEL" \
    --timeout "$TIMEOUT_SECS"
  )
  if [[ "$case_id" == "code-change-verification-repair-loop" && "$REPAIR_NO_EFFECTIVE_PROGRESS_SECS" -gt 0 ]]; then
    RUN_CMD+=(--no-effective-progress-timeout "$REPAIR_NO_EFFECTIVE_PROGRESS_SECS")
  fi
  if [[ "${#RUN_LIVE_EVAL_ARGS[@]}" -gt 0 ]]; then
    RUN_CMD+=("${RUN_LIVE_EVAL_ARGS[@]}")
  fi

  if "${RUN_CMD[@]}" 2>&1; then

    # Collect results
    WORK_DIR="$WORK_ROOT/$RUN_ID/$case_id/worktree"
    if [[ -d "$WORK_DIR" ]]; then
      scripts/run_live_eval.sh \
        --case "$case_id" \
        --mode collect \
        --workdir "$WORK_DIR" \
        --run-id "$RUN_ID" \
        --label "$LABEL" \
        --run-tests \
        2>&1 || true
    fi

    # Check quality status
    QUALITY_FILE="$REPORT_DIR/live-$RUN_ID/$case_id/agent-quality-status.txt"
    if [[ -f "$QUALITY_FILE" ]] && grep -q '^status=ok' "$QUALITY_FILE"; then
      echo "  Result: ok"
      PASSED=$((PASSED + 1))
    else
      echo "  Result: failed"
      FAILED=$((FAILED + 1))
      FAILED_CASES+=("$case_id")
    fi
  else
    echo "  Result: agent-run failed"
    FAILED=$((FAILED + 1))
    FAILED_CASES+=("$case_id")
  fi
  echo
done

# Summary
echo "=== Summary ==="
echo
echo "Passed: $PASSED / ${#DAILY_CASES[@]}"
echo "Failed: $FAILED / ${#DAILY_CASES[@]}"

if [[ ${#FAILED_CASES[@]} -gt 0 ]]; then
  echo
  echo "Failed cases:"
  for id in "${FAILED_CASES[@]}"; do
    echo "  - $id"
  done
fi

# Generate report
echo
echo "=== Generating Report ==="
generate_report "$RUN_ID"

# Exit with failure if any case failed
if [[ "$FAILED" -gt 0 ]]; then
  exit 1
fi
