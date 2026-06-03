#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

TASK_DIR="evalsets/live_tasks"
MODE="list"
CASE_ID=""
LABEL="live-eval"
RUN_ID=""
RUN_ID_PROVIDED=0
WORK_ROOT="target/live-evals"
WORKDIR=""
REPORT_DIR="docs/benchmarks"
SKIP_BUILD=0
RUN_TESTS=0
AGENT_TIMEOUT_SECS="${PRIORITY_AGENT_LIVE_EVAL_TIMEOUT_SECS:-0}"
AGENT_IDLE_SECS="${PRIORITY_AGENT_LIVE_EVAL_IDLE_SECS:-0}"
AGENT_MONITOR_INTERVAL_SECS="${PRIORITY_AGENT_LIVE_EVAL_MONITOR_INTERVAL_SECS:-30}"
AGENT_NO_EFFECTIVE_PROGRESS_SECS="${PRIORITY_AGENT_LIVE_EVAL_NO_EFFECTIVE_PROGRESS_SECS:-0}"
MIN_FREE_GB="${PRIORITY_AGENT_LIVE_EVAL_MIN_FREE_GB:-8}"
OVERLAY_WORKTREE="${PRIORITY_AGENT_LIVE_EVAL_OVERLAY_WORKTREE:-0}"
SKIP_PROVIDER_HEALTH="${PRIORITY_AGENT_LIVE_EVAL_SKIP_PROVIDER_HEALTH:-0}"

RECOMMENDED_CASES=(
  code-change-verification-repair-loop
  live-eval-dashboard-summary
  backend-todo-api-crud
  frontend-book-notes-localstorage
  memory-save-quality-gate
  skill-promotion-gate
  persistent-memory-planning-context
  memory-recall-conflict-precision
  memory-failure-lesson-promotion
  memory-stale-project-fact-demotion
  memory-save-sensitive-hard-block
  permission-default-open-dangerous-guard
  resume-session-picker
  cli-scrollback-polish
)

CORE_CODING_QUALITY_CASES=(
  core-inspection-grounding
  core-simple-stale-edit
  core-multi-file-edit
  core-terminal-install-run
  core-long-output-artifact
  core-provider-roundtrip
  core-permission-rejection-recovery
  core-rollback-product-path
)

REAL_PROJECT_CODING_GAUNTLET_CASES=(
  backend-todo-api-crud
  frontend-book-notes-localstorage
  code-change-verification-repair-loop
  core-inspection-grounding
  core-simple-stale-edit
  core-multi-file-edit
  core-terminal-install-run
  core-long-output-artifact
  core-provider-roundtrip
  core-permission-rejection-recovery
  core-rollback-product-path
  live-eval-dashboard-summary
  memory-save-quality-gate
  skill-promotion-gate
  persistent-memory-planning-context
  memory-failure-lesson-promotion
  memory-stale-project-fact-demotion
)

RELEASE_DOGFOOD_CASES=(
  core-simple-stale-edit
  core-rust-multi-file-refactor
  desktop-ui-smoke-polish
  code-change-verification-repair-loop
  core-permission-rejection-recovery
  core-long-output-artifact
)

MVP_WEIGHTED_AGENT_CASES=(
  minimum-agent-direct-answer
  minimum-agent-light-inspection
  minimum-agent-loop
  minimum-agent-verification-repair
  minimum-agent-high-risk-block
  minimum-agent-low-value-replan
  minimum-agent-memory-boundary
)

PROJECT_PARTNER_DEMO_CASES=(
  project-partner-vague-local-tool
  project-partner-resume-with-memory
  project-partner-failure-memory-proposal
)

RUNTIME_SPINE_P0B_CASES=(
  runtime-spine-p0b-permission-required
  runtime-spine-p0b-test-failure-repair
  runtime-spine-p0b-route-mistake-recovery
  runtime-spine-p0b-subagent-verifier
  runtime-spine-p0b-isolated-worktree-implementer
  runtime-spine-p0b-memory-retrieval-conflict
  runtime-spine-p0b-skill-guidance
)

usage() {
  cat <<'EOF'
Usage:
  scripts/run_live_eval.sh --list
  scripts/run_live_eval.sh --case <id|recommended|core-coding-quality|real-project-coding|release-dogfood|mvp-weighted-agent|project-partner-demo|runtime-spine-p0b|all> --mode <prepare|api-plan|agent-run|collect|full> [options]
  scripts/run_live_eval.sh --mode summary --run-id <id>

Modes:
  list      List live task samples.
  prepare   Create a git worktree, prompt.txt, and RUNBOOK.md.
  api-plan  Prepare a worktree, start the API server, and ask MiniMax for a plan.
  agent-run Prepare a worktree, run Priority Agent non-interactively, then collect.
  collect   Collect diff/test output from an existing worktree.
  full      prepare + api-plan + optional collect.
  summary   Generate docs/benchmarks/live-<run-id>/summary.md.

Options:
  --case ID          Live task id, "recommended", "core-coding-quality",
                     "real-project-coding", "release-dogfood",
                     "mvp-weighted-agent", "project-partner-demo",
                     "runtime-spine-p0b", or "all".
                     With --list, a suite name lists only that suite.
  --mode MODE        list, prepare, api-plan, agent-run, collect, or full.
  --workdir DIR      Existing task worktree for collect mode.
  --label LABEL      Report/run label (default: live-eval).
  --run-id ID        Stable run id (default: timestamp).
  --run-tests        Run acceptance.required_commands plus harness_commands during collect/agent-run/full.
  --skip-build       Reuse target/release/priority-agent for api-plan.
  --skip-provider-health
                     Skip provider health preflight before agent-run.
  --overlay-working-tree
                     Apply tracked local working-tree changes to the isolated
                     worktree and commit them as the task baseline.
  --timeout SECS     Wall-clock timeout for agent-run mode. 0 disables it
                     (default: 0; rely on idle/liveness monitoring).
  --idle-timeout SECS
                     Kill agent-run if output/events/stderr stay idle. 0 disables it
                     (default: 0; monitor log still records liveness).
  --no-effective-progress-timeout SECS
                     Kill agent-run if no worktree diff appears for SECS. 0 disables it
                     (default: 0; intended for repair-loop slow-tail checks).
  -h, --help         Show this help.

MiniMax:
  api-plan/full intentionally require MINIMAX_API_KEY. The script starts the
  local API server with MiniMax as the provider and writes the LLM planning
  response to the live eval report directory.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --list) MODE="list"; shift ;;
    --case) CASE_ID="${2:-}"; shift 2 ;;
    --mode) MODE="${2:-}"; shift 2 ;;
    --workdir) WORKDIR="${2:-}"; shift 2 ;;
    --label) LABEL="${2:-}"; shift 2 ;;
    --run-id) RUN_ID="${2:-}"; RUN_ID_PROVIDED=1; shift 2 ;;
    --run-tests) RUN_TESTS=1; shift ;;
    --skip-build) SKIP_BUILD=1; shift ;;
    --skip-provider-health) SKIP_PROVIDER_HEALTH=1; shift ;;
    --overlay-working-tree) OVERLAY_WORKTREE=1; shift ;;
    --timeout) AGENT_TIMEOUT_SECS="${2:-}"; shift 2 ;;
    --idle-timeout) AGENT_IDLE_SECS="${2:-}"; shift 2 ;;
    --no-effective-progress-timeout) AGENT_NO_EFFECTIVE_PROGRESS_SECS="${2:-}"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown option: $1" >&2; usage; exit 1 ;;
  esac
done

if [[ -z "$RUN_ID" && "$MODE" != "summary" ]]; then
  RUN_ID="${LABEL}-$(date +%Y%m%d-%H%M%S)"
fi

need_yaml() {
  ruby -e 'require "yaml"; require "json"' >/dev/null 2>&1 || {
    echo "Ruby with yaml/json stdlib is required for live eval parsing." >&2
    exit 1
  }
}

yaml_get() {
  local file="$1" path="$2" default="${3:-}"
  ruby -ryaml -rjson -e '
    file, path, default = ARGV
    cur = YAML.load_file(file) || {}
    path.split(".").each do |part|
      if cur.is_a?(Hash) && cur.key?(part)
        cur = cur[part]
      else
        puts default
        exit 0
      end
    end
    if cur.nil?
      puts default
    elsif cur.is_a?(Hash) || cur.is_a?(Array)
      puts JSON.generate(cur)
    else
      puts cur
    end
  ' "$file" "$path" "$default"
}

yaml_list() {
  local file="$1" path="$2"
  ruby -ryaml -e '
    file, path = ARGV
    cur = YAML.load_file(file) || {}
    path.split(".").each do |part|
      cur = cur.is_a?(Hash) ? (cur[part] || {}) : {}
    end
    if cur.is_a?(Array)
      cur.each { |item| puts item }
    end
  ' "$file" "$path"
}

validation_commands() {
  local file="$1"
  yaml_list "$file" acceptance.required_commands
  yaml_list "$file" acceptance.harness_commands
}

write_agent_prompt() {
  local file="$1" out="$2"
  ruby -ryaml -e '
    sample_path, out_path = ARGV
    sample = YAML.load_file(sample_path) || {}
    acceptance = sample["acceptance"] || {}
    diff_constraints = acceptance["diff_constraints"] || {}
    eval_intent = sample.fetch("eval_intent", "seeded_code_change").to_s.strip
    task_heading = case eval_intent
    when "direct_answer"
      "Direct answer regression task"
    when "read_only_audit"
      "Read-only local evidence task"
    else
      "Live coding regression task"
    end
    list_block = lambda do |title, values, empty = "(none)"|
      lines = ["## #{title}"]
      values = [] unless values.is_a?(Array)
      if values.empty?
        lines << empty
      else
        values.each { |item| lines << "- #{item}" }
      end
      lines
    end
    lines = [
      "# #{task_heading}: #{sample["title"] || sample["id"] || "unknown"}",
      "",
      "- Task id: `#{sample["id"] || "unknown"}`",
      "- Type: `#{sample["type"] || "unknown"}`",
      "- Eval intent: `#{sample["eval_intent"] || "seeded_code_change"}`",
      "- Risk: `#{sample["risk"] || "unknown"}`",
      "- Complexity: `#{sample["complexity"] || "unknown"}`",
      "",
      "## User task",
      "",
      sample["prompt"].to_s.strip,
      "",
    ]
    lines.concat(list_block.call("Allowed tools", sample["allowed_tools"] || []))
    lines << ""
    lines.concat(list_block.call("Forbidden tools", sample["forbidden_tools"] || []))
    lines << ""
    lines.concat(list_block.call("Expected behavior", sample["expected_behavior"] || []))
    lines << ""
    lines.concat(["## Acceptance checks", ""])
    required = acceptance["required_commands"] || []
    if required.empty?
      lines << "No required validation command is part of this eval."
      lines << "- (none)"
    else
      lines << "Before your final response, run every required command below. If any command fails, inspect the failure, repair the code, and rerun the relevant command. Do not claim completion while required commands are failing."
      required.each { |cmd| lines << "- `#{cmd}`" }
    end
    harness = acceptance["harness_commands"] || []
    unless harness.empty?
      lines << ""
      lines << "Additional harness-only checks will run after your turn; do not spend the agent loop on those unless a focused failure points there."
    end
    lines.concat([
      "",
      "## Diff constraints",
      "",
      "- Max files changed: `#{diff_constraints["max_files_changed"] || "unspecified"}`",
    ])
    (diff_constraints["forbidden_paths"] || []).each do |forbidden|
      lines << "- Do not change path: `#{forbidden}`"
    end
    lines.concat(["", "## Closeout requirements", ""])
    case eval_intent
    when "direct_answer"
      lines << "- This is a direct-answer evaluation. Do not use tools unless the task explicitly asks for them; answer the user request directly and close out with no file changes."
    when "read_only_audit"
      lines << "- This is a read-only local evidence evaluation. Use the smallest relevant inspection, do not edit files, and answer from observed facts."
    when "audit_or_regression_check"
      lines << "- This is an audit/regression evaluation. If the requested behavior is already present, prove it with direct evidence and required commands instead of forcing an arbitrary edit."
    when "stale_or_already_satisfied"
      lines << "- This case may already be satisfied on the current baseline. Do not force an arbitrary edit; prove the current state and call out stale-baseline risk clearly."
    else
      lines << "- This is a real code-change evaluation in an isolated worktree. Do not stop at investigation."
    end
    case eval_intent
    when "direct_answer"
      lines << "- Do not inspect the repository for this task. Provide the direct answer and a concise Closeout."
    when "read_only_audit"
      lines << "- Inspect only the smallest relevant path or query; after at most 3 read-only inspections, close out with no changes."
    when "audit_or_regression_check", "stale_or_already_satisfied"
      lines << "- Inspect only the smallest set of relevant files first; after at most 3 read-only inspections, run the required validation commands and close out with no changes if the requested behavior is already present. Make a focused edit only when a concrete missing behavior is proven."
    else
      lines << "- Inspect only the smallest set of relevant files first; after at most 3 read-only inspections, either make a focused edit or clearly state the concrete blocker."
    end
    if eval_intent == "direct_answer"
      lines.concat([
        "- Summarize that no files changed.",
        "- Mention that no validation command was required.",
        "- The final response must include a `Closeout:` section.",
      ])
    elsif eval_intent == "read_only_audit"
      lines.concat([
        "- Summarize the observed evidence.",
        "- Summarize that no files changed.",
        "- Mention that no validation command was required unless you actually ran one.",
        "- The final response must include a `Closeout:` section.",
      ])
    else
      lines.concat([
        "- If the code is already fixed, prove it with the required commands and still provide a Closeout.",
        "- Summarize files changed and why.",
        "- List validation commands you ran and their pass/fail status.",
        "- Mention any remaining risk or blocker explicitly.",
        "- The final response must include a `Closeout:` section.",
      ])
    end
    File.write(out_path, lines.join("\n").rstrip + "\n")
  ' "$file" "$out"
}

json_payload() {
  local prompt_file="$1" system_file="$2" context_file="$3"
  python3 - "$prompt_file" "$system_file" "$context_file" <<'PY'
import json, sys
prompt = open(sys.argv[1], encoding="utf-8").read()
system = open(sys.argv[2], encoding="utf-8").read()
context = open(sys.argv[3], encoding="utf-8").read()
message = prompt
if context.strip():
    message += "\n\n---\nRepository context for planning:\n" + context
print(json.dumps({
    "message": message,
    "system_prompt": system,
    "stream": False,
    "temperature": 0.2,
}, ensure_ascii=False))
PY
}

task_keywords() {
  local file="$1"
  ruby -ryaml -e '
    data = YAML.load_file(ARGV[0]) || {}
    text = %w[id title type prompt].map { |key| data[key].to_s }.join("\n")
    stop = %w[this that with from into should must only when true false mode task code file files test tests 要求 修复 问题 新增 更新 项目 任务 代码 测试]
    terms = []
    text.scan(/[A-Za-z_][A-Za-z0-9_]{3,}|[\u4e00-\u9fff]{2,}/).each do |term|
      normalized = term.strip.downcase
      next if stop.include?(normalized)
      terms << normalized unless terms.include?(normalized)
    end
    puts terms.first(14)
  ' "$file"
}

build_repo_context() {
  local file="$1" task_workdir="$2" out="$3"
  local id title
  id="$(yaml_get "$file" id)"
  title="$(yaml_get "$file" title)"
  {
    echo "Task id: $id"
    echo "Task title: $title"
    echo "Repo language: Rust"
    echo "Current ref: $(git -C "$task_workdir" rev-parse --short HEAD 2>/dev/null || echo unknown)"
    echo
    echo "High-signal repository files:"
    (cd "$task_workdir" && find src -name '*.rs' -type f | sort | sed -n '1,180p') 2>/dev/null || true
    echo
    echo "Keyword hits:"
    local term
    while IFS= read -r term; do
      [[ -z "$term" ]] && continue
      echo
      echo "## $term"
      (cd "$task_workdir" && rg -n -m 8 --glob '*.rs' --glob '*.md' "$term" src docs Cargo.toml 2>/dev/null | sed -n '1,12p') || true
    done < <(task_keywords "$file")
  } >"$out"
}

task_env_base() {
  local id="$1"
  echo "$ROOT_DIR/$WORK_ROOT/$RUN_ID/$id/env"
}

task_cargo_target_dir() {
  local id="$1"
  if [[ -n "${PRIORITY_AGENT_LIVE_EVAL_CARGO_TARGET_DIR:-}" ]]; then
    echo "$PRIORITY_AGENT_LIVE_EVAL_CARGO_TARGET_DIR"
  else
    echo "$ROOT_DIR/$WORK_ROOT/$RUN_ID/shared-cargo-target"
  fi
}

available_kb_for_path() {
  local path="$1"
  mkdir -p "$path"
  df -Pk "$path" | awk 'NR == 2 { print $4 }'
}

ensure_live_eval_disk_space() {
  local path="${1:-$ROOT_DIR}" min_gb="${MIN_FREE_GB:-0}" available_kb min_kb
  if [[ "$min_gb" == "0" || "$min_gb" == "0.0" ]]; then
    return 0
  fi
  available_kb="$(available_kb_for_path "$path")"
  min_kb="$(python3 - "$min_gb" <<'PY'
import sys
print(int(float(sys.argv[1]) * 1024 * 1024))
PY
)"
  if [[ -z "$available_kb" || "$available_kb" -lt "$min_kb" ]]; then
    python3 - "$available_kb" "$min_kb" "$path" <<'PY' >&2
import sys
available_kb = int(sys.argv[1] or 0)
min_kb = int(sys.argv[2])
path = sys.argv[3]
print(
    "live-eval disk preflight failed: "
    f"{available_kb / 1024 / 1024:.1f}GiB available under {path}, "
    f"requires at least {min_kb / 1024 / 1024:.1f}GiB. "
    "Clear target/live-evals or lower PRIORITY_AGENT_LIVE_EVAL_MIN_FREE_GB."
)
PY
    return 1
  fi
}

ensure_task_env() {
  local id="$1" env_base
  env_base="$(task_env_base "$id")"
  mkdir -p \
    "$env_base/home" \
    "$env_base/xdg-config" \
    "$env_base/xdg-data" \
    "$env_base/xdg-state" \
    "$(task_cargo_target_dir "$id")"
}

find_free_port() {
  python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()'
}

source "$ROOT_DIR/scripts/run_live_eval_tasks.sh"

resolve_ref() {
  local requested="$1"
  if git rev-parse --verify --quiet "${requested}^{commit}" >/dev/null; then
    echo "$requested"
  else
    echo "HEAD"
  fi
}

overlay_working_tree_changes() {
  local task_workdir="$1"
  local patch_file="$2"

  git diff --binary HEAD -- . >"$patch_file"
  if [[ ! -s "$patch_file" ]]; then
    return 0
  fi

  if ! git -C "$task_workdir" apply --whitespace=nowarn "$patch_file"; then
    echo "Failed to apply working-tree overlay patch: $patch_file" >&2
    return 1
  fi
  if [[ -n "$(git -C "$task_workdir" status --short)" ]]; then
    git -C "$task_workdir" add -A
    if ! git -C "$task_workdir" \
      -c user.name="Priority Agent Live Eval" \
      -c user.email="priority-agent-live-eval@example.invalid" \
      commit -m "live eval working tree overlay" >/dev/null
    then
      echo "Failed to commit working-tree overlay baseline in $task_workdir" >&2
      return 1
    fi
  fi
}

prepare_task() {
  local file="$1"
  local id title base_ref resolved_ref task_workdir prompt_file runbook metadata env_base prepare_log overlay_patch runtime_profile
  id="$(yaml_get "$file" id)"
  title="$(yaml_get "$file" title)"
  runtime_profile="$(yaml_get "$file" runtime_profile "")"
  base_ref="$(yaml_get "$file" repo.base_ref HEAD)"
  resolved_ref="$(resolve_ref "$base_ref")"
  task_workdir="$WORK_ROOT/$RUN_ID/$id/worktree"
  prompt_file="$WORK_ROOT/$RUN_ID/$id/prompt.txt"
  runbook="$WORK_ROOT/$RUN_ID/$id/RUNBOOK.md"
  metadata="$WORK_ROOT/$RUN_ID/$id/metadata.json"
  prepare_log="$WORK_ROOT/$RUN_ID/$id/prepare-commands.log"
  overlay_patch="$ROOT_DIR/$WORK_ROOT/$RUN_ID/$id/working-tree-overlay.patch"
  env_base="$(task_env_base "$id")"

  mkdir -p "$(dirname "$task_workdir")"
  ensure_task_env "$id"
  if [[ ! -d "$task_workdir/.git" && ! -f "$task_workdir/.git" ]]; then
    git worktree add --force --detach "$task_workdir" "$resolved_ref" >/dev/null
  fi

  if [[ "$OVERLAY_WORKTREE" -eq 1 ]]; then
    if ! overlay_working_tree_changes "$task_workdir" "$overlay_patch"; then
      exit 1
    fi
  fi

  if ! ruby -ryaml -e '
sample_path, workdir, log_path = ARGV
sample = YAML.load_file(sample_path) || {}
commands = (sample["repo"] || {})["prepare_commands"] || []
File.open(log_path, "w") do |log|
  commands.each do |command|
    command = command.to_s
    next if command.strip.empty?
    log.write("$ #{command}\n")
    log.flush
    system(command, chdir: workdir, out: log, err: [:child, :out])
    status = $?.exitstatus || 1
    log.write("[exit status: #{status}]\n\n")
    log.flush
    exit(status) if status != 0
  end
end
' "$file" "$task_workdir" "$prepare_log"
  then
    echo "Prepare command failed for $id. See $prepare_log" >&2
    exit 1
  fi

  if [[ -n "$(git -C "$task_workdir" status --short)" ]]; then
    git -C "$task_workdir" add -A
    git -C "$task_workdir" \
      -c user.name="Priority Agent Live Eval" \
      -c user.email="priority-agent-live-eval@example.invalid" \
      commit -m "live eval fixture setup: $id" >>"$prepare_log" 2>&1
  fi

  write_agent_prompt "$file" "$prompt_file"
  ruby -ryaml -rjson -e '
sample_path, metadata_path, workdir, resolved_ref, env_base = ARGV
sample = YAML.load_file(sample_path) || {}
sample["_sample_path"] = sample_path
sample["_workdir"] = workdir
sample["_resolved_ref"] = resolved_ref
sample["_env_base"] = env_base
File.write(metadata_path, JSON.pretty_generate(sample) + "\n")
' "$file" "$metadata" "$task_workdir" "$resolved_ref" "$env_base"

  {
    echo "# Live Eval Runbook: $id"
    echo
    echo "- Title: $title"
    echo "- Sample: $file"
    echo "- Requested base ref: $base_ref"
    echo "- Resolved base ref: $resolved_ref"
    echo "- Worktree: $task_workdir"
    echo "- Isolated env: $env_base"
    if [[ -n "$runtime_profile" ]]; then
      echo "- Runtime profile: $runtime_profile"
    fi
    echo "- MiniMax model: ${MINIMAX_MODEL:-MiniMax-M2.7}"
    if [[ -s "$prepare_log" ]]; then
      echo "- Prepare log: $prepare_log"
    fi
    echo
    echo "## Prompt"
    echo
    cat "$prompt_file"
    echo
    echo "## Required Commands"
    echo
    local cmd
    while IFS= read -r cmd; do
      [[ -z "$cmd" ]] && continue
      echo "- \`$cmd\`"
    done < <(yaml_list "$file" acceptance.required_commands)
    if [[ -n "$(yaml_list "$file" acceptance.harness_commands | sed '/^[[:space:]]*$/d')" ]]; then
      echo
      echo "## Harness-Only Commands"
      echo
      while IFS= read -r cmd; do
        [[ -z "$cmd" ]] && continue
        echo "- \`$cmd\`"
      done < <(yaml_list "$file" acceptance.harness_commands)
    fi
    echo
    echo "## Manual Agent Command"
    echo
    echo '```bash'
    echo "cd \"$task_workdir\""
    echo "mkdir -p \"$env_base/home\" \"$env_base/xdg-config\" \"$env_base/xdg-data\" \"$env_base/xdg-state\""
    echo "HOME=\"$env_base/home\" \\"
    echo "XDG_CONFIG_HOME=\"$env_base/xdg-config\" \\"
    echo "XDG_DATA_HOME=\"$env_base/xdg-data\" \\"
    echo "XDG_STATE_HOME=\"$env_base/xdg-state\" \\"
    echo "PRIORITY_AGENT_A2A_TRANSCRIPT_PATH=\"$env_base/a2a-transcript.jsonl\" \\"
    if [[ "$runtime_profile" == "minimum_viable_agent" || "$runtime_profile" == "mva" ]]; then
      echo "PRIORITY_AGENT_RUNTIME_PROFILE=\"$runtime_profile\" \\"
      echo "PRIORITY_AGENT_MVA_AUDIT_TOOLS=\"1\" \\"
      echo "PRIORITY_AGENT_CANDIDATE_ACTIONS=\"shadow\" \\"
      echo "PRIORITY_AGENT_MVA_MAX_TOOL_CALLS=\"10\" \\"
      echo "PRIORITY_AGENT_MVA_PARALLELISM_LIMIT=\"1\" \\"
    elif [[ -n "$runtime_profile" ]]; then
      echo "PRIORITY_AGENT_RUNTIME_PROFILE=\"$runtime_profile\" \\"
    fi
    echo "MINIMAX_API_KEY=\"\${MINIMAX_API_KEY:?}\" \\"
    echo "MINIMAX_BASE_URL=\"\${MINIMAX_BASE_URL:-}\" \\"
    echo "MINIMAX_MODEL=\"\${MINIMAX_MODEL:-MiniMax-M2.7}\" \\"
    echo "OPENAI_API_KEY=\"\" \\"
    echo "MOONSHOT_API_KEY=\"\" \\"
    echo "\"$ROOT_DIR/target/release/priority-agent\""
    echo '```'
    echo
    echo "After the agent run, collect results with:"
    echo
    echo '```bash'
    echo "scripts/run_live_eval.sh --case $id --mode collect --workdir \"$task_workdir\" --run-tests"
    echo '```'
  } >"$runbook"

  echo "$task_workdir"
}

build_binary() {
  if [[ "$SKIP_BUILD" -eq 0 ]]; then
    cargo build --release --features experimental-api-server >/dev/null
  fi
  if [[ ! -x "$ROOT_DIR/target/release/priority-agent" ]]; then
    echo "Missing binary: $ROOT_DIR/target/release/priority-agent" >&2
    exit 1
  fi
}

api_plan_task() {
  local file="$1" task_workdir="$2"
  local id report_dir server_log system_file response_file plan_file payload_file context_file headers_file lint_file env_base port server_pid ready
  id="$(yaml_get "$file" id)"
  report_dir="$REPORT_DIR/live-$RUN_ID/$id"
  mkdir -p "$report_dir"
  server_log="$report_dir/api-server.log"
  system_file="$report_dir/system-prompt.txt"
  response_file="$report_dir/api-response.json"
  plan_file="$report_dir/minimax-plan.md"
  payload_file="$report_dir/request.json"
  context_file="$report_dir/repo-context.txt"
  headers_file="$report_dir/api-response.headers"
  lint_file="$report_dir/plan-lint.txt"
  env_base="$(task_env_base "$id")"

  if [[ -z "${MINIMAX_API_KEY:-}" ]]; then
    echo "MINIMAX_API_KEY is required for api-plan/full mode." >&2
    exit 1
  fi

  build_binary
  ensure_task_env "$id"

  cat >"$system_file" <<'EOF'
You are evaluating Priority Agent on a live coding regression task.
Use MiniMax as the model under test.
This repository is a Rust coding-agent project, not a Python project.
This API endpoint cannot execute tools. Do not emit TOOL_CALL blocks, XML tags,
<think> blocks, hidden reasoning tags, or pretend that files were inspected or
edited.
Do not claim that you edited files. Produce a concise engineering plan only:
1. classify task type/risk,
2. identify first blocker,
3. list likely files to inspect,
4. list acceptance checks,
5. note memory/skill/evolution risks if relevant.
Any XML-like tag, pseudo tool call, or "let me inspect/run/edit" action text is
an evaluation failure for this plan-only mode. Stop after the plan.
Do not include an implementation preamble, a "ready to proceed" sentence, or a
closing sentence about running commands. The response must end immediately after
the fifth plan section.
If you are uncertain, say what should be inspected next instead of fabricating
tool results.
EOF

  build_repo_context "$file" "$task_workdir" "$context_file"
  json_payload "$WORK_ROOT/$RUN_ID/$id/prompt.txt" "$system_file" "$context_file" >"$payload_file"
  port="$(find_free_port)"
  (
    cd "$task_workdir"
    env \
      HOME="$env_base/home" \
      XDG_CONFIG_HOME="$env_base/xdg-config" \
      XDG_DATA_HOME="$env_base/xdg-data" \
      XDG_STATE_HOME="$env_base/xdg-state" \
      PRIORITY_AGENT_A2A_TRANSCRIPT_PATH="$env_base/a2a-transcript.jsonl" \
      MINIMAX_API_KEY="$MINIMAX_API_KEY" \
      MINIMAX_BASE_URL="${MINIMAX_BASE_URL:-}" \
      MINIMAX_MODEL="${MINIMAX_MODEL:-MiniMax-M2.7}" \
      OPENAI_API_KEY="" \
      MOONSHOT_API_KEY="" \
      "$ROOT_DIR/target/release/priority-agent" --api --port "$port"
  ) >"$server_log" 2>&1 &
  server_pid="$!"

  cleanup_server() {
    if kill -0 "$server_pid" >/dev/null 2>&1; then
      kill "$server_pid" >/dev/null 2>&1 || true
      wait "$server_pid" >/dev/null 2>&1 || true
    fi
  }

  ready=0
  for _ in {1..80}; do
    if curl -fsS "http://127.0.0.1:$port/api/health" >/dev/null 2>&1; then
      ready=1
      break
    fi
    sleep 0.25
  done
  if [[ "$ready" -ne 1 ]]; then
    cleanup_server
    echo "API server did not become healthy. See $server_log" >&2
    exit 1
  fi

  local curl_status=0 http_status
  curl -sS -D "$headers_file" -o "$response_file" \
    -X POST "http://127.0.0.1:$port/api/chat" \
    -H "Content-Type: application/json" \
    --data-binary "@$payload_file" || curl_status=$?
  cleanup_server
  if [[ "$curl_status" -ne 0 ]]; then
    echo "API chat request failed. See $server_log and $payload_file" >&2
    exit "$curl_status"
  fi
  http_status="$(awk 'NR==1{print $2}' "$headers_file")"
  if [[ ! "$http_status" =~ ^2[0-9][0-9]$ ]]; then
    echo "API chat request returned HTTP $http_status. See $response_file, $headers_file, and $payload_file" >&2
    exit 1
  fi

  python3 - "$response_file" "$plan_file" "$lint_file" <<'PY'
import json, sys
with open(sys.argv[1], encoding="utf-8") as fh:
    data = json.load(fh)
content = data.get("content", "")
with open(sys.argv[2], "w", encoding="utf-8") as fh:
    fh.write(content)
    if not content.endswith("\n"):
        fh.write("\n")

forbidden = ["<think", "</think>", "<thinking", "</thinking>", "TOOL_CALL", "<tool_call"]
forbidden_phrases = [
    "let me start",
    "let me inspect",
    "let me run",
    "let me edit",
    "ready to proceed with implementation",
]
lower_content = content.lower()
violations = [marker for marker in forbidden if marker.lower() in lower_content]
violations.extend(
    f"action_text:{phrase}" for phrase in forbidden_phrases if phrase in lower_content
)
with open(sys.argv[3], "w", encoding="utf-8") as fh:
    if violations:
        fh.write("status=failed\n")
        fh.write("reason=forbidden markers in API plan output\n")
        for marker in violations:
            fh.write(f"violation={marker}\n")
    else:
        fh.write("status=ok\n")
PY
  if grep -q '^status=failed' "$lint_file"; then
    echo "MiniMax plan failed lint. See $lint_file and $plan_file" >&2
    exit 1
  fi

  echo "$plan_file"
}

provider_health_enabled() {
  [[ "$SKIP_PROVIDER_HEALTH" != "1" && "${PRIORITY_AGENT_LIVE_EVAL_PROVIDER_HEALTH:-1}" != "0" ]]
}

provider_health_preflight() {
  local env_base="$1"
  if ! provider_health_enabled; then
    return 0
  fi

  local run_report_dir status_file health_json health_stdout health_stderr status
  run_report_dir="$REPORT_DIR/live-$RUN_ID"
  status_file="$run_report_dir/provider-health-status.txt"
  health_json="$run_report_dir/provider-health.json"
  health_stdout="$run_report_dir/provider-health-stdout.log"
  health_stderr="$run_report_dir/provider-health-stderr.log"
  mkdir -p "$run_report_dir" "$env_base/home" "$env_base/xdg-config" "$env_base/xdg-data" "$env_base/xdg-state"

  if [[ -f "$status_file" ]]; then
    [[ "$(cat "$status_file")" == "ok" ]]
    return $?
  fi

  (
    set +e
    HOME="$env_base/home" \
      XDG_CONFIG_HOME="$env_base/xdg-config" \
      XDG_DATA_HOME="$env_base/xdg-data" \
      XDG_STATE_HOME="$env_base/xdg-state" \
      MINIMAX_API_KEY="${MINIMAX_API_KEY:-}" \
      MINIMAX_BASE_URL="${MINIMAX_BASE_URL:-}" \
      MINIMAX_MODEL="${MINIMAX_MODEL:-MiniMax-M2.7}" \
      OPENAI_API_KEY="" \
      MOONSHOT_API_KEY="" \
      "$ROOT_DIR/target/release/priority-agent" \
        --provider-health \
        --output "$ROOT_DIR/$health_json" \
        --timeout "${PRIORITY_AGENT_PROVIDER_HEALTH_TIMEOUT_SECS:-45}" \
        >"$health_stdout" 2>"$health_stderr"
    status=$?
    if [[ "$status" -eq 0 ]]; then
      echo ok >"$status_file"
    else
      echo failed >"$status_file"
    fi
    exit "$status"
  )
}

write_provider_health_agent_failure() {
  local id="$1" task_workdir="$2" report_dir="$3" agent_stdout="$4" agent_stderr="$5" agent_output="$6" agent_events="$7" exit_file="$8" prompt_file="$9"
  local run_report_dir health_json health_stderr
  run_report_dir="$REPORT_DIR/live-$RUN_ID"
  health_json="$run_report_dir/provider-health.json"
  health_stderr="$run_report_dir/provider-health-stderr.log"

  : >"$agent_stdout"
  : >"$agent_output"
  {
    echo "provider unavailable: provider health preflight failed before agent-run"
    echo "Health status: $run_report_dir/provider-health-status.txt"
    echo "Health report: $health_json"
    if [[ -s "$health_stderr" ]]; then
      echo
      cat "$health_stderr"
    fi
  } >"$agent_stderr"
  echo 126 >"$exit_file"
  touch "$report_dir/provider-health-preflight-failed"
  if [[ -f "$health_json" ]]; then
    cp "$health_json" "$report_dir/provider-health.json"
  fi

  python3 - "$agent_events" "$prompt_file" "$task_workdir" "$health_json" "$id" <<'PY'
import json
import pathlib
import sys

events_path = pathlib.Path(sys.argv[1])
prompt_file = sys.argv[2]
cwd = sys.argv[3]
health_json = sys.argv[4]
task_id = sys.argv[5]
events_path.parent.mkdir(parents=True, exist_ok=True)
events = [
    {
        "event": "eval_started",
        "prompt_file": prompt_file,
        "cwd": cwd,
        "model": "provider-health-preflight",
    },
    {
        "event": "provider_health_preflight",
        "status": "failed",
        "task_id": task_id,
        "report": health_json,
    },
    {
        "event": "error",
        "message": "provider unavailable: provider health preflight failed before agent-run",
    },
]
with events_path.open("w", encoding="utf-8") as fh:
    for event in events:
        fh.write(json.dumps(event, ensure_ascii=False) + "\n")
PY
}

agent_run_task() {
  local file="$1" task_workdir="$2"
  local id report_dir agent_stdout agent_stderr agent_monitor agent_metrics agent_output agent_events exit_file prompt_file env_base cargo_target_dir runtime_profile
  id="$(yaml_get "$file" id)"
  runtime_profile="$(yaml_get "$file" runtime_profile "")"
  report_dir="$REPORT_DIR/live-$RUN_ID/$id"
  mkdir -p "$report_dir"
  agent_stdout="$report_dir/agent-stdout.log"
  agent_stderr="$report_dir/agent-stderr.log"
  agent_monitor="$report_dir/agent-monitor.log"
  agent_metrics="$report_dir/agent-run-metrics.json"
  agent_output="$ROOT_DIR/$report_dir/agent-output.md"
  agent_events="$ROOT_DIR/$report_dir/agent-events.jsonl"
  exit_file="$report_dir/agent-exit-status.txt"
  prompt_file="$ROOT_DIR/$WORK_ROOT/$RUN_ID/$id/prompt.txt"
  env_base="$(task_env_base "$id")"
  cargo_target_dir="$(task_cargo_target_dir "$id")"

  if [[ -z "${MINIMAX_API_KEY:-}" ]]; then
    echo "MINIMAX_API_KEY is required for agent-run mode." >&2
    return 1
  fi

  build_binary
  ensure_task_env "$id"
  if ! provider_health_preflight "$env_base"; then
    write_provider_health_agent_failure \
      "$id" \
      "$task_workdir" \
      "$report_dir" \
      "$agent_stdout" \
      "$agent_stderr" \
      "$agent_output" \
      "$agent_events" \
      "$exit_file" \
      "$prompt_file"
    return 126
  fi

  python3 - \
    "$AGENT_TIMEOUT_SECS" \
    "$AGENT_IDLE_SECS" \
    "$AGENT_NO_EFFECTIVE_PROGRESS_SECS" \
    "$AGENT_MONITOR_INTERVAL_SECS" \
    "$task_workdir" \
    "$agent_stdout" \
    "$agent_stderr" \
    "$agent_monitor" \
    "$agent_metrics" \
    "$ROOT_DIR/target/release/priority-agent" \
    "$prompt_file" \
    "$agent_output" \
    "$agent_events" \
    "$env_base" \
    "$cargo_target_dir" \
    "$runtime_profile" <<'PY' >"$exit_file"
import os
import json
import subprocess
import sys
import time

timeout = int(sys.argv[1])
idle_timeout = int(sys.argv[2])
no_effective_progress_timeout = int(sys.argv[3])
monitor_interval = int(sys.argv[4])
workdir = sys.argv[5]
stdout_path = sys.argv[6]
stderr_path = sys.argv[7]
monitor_path = sys.argv[8]
metrics_path = sys.argv[9]
binary = sys.argv[10]
prompt_file = sys.argv[11]
output_file = sys.argv[12]
events_file = sys.argv[13]
env_base = sys.argv[14]
cargo_target_dir = sys.argv[15]
runtime_profile = sys.argv[16]

env = os.environ.copy()
real_home = env.get("HOME", "")
env.update({
    "HOME": os.path.join(env_base, "home"),
    "XDG_CONFIG_HOME": os.path.join(env_base, "xdg-config"),
    "XDG_DATA_HOME": os.path.join(env_base, "xdg-data"),
    "XDG_STATE_HOME": os.path.join(env_base, "xdg-state"),
    "CARGO_HOME": os.environ.get("CARGO_HOME") or os.path.join(real_home, ".cargo"),
    "RUSTUP_HOME": os.environ.get("RUSTUP_HOME") or os.path.join(real_home, ".rustup"),
    "CARGO_TARGET_DIR": cargo_target_dir,
    "PRIORITY_AGENT_A2A_TRANSCRIPT_PATH": os.path.join(env_base, "a2a-transcript.jsonl"),
    "PRIORITY_AGENT_EVAL_EVENTS": events_file,
    "PRIORITY_AGENT_LEGACY_WORKFLOW_ENABLED": os.environ.get("PRIORITY_AGENT_LEGACY_WORKFLOW_ENABLED", os.environ.get("PRIORITY_AGENT_WORKFLOW_ENABLED", "0")),
    "PRIORITY_AGENT_WORKFLOW_ENABLED": os.environ.get("PRIORITY_AGENT_WORKFLOW_ENABLED", "0"),
    "PRIORITY_AGENT_WORKFLOW_CONTRACT": os.environ.get("PRIORITY_AGENT_WORKFLOW_CONTRACT", "1"),
    "PRIORITY_AGENT_CLOSEOUT_VISIBILITY": os.environ.get("PRIORITY_AGENT_CLOSEOUT_VISIBILITY", "full"),
    "PRIORITY_AGENT_AUTO_TEST": os.environ.get("PRIORITY_AGENT_AUTO_TEST", "check_then_test"),
    "PRIORITY_AGENT_BASH_TIMEOUT_FLOOR_SECS": os.environ.get("PRIORITY_AGENT_LIVE_EVAL_BASH_TIMEOUT_FLOOR_SECS", "600"),
    "PRIORITY_AGENT_LLM_MEMORY_EXTRACTION": os.environ.get("PRIORITY_AGENT_LLM_MEMORY_EXTRACTION", "0"),
    "MINIMAX_API_KEY": os.environ.get("MINIMAX_API_KEY", ""),
    "MINIMAX_BASE_URL": os.environ.get("MINIMAX_BASE_URL", ""),
    "MINIMAX_MODEL": os.environ.get("MINIMAX_MODEL", "MiniMax-M2.7"),
})
if runtime_profile:
    env["PRIORITY_AGENT_RUNTIME_PROFILE"] = runtime_profile
if runtime_profile in {"minimum_viable_agent", "mva"}:
    env["PRIORITY_AGENT_MVA_AUDIT_TOOLS"] = os.environ.get("PRIORITY_AGENT_MVA_AUDIT_TOOLS", "1")
    env["PRIORITY_AGENT_CANDIDATE_ACTIONS"] = os.environ.get("PRIORITY_AGENT_CANDIDATE_ACTIONS", "shadow")
    env["PRIORITY_AGENT_MVA_MAX_TOOL_CALLS"] = os.environ.get("PRIORITY_AGENT_MVA_MAX_TOOL_CALLS", "10")
    env["PRIORITY_AGENT_MVA_PARALLELISM_LIMIT"] = os.environ.get("PRIORITY_AGENT_MVA_PARALLELISM_LIMIT", "1")
for key in (
    "OPENAI_API_KEY",
    "OPENAI_BASE_URL",
    "OPENAI_MODEL",
    "MOONSHOT_API_KEY",
    "MOONSHOT_BASE_URL",
    "MOONSHOT_MODEL",
):
    env.pop(key, None)

localhost_no_proxy = "127.0.0.1,localhost,::1"
for key in ("NO_PROXY", "no_proxy"):
    current = env.get(key, "")
    if current:
        parts = [part.strip() for part in current.split(",") if part.strip()]
        for part in localhost_no_proxy.split(","):
            if part not in parts:
                parts.append(part)
        env[key] = ",".join(parts)
    else:
        env[key] = localhost_no_proxy

cmd = [
    binary,
    "--eval-run",
    "--prompt-file",
    prompt_file,
    "--output",
    output_file,
    "--events",
    events_file,
]

def file_signature(path):
    try:
        stat = os.stat(path)
        return (stat.st_size, int(stat.st_mtime_ns))
    except FileNotFoundError:
        return (0, 0)

def activity_signature():
    return (
        file_signature(stdout_path),
        file_signature(stderr_path),
        file_signature(output_file),
        file_signature(events_file),
    )

def worktree_diff_signature():
    try:
        result = subprocess.run(
            ["git", "-C", workdir, "status", "--porcelain"],
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
            timeout=5,
        )
    except Exception:
        return ""
    return result.stdout.strip()

started_at = time.monotonic()
started_wall = time.time()
last_activity_at = started_at
last_effective_action_at = started_at
last_monitor_at = started_at
last_signature = activity_signature()
last_diff_signature = worktree_diff_signature()
first_activity_after_start_secs = None
first_effective_action_after_start_secs = None
termination_reason = "process_exit"

def append_monitor(message):
    timestamp = time.strftime("%Y-%m-%dT%H:%M:%S%z")
    with open(monitor_path, "a", encoding="utf-8") as monitor:
        monitor.write(f"[{timestamp}] {message}\n")

def write_metrics(status):
    now = time.monotonic()
    metrics = {
        "status": status,
        "termination_reason": termination_reason,
        "started_at_epoch": started_wall,
        "elapsed_secs": round(max(0, now - started_at), 3),
        "idle_for_secs": round(max(0, now - last_activity_at), 3),
        "no_effective_progress_for_secs": round(max(0, now - last_effective_action_at), 3),
        "first_activity_after_start_secs": first_activity_after_start_secs,
        "first_effective_action_after_start_secs": first_effective_action_after_start_secs,
        "provider_family": "minimax",
        "provider_model": env.get("MINIMAX_MODEL", ""),
        "streaming_tool_mode": "non_streaming",
        "wall_timeout_secs": timeout,
        "idle_timeout_secs": idle_timeout,
        "no_effective_progress_timeout_secs": no_effective_progress_timeout,
    }
    with open(metrics_path, "w", encoding="utf-8") as metrics_file:
        json.dump(metrics, metrics_file, ensure_ascii=False, indent=2)
        metrics_file.write("\n")

with open(stdout_path, "wb") as stdout, open(stderr_path, "wb") as stderr:
    proc = subprocess.Popen(
        cmd,
        cwd=workdir,
        env=env,
        stdout=stdout,
        stderr=stderr,
    )

    status = None
    while status is None:
        status = proc.poll()
        now = time.monotonic()
        current_signature = activity_signature()
        if current_signature != last_signature:
            last_signature = current_signature
            last_activity_at = now
            if first_activity_after_start_secs is None:
                first_activity_after_start_secs = round(now - started_at, 3)

        current_diff_signature = worktree_diff_signature()
        if current_diff_signature and current_diff_signature != last_diff_signature:
            last_diff_signature = current_diff_signature
            last_effective_action_at = now
            if first_effective_action_after_start_secs is None:
                first_effective_action_after_start_secs = round(now - started_at, 3)

        if status is not None:
            break

        if monitor_interval > 0 and now - last_monitor_at >= monitor_interval:
            last_monitor_at = now
            append_monitor(
                "agent-run still running "
                f"elapsed={int(now - started_at)}s "
                f"idle_for={int(now - last_activity_at)}s "
                f"stdout_bytes={file_signature(stdout_path)[0]} "
                f"stderr_bytes={file_signature(stderr_path)[0]} "
                f"output_bytes={file_signature(output_file)[0]} "
                f"events_bytes={file_signature(events_file)[0]}"
            )

        if timeout > 0 and now - started_at > timeout:
            termination_reason = "wall_timeout"
            proc.terminate()
            try:
                proc.wait(timeout=10)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait()
            stderr.write(f"\n[timeout after {timeout}s]\n".encode())
            append_monitor(f"agent-run timeout elapsed={int(now - started_at)}s")
            status = 124
            break

        if idle_timeout > 0 and now - last_activity_at > idle_timeout:
            termination_reason = "idle_timeout"
            proc.terminate()
            try:
                proc.wait(timeout=10)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait()
            stderr.write(f"\n[idle timeout after {idle_timeout}s without stdout/stderr/output/event growth]\n".encode())
            append_monitor(f"agent-run idle timeout idle_for={int(now - last_activity_at)}s")
            status = 125
            break

        if no_effective_progress_timeout > 0 and now - last_effective_action_at > no_effective_progress_timeout:
            termination_reason = "no_effective_progress_timeout"
            proc.terminate()
            try:
                proc.wait(timeout=10)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait()
            stderr.write(f"\n[no effective progress timeout after {no_effective_progress_timeout}s without worktree diff]\n".encode())
            append_monitor(
                "agent-run no effective progress timeout "
                f"elapsed={int(now - started_at)}s "
                f"no_effective_progress_for={int(now - last_effective_action_at)}s"
            )
            status = 126
            break

        time.sleep(5)

write_metrics(status)
print(status)
PY

  local status
  status="$(cat "$exit_file" 2>/dev/null || echo 1)"
  if [[ "$status" != "0" ]]; then
    echo "Agent run failed for $id with exit status $status. See $agent_stderr" >&2
    return 1
  fi
  if [[ ! -f "$agent_events" ]]; then
    echo "Agent run for $id did not produce events: $agent_events" >&2
    return 1
  fi
  if [[ ! -f "$agent_output" ]]; then
    echo "Agent run for $id did not produce output: $agent_output" >&2
    return 1
  fi
}

collect_task() {
  local file="$1" task_workdir="$2"
  local id report_dir report diff_stat diff_patch cmd_log status_file quality_status_file test_status env_base
  local required_cmd_count effective_run_tests cargo_target_dir
  id="$(yaml_get "$file" id)"
  report_dir="$REPORT_DIR/live-$RUN_ID/$id"
  mkdir -p "$report_dir"
  report="$report_dir/report.md"
  diff_stat="$report_dir/diff-stat.txt"
  diff_patch="$report_dir/diff.patch"
  cmd_log="$report_dir/required-commands.log"
  status_file="$report_dir/test-status.txt"
  quality_status_file="$report_dir/agent-quality-status.txt"
  sample_json="$report_dir/sample.json"
  test_status="skipped"
  env_base="$(task_env_base "$id")"
  cargo_target_dir="$(task_cargo_target_dir "$id")"
  required_cmd_count="$(validation_commands "$file" | sed '/^[[:space:]]*$/d' | wc -l | tr -d ' ')"
  effective_run_tests="$RUN_TESTS"
  if [[ "$required_cmd_count" -gt 0 && ( "$MODE" == "agent-run" || "$MODE" == "full" ) ]]; then
    effective_run_tests=1
  fi
  if [[ -f "$report_dir/provider-health-preflight-failed" ]]; then
    effective_run_tests=0
  fi

  git -C "$task_workdir" status --short >"$report_dir/git-status.txt" || true
  git -C "$task_workdir" diff --stat >"$diff_stat" || true
  git -C "$task_workdir" diff >"$diff_patch" || true
  while IFS= read -r untracked_path; do
    [[ -z "$untracked_path" ]] && continue
    (
      cd "$task_workdir"
      git diff --stat --no-index -- /dev/null "$untracked_path" >>"$ROOT_DIR/$diff_stat" || true
      git diff --no-index -- /dev/null "$untracked_path" >>"$ROOT_DIR/$diff_patch" || true
    )
  done < <(git -C "$task_workdir" ls-files --others --exclude-standard || true)

  : >"$cmd_log"
  if [[ "$effective_run_tests" -eq 1 ]]; then
    test_status="ok"
    local cmd
    while IFS= read -r cmd; do
      [[ -z "$cmd" ]] && continue
      (
        set +e
        echo "\$ $cmd"
        (
          cd "$task_workdir" && \
            unset OPENAI_API_KEY OPENAI_BASE_URL OPENAI_MODEL \
              MOONSHOT_API_KEY MOONSHOT_BASE_URL MOONSHOT_MODEL && \
            env \
            CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}" \
            RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}" \
            CARGO_TARGET_DIR="$cargo_target_dir" \
            NO_PROXY="${NO_PROXY:+$NO_PROXY,}127.0.0.1,localhost,::1" \
            no_proxy="${no_proxy:+$no_proxy,}127.0.0.1,localhost,::1" \
            bash -lc "$cmd"
        )
        status=$?
        echo "[exit status: $status]"
        echo
        exit "$status"
      ) >>"$cmd_log" 2>&1 || test_status="failed"
    done < <(validation_commands "$file")
  fi
  echo "$test_status" >"$status_file"
  ruby -ryaml -rjson -e '
sample_path, json_path = ARGV
File.write(json_path, JSON.generate(YAML.load_file(sample_path) || {}) + "\n")
' "$file" "$sample_json"

  {
    echo "# Live Eval Report: $id"
    echo
    echo "- Run id: \`$RUN_ID\`"
    echo "- Sample: \`$file\`"
    echo "- Worktree: \`$task_workdir\`"
    echo "- Isolated env: \`$env_base\`"
    echo "- Test status: \`$test_status\`"
    echo "- Generated: \`$(date '+%Y-%m-%d %H:%M:%S %z')\`"
    echo
    if [[ -f "$report_dir/provider-health-preflight-failed" ]]; then
      echo "## Provider Health Preflight"
      echo
      echo "Provider health failed before the agent run, so required commands were not run for this task."
      echo
      if [[ -f "$report_dir/provider-health.json" ]]; then
        echo '```json'
        cat "$report_dir/provider-health.json"
        printf '\n```\n'
      else
        echo '```text'
        cat "$REPORT_DIR/live-$RUN_ID/provider-health-stderr.log" 2>/dev/null || true
        printf '\n```\n'
      fi
      echo
    fi
    echo "## Git Status"
    echo
    echo '```text'
    cat "$report_dir/git-status.txt"
    echo '```'
    echo
    echo "## Diff Stat"
    echo
    echo '```text'
    cat "$diff_stat"
    echo '```'
    echo
    echo "## Required Commands"
    echo
    echo '```text'
    cat "$cmd_log"
    echo '```'
    echo
    if [[ -f "$report_dir/agent-output.md" || -f "$report_dir/agent-events.jsonl" ]]; then
      echo "## Agent Run"
      echo
      if [[ -f "$report_dir/agent-exit-status.txt" ]]; then
        echo "- Exit status: \`$(cat "$report_dir/agent-exit-status.txt")\`"
      fi
      if [[ -f "$report_dir/agent-output.md" ]]; then
        echo "- Output: \`$report_dir/agent-output.md\`"
      fi
      if [[ -f "$report_dir/agent-events.jsonl" ]]; then
        echo "- Events: \`$report_dir/agent-events.jsonl\`"
      fi
      if [[ -f "$report_dir/agent-monitor.log" ]]; then
        echo "- Monitor: \`$report_dir/agent-monitor.log\`"
      fi
      if [[ -f "$report_dir/agent-run-metrics.json" ]]; then
        echo "- Metrics: \`$report_dir/agent-run-metrics.json\`"
      fi
      if [[ -f "$report_dir/agent-events.jsonl" ]]; then
        echo
        echo "Event counts:"
        echo
        echo '```text'
        python3 - "$report_dir/agent-events.jsonl" <<'PY'
import collections
import json
import sys

counts = collections.Counter()
for line in open(sys.argv[1], encoding="utf-8"):
    try:
        counts[json.loads(line).get("event", "unknown")] += 1
    except Exception:
        counts["invalid_json"] += 1
for key, value in sorted(counts.items()):
    print(f"{key}: {value}")
PY
        echo '```'
      fi
      echo
      echo "Quality signals:"
      echo
      echo '```text'
      python3 scripts/live_eval_quality_status.py \
        "$report_dir/agent-output.md" \
        "$report_dir/agent-events.jsonl" \
        "$diff_patch" \
        "$quality_status_file" \
        "$sample_json" \
        "$status_file" \
        "$cmd_log" \
        "$report_dir/agent-stderr.log"
      echo '```'
      echo
      echo "Specialty signals:"
      echo
      echo '```text'
      python3 - "$report_dir/agent-events.jsonl" "$sample_json" "$status_file" "$cmd_log" "$report_dir/agent-output.md" "$diff_patch" <<'PY'
import json
import pathlib
import sys
from scripts.live_eval_report_parser import (
    memory_proposal_metrics_from_trace,
    normalized_runtime_spine_assertions,
    runtime_spine_metrics_from_events,
)

events_path = pathlib.Path(sys.argv[1])
sample_json_path = pathlib.Path(sys.argv[2])
test_status_path = pathlib.Path(sys.argv[3])
cmd_log_path = pathlib.Path(sys.argv[4])
output_path = pathlib.Path(sys.argv[5])
diff_path = pathlib.Path(sys.argv[6])

sample = json.loads(sample_json_path.read_text(encoding="utf-8")) if sample_json_path.exists() else {}
test_status = test_status_path.read_text(encoding="utf-8").strip() if test_status_path.exists() else "missing"
cmd_log_text = cmd_log_path.read_text(encoding="utf-8") if cmd_log_path.exists() else ""
output = output_path.read_text(encoding="utf-8") if output_path.exists() else ""
diff = diff_path.read_text(encoding="utf-8") if diff_path.exists() else ""
events = []
if events_path.exists():
    for line in events_path.read_text(encoding="utf-8").splitlines():
        try:
            events.append(json.loads(line))
        except Exception:
            pass

trace = next((event for event in reversed(events) if event.get("event") == "trace_summary"), {})
trace_types = trace.get("event_types") or []
trace_events = (trace.get("trace") or {}).get("events") or []
acceptance_config = sample.get("acceptance") or {}
required_commands = acceptance_config.get("required_commands") or []
harness_commands = acceptance_config.get("harness_commands") or []
validation_commands = list(required_commands) + list(harness_commands)
runtime_spine_assertions = normalized_runtime_spine_assertions(sample)
runtime_spine = runtime_spine_metrics_from_events(
    events,
    assertions=runtime_spine_assertions,
)
runtime_profile = str(sample.get("runtime_profile", "")).strip()
mva_profile_active = runtime_profile in {"minimum_viable_agent", "mva"}

def trace_count(label):
    return sum(1 for item in trace_types if item == label)

def trace_events_of(kind):
    return [event for event in trace_events if event.get("type") == kind]

retrieval_events = trace_events_of("retrieval_context_built")
workflow_plans = trace_events_of("workflow_plan_progress")
workflow_judgments = trace_events_of("workflow_judgment_completed")
guided_debugs = trace_events_of("guided_debugging_completed")
verification_events = trace_events_of("verification_completed")
stage_validation_events = trace_events_of("stage_validation_completed")
acceptance_events = trace_events_of("acceptance_review_completed")
closeout_events = trace_events_of("final_closeout_prepared")
adaptive_trigger_events = trace_events_of("adaptive_workflow_triggered")
runtime_diet_events = trace_events_of("runtime_diet_report")
workflow_contract_events = trace_events_of("workflow_contract_activation")
risk_signal_events = trace_events_of("risk_signal_assessed")
progress_events = [event for event in events if event.get("event") == "tool_execution_progress"]
memory_tools = [
    event
    for event in events
    if event.get("event") == "tool_execution_start"
    and str(event.get("name", "")).startswith("memory")
]

memory_sources = []
memory_provenance = []
for event in retrieval_events:
    for source in event.get("sources") or []:
        if source not in memory_sources:
            memory_sources.append(str(source))
    for provenance in event.get("provenance") or []:
        memory_provenance.append(str(provenance))

report_signal_text = "\n".join(
    [
        output,
        diff,
        cmd_log_text,
        "\n".join(memory_provenance),
        "\n".join(json.dumps(event, sort_keys=True) for event in trace_events),
    ]
).lower()
memory_record_used = "memory_record/" in report_signal_text
memory_proposal = memory_proposal_metrics_from_trace(trace_events, report_signal_text)
memory_candidate_typed = memory_proposal["memory_candidate_typed"] == "true"
memory_candidate_has_evidence = memory_proposal["memory_candidate_has_evidence"] == "true"
memory_use_count_updated = (
    "use_count" in report_signal_text
    or "last_used" in report_signal_text
    or "memory_use_count_updated=true" in report_signal_text
)
memory_failure_lesson_promoted = (
    "strategy-failures" in report_signal_text
    or "failed_strategy=" in report_signal_text
    or "memory_failure_lesson_promoted=true" in report_signal_text
)
memory_action_weight_changed = (
    "memory modifier" in report_signal_text
    or "memory_action_weight_changed=true" in report_signal_text
)
memory_stale_demoted = (
    "memory_stale_demoted=true" in report_signal_text
    or (":stale:" in report_signal_text and "needs revalidation" in report_signal_text)
)
memory_scope_correct = (
    "memory_scope_correct=true" in report_signal_text
    or ("project_root" in report_signal_text and "session_id" in report_signal_text)
)

weighted_plan_events = [
    event
    for event in workflow_plans
    if event.get("top_priority") is not None
    or event.get("top_importance_score") is not None
    or event.get("top_weight_share") is not None
]
reweighted_events = [event for event in workflow_plans if event.get("reweighted")]
guided_reasoning_events = [
    event for event in workflow_judgments if event.get("guided_reasoning") is True
]
automation_active = bool(validation_commands or verification_events or stage_validation_events or progress_events)
memory_active = bool(trace_count("memory.sync") or memory_tools or any(source == "Memory" for source in memory_sources))
guided_debugging_active = bool(guided_debugs)
guided_reasoning_active = bool(guided_reasoning_events)
weighted_planning_active = bool(weighted_plan_events)
closeout_active = bool(closeout_events)
adaptive_workflow_active = bool(adaptive_trigger_events)

signals = {
    "memory_active": memory_active,
    "automation_active": automation_active,
    "guided_debugging_active": guided_debugging_active,
    "guided_reasoning_active": guided_reasoning_active,
    "weighted_planning_active": weighted_planning_active,
    "closeout_active": closeout_active,
    "adaptive_workflow_active": adaptive_workflow_active,
}
active_count = sum(1 for value in signals.values() if value)

latest_plan = weighted_plan_events[-1] if weighted_plan_events else {}
latest_closeout = closeout_events[-1] if closeout_events else {}
latest_acceptance = acceptance_events[-1] if acceptance_events else {}
latest_runtime_diet = runtime_diet_events[-1] if runtime_diet_events else {}
entry_risk_signal = next((event for event in reversed(risk_signal_events) if event.get("phase") == "turn_entry"), {})
runtime_risk_signal = next((event for event in reversed(risk_signal_events) if event.get("phase") == "runtime"), {})
entry_contract_events = [
    event for event in workflow_contract_events if event.get("phase") == "turn_entry"
]
latest_entry_contract = entry_contract_events[-1] if entry_contract_events else {}
if latest_entry_contract:
    entry_status = "active" if latest_entry_contract.get("active") is True else "skipped"
    entry_label = f"{entry_status}:{latest_entry_contract.get('mode', 'missing')}"
else:
    entry_label = "missing"
if guided_debugs:
    repair_label = "active_after_failure"
elif latest_entry_contract and latest_entry_contract.get("active") is True:
    repair_label = "not_needed"
else:
    repair_label = "none"
acceptance_accepted = latest_acceptance.get("accepted", "missing")
try:
    closeout_acceptance_items = int(latest_closeout.get("acceptance_items") or 0)
except Exception:
    closeout_acceptance_items = 0
if (
    acceptance_accepted == "missing"
    and latest_closeout.get("status") == "passed"
    and closeout_acceptance_items > 0
):
    acceptance_accepted = True
specialty_closeout_status = str(latest_closeout.get("status", "missing")).lower()
eval_intent = str(sample.get("eval_intent", "seeded_code_change")).strip() or "seeded_code_change"
if (
    eval_intent in {"direct_answer", "read_only_audit"}
    and specialty_closeout_status == "missing"
    and runtime_spine.get("completion_contract_status") == "completed"
):
    specialty_closeout_status = "passed"
expected_runtime_completion = str(
    (sample.get("runtime_spine_assertions") or {}).get("completion_status", "")
).strip().lower()
if (
    expected_runtime_completion == "blocked"
    and runtime_spine.get("completion_contract_status") == "blocked"
    and test_status == "ok"
):
    specialty_closeout_status = "passed"

for key, value in signals.items():
    print(f"{key}: {str(value).lower()}")
print(f"active_specialty_signals: {active_count}/{len(signals)}")
print(f"workflow_contract_activation: entry={entry_label} repair={repair_label}")
print(f"workflow_contract_events: {len(workflow_contract_events)}")
print(f"runtime_spine: {runtime_spine['runtime_spine']}")
print(f"runtime_profile: {runtime_profile or 'none'}")
print(f"mva_profile_active: {str(mva_profile_active).lower()}")
print(f"runtime_spine_detail: {runtime_spine['runtime_spine_detail']}")
print(f"runtime_spine_phase_coverage: {runtime_spine['runtime_spine_phase_coverage']}")
print(f"runtime_spine_observed_phases: {runtime_spine['runtime_spine_observed_phases']}")
print(f"runtime_spine_assertions: {runtime_spine['runtime_spine_assertions']}")
print(f"runtime_spine_status: {runtime_spine['runtime_spine_status']}")
print(f"runtime_spine_missing: {runtime_spine['runtime_spine_missing']}")
print(f"risky_tool_runs: {runtime_spine['risky_tool_runs']}")
print(f"risky_tool_reviewed: {runtime_spine['risky_tool_reviewed']}")
print(f"risky_tool_missing_action_review: {runtime_spine['risky_tool_missing_action_review']}")
print(f"gate_outcomes: {runtime_spine['gate_outcomes']}")
print(f"gate_outcome_records: {runtime_spine['gate_outcome_records']}")
print(f"gate_outcome_total: {runtime_spine['gate_outcome_total']}")
print(f"gate_outcome_protective_blocks: {runtime_spine['gate_outcome_protective_blocks']}")
print(f"gate_outcome_recoverable_friction: {runtime_spine['gate_outcome_recoverable_friction']}")
print(f"gate_outcome_unrecovered_blocks: {runtime_spine['gate_outcome_unrecovered_blocks']}")
print(f"gate_outcome_suspected_false_positives: {runtime_spine['gate_outcome_suspected_false_positives']}")
print(f"gate_outcome_policy_correct_but_ux_costly: {runtime_spine['gate_outcome_policy_correct_but_ux_costly']}")
print(f"gate_outcome_harmless_passes: {runtime_spine['gate_outcome_harmless_passes']}")
print(f"gate_outcome_failure_owners: {runtime_spine['gate_outcome_failure_owners']}")
print(f"agent_loop_steps: {runtime_spine['agent_loop_steps']}")
print(f"context_zones_materialized: {runtime_spine['context_zones_materialized']}")
print(f"context_zone_task_state_empty: {runtime_spine['context_zone_task_state_empty']}")
print(f"context_zone_current_decision_request_empty: {runtime_spine['context_zone_current_decision_request_empty']}")
print(f"context_zone_envelope_messages: {runtime_spine['context_zone_envelope_messages']}")
print(f"context_zone_source_messages: {runtime_spine['context_zone_source_messages']}")
print(f"context_zone_duplicate_blocks_removed: {runtime_spine['context_zone_duplicate_blocks_removed']}")
print(f"context_zone_provenance_markers: {runtime_spine['context_zone_provenance_markers']}")
print(f"state_transition_recorded: {runtime_spine['state_transition_recorded']}")
print(f"completion_contract_status: {runtime_spine['completion_contract_status']}")
print(f"completion_contract_proof_status: {runtime_spine['completion_contract_proof_status']}")
print(f"candidate_score_calibrated: {runtime_spine['candidate_score_calibrated']}")
print(f"candidate_score_disagreement: {runtime_spine['candidate_score_disagreement']}")
print(f"observer_outcome_recorded: {runtime_spine['observer_outcome_recorded']}")
print(f"memory_boundary_recorded: {runtime_spine['memory_boundary_recorded']}")
print(f"verification_proof_status: {runtime_spine['verification_proof_status']}")
print(f"verification_proof_summary: {runtime_spine['verification_proof_summary']}")
print(f"verification_proof_kinds: {runtime_spine['verification_proof_kinds']}")
print(f"verification_proof_support_status: {runtime_spine['verification_proof_support_status']}")
print(f"verification_proof_support_summary: {runtime_spine['verification_proof_support_summary']}")
print(f"verification_proof_supports_verified: {runtime_spine['verification_proof_supports_verified']}")
print(f"verification_proof_residual_risk: {runtime_spine['verification_proof_residual_risk']}")
risk_entry = entry_risk_signal.get("level", "missing") if entry_risk_signal else "missing"
risk_runtime = runtime_risk_signal.get("level", "none") if runtime_risk_signal else "none"
print(f"risk_signal: entry={risk_entry} runtime={risk_runtime}")
if entry_risk_signal:
    print("risk_signal_reasons: " + "; ".join(str(item) for item in (entry_risk_signal.get("reasons") or [])))
print(f"memory_sync_events: {trace_count('memory.sync')}")
print(f"memory_tool_calls: {len(memory_tools)}")
print(f"retrieval_sources: {','.join(memory_sources) if memory_sources else 'none'}")
print(f"memory_candidate_typed: {str(memory_candidate_typed).lower()}")
print(f"memory_candidate_has_evidence: {str(memory_candidate_has_evidence).lower()}")
print(f"memory_proposal_recorded: {memory_proposal['memory_proposal_recorded']}")
print(f"memory_proposal_status: {memory_proposal['memory_proposal_status']}")
print(f"memory_proposal_candidates: {memory_proposal['memory_proposal_candidates']}")
print(f"memory_proposal_kinds: {memory_proposal['memory_proposal_kinds']}")
print(f"memory_proposal_evidence_items: {memory_proposal['memory_proposal_evidence_items']}")
print(f"memory_proposal_write_policy: {memory_proposal['memory_proposal_write_policy']}")
print(f"memory_proposal_write_performed: {memory_proposal['memory_proposal_write_performed']}")
print(f"memory_record_used: {str(memory_record_used).lower()}")
print(f"memory_use_count_updated: {str(memory_use_count_updated).lower()}")
print(f"memory_failure_lesson_promoted: {str(memory_failure_lesson_promoted).lower()}")
print(f"memory_action_weight_changed: {str(memory_action_weight_changed).lower()}")
print(f"memory_stale_demoted: {str(memory_stale_demoted).lower()}")
print(f"memory_scope_correct: {str(memory_scope_correct).lower()}")
print(f"required_commands: {len(validation_commands)}")
print(f"agent_required_commands: {len(required_commands)}")
print(f"harness_commands: {len(harness_commands)}")
print(f"required_command_status: {test_status}")
print(f"validation_events: {len(verification_events)}")
print(f"stage_validation_events: {len(stage_validation_events)}")
print(f"tool_progress_events: {len(progress_events)}")
print(f"guided_debugging_events: {len(guided_debugs)}")
print(f"guided_reasoning_events: {len(guided_reasoning_events)}")
print(f"workflow_plan_events: {len(workflow_plans)}")
print(f"weighted_plan_events: {len(weighted_plan_events)}")
print(f"reweighted_plan_events: {len(reweighted_events)}")
print(f"adaptive_trigger_events: {len(adaptive_trigger_events)}")
print("adaptive_triggers: " + (",".join(dict.fromkeys(str(event.get("trigger", "")) for event in adaptive_trigger_events if event.get("trigger"))) or "none"))
print(f"latest_top_priority: {latest_plan.get('top_priority', 'none')}")
print(f"latest_top_importance_score: {latest_plan.get('top_importance_score', 'none')}")
print(f"latest_top_weight_share: {latest_plan.get('top_weight_share', 'none')}")
print(f"acceptance_accepted: {acceptance_accepted}")
print(f"closeout_status: {specialty_closeout_status}")
print(f"closeout_tool_records: {latest_closeout.get('tool_records', 0)}")
print(f"closeout_tool_evidence: {latest_closeout.get('tool_evidence', 'missing')}")
if latest_runtime_diet:
    print(
        "runtime_diet: "
        + f"prompt={latest_runtime_diet.get('prompt_tokens', 'missing')} "
        + f"tool_schema={latest_runtime_diet.get('tool_schema_tokens', 'missing')} "
        + f"tools={latest_runtime_diet.get('exposed_tools', 'missing')} "
        + f"workflow={latest_runtime_diet.get('workflow_context', 'missing')}"
    )
else:
    print("runtime_diet: missing")
if validation_commands and test_status != "ok":
    print("attention: required commands did not pass in the harness")
if "guided.debug" not in trace_types:
    print("note: guided debugging is expected only after a blocker or failed validation")
if cmd_log_text and "still running" in cmd_log_text.lower():
    print("note: required command progress appeared in command log")
PY
      echo '```'
      if [[ -f "$report_dir/agent-stderr.log" && -s "$report_dir/agent-stderr.log" ]]; then
        echo
        echo "Agent stderr tail:"
        echo
        echo '```text'
        tail -80 "$report_dir/agent-stderr.log"
        echo '```'
      fi
      if [[ -f "$report_dir/agent-monitor.log" && -s "$report_dir/agent-monitor.log" ]]; then
        echo
        echo "Agent monitor tail:"
        echo
        echo '```text'
        tail -40 "$report_dir/agent-monitor.log"
        echo '```'
      fi
      echo
    fi
    echo "## Human Review"
    echo
    echo "- accepted: TODO"
    echo "- task_success: TODO"
    echo "- mainline_hit: TODO"
    echo "- plan_coverage: TODO"
    echo "- rework_count: TODO"
    echo "- tool_efficiency: TODO"
    echo "- diff_discipline: TODO"
    echo "- closeout_accuracy: TODO"
    echo "- notes: TODO"
  } >"$report"

  local bundle_dir
  bundle_dir="$report_dir/run-bundle"
  PYTHONDONTWRITEBYTECODE=1 python3 scripts/live_eval_run_bundle.py "$report_dir" --run-id "$RUN_ID" --output-dir "$bundle_dir" >/dev/null
  if [[ "${PRIORITY_AGENT_EVAL_LLM_JUDGE:-0}" == "1" ]]; then
    PYTHONDONTWRITEBYTECODE=1 python3 scripts/live_eval_llm_judge.py "$report_dir" --output "$report_dir/judge.json" >/dev/null
  fi
  {
    echo
    echo "## Run Bundle"
    echo
    echo "- Bundle: \`$bundle_dir\`"
    echo "- Task: \`$bundle_dir/task.json\`"
    echo "- Steps: \`$bundle_dir/steps.jsonl\`"
    echo "- Events: \`$bundle_dir/events.jsonl\`"
    echo "- Final report: \`$bundle_dir/final_report.md\`"
    if [[ -f "$report_dir/judge.json" ]]; then
      echo "- Judge: \`$report_dir/judge.json\`"
    fi
  } >>"$report"

  echo "$report"
}

source "$ROOT_DIR/scripts/run_live_eval_summary.sh"

run_one() {
  local file="$1" id task_workdir plan_path report_path
  id="$(yaml_get "$file" id)"
  ensure_live_eval_disk_space "$ROOT_DIR/$WORK_ROOT" || return 2
  case "$MODE" in
    prepare)
      task_workdir="$(prepare_task "$file")"
      echo "Prepared $id: $task_workdir"
      ;;
    api-plan)
      task_workdir="$(prepare_task "$file")"
      if ! plan_path="$(api_plan_task "$file" "$task_workdir")"; then
        return 1
      fi
      echo "MiniMax plan for $id: $plan_path"
      ;;
    agent-run)
      task_workdir="$(prepare_task "$file")"
      local agent_status=0 test_status_file quality_status_file
      agent_run_task "$file" "$task_workdir" || agent_status=$?
      report_path="$(collect_task "$file" "$task_workdir")"
      echo "Agent run for $id: $REPORT_DIR/live-$RUN_ID/$id/agent-output.md"
      echo "Collected $id: $report_path"
      test_status_file="$REPORT_DIR/live-$RUN_ID/$id/test-status.txt"
      quality_status_file="$REPORT_DIR/live-$RUN_ID/$id/agent-quality-status.txt"
      if [[ "$agent_status" -ne 0 ]]; then
        return "$agent_status"
      fi
      if [[ -f "$quality_status_file" ]] && grep -q '^status=failed' "$quality_status_file"; then
        echo "Agent quality gates failed for $id. See $quality_status_file and $report_path" >&2
        return 1
      fi
      if [[ -f "$test_status_file" && "$(cat "$test_status_file")" == "failed" ]]; then
        echo "Required commands failed for $id. See $REPORT_DIR/live-$RUN_ID/$id/required-commands.log" >&2
        return 1
      fi
      ;;
    collect)
      if [[ -z "$WORKDIR" ]]; then
        echo "--workdir is required for collect mode" >&2
        exit 1
      fi
      report_path="$(collect_task "$file" "$WORKDIR")"
      echo "Collected $id: $report_path"
      ;;
    full)
      task_workdir="$(prepare_task "$file")"
      if ! plan_path="$(api_plan_task "$file" "$task_workdir")"; then
        return 1
      fi
      report_path="$(collect_task "$file" "$task_workdir")"
      echo "MiniMax plan for $id: $plan_path"
      echo "Collected $id: $report_path"
      ;;
    *)
      echo "Unknown mode: $MODE" >&2
      exit 1
      ;;
  esac
}

main() {
  if [[ "$MODE" == "list" ]]; then
    if [[ "$CASE_ID" == "recommended" || "$CASE_ID" == "core-coding-quality" || "$CASE_ID" == "real-project-coding" || "$CASE_ID" == "release-dogfood" || "$CASE_ID" == "mvp-weighted-agent" || "$CASE_ID" == "project-partner-demo" || "$CASE_ID" == "runtime-spine-p0b" ]]; then
      need_yaml
      list_task_group "$CASE_ID"
    else
      list_tasks
    fi
    exit 0
  fi
  if [[ "$MODE" == "summary" ]]; then
    if [[ "$RUN_ID_PROVIDED" -eq 0 || -z "$RUN_ID" ]]; then
      echo "--run-id is required for summary mode" >&2
      exit 1
    fi
    summary_task
    exit 0
  fi
  need_yaml
  if [[ -z "$CASE_ID" ]]; then
    echo "--case is required unless --list is used." >&2
    usage
    exit 1
  fi

  mkdir -p "$REPORT_DIR" "$WORK_ROOT/$RUN_ID"
  ensure_live_eval_disk_space "$ROOT_DIR/$WORK_ROOT"

  if [[ "$CASE_ID" == "all" || "$CASE_ID" == "recommended" || "$CASE_ID" == "core-coding-quality" || "$CASE_ID" == "real-project-coding" || "$CASE_ID" == "release-dogfood" || "$CASE_ID" == "mvp-weighted-agent" || "$CASE_ID" == "project-partner-demo" || "$CASE_ID" == "runtime-spine-p0b" ]]; then
    local file files failures=0
    if [[ "$CASE_ID" == "recommended" || "$CASE_ID" == "core-coding-quality" || "$CASE_ID" == "real-project-coding" || "$CASE_ID" == "release-dogfood" || "$CASE_ID" == "mvp-weighted-agent" || "$CASE_ID" == "project-partner-demo" || "$CASE_ID" == "runtime-spine-p0b" ]]; then
      if ! files="$(task_group_files "$CASE_ID")"; then
        exit 1
      fi
    else
      files="$(task_files)"
    fi
    for file in $files; do
      local task_status=0
      run_one "$file" || task_status=$?
      if [[ "$task_status" -eq 2 ]]; then
        echo "Live eval stopped before $(yaml_get "$file" id) due to infrastructure preflight failure." >&2
        exit 2
      fi
      if [[ "$task_status" -ne 0 ]]; then
        echo "Live eval task failed: $(yaml_get "$file" id)" >&2
        failures=$((failures + 1))
      fi
    done
    if [[ "$failures" -gt 0 ]]; then
      echo "Live eval completed with $failures failed task(s)." >&2
      exit 1
    fi
  else
    local file
    if ! file="$(find_task_file "$CASE_ID")"; then
      echo "No live task found for id: $CASE_ID" >&2
      exit 1
    fi
    run_one "$file"
  fi
}

main
