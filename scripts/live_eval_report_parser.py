import pathlib
import re
import json


RUNTIME_SPINE_PHASES = [
    "context",
    "decision",
    "permission",
    "tool_execution",
    "state_update",
    "verification",
    "closeout",
]

RUNTIME_SPINE_PHASE_EVENT_TYPES = {
    "context": {
        "user_prompt_submitted",
        "intent_routed",
        "resource_policy_selected",
        "task_context_built",
        "memory_snapshot_injected",
        "memory_prefetch",
        "retrieval_context_built",
        "memory_synced",
        "context_compacted",
        "runtime_diet_report",
        "context_zones_materialized",
        "memory_boundary_evaluated",
        "api_request_started",
        "provider_message_sequence_normalized",
        "streaming_tool_execution_shadow",
    },
    "decision": {
        "implementation_intent_recorded",
        "workflow_judgment_completed",
        "workflow_plan_progress",
        "workflow_learning_adjusted",
        "workflow_contract_activation",
        "risk_signal_assessed",
        "adaptive_workflow_triggered",
        "action_decision_evaluated",
        "candidate_actions_evaluated",
        "action_reviewed",
        "workflow_routed",
    },
    "permission": {
        "goal_drift_detected",
        "destructive_scope_checked",
        "permission_requested",
        "permission_resolved",
    },
    "tool_execution": {
        "api_request_completed",
        "tool_started",
        "tool_completed",
        "hook_completed",
        "subagent_started",
        "subagent_completed",
        "mcp_resource_accessed",
        "remote_bridge_action",
    },
    "state_update": {
        "session_goal_updated",
        "stop_check_evaluated",
        "workflow_fallback",
        "recovery_applied",
        "recovery_plan",
        "tool_observation_recorded",
        "agent_loop_step_evaluated",
    },
    "verification": {
        "stage_validation_completed",
        "reflection_pass_completed",
        "verification_completed",
        "acceptance_review_completed",
        "guided_debugging_completed",
    },
    "closeout": {
        "workflow_completed",
        "assistant_responded",
        "completion_contract_evaluated",
        "final_closeout_prepared",
        "error",
    },
}

RUNTIME_SPINE_ASSERTION_ALIASES = {
    "context": "phase:context",
    "context_phase": "phase:context",
    "decision": "phase:decision",
    "decision_phase": "phase:decision",
    "permission": "phase:permission",
    "permission_phase": "phase:permission",
    "tool_execution": "phase:tool_execution",
    "tool_execution_phase": "phase:tool_execution",
    "state_update": "phase:state_update",
    "state_update_phase": "phase:state_update",
    "verification": "phase:verification",
    "verification_phase": "phase:verification",
    "closeout": "phase:closeout",
    "closeout_phase": "phase:closeout",
    "runtime_diet": "event:runtime_diet_report",
    "runtime_diet_report": "event:runtime_diet_report",
    "runtime_diet_warnings": "special:runtime_diet_warnings",
    "runtime_diet_warning": "special:runtime_diet_warnings",
    "provider_protocol_attribution": "event:provider_message_sequence_normalized",
    "provider_message_sequence_normalized": "event:provider_message_sequence_normalized",
    "provider_protocol_repair": "special:provider_protocol_repair",
    "provider_protocol_repairs": "special:provider_protocol_repair",
    "streaming_tool_shadow": "event:streaming_tool_execution_shadow",
    "streaming_tool_execution_shadow": "event:streaming_tool_execution_shadow",
    "context_zones": "event:context_zones_materialized",
    "context_zones_materialized": "event:context_zones_materialized",
    "context_task_state_non_empty": "special:context_task_state_non_empty",
    "current_decision_request_non_empty": "special:current_decision_request_non_empty",
    "memory_boundary": "event:memory_boundary_evaluated",
    "memory_boundary_evaluated": "event:memory_boundary_evaluated",
    "task_context": "event:task_context_built",
    "task_context_built": "event:task_context_built",
    "task_contract": "event:task_contract_materialized",
    "task_contract_materialized": "event:task_contract_materialized",
    "context_pack": "event:context_pack_materialized",
    "context_pack_materialized": "event:context_pack_materialized",
    "implementation_intent": "event:implementation_intent_recorded",
    "implementation_intent_recorded": "event:implementation_intent_recorded",
    "action_decision": "event:action_decision_evaluated",
    "action_decision_evaluated": "event:action_decision_evaluated",
    "action_review": "event:action_reviewed",
    "action_reviewed": "event:action_reviewed",
    "tool_observation": "special:tool_observation",
    "tool_observation_recorded": "event:tool_observation_recorded",
    "observer_key_findings": "special:observer_key_findings",
    "tool_observation_key_findings": "special:observer_key_findings",
    "observer_evidence": "special:observer_evidence",
    "tool_observation_evidence": "special:observer_evidence",
    "observer_raw_result_ref": "special:observer_raw_result_ref",
    "tool_observation_raw_result_ref": "special:observer_raw_result_ref",
    "observer_model_visibility": "special:observer_model_visibility",
    "tool_observation_model_visibility": "special:observer_model_visibility",
    "observer_context_inclusion": "special:observer_context_inclusion",
    "tool_observation_context_inclusion": "special:observer_context_inclusion",
    "observer_state_storage": "special:observer_state_storage",
    "tool_observation_state_storage": "special:observer_state_storage",
    "checkpoint_metadata": "special:checkpoint_metadata",
    "checkpoint_present": "special:checkpoint_metadata",
    "action_review_revise": "special:action_review_revise",
    "action_review_deny": "special:action_review_deny",
    "risky_tool_action_review": "special:risky_tool_action_review",
    "risky_tools_action_review": "special:risky_tool_action_review",
    "risky_tool_reviews": "special:risky_tool_action_review",
    "review_risky_tools": "special:risky_tool_action_review",
    "action_review_for_risky_tools": "special:risky_tool_action_review",
    "action_score_recorded": "special:action_score_recorded",
    "scope_fit_recorded": "special:scope_fit_recorded",
    "early_edit_demoted": "special:early_edit_demoted",
    "observer_modified_next_action": "special:observer_modified_next_action",
    "observer_action_modifier": "special:observer_modified_next_action",
    "memory_modified_action_score": "special:memory_modified_action_score",
    "memory_action_modifier": "special:memory_modified_action_score",
    "low_score_replan_triggered": "special:low_score_replan_triggered",
    "candidate_ranking_used": "special:candidate_ranking_used",
    "candidate_score_calibrated": "special:candidate_score_calibrated",
    "candidate_score_disagreement": "special:candidate_score_disagreement",
    "observer_outcome": "special:observer_outcome_recorded",
    "observer_outcome_recorded": "special:observer_outcome_recorded",
    "observer_quality_warnings": "special:observer_quality_warnings",
    "observer_quality_warning": "special:observer_quality_warnings",
    "permission_source": "special:permission_source",
    "permission_provenance": "special:permission_source",
    "agent_loop_step": "event:agent_loop_step_evaluated",
    "agent_loop_step_evaluated": "event:agent_loop_step_evaluated",
    "state_transition_recorded": "special:state_transition_recorded",
    "completion_contract": "event:completion_contract_evaluated",
    "completion_contract_evaluated": "event:completion_contract_evaluated",
    "stop_check": "event:stop_check_evaluated",
    "stop_check_evaluated": "event:stop_check_evaluated",
    "stop_terminal_status": "special:stop_terminal_status",
    "terminal_status": "special:stop_terminal_status",
    "stop_action": "special:stop_action",
    "stop_recovery_plan": "special:stop_recovery_plan",
    "stop_failure_type": "special:failure_type",
    "failure_type": "special:failure_type",
    "recovery_plan_typed": "special:recovery_plan_typed",
    "recovery_failure_type": "special:recovery_plan_typed",
    "typed_recovery": "special:recovery_plan_typed",
    "rollback_recommended": "special:rollback_recommended",
    "rollback_completed": "special:rollback_completed",
    "needs_user": "special:needs_user",
    "stopped_by_user": "special:stopped_by_user",
    "uncertainty_not_reduced": "special:uncertainty_not_reduced",
    "model_output_invalid": "special:model_output_invalid",
    "closeout_prepared": "event:final_closeout_prepared",
    "final_closeout_prepared": "event:final_closeout_prepared",
    "execution_report": "event:execution_report_prepared",
    "execution_report_prepared": "event:execution_report_prepared",
    "memory_proposal": "event:memory_proposal_prepared",
    "memory_proposal_prepared": "event:memory_proposal_prepared",
    "verification_proof": "special:verification_proof",
    "verification_proof_present": "special:verification_proof",
    "verification_proof_verified": "special:verification_proof_verified",
    "subagent_claim_only": "verification_proof_kind:subagent_claim_only",
    "parent_verified_subagent_result": "verification_proof_kind:parent_verified_subagent_result",
    "verification_proof_support_partial": "verification_proof_support_status:partial",
    "verification_proof_support_verified": "verification_proof_support_status:verified",
}

LOW_RISK_TOOL_NAMES = {
    "ask_user",
    "bash_output",
    "bash_tasks",
    "brief",
    "calculate",
    "clear",
    "context",
    "context_vis",
    "cost",
    "datetime",
    "diff",
    "encode",
    "file_read",
    "git_diff",
    "git_status",
    "glob",
    "grep",
    "list_mcp_resources",
    "memory_load",
    "read_mcp_resource",
    "resume",
    "run_tests",
    "sleep",
    "symbol_query",
    "task_get",
    "task_list",
    "tool_search",
}

RISKY_TOOL_NAMES = {
    "agent",
    "bash",
    "bash_cancel",
    "browser",
    "config",
    "copy",
    "desktop",
    "file_edit",
    "file_patch",
    "file_write",
    "format",
    "git",
    "github",
    "install_dependencies",
    "lsp",
    "mcp_auth",
    "mcp_tool",
    "memory_clear",
    "memory_save",
    "notebook",
    "plugin_manage",
    "plugin_runtime",
    "powershell",
    "refactor",
    "remote_dev",
    "remote_trigger",
    "rewind",
    "send_message",
    "share",
    "start_dev_server",
    "task_create",
    "task_stop",
    "task_update",
    "team",
    "todo_write",
    "voice",
    "web_fetch",
    "web_search",
    "workbench",
    "worktree",
}

GATE_OUTCOME_CLASSES = [
    "protective_block",
    "recoverable_friction",
    "unrecovered_block",
    "suspected_false_positive",
    "policy_correct_but_ux_costly",
    "harmless_pass",
]


def read(path):
    path = pathlib.Path(path)
    return path.read_text(encoding="utf-8") if path.exists() else ""


def status_value(text, key, default="missing"):
    match = re.search(rf"^{re.escape(key)}=(.+)$", text, re.MULTILINE)
    return match.group(1).strip() if match else default


def report_value(text, key, default="missing"):
    match = re.search(rf"^{re.escape(key)}:\s*(.+)$", text, re.MULTILINE)
    return match.group(1).strip() if match else default


def report_values(text, key):
    return [
        match.group(1).strip()
        for match in re.finditer(rf"^{re.escape(key)}:\s*(.+)$", text, re.MULTILINE)
    ]


def status_values(text, key):
    return [
        match.group(1).strip()
        for match in re.finditer(rf"^{re.escape(key)}=(.+)$", text, re.MULTILINE)
    ]


def has_warning(text, warning):
    return bool(re.search(rf"^warning={re.escape(warning)}$", text, re.MULTILINE))


def unique_items(items):
    seen = set()
    result = []
    for item in items:
        if item and item not in seen:
            seen.add(item)
            result.append(item)
    return result


def jsonl_events(path):
    events = []
    path = pathlib.Path(path)
    if not path.exists():
        return events
    for line in path.read_text(encoding="utf-8").splitlines():
        try:
            events.append(json.loads(line))
        except Exception:
            continue
    return events


def latest_trace(events):
    return next(
        (event for event in reversed(events) if event.get("event") == "trace_summary"),
        {},
    )


def trace_events(events):
    return (latest_trace(events).get("trace") or {}).get("events") or []


def trace_event_types(events):
    return latest_trace(events).get("event_types") or []


def bool_text(value):
    return "true" if value else "false"


def parse_boolish(value):
    return str(value).strip().lower() in {"1", "true", "yes", "on"}


def token(value):
    return re.sub(r"[^a-z0-9]+", "_", str(value).strip().lower()).strip("_")


def meaningful_token(value):
    normalized = token(value)
    return "" if normalized in {"", "missing", "none", "null"} else normalized


def int_text(value, default=0):
    try:
        return str(int(value))
    except Exception:
        return str(default)


def int_value(value, default=0):
    try:
        return int(value)
    except Exception:
        return default


def _memory_proposal_events(trace_items):
    return [
        event
        for event in trace_items
        if token(event.get("type", "")) == "memory_proposal_prepared"
    ]


def _join_memory_proposal_kinds(kinds):
    if isinstance(kinds, list):
        values = [str(kind).strip() for kind in kinds if str(kind).strip()]
    else:
        values = [part.strip() for part in str(kinds or "").split(",") if part.strip()]
    return ",".join(unique_items(values)) if values else "none"


def memory_proposal_metrics_from_trace(trace_items, signal_text=""):
    signal_lower = str(signal_text or "").lower()
    proposals = _memory_proposal_events(trace_items)
    latest = proposals[-1] if proposals else {}
    candidate_kinds = _join_memory_proposal_kinds(latest.get("candidate_kinds"))
    candidates = int_value(latest.get("candidates"), 0)
    evidence_items = int_value(latest.get("evidence_items"), 0)
    write_performed_value = latest.get("write_performed", False)
    write_performed = (
        write_performed_value is True
        or parse_boolish(write_performed_value)
    )
    old_typed_signal = (
        "memory-id:" in signal_lower
        or "records.jsonl" in signal_lower
        or "typed memory record" in signal_lower
        or "memory_candidate_typed=true" in signal_lower
    )
    old_evidence_signal = (
        "evidence_status" in signal_lower
        or "memory_candidate_has_evidence=true" in signal_lower
        or "missing evidence" in signal_lower
        or "runtimeobservation" in signal_lower
        or "memoryevidencekind" in signal_lower
    )
    typed = old_typed_signal or (candidates > 0 and candidate_kinds != "none")
    has_evidence = old_evidence_signal or evidence_items > 0
    return {
        "memory_candidate_typed": bool_text(typed),
        "memory_candidate_has_evidence": bool_text(has_evidence),
        "memory_proposal_recorded": bool_text(bool(proposals)),
        "memory_proposal_status": str(latest.get("status", "missing")),
        "memory_proposal_candidates": str(candidates),
        "memory_proposal_kinds": candidate_kinds,
        "memory_proposal_evidence_items": str(evidence_items),
        "memory_proposal_write_policy": str(latest.get("write_policy", "missing")),
        "memory_proposal_write_performed": bool_text(write_performed),
    }


def float_value(value, default=0.0):
    try:
        return float(value)
    except Exception:
        return default


def clamp_int(value, low=0, high=100):
    return max(low, min(high, int(round(value))))


def split_list(value):
    if value is None:
        return []
    if isinstance(value, list):
        raw = value
    elif isinstance(value, set):
        raw = sorted(value)
    else:
        raw = str(value).split(",")
    return [
        str(item).strip()
        for item in raw
        if str(item).strip() and str(item).strip().lower() != "none"
    ]


def score_text(value):
    return str(clamp_int(value))


def short_identifier(value, max_chars=12):
    text = str(value or "").strip()
    if not text:
        return "missing"
    return text if len(text) <= max_chars else text[:max_chars]


def event_tool_name(event):
    return str(event.get("tool") or event.get("name") or "").strip()


def event_call_id(event):
    return str(event.get("call_id") or event.get("id") or "").strip()


WRITE_TOOL_NAMES = {
    "file_edit",
    "file_patch",
    "file_write",
    "format",
    "notebook",
    "refactor",
    "rewind",
}

EVIDENCE_TOOL_NAMES = {
    "bash",
    "file_read",
    "git_diff",
    "git_status",
    "glob",
    "grep",
    "list_mcp_resources",
    "read_mcp_resource",
    "run_tests",
    "symbol_query",
}


def event_sequence_key(event):
    name = token(event_tool_name(event))
    args = event.get("arguments") or event.get("args") or event.get("metadata") or ""
    try:
        args_text = json.dumps(args, sort_keys=True)
    except Exception:
        args_text = str(args)
    if len(args_text) > 160:
        args_text = args_text[:160]
    return f"{name}:{args_text}"


def is_risky_tool_name(name):
    normalized = token(name)
    if not normalized or normalized in LOW_RISK_TOOL_NAMES:
        return False
    if normalized in RISKY_TOOL_NAMES:
        return True
    return normalized.startswith(("remote_", "plugin_", "memory_save", "memory_clear"))


def risky_tool_action_review_gaps(events):
    trace_items = trace_events(events)
    action_review_events = [
        event for event in trace_items if token(event.get("type", "")) == "action_reviewed"
    ]
    reviewed_call_ids = {
        event_call_id(event) for event in action_review_events if event_call_id(event)
    }
    reviewed_pairs = {
        (token(event_tool_name(event)), event_call_id(event))
        for event in action_review_events
        if event_call_id(event)
    }

    candidates = []
    seen = set()

    def add_candidate(event, source, kind, index):
        tool = event_tool_name(event)
        if not is_risky_tool_name(tool):
            return
        call_id = event_call_id(event)
        normalized_tool = token(tool) or "unknown"
        key = call_id or f"{source}:{kind}:{index}:{normalized_tool}"
        if key in seen:
            return
        seen.add(key)
        identity = f"{normalized_tool}:{short_identifier(call_id or index)}"
        candidates.append(
            {
                "tool": normalized_tool,
                "call_id": call_id,
                "identity": identity,
            }
        )

    for index, event in enumerate(trace_items, start=1):
        event_type = token(event.get("type", ""))
        if event_type in {"tool_started", "tool_completed"}:
            add_candidate(event, "trace", event_type, index)

    for index, event in enumerate(events, start=1):
        event_type = token(event.get("event", ""))
        if event_type in {"tool_execution_start", "tool_execution_complete"}:
            add_candidate(event, "agent", event_type, index)

    missing = [
        candidate
        for candidate in candidates
        if not (
            candidate["call_id"] in reviewed_call_ids
            or (candidate["tool"], candidate["call_id"]) in reviewed_pairs
        )
    ]
    return {
        "runs": candidates,
        "missing": missing,
        "reviewed": len(candidates) - len(missing),
    }


def latest_trace_value(trace_items, event_type, field):
    event_type = token(event_type)
    for event in reversed(trace_items):
        if token(event.get("type", "")) != event_type:
            continue
        value = event.get(field)
        if value is not None:
            return str(value)
    return None


def gate_outcome_from_action_review(event, final_status):
    decision = token(event.get("decision", ""))
    reason = token(event.get("reason", ""))
    checkpoint = token(event.get("checkpoint", ""))
    recovery = str(event.get("recovery", "")).lower()
    permission = token(event.get("permission", ""))
    scope_allowed = event.get("scope_allowed", True) is not False
    budget_allowed = event.get("budget_allowed", True) is not False
    final_status = token(final_status or "")

    if decision in {"allow", "allowed"}:
        return "harmless_pass"

    if (
        not scope_allowed
        or not budget_allowed
        or "required" in checkpoint
        or permission == "ask"
        or "destructive" in reason
        or "scope" in reason
        or "checkpoint" in reason
    ):
        return "recoverable_friction" if final_status in {"completed", "passed"} else "protective_block"

    if "alternative" in recovery or final_status in {"completed", "passed"}:
        return "recoverable_friction"
    return "unrecovered_block"


def gate_failure_owner(outcome):
    if outcome in {"unrecovered_block", "suspected_false_positive", "policy_correct_but_ux_costly"}:
        return "action_review"
    return "none"


def gate_outcome_records_from_events(events):
    trace_items = trace_events(events)
    final_status = latest_trace_value(trace_items, "completion_contract_evaluated", "status")
    recovered = token(final_status or "") in {"completed", "passed"}
    route = (
        latest_trace_value(trace_items, "agent_loop_step_evaluated", "route_workflow")
        or latest_trace_value(trace_items, "intent_routed", "workflow")
        or "missing"
    )
    risk = (
        latest_trace_value(trace_items, "agent_loop_step_evaluated", "route_risk")
        or latest_trace_value(trace_items, "intent_routed", "risk")
        or "missing"
    )
    records = []

    for event in trace_items:
        event_type = token(event.get("type", ""))
        if event_type == "action_reviewed":
            outcome = gate_outcome_from_action_review(event, final_status)
            records.append({
                "gate": "action_review",
                "decision": token(event.get("decision", "")) or "missing",
                "outcome": outcome,
                "reason": token(event.get("reason", "")) or "missing",
                "tool": token(event_tool_name(event)) or "missing",
                "route": token(route) or "missing",
                "risk": token(risk) or "missing",
                "recovered_after_gate": bool_text(recovered),
                "final_status": token(final_status or "") or "missing",
                "failure_owner": gate_failure_owner(outcome),
            })
        elif event_type == "permission_resolved":
            approved = event.get("approved") is True or parse_boolish(event.get("approved"))
            if approved:
                outcome = "harmless_pass"
            elif recovered:
                outcome = "recoverable_friction"
            else:
                outcome = "unrecovered_block"
            records.append({
                "gate": "permission",
                "decision": token(event.get("decision", "")) or ("approved" if approved else "denied"),
                "outcome": outcome,
                "reason": "permission_approved" if approved else "permission_denied",
                "tool": token(event_tool_name(event)) or "missing",
                "route": token(route) or "missing",
                "risk": token(risk) or "missing",
                "recovered_after_gate": bool_text(recovered),
                "final_status": token(final_status or "") or "missing",
                "failure_owner": gate_failure_owner(outcome),
            })
        elif event_type == "final_closeout_prepared":
            status = token(event.get("status", ""))
            proof_status = event.get("verification_proof_status")
            proof_token = token(proof_status or "")
            failure_type = meaningful_token(event.get("failure_type", ""))
            if status in {"passed", "completed"} and (
                proof_status is None or proof_token in {"verified", "not_applicable"}
            ):
                outcome = "harmless_pass"
            elif failure_type:
                outcome = "protective_block"
            else:
                outcome = "unrecovered_block"
            records.append({
                "gate": "closeout",
                "decision": status or "missing",
                "outcome": outcome,
                "reason": proof_token or "no_verification_proof_status",
                "tool": "none",
                "route": token(route) or "missing",
                "risk": token(risk) or "missing",
                "recovered_after_gate": bool_text(recovered),
                "final_status": token(final_status or "") or "missing",
                "failure_owner": gate_failure_owner(outcome),
            })
    return records


def gate_outcome_metrics_from_records(records):
    counts = {klass: 0 for klass in GATE_OUTCOME_CLASSES}
    for record in records:
        outcome = record.get("outcome", "missing")
        if outcome in counts:
            counts[outcome] += 1
    record_text = ",".join(
        f"{record['gate']}:{record['decision']}:{record['outcome']}"
        for record in records[:12]
    )
    if not record_text:
        record_text = "none"
    if len(records) > 12:
        record_text += f",+{len(records) - 12}"
    summary = (
        f"total={len(records)}, protective_block={counts['protective_block']}, "
        f"recoverable_friction={counts['recoverable_friction']}, "
        f"unrecovered_block={counts['unrecovered_block']}, "
        f"suspected_false_positive={counts['suspected_false_positive']}, "
        f"policy_correct_but_ux_costly={counts['policy_correct_but_ux_costly']}, "
        f"harmless_pass={counts['harmless_pass']}"
    )
    failure_owners = unique_items(
        record.get("failure_owner", "")
        for record in records
        if record.get("failure_owner") not in {"", "none", None}
    )
    return {
        "gate_outcomes": summary,
        "gate_outcome_records": record_text,
        "gate_outcome_total": str(len(records)),
        "gate_outcome_protective_blocks": str(counts["protective_block"]),
        "gate_outcome_recoverable_friction": str(counts["recoverable_friction"]),
        "gate_outcome_unrecovered_blocks": str(counts["unrecovered_block"]),
        "gate_outcome_suspected_false_positives": str(counts["suspected_false_positive"]),
        "gate_outcome_policy_correct_but_ux_costly": str(counts["policy_correct_but_ux_costly"]),
        "gate_outcome_harmless_passes": str(counts["harmless_pass"]),
        "gate_outcome_failure_owners": ",".join(failure_owners) if failure_owners else "none",
    }


def report_run_status(tool_boundary, quality_status, test_status, plan_quality):
    if quality_status == "failed" or test_status == "failed" or plan_quality == "failed":
        return "failed"
    if tool_boundary == "plan-only":
        return "passed" if plan_quality in {"ok", "api_response"} else "skipped"
    if tool_boundary == "collect-only":
        return "passed" if test_status == "ok" else "skipped"
    if tool_boundary == "agent-run":
        if quality_status == "ok" and test_status in {"ok", "skipped"}:
            return "passed"
        return "skipped"
    return "skipped"


def normalize_runtime_spine_assertion(value):
    raw = str(value).strip()
    if not raw:
        return None
    if ":" in raw:
        prefix, name = raw.split(":", 1)
        prefix = token(prefix)
        name = token(name)
        if prefix == "phase" and name in RUNTIME_SPINE_PHASES:
            return f"phase:{name}"
        if prefix in {"event", "trace", "trace_event"}:
            return f"event:{RUNTIME_SPINE_ASSERTION_ALIASES.get(name, 'event:' + name).split(':', 1)[1]}"
        if prefix == "special" and name in {
            "verification_proof",
            "verification_proof_verified",
            "action_review_revise",
            "action_review_deny",
            "risky_tool_action_review",
            "checkpoint_metadata",
            "tool_observation",
            "observer_key_findings",
            "observer_evidence",
            "observer_raw_result_ref",
            "observer_model_visibility",
            "observer_context_inclusion",
            "observer_state_storage",
            "stop_terminal_status",
            "stop_action",
            "stop_recovery_plan",
            "failure_type",
            "recovery_plan_typed",
            "rollback_recommended",
            "rollback_completed",
            "needs_user",
            "stopped_by_user",
            "uncertainty_not_reduced",
            "model_output_invalid",
            "candidate_score_calibrated",
            "candidate_score_disagreement",
            "observer_outcome_recorded",
            "observer_quality_warnings",
            "permission_source",
            "runtime_diet_warnings",
            "provider_protocol_repair",
            "context_task_state_non_empty",
            "current_decision_request_non_empty",
        }:
            return f"special:{name}"
        if prefix in {
            "completion_status",
            "terminal_status",
            "verification_proof_status",
            "verification_proof_kind",
            "verification_proof_kinds",
            "verification_proof_support_status",
            "verification_proof_supports_verified",
        }:
            return f"{prefix}:{name}"
    normalized = token(raw.removeprefix("runtime_spine_"))
    return RUNTIME_SPINE_ASSERTION_ALIASES.get(normalized, f"unknown:{normalized}")


def normalized_runtime_spine_assertions(sample):
    raw = sample.get("runtime_spine_assertions")
    if raw is None:
        raw = (sample.get("quality_assertions") or {}).get("runtime_spine")
    if raw is None:
        return []

    values = []
    if isinstance(raw, dict):
        for phase in raw.get("required_phases") or []:
            values.append(f"phase:{phase}")
        for event in raw.get("required_events") or []:
            values.append(f"event:{event}")
        for item in raw.get("required") or raw.get("assertions") or []:
            values.append(item)
        for field in (
            "completion_status",
            "terminal_status",
            "verification_proof_status",
            "verification_proof_kind",
            "verification_proof_kinds",
            "verification_proof_support_status",
            "verification_proof_supports_verified",
        ):
            expected = raw.get(field) or raw.get(f"expected_{field}")
            if expected:
                values.append(f"{field}:{expected}")
        if raw.get("verification_proof") is True or raw.get("require_verification_proof") is True:
            values.append("verification_proof")
        if raw.get("verification_proof_verified") is True:
            values.append("verification_proof_verified")
        if raw.get("action_decision") is True:
            values.append("action_decision")
        if raw.get("stop_check") is True:
            values.append("stop_check")
        if raw.get("context_task_state_non_empty") is True:
            values.append("context_task_state_non_empty")
        if raw.get("current_decision_request_non_empty") is True:
            values.append("current_decision_request_non_empty")
        if (
            raw.get("risky_tool_action_review") is True
            or raw.get("require_risky_tool_action_review") is True
            or raw.get("require_action_review_for_risky_tools") is True
            or raw.get("action_review_for_risky_tools") is True
        ):
            values.append("risky_tool_action_review")
    elif isinstance(raw, str):
        values = [item.strip() for item in raw.split(",")]
    elif isinstance(raw, list):
        values = raw
    else:
        values = [raw]

    result = []
    for value in values:
        assertion = normalize_runtime_spine_assertion(value)
        if assertion and assertion not in result:
            result.append(assertion)
    return result


def runtime_spine_phase_for_event(event_type):
    event_type = token(event_type)
    for phase, event_types in RUNTIME_SPINE_PHASE_EVENT_TYPES.items():
        if event_type in event_types:
            return phase
    return None


def runtime_spine_metrics_from_events(events, report_text="", assertions=None):
    assertions = list(assertions or [])
    trace = latest_trace(events)
    trace_items = trace_events(events)
    phase_counts = {phase: 0 for phase in RUNTIME_SPINE_PHASES}
    phase_latest = {phase: "none" for phase in RUNTIME_SPINE_PHASES}
    event_counts = {}

    for event in trace_items:
        event_type = token(event.get("type", ""))
        if not event_type:
            continue
        event_counts[event_type] = event_counts.get(event_type, 0) + 1
        phase = runtime_spine_phase_for_event(event_type)
        if not phase:
            continue
        phase_counts[phase] += 1
        phase_latest[phase] = event_type

    action_review_events = [
        event for event in trace_items if token(event.get("type", "")) == "action_reviewed"
    ]
    action_decision_events = [
        event for event in trace_items if token(event.get("type", "")) == "action_decision_evaluated"
    ]
    tool_observation_events = [
        event
        for event in trace_items
        if token(event.get("type", "")) == "tool_observation_recorded"
    ]
    permission_resolved_events = [
        event for event in trace_items if token(event.get("type", "")) == "permission_resolved"
    ]
    runtime_diet_events = [
        event for event in trace_items if token(event.get("type", "")) == "runtime_diet_report"
    ]
    provider_protocol_events = [
        event
        for event in trace_items
        if token(event.get("type", "")) == "provider_message_sequence_normalized"
    ]
    streaming_shadow_events = [
        event
        for event in trace_items
        if token(event.get("type", "")) == "streaming_tool_execution_shadow"
    ]
    stop_check_events = [
        event for event in trace_items if token(event.get("type", "")) == "stop_check_evaluated"
    ]
    agent_loop_events = [
        event for event in trace_items if token(event.get("type", "")) == "agent_loop_step_evaluated"
    ]
    context_zone_events = [
        event for event in trace_items if token(event.get("type", "")) == "context_zones_materialized"
    ]
    memory_boundary_events = [
        event for event in trace_items if token(event.get("type", "")) == "memory_boundary_evaluated"
    ]
    task_contract_events = [
        event for event in trace_items if token(event.get("type", "")) == "task_contract_materialized"
    ]
    context_pack_events = [
        event for event in trace_items if token(event.get("type", "")) == "context_pack_materialized"
    ]
    execution_report_events = [
        event for event in trace_items if token(event.get("type", "")) == "execution_report_prepared"
    ]
    memory_proposal_events = [
        event for event in trace_items if token(event.get("type", "")) == "memory_proposal_prepared"
    ]
    completion_contract_events = [
        event
        for event in trace_items
        if token(event.get("type", "")) == "completion_contract_evaluated"
    ]
    recovery_plan_events = [
        event for event in trace_items if token(event.get("type", "")) == "recovery_plan"
    ]
    risky_review_gaps = risky_tool_action_review_gaps(events)
    risky_tool_runs = len(risky_review_gaps["runs"])
    risky_tool_reviewed = risky_review_gaps["reviewed"]
    risky_tool_missing_reviews = [
        candidate["identity"] for candidate in risky_review_gaps["missing"]
    ]
    gate_outcome_metrics = gate_outcome_metrics_from_records(
        gate_outcome_records_from_events(events)
    )

    latest_closeout = next(
        (
            event
            for event in reversed(trace_items)
            if token(event.get("type", "")) == "final_closeout_prepared"
        ),
        {},
    )
    proof_status = report_value(report_text, "verification_proof_status", "")
    proof_summary = report_value(report_text, "verification_proof_summary", "")
    proof_kind_summary = report_value(report_text, "verification_proof_kinds", "")
    proof_support_status = report_value(report_text, "verification_proof_support_status", "")
    proof_support_summary = report_value(report_text, "verification_proof_support_summary", "")
    proof_supports_verified = report_value(report_text, "verification_proof_supports_verified", "")
    proof_residual_risk = report_value(report_text, "verification_proof_residual_risk", "")
    if not proof_status:
        proof_status = str(latest_closeout.get("verification_proof_status", "missing"))
    if not proof_summary:
        proof_summary = str(latest_closeout.get("verification_proof_summary", "missing"))
    if not proof_kind_summary:
        proof_kind_summary = str(latest_closeout.get("verification_proof_kind_summary", "none"))
    if not proof_support_status:
        proof_support_status = str(latest_closeout.get("verification_proof_support_status", "missing"))
    if not proof_support_summary:
        proof_support_summary = str(latest_closeout.get("verification_proof_support_summary", "missing"))
    if not proof_supports_verified:
        proof_supports_verified = bool_text(
            latest_closeout.get("verification_proof_supports_verified") is True
        )
    if not proof_residual_risk:
        proof_residual_risk = bool_text(
            latest_closeout.get("verification_proof_residual_risk") is True
        )

    latest_stop = stop_check_events[-1] if stop_check_events else {}
    stop_terminal_status = str(
        latest_stop.get("terminal_status")
        or latest_closeout.get("terminal_status")
        or "missing"
    )
    stop_action = str(
        latest_stop.get("action") or latest_closeout.get("stop_action") or "missing"
    )
    stop_reason = str(
        latest_stop.get("reason") or latest_closeout.get("stop_reason") or "missing"
    )
    stop_failure_type = str(
        latest_stop.get("failure_type")
        or latest_closeout.get("failure_type")
        or "missing"
    )
    stop_recovery_plan_id = str(
        latest_stop.get("recovery_plan_id")
        or latest_closeout.get("recovery_plan_id")
        or "missing"
    )
    rollback_recommended = any(
        bool(event.get("rollback_recommended"))
        or token(event.get("reason", "")) == "rollback_recommended"
        for event in stop_check_events
    ) or token(latest_closeout.get("rollback_status", "")) not in {"", "none", "missing", "null"}
    rollback_completed = (
        token(latest_closeout.get("terminal_status", "")) == "rolled_back"
        or any(
            token(event.get("tool", "")) in {"rewind", "rollback"}
            and token(event.get("status", "")) == "success"
            for event in tool_observation_events
        )
    )
    typed_recovery_events = [
        event
        for event in recovery_plan_events
        if token(event.get("failure_type", "")) or token(event.get("recovery_kind", ""))
    ]
    recovery_failure_types = unique_items(
        meaningful_token(event.get("failure_type", ""))
        for event in recovery_plan_events
        if meaningful_token(event.get("failure_type", ""))
    )
    recovery_kinds = unique_items(
        meaningful_token(event.get("recovery_kind", ""))
        for event in recovery_plan_events
        if meaningful_token(event.get("recovery_kind", ""))
    )
    trace_failure_types = unique_items(
        meaningful_token(value)
        for value in [
            *(event.get("failure_type") for event in stop_check_events),
            *(event.get("failure_type") for event in recovery_plan_events),
            *(event.get("failure_type") for event in tool_observation_events),
            latest_closeout.get("failure_type"),
        ]
        if meaningful_token(value)
    )
    action_scores = [
        int_value(event.get("action_score"))
        for event in action_decision_events
        if event.get("action_score") is not None
    ]
    scope_fit_values = [
        int_value(event.get("scope_fit"))
        for event in action_decision_events
        if event.get("scope_fit") is not None
    ]
    latest_action_score = action_scores[-1] if action_scores else None
    low_action_score_count = sum(1 for score in action_scores if score <= 3)
    phase_misaligned_actions = sum(
        1 for event in action_decision_events if event.get("phase_aligned") is False
    )
    memory_modifier_applied = any(
        any(token(modifier.get("source", "")) == "memory" for modifier in event.get("modifiers") or [])
        or "memory modifier" in str(event.get("reason", "")).lower()
        for event in action_decision_events
    )
    observer_modifier_applied = any(
        any(token(modifier.get("source", "")) == "observer" for modifier in event.get("modifiers") or [])
        or "observer modifier" in str(event.get("reason", "")).lower()
        for event in action_decision_events
    )
    scope_fit_revision = any(
        token(event.get("reason", "")) == "low_scope_fit" for event in action_review_events
    )
    early_edit_demoted = any(
        token(event.get("reason", "")) == "low_value_action" for event in action_review_events
    ) or any(
        event.get("phase_aligned") is False
        and bool(event.get("mutates_workspace"))
        and token(event.get("stage", "")) in {"understand", "diagnosis"}
        for event in action_decision_events
    )
    candidate_action_count = sum(
        int_value(event.get("candidate_count", 0))
        for event in trace_items
        if token(event.get("type", "")) == "candidate_actions_evaluated"
    )
    candidate_events = [
        event
        for event in trace_items
        if token(event.get("type", "")) == "candidate_actions_evaluated"
    ]
    candidate_selected_by_runtime = any(
        bool(event.get("selected_id"))
        for event in candidate_events
    ) or "candidate_action_request" in report_text.lower()
    candidate_score_calibrated = any(
        event.get("selected_runtime_score") is not None
        and event.get("selected_model_score") is not None
        for event in candidate_events
    )
    candidate_score_disagreement = any(
        bool(event.get("runtime_selected_differs_from_model_order"))
        for event in candidate_events
    )
    score_driven_replan = any(
        token(event.get("reason", ""))
        in {
            "low_action_value_loop",
            "score_not_reducing_uncertainty",
            "repeated_action_revision",
        }
        and token(event.get("action", "")) == "replan"
        for event in stop_check_events
    )
    score_driven_stop = any(
        token(event.get("reason", ""))
        in {
            "low_action_value_loop",
            "score_not_reducing_uncertainty",
            "repeated_action_revision",
        }
        and token(event.get("status", "")) == "stop"
        for event in stop_check_events
    )
    formula_versions = unique_items(
        meaningful_token(event.get("formula_version", "")) for event in action_decision_events
    )
    latest_agent_loop = agent_loop_events[-1] if agent_loop_events else {}
    latest_context_zones = context_zone_events[-1] if context_zone_events else {}
    latest_memory_boundary = memory_boundary_events[-1] if memory_boundary_events else {}
    latest_completion_contract = (
        completion_contract_events[-1] if completion_contract_events else {}
    )
    permission_sources = unique_items(
        meaningful_token(event.get("source", ""))
        for event in permission_resolved_events
        if meaningful_token(event.get("source", ""))
    )
    observer_quality_warning_count = sum(
        int_value(event.get("quality_warnings", 0)) for event in tool_observation_events
    )
    observer_quality_warning_labels = unique_items(
        meaningful_token(label)
        for event in tool_observation_events
        for label in event.get("quality_warning_labels") or []
        if meaningful_token(label)
    )
    runtime_diet_warning_labels = unique_items(
        meaningful_token(label)
        for event in runtime_diet_events
        for label in event.get("warnings") or []
        if meaningful_token(label)
    )
    provider_protocol_repair_count = sum(
        int_value(event.get("system_messages_merged", 0))
        + int_value(event.get("dropped_assistant_tool_calls", 0))
        + int_value(event.get("dropped_tool_results", 0))
        for event in provider_protocol_events
    )
    streaming_shadow_eligible = sum(
        int_value(event.get("eligible_tool_calls", 0)) for event in streaming_shadow_events
    )
    state_transition_recorded = any(
        token(event.get("stage_before", "")) != token(event.get("stage_after", ""))
        for event in agent_loop_events
    )

    missing = []
    for assertion in assertions:
        kind, _, name = assertion.partition(":")
        if kind == "phase":
            if phase_counts.get(name, 0) <= 0:
                missing.append(assertion)
        elif kind == "event":
            if event_counts.get(token(name), 0) <= 0:
                missing.append(assertion)
        elif kind == "special" and name == "verification_proof":
            if token(proof_status) in {"", "missing", "none", "null"}:
                missing.append(assertion)
        elif kind == "special" and name == "verification_proof_verified":
            if token(proof_status) != "verified":
                missing.append(assertion)
        elif kind == "special" and name == "action_review_revise":
            if not any(token(event.get("decision", "")) == "revise" for event in action_review_events):
                missing.append(assertion)
        elif kind == "special" and name == "action_review_deny":
            if not any(token(event.get("decision", "")) == "deny" for event in action_review_events):
                missing.append(assertion)
        elif kind == "special" and name == "checkpoint_metadata":
            checkpoint_statuses = {
                token(event.get("checkpoint", "")) for event in action_review_events
            }
            if (
                "required_and_present" not in checkpoint_statuses
                and "tool_managed" not in checkpoint_statuses
                and "checkpoint_id" not in report_text.lower()
            ):
                missing.append(assertion)
        elif kind == "special" and name == "tool_observation":
            if (
                not tool_observation_events
                and "tool_observation" not in report_text.lower()
                and "context_ledger.tool_observation" not in report_text.lower()
            ):
                missing.append(assertion)
        elif kind == "special" and name == "observer_key_findings":
            if not any(int_value(event.get("key_findings", 0)) > 0 for event in tool_observation_events):
                missing.append(assertion)
        elif kind == "special" and name == "observer_evidence":
            if not any(int_value(event.get("evidence_items", 0)) > 0 for event in tool_observation_events):
                missing.append(assertion)
        elif kind == "special" and name == "observer_raw_result_ref":
            if not any(str(event.get("raw_result_ref", "")).strip() for event in tool_observation_events):
                missing.append(assertion)
        elif kind == "special" and name == "observer_model_visibility":
            if not any(str(event.get("model_visibility", "")).strip() for event in tool_observation_events):
                missing.append(assertion)
        elif kind == "special" and name == "observer_context_inclusion":
            if not any(bool(event.get("include_in_next_context")) for event in tool_observation_events):
                missing.append(assertion)
        elif kind == "special" and name == "observer_state_storage":
            if not any(bool(event.get("store_in_state")) for event in tool_observation_events):
                missing.append(assertion)
        elif kind == "special" and name == "stop_terminal_status":
            if token(stop_terminal_status) in {"", "missing", "none", "null"}:
                missing.append(assertion)
        elif kind == "special" and name == "stop_action":
            if token(stop_action) in {"", "missing", "none", "null"}:
                missing.append(assertion)
        elif kind == "special" and name == "stop_recovery_plan":
            if (
                token(stop_recovery_plan_id) in {"", "missing", "none", "null"}
                and not recovery_plan_events
            ):
                missing.append(assertion)
        elif kind == "special" and name == "failure_type":
            if not trace_failure_types:
                missing.append(assertion)
        elif kind == "special" and name == "recovery_plan_typed":
            if not typed_recovery_events:
                missing.append(assertion)
        elif kind == "special" and name == "rollback_recommended":
            if not rollback_recommended:
                missing.append(assertion)
        elif kind == "special" and name == "rollback_completed":
            if not rollback_completed:
                missing.append(assertion)
        elif kind == "special" and name == "needs_user":
            if not (
                token(stop_terminal_status) == "needs_user"
                or token(stop_action) == "ask_user"
                or any(bool(event.get("requires_user_decision")) for event in recovery_plan_events)
            ):
                missing.append(assertion)
        elif kind == "special" and name == "stopped_by_user":
            if token(stop_terminal_status) != "stopped_by_user":
                missing.append(assertion)
        elif kind == "special" and name == "uncertainty_not_reduced":
            if not any(token(event.get("reason", "")) == "uncertainty_not_reduced" for event in stop_check_events):
                missing.append(assertion)
        elif kind == "special" and name == "model_output_invalid":
            if not any(
                token(event.get("reason", "")) == "model_output_invalid"
                or token(event.get("failure_type", "")) == "model_output_invalid"
                for event in stop_check_events
            ):
                missing.append(assertion)
        elif kind == "special" and name == "risky_tool_action_review":
            if risky_tool_missing_reviews:
                missing.append(assertion)
        elif kind == "special" and name == "action_score_recorded":
            if not action_scores:
                missing.append(assertion)
        elif kind == "special" and name == "scope_fit_recorded":
            if not scope_fit_values:
                missing.append(assertion)
        elif kind == "special" and name == "early_edit_demoted":
            if not early_edit_demoted:
                missing.append(assertion)
        elif kind == "special" and name == "observer_modified_next_action":
            if not observer_modifier_applied:
                missing.append(assertion)
        elif kind == "special" and name == "memory_modified_action_score":
            if not memory_modifier_applied:
                missing.append(assertion)
        elif kind == "special" and name == "low_score_replan_triggered":
            if not score_driven_replan:
                missing.append(assertion)
        elif kind == "special" and name == "candidate_ranking_used":
            if not candidate_selected_by_runtime:
                missing.append(assertion)
        elif kind == "special" and name == "candidate_score_calibrated":
            if not candidate_score_calibrated:
                missing.append(assertion)
        elif kind == "special" and name == "candidate_score_disagreement":
            if not candidate_score_disagreement:
                missing.append(assertion)
        elif kind == "special" and name == "observer_outcome_recorded":
            if not tool_observation_events:
                missing.append(assertion)
        elif kind == "special" and name == "observer_quality_warnings":
            if observer_quality_warning_count <= 0 and not observer_quality_warning_labels:
                missing.append(assertion)
        elif kind == "special" and name == "permission_source":
            if not permission_sources:
                missing.append(assertion)
        elif kind == "special" and name == "runtime_diet_warnings":
            if not runtime_diet_warning_labels:
                missing.append(assertion)
        elif kind == "special" and name == "provider_protocol_repair":
            if provider_protocol_repair_count <= 0:
                missing.append(assertion)
        elif kind == "special" and name == "context_task_state_non_empty":
            if latest_context_zones.get("task_state_empty") is not False:
                missing.append(assertion)
        elif kind == "special" and name == "current_decision_request_non_empty":
            if latest_context_zones.get("current_decision_request_empty") is not False:
                missing.append(assertion)
        elif kind == "special" and name == "state_transition_recorded":
            if not state_transition_recorded:
                missing.append(assertion)
        elif kind == "completion_status":
            if token(latest_completion_contract.get("status", "")) != name:
                missing.append(assertion)
        elif kind == "terminal_status":
            terminal = token(
                latest_completion_contract.get("terminal_status", "")
                or stop_terminal_status
            )
            if terminal != name:
                missing.append(assertion)
        elif kind == "verification_proof_status":
            proof = token(
                latest_completion_contract.get("verification_proof_status", "")
                or proof_status
            )
            if proof != name:
                missing.append(assertion)
        elif kind in {"verification_proof_kind", "verification_proof_kinds"}:
            proof_kinds = {
                token(item)
                for item in re.split(r"[,;\s]+", str(proof_kind_summary))
                if token(item)
            }
            if name not in proof_kinds:
                missing.append(assertion)
        elif kind == "verification_proof_support_status":
            if token(proof_support_status) != name:
                missing.append(assertion)
        elif kind == "verification_proof_supports_verified":
            if token(proof_supports_verified) != name:
                missing.append(assertion)
        else:
            missing.append(assertion)

    observed_phases = [phase for phase in RUNTIME_SPINE_PHASES if phase_counts[phase] > 0]
    phase_coverage = f"{len(observed_phases)}/{len(RUNTIME_SPINE_PHASES)}"
    trace_present = bool(trace)
    if assertions:
        runtime_spine_status = "failed" if missing else "passed"
    else:
        runtime_spine_status = "none"
    if assertions and not trace_present and not report_text:
        runtime_spine_status = "missing"

    detail = " ".join(
        f"{phase}={phase_counts[phase]} latest={phase_latest[phase]}"
        for phase in RUNTIME_SPINE_PHASES
    )
    risky_tool_missing_text = (
        ",".join(risky_tool_missing_reviews) if risky_tool_missing_reviews else "none"
    )
    detail = (
        f"{detail} risky_tool_runs={risky_tool_runs} "
        f"risky_tool_reviewed={risky_tool_reviewed} "
        f"risky_tool_missing_action_review={risky_tool_missing_text} "
        f"gate_outcomes={gate_outcome_metrics['gate_outcomes']} "
        f"stop_reason={stop_reason} stop_terminal_status={stop_terminal_status} "
        f"stop_action={stop_action} stop_failure_type={stop_failure_type} "
        f"rollback_recommended={bool_text(rollback_recommended)} "
        f"rollback_completed={bool_text(rollback_completed)} "
        f"recovery_failure_types={','.join(recovery_failure_types) if recovery_failure_types else 'none'} "
        f"recovery_kinds={','.join(recovery_kinds) if recovery_kinds else 'none'} "
        f"action_scores={len(action_scores)} latest_action_score={latest_action_score if latest_action_score is not None else 'none'} "
        f"low_action_score_count={low_action_score_count} phase_misaligned_actions={phase_misaligned_actions} "
        f"observer_modifier_applied={bool_text(observer_modifier_applied)} "
        f"memory_modifier_applied={bool_text(memory_modifier_applied)} "
        f"observer_outcome_recorded={bool_text(bool(tool_observation_events))} "
        f"observer_quality_warnings={observer_quality_warning_count} "
        f"observer_quality_warning_labels={','.join(observer_quality_warning_labels) if observer_quality_warning_labels else 'none'} "
        f"permission_sources={','.join(permission_sources) if permission_sources else 'none'} "
        f"runtime_diet_warnings={','.join(runtime_diet_warning_labels) if runtime_diet_warning_labels else 'none'} "
        f"provider_protocol_events={len(provider_protocol_events)} "
        f"provider_protocol_repairs={provider_protocol_repair_count} "
        f"streaming_tool_shadow_events={len(streaming_shadow_events)} "
        f"streaming_tool_shadow_eligible={streaming_shadow_eligible} "
        f"memory_boundary_recorded={bool_text(bool(memory_boundary_events))} "
        f"task_contract_recorded={bool_text(bool(task_contract_events))} "
        f"context_pack_recorded={bool_text(bool(context_pack_events))} "
        f"execution_report_recorded={bool_text(bool(execution_report_events))} "
        f"memory_proposal_recorded={bool_text(bool(memory_proposal_events))} "
        f"agent_loop_steps={len(agent_loop_events)} "
        f"context_zones={len(context_zone_events)} "
        f"completion_contract={token(latest_completion_contract.get('status', 'missing'))}"
    )
    missing_text = ",".join(missing) if missing else "none"
    assertions_text = ",".join(assertions) if assertions else "none"
    summary = (
        f"coverage={phase_coverage}, status={runtime_spine_status}, missing={missing_text}"
    )

    return {
        "runtime_spine": summary,
        "runtime_spine_detail": detail,
        "runtime_spine_trace_present": bool_text(trace_present),
        "runtime_spine_phase_coverage": phase_coverage,
        "runtime_spine_observed_phases": ",".join(observed_phases) if observed_phases else "none",
        "runtime_spine_assertions": assertions_text,
        "runtime_spine_status": runtime_spine_status,
        "runtime_spine_missing": missing_text,
        "risky_tool_runs": str(risky_tool_runs),
        "risky_tool_reviewed": str(risky_tool_reviewed),
        "risky_tool_missing_action_review": risky_tool_missing_text,
        **gate_outcome_metrics,
        "stop_reason": stop_reason,
        "stop_terminal_status": stop_terminal_status,
        "stop_action": stop_action,
        "stop_failure_type": stop_failure_type,
        "stop_recovery_plan_id": stop_recovery_plan_id,
        "rollback_recommended": bool_text(rollback_recommended),
        "rollback_completed": bool_text(rollback_completed),
        "trace_failure_types": ",".join(trace_failure_types) if trace_failure_types else "none",
        "recovery_failure_types": ",".join(recovery_failure_types) if recovery_failure_types else "none",
        "recovery_kinds": ",".join(recovery_kinds) if recovery_kinds else "none",
        "recovery_requires_user": bool_text(any(bool(event.get("requires_user_decision")) for event in recovery_plan_events)),
        "recovery_side_effect_uncertain": bool_text(any(bool(event.get("side_effect_uncertain")) for event in recovery_plan_events)),
        "action_scoring_active": bool_text(bool(action_scores or scope_fit_values)),
        "selected_action_score": str(latest_action_score) if latest_action_score is not None else "missing",
        "selected_action_score_min": str(min(action_scores)) if action_scores else "missing",
        "selected_action_score_avg": "{:.2f}".format(sum(action_scores) / len(action_scores)) if action_scores else "missing",
        "low_score_actions": str(low_action_score_count),
        "low_action_score_count": str(low_action_score_count),
        "phase_misaligned_actions": str(phase_misaligned_actions),
        "action_score_formula_version": ",".join(formula_versions) if formula_versions else "missing",
        "observer_modifier_applied": bool_text(observer_modifier_applied),
        "memory_modifier_applied": bool_text(memory_modifier_applied),
        "scope_fit_revision": bool_text(scope_fit_revision),
        "early_edit_demoted": bool_text(early_edit_demoted),
        "candidate_action_count": str(candidate_action_count),
        "candidate_selected_by_runtime": bool_text(candidate_selected_by_runtime),
        "candidate_score_calibrated": bool_text(candidate_score_calibrated),
        "candidate_score_disagreement": bool_text(candidate_score_disagreement),
        "score_driven_replan": bool_text(score_driven_replan),
        "score_driven_stop": bool_text(score_driven_stop),
        "agent_loop_steps": str(len(agent_loop_events)),
        "agent_loop_latest_stage_before": str(latest_agent_loop.get("stage_before", "missing")),
        "agent_loop_latest_stage_after": str(latest_agent_loop.get("stage_after", "missing")),
        "agent_loop_latest_stop_reason": str(latest_agent_loop.get("stop_reason", "missing")),
        "state_transition_recorded": bool_text(state_transition_recorded),
        "context_zones_materialized": bool_text(bool(context_zone_events)),
        "context_zone_relevant_items": str(latest_context_zones.get("relevant_material_items", "missing")),
        "context_zone_observation_items": str(latest_context_zones.get("recent_observation_items", "missing")),
        "context_zone_task_state_empty": bool_text(bool(latest_context_zones.get("task_state_empty"))),
        "context_zone_current_decision_request_empty": bool_text(bool(latest_context_zones.get("current_decision_request_empty"))),
        "observer_outcome_recorded": bool_text(bool(tool_observation_events)),
        "observer_outcome_latest_status": str((tool_observation_events[-1] if tool_observation_events else {}).get("status", "missing")),
        "observer_outcome_latest_findings": str((tool_observation_events[-1] if tool_observation_events else {}).get("key_findings", "missing")),
        "observer_outcome_latest_evidence": str((tool_observation_events[-1] if tool_observation_events else {}).get("evidence_items", "missing")),
        "memory_boundary_recorded": bool_text(bool(memory_boundary_events)),
        "task_contract_recorded": bool_text(bool(task_contract_events)),
        "context_pack_recorded": bool_text(bool(context_pack_events)),
        "execution_report_recorded": bool_text(bool(execution_report_events)),
        "memory_proposal_recorded": bool_text(bool(memory_proposal_events)),
        "memory_boundary_read_status": str(latest_memory_boundary.get("read_status", "missing")),
        "memory_boundary_closeout_write_candidate_status": str(latest_memory_boundary.get("closeout_write_candidate_status", "missing")),
        "completion_contract_status": str(latest_completion_contract.get("status", "missing")),
        "completion_contract_terminal_status": str(latest_completion_contract.get("terminal_status", "missing")),
        "completion_contract_proof_status": str(latest_completion_contract.get("verification_proof_status", "missing")),
        "verification_proof_status": proof_status,
        "verification_proof_summary": proof_summary,
        "verification_proof_kinds": proof_kind_summary,
        "verification_proof_support_status": proof_support_status,
        "verification_proof_support_summary": proof_support_summary,
        "verification_proof_supports_verified": proof_supports_verified,
        "verification_proof_residual_risk": proof_residual_risk,
    }


def runtime_spine_metrics(task_dir, report_text):
    events = jsonl_events(pathlib.Path(task_dir) / "agent-events.jsonl")
    report_assertions = report_value(report_text, "runtime_spine_assertions", "none")
    assertions = [] if report_assertions == "none" else [
        assertion
        for assertion in (normalize_runtime_spine_assertion(item) for item in report_assertions.split(","))
        if assertion
    ]
    metrics = runtime_spine_metrics_from_events(events, report_text, assertions)
    for key in [
        "runtime_spine",
        "runtime_spine_detail",
        "runtime_spine_trace_present",
        "runtime_spine_phase_coverage",
        "runtime_spine_observed_phases",
        "runtime_spine_assertions",
        "runtime_spine_status",
        "runtime_spine_missing",
        "risky_tool_runs",
        "risky_tool_reviewed",
        "risky_tool_missing_action_review",
        "gate_outcomes",
        "gate_outcome_records",
        "gate_outcome_total",
        "gate_outcome_protective_blocks",
        "gate_outcome_recoverable_friction",
        "gate_outcome_unrecovered_blocks",
        "gate_outcome_suspected_false_positives",
        "gate_outcome_policy_correct_but_ux_costly",
        "gate_outcome_harmless_passes",
        "gate_outcome_failure_owners",
        "stop_reason",
        "stop_terminal_status",
        "stop_action",
        "stop_failure_type",
        "stop_recovery_plan_id",
        "rollback_recommended",
        "rollback_completed",
        "trace_failure_types",
        "recovery_failure_types",
        "recovery_kinds",
        "recovery_requires_user",
        "recovery_side_effect_uncertain",
        "action_scoring_active",
        "selected_action_score",
        "selected_action_score_min",
        "selected_action_score_avg",
        "low_score_actions",
        "low_action_score_count",
        "phase_misaligned_actions",
        "action_score_formula_version",
        "observer_modifier_applied",
        "memory_modifier_applied",
        "scope_fit_revision",
        "early_edit_demoted",
        "candidate_action_count",
        "candidate_selected_by_runtime",
        "candidate_score_calibrated",
        "candidate_score_disagreement",
        "score_driven_replan",
        "score_driven_stop",
        "agent_loop_steps",
        "agent_loop_latest_stage_before",
        "agent_loop_latest_stage_after",
        "agent_loop_latest_stop_reason",
        "state_transition_recorded",
        "context_zones_materialized",
        "context_zone_relevant_items",
        "context_zone_observation_items",
        "context_zone_task_state_empty",
        "context_zone_current_decision_request_empty",
        "observer_outcome_recorded",
        "observer_outcome_latest_status",
        "observer_outcome_latest_findings",
        "observer_outcome_latest_evidence",
        "memory_boundary_recorded",
        "task_contract_recorded",
        "context_pack_recorded",
        "execution_report_recorded",
        "memory_proposal_recorded",
        "memory_boundary_read_status",
        "memory_boundary_closeout_write_candidate_status",
        "completion_contract_status",
        "completion_contract_terminal_status",
        "completion_contract_proof_status",
        "verification_proof_status",
        "verification_proof_summary",
        "verification_proof_kinds",
        "verification_proof_support_status",
        "verification_proof_support_summary",
        "verification_proof_supports_verified",
        "verification_proof_residual_risk",
    ]:
        value = report_value(report_text, key, "")
        if value:
            metrics[key] = value
    return metrics


def output_assertions_from_sample(sample):
    raw = sample.get("output_assertions")
    if raw is None:
        raw = (sample.get("quality_assertions") or {}).get("output")
    if raw is None:
        return {}
    if isinstance(raw, str):
        return {"contains": [raw]}
    if isinstance(raw, list):
        return {"contains": raw}
    if isinstance(raw, dict):
        return raw
    return {"contains": [str(raw)]}


def trajectory_assertions_from_sample(sample):
    raw = sample.get("trajectory_assertions")
    if raw is None:
        raw = (sample.get("quality_assertions") or {}).get("trajectory")
    if raw is None:
        return {}
    return raw if isinstance(raw, dict) else {}


def evaluate_output_assertions(sample, output):
    assertions = output_assertions_from_sample(sample)
    if not assertions:
        return {
            "output_assertions": "none",
            "output_assertion_status": "none",
            "output_assertion_missing": "none",
        }

    missing = []
    output_text = output or ""
    output_lower = output_text.lower()

    def normalized_values(value):
        if value is None:
            return []
        if isinstance(value, str):
            return [value]
        if isinstance(value, list):
            return value
        return [value]

    for expected in assertions.get("contains") or []:
        expected = str(expected)
        if expected and expected.lower() not in output_lower:
            missing.append(f"contains:{expected}")
    for group in assertions.get("contains_any") or []:
        values = normalized_values(group)
        group_values = [str(value) for value in values if str(value)]
        if group_values and not any(value.lower() in output_lower for value in group_values):
            missing.append(f"contains_any:{'|'.join(group_values)}")
    for forbidden in (
        assertions.get("not_contains")
        or assertions.get("forbidden")
        or assertions.get("forbidden_output")
        or []
    ):
        forbidden = str(forbidden)
        if forbidden and forbidden.lower() in output_lower:
            missing.append(f"not_contains:{forbidden}")
    for pattern in assertions.get("regex") or []:
        pattern = str(pattern)
        try:
            if not re.search(pattern, output_text, re.MULTILINE | re.IGNORECASE):
                missing.append(f"regex:{pattern}")
        except re.error:
            missing.append(f"invalid_regex:{pattern}")
    for group in assertions.get("regex_any") or []:
        patterns = [str(value) for value in normalized_values(group) if str(value)]
        if not patterns:
            continue
        matched = False
        invalid_patterns = []
        for pattern in patterns:
            try:
                if re.search(pattern, output_text, re.MULTILINE | re.IGNORECASE):
                    matched = True
                    break
            except re.error:
                invalid_patterns.append(pattern)
        if not matched:
            if invalid_patterns and len(invalid_patterns) == len(patterns):
                missing.append(f"invalid_regex_any:{'|'.join(invalid_patterns)}")
            else:
                missing.append(f"regex_any:{'|'.join(patterns)}")

    labels = []
    for key in (
        "contains",
        "contains_any",
        "not_contains",
        "forbidden",
        "forbidden_output",
        "regex",
        "regex_any",
    ):
        values = assertions.get(key) or []
        if isinstance(values, str):
            values = [values]
        if values:
            labels.append(f"{key}={len(values)}")

    return {
        "output_assertions": ",".join(labels) if labels else "configured",
        "output_assertion_status": "failed" if missing else "passed",
        "output_assertion_missing": ";".join(missing) if missing else "none",
    }


def product_differentiation_assertions_from_sample(sample):
    raw = sample.get("product_differentiation_assertions")
    if raw is None:
        raw = (sample.get("quality_assertions") or {}).get("product_differentiation")
    if raw is None:
        return {}
    return raw if isinstance(raw, dict) else {}


def evaluate_product_differentiation_assertions(sample, output, report_text, runtime_spine):
    assertions = product_differentiation_assertions_from_sample(sample)
    if not assertions:
        return {
            "product_assertions": "none",
            "product_assertion_status": "none",
            "product_assertion_missing": "none",
        }

    combined = f"{output or ''}\n{report_text or ''}"
    combined_lower = combined.lower()
    missing = []

    def require(flag, condition):
        if assertions.get(flag) is True and not condition:
            missing.append(flag)

    require("requires_task_contract", runtime_spine.get("task_contract_recorded") == "true")
    require("requires_context_pack", runtime_spine.get("context_pack_recorded") == "true")
    require("requires_execution_report", runtime_spine.get("execution_report_recorded") == "true")
    require("requires_memory_proposal", runtime_spine.get("memory_proposal_recorded") == "true")
    require("requires_assumptions", "assumption" in combined_lower)
    require(
        "requires_prior_memory_citation",
        "project memory" in combined_lower or "memory/project.md" in combined_lower,
    )
    require(
        "requires_prior_execution_report_citation",
        "execution report" in combined_lower or "previous_execution_report" in combined_lower,
    )
    require(
        "requires_proposal_evidence",
        runtime_spine.get("memory_proposal_recorded") == "true" and "evidence" in combined_lower,
    )
    require(
        "requires_proposal_scope",
        runtime_spine.get("memory_proposal_recorded") == "true" and "scope" in combined_lower,
    )

    if assertions.get("forbids_auto_memory_write") is True and "write_performed=true" in combined_lower:
        missing.append("forbids_auto_memory_write")
    if assertions.get("forbids_cloud_scope") is True and any(
        term in combined_lower
        for term in ("oauth", "cloud deployment", "database server")
    ):
        missing.append("forbids_cloud_scope")
    if assertions.get("forbids_scope_expansion") is True and any(
        term in combined_lower
        for term in ("add login", "cloud sync is next", "deployment is next")
    ):
        missing.append("forbids_scope_expansion")
    if assertions.get("forbids_invented_state") is True and "implemented and verified" in combined_lower:
        missing.append("forbids_invented_state")

    labels = sorted(key for key, value in assertions.items() if value is True)
    return {
        "product_assertions": ",".join(labels) if labels else "configured",
        "product_assertion_status": "failed" if missing else "passed",
        "product_assertion_missing": ";".join(unique_items(missing)) if missing else "none",
    }


def action_was_revised_before_execution(action_event, action_review_events):
    call_id = str(action_event.get("call_id", "")).strip()
    if not call_id:
        return False
    for review in action_review_events:
        if str(review.get("call_id", "")).strip() != call_id:
            continue
        if token(review.get("decision", "")) in {"revise", "deny", "block", "blocked", "reject"}:
            return True
    return False


def derived_trajectory_metrics_from_events(
    events,
    report_text="",
    output="",
    sample=None,
    test_status="missing",
    cmd_log_text="",
):
    sample = sample or {}
    trace_items = trace_events(events)
    action_decision_events = [
        event
        for event in trace_items
        if token(event.get("type", "")) == "action_decision_evaluated"
    ]
    action_review_events = [
        event for event in trace_items if token(event.get("type", "")) == "action_reviewed"
    ]
    stop_check_events = [
        event for event in trace_items if token(event.get("type", "")) == "stop_check_evaluated"
    ]
    verification_events = [
        event for event in trace_items if token(event.get("type", "")) == "verification_completed"
    ]
    stage_validation_events = [
        event
        for event in trace_items
        if token(event.get("type", "")) == "stage_validation_completed"
    ]
    tool_observation_events = [
        event
        for event in trace_items
        if token(event.get("type", "")) == "tool_observation_recorded"
    ]
    api_events = [
        event
        for event in trace_items
        if token(event.get("type", ""))
        in {"api_request_started", "api_request_completed"}
    ]
    tool_starts = [
        event for event in events if token(event.get("event", "")) == "tool_execution_start"
    ]
    tool_completes = [
        event for event in events if token(event.get("event", "")) == "tool_execution_complete"
    ]
    if not tool_starts:
        tool_starts = [
            event for event in trace_items if token(event.get("type", "")) == "tool_started"
        ]

    write_indexes = [
        index
        for index, event in enumerate(tool_starts, start=1)
        if token(event_tool_name(event)) in WRITE_TOOL_NAMES
    ]
    first_write_index = write_indexes[0] if write_indexes else None
    evidence_before_first_edit = False
    if first_write_index is None:
        evidence_before_first_edit = True
    else:
        preceding_tools = [
            token(event_tool_name(event)) for event in tool_starts[: first_write_index - 1]
        ]
        evidence_before_first_edit = any(
            name in EVIDENCE_TOOL_NAMES for name in preceding_tools
        ) or bool(tool_observation_events)

    early_edit_demoted = any(
        token(event.get("reason", "")) == "low_value_action"
        for event in action_review_events
    ) or any(
        event.get("phase_aligned") is False
        and bool(event.get("mutates_workspace"))
        and token(event.get("stage", "")) in {"understand", "diagnosis"}
        and not action_was_revised_before_execution(event, action_review_events)
        for event in action_decision_events
    )
    premature_edit_count = 0
    if first_write_index == 1 and not evidence_before_first_edit:
        premature_edit_count += 1
    if early_edit_demoted:
        premature_edit_count += 1

    scope_drift_count = sum(
        1
        for event in action_decision_events
        if event.get("scope_fit") is not None and int_value(event.get("scope_fit"), 10) <= 2
        and not action_was_revised_before_execution(event, action_review_events)
    )
    scope_drift_count += sum(
        1
        for event in trace_items
        if token(event.get("type", "")) == "goal_drift_detected"
        and token(event.get("severity", event.get("level", ""))) in {"medium", "high"}
    )

    repeated_action_count = 0
    previous_key = None
    for event in tool_starts:
        key = event_sequence_key(event)
        if previous_key == key:
            repeated_action_count += 1
        previous_key = key
    repeated_action_count += sum(
        1
        for event in stop_check_events
        if token(event.get("reason", ""))
        in {"repeated_action_revision", "low_action_value_loop"}
    )

    failed_action_count = sum(
        1
        for event in tool_completes
        if "result: error" in str(event.get("result_preview", "")).lower()
    )
    failed_action_count += sum(
        1
        for event in trace_items
        if token(event.get("type", "")) == "tool_completed"
        and event.get("success") is False
    )

    risky_gaps = risky_tool_action_review_gaps(events)
    risky_missing_review_count = len(risky_gaps["missing"])
    phase_misaligned_actions = sum(
        1 for event in action_decision_events if event.get("phase_aligned") is False
    )
    invalid_action_count = (
        premature_edit_count
        + scope_drift_count
        + repeated_action_count
        + risky_missing_review_count
        + phase_misaligned_actions
    )

    user_question_count = sum(
        1 for event in tool_starts if token(event_tool_name(event)) == "ask_user"
    )
    user_question_count += sum(
        1
        for event in stop_check_events
        if token(event.get("action", "")) == "ask_user"
        or token(event.get("terminal_status", "")) == "needs_user"
    )
    eval_intent = token(sample.get("eval_intent", report_value(report_text, "eval_intent", "")))
    unnecessary_question_count = (
        user_question_count
        if eval_intent in {"direct_answer", "read_only_audit"}
        and "needs user" not in output.lower()
        else 0
    )

    verification_attempted = bool(
        verification_events
        or stage_validation_events
        or str(test_status).strip() not in {"", "missing", "skipped"}
        or cmd_log_text.strip()
    )
    verification_passed = (
        str(test_status).strip() == "ok"
        or any(event.get("passed") is True for event in verification_events)
        or any(token(event.get("status", "")) in {"passed", "ok"} for event in stage_validation_events)
    )
    llm_call_count = max(
        1 if events else 0,
        sum(1 for event in api_events if token(event.get("type", "")) == "api_request_completed"),
        sum(1 for event in events if token(event.get("event", "")) == "usage"),
    )

    return {
        "premature_edit_count": str(premature_edit_count),
        "evidence_before_first_edit": bool_text(evidence_before_first_edit),
        "scope_drift_count": str(scope_drift_count),
        "invalid_action_count": str(invalid_action_count),
        "repeated_action_count": str(repeated_action_count),
        "failed_action_count": str(failed_action_count),
        "user_question_count": str(user_question_count),
        "unnecessary_question_count": str(unnecessary_question_count),
        "verification_attempted": bool_text(verification_attempted),
        "verification_passed": bool_text(verification_passed),
        "tool_call_count": str(len(tool_starts) or int_value(report_value(report_text, "tool_executions", 0))),
        "llm_call_count": str(llm_call_count),
        "risky_missing_review_count": str(risky_missing_review_count),
        "phase_misaligned_action_count": str(phase_misaligned_actions),
        "first_write_tool_index_normalized": str(first_write_index) if first_write_index else "none",
    }


def derived_trajectory_metrics(task_dir, report_text, output="", sample=None, test_status="missing", cmd_log_text=""):
    events = jsonl_events(pathlib.Path(task_dir) / "agent-events.jsonl")
    return derived_trajectory_metrics_from_events(
        events,
        report_text=report_text,
        output=output,
        sample=sample,
        test_status=test_status,
        cmd_log_text=cmd_log_text,
    )


def evaluate_trajectory_assertions(sample, trajectory_metrics, runtime_spine=None):
    assertions = trajectory_assertions_from_sample(sample)
    if not assertions:
        return {
            "trajectory_assertions": "none",
            "trajectory_assertion_status": "none",
            "trajectory_assertion_missing": "none",
        }
    runtime_spine = runtime_spine or {}
    missing = []
    labels = []

    def require_bool(field, metric_key=None):
        metric_key = metric_key or field
        if assertions.get(field) is True:
            labels.append(field)
            if str(trajectory_metrics.get(metric_key, "false")).lower() != "true":
                missing.append(field)

    require_bool("evidence_before_edit", "evidence_before_first_edit")
    require_bool("requires_observer_outcome", "observer_outcome_recorded")
    if assertions.get("requires_stop_check") is True:
        labels.append("requires_stop_check")
        stop_action = str(runtime_spine.get("stop_action") or trajectory_metrics.get("stop_action") or "")
        if token(stop_action) in {"", "missing", "none", "null"}:
            missing.append("requires_stop_check")

    for field, metric_key in [
        ("max_repeated_action_count", "repeated_action_count"),
        ("max_scope_drift_count", "scope_drift_count"),
        ("max_premature_edit_count", "premature_edit_count"),
        ("max_invalid_action_count", "invalid_action_count"),
        ("max_failed_action_count", "failed_action_count"),
    ]:
        if field in assertions:
            labels.append(field)
            if int_value(trajectory_metrics.get(metric_key), 0) > int_value(assertions.get(field), 0):
                missing.append(f"{field}:{trajectory_metrics.get(metric_key)}>{assertions.get(field)}")

    if assertions.get("requires_runtime_spine_passed") is True:
        labels.append("requires_runtime_spine_passed")
        if runtime_spine.get("runtime_spine_status") != "passed":
            missing.append("requires_runtime_spine_passed")

    return {
        "trajectory_assertions": ",".join(labels) if labels else "configured",
        "trajectory_assertion_status": "failed" if missing else "passed",
        "trajectory_assertion_missing": ";".join(unique_items(missing)) if missing else "none",
    }


def score_live_eval_record(record):
    warnings = set(split_list(record.get("warnings")))
    failures = set(split_list(record.get("failures")))
    behavior_status = token(record.get("behavior_assertion_status", "none"))
    output_status = token(record.get("output_assertion_status", "none"))
    trajectory_status = token(record.get("trajectory_assertion_status", "none"))
    product_status = token(record.get("product_assertion_status", "none"))
    runtime_status = token(record.get("runtime_spine_status", "none"))
    required = token(record.get("required", record.get("required_command_status", "missing")))
    closeout = token(record.get("closeout", record.get("closeout_status", "missing")))
    verification = token(record.get("verification", record.get("verification_status", "unknown")))
    status = token(record.get("status", record.get("quality_status", "missing")))
    eval_intent = token(record.get("intent", record.get("eval_intent", "")))
    diff = token(record.get("diff", "no"))

    outcome = 100
    outcome_penalties = []

    def penalize_outcome(points, reason):
        nonlocal outcome
        outcome -= points
        outcome_penalties.append(reason)

    if status in {"failed", "error"}:
        penalize_outcome(25, "run_failed")
    elif status in {"skipped", "missing"}:
        penalize_outcome(10, "run_unscored")
    if required == "failed":
        penalize_outcome(25, "required_commands_failed")
    elif required == "missing":
        penalize_outcome(5, "required_commands_missing")
    if verification == "failed" or (
        record.get("verification_passed") == "false"
        and record.get("verification_attempted") == "true"
    ):
        penalize_outcome(20, "verification_failed")
    if closeout in {"failed", "not_verified", "missing"}:
        penalize_outcome(15, "closeout_not_successful")
    if runtime_status in {"failed", "missing"}:
        penalize_outcome(10, "runtime_spine_failed")
    if behavior_status == "failed":
        penalize_outcome(10, "behavior_assertions_failed")
    if output_status == "failed":
        penalize_outcome(10, "output_assertions_failed")
    if trajectory_status == "failed":
        penalize_outcome(10, "trajectory_assertions_failed")
    if product_status == "failed":
        penalize_outcome(10, "product_assertions_failed")
    if "forbidden_tool_used" in failures or "forbidden_tool_used" in warnings:
        penalize_outcome(30, "forbidden_tool_used")
    if eval_intent == "seeded_code_change" and diff == "no":
        penalize_outcome(15, "expected_code_diff_missing")

    process = 100
    process_penalties = []

    def penalize_process(points, reason):
        nonlocal process
        process -= points
        process_penalties.append(reason)

    premature_edits = int_value(record.get("premature_edit_count"), 0)
    scope_drifts = int_value(record.get("scope_drift_count"), 0)
    repeated_actions = int_value(record.get("repeated_action_count"), 0)
    invalid_actions = int_value(record.get("invalid_action_count"), 0)
    risky_missing = int_value(record.get("risky_missing_review_count"), 0)
    if record.get("evidence_before_first_edit") == "false":
        penalize_process(20, "missing_evidence_before_edit")
    if premature_edits:
        penalize_process(min(30, 15 * premature_edits), "premature_edit")
    if scope_drifts:
        penalize_process(min(30, 15 * scope_drifts), "scope_drift")
    if repeated_actions:
        penalize_process(min(20, 8 * repeated_actions), "repeated_action")
    if invalid_actions:
        penalize_process(min(20, 5 * invalid_actions), "invalid_action")
    if risky_missing:
        penalize_process(min(25, 12 * risky_missing), "risky_tool_missing_review")
    if runtime_status in {"failed", "missing"}:
        penalize_process(15, "runtime_spine_not_passing")
    if record.get("observer_outcome_recorded") == "false" and int_value(record.get("tool_call_count"), 0) > 0:
        penalize_process(8, "observer_outcome_missing")
    if token(record.get("stop_action", "")) in {"", "missing", "none", "null"}:
        penalize_process(5, "stop_check_missing")

    efficiency = 100
    efficiency_penalties = []

    def penalize_efficiency(points, reason):
        nonlocal efficiency
        efficiency -= points
        efficiency_penalties.append(reason)

    tool_calls = int_value(record.get("tool_call_count", record.get("tool_executions")), 0)
    failed_actions = int_value(record.get("failed_action_count", record.get("tool_failures")), 0)
    user_questions = int_value(record.get("user_question_count"), 0)
    unnecessary_questions = int_value(record.get("unnecessary_question_count"), 0)
    max_tools = 10 if record.get("mva_profile_active") == "true" else 25
    if tool_calls > max_tools:
        penalize_efficiency(min(25, (tool_calls - max_tools) * 2), "tool_budget_exceeded")
    if failed_actions:
        penalize_efficiency(min(25, failed_actions * 8), "failed_actions")
    if repeated_actions:
        penalize_efficiency(min(20, repeated_actions * 7), "repeated_actions")
    if user_questions:
        penalize_efficiency(min(15, user_questions * 5), "user_questions")
    if unnecessary_questions:
        penalize_efficiency(min(20, unnecessary_questions * 10), "unnecessary_questions")
    if int_value(record.get("llm_call_count"), 0) > 8:
        penalize_efficiency(10, "llm_call_budget_pressure")

    outcome = clamp_int(outcome)
    process = clamp_int(process)
    efficiency = clamp_int(efficiency)
    agent = clamp_int(outcome * 0.5 + process * 0.3 + efficiency * 0.2)
    penalties = unique_items(outcome_penalties + process_penalties + efficiency_penalties)

    return {
        "outcome_score": score_text(outcome),
        "process_score": score_text(process),
        "efficiency_score": score_text(efficiency),
        "agent_score": score_text(agent),
        "score_penalties": ",".join(penalties) if penalties else "none",
    }


FAILURE_CLASS_ORDER = [
    "runtime_spine",
    "tool_contract",
    "file_state",
    "bash_permission",
    "permission_recovery",
    "compaction_continuity",
    "llm_reasoning",
    "desktop_evidence",
]


def classify_failure_classes(
    task_id,
    report_text,
    quality_text,
    agent_output,
    warnings,
    failures,
    failure_owner,
    run_status,
):
    text = "\n".join(
        [
            task_id,
            report_text,
            quality_text,
            agent_output,
            " ".join(warnings),
            " ".join(failures),
            failure_owner,
        ]
    ).lower()
    classes = set()

    if any(
        marker in text
        for marker in [
            "runtime_spine_assertions_not_passing",
            "runtime_spine_status: failed",
            "runtime_spine_status=failed",
            "runtime_spine_status: missing",
            "runtime_spine_status=missing",
        ]
    ):
        classes.add("runtime_spine")
    if any(
        marker in text
        for marker in [
            "tool_contract",
            "tool_errors_seen",
            "forbidden_tool",
            "invalid tool",
            "not exposed",
            "orphan tool",
            "tool-result",
            "tool_result",
            "schema",
            "strict schema",
        ]
    ):
        classes.add("tool_contract")
    if any(
        marker in text
        for marker in [
            "file_state",
            "stale",
            "read-before-edit",
            "read before edit",
            "checkpoint",
            "rollback",
            "file_change",
            "no_code_diff",
            "diff_files_changed: 0",
            "settings_schema_validation",
        ]
    ):
        classes.add("file_state")
    if any(
        marker in text
        for marker in [
            "bash_permission",
            "shell_fail_closed",
            "shell_structure_review",
            "dangerous command",
            "redirection",
            "heredoc",
            "command_substitution",
            "permission_family: shell",
        ]
    ):
        classes.add("bash_permission")
    if any(
        marker in text
        for marker in [
            "permission_recovery",
            "permission denied",
            "permission_denied",
            "permission pending",
            "approval",
            "rejected",
            "denial",
        ]
    ):
        classes.add("permission_recovery")
    if any(
        marker in text
        for marker in [
            "compaction_continuity",
            "compact",
            "runtime_continuity",
            "context too long",
            "prompt too long",
            "retained_context",
            "token_delta",
        ]
    ):
        classes.add("compaction_continuity")
    if any(
        marker in text
        for marker in [
            "desktop_evidence",
            "desktop",
            "screenshot",
            "ui smoke",
            "tauri",
            "visual",
        ]
    ):
        classes.add("desktop_evidence")
    if (
        failure_owner == "llm_reasoning"
        or "llm_reasoning" in text
        or "missing_closeout" in text
        or "closeout_not_successful" in text
        or "empty_agent_output" in text
        or "no_code_diff" in text
        or "audit_no_code_diff" in text
        or "required_commands_not_passing" in text
    ):
        classes.add("llm_reasoning")

    if run_status == "failed" and not classes:
        classes.add("llm_reasoning")

    return [name for name in FAILURE_CLASS_ORDER if name in classes]


def specialty_metrics(task_dir, report_text):
    events = jsonl_events(task_dir / "agent-events.jsonl")
    task_name = task_dir.name.lower()
    report_lower = report_text.lower()
    event_types = trace_event_types(events)
    trace_items = trace_events(events)
    tool_starts = [
        event for event in events if event.get("event") == "tool_execution_start"
    ]

    retrieval_events = [
        event for event in trace_items if event.get("type") == "retrieval_context_built"
    ]
    memory_retrievals = [
        event
        for event in retrieval_events
        if any(str(source) == "Memory" for source in event.get("sources") or [])
    ]
    memory_tools = [
        event
        for event in tool_starts
        if str(event.get("name", "")).startswith("memory")
    ]
    skill_tools = [
        event
        for event in tool_starts
        if str(event.get("name", "")).startswith("skill")
        or str(event.get("name", "")) == "skill_manage"
    ]
    learning_adjusted = any(
        event.get("type") == "workflow_learning_adjusted" for event in trace_items
    )
    plan_reweighted = any(
        event.get("type") == "workflow_plan_progress" and event.get("reweighted") is True
        for event in trace_items
    )
    memory_sync_events = event_types.count("memory.sync")
    memory_recalled_items = sum(int(event.get("items") or 0) for event in memory_retrievals)
    memory_conflicts = sum(int(event.get("conflicts") or 0) for event in memory_retrievals)
    memory_record_used = any(
        "memory_record/" in " ".join(str(value) for value in event.get("provenance") or [])
        for event in memory_retrievals
    ) or "memory_record/" in report_lower
    memory_proposal = memory_proposal_metrics_from_trace(trace_items, report_text)
    memory_use_count_updated = (
        "use_count" in report_lower
        or "last_used" in report_lower
        or "memory_use_count_updated=true" in report_lower
    )
    memory_failure_lesson_promoted = (
        "strategy-failures" in report_lower
        or "failed_strategy=" in report_lower
        or "memory_failure_lesson_promoted=true" in report_lower
    )
    memory_action_weight_changed = any(
        str(event.get("reason", "")).lower().find("memory modifier") >= 0
        for event in trace_items
        if event.get("type") in {"action_decision_evaluated", "action.decision"}
    ) or "memory_action_weight_changed=true" in report_lower
    memory_stale_demoted = (
        "memory_stale_demoted=true" in report_lower
        or "stale" in report_lower and "memory" in report_lower and "demot" in report_lower
    )
    memory_scope_correct = (
        "memory_scope_correct=true" in report_lower
        or "scope=" in report_lower and "project=" in report_lower
    )

    report_memory_active = report_value(report_text, "memory_active", "")
    memory_active = (
        parse_boolish(report_memory_active)
        if report_memory_active
        else bool(memory_sync_events or memory_tools or memory_retrievals)
    )
    report_skill_active = report_value(report_text, "skill_active", "")
    skill_promotion_signal = (
        "skill-promotion" in task_name
        or "promotion gate" in report_lower
        or "compare_skill_versions_for_promotion" in report_lower
        or "validate_skill_promotion_for_apply" in report_lower
    )
    skill_active = (
        parse_boolish(report_skill_active)
        if report_skill_active
        else bool(skill_tools or "skill" in " ".join(event_types) or skill_promotion_signal)
    )
    memory_changed_plan = parse_boolish(
        report_value(report_text, "memory_changed_plan", "")
    ) or learning_adjusted or plan_reweighted
    skill_promotion_evidence = parse_boolish(
        report_value(report_text, "skill_promotion_evidence", "")
    ) or bool(skill_tools or skill_promotion_signal)

    memory_summary = "active={active}, recalled={recalled}, conflicts={conflicts}, changed_plan={changed}".format(
        active=bool_text(memory_active),
        recalled=int_text(report_value(report_text, "memory_recalled_items", memory_recalled_items)),
        conflicts=int_text(report_value(report_text, "memory_conflicts", memory_conflicts)),
        changed=bool_text(memory_changed_plan),
    )
    skill_summary = "active={active}, tool_calls={tool_calls}, usage_events={usage}, promotion={promotion}".format(
        active=bool_text(skill_active),
        tool_calls=int_text(report_value(report_text, "skill_tool_calls", len(skill_tools))),
        usage=int_text(report_value(report_text, "skill_usage_events", 0)),
        promotion=bool_text(skill_promotion_evidence),
    )

    return {
        "memory_active": bool_text(memory_active),
        "memory_recalled_items": int_text(
            report_value(report_text, "memory_recalled_items", memory_recalled_items)
        ),
        "memory_conflicts": int_text(
            report_value(report_text, "memory_conflicts", memory_conflicts)
        ),
        "memory_changed_plan": bool_text(memory_changed_plan),
        "memory_candidate_typed": memory_proposal["memory_candidate_typed"],
        "memory_candidate_has_evidence": memory_proposal["memory_candidate_has_evidence"],
        "memory_proposal_recorded": memory_proposal["memory_proposal_recorded"],
        "memory_proposal_status": memory_proposal["memory_proposal_status"],
        "memory_proposal_candidates": memory_proposal["memory_proposal_candidates"],
        "memory_proposal_kinds": memory_proposal["memory_proposal_kinds"],
        "memory_proposal_evidence_items": memory_proposal["memory_proposal_evidence_items"],
        "memory_proposal_write_policy": memory_proposal["memory_proposal_write_policy"],
        "memory_proposal_write_performed": memory_proposal["memory_proposal_write_performed"],
        "memory_record_used": bool_text(memory_record_used),
        "memory_use_count_updated": bool_text(memory_use_count_updated),
        "memory_failure_lesson_promoted": bool_text(memory_failure_lesson_promoted),
        "memory_action_weight_changed": bool_text(memory_action_weight_changed),
        "memory_stale_demoted": bool_text(memory_stale_demoted),
        "memory_scope_correct": bool_text(memory_scope_correct),
        "memory": memory_summary,
        "skill_active": bool_text(skill_active),
        "skill_tool_calls": int_text(report_value(report_text, "skill_tool_calls", len(skill_tools))),
        "skill_usage_events": int_text(report_value(report_text, "skill_usage_events", 0)),
        "skill_promotion_evidence": bool_text(skill_promotion_evidence),
        "skill": skill_summary,
    }


def report_rows(run_dir):
    run_dir = pathlib.Path(run_dir)
    rows = []
    for report in sorted(run_dir.glob("*/report.md")):
        task_dir = report.parent
        task_id = task_dir.name
        report_text = read(report)
        quality_text = read(task_dir / "agent-quality-status.txt")
        test_status = read(task_dir / "test-status.txt").strip() or "missing"
        diff_stat = read(task_dir / "diff-stat.txt").strip()
        agent_output = read(task_dir / "agent-output.md")
        cmd_log_text = read(task_dir / "required-commands.log")
        sample_json = task_dir / "sample.json"
        try:
            sample = json.loads(read(sample_json)) if sample_json.exists() else {}
        except Exception:
            sample = {}
        plan_file = task_dir / "minimax-plan.md"
        plan_lint = task_dir / "plan-lint.txt"
        api_response = task_dir / "api-response.json"
        agent_events = task_dir / "agent-events.jsonl"
        quality_status = status_value(quality_text, "status", "missing")
        failure_owner = status_value(
            quality_text,
            "failure_owner",
            report_value(report_text, "failure_owner", "missing"),
        )
        eval_intent = report_value(report_text, "eval_intent", "missing")
        closeout = report_value(report_text, "closeout_status", "missing")
        closeout_tool_records = report_value(
            report_text, "closeout_tool_records", "missing"
        )
        closeout_tool_evidence = report_value(
            report_text, "closeout_tool_evidence", "missing"
        )
        runtime_diet = report_value(report_text, "runtime_diet", "missing")
        workflow_contract_activation = report_value(
            report_text, "workflow_contract_activation", "missing"
        )
        risk_signal = report_value(report_text, "risk_signal", "missing")
        runtime_profile = report_value(report_text, "runtime_profile", "none")
        mva_profile_active = report_value(report_text, "mva_profile_active", "false")
        behavior_assertions = report_value(
            report_text, "behavior_assertions", "none"
        )
        behavior_assertion_status = report_value(
            report_text, "behavior_assertion_status", "none"
        )
        runtime_spine = runtime_spine_metrics(task_dir, report_text)
        adaptive_triggers = report_value(report_text, "adaptive_triggers", "none")
        first_write = report_value(report_text, "first_write_tool_index", "missing")
        required = report_value(report_text, "required_command_status", test_status)
        if plan_file.exists():
            plan_quality = status_value(read(plan_lint), "status", "missing")
        elif api_response.exists():
            plan_quality = "api_response"
        else:
            plan_quality = "none"
        if agent_events.exists():
            tool_boundary = "agent-run"
        elif plan_file.exists() or api_response.exists():
            tool_boundary = "plan-only"
        else:
            tool_boundary = "collect-only"
        if closeout == "passed" and required == "ok":
            verification_status = "passed"
        elif quality_status == "failed" or test_status == "failed":
            verification_status = "failed"
        else:
            verification_status = "unknown"
        run_status = report_run_status(
            tool_boundary,
            quality_status,
            test_status,
            plan_quality,
        )
        warnings = []
        output_warning_markers = {
            "action_checkpoint_no_patch": "Stopped action checkpoint without patch synthesis",
            "action_checkpoint_invalid_tools": "Stopped action checkpoint after repeated invalid tool requests",
            "patch_synthesis_no_change": "Patch synthesis did not produce a file change",
        }
        for warning in (
            "no_code_diff",
            "audit_no_code_diff",
            "current_head_no_fixture_already_satisfied",
            "tool_errors_seen",
            "action_checkpoint_no_patch",
            "action_checkpoint_invalid_tools",
            "patch_synthesis_no_change",
        ):
            if (
                has_warning(quality_text, warning)
                or warning in report_values(report_text, "warning")
                or output_warning_markers.get(warning, "\0") in agent_output
            ):
                warnings.append(warning)
        failures = unique_items(
            status_values(quality_text, "failure")
            + [
                warning
                for warning in report_values(report_text, "warning")
                if warning not in warnings
            ]
        )
        failure_classes = classify_failure_classes(
            task_id,
            report_text,
            quality_text,
            agent_output,
            warnings,
            failures,
            failure_owner,
            run_status,
        )
        specialty = specialty_metrics(task_dir, report_text)
        output_assertions = evaluate_output_assertions(sample, agent_output)
        trajectory = derived_trajectory_metrics(
            task_dir,
            report_text,
            output=agent_output,
            sample=sample,
            test_status=test_status,
            cmd_log_text=cmd_log_text,
        )
        trajectory_assertions = evaluate_trajectory_assertions(
            sample,
            {**trajectory, **runtime_spine},
            runtime_spine=runtime_spine,
        )
        product_assertions = evaluate_product_differentiation_assertions(
            sample,
            agent_output,
            report_text,
            runtime_spine,
        )
        tool_executions = int_value(report_value(report_text, "tool_executions", 0))
        diff_files_changed = int_value(report_value(report_text, "diff_files_changed", 0))
        validation_events = int_value(report_value(report_text, "validation_events", 0))
        stage_validation_events = int_value(
            report_value(report_text, "stage_validation_events", 0)
        )
        tool_failures = int_value(report_value(report_text, "tool_failures", 0))
        repair_signals = tool_failures
        for warning in warnings:
            if warning.startswith("earlier_") or warning.startswith("recovered_"):
                repair_signals += 1
        if (
            "required_validation" in adaptive_triggers
            and required == "ok"
            and (tool_failures > 0 or "tool_errors_seen" in warnings)
        ):
            repair_signals += 1
        if run_status == "passed" and required == "ok":
            if repair_signals > 0:
                first_pass_signal = "repaired"
            elif first_write not in {"none", "missing"}:
                first_pass_signal = "likely_clean"
            else:
                first_pass_signal = "no_write"
        elif run_status == "failed":
            first_pass_signal = "failed"
        else:
            first_pass_signal = "unknown"
        if tool_boundary == "agent-run":
            if run_status == "passed" and required == "ok":
                coding_gauntlet_status = "passed"
            elif run_status == "failed":
                coding_gauntlet_status = "failed"
            else:
                coding_gauntlet_status = "unscored"
        else:
            coding_gauntlet_status = "not_applicable"
        coding_summary = (
            f"tools={tool_executions}, "
            f"tool_records={closeout_tool_records}, "
            f"validations={validation_events + stage_validation_events}, "
            f"repair={repair_signals}, files={diff_files_changed}"
        )
        row = {
            "task": task_id,
            "status": run_status,
            "intent": eval_intent,
            "owner": failure_owner,
            "required": required,
            "plan": plan_quality,
            "boundary": tool_boundary,
            "verification": verification_status,
            "closeout": closeout,
            "closeout_tool_records": closeout_tool_records,
            "closeout_tool_evidence": closeout_tool_evidence,
            "runtime_diet": runtime_diet,
            "workflow_contract_activation": workflow_contract_activation,
            "risk_signal": risk_signal,
            "runtime_profile": runtime_profile,
            "mva_profile_active": mva_profile_active,
            "behavior_assertions": behavior_assertions,
            "behavior_assertion_status": behavior_assertion_status,
            **runtime_spine,
            **trajectory,
            **output_assertions,
            **trajectory_assertions,
            **product_assertions,
            "triggers": adaptive_triggers,
            "first_write": first_write,
            "diff": "yes" if diff_stat else "no",
            "warnings": ",".join(warnings) if warnings else "none",
            "failures": failures,
            "failure_classes": failure_classes,
            "failure_class": ",".join(failure_classes) if failure_classes else "none",
            "has_output": bool(agent_output.strip()),
            "coding_gauntlet_status": coding_gauntlet_status,
            "first_pass_signal": first_pass_signal,
            "tool_executions": str(tool_executions),
            "validation_events": str(validation_events),
            "stage_validation_events": str(stage_validation_events),
            "repair_signals": str(repair_signals),
            "diff_files_changed": str(diff_files_changed),
            "coding": coding_summary,
            **specialty,
        }
        row.update(score_live_eval_record(row))
        rows.append(row)
    return rows
