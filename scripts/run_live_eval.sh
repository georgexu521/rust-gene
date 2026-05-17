#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

TASK_DIR="evalsets/live_tasks"
MODE="list"
CASE_ID=""
LABEL="live-eval"
RUN_ID=""
WORK_ROOT="target/live-evals"
WORKDIR=""
REPORT_DIR="docs/benchmarks"
SKIP_BUILD=0
RUN_TESTS=0
AGENT_TIMEOUT_SECS="${PRIORITY_AGENT_LIVE_EVAL_TIMEOUT_SECS:-1800}"
AGENT_IDLE_SECS="${PRIORITY_AGENT_LIVE_EVAL_IDLE_SECS:-300}"
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
)

usage() {
  cat <<'EOF'
Usage:
  scripts/run_live_eval.sh --list
  scripts/run_live_eval.sh --case <id|recommended|core-coding-quality|real-project-coding|all> --mode <prepare|api-plan|agent-run|collect|full> [options]
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
                     "real-project-coding", or "all".
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
  --timeout SECS     Timeout for agent-run mode (default: 1800).
  --idle-timeout SECS
                     Kill agent-run if output/events/stderr stay idle (default: 300).
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
    --run-id) RUN_ID="${2:-}"; shift 2 ;;
    --run-tests) RUN_TESTS=1; shift ;;
    --skip-build) SKIP_BUILD=1; shift ;;
    --skip-provider-health) SKIP_PROVIDER_HEALTH=1; shift ;;
    --overlay-working-tree) OVERLAY_WORKTREE=1; shift ;;
    --timeout) AGENT_TIMEOUT_SECS="${2:-}"; shift 2 ;;
    --idle-timeout) AGENT_IDLE_SECS="${2:-}"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown option: $1" >&2; usage; exit 1 ;;
  esac
done

if [[ -z "$RUN_ID" ]]; then
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
      "# Live coding regression task: #{sample["title"] || sample["id"] || "unknown"}",
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
    lines.concat([
      "## Acceptance checks",
      "",
      "Before your final response, run every required command below. If any command fails, inspect the failure, repair the code, and rerun the relevant command. Do not claim completion while required commands are failing.",
    ])
    required = acceptance["required_commands"] || []
    if required.empty?
      lines << "- (none)"
    else
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
    case sample.fetch("eval_intent", "seeded_code_change").to_s.strip
    when "audit_or_regression_check"
      lines << "- This is an audit/regression evaluation. If the requested behavior is already present, prove it with direct evidence and required commands instead of forcing an arbitrary edit."
    when "stale_or_already_satisfied"
      lines << "- This case may already be satisfied on the current baseline. Do not force an arbitrary edit; prove the current state and call out stale-baseline risk clearly."
    else
      lines << "- This is a real code-change evaluation in an isolated worktree. Do not stop at investigation."
    end
    case sample.fetch("eval_intent", "seeded_code_change").to_s.strip
    when "audit_or_regression_check", "stale_or_already_satisfied"
      lines << "- Inspect only the smallest set of relevant files first; after at most 3 read-only inspections, run the required validation commands and close out with no changes if the requested behavior is already present. Make a focused edit only when a concrete missing behavior is proven."
    else
      lines << "- Inspect only the smallest set of relevant files first; after at most 3 read-only inspections, either make a focused edit or clearly state the concrete blocker."
    end
    lines.concat([
      "- If the code is already fixed, prove it with the required commands and still provide a Closeout.",
      "- Summarize files changed and why.",
      "- List validation commands you ran and their pass/fail status.",
      "- Mention any remaining risk or blocker explicitly.",
      "- The final response must include a `Closeout:` section.",
    ])
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
    echo "$(task_env_base "$id")/cargo-target"
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

task_files() {
  find "$TASK_DIR" -maxdepth 1 -type f -name '*.yaml' | sort
}

find_task_file() {
  local id="$1"
  local file task_id
  for file in $(task_files); do
    task_id="$(yaml_get "$file" id)"
    if [[ "$task_id" == "$id" ]]; then
      echo "$file"
      return 0
    fi
  done
  return 1
}

recommended_task_files() {
  local id file missing=0
  for id in "${RECOMMENDED_CASES[@]}"; do
    if file="$(find_task_file "$id")"; then
      echo "$file"
    else
      echo "Recommended live task missing: $id" >&2
      missing=1
    fi
  done
  return "$missing"
}

core_coding_quality_task_files() {
  local id file missing=0
  for id in "${CORE_CODING_QUALITY_CASES[@]}"; do
    if file="$(find_task_file "$id")"; then
      echo "$file"
    else
      echo "Core coding quality live task missing: $id" >&2
      missing=1
    fi
  done
  return "$missing"
}

real_project_coding_gauntlet_task_files() {
  local id file missing=0
  for id in "${REAL_PROJECT_CODING_GAUNTLET_CASES[@]}"; do
    if file="$(find_task_file "$id")"; then
      echo "$file"
    else
      echo "Real-project coding gauntlet live task missing: $id" >&2
      missing=1
    fi
  done
  return "$missing"
}

task_group_files() {
  local group="$1"
  case "$group" in
    recommended)
      recommended_task_files
      ;;
    core-coding-quality)
      core_coding_quality_task_files
      ;;
    real-project-coding)
      real_project_coding_gauntlet_task_files
      ;;
    *)
      return 1
      ;;
  esac
}

print_task_table_header() {
  printf '%-36s %-12s %-26s %-10s %s\n' \
    id type eval_intent risk title
  printf '%-36s %-12s %-26s %-10s %s\n' \
    -- ---- ----------- ---- -----
}

print_task_table_row() {
  local file="$1"
  printf '%-36s %-12s %-26s %-10s %s\n' \
    "$(yaml_get "$file" id)" \
    "$(yaml_get "$file" type unknown)" \
    "$(yaml_get "$file" eval_intent seeded_code_change)" \
    "$(yaml_get "$file" risk unknown)" \
    "$(yaml_get "$file" title "$(yaml_get "$file" id)")"
}

list_recommended_tasks() {
  local files file
  if ! files="$(task_group_files recommended)"; then
    return 1
  fi
  print_task_table_header
  for file in $files; do
    print_task_table_row "$file"
  done
}

list_task_group() {
  local group="$1"
  local files file
  if ! files="$(task_group_files "$group")"; then
    return 1
  fi
  print_task_table_header
  for file in $files; do
    print_task_table_row "$file"
  done
}

list_tasks() {
  python3 - "$TASK_DIR" <<'PY'
import pathlib
import re
import sys

task_dir = pathlib.Path(sys.argv[1])
print(f"{'id':<36} {'type':<12} {'eval_intent':<26} {'risk':<10} title")
print(f"{'--':<36} {'----':<12} {'-----------':<26} {'----':<10} -----")

def scalar(lines, key, default=""):
    pattern = re.compile(rf"^{re.escape(key)}:\s*(.*)$")
    for line in lines:
        match = pattern.match(line)
        if not match:
            continue
        value = match.group(1).strip()
        if (value.startswith('"') and value.endswith('"')) or (
            value.startswith("'") and value.endswith("'")
        ):
            value = value[1:-1]
        return value or default
    return default

for path in sorted(task_dir.glob("*.yaml")):
    lines = path.read_text(encoding="utf-8").splitlines()
    task_id = scalar(lines, "id", path.stem)
    task_type = scalar(lines, "type", "unknown")
    intent = scalar(lines, "eval_intent", "seeded_code_change")
    risk = scalar(lines, "risk", "unknown")
    title = scalar(lines, "title", task_id)
    print(f"{task_id:<36} {task_type:<12} {intent:<26} {risk:<10} {title}")
PY
}

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
  local id title base_ref resolved_ref task_workdir prompt_file runbook metadata env_base prepare_log overlay_patch
  id="$(yaml_get "$file" id)"
  title="$(yaml_get "$file" title)"
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
  local id report_dir agent_stdout agent_stderr agent_output agent_events exit_file prompt_file env_base cargo_target_dir
  id="$(yaml_get "$file" id)"
  report_dir="$REPORT_DIR/live-$RUN_ID/$id"
  mkdir -p "$report_dir"
  agent_stdout="$report_dir/agent-stdout.log"
  agent_stderr="$report_dir/agent-stderr.log"
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
    "$task_workdir" \
    "$agent_stdout" \
    "$agent_stderr" \
    "$ROOT_DIR/target/release/priority-agent" \
    "$prompt_file" \
    "$agent_output" \
    "$agent_events" \
    "$env_base" \
    "$cargo_target_dir" <<'PY' >"$exit_file"
import os
import subprocess
import sys
import time

timeout = int(sys.argv[1])
idle_timeout = int(sys.argv[2])
workdir = sys.argv[3]
stdout_path = sys.argv[4]
stderr_path = sys.argv[5]
binary = sys.argv[6]
prompt_file = sys.argv[7]
output_file = sys.argv[8]
events_file = sys.argv[9]
env_base = sys.argv[10]
cargo_target_dir = sys.argv[11]

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

started_at = time.monotonic()
last_activity_at = started_at
last_signature = activity_signature()

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

        if status is not None:
            break

        if now - started_at > timeout:
            proc.terminate()
            try:
                proc.wait(timeout=10)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait()
            stderr.write(f"\n[timeout after {timeout}s]\n".encode())
            status = 124
            break

        if idle_timeout > 0 and now - last_activity_at > idle_timeout:
            proc.terminate()
            try:
                proc.wait(timeout=10)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait()
            stderr.write(f"\n[idle timeout after {idle_timeout}s without stdout/stderr/output/event growth]\n".encode())
            status = 125
            break

        time.sleep(5)

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
      python3 - "$report_dir/agent-output.md" "$report_dir/agent-events.jsonl" "$diff_patch" "$quality_status_file" "$sample_json" "$status_file" "$cmd_log" "$report_dir/agent-stderr.log" <<'PY'
import json
import pathlib
import sys

output_path = pathlib.Path(sys.argv[1])
events_path = pathlib.Path(sys.argv[2])
diff_path = pathlib.Path(sys.argv[3])
status_path = pathlib.Path(sys.argv[4])
sample_json_path = pathlib.Path(sys.argv[5])
test_status_path = pathlib.Path(sys.argv[6])
cmd_log_path = pathlib.Path(sys.argv[7])
stderr_path = pathlib.Path(sys.argv[8])
output = output_path.read_text(encoding="utf-8") if output_path.exists() else ""
diff = diff_path.read_text(encoding="utf-8") if diff_path.exists() else ""
sample = json.loads(sample_json_path.read_text(encoding="utf-8")) if sample_json_path.exists() else {}
test_status = test_status_path.read_text(encoding="utf-8").strip() if test_status_path.exists() else "missing"
cmd_log_text = cmd_log_path.read_text(encoding="utf-8") if cmd_log_path.exists() else ""
stderr_text = stderr_path.read_text(encoding="utf-8") if stderr_path.exists() else ""
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
tool_done = sum(1 for event in events if event.get("event") == "tool_execution_complete")
tool_starts = [event for event in events if event.get("event") == "tool_execution_start"]
first_write_tool_index = next(
    (
        idx
        for idx, event in enumerate(tool_starts, start=1)
        if event.get("name") in {"file_edit", "file_write", "file_patch"}
    ),
    None,
)
forbidden_tools = {str(tool).strip() for tool in (sample.get("forbidden_tools") or []) if str(tool).strip()}
forbidden_tool_uses = [
    str(event.get("name"))
    for event in tool_starts
    if str(event.get("name")) in forbidden_tools
]
diff_files = []
for line in diff.splitlines():
    if not line.startswith("diff --git "):
        continue
    parts = line.split()
    if len(parts) < 4:
        continue
    path = parts[3]
    if path.startswith("b/"):
        path = path[2:]
    if path not in diff_files:
        diff_files.append(path)
diff_constraints = (sample.get("acceptance") or {}).get("diff_constraints") or {}
max_files_changed = diff_constraints.get("max_files_changed")
try:
    max_files_changed = None if max_files_changed in (None, "", "unspecified") else int(max_files_changed)
except (TypeError, ValueError):
    max_files_changed = None
tool_errors = sum(
    1
    for event in events
    if event.get("event") == "tool_execution_complete"
    and "Result: ERROR" in str(event.get("result_preview", ""))
)
tool_failures = sum(1 for event in trace_events if event.get("type") == "tool_completed" and event.get("success") is False)
verification_events = [event for event in trace_events if event.get("type") == "verification_completed"]
stage_validation_events = [event for event in trace_events if event.get("type") == "stage_validation_completed"]
acceptance_events = [event for event in trace_events if event.get("type") == "acceptance_review_completed"]
closeout_events = [event for event in trace_events if event.get("type") == "final_closeout_prepared"]
adaptive_trigger_events = [event for event in trace_events if event.get("type") == "adaptive_workflow_triggered"]
runtime_diet_events = [event for event in trace_events if event.get("type") == "runtime_diet_report"]
adaptive_triggers = []
for event in adaptive_trigger_events:
    trigger = str(event.get("trigger", "")).strip()
    if trigger and trigger not in adaptive_triggers:
        adaptive_triggers.append(trigger)
latest_verification = verification_events[-1] if verification_events else {}
latest_stage_validation = stage_validation_events[-1] if stage_validation_events else {}
latest_closeout = closeout_events[-1] if closeout_events else {}
latest_acceptance = acceptance_events[-1] if acceptance_events else {}
latest_runtime_diet = runtime_diet_events[-1] if runtime_diet_events else {}
closeout_status = str(latest_closeout.get("status", "missing")).lower()
runtime_validation = str(latest_runtime_diet.get("validation_evidence", "")).lower()

def positive_count(value):
    try:
        return int(value) > 0
    except Exception:
        return False

closeout_validation_passed = (
    closeout_status == "passed"
    and (
        runtime_validation.startswith("passed:")
        or positive_count(latest_closeout.get("validation_items"))
    )
)
verification_passed = (
    (bool(verification_events) and latest_verification.get("passed") is True)
    or (not verification_events and closeout_validation_passed)
)
stage_validation_passed = (
    (
        bool(stage_validation_events)
        and str(latest_stage_validation.get("status", "")).lower() in {"passed", "ok", "success"}
    )
    or (not stage_validation_events and closeout_validation_passed)
)
accepted = latest_acceptance.get("accepted")
if accepted is None and closeout_status == "passed" and positive_count(latest_closeout.get("acceptance_items")):
    accepted = True

def normalized_behavior_assertions(sample):
    raw = sample.get("behavior_assertions")
    if raw is None:
        raw = (sample.get("quality_assertions") or {}).get("behavior")
    if raw is None:
        return []
    if isinstance(raw, str):
        raw = [raw]
    if isinstance(raw, dict):
        raw = [f"{key}:{value}" for key, value in raw.items()]
    if not isinstance(raw, list):
        raw = [raw]
    result = []
    for item in raw:
        value = str(item).strip()
        if value and value not in result:
            result.append(value)
    return result

print(f"output_chars: {len(output)}")
print(f"diff_chars: {len(diff)}")
print(f"diff_files_changed: {len(diff_files)}")
print(f"tool_executions: {tool_done}")
print(f"first_write_tool_index: {first_write_tool_index if first_write_tool_index is not None else 'none'}")
print(f"forbidden_tool_uses: {','.join(forbidden_tool_uses) if forbidden_tool_uses else 'none'}")
print(f"tool_errors: {tool_errors}")
print(f"tool_failures: {tool_failures}")
print(f"has_closeout: {str('Closeout:' in output).lower()}")
print(f"has_validation_claim: {str(any(marker in output.lower() for marker in ['validation', 'verified', 'cargo test', '测试', '验证'])).lower()}")
print(f"trace_status: {trace.get('status', 'missing')}")
print(f"trace_events: {len(trace_types)}")
print(f"test_status: {test_status}")
print(f"verification_passed: {str(verification_passed).lower()}")
print(f"stage_validation_passed: {str(stage_validation_passed).lower()}")
print(f"acceptance_accepted: {accepted}")
print(f"closeout_status: {closeout_status}")
if latest_runtime_diet:
    print(
        "runtime_diet: "
        + f"prompt={latest_runtime_diet.get('prompt_tokens', 'missing')} "
        + f"tool_schema={latest_runtime_diet.get('tool_schema_tokens', 'missing')} "
        + f"tools={latest_runtime_diet.get('exposed_tools', 'missing')} "
        + f"workflow={latest_runtime_diet.get('workflow_context', 'missing')} "
        + f"closeout={latest_runtime_diet.get('closeout_visibility', 'missing')} "
        + f"validation={latest_runtime_diet.get('validation_evidence', 'missing')}"
    )
else:
    print("runtime_diet: missing")
print(f"adaptive_triggers: {','.join(adaptive_triggers) if adaptive_triggers else 'none'}")
if trace_types:
    print("trace_event_types: " + ",".join(trace_types[-12:]))
stale_edit_warnings = stderr_text.count("was modified since it was read")
print(f"stale_edit_warnings: {stale_edit_warnings}")
action_checkpoint_no_patch = "Stopped action checkpoint without patch synthesis" in output
action_checkpoint_invalid_tools = "Stopped action checkpoint after repeated invalid tool requests" in output
patch_synthesis_no_change = "Patch synthesis did not produce a file change" in output
legacy_workflow_hijack = "# Workflow 执行报告" in output
print(f"action_checkpoint_no_patch: {str(action_checkpoint_no_patch).lower()}")
print(f"action_checkpoint_invalid_tools: {str(action_checkpoint_invalid_tools).lower()}")
print(f"patch_synthesis_no_change: {str(patch_synthesis_no_change).lower()}")

failures = []
warnings = []
acceptance_config = sample.get("acceptance") or {}
required_commands = acceptance_config.get("required_commands") or []
harness_commands = acceptance_config.get("harness_commands") or []
validation_commands = list(required_commands) + list(harness_commands)
repo = sample.get("repo") or {}
base_ref = str(repo.get("base_ref", "HEAD")).strip()
prepare_commands = repo.get("prepare_commands") or []
task_type = str(sample.get("type", "")).strip()
eval_intent = str(sample.get("eval_intent", "seeded_code_change")).strip() or "seeded_code_change"
behavior_assertions = normalized_behavior_assertions(sample)
if behavior_assertions:
    if validation_commands and test_status == "ok":
        behavior_assertion_status = "passed"
    elif validation_commands:
        behavior_assertion_status = "failed"
    else:
        behavior_assertion_status = "missing"
else:
    behavior_assertion_status = "none"
code_change_types = {"bug_fix", "feature", "refactor", "ux"}
current_head_without_fixture = (
    task_type in code_change_types
    and base_ref in {"", "HEAD", "head"}
    and not prepare_commands
)
seeded_code_change = eval_intent == "seeded_code_change"
audit_or_regression_check = eval_intent == "audit_or_regression_check"
stale_or_already_satisfied = eval_intent == "stale_or_already_satisfied"
print(f"eval_intent: {eval_intent}")
print(f"behavior_assertions: {','.join(behavior_assertions) if behavior_assertions else 'none'}")
print(f"behavior_assertion_status: {behavior_assertion_status}")
if not output.strip():
    print("warning: empty_agent_output")
    failures.append("empty_agent_output")
if tool_done and "Closeout:" not in output:
    print("warning: tool_run_without_closeout")
    failures.append("tool_run_without_closeout")
if not diff.strip():
    print("warning: no_code_diff")
    if audit_or_regression_check:
        warnings.append("audit_no_code_diff")
    else:
        warnings.append("no_code_diff")
    if (
        (stale_or_already_satisfied or (current_head_without_fixture and not seeded_code_change))
        and test_status == "ok"
    ):
        print("warning: current_head_no_fixture_already_satisfied")
        warnings.append("current_head_no_fixture_already_satisfied")
if tool_errors:
    print("warning: tool_errors_seen")
    warnings.append("tool_errors_seen")
if stale_edit_warnings >= 2:
    print("warning: repeated_stale_edit_warnings")
    warnings.append("repeated_stale_edit_warnings")
if action_checkpoint_no_patch:
    print("warning: action_checkpoint_no_patch")
    failures.append("action_checkpoint_no_patch")
if action_checkpoint_invalid_tools:
    print("warning: action_checkpoint_invalid_tools")
    failures.append("action_checkpoint_invalid_tools")
if patch_synthesis_no_change:
    print("warning: patch_synthesis_no_change")
    failures.append("patch_synthesis_no_change")
if legacy_workflow_hijack:
    print("warning: legacy_workflow_hijack")
    failures.append("legacy_workflow_hijack")
if forbidden_tool_uses:
    print("warning: forbidden_tool_used")
    failures.append("forbidden_tool_used")
if max_files_changed is not None and len(diff_files) > max_files_changed:
    print("warning: max_files_changed_exceeded")
    failures.append("max_files_changed_exceeded")
if verification_events and any(event.get("passed") is not True for event in verification_events[:-1]):
    print("warning: earlier_verification_failed_before_repair")
    warnings.append("earlier_verification_failed_before_repair")
if stage_validation_events and any(str(event.get("status", "")).lower() not in {"passed", "ok", "success"} for event in stage_validation_events[:-1]):
    print("warning: earlier_stage_validation_failed_before_repair")
    warnings.append("earlier_stage_validation_failed_before_repair")
if not trace:
    print("warning: missing_trace_summary")
    failures.append("missing_trace_summary")
if validation_commands and test_status != "ok":
    print("warning: required_commands_not_passing")
    failures.append("required_commands_not_passing")
if behavior_assertion_status == "failed":
    print("warning: behavior_assertions_not_passing")
    failures.append("behavior_assertions_not_passing")
elif behavior_assertion_status == "missing":
    print("warning: behavior_assertions_missing_checks")
    failures.append("behavior_assertions_missing_checks")
if closeout_status in {"failed", "not_verified", "blocked", "missing"}:
    print("warning: closeout_not_successful")
    failures.append("closeout_not_successful")
if accepted is False:
    print("warning: acceptance_review_rejected")
    failures.append("acceptance_review_rejected")
if stage_validation_events and not stage_validation_passed:
    print("warning: stage_validation_failed")
    failures.append("stage_validation_failed")
if verification_events and not verification_passed:
    print("warning: verification_failed")
    failures.append("verification_failed")

diff_required = seeded_code_change and task_type in code_change_types
if diff_required and not diff.strip():
    failures.append("expected_code_diff_missing")

harness_acceptance_passed = test_status == "ok" and (not diff_required or bool(diff.strip()))
if harness_acceptance_passed:
    downgraded = []
    for item in (
        "action_checkpoint_invalid_tools",
        "acceptance_review_rejected",
        "stage_validation_failed",
        "verification_failed",
    ):
        if item in failures:
            failures = [failure for failure in failures if failure != item]
            downgraded.append(item)
    for item in downgraded:
        warning = f"recovered_{item}"
        if warning not in warnings:
            warnings.append(warning)
        print(f"warning: {warning}")

status = "failed" if failures else "ok"

def infer_failure_owner():
    if not failures:
        return "none"
    stderr_without_recovered_retries = "\n".join(
        line for line in stderr_text.splitlines() if "reconnecting" not in line.lower()
    )
    provider_text = "\n".join([stderr_without_recovered_retries, output]).lower()
    if (
        "error sending request for url" in provider_text
        or "connection refused" in provider_text
        or "connection reset" in provider_text
        or "operation timed out" in provider_text
        or "provider unavailable" in provider_text
    ):
        return "environment"
    lower_cmd = cmd_log_text.lower()
    if "502" in lower_cmd or "proxy" in lower_cmd or "connection refused" in lower_cmd:
        return "environment"
    if "modulenotfounderror" in lower_cmd or "failed to import test module" in lower_cmd:
        return "eval_harness"
    if "empty_agent_output" in failures or "missing_trace_summary" in failures:
        return "agent_flow"
    if "tool_run_without_closeout" in failures:
        return "agent_flow"
    if (
        "action_checkpoint_no_patch" in failures
        or "action_checkpoint_invalid_tools" in failures
        or "patch_synthesis_no_change" in failures
        or "legacy_workflow_hijack" in failures
    ):
        return "agent_flow"
    if (
        "no_code_diff" in warnings
        and "current_head_no_fixture_already_satisfied" in warnings
        and test_status == "ok"
    ):
        return "eval_harness"
    if "closeout_not_successful" in failures and test_status == "ok":
        return "agent_flow"
    if (
        "required_commands_not_passing" in failures
        and (
            verification_passed
            or stage_validation_passed
            or closeout_status == "passed"
            or accepted is True
        )
    ):
        return "agent_flow"
    if "verification_failed" in failures or "stage_validation_failed" in failures:
        if closeout_status in {"failed", "not_verified", "blocked"}:
            return "llm_reasoning"
        return "agent_flow"
    if "acceptance_review_rejected" in failures:
        return "mixed"
    if "expected_code_diff_missing" in failures:
        return "llm_reasoning"
    return "mixed"

failure_owner = infer_failure_owner()
print(f"failure_owner: {failure_owner}")
with status_path.open("w", encoding="utf-8") as fh:
    fh.write(f"status={status}\n")
    fh.write(f"failure_owner={failure_owner}\n")
    for item in failures:
        fh.write(f"failure={item}\n")
    for item in warnings:
        fh.write(f"warning={item}\n")
PY
      echo '```'
      echo
      echo "Specialty signals:"
      echo
      echo '```text'
      python3 - "$report_dir/agent-events.jsonl" "$sample_json" "$status_file" "$cmd_log" <<'PY'
import json
import pathlib
import sys

events_path = pathlib.Path(sys.argv[1])
sample_json_path = pathlib.Path(sys.argv[2])
test_status_path = pathlib.Path(sys.argv[3])
cmd_log_path = pathlib.Path(sys.argv[4])

sample = json.loads(sample_json_path.read_text(encoding="utf-8")) if sample_json_path.exists() else {}
test_status = test_status_path.read_text(encoding="utf-8").strip() if test_status_path.exists() else "missing"
cmd_log_text = cmd_log_path.read_text(encoding="utf-8") if cmd_log_path.exists() else ""
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
progress_events = [event for event in events if event.get("event") == "tool_execution_progress"]
memory_tools = [
    event
    for event in events
    if event.get("event") == "tool_execution_start"
    and str(event.get("name", "")).startswith("memory")
]

memory_sources = []
for event in retrieval_events:
    for source in event.get("sources") or []:
        if source not in memory_sources:
            memory_sources.append(str(source))

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

for key, value in signals.items():
    print(f"{key}: {str(value).lower()}")
print(f"active_specialty_signals: {active_count}/{len(signals)}")
print(f"memory_sync_events: {trace_count('memory.sync')}")
print(f"memory_tool_calls: {len(memory_tools)}")
print(f"retrieval_sources: {','.join(memory_sources) if memory_sources else 'none'}")
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
print(f"closeout_status: {latest_closeout.get('status', 'missing')}")
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

  echo "$report"
}

summary_task() {
  local run_report_dir="$REPORT_DIR/live-$RUN_ID"
  local summary="$run_report_dir/summary.md"
  mkdir -p "$run_report_dir"
PYTHONDONTWRITEBYTECODE=1 python3 - "$run_report_dir" "$summary" "$RUN_ID" <<'PY'
import pathlib
import sys
from scripts.live_eval_report_parser import report_rows

run_dir = pathlib.Path(sys.argv[1])
summary_path = pathlib.Path(sys.argv[2])
run_id = sys.argv[3]

def md_cell(value):
    text = str(value)
    return text.replace("\\", "\\\\").replace("|", "\\|").replace("\n", " ")

def pct(part, whole):
    if whole == 0:
        return "0.0%"
    return f"{(part / whole) * 100:.1f}%"

def as_int(value):
    try:
        return int(value)
    except Exception:
        return 0

rows = report_rows(run_dir)

totals = {}
for row in rows:
    totals[row["status"]] = totals.get(row["status"], 0) + 1
owners = {}
for row in rows:
    owners[row["owner"]] = owners.get(row["owner"], 0) + 1
intents = {}
for row in rows:
    intents[row["intent"]] = intents.get(row["intent"], 0) + 1
failure_modes = {}
for row in rows:
    for failure in row["failures"]:
        failure_modes[failure] = failure_modes.get(failure, 0) + 1
for row in rows:
    if row["warnings"] != "none":
        for warning in row["warnings"].split(","):
            failure_modes[f"warning:{warning}"] = failure_modes.get(f"warning:{warning}", 0) + 1

task_count = len(rows)
passed_count = totals.get("passed", 0)
failed_count = totals.get("failed", 0)
scored_count = passed_count + failed_count
skipped_count = task_count - scored_count
real_code_change_passed = sum(
    1
    for row in rows
    if row["status"] == "passed" and row["boundary"] == "agent-run" and row["diff"] == "yes"
)
plan_only_passed = sum(
    1
    for row in rows
    if row["status"] == "passed" and row["boundary"] == "plan-only"
)
seeded_no_diff_failures = sum(
    1
    for row in rows
    if row["status"] == "failed"
    and row["intent"] == "seeded_code_change"
    and row["diff"] == "no"
)
memory_active_tasks = sum(1 for row in rows if row["memory_active"] == "true")
memory_changed_plan_tasks = sum(1 for row in rows if row["memory_changed_plan"] == "true")
memory_recalled_items = sum(int(row["memory_recalled_items"]) for row in rows)
memory_conflicts = sum(int(row["memory_conflicts"]) for row in rows)
skill_active_tasks = sum(1 for row in rows if row["skill_active"] == "true")
skill_promotion_tasks = sum(1 for row in rows if row["skill_promotion_evidence"] == "true")
behavior_assertion_tasks = sum(1 for row in rows if row["behavior_assertions"] != "none")
behavior_assertion_passed = sum(1 for row in rows if row["behavior_assertion_status"] == "passed")
memory_behavior_assertion_tasks = sum(
    1 for row in rows if "memory" in row["behavior_assertions"].lower()
)
skill_behavior_assertion_tasks = sum(
    1 for row in rows if "skill" in row["behavior_assertions"].lower()
)
coding_rows = [row for row in rows if row["boundary"] == "agent-run"]
coding_task_count = len(coding_rows)
coding_passed = sum(1 for row in coding_rows if row["coding_gauntlet_status"] == "passed")
coding_failed = sum(1 for row in coding_rows if row["coding_gauntlet_status"] == "failed")
coding_clean_likely_passed = sum(
    1 for row in coding_rows if row["first_pass_signal"] == "likely_clean"
)
coding_repaired_passed = sum(
    1
    for row in coding_rows
    if row["coding_gauntlet_status"] == "passed"
    and row["first_pass_signal"] == "repaired"
)
coding_required_passed = sum(1 for row in coding_rows if row["required"] == "ok")
coding_first_write_observed = sum(
    1 for row in coding_rows if row["first_write"] not in {"none", "missing"}
)
coding_repair_signals = sum(as_int(row["repair_signals"]) for row in coding_rows)
coding_diff_files_changed = sum(as_int(row["diff_files_changed"]) for row in coding_rows)

lines = [
    f"# Live Eval Summary: {run_id}",
    "",
    f"- Run directory: `{run_dir}`",
    f"- Tasks found: `{task_count}`",
    f"- Pass rate: `{passed_count}/{scored_count}` ({pct(passed_count, scored_count)})",
    f"- Failure rate: `{failed_count}/{scored_count}` ({pct(failed_count, scored_count)})",
    f"- Skipped/unscored tasks: `{skipped_count}`",
    f"- Real code-change passes: `{real_code_change_passed}`",
    f"- Plan-only passes: `{plan_only_passed}`",
    f"- Seeded no-diff failures: `{seeded_no_diff_failures}`",
    f"- Memory active tasks: `{memory_active_tasks}`",
    f"- Memory changed-plan tasks: `{memory_changed_plan_tasks}`",
    f"- Memory recalled items: `{memory_recalled_items}`",
    f"- Memory conflicts: `{memory_conflicts}`",
    f"- Skill active tasks: `{skill_active_tasks}`",
    f"- Skill promotion-evidence tasks: `{skill_promotion_tasks}`",
    f"- Behavior assertion tasks: `{behavior_assertion_tasks}`",
    f"- Behavior assertions passed: `{behavior_assertion_passed}`",
    f"- Coding gauntlet agent-run tasks: `{coding_task_count}`",
    f"- Coding gauntlet passes: `{coding_passed}`",
    f"- Coding gauntlet failures: `{coding_failed}`",
    f"- Coding gauntlet likely clean passes: `{coding_clean_likely_passed}`",
    f"- Coding gauntlet repaired passes: `{coding_repaired_passed}`",
    f"- Coding gauntlet required-validation passes: `{coding_required_passed}/{coding_task_count}`",
    f"- Coding gauntlet first-write observed: `{coding_first_write_observed}/{coding_task_count}`",
    f"- Coding gauntlet repair signals: `{coding_repair_signals}`",
    f"- Coding gauntlet changed files: `{coding_diff_files_changed}`",
    "- Status counts: "
    + (", ".join(f"{key}={value}" for key, value in sorted(totals.items())) if totals else "none"),
    "- Failure owners: "
    + (", ".join(f"{key}={value}" for key, value in sorted(owners.items())) if owners else "none"),
    "- Eval intents: "
    + (", ".join(f"{key}={value}" for key, value in sorted(intents.items())) if intents else "none"),
    "",
    "## Failure Modes",
    "",
]

if failure_modes:
    for key, value in sorted(failure_modes.items(), key=lambda item: (-item[1], item[0])):
        lines.append(f"- `{key}`: `{value}`")
else:
    lines.append("- none")

lines.extend([
    "",
    "## Memory And Skill Evidence",
    "",
    "| dimension | count | meaning |",
    "|-----------|-------|---------|",
    f"| memory_active_tasks | {memory_active_tasks} | Tasks where retrieval, sync, or memory tools were active. |",
    f"| memory_changed_plan_tasks | {memory_changed_plan_tasks} | Tasks where memory or learning signals reweighted planning. |",
    f"| memory_recalled_items | {memory_recalled_items} | Retrieved memory-backed context items across tasks. |",
    f"| memory_conflicts | {memory_conflicts} | Retrieval-context conflict count from memory-backed context. |",
    f"| skill_active_tasks | {skill_active_tasks} | Tasks where skill tools or skill-specific signals were active. |",
    f"| skill_promotion_evidence_tasks | {skill_promotion_tasks} | Tasks with promotion-related skill evidence. |",
    f"| behavior_assertion_tasks | {behavior_assertion_tasks} | Tasks with explicit behavior assertions in the live-eval sample. |",
    f"| behavior_assertions_passed | {behavior_assertion_passed} | Explicit behavior-assertion tasks whose required checks passed. |",
    f"| memory_behavior_assertion_tasks | {memory_behavior_assertion_tasks} | Behavior assertions covering memory semantics rather than only memory activity signals. |",
    f"| skill_behavior_assertion_tasks | {skill_behavior_assertion_tasks} | Behavior assertions covering skill semantics rather than only skill activity signals. |",
    "",
    "## Outcome Classes",
    "",
    "| class | count | meaning |",
    "|-------|-------|---------|",
    f"| real_code_change_passed | {real_code_change_passed} | Agent-run tasks with passing status and a real diff. |",
    f"| plan_only_passed | {plan_only_passed} | Planning/API-only artifacts that passed their available checks. |",
    f"| seeded_no_diff_failed | {seeded_no_diff_failures} | Seeded code-change tasks where the agent did not produce a diff. |",
    "",
    "## Coding Gauntlet Evidence",
    "",
    "| task | gauntlet_status | first_pass_signal | coding | required | closeout | first_write | diff | warnings |",
    "|------|-----------------|-------------------|--------|----------|----------|-------------|------|----------|",
])

if coding_rows:
    for row in coding_rows:
        lines.append(
            "| {task} | {coding_gauntlet_status} | {first_pass_signal} | {coding} | {required} | {closeout} | {first_write} | {diff} | {warnings} |".format(
                **{key: md_cell(value) for key, value in row.items()}
            )
        )
else:
    lines.append("| none | not_applicable | unknown | tools=0, validations=0, repair=0, files=0 | missing | missing | missing | no | none |")

lines.extend([
    "",
    "## Task Matrix",
    "",
    "| task | status | intent | owner | required | plan_quality | tool_boundary | verification_status | closeout | runtime_diet | behavior_assertions | behavior_status | triggers | first_write | diff | memory | skill | warnings |",
    "|------|--------|--------|-------|----------|--------------|---------------|---------------------|----------|--------------|---------------------|-----------------|----------|-------------|------|--------|-------|----------|",
])

if rows:
    for row in rows:
        lines.append(
            "| {task} | {status} | {intent} | {owner} | {required} | {plan} | {boundary} | {verification} | {closeout} | {runtime_diet} | {behavior_assertions} | {behavior_assertion_status} | {triggers} | {first_write} | {diff} | {memory} | {skill} | {warnings} |".format(
                **{key: md_cell(value) for key, value in row.items()}
            )
        )
else:
    lines.append("| none | missing | missing | missing | missing | none | none | unknown | missing | none | none | none | missing | none | no | none | none | none |")

lines.extend([
    "",
    "## Notes",
    "",
    "- `plan_quality` describes plan-only/API artifacts when present.",
    "- `tool_boundary` separates plan-only, collect-only, and real agent-run reports.",
    "- `verification_status` combines closeout and required-command evidence; it is not a human-quality score.",
    "- `skipped` reports are excluded from pass/fail rate denominators; collect-only reports need passing required commands to be scored.",
    "- `real_code_change_passed` requires an agent-run report with a non-empty diff; plan-only success is tracked separately.",
    "- `memory` and `skill` summarize evidence signals; they do not by themselves mean the task succeeded.",
    "- `behavior_assertions` are explicit sample-level checks; memory/skill behavior assertions are stronger evidence than activity signals alone.",
])

summary_path.write_text("\n".join(lines).rstrip() + "\n", encoding="utf-8")
print(summary_path)
PY
}

run_one() {
  local file="$1" id task_workdir plan_path report_path
  id="$(yaml_get "$file" id)"
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
    if [[ "$CASE_ID" == "recommended" || "$CASE_ID" == "core-coding-quality" || "$CASE_ID" == "real-project-coding" ]]; then
      need_yaml
      list_task_group "$CASE_ID"
    else
      list_tasks
    fi
    exit 0
  fi
  if [[ "$MODE" == "summary" ]]; then
    if [[ -z "$RUN_ID" ]]; then
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

  if [[ "$CASE_ID" == "all" || "$CASE_ID" == "recommended" || "$CASE_ID" == "core-coding-quality" || "$CASE_ID" == "real-project-coding" ]]; then
    local file files failures=0
    if [[ "$CASE_ID" == "recommended" || "$CASE_ID" == "core-coding-quality" || "$CASE_ID" == "real-project-coding" ]]; then
      if ! files="$(task_group_files "$CASE_ID")"; then
        exit 1
      fi
    else
      files="$(task_files)"
    fi
    for file in $files; do
      if ! run_one "$file"; then
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
