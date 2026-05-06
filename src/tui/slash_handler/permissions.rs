//! Permission slash-command handlers.

use super::utils::*;
use crate::permissions::{match_wildcard, PermissionMode, RuleSource, SourcedRule};
use crate::tui::app::{
    parse_permission_mode, permission_mode_name, persist_permission_rule, TuiApp,
};

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
            format!(
                "Permission mode: {}\nRules: allow={} deny={} ask={}\nProject config: {}\nGlobal config: {}\n\nUsage:\n  /permissions mode <default|auto|auto_low_risk|auto_all|read_only>\n  /permissions rules [tool_name]\n  /permissions explain <tool_name> - explain why a decision was made (with confidence & warnings)\n  /permissions export [path] - export rules to a file\n  /permissions import <path> [project|global] [merge] - import rules (merge to append)\n  /permissions dry-run <allow|deny|ask> <pattern> - test a rule against all registered tools\n  /permissions <allow|deny|ask> <pattern> [project|global]",
                permission_mode_name(mode),
                ctx.rules.always_allow.len(),
                ctx.rules.always_deny.len(),
                ctx.rules.always_ask.len(),
                cwd.join(".priority-agent").join("permissions.toml").display(),
                dirs::home_dir()
                    .unwrap_or_else(|| std::path::PathBuf::from("."))
                    .join(".priority-agent")
                    .join("permissions.toml")
                    .display(),
            )
        }
        Some("explain") => {
            let tool_name = match parts.next() {
                Some(t) if !t.trim().is_empty() => t.trim(),
                _ => return "Usage: /permissions explain <tool_name>".to_string(),
            };
            let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            let ctx = crate::permissions::PermissionContext::new(&cwd);
            let explainable = ctx.explain_decision(tool_name, &serde_json::Value::Null);
            let mut output = explainable.format();

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
                    return "Usage: /permissions import <path> [project|global] [merge]".to_string()
                }
            };
            let scope = match parts.next().map(|s| s.to_ascii_lowercase()) {
                Some(s) if s == "global" => RuleSource::Global,
                Some(s) if s == "project" => RuleSource::Project,
                Some(other) => {
                    return format!("Invalid scope '{}'. Use 'project' or 'global'.", other)
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
                        .to_string()
                }
            };
            let scope = match parts.next().map(|s| s.to_ascii_lowercase()) {
                Some(s) if s == "global" => RuleSource::Global,
                Some(s) if s == "project" => RuleSource::Project,
                Some(other) => {
                    return format!("Invalid scope '{}'. Use 'project' or 'global'.", other)
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
        Some(_) => "Usage: /permissions [mode|rules|allow|deny|ask] ...".to_string(),
    }
}
