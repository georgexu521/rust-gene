//! LabRun provider diagnostics and durable smoke commands.
//!
//! Provider commands certify whether the current model/tool configuration can
//! support LabRun flows. They should report evidence and failure ownership
//! rather than weakening LabRun gates.

use super::*;

pub(super) fn handle_provider_command(project_root: &Path, tool_context: ToolContext) -> String {
    let report = provider_certification_report(&tool_context);
    let mut lines = vec![
        "Lab provider diagnostics:".to_string(),
        format!("Provider: {}", report.provider_id),
        format!("Model: {}", report.model),
        format!(
            "Graduate diagnostic status: {}",
            report.graduate_certification.as_str()
        ),
        format!(
            "Graduate dispatch policy: {}",
            if report.graduate_execution_allowed {
                "provider_neutral_task_evidence"
            } else {
                "blocked"
            }
        ),
        format!(
            "Graduate execution reason: {}",
            report.graduate_execution_policy.reason
        ),
        format!(
            "Graduate safeguards: isolated_worktree={} controlled_validation={} postdoc_audit={} user_override_required={}",
            report
                .graduate_execution_policy
                .isolated_worktree_required,
            report
                .graduate_execution_policy
                .controlled_validation_required,
            report.graduate_execution_policy.postdoc_audit_required,
            report.graduate_execution_policy.user_override_required
        ),
        format!(
            "Graduate proof labels: {}",
            report
                .graduate_execution_policy
                .proof_labels
                .join(",")
        ),
        format!("Diagnostic override enabled: {}", report.override_enabled),
        format!("Legacy override env enabled: {}", report.override_enabled),
        format!("Control-plane diagnostic: {}", report.control_plane_command),
        format!("Graduate diagnostic: {}", report.graduate_command),
        format!("Recommendation: {}", report.recommendation),
    ];
    lines.push(provider_record_line(
        "Latest control-plane record",
        report.latest_control_plane_record.as_ref(),
    ));
    lines.push(provider_record_line(
        "Latest graduate record",
        report.latest_graduate_record.as_ref(),
    ));
    lines.push(format!(
        "Diagnostics store: {}",
        LabStore::for_project(project_root)
            .root()
            .join("provider_certifications.jsonl")
            .display()
    ));
    lines.join("\n")
}

fn provider_record_line(label: &str, record: Option<&LabProviderCertificationRecord>) -> String {
    let Some(record) = record else {
        return format!("{label}: none");
    };
    format!(
        "{}: {} {} at {} evidence={} summary={}",
        label,
        record.kind.as_str(),
        record.outcome.as_str(),
        record.recorded_at,
        record.evidence_path,
        truncate_single_line(&record.summary, 160)
    )
}

pub(super) fn handle_provider_record_command(
    project_root: &Path,
    rest: &str,
    tool_context: ToolContext,
) -> String {
    let (kind_raw, rest) = split_once(rest);
    let (outcome_raw, rest) = split_once(rest);
    let (evidence_path, summary) = split_once(rest);
    if kind_raw.trim().is_empty()
        || outcome_raw.trim().is_empty()
        || evidence_path.trim().is_empty()
    {
        return "Usage: /lab provider record <control-plane|graduate> <passed|failed> <evidence_path> [summary]".to_string();
    }
    let kind = match parse_provider_certification_kind(kind_raw) {
        Ok(kind) => kind,
        Err(err) => return err,
    };
    let outcome = match parse_provider_certification_outcome(outcome_raw) {
        Ok(outcome) => outcome,
        Err(err) => return err,
    };
    let report = provider_certification_report(&tool_context);
    if report.provider_id == "unknown" || report.model == "unknown" {
        return "Failed to record provider diagnostic: active provider/model is unknown"
            .to_string();
    }
    let summary = if summary.trim().is_empty() {
        format!(
            "{} {} validation {}",
            report.provider_id,
            report.model,
            outcome.as_str()
        )
    } else {
        summary.trim().to_string()
    };
    let store = LabStore::for_project(project_root);
    match store.record_provider_certification(
        &report.provider_id,
        &report.model,
        kind,
        outcome,
        evidence_path,
        &summary,
    ) {
        Ok(record) => format!(
            "Recorded provider diagnostic: {}\nProvider: {}\nModel: {}\nKind: {}\nOutcome: {}\nEvidence: {}\nStore: {}",
            record.record_id,
            record.provider_id,
            record.model,
            record.kind.as_str(),
            record.outcome.as_str(),
            record.evidence_path,
            store.root().join("provider_certifications.jsonl").display()
        ),
        Err(err) => format!("Failed to record provider diagnostic: {err}"),
    }
}

fn parse_provider_certification_kind(value: &str) -> Result<LabProviderCertificationKind, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "control-plane" | "control_plane" | "control" | "professor" => {
            Ok(LabProviderCertificationKind::ControlPlane)
        }
        "graduate" | "lab-graduate" | "worker" => Ok(LabProviderCertificationKind::Graduate),
        _ => {
            Err("Invalid provider diagnostic kind. Expected control-plane or graduate.".to_string())
        }
    }
}

fn parse_provider_certification_outcome(
    value: &str,
) -> Result<LabProviderCertificationOutcome, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "passed" | "pass" | "ok" | "success" | "succeeded" => {
            Ok(LabProviderCertificationOutcome::Passed)
        }
        "failed" | "fail" | "error" | "blocked" => Ok(LabProviderCertificationOutcome::Failed),
        _ => Err("Invalid provider diagnostic outcome. Expected passed or failed.".to_string()),
    }
}

pub(super) async fn handle_provider_compare_command(
    project_root: &Path,
    tool_context: ToolContext,
) -> String {
    let report = provider_certification_report(&tool_context);
    let generic_result =
        run_generic_subagent_provider_smoke(project_root, tool_context.clone()).await;
    let background_result =
        run_generic_background_subagent_provider_smoke(project_root, tool_context.clone()).await;
    let lab_result =
        run_lab_graduate_provider_smoke(project_root, tool_context, &report.provider_id).await;
    [
        "Provider subagent comparison:".to_string(),
        format!("Provider: {}", report.provider_id),
        format!("Model: {}", report.model),
        format!(
            "Graduate diagnostic status: {}",
            report.graduate_certification.as_str()
        ),
        format!(
            "Graduate dispatch policy: {}",
            if report.graduate_execution_allowed {
                "provider_neutral_task_evidence"
            } else {
                "blocked"
            }
        ),
        format!(
            "Graduate execution reason: {}",
            report.graduate_execution_policy.reason
        ),
        format!(
            "Graduate safeguards: isolated_worktree={} controlled_validation={} postdoc_audit={} user_override_required={}",
            report
                .graduate_execution_policy
                .isolated_worktree_required,
            report
                .graduate_execution_policy
                .controlled_validation_required,
            report.graduate_execution_policy.postdoc_audit_required,
            report.graduate_execution_policy.user_override_required
        ),
        format!(
            "Graduate proof labels: {}",
            report
                .graduate_execution_policy
                .proof_labels
                .join(",")
        ),
        String::new(),
        generic_result.summary.clone(),
        String::new(),
        background_result.summary.clone(),
        String::new(),
        lab_result.summary.clone(),
        String::new(),
        provider_compare_conclusion(&generic_result, &lab_result),
    ]
    .join("\n")
}

pub(super) async fn handle_provider_tool_diagnostics_command(tool_context: ToolContext) -> String {
    let report = provider_certification_report(&tool_context);
    let Some(provider) = tool_context.llm_provider.clone() else {
        return "Usage: /lab provider diagnose-tools requires the Lab Mode shell provider. In non-interactive mode use `pa lab --command \"provider diagnose-tools\" --with-provider`."
            .to_string();
    };
    if tool_context.model.trim().is_empty() {
        return "Usage: /lab provider diagnose-tools requires an active model.".to_string();
    }

    let probes = vec![
        ProviderToolProbe {
            label: "minimal_auto".to_string(),
            tool_choice: ToolChoice::Auto,
            tools: vec![provider_echo_tool()],
            user_prompt: "Call lab_provider_echo with message exactly `tool probe ok`."
                .to_string(),
        },
        ProviderToolProbe {
            label: "minimal_required".to_string(),
            tool_choice: ToolChoice::Required,
            tools: vec![provider_echo_tool()],
            user_prompt: "Call lab_provider_echo with message exactly `tool probe ok`."
                .to_string(),
        },
        ProviderToolProbe {
            label: "minimal_forced".to_string(),
            tool_choice: ToolChoice::Function("lab_provider_echo".to_string()),
            tools: vec![provider_echo_tool()],
            user_prompt: "Call lab_provider_echo with message exactly `tool probe ok`."
                .to_string(),
        },
        ProviderToolProbe {
            label: "runtime_file_write_auto".to_string(),
            tool_choice: ToolChoice::Auto,
            tools: provider_runtime_tools(&["file_write"]),
            user_prompt: "Use file_write to create `lab-provider-schema-probe.txt` with content exactly `schema probe ok`."
                .to_string(),
        },
        ProviderToolProbe {
            label: "runtime_file_write_bash_auto".to_string(),
            tool_choice: ToolChoice::Auto,
            tools: provider_runtime_tools(&["file_write", "bash"]),
            user_prompt: "Use file_write to create `lab-provider-schema-probe.txt` with content exactly `schema probe ok`; do not answer in prose."
                .to_string(),
        },
        ProviderToolProbe {
            label: "runtime_subagent_allowed_auto".to_string(),
            tool_choice: ToolChoice::Auto,
            tools: provider_runtime_tools(&["file_write", "file_edit", "bash", "diff"]),
            user_prompt: "Use file_write to create `lab-provider-schema-probe.txt` with content exactly `schema probe ok`; do not answer in prose."
                .to_string(),
        },
    ];
    let mut lines = vec![
        "Provider tool-call diagnostics:".to_string(),
        format!("Provider: {}", report.provider_id),
        format!("Model: {}", report.model),
        format!(
            "Graduate diagnostic status: {}",
            report.graduate_certification.as_str()
        ),
        format!(
            "Graduate dispatch policy: {}",
            if report.graduate_execution_allowed {
                "provider_neutral_task_evidence"
            } else {
                "blocked"
            }
        ),
        format!(
            "Graduate execution reason: {}",
            report.graduate_execution_policy.reason
        ),
        format!(
            "Graduate safeguards: isolated_worktree={} controlled_validation={} postdoc_audit={} user_override_required={}",
            report
                .graduate_execution_policy
                .isolated_worktree_required,
            report
                .graduate_execution_policy
                .controlled_validation_required,
            report.graduate_execution_policy.postdoc_audit_required,
            report.graduate_execution_policy.user_override_required
        ),
        format!(
            "Graduate proof labels: {}",
            report
                .graduate_execution_policy
                .proof_labels
                .join(",")
        ),
    ];

    for probe in probes {
        let label = probe.label.clone();
        let request = provider_tool_probe_request(&tool_context.model, probe);
        let request_tool_names = request
            .tools
            .as_ref()
            .map(|tools| {
                tools
                    .iter()
                    .map(|tool| tool.name.as_str())
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .unwrap_or_else(|| "none".to_string());
        let request_tool_count = request.tools.as_ref().map_or(0, Vec::len);
        let request_tool_choice = format!("{:?}", request.tool_choice);
        let response = provider.chat(request).await;
        lines.push(String::new());
        lines.push(format!("Probe: {label}"));
        lines.push(format!("  request_tools_count: {request_tool_count}"));
        lines.push(format!("  request_tools: {request_tool_names}"));
        lines.push(format!("  request_tool_choice: {request_tool_choice}"));
        match response {
            Ok(response) => append_provider_tool_probe_response(&mut lines, response),
            Err(err) => {
                lines.push("  success: false".to_string());
                lines.push(format!("  error: {err:#}"));
            }
        }
    }

    lines.join("\n")
}

struct ProviderToolProbe {
    label: String,
    tool_choice: ToolChoice,
    tools: Vec<ApiTool>,
    user_prompt: String,
}

fn provider_tool_probe_request(model: &str, probe: ProviderToolProbe) -> ChatRequest {
    ChatRequest::new(model)
        .with_messages(vec![
            ApiMessage::system(
                "You are a function-calling probe. When a tool is available, respond by calling the tool. Do not describe the tool call in prose.",
            ),
            ApiMessage::user(probe.user_prompt),
        ])
        .with_tools(probe.tools)
        .with_tool_choice(probe.tool_choice)
        .with_temperature(0.0)
        .with_output_cap(Some(256))
}

fn provider_runtime_tools(names: &[&str]) -> Vec<ApiTool> {
    let registry = crate::tools::ToolRegistry::full_registry();
    names
        .iter()
        .filter_map(|name| registry.get(name).map(provider_tool_from_runtime_tool))
        .collect()
}

fn provider_tool_from_runtime_tool(tool: &dyn Tool) -> ApiTool {
    ApiTool {
        name: tool.name().to_string(),
        description: tool.description().to_string(),
        parameters: tool.parameters(),
        strict_schema: tool.strict_schema(),
    }
}

fn provider_echo_tool() -> ApiTool {
    ApiTool {
        name: "lab_provider_echo".to_string(),
        description: "Echo a short diagnostic message.".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "message": {
                    "type": "string",
                    "description": "Diagnostic message to echo."
                }
            },
            "required": ["message"],
            "additionalProperties": false
        }),
        strict_schema: true,
    }
}

fn append_provider_tool_probe_response(lines: &mut Vec<String>, response: ChatResponse) {
    let tool_calls = response.tool_calls.unwrap_or_default();
    let tool_names = if tool_calls.is_empty() {
        "none".to_string()
    } else {
        tool_calls
            .iter()
            .map(|call| call.name.as_str())
            .collect::<Vec<_>>()
            .join(",")
    };
    let repair_summary = response
        .tool_call_repair
        .as_ref()
        .map(provider_tool_repair_summary)
        .unwrap_or_else(|| "none".to_string());
    lines.push("  success: true".to_string());
    lines.push(format!("  response_tool_calls_count: {}", tool_calls.len()));
    lines.push(format!("  response_tool_calls: {tool_names}"));
    lines.push(format!(
        "  finish_reason: {}",
        response.finish_reason.unwrap_or_else(|| "none".to_string())
    ));
    lines.push(format!("  tool_call_repair: {repair_summary}"));
    lines.push(format!(
        "  content_preview: {}",
        truncate_single_line(&response.content, 240)
    ));
}

fn provider_tool_repair_summary(
    report: &crate::services::api::tool_call_repair::ToolCallRepairReport,
) -> String {
    format!(
        "family={} flattened_tools={} scavenged={} argument_repairs={} unflattened={} duplicates_dropped={} malformed={} warnings={}",
        report.provider_family,
        report.schema_flattened_tools,
        report.scavenged_tool_calls,
        report.argument_repairs,
        report.unflattened_arguments,
        report.dropped_duplicate_calls,
        report.malformed_tool_calls,
        report.warnings.len()
    )
}

pub(super) struct ProviderComparePathResult {
    pub(super) summary: String,
    pub(super) success: bool,
    pub(super) used_mutating_tool: bool,
    pub(super) blocked_by_certification: bool,
}

async fn run_generic_subagent_provider_smoke(
    project_root: &Path,
    tool_context: ToolContext,
) -> ProviderComparePathResult {
    let session_store = tool_context.session_store.clone();
    let session_id = tool_context.session_id.clone();
    let task_id = "provider-compare-generic";
    let params = serde_json::json!({
        "description": "Provider comparison generic implementer smoke",
        "prompt": "Inside the current isolated worktree only, create or overwrite the relative path exactly `lab-provider-compare-generic.txt` with exactly one line: generic subagent tool smoke. Do not use an absolute path and do not write in the parent workspace. Then verify by reading the same relative path with file_read; if bash validation is allowed, also run `test -f lab-provider-compare-generic.txt`. Use real tools only, then summarize the tools used.",
        "files": ["lab-provider-compare-generic.txt"],
        "profile": "implementer",
        "context_mode": "isolated_worktree_fork",
        "allowed_tools": ["file_read", "file_write", "file_edit", "bash", "diff"],
        "timeout_secs": 90,
        "max_turns": 3,
        "task_id": task_id
    });
    let result = AgentTool::with_working_dir(project_root)
        .execute(params, tool_context)
        .await;
    if !result.success {
        if let Some(store) = session_store.as_ref() {
            if let Some(recovered) = recover_provider_compare_durable_subagent(
                "Generic subagent",
                store,
                &session_id,
                task_id,
                "lab-provider-compare-generic.txt",
                result.error.as_deref().unwrap_or("none"),
            )
            .await
            {
                return recovered;
            }
        }
    }
    let data = result.data.as_ref();
    let tools_used = data
        .and_then(|value| value.get("tools_used"))
        .map(format_json_value)
        .unwrap_or_else(|| "none".to_string());
    let allowed_tools = data
        .and_then(|value| value.get("allowed_tools"))
        .map(format_json_value)
        .unwrap_or_else(|| "unknown".to_string());
    let status = data
        .and_then(|value| value.get("status"))
        .and_then(Value::as_str)
        .unwrap_or(if result.success { "success" } else { "failed" });
    let agent_id = data
        .and_then(|value| value.get("agent_id"))
        .and_then(Value::as_str)
        .unwrap_or("none");
    let result_preview = data
        .and_then(|value| value.get("result"))
        .and_then(Value::as_str)
        .map(|value| truncate_single_line(value, 240))
        .unwrap_or_else(|| "none".to_string());
    let profile = data
        .and_then(|value| value.get("profile"))
        .map(format_json_value)
        .unwrap_or_else(|| "unknown".to_string());
    let context_mode = data
        .and_then(|value| value.get("context_mode"))
        .map(format_json_value)
        .unwrap_or_else(|| "unknown".to_string());
    let error = result.error.unwrap_or_else(|| "none".to_string());
    let attempted_mutating_tool = data
        .and_then(|value| value.get("tools_used"))
        .is_some_and(value_contains_mutating_tool);
    let (file_proof, file_exists) = subagent_smoke_file_proof(
        session_store.as_deref(),
        &session_id,
        agent_id,
        "lab-provider-compare-generic.txt",
    );
    ProviderComparePathResult {
        summary: [
            "Generic subagent:".to_string(),
            format!("  success: {}", result.success),
            format!("  status: {status}"),
            format!("  agent_id: {agent_id}"),
            format!("  profile: {profile}"),
            format!("  context_mode: {context_mode}"),
            format!("  allowed_tools: {allowed_tools}"),
            format!("  tools_used: {tools_used}"),
            format!("  file_proof: {file_proof}"),
            format!("  result_preview: {result_preview}"),
            format!("  error: {error}"),
        ]
        .join("\n"),
        success: result.success,
        used_mutating_tool: hard_subagent_mutation_proof(
            attempted_mutating_tool,
            file_exists,
            &result_preview,
        ),
        blocked_by_certification: false,
    }
}

pub(super) async fn recover_provider_compare_durable_subagent(
    label: &str,
    store: &std::sync::Arc<crate::session_store::SessionStore>,
    session_id: &str,
    task_id: &str,
    proof_file: &str,
    foreground_error: &str,
) -> Option<ProviderComparePathResult> {
    let mut final_state = None;
    for _ in 0..60 {
        match store.agent_task_state(session_id, task_id) {
            Ok(Some(state)) if state.result_artifact_id.is_some() || state.status != "running" => {
                final_state = Some(state);
                if final_state
                    .as_ref()
                    .is_some_and(|state| state.result_artifact_id.is_some())
                {
                    break;
                }
            }
            Ok(Some(state)) => {
                final_state = Some(state);
            }
            Ok(None) | Err(_) => {}
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }

    let state = final_state?;
    let artifact = state
        .result_artifact_id
        .and_then(|id| store.agent_artifact(session_id, id).ok().flatten());
    let tools_used = artifact
        .as_ref()
        .and_then(|artifact| artifact.payload.get("tools_used"))
        .map(format_json_value)
        .or_else(|| state.payload.get("tools_used").map(format_json_value))
        .unwrap_or_else(|| "none".to_string());
    let allowed_tools = state
        .payload
        .get("allowed_tools")
        .map(format_json_value)
        .unwrap_or_else(|| "unknown".to_string());
    let profile = state
        .profile
        .clone()
        .or_else(|| {
            state
                .payload
                .pointer("/agent_definition/name")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| "unknown".to_string());
    let context_mode = state
        .payload
        .get("context_mode")
        .map(format_json_value)
        .unwrap_or_else(|| "unknown".to_string());
    let completion_sink = artifact
        .as_ref()
        .and_then(|artifact| artifact.payload.get("completion_sink"))
        .and_then(Value::as_str)
        .or_else(|| state.payload.get("completion_sink").and_then(Value::as_str))
        .unwrap_or("none");
    let result_preview = artifact
        .as_ref()
        .map(|artifact| truncate_single_line(&artifact.output, 240))
        .unwrap_or_else(|| "none".to_string());
    let (file_proof, file_exists) = subagent_smoke_file_proof(
        Some(store.as_ref()),
        session_id,
        &state.agent_id,
        proof_file,
    );
    let attempted_mutating_tool = artifact
        .as_ref()
        .and_then(|artifact| artifact.payload.get("tools_used"))
        .is_some_and(value_contains_mutating_tool)
        || state
            .payload
            .get("tools_used")
            .is_some_and(value_contains_mutating_tool);
    let hard_mutation_proof =
        hard_subagent_mutation_proof(attempted_mutating_tool, file_exists, &result_preview);
    let success = state.status == "completed" && artifact.is_some() && hard_mutation_proof;

    Some(ProviderComparePathResult {
        summary: [
            format!("{label}:"),
            format!("  success: {success}"),
            format!("  status: {}", state.status),
            format!("  agent_id: {}", state.agent_id),
            format!("  task_id: {}", state.task_id),
            format!("  profile: {profile}"),
            format!("  context_mode: {context_mode}"),
            format!("  allowed_tools: {allowed_tools}"),
            format!("  tools_used: {tools_used}"),
            format!(
                "  result_artifact_id: {}",
                state
                    .result_artifact_id
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| "none".to_string())
            ),
            format!("  completion_sink: {completion_sink}"),
            format!("  file_proof: {file_proof}"),
            format!("  hard_file_proof: {file_exists}"),
            "  recovered_from_durable_sink: true".to_string(),
            format!("  result_preview: {result_preview}"),
            format!("  foreground_error: {foreground_error}"),
        ]
        .join("\n"),
        success,
        used_mutating_tool: hard_mutation_proof,
        blocked_by_certification: false,
    })
}

async fn run_generic_background_subagent_provider_smoke(
    project_root: &Path,
    tool_context: ToolContext,
) -> ProviderComparePathResult {
    let Some(store) = tool_context.session_store.clone() else {
        return ProviderComparePathResult {
            summary: "Background subagent:\n  skipped: no session store available for completion sink validation".to_string(),
            success: false,
            used_mutating_tool: false,
            blocked_by_certification: false,
        };
    };
    let session_id = tool_context.session_id.clone();
    let task_id = "provider-compare-background";
    let params = serde_json::json!({
        "description": "Provider comparison background implementer smoke",
        "prompt": "Inside the current isolated worktree only, create or overwrite the relative path exactly `lab-provider-compare-background.txt` with exactly one line: background subagent tool smoke. Do not use an absolute path and do not write in the parent workspace. Then verify by reading the same relative path with file_read; if bash validation is allowed, also run `test -f lab-provider-compare-background.txt`. Use real tools only, then summarize the tools used.",
        "files": ["lab-provider-compare-background.txt"],
        "profile": "implementer",
        "context_mode": "isolated_worktree_fork",
        "allowed_tools": ["file_read", "file_write", "file_edit", "bash", "diff"],
        "timeout_secs": 90,
        "max_turns": 3,
        "task_id": task_id,
        "background": true
    });
    let launch = AgentTool::with_working_dir(project_root)
        .execute(params, tool_context.clone())
        .await;
    if !launch.success {
        return ProviderComparePathResult {
            summary: [
                "Background subagent:".to_string(),
                "  success: false".to_string(),
                "  launch_status: failed".to_string(),
                format!("  task_id: {task_id}"),
                format!(
                    "  error: {}",
                    launch.error.unwrap_or_else(|| "none".to_string())
                ),
            ]
            .join("\n"),
            success: false,
            used_mutating_tool: false,
            blocked_by_certification: false,
        };
    }

    if let (Some(manager), Some(agent_id)) = (
        tool_context.agent_manager.as_ref(),
        launch
            .data
            .as_ref()
            .and_then(|data| data.get("agent_id"))
            .and_then(Value::as_str),
    ) {
        let _ = manager
            .wait_for_result(&AgentId(agent_id.to_string()), 180)
            .await;
    }

    let mut final_state = None;
    for _ in 0..120 {
        match store.agent_task_state(&session_id, task_id) {
            Ok(Some(state)) if state.status != "running" => {
                final_state = Some(state);
                break;
            }
            Ok(Some(state)) if state.result_artifact_id.is_some() => {
                final_state = Some(state);
                break;
            }
            Ok(Some(state)) => {
                final_state = Some(state);
            }
            Ok(None) | Err(_) => {}
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }

    let Some(state) = final_state else {
        return ProviderComparePathResult {
            summary: [
                "Background subagent:".to_string(),
                "  success: false".to_string(),
                "  launch_status: launched".to_string(),
                format!("  task_id: {task_id}"),
                "  durable_state: missing".to_string(),
            ]
            .join("\n"),
            success: false,
            used_mutating_tool: false,
            blocked_by_certification: false,
        };
    };

    let artifact = state
        .result_artifact_id
        .and_then(|id| store.agent_artifact(&session_id, id).ok().flatten());
    let tools_used = state
        .payload
        .get("tools_used")
        .map(format_json_value)
        .unwrap_or_else(|| "none".to_string());
    let completion_sink = artifact
        .as_ref()
        .and_then(|artifact| artifact.payload.get("completion_sink"))
        .and_then(Value::as_str)
        .or_else(|| state.payload.get("completion_sink").and_then(Value::as_str))
        .unwrap_or("none");
    let result_preview = artifact
        .as_ref()
        .map(|artifact| truncate_single_line(&artifact.output, 240))
        .unwrap_or_else(|| "none".to_string());
    let (file_proof, file_exists) = subagent_smoke_file_proof(
        Some(store.as_ref()),
        &session_id,
        &state.agent_id,
        "lab-provider-compare-background.txt",
    );
    let attempted_mutating_tool = value_contains_mutating_tool(&state.payload["tools_used"]);
    let hard_mutation_proof =
        hard_subagent_mutation_proof(attempted_mutating_tool, file_exists, &result_preview);
    let permission_denied = subagent_runtime_denied(&result_preview);
    let success = state.status == "completed"
        && artifact.is_some()
        && completion_sink == "agent_manager"
        && hard_mutation_proof;

    ProviderComparePathResult {
        summary: [
            "Background subagent:".to_string(),
            format!("  success: {success}"),
            "  launch_status: launched".to_string(),
            format!("  task_id: {}", state.task_id),
            format!("  status: {}", state.status),
            format!("  agent_id: {}", state.agent_id),
            format!(
                "  result_artifact_id: {}",
                state
                    .result_artifact_id
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| "none".to_string())
            ),
            format!("  completion_sink: {completion_sink}"),
            format!("  tools_used: {tools_used}"),
            format!("  file_proof: {file_proof}"),
            format!("  hard_file_proof: {file_exists}"),
            format!("  permission_denied: {permission_denied}"),
            format!("  result_preview: {result_preview}"),
        ]
        .join("\n"),
        success,
        used_mutating_tool: hard_mutation_proof,
        blocked_by_certification: false,
    }
}

async fn run_lab_graduate_provider_smoke(
    project_root: &Path,
    tool_context: ToolContext,
    provider_id: &str,
) -> ProviderComparePathResult {
    let orchestrator = LabOrchestrator::for_project(project_root);
    let store = orchestrator.store();
    let run = match store.latest_run() {
        Ok(Some(run)) if matches!(run.status, LabRunStatus::Active) => run,
        Ok(Some(run)) => {
            return ProviderComparePathResult {
                summary: format!(
                    "Lab graduate:\n  skipped: latest LabRun {} is not active ({:?})",
                    run.lab_run_id, run.status
                ),
                success: false,
                used_mutating_tool: false,
                blocked_by_certification: false,
            };
        }
        Ok(None) => {
            return ProviderComparePathResult {
                summary: "Lab graduate:\n  skipped: no active LabRun; create and approve a LabRun before comparing the Lab graduate path.".to_string(),
                success: false,
                used_mutating_tool: false,
                blocked_by_certification: false,
            };
        }
        Err(err) => {
            return ProviderComparePathResult {
                summary: format!("Lab graduate:\n  failed to read latest LabRun: {err}"),
                success: false,
                used_mutating_tool: false,
                blocked_by_certification: false,
            };
        }
    };
    let task = match store.create_graduate_task(
        &run.lab_run_id,
        "Provider comparison Lab graduate smoke",
        &format!(
            "Provider comparison smoke for {provider_id}. Create or overwrite lab-provider-compare-lab.txt with exactly one line: lab graduate tool smoke. Then run `test -f lab-provider-compare-lab.txt`. Return the required graduate_result JSON with changed_files containing lab-provider-compare-lab.txt and validation_results containing the test command."
        ),
        vec!["lab-provider-compare-lab.txt".to_string()],
        vec!["test -f lab-provider-compare-lab.txt".to_string()],
    ) {
        Ok(task) => task,
        Err(err) => {
            return ProviderComparePathResult {
                summary: format!("Lab graduate:\n  failed to create smoke task: {err}"),
                success: false,
                used_mutating_tool: false,
                blocked_by_certification: false,
            };
        }
    };
    match orchestrator
        .execute_graduate_task_latest_with_context(&task.task_id, tool_context.clone())
        .await
    {
        Ok(record) => {
            let error = record.error.as_deref().unwrap_or("none");
            let blocked_by_certification = error.contains("graduate provider")
                || error.contains("not certified")
                || error.contains("certification");
            let agent_task_id = graduate_agent_task_id(&task);
            let (durable_lines, durable_mutation_proof) = lab_graduate_durable_smoke_details(
                &tool_context,
                &agent_task_id,
                "lab-provider-compare-lab.txt",
            );
            let success = matches!(
                record.status,
                crate::lab::model::GraduateDispatchStatus::Succeeded
            );
            let mut summary = vec![
                "Lab graduate:".to_string(),
                format!("  success: {success}"),
                format!("  status: {:?}", record.status),
                format!("  dispatch_id: {}", record.dispatch_id),
                format!("  task_id: {}", record.task_id),
                format!("  durable_task_id: {agent_task_id}"),
                format!(
                    "  agent_id: {}",
                    record.agent_id.as_deref().unwrap_or("none")
                ),
                format!(
                    "  result_artifact_id: {}",
                    record.result_artifact_id.as_deref().unwrap_or("none")
                ),
                format!("  error: {error}"),
            ];
            summary.extend(durable_lines);
            ProviderComparePathResult {
                summary: summary.join("\n"),
                success,
                used_mutating_tool: success && durable_mutation_proof,
                blocked_by_certification,
            }
        }
        Err(err) => ProviderComparePathResult {
            summary: format!("Lab graduate:\n  failed before/while dispatching: {err}"),
            success: false,
            used_mutating_tool: false,
            blocked_by_certification: err.to_string().contains("certification"),
        },
    }
}

pub(super) fn lab_graduate_durable_smoke_details(
    tool_context: &ToolContext,
    agent_task_id: &str,
    file_name: &str,
) -> (Vec<String>, bool) {
    let Some(store) = tool_context.session_store.as_deref() else {
        return (
            vec!["  durable_state: unavailable: no session store".to_string()],
            false,
        );
    };
    let state = match store.agent_task_state(&tool_context.session_id, agent_task_id) {
        Ok(Some(state)) => state,
        Ok(None) => {
            return (
                vec![format!(
                    "  durable_state: missing for task_id {agent_task_id}"
                )],
                false,
            );
        }
        Err(err) => {
            return (vec![format!("  durable_state: error: {err}")], false);
        }
    };

    let artifact = state.result_artifact_id.and_then(|id| {
        store
            .agent_artifact(&tool_context.session_id, id)
            .ok()
            .flatten()
    });
    let tools_used = state
        .payload
        .get("tools_used")
        .map(format_json_value)
        .or_else(|| {
            artifact
                .as_ref()
                .and_then(|artifact| artifact.payload.get("tools_used"))
                .map(format_json_value)
        })
        .unwrap_or_else(|| "none".to_string());
    let completion_sink = artifact
        .as_ref()
        .and_then(|artifact| artifact.payload.get("completion_sink"))
        .and_then(Value::as_str)
        .or_else(|| state.payload.get("completion_sink").and_then(Value::as_str))
        .unwrap_or("none");
    let result_preview = artifact
        .as_ref()
        .map(|artifact| truncate_single_line(&artifact.output, 240))
        .unwrap_or_else(|| "none".to_string());
    let (file_proof, file_exists) = subagent_smoke_file_proof(
        Some(store),
        &tool_context.session_id,
        &state.agent_id,
        file_name,
    );
    let attempted_mutating_tool = state
        .payload
        .get("tools_used")
        .is_some_and(value_contains_mutating_tool);
    let hard_mutation_proof =
        hard_subagent_mutation_proof(attempted_mutating_tool, file_exists, &result_preview);
    let permission_denied = subagent_runtime_denied(&result_preview);
    let context_mode = state
        .payload
        .get("context_mode")
        .map(format_json_value)
        .unwrap_or_else(|| "unknown".to_string());
    let result_artifact_id = state
        .result_artifact_id
        .map(|id| id.to_string())
        .unwrap_or_else(|| "none".to_string());

    (
        vec![
            "  durable_state: present".to_string(),
            format!("  durable_status: {}", state.status),
            format!("  durable_agent_id: {}", state.agent_id),
            format!(
                "  durable_profile: {}",
                state.profile.as_deref().unwrap_or("none")
            ),
            format!("  durable_context_mode: {context_mode}"),
            format!("  durable_result_artifact_id: {result_artifact_id}"),
            format!("  completion_sink: {completion_sink}"),
            format!("  tools_used: {tools_used}"),
            format!("  file_proof: {file_proof}"),
            format!("  hard_file_proof: {file_exists}"),
            format!("  permission_denied: {permission_denied}"),
            format!("  durable_result_preview: {result_preview}"),
        ],
        hard_mutation_proof,
    )
}

fn provider_compare_conclusion(
    generic: &ProviderComparePathResult,
    lab: &ProviderComparePathResult,
) -> String {
    let conclusion = if lab.blocked_by_certification {
        if generic.used_mutating_tool {
            "Generic subagent demonstrated tool use; Lab graduate reported a legacy certification block, which should now be treated as a diagnostics bug rather than provider policy."
        } else if generic.success {
            "Generic subagent completed but produced no hard mutating-tool proof through tools_used or isolated-worktree file evidence; inspect task-level evidence before trusting graduate output."
        } else {
            "Generic subagent also failed; this points first at provider/subagent tool-call capability or runtime provider wiring, not only Lab graduate prompting."
        }
    } else if generic.used_mutating_tool && !lab.success {
        "Generic subagent demonstrated tool use but Lab graduate failed; inspect Lab graduate envelope, prompt contract, scope validation, JSON binding, and task evidence."
    } else if generic.success && lab.success {
        "Both paths completed; provider tool use is available through generic and Lab graduate routing."
    } else if !generic.success && lab.success {
        "Lab graduate completed while generic failed; compare profile/tool exposure differences before changing provider policy."
    } else {
        "Both paths failed or lacked tool-use proof; treat this run as weak task evidence and rely on postdoc review, not provider allowlists."
    };
    format!("Conclusion: {conclusion}")
}

fn value_contains_mutating_tool(value: &Value) -> bool {
    match value {
        Value::String(tool) => is_mutating_tool_name(tool),
        Value::Array(items) => items.iter().any(value_contains_mutating_tool),
        Value::Object(map) => map.values().any(value_contains_mutating_tool),
        _ => false,
    }
}

fn is_mutating_tool_name(tool: &str) -> bool {
    matches!(tool, "file_write" | "file_edit" | "bash" | "format")
}

pub(super) fn hard_subagent_mutation_proof(
    attempted_mutating_tool: bool,
    file_exists: bool,
    result_preview: &str,
) -> bool {
    attempted_mutating_tool && file_exists && !subagent_runtime_denied(result_preview)
}

fn subagent_runtime_denied(value: &str) -> bool {
    let value = value.to_ascii_lowercase();
    [
        "permission denied",
        "requires user confirmation",
        "blocked by the runtime permission system",
        "checkpoint_required",
        "action rejected before execution",
    ]
    .iter()
    .any(|needle| value.contains(needle))
}

fn format_json_value(value: &Value) -> String {
    match value {
        Value::Null => "none".to_string(),
        Value::String(value) => value.to_string(),
        Value::Array(items) if items.is_empty() => "none".to_string(),
        Value::Array(items) => items
            .iter()
            .map(format_json_value)
            .collect::<Vec<_>>()
            .join(","),
        other => other.to_string(),
    }
}

fn subagent_smoke_file_proof(
    store: Option<&crate::session_store::SessionStore>,
    session_id: &str,
    agent_id: &str,
    file_name: &str,
) -> (String, bool) {
    if agent_id == "none" {
        return ("unavailable: no agent_id".to_string(), false);
    }
    let Some(store) = store else {
        return ("unavailable: no session store".to_string(), false);
    };
    let state = match store.agent_task_state(session_id, agent_id) {
        Ok(Some(state)) => state,
        Ok(None) => return ("unavailable: no durable agent state".to_string(), false),
        Err(err) => return (format!("unavailable: durable state error: {err}"), false),
    };
    let Some(worktree_path) = state
        .payload
        .pointer("/isolated_worktree/path")
        .and_then(Value::as_str)
    else {
        return (
            "unavailable: no isolated_worktree path in durable state".to_string(),
            false,
        );
    };
    let proof_path = Path::new(worktree_path).join(file_name);
    let exists = proof_path.is_file();
    (
        format!("{} exists={}", proof_path.display(), exists),
        exists,
    )
}

fn truncate_single_line(value: &str, max_chars: usize) -> String {
    let single_line = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut truncated = single_line.chars().take(max_chars).collect::<String>();
    if single_line.chars().count() > max_chars {
        truncated.push_str("...");
    }
    truncated
}
