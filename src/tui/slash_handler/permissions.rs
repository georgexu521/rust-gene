//! Permission slash-command handlers.

use super::utils::*;
use crate::engine::action_decision::{ActionDecision, ActionDecisionInput};
use crate::engine::action_review::{ActionReview, ActionReviewInput};
use crate::engine::task_context::AgentTaskStage;
use crate::permissions::{
    match_wildcard, PermissionContext, PermissionMode, PermissionPreset, RuleSource, SourcedRule,
};
use crate::services::api::ToolCall;
use crate::tools::ToolRegistry;
use crate::tui::app::{
    parse_permission_mode, permission_mode_name, persist_permission_rule, TuiApp,
};
use serde_json::Value;
use std::collections::HashSet;
use std::path::Path;

pub fn handle_permissions(app: &mut TuiApp, args: &str) -> String {
    let mut parts = args.split_whitespace();
    let sub = parts.next();

    match sub {
        None => {
            let mode = app
                .streaming_engine
                .as_ref()
                .map(|e| e.permission_mode())
                .unwrap_or(PermissionMode::AutoAll);
            let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            let ctx = crate::permissions::PermissionContext::new(&cwd);
            let mut output = format!(
                "Permission mode: {}\nRules: allow={} deny={} ask={}\nProject config: {}\nGlobal config: {}\n\nUsage:\n  /permissions mode <default|auto|auto_low_risk|auto_all|read_only>\n  /permissions preset <fast-coding|safe-coding|review-only|labrun>\n  /permissions rules [tool_name]\n  /permissions explain <tool_name> [json_params] - explain permission and runtime action review\n  /permissions export [path] - export rules to a file\n  /permissions import <path> [project|global] [merge] - import rules (merge to append)\n  /permissions dry-run <allow|deny|ask> <pattern> - test a rule against all registered tools\n  /permissions <allow|deny|ask> <pattern> [project|global]",
                permission_mode_name(mode),
                ctx.rules.always_allow.len(),
                ctx.rules.always_deny.len(),
                ctx.rules.always_ask.len(),
                cwd.join(".priority-agent")
                    .join("permissions.toml")
                    .display(),
                dirs::home_dir()
                    .unwrap_or_else(|| std::path::PathBuf::from("."))
                    .join(".priority-agent")
                    .join("permissions.toml")
                    .display(),
            );
            if app.pending_permission_request.is_some() {
                output.push_str("\n\n");
                output.push_str(&crate::tui::runtime_panels::render_approval_panel(app));
            }
            output
        }
        Some("explain") => {
            let explain_args = args
                .trim_start()
                .strip_prefix("explain")
                .unwrap_or("")
                .trim();
            let (tool_name, params) = match parse_permission_explain_args(explain_args) {
                Ok(parsed) => parsed,
                Err(message) => return message,
            };
            let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            let ctx = permission_context_for_app(app, &cwd);
            let explainable = ctx.explain_decision(&tool_name, &params);

            // Show the actual match keys used for rule lookup
            let match_keys = crate::permissions::permission_match_keys(&tool_name, &params);
            let mut output = format!("Match keys: [{}]\n\n", match_keys.join(", "));
            output.push_str(&explainable.format());

            let mode = app
                .streaming_engine
                .as_ref()
                .map(|e| e.permission_mode())
                .unwrap_or(PermissionMode::AutoAll);
            output.push_str(&format!("\n\nCurrent mode: {}", permission_mode_name(mode)));
            match mode {
                PermissionMode::AutoAll => {
                    output.push_str(
                        "\n  (developer auto mode: common coding actions auto-run; high-risk actions still ask)",
                    )
                }
                PermissionMode::AutoLowRisk => {
                    output.push_str("\n  (low-risk operations auto-allowed, others follow rules)")
                }
                PermissionMode::ReadOnly => output.push_str("\n  (all write operations denied)"),
                PermissionMode::Once => {
                    output.push_str("\n  (each operation allowed once then denied)")
                }
                _ => {}
            }
            output.push_str("\n\n");
            output.push_str(&format_runtime_action_review(
                app, &tool_name, &params, &ctx, &cwd,
            ));
            output
        }
        Some("export") => {
            let path = parts
                .next()
                .map(|p| {
                    if p == "global" || p == "project" {
                        return None;
                    }
                    Some(std::path::PathBuf::from(p))
                })
                .unwrap_or_else(|| {
                    let cwd =
                        std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
                    Some(cwd.join(".priority-agent").join("permissions_export.toml"))
                });

            let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            let ctx = crate::permissions::PermissionContext::new(&cwd);

            let mut content = String::new();
            content.push_str("# Permission Rules Export\n");
            content.push_str(&format!(
                "# Exported at: {}\n\n",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
            ));

            content.push_str("[allow]\npatterns = [");
            for (i, r) in ctx.rules.always_allow.iter().enumerate() {
                if i > 0 {
                    content.push_str(", ");
                }
                content.push_str(&format!("\"{}\"", r.pattern));
            }
            content.push_str("]\n");

            content.push_str("\n[deny]\npatterns = [");
            for (i, r) in ctx.rules.always_deny.iter().enumerate() {
                if i > 0 {
                    content.push_str(", ");
                }
                content.push_str(&format!("\"{}\"", r.pattern));
            }
            content.push_str("]\n");

            content.push_str("\n[ask]\npatterns = [");
            for (i, r) in ctx.rules.always_ask.iter().enumerate() {
                if i > 0 {
                    content.push_str(", ");
                }
                content.push_str(&format!("\"{}\"", r.pattern));
            }
            content.push_str("]\n");

            if let Some(ref p) = path {
                if let Some(parent) = p.parent() {
                    std::fs::create_dir_all(parent).ok();
                }
                match std::fs::write(p, &content) {
                    Ok(_) => format!("Rules exported to: {}", p.display()),
                    Err(e) => format!("Failed to export: {}", e),
                }
            } else {
                content
            }
        }
        Some("import") => {
            let file_path = match parts.next() {
                Some(p) if !p.trim().is_empty() => p.trim(),
                _ => {
                    return "Usage: /permissions import <path> [project|global] [merge]"
                        .to_string();
                }
            };
            let scope = match parts.next().map(|s| s.to_ascii_lowercase()) {
                Some(s) if s == "global" => RuleSource::Global,
                Some(s) if s == "project" => RuleSource::Project,
                Some(other) => {
                    return format!("Invalid scope '{}'. Use 'project' or 'global'.", other);
                }
                None => RuleSource::Project,
            };
            let merge = match parts.next().map(|s| s.to_ascii_lowercase()) {
                Some(s) if s == "merge" => true,
                Some(other) => return format!("Invalid option '{}'. Use 'merge' or omit.", other),
                None => false,
            };

            let import_content = match std::fs::read_to_string(file_path) {
                Ok(c) => c,
                Err(e) => return format!("Failed to read file: {}", e),
            };

            let target_path = match scope {
                RuleSource::Global => dirs::home_dir()
                    .unwrap_or_else(|| std::path::PathBuf::from("."))
                    .join(".priority-agent")
                    .join("permissions.toml"),
                _ => std::env::current_dir()
                    .unwrap_or_else(|_| std::path::PathBuf::from("."))
                    .join(".priority-agent")
                    .join("permissions.toml"),
            };

            if let Some(parent) = target_path.parent() {
                std::fs::create_dir_all(parent).ok();
            }

            let final_content = if merge && target_path.exists() {
                let existing = std::fs::read_to_string(&target_path).unwrap_or_default();
                match merge_permission_toml(&existing, &import_content) {
                    Ok(merged) => merged,
                    Err(e) => return format!("Failed to merge rules: {}", e),
                }
            } else {
                import_content
            };

            match std::fs::write(&target_path, &final_content) {
                Ok(_) => {
                    let action = if merge { "merged into" } else { "imported to" };
                    format!(
                        "Rules {} '{}' -> {}",
                        action,
                        file_path,
                        target_path.display()
                    )
                }
                Err(e) => format!("Failed to import: {}", e),
            }
        }
        Some("dry-run") => {
            let action = match parts.next() {
                Some(a) if a == "allow" || a == "deny" || a == "ask" => a,
                _ => return "Usage: /permissions dry-run <allow|deny|ask> <pattern>".to_string(),
            };
            let pattern = match parts.next() {
                Some(p) if !p.trim().is_empty() => p.trim(),
                _ => return "Usage: /permissions dry-run <allow|deny|ask> <pattern>".to_string(),
            };

            let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            let ctx = crate::permissions::PermissionContext::new(&cwd);

            let mut test_rules = ctx.rules.clone();
            let test_rule = SourcedRule::new(pattern, RuleSource::User);

            match action {
                "allow" => test_rules.always_allow.push(test_rule),
                "deny" => test_rules.always_deny.push(test_rule),
                "ask" => test_rules.always_ask.push(test_rule),
                _ => unreachable!(),
            }

            let mut lines = vec![
                format!("Dry-run: {} '{}'", action, pattern),
                format!(
                    "Config path: {}/.priority-agent/permissions.toml",
                    cwd.display()
                ),
                "".to_string(),
                "This rule would affect:".to_string(),
            ];

            let registry = crate::tools::ToolRegistry::default_registry();
            let mut affected = 0;
            for tool in &registry.tool_names() {
                if match_wildcard(pattern, tool) {
                    affected += 1;
                    let decision = test_rules.check(tool);
                    let explainable = ctx.explain_decision(tool, &serde_json::Value::Null);
                    let conf = (explainable.confidence * 100.0) as u32;
                    let warn = if explainable.warnings.is_empty() {
                        "".to_string()
                    } else {
                        format!(" ⚠️ {}", explainable.warnings.join(", "))
                    };
                    lines.push(format!(
                        "  {} -> {:?} (confidence: {}%){}",
                        tool, decision, conf, warn
                    ));
                }
            }
            if affected == 0 {
                lines.push("  (no registered tools match this pattern)".to_string());
            } else {
                lines.push(format!("\nTotal affected tools: {}", affected));
            }

            lines.join("\n")
        }
        Some("mode") => {
            if let Some(mode_arg) = parts.next() {
                if let Some(mode) = parse_permission_mode(mode_arg) {
                    if let Some(ref engine) = app.streaming_engine {
                        engine.set_permission_mode(mode);
                        format!("Permission mode set to '{}'.", permission_mode_name(mode))
                    } else {
                        "Cannot set permission mode: engine unavailable.".to_string()
                    }
                } else {
                    "Invalid mode. Use: default | auto | auto_low_risk | auto_all | read_only"
                        .to_string()
                }
            } else {
                let current = app
                    .streaming_engine
                    .as_ref()
                    .map(|e| e.permission_mode())
                    .unwrap_or(PermissionMode::AutoAll);
                format!(
                    "Current mode: {}\nAvailable: default | auto | auto_low_risk | auto_all | read_only",
                    permission_mode_name(current)
                )
            }
        }
        Some("preset") => {
            if let Some(preset_arg) = parts.next() {
                if let Some(preset) = PermissionPreset::parse(preset_arg) {
                    let mode = preset.permission_mode();
                    if let Some(ref engine) = app.streaming_engine {
                        engine.set_permission_mode(mode);
                        format!(
                            "Permission preset '{}' applied.\nMode: {}\nPolicy: {}",
                            preset.label(),
                            permission_mode_name(mode),
                            preset.description()
                        )
                    } else {
                        "Cannot set permission preset: engine unavailable.".to_string()
                    }
                } else {
                    permission_preset_usage()
                }
            } else {
                let current = app
                    .streaming_engine
                    .as_ref()
                    .map(|e| e.permission_mode())
                    .unwrap_or(PermissionMode::AutoAll);
                let presets = PermissionPreset::all()
                    .iter()
                    .map(|preset| {
                        format!(
                            "{} -> {} ({})",
                            preset.label(),
                            permission_mode_name(preset.permission_mode()),
                            preset.description()
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                format!(
                    "Current mode: {}\nAvailable permission presets:\n{}",
                    permission_mode_name(current),
                    presets
                )
            }
        }
        Some("rules") => {
            let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            let ctx = crate::permissions::PermissionContext::new(&cwd);
            if let Some(tool_name) = parts.next() {
                let (decision, details) = ctx.check_with_details(tool_name);
                let mut lines = vec![format!("Tool '{}': {:?}", tool_name, decision)];
                if details.is_empty() {
                    lines.push(
                        "No explicit matching rules (fallback behavior applies).".to_string(),
                    );
                } else {
                    lines.push("Matched rules:".to_string());
                    for d in details {
                        lines.push(format!("- {}", d));
                    }
                }
                lines.join("\n")
            } else {
                let mut lines = vec![
                    format!("Rules overview (cwd={}):", cwd.display()),
                    format!("allow({}):", ctx.rules.always_allow.len()),
                ];
                for r in ctx.rules.always_allow.iter().take(30) {
                    lines.push(format!("- [{:?}] {}", r.source, r.pattern));
                }
                lines.push(format!("deny({}):", ctx.rules.always_deny.len()));
                for r in ctx.rules.always_deny.iter().take(30) {
                    lines.push(format!("- [{:?}] {}", r.source, r.pattern));
                }
                lines.push(format!("ask({}):", ctx.rules.always_ask.len()));
                for r in ctx.rules.always_ask.iter().take(30) {
                    lines.push(format!("- [{:?}] {}", r.source, r.pattern));
                }
                lines.join("\n")
            }
        }
        Some(action @ ("allow" | "deny" | "ask")) => {
            let pattern = match parts.next() {
                Some(p) if !p.trim().is_empty() => p.trim(),
                _ => {
                    return "Usage: /permissions <allow|deny|ask> <pattern> [project|global]"
                        .to_string();
                }
            };
            let scope = match parts.next().map(|s| s.to_ascii_lowercase()) {
                Some(s) if s == "global" => RuleSource::Global,
                Some(s) if s == "project" => RuleSource::Project,
                Some(other) => {
                    return format!("Invalid scope '{}'. Use 'project' or 'global'.", other);
                }
                None => RuleSource::Project,
            };
            let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            match persist_permission_rule(scope, action, pattern, &cwd) {
                Ok(path) => {
                    let path: std::path::PathBuf = path;
                    format!(
                        "Rule saved: {} '{}' ({:?})\nConfig: {}",
                        action,
                        pattern,
                        scope,
                        path.display()
                    )
                }
                Err(e) => format!("Failed to save rule: {}", e),
            }
        }
        Some(_) => "Usage: /permissions [mode|preset|rules|allow|deny|ask] ...".to_string(),
    }
}

fn permission_preset_usage() -> String {
    "Invalid preset. Use: fast-coding | safe-coding | review-only | labrun".to_string()
}

fn parse_permission_explain_args(args: &str) -> Result<(String, Value), String> {
    let trimmed = args.trim();
    if trimmed.is_empty() {
        return Err(permission_explain_usage());
    }

    let mut parts = trimmed.splitn(2, char::is_whitespace);
    let tool_name = parts
        .next()
        .map(str::trim)
        .filter(|tool| !tool.is_empty())
        .ok_or_else(permission_explain_usage)?
        .to_string();
    let raw_params = parts.next().unwrap_or("").trim();
    if raw_params.is_empty() {
        return Ok((tool_name, Value::Object(serde_json::Map::new())));
    }

    let params = serde_json::from_str::<Value>(raw_params).map_err(|err| {
        format!(
            "Invalid JSON params for /permissions explain: {err}\n{}",
            permission_explain_usage()
        )
    })?;
    if !params.is_object() {
        return Err(format!(
            "Invalid JSON params for /permissions explain: expected an object.\n{}",
            permission_explain_usage()
        ));
    }
    Ok((tool_name, params))
}

fn permission_explain_usage() -> String {
    "Usage: /permissions explain <tool_name> [json_params]".to_string()
}

fn permission_context_for_app(app: &TuiApp, cwd: &Path) -> PermissionContext {
    let mut ctx = PermissionContext::new(cwd);
    if let Some(ref engine) = app.streaming_engine {
        ctx.mode = engine.permission_mode();
        let session_rules = engine.session_permission_rules();
        ctx.rules.always_allow.extend(session_rules.always_allow);
        ctx.rules.always_deny.extend(session_rules.always_deny);
        ctx.rules.always_ask.extend(session_rules.always_ask);
    }
    ctx
}

fn format_runtime_action_review(
    app: &TuiApp,
    tool_name: &str,
    params: &Value,
    ctx: &PermissionContext,
    cwd: &Path,
) -> String {
    let owned_registry;
    let registry: &ToolRegistry = match app.streaming_engine.as_ref() {
        Some(engine) => engine.tool_registry().as_ref(),
        None => {
            owned_registry = ToolRegistry::default_registry();
            &owned_registry
        }
    };
    format_runtime_action_review_with_registry(registry, tool_name, params, ctx, cwd)
}

fn format_runtime_action_review_with_registry(
    registry: &ToolRegistry,
    tool_name: &str,
    params: &Value,
    ctx: &PermissionContext,
    cwd: &Path,
) -> String {
    let tool = registry.get(tool_name);
    let canonical_tool_name = tool.map(|tool| tool.name()).unwrap_or(tool_name);
    let tool_call = ToolCall {
        id: "permissions_explain_preview".to_string(),
        name: canonical_tool_name.to_string(),
        arguments: params.clone(),
    };
    let exposed_tool_names = registry
        .tool_names()
        .into_iter()
        .filter(|name| ctx.should_expose_tool(name))
        .map(str::to_string)
        .collect::<HashSet<_>>();
    let stage = explain_stage_for_tool(canonical_tool_name, params);
    let action_decision = ActionDecision::for_tool_call(
        &tool_call,
        ActionDecisionInput {
            task_stage: stage,
            route_workflow: None,
            route_risk: None,
            action_checkpoint_active: false,
            has_changes_before_tools: false,
            no_progress_rounds: 0,
        },
    );
    let review = ActionReview::build(ActionReviewInput {
        tool_call: &tool_call,
        tool,
        exposed_tool_names: &exposed_tool_names,
        scheduled_count: 0,
        max_tool_calls: 4,
        action_decision,
        permission_context: Some(ctx),
        task_state: None,
        working_dir: Some(cwd),
        labrun_context: None,
        tool_allowed_by_context: ctx.should_expose_tool(canonical_tool_name),
        destructive_scope_check: None,
        action_checkpoint_rejection: None,
    });
    format_action_review(&review, stage)
}

fn explain_stage_for_tool(tool_name: &str, params: &Value) -> AgentTaskStage {
    match tool_name {
        "file_read" | "glob" | "grep" | "project_list" | "lsp" | "symbol_query" | "memory_load" => {
            AgentTaskStage::Understand
        }
        "run_tests" | "git_status" | "git_diff" | "diff" => AgentTaskStage::Validate,
        "file_write" | "file_edit" | "file_patch" | "format" => AgentTaskStage::Edit,
        "start_dev_server" => AgentTaskStage::Validate,
        "install_dependencies" => AgentTaskStage::Repair,
        "git" => match params["action"].as_str() {
            Some("status" | "diff" | "log" | "show") => AgentTaskStage::Validate,
            _ => AgentTaskStage::Edit,
        },
        "bash" => {
            let command = params["command"]
                .as_str()
                .or_else(|| params["cmd"].as_str())
                .unwrap_or_default();
            let classification =
                crate::tools::bash_tool::command_classifier::classify_command(command);
            if classification.is_safe_validation() {
                AgentTaskStage::Validate
            } else if classification.mutation_paths.is_empty()
                && classification.mutation_indicators.is_empty()
                && !classification.command_plan.has_write_redirection
                && !matches!(
                    classification.command_kind,
                    crate::tools::bash_tool::command_classifier::CommandKind::Mutation
                        | crate::tools::bash_tool::command_classifier::CommandKind::Dangerous
                )
            {
                AgentTaskStage::Understand
            } else {
                AgentTaskStage::Edit
            }
        }
        _ => AgentTaskStage::Repair,
    }
}

fn format_action_review(review: &ActionReview, stage: AgentTaskStage) -> String {
    let reasons = review
        .reasons
        .iter()
        .map(|reason| reason.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let validation = review
        .tool_contract
        .validation_error
        .as_deref()
        .unwrap_or("ok");
    let permission_decision = review.permission.decision.as_deref().unwrap_or("unknown");
    let risk_level = review.permission.risk_level.as_deref().unwrap_or("unknown");
    let confidence = review
        .permission
        .confidence
        .map(|value| format!("{value:.2}"))
        .unwrap_or_else(|| "unknown".to_string());
    let network_target = review
        .side_effects
        .network
        .target
        .as_deref()
        .unwrap_or("none");
    let suggested_next = review
        .worth
        .suggested_next_action
        .as_deref()
        .unwrap_or("none");

    let mut lines = vec![
        "Runtime Action Review:".to_string(),
        format!("  tool: {}", review.tool),
        format!("  preview_stage: {:?}", stage),
        format!(
            "  decision: {} ({})",
            review.decision.as_str(),
            review.primary_reason.as_str()
        ),
        format!("  reasons: {}", reasons),
        format!(
            "  contract: available={} exposed={} validation={} operation={} permission={} read_only={} destructive={} open_world={} raw_confirmation={}",
            review.tool_contract.available,
            review.tool_contract.exposed,
            validation,
            review
                .tool_contract
                .operation_kind
                .as_deref()
                .unwrap_or("unknown"),
            review
                .tool_contract
                .permission_level
                .as_deref()
                .unwrap_or("unknown"),
            opt_bool_label(review.tool_contract.read_only),
            opt_bool_label(review.tool_contract.destructive),
            opt_bool_label(review.tool_contract.open_world),
            opt_bool_label(review.tool_contract.requires_confirmation),
        ),
        format!(
            "  worth: value={} risk={} uncertainty={} cost={} reversibility={} phase_aligned={} confirmation={}",
            review.worth.value,
            review.worth.risk,
            review.worth.uncertainty_reduction,
            review.worth.cost,
            review.worth.reversibility,
            review.worth.phase_aligned,
            review.worth.requires_confirmation,
        ),
        format!(
            "  permission: allowed_by_context={} confirmation={} decision={} risk={} confidence={}",
            review.permission.allowed_by_context,
            review.permission.requires_confirmation,
            permission_decision,
            risk_level,
            confidence,
        ),
        format!(
            "  side_effect: external={} network={} trusted={} target={} local_workspace={} local_machine={} remote={} paths={}",
            json_label(&review.side_effects.external_side_effect),
            json_label(&review.side_effects.network.class),
            review.side_effects.network.trusted,
            network_target,
            review.side_effects.mutates_local_workspace,
            review.side_effects.mutates_local_machine,
            review.side_effects.remote_side_effect,
            review.side_effects.paths.len(),
        ),
        format!(
            "  checkpoint: required={} status={} approval={} scope={} reason={}",
            review.checkpoint.required,
            review.checkpoint.status,
            review.checkpoint.requires_user_approval,
            review.checkpoint.rollback_scope,
            review.checkpoint.reason,
        ),
        format!("  suggested_next: {}", suggested_next),
        format!("  user_reason: {}", review.user_reason),
        format!("  model_recovery: {}", review.model_recovery),
    ];
    if !review.permission.warnings.is_empty() {
        lines.push(format!(
            "  permission_warnings: {}",
            review.permission.warnings.join(", ")
        ));
    }
    lines.join("\n")
}

fn opt_bool_label(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "true",
        Some(false) => "false",
        None => "unknown",
    }
}

fn json_label<T>(value: &T) -> String
where
    T: serde::Serialize + std::fmt::Debug,
{
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| format!("{:?}", value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_explain_args_defaults_to_empty_object() {
        let (tool, params) = parse_permission_explain_args("bash").unwrap();

        assert_eq!(tool, "bash");
        assert_eq!(params, serde_json::json!({}));
    }

    #[test]
    fn parse_explain_args_accepts_json_params() {
        let (tool, params) =
            parse_permission_explain_args(r#"bash {"command":"cargo test -q"}"#).unwrap();

        assert_eq!(tool, "bash");
        assert_eq!(params["command"], "cargo test -q");
    }

    #[test]
    fn parse_explain_args_rejects_non_object_json() {
        let err = parse_permission_explain_args(r#"bash ["cargo"]"#).unwrap_err();

        assert!(err.contains("expected an object"));
    }

    #[test]
    fn permissions_explain_includes_runtime_action_review() {
        let mut app = TuiApp::new();

        let output = handle_permissions(&mut app, r#"explain bash {"command":"cargo test -q"}"#);

        assert!(output.contains("Runtime Action Review:"));
        assert!(output.contains("decision:"));
        assert!(output.contains("contract:"));
        assert!(output.contains("side_effect:"));
        assert!(output.contains("checkpoint:"));
        assert!(output.contains("model_recovery:"));
    }
}
