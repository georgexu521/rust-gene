# Sourced by scripts/run_live_eval.sh. Keep functions side-effect-free at source time.

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

release_dogfood_task_files() {
  local id file missing=0
  for id in "${RELEASE_DOGFOOD_CASES[@]}"; do
    if file="$(find_task_file "$id")"; then
      echo "$file"
    else
      echo "Release dogfood live task missing: $id" >&2
      missing=1
    fi
  done
  return "$missing"
}

mvp_weighted_agent_task_files() {
  local id file missing=0
  for id in "${MVP_WEIGHTED_AGENT_CASES[@]}"; do
    if file="$(find_task_file "$id")"; then
      echo "$file"
    else
      echo "MVP weighted-agent live task missing: $id" >&2
      missing=1
    fi
  done
  return "$missing"
}

project_partner_demo_task_files() {
  local id file missing=0
  for id in "${PROJECT_PARTNER_DEMO_CASES[@]}"; do
    if file="$(find_task_file "$id")"; then
      echo "$file"
    else
      echo "Project-partner demo live task missing: $id" >&2
      missing=1
    fi
  done
  return "$missing"
}

runtime_spine_p0b_task_files() {
  local id file missing=0
  for id in "${RUNTIME_SPINE_P0B_CASES[@]}"; do
    if file="$(find_task_file "$id")"; then
      echo "$file"
    else
      echo "Runtime-spine P0b live task missing: $id" >&2
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
    release-dogfood)
      release_dogfood_task_files
      ;;
    mvp-weighted-agent)
      mvp_weighted_agent_task_files
      ;;
    project-partner-demo)
      project_partner_demo_task_files
      ;;
    runtime-spine-p0b)
      runtime_spine_p0b_task_files
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

