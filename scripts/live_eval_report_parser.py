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
        "api_request_started",
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
    "task_context": "event:task_context_built",
    "task_context_built": "event:task_context_built",
    "implementation_intent": "event:implementation_intent_recorded",
    "implementation_intent_recorded": "event:implementation_intent_recorded",
    "action_decision": "event:action_decision_evaluated",
    "action_decision_evaluated": "event:action_decision_evaluated",
    "action_review": "event:action_reviewed",
    "action_reviewed": "event:action_reviewed",
    "tool_observation": "special:tool_observation",
    "tool_observation_recorded": "event:tool_observation_recorded",
    "checkpoint_metadata": "special:checkpoint_metadata",
    "checkpoint_present": "special:checkpoint_metadata",
    "action_review_revise": "special:action_review_revise",
    "action_review_deny": "special:action_review_deny",
    "risky_tool_action_review": "special:risky_tool_action_review",
    "risky_tools_action_review": "special:risky_tool_action_review",
    "risky_tool_reviews": "special:risky_tool_action_review",
    "review_risky_tools": "special:risky_tool_action_review",
    "action_review_for_risky_tools": "special:risky_tool_action_review",
    "stop_check": "event:stop_check_evaluated",
    "stop_check_evaluated": "event:stop_check_evaluated",
    "closeout_prepared": "event:final_closeout_prepared",
    "final_closeout_prepared": "event:final_closeout_prepared",
    "verification_proof": "special:verification_proof",
    "verification_proof_present": "special:verification_proof",
    "verification_proof_verified": "special:verification_proof_verified",
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


def short_identifier(value, max_chars=12):
    text = str(value or "").strip()
    if not text:
        return "missing"
    return text if len(text) <= max_chars else text[:max_chars]


def event_tool_name(event):
    return str(event.get("tool") or event.get("name") or "").strip()


def event_call_id(event):
    return str(event.get("call_id") or event.get("id") or "").strip()


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
        }:
            return f"special:{name}"
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
        if raw.get("verification_proof") is True or raw.get("require_verification_proof") is True:
            values.append("verification_proof")
        if raw.get("verification_proof_verified") is True:
            values.append("verification_proof_verified")
        if raw.get("action_decision") is True:
            values.append("action_decision")
        if raw.get("stop_check") is True:
            values.append("stop_check")
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
    tool_observation_events = [
        event
        for event in trace_items
        if token(event.get("type", "")) == "tool_observation_recorded"
    ]
    risky_review_gaps = risky_tool_action_review_gaps(events)
    risky_tool_runs = len(risky_review_gaps["runs"])
    risky_tool_reviewed = risky_review_gaps["reviewed"]
    risky_tool_missing_reviews = [
        candidate["identity"] for candidate in risky_review_gaps["missing"]
    ]

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
    if not proof_status:
        proof_status = str(latest_closeout.get("verification_proof_status", "missing"))
    if not proof_summary:
        proof_summary = str(latest_closeout.get("verification_proof_summary", "missing"))

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
        elif kind == "special" and name == "risky_tool_action_review":
            if risky_tool_missing_reviews:
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
        f"risky_tool_missing_action_review={risky_tool_missing_text}"
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
        "verification_proof_status": proof_status,
        "verification_proof_summary": proof_summary,
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
        "verification_proof_status",
        "verification_proof_summary",
    ]:
        value = report_value(report_text, key, "")
        if value:
            metrics[key] = value
    return metrics


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
        rows.append(
            {
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
                "behavior_assertions": behavior_assertions,
                "behavior_assertion_status": behavior_assertion_status,
                **runtime_spine,
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
        )
    return rows
