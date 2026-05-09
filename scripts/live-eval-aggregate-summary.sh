#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

OUTPUT="${1:-docs/benchmarks/live-eval-shortfall-summary.md}"
REFRESH_SUMMARIES="${LIVE_EVAL_AGGREGATE_REFRESH_SUMMARIES:-0}"
BENCHMARKS_DIR="${LIVE_EVAL_AGGREGATE_BENCHMARKS_DIR:-docs/benchmarks}"
RUN_GLOB="${LIVE_EVAL_AGGREGATE_RUN_GLOB:-live-*}"

if [[ "$REFRESH_SUMMARIES" == "1" ]]; then
  while IFS= read -r report_path; do
    run_dir="$(basename "$(dirname "$(dirname "$report_path")")")"
    run_id="${run_dir#live-}"
    scripts/run_live_eval.sh --mode summary --run-id "$run_id" >/dev/null
  done < <(find "$BENCHMARKS_DIR" -maxdepth 3 -name report.md | sort -u)
fi

python3 - "$OUTPUT" "$BENCHMARKS_DIR" "$RUN_GLOB" <<'PY'
import collections
import datetime as dt
import pathlib
import re
import sys
from scripts.live_eval_report_parser import read, report_rows

output = pathlib.Path(sys.argv[1])
benchmarks = pathlib.Path(sys.argv[2])
run_glob = sys.argv[3]

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
            if len(cells) >= 16:
                rows.append(cells[:16])
            elif len(cells) >= 14:
                rows.append(cells[:13] + [
                    "active=false, recalled=0, conflicts=0, changed_plan=false",
                    "active=false, tool_calls=0, usage_events=0, promotion=false",
                    cells[13],
                ])
            elif len(cells) >= 12:
                rows.append(cells[:9] + ["none"] + cells[9:12] + [
                    "active=false, recalled=0, conflicts=0, changed_plan=false",
                    "active=false, tool_calls=0, usage_events=0, promotion=false",
                    cells[12] if len(cells) > 12 else "none",
                ])
    return rows

def specialty_flag(summary, key):
    match = re.search(rf"{re.escape(key)}=(true|false)", summary)
    return match.group(1) if match else "false"

def specialty_int(summary, key):
    match = re.search(rf"{re.escape(key)}=(\d+)", summary)
    return int(match.group(1)) if match else 0

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
    if (
        "action_checkpoint_no_patch" in warnings
        or "action_checkpoint_invalid_tools" in warnings
        or "patch_synthesis_no_change" in warnings
    ):
        return "agent_flow"
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
trigger_counts = collections.Counter()
agent_flow_stop_modes = {
    "action_checkpoint_no_patch",
    "action_checkpoint_invalid_tools",
    "patch_synthesis_no_change",
    "tool_run_without_closeout",
    "empty_agent_output",
    "missing_trace_summary",
}

for run_dir in sorted(benchmarks.glob(run_glob)):
    if not run_dir.is_dir():
        continue
    run_id = run_dir.name.removeprefix("live-")
    rows = report_rows(run_dir)
    if not rows:
        summary = run_dir / "summary.md"
        text = read(summary)
        rows = [
            {
                "task": task,
                "status": status,
                "intent": intent,
                "owner": owner,
                "required": required,
                "plan": plan,
                "boundary": boundary,
                "verification": verification,
                "closeout": closeout,
                "runtime_diet": runtime_diet,
                "triggers": triggers,
                "first_write": first_write,
                "diff": diff,
                "memory": memory,
                "skill": skill,
                "warnings": warnings,
                "failures": [],
            }
            for task, status, intent, owner, required, plan, boundary, verification, closeout, runtime_diet, triggers, first_write, diff, memory, skill, warnings
            in table_rows(text)
        ]
    if not rows:
        continue
    passed = sum(1 for row in rows if row["status"] == "passed")
    failed = sum(1 for row in rows if row["status"] == "failed")
    total = len(rows)
    real_code_passes = sum(
        1
        for row in rows
        if row["status"] == "passed"
        and row["boundary"] == "agent-run"
        and row["diff"] == "yes"
    )
    plan_only_passes = sum(
        1
        for row in rows
        if row["status"] == "passed"
        and row["boundary"] == "plan-only"
    )
    seeded_no_diff = sum(
        1
        for row in rows
        if row["status"] == "failed"
        and row["intent"] == "seeded_code_change"
        and row["diff"] == "no"
    )
    run_records.append({
        "run": run_id,
        "passed": passed,
        "failed": failed,
        "total": total,
        "real_code_passes": real_code_passes,
        "plan_only_passes": plan_only_passes,
        "seeded_no_diff": seeded_no_diff,
    })

    for row in rows:
        for failure in row["failures"]:
            failure_modes[failure] += 1
        if row["warnings"] != "none":
            for warning in row["warnings"].split(","):
                failure_modes[f"warning:{warning}"] += 1
                warning_counts[warning] += 1
        record = {
            "run": run_id,
            "task": row["task"],
            "status": row["status"],
            "intent": row["intent"],
            "owner": row["owner"],
            "required": row["required"],
            "plan": row["plan"],
            "boundary": row["boundary"],
            "verification": row["verification"],
            "closeout": row["closeout"],
            "triggers": row["triggers"],
            "first_write": row["first_write"],
            "diff": row["diff"],
            "warnings": row["warnings"],
            "memory": row.get("memory", "active=false, recalled=0, conflicts=0, changed_plan=false"),
            "skill": row.get("skill", "active=false, tool_calls=0, usage_events=0, promotion=false"),
        }
        record["memory_active"] = row.get("memory_active", specialty_flag(record["memory"], "active"))
        record["memory_recalled_items"] = int(row.get("memory_recalled_items", specialty_int(record["memory"], "recalled")))
        record["memory_conflicts"] = int(row.get("memory_conflicts", specialty_int(record["memory"], "conflicts")))
        record["memory_changed_plan"] = row.get("memory_changed_plan", specialty_flag(record["memory"], "changed_plan"))
        record["skill_active"] = row.get("skill_active", specialty_flag(record["skill"], "active"))
        record["skill_tool_calls"] = int(row.get("skill_tool_calls", specialty_int(record["skill"], "tool_calls")))
        record["skill_usage_events"] = int(row.get("skill_usage_events", specialty_int(record["skill"], "usage_events")))
        record["skill_promotion_evidence"] = row.get("skill_promotion_evidence", specialty_flag(record["skill"], "promotion"))
        record["inferred_owner"] = infer_owner(record)
        task_records.append(record)
        owners[row["owner"]] += 1
        inferred_owners[record["inferred_owner"]] += 1
        intents[row["intent"]] += 1
        status_counts[row["status"]] += 1
        if row["triggers"] != "none":
            for trigger in row["triggers"].split(","):
                trigger = trigger.strip()
                if trigger:
                    trigger_counts[trigger] += 1

total_tasks = len(task_records)
passed_tasks = status_counts["passed"]
failed_tasks = status_counts["failed"]
real_code_passes = sum(record["real_code_passes"] for record in run_records)
plan_only_passes = sum(record["plan_only_passes"] for record in run_records)
seeded_no_diff = sum(record["seeded_no_diff"] for record in run_records)
required_failed = sum(1 for record in task_records if record["required"] == "failed")
verification_failed = sum(1 for record in task_records if record["verification"] == "failed")
memory_active_tasks = sum(1 for record in task_records if record["memory_active"] == "true")
memory_changed_plan_tasks = sum(1 for record in task_records if record["memory_changed_plan"] == "true")
memory_recalled_items = sum(record["memory_recalled_items"] for record in task_records)
memory_conflicts = sum(record["memory_conflicts"] for record in task_records)
skill_active_tasks = sum(1 for record in task_records if record["skill_active"] == "true")
skill_tool_calls = sum(record["skill_tool_calls"] for record in task_records)
skill_usage_events = sum(record["skill_usage_events"] for record in task_records)
skill_promotion_tasks = sum(1 for record in task_records if record["skill_promotion_evidence"] == "true")
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
instrumented_records = [
    record
    for record in task_records
    if record["owner"] != "missing"
    or record["intent"] != "missing"
    or record["triggers"] != "none"
]
recent_passes = [
    record
    for record in task_records
    if record["status"] == "passed"
][-12:]

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

def counter_rows(records, key, limit=12):
    counter = collections.Counter(record[key] for record in records)
    total = len(records)
    return [[k, v, pct(v, total)] for k, v in counter.most_common(limit)]

def failure_mode_rows(records, limit=12):
    counter = collections.Counter()
    for record in records:
        if record["status"] == "failed":
            if record["required"] == "failed":
                counter["required_command_failed"] += 1
            if record["verification"] == "failed":
                counter["verification_failed"] += 1
            if record["closeout"] != "passed":
                counter["closeout_not_successful"] += 1
            for warning in record["warnings"].split(","):
                warning = warning.strip()
                if warning and warning != "none":
                    counter[f"warning:{warning}"] += 1
    return counter.most_common(limit)

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

lines.extend(["", "## Metadata Coverage", ""])
lines.extend(md_table(
    ["dimension", "count", "share"],
    [
        [
            "structured_failure_owner",
            total_tasks - owner_metadata_missing,
            pct(total_tasks - owner_metadata_missing, total_tasks),
        ],
        [
            "structured_eval_intent",
            total_tasks - intents["missing"],
            pct(total_tasks - intents["missing"], total_tasks),
        ],
        [
            "adaptive_trigger_metadata",
            len([record for record in task_records if record["triggers"] != "none"]),
            pct(len([record for record in task_records if record["triggers"] != "none"]), total_tasks),
        ],
        [
            "instrumented_task_reports",
            len(instrumented_records),
            pct(len(instrumented_records), total_tasks),
        ],
    ],
))

lines.extend(["", "## Memory And Skill Evidence", ""])
lines.extend(md_table(
    ["dimension", "count", "share"],
    [
        ["memory_active_tasks", memory_active_tasks, pct(memory_active_tasks, total_tasks)],
        ["memory_changed_plan_tasks", memory_changed_plan_tasks, pct(memory_changed_plan_tasks, total_tasks)],
        ["memory_recalled_items", memory_recalled_items, "n/a"],
        ["memory_conflicts", memory_conflicts, "n/a"],
        ["skill_active_tasks", skill_active_tasks, pct(skill_active_tasks, total_tasks)],
        ["skill_tool_calls", skill_tool_calls, "n/a"],
        ["skill_usage_events", skill_usage_events, "n/a"],
        ["skill_promotion_evidence_tasks", skill_promotion_tasks, pct(skill_promotion_tasks, total_tasks)],
    ],
))

lines.extend(["", "## Instrumented Slice", ""])
instrumented_total = len(instrumented_records)
instrumented_passed = sum(1 for record in instrumented_records if record["status"] == "passed")
instrumented_failed = sum(1 for record in instrumented_records if record["status"] == "failed")
instrumented_required_failed = sum(1 for record in instrumented_records if record["required"] == "failed")
instrumented_verification_failed = sum(1 for record in instrumented_records if record["verification"] == "failed")
instrumented_seeded_no_diff = sum(
    1
    for record in instrumented_records
    if record["intent"] == "seeded_code_change"
    and record["status"] == "failed"
    and record["diff"] == "no"
)
lines.extend(md_table(
    ["dimension", "count", "share"],
    [
        ["task_reports", instrumented_total, pct(instrumented_total, instrumented_total)],
        ["passed", instrumented_passed, pct(instrumented_passed, instrumented_total)],
        ["failed", instrumented_failed, pct(instrumented_failed, instrumented_total)],
        [
            "required_command_failed",
            instrumented_required_failed,
            pct(instrumented_required_failed, instrumented_total),
        ],
        [
            "verification_failed",
            instrumented_verification_failed,
            pct(instrumented_verification_failed, instrumented_total),
        ],
        [
            "seeded_no_diff_failed",
            instrumented_seeded_no_diff,
            pct(instrumented_seeded_no_diff, instrumented_total),
        ],
    ],
))

lines.extend(["", "### Instrumented Owners", ""])
lines.extend(md_table(
    ["owner", "count", "share"],
    counter_rows(instrumented_records, "owner") or [["none", 0, "0.0%"]],
))

lines.extend(["", "### Instrumented Failure Modes", ""])
lines.extend(md_table(
    ["mode", "count"],
    failure_mode_rows(instrumented_records) or [["none", 0]],
))

lines.extend(["", "## Failure Modes", ""])
lines.extend(md_table(["mode", "count"], top(failure_modes)))

agent_flow_rows = []
for mode in sorted(agent_flow_stop_modes):
    count = failure_modes[mode] + failure_modes[f"warning:{mode}"]
    if count:
        agent_flow_rows.append([mode, count, pct(count, total_tasks)])
lines.extend(["", "## Agent Flow Stops", ""])
lines.extend(md_table(
    ["mode", "count", "share"],
    agent_flow_rows or [["none", 0, "0.0%"]],
))

lines.extend(["", "## Adaptive Workflow Triggers", ""])
lines.extend(md_table(
    ["trigger", "count", "share"],
    [[k, v, pct(v, total_tasks)] for k, v in top(trigger_counts)] or [["none", 0, "0.0%"]],
))

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
    ["run", "task", "intent", "owner", "inferred_owner", "required", "verification", "diff", "memory", "skill", "triggers", "warnings"],
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
            record["memory"],
            record["skill"],
            record["triggers"],
            record["warnings"],
        ]
        for record in recent_failures
    ] or [["none", "none", "none", "none", "none", "none", "none", "none", "none", "none", "none", "none"]],
))

lines.extend(["", "## Recent Passed Tasks", ""])
lines.extend(md_table(
    ["run", "task", "intent", "owner", "required", "verification", "diff", "memory", "skill", "triggers", "warnings"],
    [
        [
            record["run"],
            record["task"],
            record["intent"],
            record["owner"],
            record["required"],
            record["verification"],
            record["diff"],
            record["memory"],
            record["skill"],
            record["triggers"],
            record["warnings"],
        ]
        for record in recent_passes
    ] or [["none", "none", "none", "none", "none", "none", "none", "none", "none", "none", "none"]],
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
    "- `instrumented_task_reports` is the cleaner current slice because it excludes reports with no structured owner, intent, or trigger metadata.",
    "- `memory` and `skill` columns show evidence signals only; they are not success labels.",
])

output.write_text("\n".join(lines).rstrip() + "\n", encoding="utf-8")
print(output)
PY
