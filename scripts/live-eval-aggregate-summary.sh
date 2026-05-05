#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

OUTPUT="${1:-docs/benchmarks/live-eval-shortfall-summary.md}"
REFRESH_SUMMARIES="${LIVE_EVAL_AGGREGATE_REFRESH_SUMMARIES:-1}"

if [[ "$REFRESH_SUMMARIES" == "1" ]]; then
  while IFS= read -r run_dir; do
    run_id="${run_dir#live-}"
    scripts/run_live_eval.sh --mode summary --run-id "$run_id" >/dev/null
  done < <(find docs/benchmarks -maxdepth 3 -name report.md | awk -F/ '{print $3}' | sort -u)
fi

python3 - "$OUTPUT" <<'PY'
import collections
import datetime as dt
import pathlib
import re
import sys

output = pathlib.Path(sys.argv[1])
benchmarks = pathlib.Path("docs/benchmarks")

def read(path):
    return path.read_text(encoding="utf-8") if path.exists() else ""

def summary_value(text, label, default="missing"):
    match = re.search(rf"^- {re.escape(label)}: `?([^`\n]+)`?", text, re.MULTILINE)
    return match.group(1).strip() if match else default

def parse_count_pair(value):
    match = re.match(r"(\d+)/(\d+)", value)
    if not match:
        return 0, 0
    return int(match.group(1)), int(match.group(2))

def parse_int(value):
    match = re.search(r"\d+", value)
    return int(match.group(0)) if match else 0

def table_rows(text):
    rows = []
    in_matrix = False
    for line in text.splitlines():
        if line.startswith("| task | status |"):
            in_matrix = True
            continue
        if in_matrix and line.startswith("|------"):
            continue
        if in_matrix:
            if not line.startswith("|"):
                break
            cells = [cell.strip() for cell in line.strip("|").split("|")]
            if len(cells) >= 12:
                rows.append(cells[:12])
    return rows

def infer_owner(record):
    if record["owner"] != "missing":
        return record["owner"]
    warnings = record["warnings"].split(",") if record["warnings"] != "none" else []
    if record["status"] == "passed":
        return "none"
    if record["diff"] == "no" and record["intent"] == "seeded_code_change":
        return "llm_reasoning"
    if record["required"] == "failed" and record["diff"] == "yes":
        return "llm_reasoning"
    if "tool_errors_seen" in warnings:
        return "agent_flow"
    if "no_code_diff" in warnings:
        return "agent_flow"
    if record["verification"] == "failed":
        return "llm_reasoning"
    return "unknown"

run_records = []
task_records = []
failure_modes = collections.Counter()
owners = collections.Counter()
inferred_owners = collections.Counter()
intents = collections.Counter()
status_counts = collections.Counter()
warning_counts = collections.Counter()

for summary in sorted(benchmarks.glob("live-*/summary.md")):
    text = read(summary)
    run_id = summary.parent.name.removeprefix("live-")
    passed, total = parse_count_pair(summary_value(text, "Pass rate", "0/0"))
    failed, _ = parse_count_pair(summary_value(text, "Failure rate", "0/0"))
    real_code_passes = parse_int(summary_value(text, "Real code-change passes", "0"))
    plan_only_passes = parse_int(summary_value(text, "Plan-only passes", "0"))
    seeded_no_diff = parse_int(summary_value(text, "Seeded no-diff failures", "0"))
    run_records.append({
        "run": run_id,
        "passed": passed,
        "failed": failed,
        "total": total,
        "real_code_passes": real_code_passes,
        "plan_only_passes": plan_only_passes,
        "seeded_no_diff": seeded_no_diff,
    })

    for mode, count in re.findall(r"^- `([^`]+)`: `(\d+)`$", text, re.MULTILINE):
        failure_modes[mode] += int(count)
        if mode.startswith("warning:"):
            warning_counts[mode.removeprefix("warning:")] += int(count)

    for task, status, intent, owner, required, plan, boundary, verification, closeout, first_write, diff, warnings in table_rows(text):
        if task == "none":
            continue
        record = {
            "run": run_id,
            "task": task,
            "status": status,
            "intent": intent,
            "owner": owner,
            "required": required,
            "plan": plan,
            "boundary": boundary,
            "verification": verification,
            "closeout": closeout,
            "first_write": first_write,
            "diff": diff,
            "warnings": warnings,
        }
        record["inferred_owner"] = infer_owner(record)
        task_records.append(record)
        owners[owner] += 1
        inferred_owners[record["inferred_owner"]] += 1
        intents[intent] += 1
        status_counts[status] += 1

total_tasks = len(task_records)
passed_tasks = status_counts["passed"]
failed_tasks = status_counts["failed"]
real_code_passes = sum(record["real_code_passes"] for record in run_records)
plan_only_passes = sum(record["plan_only_passes"] for record in run_records)
seeded_no_diff = sum(record["seeded_no_diff"] for record in run_records)
required_failed = sum(1 for record in task_records if record["required"] == "failed")
verification_failed = sum(1 for record in task_records if record["verification"] == "failed")
closeout_not_successful = failure_modes["closeout_not_successful"]
owner_metadata_missing = owners["missing"]
recovered_validation_failures = (
    failure_modes["earlier_verification_failed_before_repair"]
    + failure_modes["earlier_stage_validation_failed_before_repair"]
)
no_diff_seeded_tasks = [
    record
    for record in task_records
    if record["intent"] == "seeded_code_change"
    and record["status"] == "failed"
    and record["diff"] == "no"
]

def pct(part, whole):
    if whole == 0:
        return "0.0%"
    return f"{(part / whole) * 100:.1f}%"

def top(counter, limit=12):
    return counter.most_common(limit)

def md_table(headers, rows):
    lines = [
        "| " + " | ".join(headers) + " |",
        "|" + "|".join("---" for _ in headers) + "|",
    ]
    for row in rows:
        lines.append("| " + " | ".join(str(cell).replace("|", "\\|") for cell in row) + " |")
    return lines

recent_failures = [
    record
    for record in task_records
    if record["status"] == "failed"
][-20:]

lines = [
    "# Live Eval Shortfall Summary",
    "",
    f"- Generated: `{dt.datetime.now().astimezone().strftime('%Y-%m-%d %H:%M:%S %z')}`",
    f"- Runs scanned: `{len(run_records)}`",
    f"- Task reports scanned: `{total_tasks}`",
    f"- Pass rate: `{passed_tasks}/{total_tasks}` ({pct(passed_tasks, total_tasks)})",
    f"- Failure rate: `{failed_tasks}/{total_tasks}` ({pct(failed_tasks, total_tasks)})",
    f"- Real code-change passes: `{real_code_passes}`",
    f"- Plan-only passes: `{plan_only_passes}`",
    f"- Seeded no-diff failures: `{seeded_no_diff}`",
    f"- Required-command failures: `{required_failed}`",
    f"- Verification failures: `{verification_failed}`",
    "",
    "## Shortfall Distribution",
    "",
]

lines.extend(md_table(
    ["dimension", "count", "share"],
    [
        ["failed_tasks", failed_tasks, pct(failed_tasks, total_tasks)],
        ["required_command_failed", required_failed, pct(required_failed, total_tasks)],
        ["verification_failed", verification_failed, pct(verification_failed, total_tasks)],
        ["closeout_not_successful", closeout_not_successful, pct(closeout_not_successful, total_tasks)],
        ["recovered_validation_failures", recovered_validation_failures, pct(recovered_validation_failures, total_tasks)],
        ["seeded_no_diff_failed", seeded_no_diff, pct(seeded_no_diff, total_tasks)],
        ["owner_metadata_missing", owner_metadata_missing, pct(owner_metadata_missing, total_tasks)],
        ["real_code_change_passed", real_code_passes, pct(real_code_passes, total_tasks)],
        ["plan_only_passed", plan_only_passes, pct(plan_only_passes, total_tasks)],
    ],
))

lines.extend(["", "## Failure Owners", ""])
lines.extend(md_table(["owner", "count", "share"], [[k, v, pct(v, total_tasks)] for k, v in top(owners)]))

lines.extend(["", "## Inferred Owners", ""])
lines.extend(md_table(
    ["owner", "count", "share"],
    [[k, v, pct(v, total_tasks)] for k, v in top(inferred_owners)],
))

lines.extend(["", "## Failure Modes", ""])
lines.extend(md_table(["mode", "count"], top(failure_modes)))

lines.extend(["", "## Eval Intents", ""])
lines.extend(md_table(["intent", "count", "share"], [[k, v, pct(v, total_tasks)] for k, v in top(intents)]))

lines.extend(["", "## Seeded No-Diff Tasks", ""])
lines.extend(md_table(
    ["run", "task", "owner", "required", "closeout", "warnings"],
    [
        [record["run"], record["task"], record["owner"], record["required"], record["closeout"], record["warnings"]]
        for record in no_diff_seeded_tasks[-25:]
    ] or [["none", "none", "none", "none", "none", "none"]],
))

lines.extend(["", "## Recent Failed Tasks", ""])
lines.extend(md_table(
    ["run", "task", "intent", "owner", "inferred_owner", "required", "verification", "diff", "warnings"],
    [
        [
            record["run"],
            record["task"],
            record["intent"],
            record["owner"],
            record["inferred_owner"],
            record["required"],
            record["verification"],
            record["diff"],
            record["warnings"],
        ]
        for record in recent_failures
    ] or [["none", "none", "none", "none", "none", "none", "none", "none", "none"]],
))

lines.extend([
    "",
    "## Reading",
    "",
    "- `real_code_change_passed` requires an agent-run report with a non-empty diff.",
    "- `plan_only_passed` is tracked separately so planning success is not counted as code-change success.",
    "- `seeded_no_diff_failed` is the strongest signal for agents that inspect but do not patch.",
    "- `inferred_owner` is a conservative backfill for older reports that predate structured `failure_owner` fields.",
    "- `owner_metadata_missing` tracks that historical evidence gap separately from inferred product failures.",
])

output.write_text("\n".join(lines).rstrip() + "\n", encoding="utf-8")
print(output)
PY
