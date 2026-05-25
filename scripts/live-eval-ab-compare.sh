#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

SUITE="mvp-weighted-agent"
MODE="agent-run"
RUN_TESTS=1
OVERLAY=0
SKIP_PROVIDER_HEALTH="${PRIORITY_AGENT_AB_SKIP_PROVIDER_HEALTH:-1}"
BASELINE_RUN_ID=""
WEIGHTED_RUN_ID=""
OUTPUT=""
RUN_PREFIX="ab-$(date +%Y%m%d-%H%M%S)"

usage() {
  cat <<'EOF'
Usage:
  scripts/live-eval-ab-compare.sh [--suite mvp-weighted-agent] [--run-tests] [--overlay-working-tree]
  scripts/live-eval-ab-compare.sh --baseline-run-id <id> --weighted-run-id <id> [--output <path>]

Options:
  --suite ID              Live-eval suite to run when run ids are not supplied.
  --mode MODE             Runner mode for new runs, default: agent-run.
  --run-tests             Run required commands during new runs, default: enabled.
  --no-run-tests          Skip required commands during new runs.
  --overlay-working-tree  Apply current tracked changes to live-eval worktrees.
  --skip-provider-health  Skip provider health preflight for both profiles, default: enabled.
  --provider-health       Enable provider health preflight for both profiles.
  --baseline-run-id ID    Existing baseline run id under docs/benchmarks/live-<id>.
  --weighted-run-id ID    Existing weighted run id under docs/benchmarks/live-<id>.
  --output PATH           Comparison report path.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --suite) SUITE="${2:-}"; shift 2 ;;
    --mode) MODE="${2:-}"; shift 2 ;;
    --run-tests) RUN_TESTS=1; shift ;;
    --no-run-tests) RUN_TESTS=0; shift ;;
    --overlay-working-tree) OVERLAY=1; shift ;;
    --skip-provider-health) SKIP_PROVIDER_HEALTH=1; shift ;;
    --provider-health) SKIP_PROVIDER_HEALTH=0; shift ;;
    --baseline-run-id) BASELINE_RUN_ID="${2:-}"; shift 2 ;;
    --weighted-run-id) WEIGHTED_RUN_ID="${2:-}"; shift 2 ;;
    --output) OUTPUT="${2:-}"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown argument: $1" >&2; usage; exit 1 ;;
  esac
done

run_profile() {
  local profile="$1" run_id="$2"
  local args=(scripts/run_live_eval.sh --case "$SUITE" --mode "$MODE" --run-id "$run_id")
  if [[ "$RUN_TESTS" -eq 1 ]]; then
    args+=(--run-tests)
  fi
  if [[ "$OVERLAY" -eq 1 ]]; then
    args+=(--overlay-working-tree)
  fi
  if [[ "$SKIP_PROVIDER_HEALTH" -eq 1 ]]; then
    args+=(--skip-provider-health)
  fi

  if [[ "$profile" == "baseline" ]]; then
    set +e
    PRIORITY_AGENT_CANDIDATE_ACTIONS=off \
    PRIORITY_AGENT_WEIGHT_PROFILE=baseline \
    PRIORITY_AGENT_LOW_VALUE_REPLAN=0 \
      "${args[@]}"
    local status=$?
    set -e
  else
    set +e
    PRIORITY_AGENT_CANDIDATE_ACTIONS="${PRIORITY_AGENT_CANDIDATE_ACTIONS:-shadow}" \
    PRIORITY_AGENT_WEIGHT_PROFILE=weighted \
    PRIORITY_AGENT_LOW_VALUE_REPLAN="${PRIORITY_AGENT_LOW_VALUE_REPLAN:-1}" \
      "${args[@]}"
    local status=$?
    set -e
  fi
  if [[ "$status" -ne 0 ]]; then
    echo "Live eval profile '$profile' completed with failing tasks; continuing to summary comparison." >&2
  fi
  scripts/run_live_eval.sh --mode summary --run-id "$run_id" >/dev/null
}

if [[ -z "$BASELINE_RUN_ID" || -z "$WEIGHTED_RUN_ID" ]]; then
  BASELINE_RUN_ID="${BASELINE_RUN_ID:-$RUN_PREFIX-baseline}"
  WEIGHTED_RUN_ID="${WEIGHTED_RUN_ID:-$RUN_PREFIX-weighted}"
  run_profile baseline "$BASELINE_RUN_ID"
  run_profile weighted "$WEIGHTED_RUN_ID"
fi

OUTPUT="${OUTPUT:-docs/benchmarks/live-${WEIGHTED_RUN_ID}/ab-compare-${BASELINE_RUN_ID}-vs-${WEIGHTED_RUN_ID}.md}"

PYTHONDONTWRITEBYTECODE=1 python3 - "$BASELINE_RUN_ID" "$WEIGHTED_RUN_ID" "$OUTPUT" <<'PY'
import pathlib
import sys
from scripts.live_eval_report_parser import report_rows

baseline_id, weighted_id, output_path = sys.argv[1:4]
baseline_dir = pathlib.Path("docs/benchmarks") / f"live-{baseline_id}"
weighted_dir = pathlib.Path("docs/benchmarks") / f"live-{weighted_id}"
output = pathlib.Path(output_path)
output.parent.mkdir(parents=True, exist_ok=True)

def as_int(value, default=0):
    try:
        return int(value)
    except Exception:
        return default

def rows_by_task(run_dir):
    return {row["task"]: row for row in report_rows(run_dir)}

baseline = rows_by_task(baseline_dir)
weighted = rows_by_task(weighted_dir)
tasks = sorted(set(baseline) | set(weighted))

def avg(rows, key):
    values = [as_int(row.get(key), 0) for row in rows if row.get(key) != "missing"]
    return sum(values) / len(values) if values else 0

baseline_rows = list(baseline.values())
weighted_rows = list(weighted.values())
baseline_agent = avg(baseline_rows, "agent_score")
weighted_agent = avg(weighted_rows, "agent_score")
delta = weighted_agent - baseline_agent
helped = delta > 2
hurt = delta < -2
verdict = "weighted_helped" if helped else "weighted_hurt" if hurt else "inconclusive"

def md_cell(value):
    return str(value).replace("|", "\\|").replace("\n", " ")

lines = [
    "# Live Eval A/B Comparison",
    "",
    f"- Baseline run: `{baseline_id}`",
    f"- Weighted run: `{weighted_id}`",
    f"- Baseline average agent score: `{baseline_agent:.1f}`",
    f"- Weighted average agent score: `{weighted_agent:.1f}`",
    f"- Delta: `{delta:+.1f}`",
    f"- Verdict: `{verdict}`",
    "",
    "## Suite Delta",
    "",
    "| metric | baseline | weighted | delta |",
    "|--------|----------|----------|-------|",
]
for key in ("outcome_score", "process_score", "efficiency_score", "agent_score"):
    b = avg(baseline_rows, key)
    w = avg(weighted_rows, key)
    lines.append(f"| {key} | {b:.1f} | {w:.1f} | {w - b:+.1f} |")

for key in ("invalid_action_count", "premature_edit_count", "scope_drift_count", "repeated_action_count", "failed_action_count"):
    b = sum(as_int(row.get(key)) for row in baseline_rows)
    w = sum(as_int(row.get(key)) for row in weighted_rows)
    lines.append(f"| {key} | {b} | {w} | {w - b:+d} |")

lines.extend([
    "",
    "## Task Delta",
    "",
    "| task | baseline_status | weighted_status | baseline_agent | weighted_agent | delta | baseline_penalties | weighted_penalties |",
    "|------|-----------------|-----------------|----------------|----------------|-------|--------------------|--------------------|",
])
for task in tasks:
    b = baseline.get(task, {})
    w = weighted.get(task, {})
    b_score = as_int(b.get("agent_score"), 0)
    w_score = as_int(w.get("agent_score"), 0)
    lines.append(
        "| {task} | {baseline_status} | {weighted_status} | {baseline_agent} | {weighted_agent} | {delta:+d} | {baseline_penalties} | {weighted_penalties} |".format(
            task=md_cell(task),
            baseline_status=md_cell(b.get("status", "missing")),
            weighted_status=md_cell(w.get("status", "missing")),
            baseline_agent=md_cell(b.get("agent_score", "missing")),
            weighted_agent=md_cell(w.get("agent_score", "missing")),
            delta=w_score - b_score,
            baseline_penalties=md_cell(b.get("score_penalties", "none")),
            weighted_penalties=md_cell(w.get("score_penalties", "none")),
        )
    )

lines.extend([
    "",
    "## Reading",
    "",
    "- Baseline keeps safety gates on; it only disables or shadows weighted planning controls.",
    "- Weighted is a product profile comparison, not a permission bypass.",
    "- `inconclusive` means the average agent-score delta is within +/-2 points.",
])

output.write_text("\n".join(lines).rstrip() + "\n", encoding="utf-8")
print(output)
PY
