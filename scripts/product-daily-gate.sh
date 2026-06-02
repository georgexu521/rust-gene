#!/usr/bin/env bash
# Product daily gate: compact daily scoreboard for Priority Agent.
#
# Runs a small representative eval set and produces a summary report.
# Designed to be fast enough for daily development feedback.
#
# Usage:
#   scripts/product-daily-gate.sh                    # full run
#   scripts/product-daily-gate.sh --dry-run           # show cases, skip agent run
#   scripts/product-daily-gate.sh --skip-provider-health
#   scripts/product-daily-gate.sh --timeout 600
#   scripts/product-daily-gate.sh --report-only RUN_ID

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

# ── Daily eval case list ──
# These cases cover the core product surface: inspection, stale edit,
# multi-file edit, repair loop, memory, and agent verification.
DAILY_CASES=(
  core-inspection-grounding
  core-simple-stale-edit
  core-multi-file-edit
  core-rust-multi-file-refactor
  code-change-verification-repair-loop
  project-partner-resume-with-memory
  memory-recall-conflict-precision
  minimum-agent-verification-repair
  desktop-ui-smoke-polish
)

# ── Defaults ──
DRY_RUN=0
SKIP_PROVIDER_HEALTH=0
TIMEOUT_SECS="${PRIORITY_AGENT_DAILY_TIMEOUT_SECS:-1200}"
REPORT_ONLY=""
LABEL="product-daily"
RUN_ID=""

# ── Parse args ──
while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run) DRY_RUN=1; shift ;;
    --skip-provider-health) SKIP_PROVIDER_HEALTH=1; shift ;;
    --timeout) TIMEOUT_SECS="${2:-600}"; shift 2 ;;
    --report-only) REPORT_ONLY="${2:-}"; shift 2 ;;
    --label) LABEL="${2:-product-daily}"; shift 2 ;;
    --run-id) RUN_ID="${2:-}"; shift 2 ;;
    -h|--help)
      cat <<'EOF'
Usage: scripts/product-daily-gate.sh [options]

Options:
  --dry-run              Show case list and exit without running agent
  --skip-provider-health Skip provider health preflight
  --timeout SECS         Wall-clock timeout per agent run (default: 600)
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
  echo "Cases: ${#DAILY_CASES[@]}"
  echo "Timeout: ${TIMEOUT_SECS}s per case"
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

  python3 - "$run_dir" "${DAILY_CASES[*]}" "$run_id" "$report_file" "$json_file" <<'PY'
import json
import pathlib
import sys
from scripts.live_eval_report_parser import (
    derived_trajectory_metrics_from_events,
    evaluate_output_assertions,
    evaluate_trajectory_assertions,
    memory_proposal_metrics_from_trace,
    normalized_runtime_spine_assertions,
    runtime_spine_metrics_from_events,
    score_live_eval_record,
)

run_dir = pathlib.Path(sys.argv[1])
daily_cases = sys.argv[2].split()
run_id = sys.argv[3]
report_path = pathlib.Path(sys.argv[4])
json_path = pathlib.Path(sys.argv[5])

results = []
for case_id in daily_cases:
    case_dir = run_dir / case_id
    if not case_dir.exists():
        results.append({
            "id": case_id,
            "status": "missing",
            "reason": "no run directory",
        })
        continue

    # Load test status
    test_status_file = case_dir / "test-status.txt"
    test_status = test_status_file.read_text().strip() if test_status_file.exists() else "unknown"

    # Load agent output
    output_file = case_dir / "agent-output.md"
    output = output_file.read_text(encoding="utf-8") if output_file.exists() else ""

    # Load events
    events_file = case_dir / "agent-events.jsonl"
    events = []
    if events_file.exists():
        for line in events_file.read_text(encoding="utf-8").splitlines():
            line = line.strip()
            if line:
                try:
                    events.append(json.loads(line))
                except json.JSONDecodeError:
                    pass

    # Load diff
    diff_file = case_dir / "diff.patch"
    diff = diff_file.read_text(encoding="utf-8") if diff_file.exists() else ""

    # Load sample
    sample_file = case_dir / "sample.json"
    sample = {}
    if sample_file.exists():
        try:
            sample = json.loads(sample_file.read_text(encoding="utf-8"))
        except json.JSONDecodeError:
            pass

    # Load agent quality status
    quality_file = case_dir / "agent-quality-status.txt"
    quality_status = quality_file.read_text().strip() if quality_file.exists() else "unknown"

    # Load required commands log
    cmd_log_file = case_dir / "required-commands.log"
    cmd_log = cmd_log_file.read_text(encoding="utf-8") if cmd_log_file.exists() else ""

    # Load stderr
    stderr_file = case_dir / "agent-stderr.log"
    stderr = stderr_file.read_text(encoding="utf-8") if stderr_file.exists() else ""

    # Score
    try:
        score_result = score_live_eval_record(
            output=output,
            diff=diff,
            events=events,
            sample=sample,
            test_status=test_status,
            cmd_log=cmd_log,
            stderr=stderr,
        )
    except Exception:
        score_result = {}

    # Runtime spine metrics
    try:
        spine_metrics = runtime_spine_metrics_from_events(events)
    except Exception:
        spine_metrics = {}

    # Memory proposal metrics
    try:
        memory_metrics = memory_proposal_metrics_from_trace(events)
    except Exception:
        memory_metrics = {}

    # Trajectory metrics
    try:
        trajectory_metrics = derived_trajectory_metrics_from_events(events)
    except Exception:
        trajectory_metrics = {}

    result = {
        "id": case_id,
        "status": quality_status if quality_status != "unknown" else test_status,
        "test_status": test_status,
        "quality_status": quality_status,
        "outcome_score": score_result.get("outcome_score"),
        "process_score": score_result.get("process_score"),
        "efficiency_score": score_result.get("efficiency_score"),
        "agent_score": score_result.get("agent_score"),
        "failure_owner": score_result.get("failure_owner", "unknown"),
        "closeout_status": score_result.get("closeout_status", "unknown"),
        "verification_passed": score_result.get("verification_passed"),
        "required_command_status": score_result.get("required_command_status", "unknown"),
        "runtime_spine_phases": spine_metrics.get("phases_seen", []),
        "memory_active": memory_metrics.get("active", False),
        "memory_proposals": memory_metrics.get("proposal_count", 0),
        "tool_errors": trajectory_metrics.get("tool_errors", 0),
        "changed_files": trajectory_metrics.get("changed_files", 0),
    }
    results.append(result)

# Compute summary
passed = sum(1 for r in results if r.get("status") == "ok" or r.get("quality_status") == "ok")
failed = sum(1 for r in results if r.get("status") in ("failed", "not_verified"))
missing = sum(1 for r in results if r.get("status") == "missing")
total = len(results)

summary = {
    "run_id": run_id,
    "generated": __import__("datetime").datetime.now().isoformat(),
    "total": total,
    "passed": passed,
    "failed": failed,
    "missing": missing,
    "pass_rate": f"{passed}/{total}" if total > 0 else "0/0",
    "cases": results,
}

# Write JSON
json_path.write_text(json.dumps(summary, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")

# Write Markdown report
lines = [
    "# Product Daily Gate Summary",
    "",
    f"- Run id: `{run_id}`",
    f"- Generated: {summary['generated']}",
    f"- Pass rate: **{summary['pass_rate']}**",
    "",
    "## Results",
    "",
    f"| {'Case':<40} | {'Status':<12} | {'Score':<6} | {'Owner':<15} | {'Closeout':<12} | {'Phases':<20} | {'Memory':<8} |",
    f"|{'-'*42}|{'-'*14}|{'-'*8}|{'-'*17}|{'-'*14}|{'-'*22}|{'-'*10}|",
]

for r in results:
    status_icon = "ok" if r.get("status") == "ok" or r.get("quality_status") == "ok" else r.get("status", "?")
    score = r.get("agent_score")
    score_str = f"{score:.0f}" if score is not None else "-"
    owner = r.get("failure_owner", "-") or "-"
    closeout = r.get("closeout_status", "-") or "-"
    phases = ", ".join(r.get("runtime_spine_phases", [])[:3]) or "-"
    memory = "yes" if r.get("memory_active") else "no"
    lines.append(f"| {r['id']:<40} | {status_icon:<12} | {score_str:<6} | {owner:<15} | {closeout:<12} | {phases:<20} | {memory:<8} |")

lines.extend([
    "",
    "## Failure Owners",
    "",
])

owner_counts = {}
for r in results:
    owner = r.get("failure_owner", "unknown") or "unknown"
    if owner != "none" and r.get("status") != "ok":
        owner_counts[owner] = owner_counts.get(owner, 0) + 1

if owner_counts:
    for owner, count in sorted(owner_counts.items(), key=lambda x: -x[1]):
        lines.append(f"- `{owner}`: {count}")
else:
    lines.append("- No failures")

lines.extend([
    "",
    "## Runtime Spine Coverage",
    "",
])

phase_counts = {}
for r in results:
    for phase in r.get("runtime_spine_phases", []):
        phase_counts[phase] = phase_counts.get(phase, 0) + 1

if phase_counts:
    for phase, count in sorted(phase_counts.items(), key=lambda x: -x[1]):
        lines.append(f"- `{phase}`: {count}/{total} cases")
else:
    lines.append("- No spine data")

lines.extend([
    "",
    "## Next Steps",
    "",
])

if failed > 0:
    lines.append(f"- {failed} case(s) failed. Check failure owners above for triage.")
    for r in results:
        if r.get("status") not in ("ok", "missing") and r.get("quality_status") != "ok":
            lines.append(f"  - `{r['id']}`: owner=`{r.get('failure_owner', 'unknown')}`")
else:
    lines.append("- All cases passed. No immediate action required.")

lines.append("")
report_path.write_text("\n".join(lines), encoding="utf-8")
print(f"Report: {report_path}")
print(f"JSON:   {json_path}")
PY

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

for case_id in "${DAILY_CASES[@]}"; do
  echo "--- $case_id ---"

  # Use run_live_eval.sh in agent-run mode
  if scripts/run_live_eval.sh \
    --case "$case_id" \
    --mode agent-run \
    --run-id "$RUN_ID" \
    --label "$LABEL" \
    --timeout "$TIMEOUT_SECS" \
    ${SKIP_PROVIDER_HEALTH:+--skip-provider-health} \
    2>&1; then

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
