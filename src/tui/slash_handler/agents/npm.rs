//! Agent slash-command handler support.
//!
//! Keeps agent listing, launch, auth, and environment checks behind the slash-command boundary.

use super::*;

/// /npm - npm 包管理辅助
pub async fn handle_npm(app: &mut TuiApp, args: &str) -> String {
    let tool = crate::tools::BashTool;
    let ctx = app.build_tool_context().await;

    let parts: Vec<&str> = args.split_whitespace().collect();
    let action = parts.first().unwrap_or(&"");

    match *action {
        "install" => {
            let pkg = parts.get(1).unwrap_or(&"");
            let cmd = if pkg.is_empty() {
                "npm install".to_string()
            } else {
                format!("npm install {}", pkg)
            };
            let params = serde_json::json!({
                "command": cmd,
                "description": "Install npm package"
            });
            let result = tool.execute(params, ctx).await;
            if result.success {
                result.content
            } else {
                result.error.unwrap_or_default()
            }
        }
        "update" => {
            let params = serde_json::json!({
                "command": "npm update",
                "description": "Update npm packages"
            });
            let result = tool.execute(params, ctx).await;
            if result.success {
                result.content
            } else {
                result.error.unwrap_or_default()
            }
        }
        "outdated" => {
            let params = serde_json::json!({
                "command": "npm outdated",
                "description": "Check outdated packages"
            });
            let result = tool.execute(params, ctx).await;
            if result.success {
                result.content
            } else {
                result.error.unwrap_or_default()
            }
        }
        "test" => {
            let params = serde_json::json!({
                "command": "npm test",
                "description": "Run npm tests"
            });
            let result = tool.execute(params, ctx).await;
            if result.success {
                result.content
            } else {
                result.error.unwrap_or_default()
            }
        }
        "run" => {
            let script = parts.get(1).unwrap_or(&"");
            let cmd = if script.is_empty() {
                "npm run".to_string()
            } else {
                format!("npm run {}", script)
            };
            let params = serde_json::json!({
                "command": cmd,
                "description": "Run npm script"
            });
            let result = tool.execute(params, ctx).await;
            if result.success {
                result.content
            } else {
                result.error.unwrap_or_default()
            }
        }
        "" => "Usage: /npm [install|update|outdated|test|run] [args]".to_string(),
        _ => {
            let cmd = args;
            let params = serde_json::json!({
                "command": format!("npm {}", cmd),
                "description": format!("npm {}", cmd)
            });
            let result = tool.execute(params, ctx).await;
            if result.success {
                result.content
            } else {
                result.error.unwrap_or_default()
            }
        }
    }
}
/// Get diagnostic suggestions based on recent failures
pub async fn get_failure_suggestions(app: &TuiApp) -> String {
    let Some(ref engine) = app.streaming_engine else {
        return String::new();
    };

    let tracker_guard = engine.cost_tracker().lock().await;

    // Get top failure reasons
    let mut agg: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    for s in tracker_guard.tool_metrics.values() {
        for (reason, cnt) in &s.failure_reasons {
            *agg.entry(reason.clone()).or_insert(0) += *cnt;
        }
    }

    if agg.is_empty() {
        return String::new();
    }

    let mut suggestions: Vec<String> = vec![];

    for (reason, _count) in agg.iter().take(3) {
        let reason_str: &str = reason.as_str();
        match reason_str {
            "timeout" => {
                suggestions.push(
                    "Timeout: Try /retry to repeat, or /doctor to check tool latency".to_string(),
                );
            }
            "permission" => {
                suggestions.push(
                    "Permission denied: Use /permissions to check rules, or /doctor to diagnose"
                        .to_string(),
                );
            }
            "not_found" => {
                suggestions.push(
                    "Not found: Check file paths with /ls, or verify resource exists".to_string(),
                );
            }
            "hook_blocked" => {
                suggestions.push(
                    "Hook blocked: Check PRE_TOOL_HOOK / POST_TOOL_HOOK env vars in /doctor"
                        .to_string(),
                );
            }
            "dangerous_command" => {
                suggestions.push(
                    "Dangerous command: Use /permissions to allow, or modify the command"
                        .to_string(),
                );
            }
            _ => {
                suggestions.push(format!(
                    "Error '{}': Run /doctor for detailed diagnostics",
                    reason_str
                ));
            }
        }
    }

    drop(tracker_guard);

    if suggestions.is_empty() {
        String::new()
    } else {
        format!("\n\nRecovery suggestions:\n- {}", suggestions.join("\n- "))
    }
}
