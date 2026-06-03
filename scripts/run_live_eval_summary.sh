# Sourced by scripts/run_live_eval.sh. Keep functions side-effect-free at source time.

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
failure_classes = {}
for row in rows:
    for klass in row.get("failure_classes", []):
        failure_classes[klass] = failure_classes.get(klass, 0) + 1

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
memory_candidate_typed_tasks = sum(1 for row in rows if row.get("memory_candidate_typed") == "true")
memory_candidate_evidence_tasks = sum(1 for row in rows if row.get("memory_candidate_has_evidence") == "true")
memory_proposal_tasks = sum(1 for row in rows if row.get("memory_proposal_recorded") == "true")
memory_proposal_candidates = sum(as_int(row.get("memory_proposal_candidates")) for row in rows)
memory_proposal_evidence_items = sum(as_int(row.get("memory_proposal_evidence_items")) for row in rows)
memory_proposal_review_required_tasks = sum(
    1 for row in rows if row.get("memory_proposal_write_policy") == "review_required"
)
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
runtime_spine_assertion_tasks = sum(1 for row in rows if row["runtime_spine_assertions"] != "none")
runtime_spine_assertions_passed = sum(1 for row in rows if row["runtime_spine_status"] == "passed")
runtime_spine_assertions_failed = sum(1 for row in rows if row["runtime_spine_status"] in {"failed", "missing"})
runtime_spine_full_coverage = sum(1 for row in rows if row["runtime_spine_phase_coverage"] == "7/7")
runtime_spine_trace_present = sum(1 for row in rows if row["runtime_spine_trace_present"] == "true")
runtime_spine_risky_tool_runs = sum(as_int(row["risky_tool_runs"]) for row in rows)
runtime_spine_risky_tool_reviewed = sum(as_int(row["risky_tool_reviewed"]) for row in rows)
runtime_spine_risky_missing_review_tasks = sum(
    1 for row in rows if row["risky_tool_missing_action_review"] != "none"
)
route_recovery_tasks = sum(1 for row in rows if as_int(row.get("route_recovery_events")) > 0)
route_recovery_events = sum(as_int(row.get("route_recovery_events")) for row in rows)
route_recovery_read_search_tasks = sum(
    1 for row in rows if row.get("route_recovery_read_search_expanded") == "true"
)
route_recovery_mutation_blocked_tasks = sum(
    1 for row in rows if row.get("route_recovery_mutation_blocked") == "true"
)
route_recovery_safety_monotonic_tasks = sum(
    1 for row in rows if row.get("route_recovery_safety_monotonic") == "true"
)
route_recovery_unsafe_mutation_expansion_tasks = sum(
    1 for row in rows if row.get("route_recovery_unsafe_mutation_expansion") == "true"
)
context_zone_envelope_tasks = sum(1 for row in rows if as_int(row.get("context_zone_envelope_messages")) > 0)
context_zone_envelope_messages = sum(as_int(row.get("context_zone_envelope_messages")) for row in rows)
context_zone_source_messages = sum(as_int(row.get("context_zone_source_messages")) for row in rows)
context_zone_duplicate_blocks_removed = sum(as_int(row.get("context_zone_duplicate_blocks_removed")) for row in rows)
context_zone_provenance_markers = sum(as_int(row.get("context_zone_provenance_markers")) for row in rows)
gate_outcome_tasks = sum(1 for row in rows if as_int(row.get("gate_outcome_total")) > 0)
gate_outcome_total = sum(as_int(row.get("gate_outcome_total")) for row in rows)
gate_outcome_protective_blocks = sum(as_int(row.get("gate_outcome_protective_blocks")) for row in rows)
gate_outcome_recoverable_friction = sum(as_int(row.get("gate_outcome_recoverable_friction")) for row in rows)
gate_outcome_unrecovered_blocks = sum(as_int(row.get("gate_outcome_unrecovered_blocks")) for row in rows)
gate_outcome_suspected_false_positives = sum(as_int(row.get("gate_outcome_suspected_false_positives")) for row in rows)
gate_outcome_policy_correct_but_ux_costly = sum(as_int(row.get("gate_outcome_policy_correct_but_ux_costly")) for row in rows)
gate_outcome_harmless_passes = sum(as_int(row.get("gate_outcome_harmless_passes")) for row in rows)
proof_support_verified_tasks = sum(1 for row in rows if row.get("verification_proof_support_status") == "verified")
proof_support_partial_tasks = sum(1 for row in rows if row.get("verification_proof_support_status") == "partial")
proof_support_not_verified_tasks = sum(
    1
    for row in rows
    if row.get("verification_proof_support_status")
    in {"failed", "not_run", "blocked", "user_deferred", "unavailable"}
)
proof_support_residual_risk_tasks = sum(1 for row in rows if row.get("verification_proof_residual_risk") == "true")
scored_score_rows = [row for row in rows if row.get("agent_score", "missing") != "missing"]
avg_outcome_score = (
    sum(as_int(row.get("outcome_score")) for row in scored_score_rows) / len(scored_score_rows)
    if scored_score_rows
    else 0
)
avg_process_score = (
    sum(as_int(row.get("process_score")) for row in scored_score_rows) / len(scored_score_rows)
    if scored_score_rows
    else 0
)
avg_efficiency_score = (
    sum(as_int(row.get("efficiency_score")) for row in scored_score_rows) / len(scored_score_rows)
    if scored_score_rows
    else 0
)
avg_agent_score = (
    sum(as_int(row.get("agent_score")) for row in scored_score_rows) / len(scored_score_rows)
    if scored_score_rows
    else 0
)
invalid_actions_total = sum(as_int(row.get("invalid_action_count")) for row in rows)
premature_edits_total = sum(as_int(row.get("premature_edit_count")) for row in rows)
scope_drifts_total = sum(as_int(row.get("scope_drift_count")) for row in rows)
repeated_actions_total = sum(as_int(row.get("repeated_action_count")) for row in rows)
failed_actions_total = sum(as_int(row.get("failed_action_count")) for row in rows)
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
    f"- Memory typed-candidate tasks: `{memory_candidate_typed_tasks}`",
    f"- Memory evidence-backed candidate tasks: `{memory_candidate_evidence_tasks}`",
    f"- Memory proposal tasks: `{memory_proposal_tasks}`",
    f"- Memory proposal candidates: `{memory_proposal_candidates}`",
    f"- Memory proposal evidence items: `{memory_proposal_evidence_items}`",
    f"- Memory proposal review-required tasks: `{memory_proposal_review_required_tasks}`",
    f"- Skill active tasks: `{skill_active_tasks}`",
    f"- Skill promotion-evidence tasks: `{skill_promotion_tasks}`",
    f"- Behavior assertion tasks: `{behavior_assertion_tasks}`",
    f"- Behavior assertions passed: `{behavior_assertion_passed}`",
    f"- Runtime-spine assertion tasks: `{runtime_spine_assertion_tasks}`",
    f"- Runtime-spine assertions passed: `{runtime_spine_assertions_passed}`",
    f"- Runtime-spine assertions failed: `{runtime_spine_assertions_failed}`",
    f"- Runtime-spine full coverage tasks: `{runtime_spine_full_coverage}`",
    f"- Runtime-spine trace-present tasks: `{runtime_spine_trace_present}`",
    f"- Runtime-spine risky tool runs: `{runtime_spine_risky_tool_runs}`",
    f"- Runtime-spine risky tool reviewed: `{runtime_spine_risky_tool_reviewed}`",
    f"- Runtime-spine risky missing-review tasks: `{runtime_spine_risky_missing_review_tasks}`",
    f"- Route recovery tasks: `{route_recovery_tasks}`",
    f"- Route recovery events: `{route_recovery_events}`",
    f"- Route recovery read/search expansions: `{route_recovery_read_search_tasks}`",
    f"- Route recovery mutation blocks: `{route_recovery_mutation_blocked_tasks}`",
    f"- Route recovery safety-monotonic tasks: `{route_recovery_safety_monotonic_tasks}`",
    f"- Route recovery unsafe mutation-expansion tasks: `{route_recovery_unsafe_mutation_expansion_tasks}`",
    f"- Context-zone envelope tasks: `{context_zone_envelope_tasks}`",
    f"- Context-zone envelope messages: `{context_zone_envelope_messages}`",
    f"- Context-zone source messages: `{context_zone_source_messages}`",
    f"- Context-zone duplicate blocks removed: `{context_zone_duplicate_blocks_removed}`",
    f"- Context-zone provenance markers: `{context_zone_provenance_markers}`",
    f"- Gate outcome tasks: `{gate_outcome_tasks}`",
    f"- Gate outcome records: `{gate_outcome_total}`",
    f"- Gate outcome protective blocks: `{gate_outcome_protective_blocks}`",
    f"- Gate outcome recoverable friction: `{gate_outcome_recoverable_friction}`",
    f"- Gate outcome unrecovered blocks: `{gate_outcome_unrecovered_blocks}`",
    f"- Gate outcome harmless passes: `{gate_outcome_harmless_passes}`",
    f"- Proof support verified tasks: `{proof_support_verified_tasks}`",
    f"- Proof support partial tasks: `{proof_support_partial_tasks}`",
    f"- Proof support not-verified tasks: `{proof_support_not_verified_tasks}`",
    f"- Proof support residual-risk tasks: `{proof_support_residual_risk_tasks}`",
    f"- Average outcome score: `{avg_outcome_score:.1f}`",
    f"- Average process score: `{avg_process_score:.1f}`",
    f"- Average efficiency score: `{avg_efficiency_score:.1f}`",
    f"- Average agent score: `{avg_agent_score:.1f}`",
    f"- Invalid actions total: `{invalid_actions_total}`",
    f"- Premature edits total: `{premature_edits_total}`",
    f"- Scope drifts total: `{scope_drifts_total}`",
    f"- Repeated actions total: `{repeated_actions_total}`",
    f"- Failed actions total: `{failed_actions_total}`",
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
    "## Release Dogfood Failure Classes",
    "",
    "| class | count | meaning |",
    "|-------|-------|---------|",
])

failure_class_meanings = {
    "tool_contract": "Tool schema, exposure, result-pair, or contract boundary failures.",
    "file_state": "Read-before-edit, stale file, checkpoint, rollback, or diff-state failures.",
    "bash_permission": "Shell command structure, redirection, heredoc, or shell permission failures.",
    "permission_recovery": "Permission denial, approval, or recovery-loop failures.",
    "compaction_continuity": "Context compression, retained context, or long-run continuity failures.",
    "llm_reasoning": "Model failed to plan, edit, validate, or close out despite available tools.",
    "desktop_evidence": "Desktop UI, screenshot, native smoke, or visual evidence failures.",
}

if failure_classes:
    for key, value in sorted(failure_classes.items(), key=lambda item: (-item[1], item[0])):
        lines.append(f"| {key} | {value} | {failure_class_meanings.get(key, 'Unclassified failure class.')} |")
else:
    lines.append("| none | 0 | No classified failures. |")

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
    f"| memory_candidate_typed_tasks | {memory_candidate_typed_tasks} | Tasks with typed memory candidates, including review-only MemoryProposal candidates. |",
    f"| memory_candidate_evidence_tasks | {memory_candidate_evidence_tasks} | Tasks with evidence-backed memory candidates, including review-only MemoryProposal evidence. |",
    f"| memory_proposal_tasks | {memory_proposal_tasks} | Tasks that emitted a review-only MemoryProposal trace event. |",
    f"| memory_proposal_candidates | {memory_proposal_candidates} | Review-only MemoryProposal candidates proposed across tasks. |",
    f"| memory_proposal_evidence_items | {memory_proposal_evidence_items} | Evidence items attached to review-only MemoryProposal candidates. |",
    f"| memory_proposal_review_required_tasks | {memory_proposal_review_required_tasks} | MemoryProposal tasks that require review before persistence. |",
    f"| skill_active_tasks | {skill_active_tasks} | Tasks where skill tools or skill-specific signals were active. |",
    f"| skill_promotion_evidence_tasks | {skill_promotion_tasks} | Tasks with promotion-related skill evidence. |",
    f"| behavior_assertion_tasks | {behavior_assertion_tasks} | Tasks with explicit behavior assertions in the live-eval sample. |",
    f"| behavior_assertions_passed | {behavior_assertion_passed} | Explicit behavior-assertion tasks whose required checks passed. |",
    f"| memory_behavior_assertion_tasks | {memory_behavior_assertion_tasks} | Behavior assertions covering memory semantics rather than only memory activity signals. |",
    f"| skill_behavior_assertion_tasks | {skill_behavior_assertion_tasks} | Behavior assertions covering skill semantics rather than only skill activity signals. |",
    "",
    "## Runtime Spine Evidence",
    "",
    "| dimension | count | meaning |",
    "|-----------|-------|---------|",
    f"| runtime_spine_assertion_tasks | {runtime_spine_assertion_tasks} | Tasks with explicit runtime-spine assertions in the live-eval sample or report. |",
    f"| runtime_spine_assertions_passed | {runtime_spine_assertions_passed} | Runtime-spine assertion tasks whose required trace/control-loop signals were present. |",
    f"| runtime_spine_assertions_failed | {runtime_spine_assertions_failed} | Runtime-spine assertion tasks missing required trace/control-loop signals. |",
    f"| runtime_spine_full_coverage_tasks | {runtime_spine_full_coverage} | Tasks whose trace touched all runtime-spine phases. |",
    f"| runtime_spine_trace_present_tasks | {runtime_spine_trace_present} | Tasks with a trace summary available to the report parser. |",
    f"| runtime_spine_risky_tool_runs | {runtime_spine_risky_tool_runs} | Risky tool executions observed from trace or agent events. |",
    f"| runtime_spine_risky_tool_reviewed | {runtime_spine_risky_tool_reviewed} | Risky tool executions with matching action.review trace evidence. |",
    f"| runtime_spine_risky_missing_review_tasks | {runtime_spine_risky_missing_review_tasks} | Tasks with risky tool executions missing matching action.review evidence. |",
    f"| route_recovery_tasks | {route_recovery_tasks} | Tasks with route-recovery plans emitted by the runtime. |",
    f"| route_recovery_events | {route_recovery_events} | Route-recovery plans observed across task traces. |",
    f"| route_recovery_read_search_expansions | {route_recovery_read_search_tasks} | Tasks where route recovery expanded only read/search understanding tools. |",
    f"| route_recovery_mutation_blocks | {route_recovery_mutation_blocked_tasks} | Tasks where route recovery explicitly blocked silent mutation expansion. |",
    f"| route_recovery_safety_monotonic_tasks | {route_recovery_safety_monotonic_tasks} | Tasks where route recovery preserved destructive-tool authority. |",
    f"| route_recovery_unsafe_mutation_expansion_tasks | {route_recovery_unsafe_mutation_expansion_tasks} | Tasks where route recovery exposed mutation alternatives and should be investigated. |",
    f"| context_zone_envelope_tasks | {context_zone_envelope_tasks} | Tasks where dynamic context was consolidated into a primary zone-first envelope. |",
    f"| context_zone_envelope_messages | {context_zone_envelope_messages} | Consolidated context-zone envelope messages observed across tasks. |",
    f"| context_zone_source_messages | {context_zone_source_messages} | Dynamic source messages consumed into context-zone envelopes. |",
    f"| context_zone_duplicate_blocks_removed | {context_zone_duplicate_blocks_removed} | Duplicate dynamic zone blocks removed during request assembly. |",
    f"| context_zone_provenance_markers | {context_zone_provenance_markers} | Provenance markers preserved inside context-zone envelopes. |",
    f"| gate_outcome_tasks | {gate_outcome_tasks} | Tasks with derived gate-outcome records from trace or report fields. |",
    f"| gate_outcome_records | {gate_outcome_total} | Total gate-outcome records derived across action review, permission, and closeout gates. |",
    f"| gate_outcome_protective_blocks | {gate_outcome_protective_blocks} | Gate blocks that protected policy, scope, budget, checkpoint, or closeout invariants. |",
    f"| gate_outcome_recoverable_friction | {gate_outcome_recoverable_friction} | Gate friction followed by a completed or passed runtime outcome. |",
    f"| gate_outcome_unrecovered_blocks | {gate_outcome_unrecovered_blocks} | Gate blocks without later runtime recovery evidence. |",
    f"| gate_outcome_suspected_false_positives | {gate_outcome_suspected_false_positives} | Scenario-oracle suspected gate false positives. |",
    f"| gate_outcome_policy_correct_but_ux_costly | {gate_outcome_policy_correct_but_ux_costly} | Policy-correct gate decisions that still created measurable UX cost. |",
    f"| gate_outcome_harmless_passes | {gate_outcome_harmless_passes} | Gate decisions that passed without measurable friction. |",
    f"| proof_support_verified_tasks | {proof_support_verified_tasks} | Tasks whose proof-kind policy supports verified closeout. |",
    f"| proof_support_partial_tasks | {proof_support_partial_tasks} | Tasks with useful proof evidence that cannot support verified closeout. |",
    f"| proof_support_not_verified_tasks | {proof_support_not_verified_tasks} | Tasks whose proof policy blocks verified closeout. |",
    f"| proof_support_residual_risk_tasks | {proof_support_residual_risk_tasks} | Tasks whose proof support carries residual risk. |",
    "",
    "### Gate Outcome Matrix",
    "",
    "| task | total | protective | recoverable | unrecovered | suspected_false_positive | policy_correct_but_ux_costly | harmless | records | owners |",
    "|------|-------|------------|-------------|-------------|--------------------------|------------------------------|----------|---------|--------|",
])

if rows:
    for row in rows:
        lines.append(
            "| {task} | {gate_outcome_total} | {gate_outcome_protective_blocks} | {gate_outcome_recoverable_friction} | {gate_outcome_unrecovered_blocks} | {gate_outcome_suspected_false_positives} | {gate_outcome_policy_correct_but_ux_costly} | {gate_outcome_harmless_passes} | {gate_outcome_records} | {gate_outcome_failure_owners} |".format(
                **{key: md_cell(value) for key, value in row.items()}
            )
        )
else:
    lines.append("| none | 0 | 0 | 0 | 0 | 0 | 0 | 0 | none | none |")

lines.extend([
    "",
    "### Proof Support Matrix",
    "",
    "| task | proof_status | support_status | supports_verified | residual_risk | proof_kinds | support_summary |",
    "|------|--------------|----------------|-------------------|---------------|-------------|-----------------|",
])

if rows:
    for row in rows:
        lines.append(
            "| {task} | {verification_proof_status} | {verification_proof_support_status} | {verification_proof_supports_verified} | {verification_proof_residual_risk} | {verification_proof_kinds} | {verification_proof_support_summary} |".format(
                **{key: md_cell(value) for key, value in row.items()}
            )
        )
else:
    lines.append("| none | missing | missing | false | false | none | none |")

lines.extend([
    "",
    "### Context Zone Matrix",
    "",
    "| task | materialized | envelopes | sources | dedupe_removed | provenance | task_state_empty | current_request_empty |",
    "|------|--------------|-----------|---------|----------------|------------|------------------|-----------------------|",
])

if rows:
    for row in rows:
        lines.append(
            "| {task} | {context_zones_materialized} | {context_zone_envelope_messages} | {context_zone_source_messages} | {context_zone_duplicate_blocks_removed} | {context_zone_provenance_markers} | {context_zone_task_state_empty} | {context_zone_current_decision_request_empty} |".format(
                **{key: md_cell(value) for key, value in row.items()}
            )
        )
else:
    lines.append("| none | false | 0 | 0 | 0 | 0 | false | false |")

lines.extend([
    "",
    "### Route Recovery Matrix",
    "",
    "| task | events | kinds | failure_types | read_search | mutation_blocked | safety_monotonic | unsafe_mutation_expansion | summary |",
    "|------|--------|-------|---------------|-------------|------------------|------------------|---------------------------|---------|",
])

if rows:
    for row in rows:
        lines.append(
            "| {task} | {route_recovery_events} | {route_recovery_kinds} | {route_recovery_failure_types} | {route_recovery_read_search_expanded} | {route_recovery_mutation_blocked} | {route_recovery_safety_monotonic} | {route_recovery_unsafe_mutation_expansion} | {route_recovery} |".format(
                **{key: md_cell(value) for key, value in row.items()}
            )
        )
else:
    lines.append("| none | 0 | none | none | false | false | missing | false | events=0, read_search=false, mutation_blocked=false, safety=missing |")

lines.extend([
    "",
    "## Evaluation Scores",
    "",
    "| dimension | value | meaning |",
    "|-----------|-------|---------|",
    f"| outcome_score_avg | {avg_outcome_score:.1f} | Average deterministic outcome score across task reports. |",
    f"| process_score_avg | {avg_process_score:.1f} | Average deterministic process score across task reports. |",
    f"| efficiency_score_avg | {avg_efficiency_score:.1f} | Average deterministic efficiency score across task reports. |",
    f"| agent_score_avg | {avg_agent_score:.1f} | Weighted score: outcome 50%, process 30%, efficiency 20%. |",
    f"| invalid_actions_total | {invalid_actions_total} | Premature edits, scope drift, repeated actions, risky review gaps, and phase-misaligned actions. |",
    f"| premature_edits_total | {premature_edits_total} | Edits attempted before enough evidence or explicitly demoted as early/low-value. |",
    f"| scope_drifts_total | {scope_drifts_total} | Action decisions with very low scope fit or medium/high goal drift. |",
    f"| repeated_actions_total | {repeated_actions_total} | Repeated tool actions or repeated-action stop signals. |",
    f"| failed_actions_total | {failed_actions_total} | Failed tool/action observations from trace and event logs. |",
    "",
    "### Score Matrix",
    "",
    "| task | outcome | process | efficiency | agent | invalid | premature_edit | scope_drift | repeated | failed_actions | penalties |",
    "|------|---------|---------|------------|-------|---------|----------------|-------------|----------|----------------|-----------|",
])

if rows:
    for row in rows:
        lines.append(
            "| {task} | {outcome_score} | {process_score} | {efficiency_score} | {agent_score} | {invalid_action_count} | {premature_edit_count} | {scope_drift_count} | {repeated_action_count} | {failed_action_count} | {score_penalties} |".format(
                **{key: md_cell(value) for key, value in row.items()}
            )
        )
else:
    lines.append("| none | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | none |")

lines.extend([
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
    "| task | gauntlet_status | first_pass_signal | failure_class | coding | required | closeout | spine | contract | risk | first_write | diff | warnings |",
    "|------|-----------------|-------------------|---------------|--------|----------|----------|-------|----------|------|-------------|------|----------|",
])

if coding_rows:
    for row in coding_rows:
        lines.append(
            "| {task} | {coding_gauntlet_status} | {first_pass_signal} | {failure_class} | {coding} | {required} | {closeout} | {runtime_spine} | {workflow_contract_activation} | {risk_signal} | {first_write} | {diff} | {warnings} |".format(
                **{key: md_cell(value) for key, value in row.items()}
            )
        )
else:
    lines.append("| none | not_applicable | unknown | none | tools=0, tool_records=0, validations=0, repair=0, files=0 | missing | missing | coverage=0/7, status=none, missing=none | missing | missing | missing | no | none |")

lines.extend([
    "",
    "## Task Matrix",
    "",
    "| task | status | intent | owner | failure_class | required | plan_quality | tool_boundary | verification_status | closeout | runtime_spine | runtime_diet | contract | risk | behavior_assertions | behavior_status | triggers | first_write | diff | memory | skill | warnings |",
    "|------|--------|--------|-------|---------------|----------|--------------|---------------|---------------------|----------|---------------|--------------|----------|------|---------------------|-----------------|----------|-------------|------|--------|-------|----------|",
])

if rows:
    for row in rows:
        lines.append(
            "| {task} | {status} | {intent} | {owner} | {failure_class} | {required} | {plan} | {boundary} | {verification} | {closeout} | {runtime_spine} | {runtime_diet} | {workflow_contract_activation} | {risk_signal} | {behavior_assertions} | {behavior_assertion_status} | {triggers} | {first_write} | {diff} | {memory} | {skill} | {warnings} |".format(
                **{key: md_cell(value) for key, value in row.items()}
            )
        )
else:
    lines.append("| none | missing | missing | missing | none | missing | none | none | unknown | missing | coverage=0/7, status=none, missing=none | missing | none | missing | none | none | missing | none | no | none | none | none |")

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
    "- `runtime_spine` summarizes trace/control-loop coverage and explicit runtime-spine assertions.",
])

summary_path.write_text("\n".join(lines).rstrip() + "\n", encoding="utf-8")
print(summary_path)
PY
}

