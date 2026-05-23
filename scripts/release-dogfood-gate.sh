#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

SUITE="release-dogfood"
LABEL="${PRIORITY_AGENT_RELEASE_DOGFOOD_LABEL:-release-dogfood}"
EXPECTED_CASES=(
  core-simple-stale-edit
  core-rust-multi-file-refactor
  desktop-ui-smoke-polish
  code-change-verification-repair-loop
  core-permission-rejection-recovery
  core-long-output-artifact
)

usage() {
  cat <<'EOF'
Usage:
  scripts/release-dogfood-gate.sh [quick|list|prepare|agent-run|summary] [options]

Modes:
  quick     Validate the release-dogfood suite manifest and deterministic gates.
  list      List the 6 release-dogfood live tasks.
  prepare   Prepare isolated worktrees for the 6 dogfood tasks.
  agent-run Run the 6 dogfood tasks through Priority Agent, then collect reports.
  summary   Generate a live-eval summary for an existing --run-id.

Examples:
  scripts/release-dogfood-gate.sh quick
  scripts/release-dogfood-gate.sh agent-run --run-tests --timeout 2400
  scripts/release-dogfood-gate.sh summary --run-id release-dogfood-20260523-123000
EOF
}

manifest_check() {
  ruby -ryaml -e '
    expected = ARGV
    files = Dir["evalsets/live_tasks/*.yaml"].sort
    by_id = {}
    files.each do |file|
      sample = YAML.load_file(file) || {}
      id = sample["id"]
      next if id.nil? || id.empty?
      by_id[id] = [file, sample]
    end

    missing = expected.reject { |id| by_id.key?(id) }
    unless missing.empty?
      warn "missing release dogfood live task(s): #{missing.join(", ")}"
      exit 1
    end

    expected.each do |id|
      file, sample = by_id.fetch(id)
      commands = sample.dig("acceptance", "required_commands")
      if !commands.is_a?(Array) || commands.empty?
        warn "#{id} in #{file} must define acceptance.required_commands"
        exit 1
      end
      if sample["prompt"].to_s.strip.empty?
        warn "#{id} in #{file} must define a prompt"
        exit 1
      end
    end

    puts "release dogfood manifest ok: #{expected.length} tasks"
  ' "${EXPECTED_CASES[@]}"
}

suite_check() {
  local expected actual
  expected="$(printf '%s\n' "${EXPECTED_CASES[@]}")"
  actual="$(scripts/run_live_eval.sh --list --case "$SUITE" | awk 'NR > 2 { print $1 }')"
  if [[ "$actual" != "$expected" ]]; then
    echo "release-dogfood suite does not match the release gate manifest" >&2
    diff -u <(printf '%s\n' "$expected") <(printf '%s\n' "$actual") >&2 || true
    exit 1
  fi
  echo "release dogfood suite ok: ${#EXPECTED_CASES[@]} tasks"
}

list_cases() {
  scripts/run_live_eval.sh --list --case "$SUITE"
}

quick_gate() {
  bash -n scripts/run_live_eval.sh scripts/release-dogfood-gate.sh
  manifest_check
  suite_check
  list_cases
  scripts/tool-file-reliability-gauntlet.sh quick
}

mode="${1:-quick}"
if [[ $# -gt 0 ]]; then
  shift
fi

case "$mode" in
  quick)
    quick_gate "$@"
    ;;
  list)
    list_cases "$@"
    ;;
  prepare)
    scripts/run_live_eval.sh --case "$SUITE" --mode prepare --label "$LABEL" "$@"
    ;;
  agent-run)
    scripts/run_live_eval.sh --case "$SUITE" --mode agent-run --label "$LABEL" "$@"
    ;;
  summary)
    scripts/run_live_eval.sh --mode summary "$@"
    ;;
  -h|--help|help)
    usage
    ;;
  *)
    echo "Unknown mode: $mode" >&2
    usage >&2
    exit 1
    ;;
esac
