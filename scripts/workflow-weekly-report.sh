#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

DB_PATH="${PRIORITY_AGENT_WORKFLOW_METRICS_DB:-$HOME/.priority-agent/workflow_metrics.db}"
OUT="docs/workflow/weekly-workflow-report.md"
LIMIT="${1:-8}"

mkdir -p docs/workflow

if [[ ! -f "$DB_PATH" ]]; then
  cat > "$OUT" <<REPORT
# Workflow Weekly Report

- Generated at: $(date -u '+%Y-%m-%d %H:%M:%S UTC')
- Status: no metrics db found at \
  \
  \
\`$DB_PATH\`
REPORT
  echo "[weekly report] no db found, wrote $OUT"
  exit 0
fi

if ! command -v sqlite3 >/dev/null 2>&1; then
  cat > "$OUT" <<REPORT
# Workflow Weekly Report

- Generated at: $(date -u '+%Y-%m-%d %H:%M:%S UTC')
- Status: sqlite3 CLI not installed, cannot generate tabular report.
- DB: \`$DB_PATH\`
REPORT
  echo "[weekly report] sqlite3 missing, wrote $OUT"
  exit 0
fi

has_objective="$(sqlite3 "$DB_PATH" "PRAGMA table_info(workflow_metrics_runs);" | grep -c "objective_score" || true)"
if [[ "$has_objective" -gt 0 ]]; then
  rows="$(sqlite3 -csv "$DB_PATH" "
SELECT week_key,
       COUNT(*) AS runs,
       ROUND(AVG(mainline_hit) * 100.0, 1) AS mainline_hit_rate,
       ROUND(AVG(first_plan_coverage), 1) AS avg_coverage,
       ROUND(AVG(rework_rate), 1) AS avg_rework,
       ROUND(AVG(objective_score), 1) AS avg_objective
FROM workflow_metrics_runs
GROUP BY week_key
ORDER BY week_key DESC
LIMIT $LIMIT;")"
else
  rows="$(sqlite3 -csv "$DB_PATH" "
SELECT week_key,
       COUNT(*) AS runs,
       ROUND(AVG(mainline_hit) * 100.0, 1) AS mainline_hit_rate,
       ROUND(AVG(first_plan_coverage), 1) AS avg_coverage,
       ROUND(AVG(rework_rate), 1) AS avg_rework,
       0.0 AS avg_objective
FROM workflow_metrics_runs
GROUP BY week_key
ORDER BY week_key DESC
LIMIT $LIMIT;")"
fi

{
  echo "# Workflow Weekly Report"
  echo
  echo "- Generated at: $(date -u '+%Y-%m-%d %H:%M:%S UTC')"
  echo "- DB: \`$DB_PATH\`"
  echo
  echo "| Week | Runs | Mainline Hit Rate | WoW | Avg Coverage | WoW | Avg Rework | WoW | Avg Objective | WoW |"
  echo "|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|"
  if [[ -z "$rows" ]]; then
    echo "| (no data) | 0 | 0.0% | - | 0.0% | - | 0.0% | - | 0.0 | - |"
  else
    prev_mainline=""
    prev_coverage=""
    prev_rework=""
    prev_objective=""
    while IFS=, read -r week runs mainline coverage rework objective; do
      [[ -z "$week" ]] && continue
      wow_mainline="-"
      wow_coverage="-"
      wow_rework="-"
      wow_objective="-"
      if [[ -n "$prev_mainline" ]]; then
        wow_mainline="$(awk -v a="$mainline" -v b="$prev_mainline" 'BEGIN{printf "%.1f%%", (a-b)}')"
        wow_coverage="$(awk -v a="$coverage" -v b="$prev_coverage" 'BEGIN{printf "%.1f%%", (a-b)}')"
        wow_rework="$(awk -v a="$rework" -v b="$prev_rework" 'BEGIN{printf "%.1f%%", (a-b)}')"
        wow_objective="$(awk -v a="$objective" -v b="$prev_objective" 'BEGIN{printf "%.1f", (a-b)}')"
      fi
      echo "| $week | $runs | ${mainline}% | ${wow_mainline} | ${coverage}% | ${wow_coverage} | ${rework}% | ${wow_rework} | ${objective} | ${wow_objective} |"
      prev_mainline="$mainline"
      prev_coverage="$coverage"
      prev_rework="$rework"
      prev_objective="$objective"
    done <<< "$rows"
  fi
} > "$OUT"

echo "[weekly report] generated $OUT"
