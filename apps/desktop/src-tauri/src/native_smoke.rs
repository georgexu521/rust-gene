use super::*;

pub(crate) fn native_smoke_enabled() -> bool {
    std::env::var("PRIORITY_AGENT_DESKTOP_NATIVE_SMOKE").as_deref() == Ok("1")
}

pub(crate) async fn emit_native_smoke_run_fixture(
    app: AppHandle,
    message: String,
    state: &State<'_, DesktopAppState>,
) -> Result<(), String> {
    {
        let mut pending = state.native_smoke_permission_pending.lock().await;
        *pending = true;
    }
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    let session_id = state.active_session_id.lock().await.clone();
    let events = vec![
        DesktopRunEvent::RunStarted {
            run_id: "native-smoke-run".to_string(),
            session_id,
        },
        DesktopRunEvent::ThinkingStarted,
        DesktopRunEvent::ThinkingCompleted,
        DesktopRunEvent::ToolStarted {
            id: "native-smoke-validation".to_string(),
            name: "bash".to_string(),
        },
        DesktopRunEvent::ToolExecutionProgress {
            id: "native-smoke-validation".to_string(),
            progress: "Running native validation fixture".to_string(),
        },
        DesktopRunEvent::ToolCompleted {
            id: "native-smoke-validation".to_string(),
            result_preview: "native smoke validation passed".to_string(),
            metadata: Some(serde_json::json!({
                "tool": "bash",
                "call_id": "native-smoke-validation",
                "success": true,
                "command": "scripts/desktop-native-smoke.sh --fixture-run",
                "command_category": "validation",
                "validation_family": "native_smoke",
                "command_kind": "script",
                "duration_ms": 410,
                "output_chars": 30,
                "terminal_task": {
                    "status": "completed",
                    "exit_code": 0,
                    "duration_ms": 410
                }
            })),
        },
        DesktopRunEvent::ToolStarted {
            id: "native-smoke-file".to_string(),
            name: "file_edit".to_string(),
        },
        DesktopRunEvent::ToolCompleted {
            id: "native-smoke-file".to_string(),
            result_preview: "Edited apps/desktop/src/app/Composer.tsx".to_string(),
            metadata: Some(serde_json::json!({
                "tool": "file_edit",
                "call_id": "native-smoke-file",
                "success": true,
                "path": "apps/desktop/src/app/Composer.tsx",
                "replacements": 1,
                "additions": 4,
                "deletions": 1,
                "diff_preview": "@@ -140,6 +140,9 @@\n <textarea aria-label=\"Message\" />\n+<button aria-label=\"Send message\" />\n",
                "diff_preview_truncated": false,
                "duration_ms": 55,
                "output_chars": 48
            })),
        },
        DesktopRunEvent::PermissionRequest {
            id: "native-smoke-permission".to_string(),
            tool_name: "bash".to_string(),
            arguments: serde_json::json!({
                "command": "git status --short"
            }),
            prompt: format!("Allow native smoke permission check for: {message}"),
            metadata: Some(serde_json::json!({
                "permission_evidence": {
                    "schema": "permission_decision_evidence.v1",
                    "request_kind": "runtime_rule",
                    "permission_family": "shell",
                    "decision": "ask",
                    "risk_level": "low",
                    "recovery": {
                        "recommended_action": "Approve once to continue the native smoke run."
                    },
                    "command_classification": {
                        "parser_status": "simple",
                        "category": "git",
                        "mutation": false
                    }
                },
                "action_review": {
                    "schema": "action_review.v1",
                    "tool": "bash",
                    "call_id": "native-smoke-permission",
                    "decision": "ask_user",
                    "primary_reason": "permission_required",
                    "permission": {
                        "allowed_by_context": true,
                        "requires_confirmation": true,
                        "decision": "Ask",
                        "risk_level": "Low",
                        "confidence": 0.82,
                        "warnings": []
                    },
                    "scope": {
                        "allowed": true,
                        "reason": "native smoke request is inside the selected project"
                    },
                    "budget": {
                        "allowed": true,
                        "scheduled_count": 0,
                        "max_tool_calls": 4,
                        "reason": "tool-call budget still has room"
                    },
                    "checkpoint": {
                        "required": false,
                        "status": "not_needed",
                        "enforcement": "none",
                        "rollback_scope": "none",
                        "requires_user_approval": false,
                        "reason": "git status is observational"
                    },
                    "side_effects": {
                        "schema": "action_side_effect_profile.v1",
                        "external_side_effect": "none",
                        "network": {
                            "class": "none",
                            "target": null,
                            "trusted": true,
                            "reason": "no network access detected"
                        },
                        "mutates_local_workspace": false,
                        "mutates_local_machine": false,
                        "remote_side_effect": false,
                        "paths": [],
                        "summary": "external_effect=None network=None paths=0"
                    },
                    "user_reason": "Action requires user confirmation before execution: permission_required.",
                    "model_recovery": "Action needs user approval before execution: permission_required. Wait for the permission result and do not claim the tool ran until it succeeds."
                }
            })),
            review: None,
        },
    ];

    for event in events {
        app.emit("desktop-run-event", event)
            .map_err(|err| err.to_string())?;
        tokio::time::sleep(std::time::Duration::from_millis(75)).await;
    }
    let _ = append_desktop_log(
        &state.diagnostic_logs_path,
        "native_smoke_fixture permission_request=true",
    );
    Ok(())
}

pub(crate) async fn emit_native_smoke_permission_resolution(
    app: AppHandle,
    diagnostic_logs_path: PathBuf,
    approved: bool,
) {
    tokio::time::sleep(std::time::Duration::from_millis(350)).await;
    if approved {
        let Some(window) = app.get_webview_window("main") else {
            let _ = append_desktop_log(
                &diagnostic_logs_path,
                "native_smoke_fixture emit_error=missing main window",
            );
            return;
        };
        let events = vec![
            DesktopRunEvent::ToolCompleted {
                id: "native-smoke-permission-result".to_string(),
                result_preview: "Permission approved; inspected git status".to_string(),
                metadata: Some(serde_json::json!({
                    "tool": "bash",
                    "call_id": "native-smoke-permission-result",
                    "success": true,
                    "command": "git status --short",
                    "command_category": "inspection",
                    "command_kind": "git",
                    "duration_ms": 75,
                    "output_chars": 12,
                    "terminal_task": {
                        "status": "completed",
                        "exit_code": 0,
                        "duration_ms": 75
                    }
                })),
            },
            DesktopRunEvent::AssistantDelta {
                text: "Native smoke fixture completed. Timeline cards, permission approval, and final answer rendering are visible.".to_string(),
            },
            DesktopRunEvent::RuntimeDiagnostic {
                diagnostic: serde_json::json!({
                    "schema": "desktop_runtime_diagnostic.v1",
                    "task_state": {
                        "goal": "native smoke fixture",
                        "mode": "full",
                        "stage": "closeout",
                        "mode_score": {
                            "confidence": 82,
                            "complexity": 7,
                            "risk": 5,
                            "uncertainty": 3,
                            "tool_need": 8,
                            "user_impact": 7
                        },
                        "lightweight_plan": null,
                        "verification": {
                            "status": "verified",
                            "required_checks": ["scripts/desktop-native-smoke.sh --fixture-run"]
                        },
                        "done": {
                            "satisfied": true,
                            "summary": "native smoke fixture completed"
                        },
                        "active_files": ["apps/desktop/src/app/Composer.tsx"],
                        "stop_check": {
                            "status": "stop",
                            "reason": "verification_ready",
                            "summary": "ready for closeout"
                        }
                    },
                    "verification_proof": {
                        "status": "verified",
                        "summary": "native smoke validation passed",
                        "closeout_status": "passed",
                        "changed_files": 1,
                        "validation_items": 1,
                        "acceptance_items": 1,
                        "residual_risks": 0
                    },
                    "control_loop": {
                        "coverage": "7/7",
                        "summary": "native smoke runtime diagnostic",
                        "phases": [
                            { "phase": "context", "events": 1, "latest_label": "task.context" },
                            { "phase": "decision", "events": 1, "latest_label": "action.decision" },
                            { "phase": "permission", "events": 1, "latest_label": "permission.resolve" },
                            { "phase": "tool_execution", "events": 3, "latest_label": "tool.done" },
                            { "phase": "state_update", "events": 1, "latest_label": "stop.check" },
                            { "phase": "verification", "events": 1, "latest_label": "verify.done" },
                            { "phase": "closeout", "events": 2, "latest_label": "assistant" }
                        ]
                    }
                }),
            },
            DesktopRunEvent::Usage {
                prompt_tokens: 32,
                completion_tokens: 18,
                reasoning_tokens: Some(4),
                cached_tokens: Some(8),
                cache_write_tokens: Some(12),
            },
            DesktopRunEvent::RunCompleted,
        ];
        for event in events {
            if let Err(err) = window.emit("desktop-run-event", event) {
                let _ = append_desktop_log(
                    &diagnostic_logs_path,
                    &format!(
                        "native_smoke_fixture emit_error={}",
                        sanitize_log_value(&err.to_string())
                    ),
                );
            }
            tokio::time::sleep(std::time::Duration::from_millis(75)).await;
        }
        let _ = append_desktop_log(&diagnostic_logs_path, "run_completed");
    } else {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.emit(
                "desktop-run-event",
                DesktopRunEvent::RunError {
                    message: "Native smoke permission rejected".to_string(),
                },
            );
        }
        let _ = append_desktop_log(
            &diagnostic_logs_path,
            "run_error message=Native smoke permission rejected",
        );
    }
}
