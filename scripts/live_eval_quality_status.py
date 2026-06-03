#!/usr/bin/env python3
import json
import pathlib
import sys

REPO_ROOT = pathlib.Path(__file__).resolve().parents[1]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from scripts.live_eval_report_parser import (
    derived_trajectory_metrics_from_events,
    evaluate_output_assertions,
    evaluate_trajectory_assertions,
    memory_proposal_metrics_from_trace,
    normalized_runtime_spine_assertions,
    runtime_spine_metrics_from_events,
    score_live_eval_record,
)

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

def should_ignore_generated_dependency_path(path: str) -> bool:
    normalized = path.strip().lower()
    if normalized.startswith("./"):
        normalized = normalized[2:]
    return normalized.startswith(".venv/") or "/.venv/" in normalized

diff_files_for_limit = [
    path for path in diff_files if not should_ignore_generated_dependency_path(path)
]
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
risk_signal_events = [event for event in trace_events if event.get("type") == "risk_signal_assessed"]
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
entry_risk_signal = next((event for event in reversed(risk_signal_events) if event.get("phase") == "turn_entry"), {})
runtime_risk_signal = next((event for event in reversed(risk_signal_events) if event.get("phase") == "runtime"), {})
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
runtime_spine_assertions = normalized_runtime_spine_assertions(sample)
runtime_spine = runtime_spine_metrics_from_events(
    events,
    assertions=runtime_spine_assertions,
)
trajectory_metrics = derived_trajectory_metrics_from_events(
    events,
    output=output,
    sample=sample,
    test_status=test_status,
    cmd_log_text=cmd_log_text,
)
output_assertions = evaluate_output_assertions(sample, output)
trajectory_assertions = evaluate_trajectory_assertions(
    sample,
    {**trajectory_metrics, **runtime_spine},
    runtime_spine=runtime_spine,
)
runtime_profile = str(sample.get("runtime_profile", "")).strip()
mva_profile_active = runtime_profile in {"minimum_viable_agent", "mva"}
if (
    eval_intent in {"direct_answer", "read_only_audit"}
    and closeout_status == "missing"
    and runtime_spine.get("completion_contract_status") == "completed"
):
    closeout_status = "passed"
expected_runtime_completion = str(
    (sample.get("runtime_spine_assertions") or {}).get("completion_status", "")
).strip().lower()
if (
    expected_runtime_completion == "blocked"
    and runtime_spine.get("completion_contract_status") == "blocked"
    and test_status == "ok"
):
    closeout_status = "passed"

print(f"output_chars: {len(output)}")
print(f"diff_chars: {len(diff)}")
print(f"diff_files_changed: {len(diff_files_for_limit)}")
print(f"diff_files_changed_raw: {len(diff_files)}")
print(f"generated_dependency_files_ignored: {len(diff_files) - len(diff_files_for_limit)}")
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
print(f"closeout_tool_records: {latest_closeout.get('tool_records', 0)}")
print(f"closeout_tool_evidence: {latest_closeout.get('tool_evidence', 'missing')}")
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
risk_entry = entry_risk_signal.get("level", "missing") if entry_risk_signal else "missing"
risk_runtime = runtime_risk_signal.get("level", "none") if runtime_risk_signal else "none"
print(f"risk_signal: entry={risk_entry} runtime={risk_runtime}")
if entry_risk_signal:
    print("risk_signal_reasons: " + "; ".join(str(item) for item in (entry_risk_signal.get("reasons") or [])))
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

code_change_types = {"bug_fix", "feature", "refactor", "ux"}
current_head_without_fixture = (
    task_type in code_change_types
    and base_ref in {"", "HEAD", "head"}
    and not prepare_commands
)
seeded_code_change = eval_intent == "seeded_code_change"
audit_or_regression_check = eval_intent in {"audit_or_regression_check", "read_only_audit"}
stale_or_already_satisfied = eval_intent == "stale_or_already_satisfied"
print(f"eval_intent: {eval_intent}")
print(f"behavior_assertions: {','.join(behavior_assertions) if behavior_assertions else 'none'}")
print(f"behavior_assertion_status: {behavior_assertion_status}")
print(f"output_assertions: {output_assertions['output_assertions']}")
print(f"output_assertion_status: {output_assertions['output_assertion_status']}")
print(f"output_assertion_missing: {output_assertions['output_assertion_missing']}")
print(f"trajectory_assertions: {trajectory_assertions['trajectory_assertions']}")
print(f"trajectory_assertion_status: {trajectory_assertions['trajectory_assertion_status']}")
print(f"trajectory_assertion_missing: {trajectory_assertions['trajectory_assertion_missing']}")
print(f"runtime_spine: {runtime_spine['runtime_spine']}")
print(f"runtime_profile: {runtime_profile or 'none'}")
print(f"mva_profile_active: {str(mva_profile_active).lower()}")
print(f"runtime_spine_detail: {runtime_spine['runtime_spine_detail']}")
print(f"runtime_spine_trace_present: {runtime_spine['runtime_spine_trace_present']}")
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
print(f"route_recovery: {runtime_spine['route_recovery']}")
print(f"route_recovery_events: {runtime_spine['route_recovery_events']}")
print(f"route_recovery_failure_types: {runtime_spine['route_recovery_failure_types']}")
print(f"route_recovery_kinds: {runtime_spine['route_recovery_kinds']}")
print(f"route_recovery_read_search_expanded: {runtime_spine['route_recovery_read_search_expanded']}")
print(f"route_recovery_mutation_blocked: {runtime_spine['route_recovery_mutation_blocked']}")
print(f"route_recovery_safety_monotonic: {runtime_spine['route_recovery_safety_monotonic']}")
print(f"route_recovery_unsafe_mutation_expansion: {runtime_spine['route_recovery_unsafe_mutation_expansion']}")
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
for key in (
    "premature_edit_count",
    "evidence_before_first_edit",
    "scope_drift_count",
    "invalid_action_count",
    "repeated_action_count",
    "failed_action_count",
    "user_question_count",
    "unnecessary_question_count",
    "verification_attempted",
    "verification_passed",
    "tool_call_count",
    "llm_call_count",
):
    print(f"{key}: {trajectory_metrics[key]}")
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
if "no effective progress timeout after" in stderr_text.lower():
    print("warning: no_effective_progress_timeout")
    failures.append("no_effective_progress_timeout")
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
if max_files_changed is not None and len(diff_files_for_limit) > max_files_changed:
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
if output_assertions["output_assertion_status"] == "failed":
    print("warning: output_assertions_not_passing")
    failures.append("output_assertions_not_passing")
if trajectory_assertions["trajectory_assertion_status"] == "failed":
    print("warning: trajectory_assertions_not_passing")
    failures.append("trajectory_assertions_not_passing")
if runtime_spine["runtime_spine_status"] in {"failed", "missing"}:
    print("warning: runtime_spine_assertions_not_passing")
    failures.append("runtime_spine_assertions_not_passing")
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
        "non-streaming chat timed out after" in provider_text
        or "chat timed out after" in provider_text
        or "tool-result continuation timed out after" in provider_text
        or "provider health step timed out after" in provider_text
    ):
        return "environment"
    if "no effective progress timeout after" in provider_text:
        return "llm_reasoning"
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
    if "modulenotfounderror" in lower_cmd:
        return "eval_harness"
    if "failed to import test module" in lower_cmd:
        if "syntaxerror" in lower_cmd or "indentationerror" in lower_cmd:
            return "llm_reasoning"
        return "eval_harness"
    if "empty_agent_output" in failures or "missing_trace_summary" in failures:
        return "agent_flow"
    if (
        "required_commands_not_passing" in failures
        and "expected_code_diff_missing" in failures
        and closeout_status in {"failed", "not_verified", "blocked", "missing"}
    ):
        return "llm_reasoning"
    if "runtime_spine_assertions_not_passing" in failures:
        return "agent_flow"
    if "trajectory_assertions_not_passing" in failures:
        return "agent_flow"
    if "output_assertions_not_passing" in failures:
        return "llm_reasoning"
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
score_record = {
    "status": "failed" if failures else "passed",
    "intent": eval_intent,
    "required": test_status,
    "verification": "passed" if verification_passed else "failed",
    "closeout": closeout_status,
    "behavior_assertion_status": behavior_assertion_status,
    "output_assertion_status": output_assertions["output_assertion_status"],
    "trajectory_assertion_status": trajectory_assertions["trajectory_assertion_status"],
    "runtime_spine_status": runtime_spine["runtime_spine_status"],
    "diff": "yes" if diff.strip() else "no",
    "warnings": warnings,
    "failures": failures,
    "mva_profile_active": str(mva_profile_active).lower(),
    **runtime_spine,
    **trajectory_metrics,
}
scorecard = score_live_eval_record(score_record)
for key in ("outcome_score", "process_score", "efficiency_score", "agent_score", "score_penalties"):
    print(f"{key}: {scorecard[key]}")
with status_path.open("w", encoding="utf-8") as fh:
    fh.write(f"status={status}\n")
    fh.write(f"failure_owner={failure_owner}\n")
    for item in failures:
        fh.write(f"failure={item}\n")
    for item in warnings:
        fh.write(f"warning={item}\n")