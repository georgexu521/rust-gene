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

PYTHONDONTWRITEBYTECODE=1 python3 - "$OUTPUT" "$BENCHMARKS_DIR" "$RUN_GLOB" <<'PY'
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
    headers = []
    in_matrix = False
    for line in text.splitlines():
        if line.startswith("| task | status |"):
            headers = [re.sub(r"[^a-z0-9]+", "_", cell.strip().lower()).strip("_") for cell in line.strip("|").split("|")]
            in_matrix = True
            continue
        if in_matrix and line.startswith("|------"):
            continue
        if in_matrix:
            if not line.startswith("|"):
                break
            cells = [cell.strip() for cell in line.strip("|").split("|")]
            if not headers or len(cells) < 10:
                continue
            record = dict(zip(headers, cells))
            rows.append({
                "task": record.get("task", "missing"),
                "status": record.get("status", "missing"),
                "intent": record.get("intent", "missing"),
                "owner": record.get("owner", "missing"),
                "required": record.get("required", "missing"),
                "plan": record.get("plan_quality", record.get("plan", "none")),
                "boundary": record.get("tool_boundary", record.get("boundary", "none")),
                "verification": record.get("verification_status", record.get("verification", "unknown")),
                "closeout": record.get("closeout", "missing"),
                "runtime_spine": record.get("runtime_spine", "coverage=0/7, status=none, missing=none"),
                "route_recovery": record.get("route_recovery", "events=0, read_search=false, mutation_blocked=false, safety=missing"),
                "route_recovery_events": record.get("route_recovery_events", "0"),
                "route_recovery_failure_types": record.get("route_recovery_failure_types", "none"),
                "route_recovery_kinds": record.get("route_recovery_kinds", "none"),
                "route_recovery_read_search_expanded": record.get("route_recovery_read_search_expanded", "false"),
                "route_recovery_mutation_blocked": record.get("route_recovery_mutation_blocked", "false"),
                "route_recovery_safety_monotonic": record.get("route_recovery_safety_monotonic", "missing"),
                "route_recovery_unsafe_mutation_expansion": record.get("route_recovery_unsafe_mutation_expansion", "false"),
                "outcome_score": record.get("outcome_score", "missing"),
                "process_score": record.get("process_score", "missing"),
                "efficiency_score": record.get("efficiency_score", "missing"),
                "agent_score": record.get("agent_score", "missing"),
                "invalid_action_count": record.get("invalid_action_count", "0"),
                "premature_edit_count": record.get("premature_edit_count", "0"),
                "scope_drift_count": record.get("scope_drift_count", "0"),
                "repeated_action_count": record.get("repeated_action_count", "0"),
                "failed_action_count": record.get("failed_action_count", "0"),
                "score_penalties": record.get("score_penalties", "none"),
                "runtime_diet": record.get("runtime_diet", "missing"),
                "behavior_assertions": record.get("behavior_assertions", "none"),
                "behavior_assertion_status": record.get("behavior_status", record.get("behavior_assertion_status", "none")),
                "triggers": record.get("triggers", "none"),
                "first_write": record.get("first_write", "none"),
                "diff": record.get("diff", "no"),
                "memory": record.get("memory", "active=false, recalled=0, conflicts=0, changed_plan=false"),
                "skill": record.get("skill", "active=false, tool_calls=0, usage_events=0, promotion=false"),
                "warnings": record.get("warnings", "none"),
                "failures": [],
            })
    return rows

def specialty_flag(summary, key):
    match = re.search(rf"{re.escape(key)}=(true|false)", summary)
    return match.group(1) if match else "false"

def specialty_int(summary, key):
    match = re.search(rf"{re.escape(key)}=(\d+)", summary)
    return int(match.group(1)) if match else 0

def summary_field(summary, key, default="missing"):
    match = re.search(rf"{re.escape(key)}=([^, ]+)", summary)
    return match.group(1) if match else default

def as_int(value, default=0):
    try:
        return int(value)
    except (TypeError, ValueError):
        return default

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
        rows = table_rows(text)
    if not rows:
        continue
    passed = sum(1 for row in rows if row["status"] == "passed")
    failed = sum(1 for row in rows if row["status"] == "failed")
    total = len(rows)
    scored = passed + failed
    skipped = total - scored
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
        "scored": scored,
        "skipped": skipped,
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
            "runtime_spine": row.get("runtime_spine", "coverage=0/7, status=none, missing=none"),
            "outcome_score": row.get("outcome_score", "missing"),
            "process_score": row.get("process_score", "missing"),
            "efficiency_score": row.get("efficiency_score", "missing"),
            "agent_score": row.get("agent_score", "missing"),
            "invalid_action_count": row.get("invalid_action_count", "0"),
            "premature_edit_count": row.get("premature_edit_count", "0"),
            "scope_drift_count": row.get("scope_drift_count", "0"),
            "repeated_action_count": row.get("repeated_action_count", "0"),
            "failed_action_count": row.get("failed_action_count", "0"),
            "score_penalties": row.get("score_penalties", "none"),
            "runtime_spine_assertions": row.get("runtime_spine_assertions", "none"),
            "runtime_spine_status": row.get(
                "runtime_spine_status",
                summary_field(row.get("runtime_spine", ""), "status", "none"),
            ),
            "runtime_spine_phase_coverage": row.get(
                "runtime_spine_phase_coverage",
                summary_field(row.get("runtime_spine", ""), "coverage", "0/7"),
            ),
            "runtime_spine_missing": row.get(
                "runtime_spine_missing",
                summary_field(row.get("runtime_spine", ""), "missing", "none"),
            ),
            "runtime_spine_trace_present": row.get("runtime_spine_trace_present", "false"),
            "route_recovery": row.get("route_recovery", "events=0, read_search=false, mutation_blocked=false, safety=missing"),
            "route_recovery_events": row.get("route_recovery_events", "0"),
            "route_recovery_failure_types": row.get("route_recovery_failure_types", "none"),
            "route_recovery_kinds": row.get("route_recovery_kinds", "none"),
            "route_recovery_read_search_expanded": row.get("route_recovery_read_search_expanded", "false"),
            "route_recovery_mutation_blocked": row.get("route_recovery_mutation_blocked", "false"),
            "route_recovery_safety_monotonic": row.get("route_recovery_safety_monotonic", "missing"),
            "route_recovery_unsafe_mutation_expansion": row.get("route_recovery_unsafe_mutation_expansion", "false"),
            "gate_outcomes": row.get("gate_outcomes", "total=0, protective_block=0, recoverable_friction=0, unrecovered_block=0, suspected_false_positive=0, policy_correct_but_ux_costly=0, harmless_pass=0"),
            "gate_outcome_records": row.get("gate_outcome_records", "none"),
            "gate_outcome_total": row.get("gate_outcome_total", "0"),
            "gate_outcome_protective_blocks": row.get("gate_outcome_protective_blocks", "0"),
            "gate_outcome_recoverable_friction": row.get("gate_outcome_recoverable_friction", "0"),
            "gate_outcome_unrecovered_blocks": row.get("gate_outcome_unrecovered_blocks", "0"),
            "gate_outcome_suspected_false_positives": row.get("gate_outcome_suspected_false_positives", "0"),
            "gate_outcome_policy_correct_but_ux_costly": row.get("gate_outcome_policy_correct_but_ux_costly", "0"),
            "gate_outcome_harmless_passes": row.get("gate_outcome_harmless_passes", "0"),
            "gate_outcome_failure_owners": row.get("gate_outcome_failure_owners", "none"),
            "verification_proof_status": row.get("verification_proof_status", "missing"),
            "verification_proof_kinds": row.get("verification_proof_kinds", "none"),
            "verification_proof_support_status": row.get("verification_proof_support_status", "missing"),
            "verification_proof_supports_verified": row.get("verification_proof_supports_verified", "false"),
            "verification_proof_residual_risk": row.get("verification_proof_residual_risk", "false"),
            "runtime_profile": row.get("runtime_profile", "none"),
            "mva_profile_active": row.get("mva_profile_active", "false"),
            "behavior_assertions": row.get("behavior_assertions", "none"),
            "behavior_assertion_status": row.get("behavior_assertion_status", "none"),
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
        record["memory_candidate_typed"] = row.get("memory_candidate_typed", "false")
        record["memory_candidate_has_evidence"] = row.get("memory_candidate_has_evidence", "false")
        record["memory_proposal_recorded"] = row.get("memory_proposal_recorded", "false")
        record["memory_proposal_status"] = row.get("memory_proposal_status", "missing")
        record["memory_proposal_candidates"] = as_int(row.get("memory_proposal_candidates", "0"))
        record["memory_proposal_kinds"] = row.get("memory_proposal_kinds", "none")
        record["memory_proposal_evidence_items"] = as_int(row.get("memory_proposal_evidence_items", "0"))
        record["memory_proposal_write_policy"] = row.get("memory_proposal_write_policy", "missing")
        record["memory_proposal_write_performed"] = row.get("memory_proposal_write_performed", "false")
        record["memory_record_used"] = row.get("memory_record_used", "false")
        record["memory_use_count_updated"] = row.get("memory_use_count_updated", "false")
        record["memory_failure_lesson_promoted"] = row.get("memory_failure_lesson_promoted", "false")
        record["memory_action_weight_changed"] = row.get("memory_action_weight_changed", "false")
        record["memory_stale_demoted"] = row.get("memory_stale_demoted", "false")
        record["memory_scope_correct"] = row.get("memory_scope_correct", "false")
        record["agent_loop_steps"] = as_int(row.get("agent_loop_steps", "0"))
        record["context_zones_materialized"] = row.get("context_zones_materialized", "false")
        record["context_zone_task_state_empty"] = row.get("context_zone_task_state_empty", "false")
        record["context_zone_current_decision_request_empty"] = row.get("context_zone_current_decision_request_empty", "false")
        record["state_transition_recorded"] = row.get("state_transition_recorded", "false")
        record["completion_contract_status"] = row.get("completion_contract_status", "missing")
        record["candidate_score_calibrated"] = row.get("candidate_score_calibrated", "false")
        record["candidate_score_disagreement"] = row.get("candidate_score_disagreement", "false")
        record["observer_outcome_recorded"] = row.get("observer_outcome_recorded", "false")
        record["memory_boundary_recorded"] = row.get("memory_boundary_recorded", "false")
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
scored_tasks = passed_tasks + failed_tasks
skipped_tasks = total_tasks - scored_tasks
real_code_passes = sum(record["real_code_passes"] for record in run_records)
plan_only_passes = sum(record["plan_only_passes"] for record in run_records)
seeded_no_diff = sum(record["seeded_no_diff"] for record in run_records)
required_failed = sum(1 for record in task_records if record["required"] == "failed")
verification_failed = sum(1 for record in task_records if record["verification"] == "failed")
memory_active_tasks = sum(1 for record in task_records if record["memory_active"] == "true")
memory_changed_plan_tasks = sum(1 for record in task_records if record["memory_changed_plan"] == "true")
memory_recalled_items = sum(record["memory_recalled_items"] for record in task_records)
memory_conflicts = sum(record["memory_conflicts"] for record in task_records)
memory_candidate_typed_tasks = sum(1 for record in task_records if record["memory_candidate_typed"] == "true")
memory_candidate_evidence_tasks = sum(1 for record in task_records if record["memory_candidate_has_evidence"] == "true")
memory_proposal_tasks = sum(1 for record in task_records if record["memory_proposal_recorded"] == "true")
memory_proposal_candidates = sum(record["memory_proposal_candidates"] for record in task_records)
memory_proposal_evidence_items = sum(record["memory_proposal_evidence_items"] for record in task_records)
memory_proposal_review_required_tasks = sum(
    1 for record in task_records if record["memory_proposal_write_policy"] == "review_required"
)
memory_record_used_tasks = sum(1 for record in task_records if record["memory_record_used"] == "true")
memory_use_count_updated_tasks = sum(1 for record in task_records if record["memory_use_count_updated"] == "true")
memory_failure_lesson_promoted_tasks = sum(1 for record in task_records if record["memory_failure_lesson_promoted"] == "true")
memory_action_weight_changed_tasks = sum(1 for record in task_records if record["memory_action_weight_changed"] == "true")
memory_stale_demoted_tasks = sum(1 for record in task_records if record["memory_stale_demoted"] == "true")
memory_scope_correct_tasks = sum(1 for record in task_records if record["memory_scope_correct"] == "true")
skill_active_tasks = sum(1 for record in task_records if record["skill_active"] == "true")
skill_tool_calls = sum(record["skill_tool_calls"] for record in task_records)
skill_usage_events = sum(record["skill_usage_events"] for record in task_records)
skill_promotion_tasks = sum(1 for record in task_records if record["skill_promotion_evidence"] == "true")
behavior_assertion_tasks = sum(1 for record in task_records if record["behavior_assertions"] != "none")
behavior_assertion_passed = sum(1 for record in task_records if record["behavior_assertion_status"] == "passed")
memory_behavior_assertion_tasks = sum(
    1 for record in task_records if "memory" in record["behavior_assertions"].lower()
)
skill_behavior_assertion_tasks = sum(
    1 for record in task_records if "skill" in record["behavior_assertions"].lower()
)
runtime_spine_assertion_tasks = sum(1 for record in task_records if record["runtime_spine_assertions"] != "none")
runtime_spine_assertions_passed = sum(1 for record in task_records if record["runtime_spine_status"] == "passed")
runtime_spine_assertions_failed = sum(
    1 for record in task_records if record["runtime_spine_status"] in {"failed", "missing"}
)
runtime_spine_full_coverage = sum(
    1 for record in task_records if record["runtime_spine_phase_coverage"] == "7/7"
)
runtime_spine_trace_present = sum(
    1 for record in task_records if record["runtime_spine_trace_present"] == "true"
)
runtime_spine_agent_loop_steps = sum(record["agent_loop_steps"] for record in task_records)
runtime_spine_context_zone_tasks = sum(
    1 for record in task_records if record["context_zones_materialized"] == "true"
)
runtime_spine_stage_transition_tasks = sum(
    1 for record in task_records if record["state_transition_recorded"] == "true"
)
runtime_spine_completion_contract_tasks = sum(
    1 for record in task_records if record["completion_contract_status"] not in {"", "missing", "none"}
)
route_recovery_tasks = sum(1 for record in task_records if as_int(record["route_recovery_events"]) > 0)
route_recovery_events = sum(as_int(record["route_recovery_events"]) for record in task_records)
route_recovery_read_search_tasks = sum(
    1 for record in task_records if record["route_recovery_read_search_expanded"] == "true"
)
route_recovery_mutation_blocked_tasks = sum(
    1 for record in task_records if record["route_recovery_mutation_blocked"] == "true"
)
route_recovery_safety_monotonic_tasks = sum(
    1 for record in task_records if record["route_recovery_safety_monotonic"] == "true"
)
route_recovery_unsafe_mutation_expansion_tasks = sum(
    1 for record in task_records if record["route_recovery_unsafe_mutation_expansion"] == "true"
)
gate_outcome_tasks = sum(1 for record in task_records if as_int(record["gate_outcome_total"]) > 0)
gate_outcome_total = sum(as_int(record["gate_outcome_total"]) for record in task_records)
gate_outcome_protective_blocks = sum(
    as_int(record["gate_outcome_protective_blocks"]) for record in task_records
)
gate_outcome_recoverable_friction = sum(
    as_int(record["gate_outcome_recoverable_friction"]) for record in task_records
)
gate_outcome_unrecovered_blocks = sum(
    as_int(record["gate_outcome_unrecovered_blocks"]) for record in task_records
)
gate_outcome_suspected_false_positives = sum(
    as_int(record["gate_outcome_suspected_false_positives"]) for record in task_records
)
gate_outcome_policy_correct_but_ux_costly = sum(
    as_int(record["gate_outcome_policy_correct_but_ux_costly"]) for record in task_records
)
gate_outcome_harmless_passes = sum(
    as_int(record["gate_outcome_harmless_passes"]) for record in task_records
)
proof_support_verified_tasks = sum(
    1 for record in task_records if record["verification_proof_support_status"] == "verified"
)
proof_support_partial_tasks = sum(
    1 for record in task_records if record["verification_proof_support_status"] == "partial"
)
proof_support_not_verified_tasks = sum(
    1
    for record in task_records
    if record["verification_proof_support_status"]
    in {"failed", "not_run", "blocked", "user_deferred", "unavailable"}
)
proof_support_residual_risk_tasks = sum(
    1 for record in task_records if record["verification_proof_residual_risk"] == "true"
)
mva_profile_tasks = sum(1 for record in task_records if record["mva_profile_active"] == "true")
mva_profile_runtime_spine_passed = sum(
    1
    for record in task_records
    if record["mva_profile_active"] == "true" and record["runtime_spine_status"] == "passed"
)
candidate_score_calibrated_tasks = sum(
    1 for record in task_records if record["candidate_score_calibrated"] == "true"
)
candidate_score_disagreement_tasks = sum(
    1 for record in task_records if record["candidate_score_disagreement"] == "true"
)
observer_outcome_tasks = sum(
    1 for record in task_records if record["observer_outcome_recorded"] == "true"
)
memory_boundary_tasks = sum(
    1 for record in task_records if record["memory_boundary_recorded"] == "true"
)
score_records = [record for record in task_records if record.get("agent_score") != "missing"]
avg_outcome_score = (
    sum(as_int(record.get("outcome_score")) for record in score_records) / len(score_records)
    if score_records
    else 0
)
avg_process_score = (
    sum(as_int(record.get("process_score")) for record in score_records) / len(score_records)
    if score_records
    else 0
)
avg_efficiency_score = (
    sum(as_int(record.get("efficiency_score")) for record in score_records) / len(score_records)
    if score_records
    else 0
)
avg_agent_score = (
    sum(as_int(record.get("agent_score")) for record in score_records) / len(score_records)
    if score_records
    else 0
)
invalid_actions_total = sum(as_int(record.get("invalid_action_count")) for record in task_records)
premature_edits_total = sum(as_int(record.get("premature_edit_count")) for record in task_records)
scope_drifts_total = sum(as_int(record.get("scope_drift_count")) for record in task_records)
repeated_actions_total = sum(as_int(record.get("repeated_action_count")) for record in task_records)
failed_actions_total = sum(as_int(record.get("failed_action_count")) for record in task_records)
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
    f"- Scored task reports: `{scored_tasks}`",
    f"- Pass rate: `{passed_tasks}/{scored_tasks}` ({pct(passed_tasks, scored_tasks)})",
    f"- Failure rate: `{failed_tasks}/{scored_tasks}` ({pct(failed_tasks, scored_tasks)})",
    f"- Skipped/unscored task reports: `{skipped_tasks}`",
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
        ["skipped_unscored_tasks", skipped_tasks, pct(skipped_tasks, total_tasks)],
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
        [
            "behavior_assertion_metadata",
            behavior_assertion_tasks,
            pct(behavior_assertion_tasks, total_tasks),
        ],
        [
            "runtime_spine_assertion_metadata",
            runtime_spine_assertion_tasks,
            pct(runtime_spine_assertion_tasks, total_tasks),
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
        ["memory_candidate_typed_tasks", memory_candidate_typed_tasks, pct(memory_candidate_typed_tasks, total_tasks)],
        ["memory_candidate_evidence_tasks", memory_candidate_evidence_tasks, pct(memory_candidate_evidence_tasks, total_tasks)],
        ["memory_proposal_tasks", memory_proposal_tasks, pct(memory_proposal_tasks, total_tasks)],
        ["memory_proposal_candidates", memory_proposal_candidates, "n/a"],
        ["memory_proposal_evidence_items", memory_proposal_evidence_items, "n/a"],
        ["memory_proposal_review_required_tasks", memory_proposal_review_required_tasks, pct(memory_proposal_review_required_tasks, total_tasks)],
        ["memory_record_used_tasks", memory_record_used_tasks, pct(memory_record_used_tasks, total_tasks)],
        ["memory_use_count_updated_tasks", memory_use_count_updated_tasks, pct(memory_use_count_updated_tasks, total_tasks)],
        ["memory_failure_lesson_promoted_tasks", memory_failure_lesson_promoted_tasks, pct(memory_failure_lesson_promoted_tasks, total_tasks)],
        ["memory_action_weight_changed_tasks", memory_action_weight_changed_tasks, pct(memory_action_weight_changed_tasks, total_tasks)],
        ["memory_stale_demoted_tasks", memory_stale_demoted_tasks, pct(memory_stale_demoted_tasks, total_tasks)],
        ["memory_scope_correct_tasks", memory_scope_correct_tasks, pct(memory_scope_correct_tasks, total_tasks)],
        ["skill_active_tasks", skill_active_tasks, pct(skill_active_tasks, total_tasks)],
        ["skill_tool_calls", skill_tool_calls, "n/a"],
        ["skill_usage_events", skill_usage_events, "n/a"],
        ["skill_promotion_evidence_tasks", skill_promotion_tasks, pct(skill_promotion_tasks, total_tasks)],
        ["behavior_assertion_tasks", behavior_assertion_tasks, pct(behavior_assertion_tasks, total_tasks)],
        ["behavior_assertions_passed", behavior_assertion_passed, pct(behavior_assertion_passed, behavior_assertion_tasks)],
        ["memory_behavior_assertion_tasks", memory_behavior_assertion_tasks, pct(memory_behavior_assertion_tasks, total_tasks)],
        ["skill_behavior_assertion_tasks", skill_behavior_assertion_tasks, pct(skill_behavior_assertion_tasks, total_tasks)],
    ],
))

lines.extend(["", "## Runtime Spine Evidence", ""])
lines.extend(md_table(
    ["dimension", "count", "share"],
    [
        ["runtime_spine_assertion_tasks", runtime_spine_assertion_tasks, pct(runtime_spine_assertion_tasks, total_tasks)],
        ["runtime_spine_assertions_passed", runtime_spine_assertions_passed, pct(runtime_spine_assertions_passed, runtime_spine_assertion_tasks)],
        ["runtime_spine_assertions_failed", runtime_spine_assertions_failed, pct(runtime_spine_assertions_failed, runtime_spine_assertion_tasks)],
        ["runtime_spine_full_coverage_tasks", runtime_spine_full_coverage, pct(runtime_spine_full_coverage, total_tasks)],
        ["runtime_spine_trace_present_tasks", runtime_spine_trace_present, pct(runtime_spine_trace_present, total_tasks)],
        ["runtime_spine_agent_loop_steps", runtime_spine_agent_loop_steps, "n/a"],
        ["runtime_spine_context_zone_tasks", runtime_spine_context_zone_tasks, pct(runtime_spine_context_zone_tasks, total_tasks)],
        ["runtime_spine_stage_transition_tasks", runtime_spine_stage_transition_tasks, pct(runtime_spine_stage_transition_tasks, total_tasks)],
        ["runtime_spine_completion_contract_tasks", runtime_spine_completion_contract_tasks, pct(runtime_spine_completion_contract_tasks, total_tasks)],
        ["route_recovery_tasks", route_recovery_tasks, pct(route_recovery_tasks, total_tasks)],
        ["route_recovery_events", route_recovery_events, "n/a"],
        ["route_recovery_read_search_expansions", route_recovery_read_search_tasks, pct(route_recovery_read_search_tasks, total_tasks)],
        ["route_recovery_mutation_blocks", route_recovery_mutation_blocked_tasks, pct(route_recovery_mutation_blocked_tasks, total_tasks)],
        ["route_recovery_safety_monotonic_tasks", route_recovery_safety_monotonic_tasks, pct(route_recovery_safety_monotonic_tasks, total_tasks)],
        ["route_recovery_unsafe_mutation_expansion_tasks", route_recovery_unsafe_mutation_expansion_tasks, pct(route_recovery_unsafe_mutation_expansion_tasks, total_tasks)],
        ["gate_outcome_tasks", gate_outcome_tasks, pct(gate_outcome_tasks, total_tasks)],
        ["gate_outcome_records", gate_outcome_total, "n/a"],
        ["gate_outcome_protective_blocks", gate_outcome_protective_blocks, "n/a"],
        ["gate_outcome_recoverable_friction", gate_outcome_recoverable_friction, "n/a"],
        ["gate_outcome_unrecovered_blocks", gate_outcome_unrecovered_blocks, "n/a"],
        ["gate_outcome_suspected_false_positives", gate_outcome_suspected_false_positives, "n/a"],
        ["gate_outcome_policy_correct_but_ux_costly", gate_outcome_policy_correct_but_ux_costly, "n/a"],
        ["gate_outcome_harmless_passes", gate_outcome_harmless_passes, "n/a"],
        ["proof_support_verified_tasks", proof_support_verified_tasks, pct(proof_support_verified_tasks, total_tasks)],
        ["proof_support_partial_tasks", proof_support_partial_tasks, pct(proof_support_partial_tasks, total_tasks)],
        ["proof_support_not_verified_tasks", proof_support_not_verified_tasks, pct(proof_support_not_verified_tasks, total_tasks)],
        ["proof_support_residual_risk_tasks", proof_support_residual_risk_tasks, pct(proof_support_residual_risk_tasks, total_tasks)],
        ["mva_profile_tasks", mva_profile_tasks, pct(mva_profile_tasks, total_tasks)],
        ["mva_profile_runtime_spine_passed", mva_profile_runtime_spine_passed, pct(mva_profile_runtime_spine_passed, mva_profile_tasks)],
        ["candidate_score_calibrated_tasks", candidate_score_calibrated_tasks, pct(candidate_score_calibrated_tasks, total_tasks)],
        ["candidate_score_disagreement_tasks", candidate_score_disagreement_tasks, pct(candidate_score_disagreement_tasks, total_tasks)],
        ["observer_outcome_tasks", observer_outcome_tasks, pct(observer_outcome_tasks, total_tasks)],
        ["memory_boundary_tasks", memory_boundary_tasks, pct(memory_boundary_tasks, total_tasks)],
    ],
))

lines.extend(["", "### Route Recovery Matrix", ""])
lines.extend(md_table(
    [
        "run",
        "task",
        "events",
        "kinds",
        "failure_types",
        "read_search",
        "mutation_blocked",
        "safety",
        "unsafe_mutation_expansion",
    ],
    [
        [
            record["run"],
            record["task"],
            record["route_recovery_events"],
            record["route_recovery_kinds"],
            record["route_recovery_failure_types"],
            record["route_recovery_read_search_expanded"],
            record["route_recovery_mutation_blocked"],
            record["route_recovery_safety_monotonic"],
            record["route_recovery_unsafe_mutation_expansion"],
        ]
        for record in task_records
        if as_int(record["route_recovery_events"]) > 0
    ] or [["none", "none", 0, "none", "none", "false", "false", "missing", "false"]],
))

lines.extend(["", "## Evaluation Scores", ""])
lines.extend(md_table(
    ["dimension", "value", "share"],
    [
        ["score_task_reports", len(score_records), pct(len(score_records), total_tasks)],
        ["outcome_score_avg", f"{avg_outcome_score:.1f}", "n/a"],
        ["process_score_avg", f"{avg_process_score:.1f}", "n/a"],
        ["efficiency_score_avg", f"{avg_efficiency_score:.1f}", "n/a"],
        ["agent_score_avg", f"{avg_agent_score:.1f}", "n/a"],
        ["invalid_actions_total", invalid_actions_total, "n/a"],
        ["premature_edits_total", premature_edits_total, "n/a"],
        ["scope_drifts_total", scope_drifts_total, "n/a"],
        ["repeated_actions_total", repeated_actions_total, "n/a"],
        ["failed_actions_total", failed_actions_total, "n/a"],
    ],
))

lines.extend(["", "### Lowest Agent Scores", ""])
lowest_score_records = sorted(
    score_records,
    key=lambda record: as_int(record.get("agent_score"), 100),
)[:20]
lines.extend(md_table(
    ["run", "task", "status", "agent", "outcome", "process", "efficiency", "invalid", "penalties"],
    [
        [
            record["run"],
            record["task"],
            record["status"],
            record.get("agent_score", "missing"),
            record.get("outcome_score", "missing"),
            record.get("process_score", "missing"),
            record.get("efficiency_score", "missing"),
            record.get("invalid_action_count", "0"),
            record.get("score_penalties", "none"),
        ]
        for record in lowest_score_records
    ] or [["none", "none", "none", "0", "0", "0", "0", "0", "none"]],
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
    ["run", "task", "intent", "owner", "inferred_owner", "required", "verification", "diff", "spine", "behavior", "behavior_status", "memory", "skill", "triggers", "warnings"],
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
            record["runtime_spine"],
            record["behavior_assertions"],
            record["behavior_assertion_status"],
            record["memory"],
            record["skill"],
            record["triggers"],
            record["warnings"],
        ]
        for record in recent_failures
    ] or [["none", "none", "none", "none", "none", "none", "none", "none", "none", "none", "none", "none", "none", "none", "none"]],
))

lines.extend(["", "## Recent Passed Tasks", ""])
lines.extend(md_table(
    ["run", "task", "intent", "owner", "required", "verification", "diff", "spine", "behavior", "behavior_status", "memory", "skill", "triggers", "warnings"],
    [
        [
            record["run"],
            record["task"],
            record["intent"],
            record["owner"],
            record["required"],
            record["verification"],
            record["diff"],
            record["runtime_spine"],
            record["behavior_assertions"],
            record["behavior_assertion_status"],
            record["memory"],
            record["skill"],
            record["triggers"],
            record["warnings"],
        ]
        for record in recent_passes
    ] or [["none", "none", "none", "none", "none", "none", "none", "none", "none", "none", "none", "none", "none", "none"]],
))

lines.extend([
    "",
    "## Reading",
    "",
    "- `real_code_change_passed` requires an agent-run report with a non-empty diff.",
    "- `plan_only_passed` is tracked separately so planning success is not counted as code-change success.",
    "- `skipped` reports are excluded from pass/fail rate denominators; collect-only reports need passing required commands to be scored.",
    "- `seeded_no_diff_failed` is the strongest signal for agents that inspect but do not patch.",
    "- `inferred_owner` is a conservative backfill for older reports that predate structured `failure_owner` fields.",
    "- `owner_metadata_missing` tracks that historical evidence gap separately from inferred product failures.",
    "- `instrumented_task_reports` is the cleaner current slice because it excludes reports with no structured owner, intent, or trigger metadata.",
    "- `memory` and `skill` columns show evidence signals only; they are not success labels.",
    "- `behavior_assertions` are sample-level checks that turn memory/skill semantics into explicit pass/fail evidence.",
    "- `runtime_spine` tracks trace/control-loop coverage and sample-level runtime assertions.",
])

output.write_text("\n".join(lines).rstrip() + "\n", encoding="utf-8")
print(output)
PY
