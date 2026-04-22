#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "[1/3] Run workflow unit tests"
cargo test workflow --quiet

echo "[2/3] Validate required workflow docs"
required_files=(
  "workflow-v1-design.md"
  "docs/workflow/baseline-report.md"
  "docs/workflow/gate-spec.md"
  "docs/workflow/weights-spec.md"
  "docs/workflow/questioning-spec.md"
  "docs/workflow/planner-executor-spec.md"
  "docs/workflow/metrics-pipeline-spec.md"
  "docs/workflow/m1-integration-plan.md"
  "docs/workflow/m1-acceptance-checklist.md"
  "docs/workflow/m1-gap-list.md"
  "docs/workflow/m2-optimization-plan.md"
  "docs/workflow/rollout-plan.md"
  "docs/workflow/operator-guide.md"
)

for file in "${required_files[@]}"; do
  if [[ ! -f "$file" ]]; then
    echo "Missing required file: $file"
    exit 1
  fi
  echo "  - ok: $file"
done

echo "[3/3] Quick workflow code sanity"
rg -n "pub struct WorkflowEngine|pub enum GateDecision|pub struct WeightEngine" src/engine/workflow >/dev/null

echo "Workflow M1 acceptance passed."
