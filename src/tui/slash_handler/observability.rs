//! Observability and runtime inspection slash command handlers.

use super::utils::*;

use crate::tui::app::TuiApp;
use std::collections::HashSet;

const RECENT_TRACE_LIMIT: usize = 10;

/// /hooks - Show hook configuration status
pub fn handle_hooks(app: &TuiApp) -> String {
    crate::tui::runtime_panels::render_hooks_panel(app)
}

/// /profiling - Show runtime profiling info
pub fn handle_profiling(app: &TuiApp) -> String {
    let mut lines = vec!["Profiling Info:".to_string()];

    // Session info
    if let Some(id) = app.session_manager.current_session_id() {
        lines.push(format!("  Session: {}...", &id[..8.min(id.len())]));
    }
    lines.push(format!("  Messages: {}", app.messages.len()));

    // Engine info
    if app.streaming_engine.is_some() {
        lines.push("  Engine: StreamingQueryEngine".to_string());
    } else {
        lines.push("  Engine: not initialized".to_string());
    }

    // Memory
    if let Some(ref engine) = app.streaming_engine {
        if engine.memory_manager().is_some() {
            lines.push("  Memory: active (use /memory to view)".to_string());
        }
    }

    lines.join("\n")
}

/// /plugins - List discovered plugins, their runtime status, declared slots, and active static UI contributions.
pub fn handle_plugins(app: &TuiApp) -> String {
    if app.plugin_facts.is_empty() {
        return "No plugins discovered.".to_string();
    }

    let mut lines = vec!["Discovered plugins:".to_string()];
    for fact in &app.plugin_facts {
        let status_glyph = match fact.status {
            crate::plugins::PluginRuntimeStatus::Ready => "●",
            crate::plugins::PluginRuntimeStatus::UsableWithWarnings => "◐",
            crate::plugins::PluginRuntimeStatus::Disabled => "○",
            crate::plugins::PluginRuntimeStatus::Blocked => "✗",
        };
        lines.push(format!(
            "{} {} ({}) — {}",
            status_glyph, fact.name, fact.version, fact.diagnostic
        ));
        if !fact.tui_slots.is_empty() {
            let slot_names: Vec<String> = fact
                .tui_slots
                .iter()
                .map(|slot| format!("{:?}", slot))
                .collect();
            lines.push(format!("    declared slots: {}", slot_names.join(", ")));
        }
        let active_slots: Vec<&crate::plugins::PluginUiSlotContent> = app
            .plugin_ui_contributions
            .iter()
            .filter(|c| c.plugin_id == fact.id)
            .collect();
        if !active_slots.is_empty() {
            let active_names: Vec<String> = active_slots
                .iter()
                .map(|c| format!("{:?}", c.slot))
                .collect();
            lines.push(format!(
                "    active static slots: {}",
                active_names.join(", ")
            ));
        }
        let deferred: Vec<String> = fact
            .tui_slots
            .iter()
            .filter(|slot| {
                !matches!(
                    slot,
                    crate::plugins::TuiSlot::SidebarFooter | crate::plugins::TuiSlot::StatusBar
                )
            })
            .map(|slot| format!("{:?}", slot))
            .collect();
        if !deferred.is_empty() {
            lines.push(format!("    deferred slots: {}", deferred.join(", ")));
        }
    }
    lines.join("\n")
}

/// /debug - Toggle debug mode
pub fn handle_debug_cmd(_app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() || args == "on" {
        std::env::set_var("RUST_LOG", "debug");
        "Debug mode enabled (RUST_LOG=debug)".to_string()
    } else if args == "off" {
        std::env::set_var("RUST_LOG", "info");
        "Debug mode disabled (RUST_LOG=info)".to_string()
    } else {
        "Usage: /debug [on|off]".to_string()
    }
}

/// /trace - Runtime trace viewer and tracing controls
pub fn handle_trace(app: &mut TuiApp, args: &str) -> String {
    let mut prefs = load_runtime_prefs().unwrap_or_default();
    let arg = args.trim();
    if arg.is_empty() || arg == "last" {
        if let Some(engine) = &app.streaming_engine {
            if let Some(trace) = engine.trace_store().latest() {
                return crate::engine::trace::format_trace_summary(&trace, 80);
            }
        }
        return match app.session_manager.latest_trace() {
            Ok(Some(trace)) => crate::engine::trace::format_trace_summary(&trace, 80),
            Ok(None) => "No turn trace recorded yet.".to_string(),
            Err(e) => format!("Failed to load latest trace: {}", e),
        };
    } else if arg == "recent" {
        let mut traces = Vec::new();
        if let Some(engine) = &app.streaming_engine {
            traces = engine.trace_store().recent(RECENT_TRACE_LIMIT);
        }
        match app.session_manager.recent_traces(RECENT_TRACE_LIMIT as i64) {
            Ok(persisted) => {
                traces = merge_recent_traces(traces, persisted, RECENT_TRACE_LIMIT);
            }
            Err(e) if traces.is_empty() => return format!("Failed to load recent traces: {}", e),
            _ => {}
        }
        if !traces.is_empty() {
            return format_trace_recent_lines(traces);
        } else {
            return "No recent traces recorded.".to_string();
        }
    } else if arg == "status" {
        return format!(
            "Log tracing: {}\nRuntime traces: {}",
            if prefs.trace { "enabled" } else { "disabled" },
            app.streaming_engine
                .as_ref()
                .map(|engine| engine.trace_store().len().to_string())
                .unwrap_or_else(|| "unavailable".to_string())
        );
    }

    match arg {
        "on" | "enable" => prefs.trace = true,
        "off" | "disable" => prefs.trace = false,
        "toggle" => prefs.trace = !prefs.trace,
        _ => return "Usage: /trace [last|recent|on|off|toggle|status]".to_string(),
    }
    std::env::set_var(
        "RUST_LOG",
        if prefs.trace {
            "trace"
        } else if prefs.verbose {
            "debug"
        } else {
            "info"
        },
    );
    if let Err(e) = save_runtime_prefs(&prefs) {
        return format!("Tracing changed but failed to persist: {}", e);
    }
    format!(
        "Tracing {}.",
        if prefs.trace { "enabled" } else { "disabled" }
    )
}

fn format_trace_recent_lines(traces: Vec<crate::engine::trace::TurnTrace>) -> String {
    let mut lines = vec!["Recent traces:".to_string()];
    for trace in traces {
        lines.push(crate::engine::trace::format_trace_recent_line(&trace));
    }
    lines.join("\n")
}

fn merge_recent_traces(
    memory_traces: Vec<crate::engine::trace::TurnTrace>,
    persisted_traces: Vec<crate::engine::trace::TurnTrace>,
    limit: usize,
) -> Vec<crate::engine::trace::TurnTrace> {
    let mut seen = HashSet::new();
    let mut traces = Vec::new();
    for trace in memory_traces.into_iter().chain(persisted_traces) {
        if seen.insert(trace.trace_id.clone()) {
            traces.push(trace);
        }
    }
    traces.sort_by(|left, right| {
        right
            .turn_index
            .cmp(&left.turn_index)
            .then_with(|| right.started_at.cmp(&left.started_at))
    });
    traces.truncate(limit);
    traces
}

/// /eval - Deterministic behavior evalsets
pub fn handle_eval(_app: &mut TuiApp, args: &str) -> String {
    let eval_dir = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join("evalsets");
    let mut parts = args.split_whitespace();
    let action = parts.next().unwrap_or("list");

    match action {
        "matrix" | "scenarios" => crate::engine::scenario_matrix::format_scenario_matrix(),
        "list" => match crate::engine::evalset::load_evalsets_from_dir(&eval_dir) {
            Ok(sets) if sets.is_empty() => {
                format!("No evalsets found in {}.", eval_dir.display())
            }
            Ok(sets) => {
                let mut lines = vec![format!("Evalsets in {}:", eval_dir.display())];
                for (path, set) in sets {
                    lines.push(format!(
                        "- {} [{} scenarios] {}",
                        set.name,
                        set.scenarios.len(),
                        path.file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or("unknown")
                    ));
                }
                lines.push("Run with /eval run <name|all>.".to_string());
                lines.join("\n")
            }
            Err(e) => format!("Failed to list evalsets: {}", e),
        },
        "run" => {
            let target = parts.next().unwrap_or("all");
            match crate::engine::evalset::run_evalsets_from_dir(&eval_dir, Some(target)) {
                Ok(reports) => crate::engine::evalset::format_reports(&reports),
                Err(e) => format!("Eval run failed: {}", e),
            }
        }
        "json" => {
            let target = parts.next().unwrap_or("all");
            match crate::engine::evalset::run_evalsets_from_dir(&eval_dir, Some(target)) {
                Ok(reports) => crate::engine::evalset::format_reports_json(&reports)
                    .unwrap_or_else(|e| format!("Eval JSON failed: {}", e)),
                Err(e) => format!("Eval run failed: {}", e),
            }
        }
        "record" => {
            let target = parts.next().unwrap_or("all");
            match crate::engine::evalset::run_evalsets_from_dir(&eval_dir, Some(target)) {
                Ok(reports) => {
                    let report_dir = std::env::current_dir()
                        .unwrap_or_else(|_| std::path::PathBuf::from("."))
                        .join("target")
                        .join("eval-reports");
                    match crate::engine::evalset::write_reports_json(&reports, &report_dir, target)
                    {
                        Ok(path) => format!(
                            "{}\n\nRecorded JSON report: {}",
                            crate::engine::evalset::format_reports(&reports),
                            path.display()
                        ),
                        Err(e) => format!("Eval record failed: {}", e),
                    }
                }
                Err(e) => format!("Eval run failed: {}", e),
            }
        }
        "trend" => {
            let limit = parts
                .next()
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(10)
                .clamp(1, 50);
            let report_dir = std::env::current_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."))
                .join("target")
                .join("eval-reports");
            match crate::engine::evalset::load_eval_report_bundles(&report_dir, limit) {
                Ok(entries) => crate::engine::evalset::format_eval_trend(&entries),
                Err(e) => format!("Eval trend failed: {}", e),
            }
        }
        "baseline" => {
            let provider = parts.next();
            let baseline_dir = eval_dir.join("external_baselines");
            match crate::engine::evalset::load_external_baselines_from_dir(&baseline_dir) {
                Ok(baselines) => {
                    crate::engine::evalset::format_external_baseline_comparison(&baselines, provider)
                }
                Err(e) => format!("Eval baseline failed: {}", e),
            }
        }
        "baseline-validate" => {
            let provider = parts.next();
            let baseline_dir = eval_dir.join("external_baselines");
            match crate::engine::evalset::load_external_baselines_from_dir(&baseline_dir) {
                Ok(baselines) => {
                    crate::engine::evalset::format_external_baseline_validation(&baselines, provider)
                }
                Err(e) => format!("Eval baseline validate failed: {}", e),
            }
        }
        "parity" | "baseline-parity" => {
            let provider = parts.next();
            let baseline_dir = eval_dir.join("external_baselines");
            match crate::engine::evalset::load_external_baselines_from_dir(&baseline_dir) {
                Ok(baselines) => {
                    crate::engine::evalset::format_external_parity_report(&baselines, provider)
                }
                Err(e) => format!("Eval parity report failed: {}", e),
            }
        }
        "parity-record" | "baseline-parity-record" => {
            let provider = parts.next();
            let baseline_dir = eval_dir.join("external_baselines");
            let report_dir = std::env::current_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."))
                .join("target")
                .join("eval-reports");
            match crate::engine::evalset::load_external_baselines_from_dir(&baseline_dir)
                .and_then(|baselines| {
                    crate::engine::evalset::write_external_parity_report(
                        &baselines,
                        provider,
                        &report_dir,
                    )
                }) {
                Ok(path) => format!("Parity report recorded: {}", path.display()),
                Err(e) => format!("Eval parity record failed: {}", e),
            }
        }
        "baseline-template" | "baseline-draft" => {
            let provider = parts.next().unwrap_or("external-agent");
            let model = parts.next();
            crate::engine::evalset::format_external_baseline_template(provider, model)
                .unwrap_or_else(|e| format!("Eval baseline template failed: {}", e))
        }
        "baseline-write" => {
            let provider = parts.next().unwrap_or("external-agent");
            let model = parts.next();
            let baseline_dir = eval_dir.join("external_baselines");
            match crate::engine::evalset::write_external_baseline_template(
                &baseline_dir,
                provider,
                model,
            ) {
                Ok(path) => format!("External baseline template written: {}", path.display()),
                Err(e) => format!("Eval baseline write failed: {}", e),
            }
        }
        "baseline-import" => {
            let Some(artifact) = parts.next() else {
                return "Usage: /eval baseline-import <artifact_path> <provider> [model]"
                    .to_string();
            };
            let provider = parts.next().unwrap_or("external-agent");
            let model = parts.next();
            let baseline_dir = eval_dir.join("external_baselines");
            match crate::engine::evalset::write_external_baseline_import(
                artifact,
                &baseline_dir,
                provider,
                model,
            ) {
                Ok(path) => format!("External baseline imported: {}", path.display()),
                Err(e) => format!("Eval baseline import failed: {}", e),
            }
        }
        _ => "Usage: /eval [list|matrix|parity [provider|all]|parity-record [provider|all]|baseline [provider|all]|baseline-validate [provider|all]|baseline-template <provider> [model]|baseline-write <provider> [model]|baseline-import <artifact_path> <provider> [model]|run <name|all>|json <name|all>|record <name|all>|trend [limit]]"
            .to_string(),
    }
}

/// /resource - Show the latest selected resource policy
pub fn handle_resource(app: &mut TuiApp) -> String {
    let Some(trace) = latest_trace_for_app(app) else {
        return "No resource policy recorded yet. Send a message, then run /resource or /trace last."
            .to_string();
    };
    let policy = trace.events.iter().rev().find_map(|event| {
        if let crate::engine::trace::TraceEvent::ResourcePolicySelected {
            latency,
            target_ms,
            cost_ceiling_usd,
            reasoning,
            parallelism_limit,
            max_tool_calls,
            context_budget_tokens,
            allow_fallback_model,
            reason,
        } = event
        {
            Some((
                latency,
                target_ms,
                cost_ceiling_usd,
                reasoning,
                parallelism_limit,
                max_tool_calls,
                context_budget_tokens,
                allow_fallback_model,
                reason,
            ))
        } else {
            None
        }
    });

    let Some((
        latency,
        target_ms,
        cost_ceiling_usd,
        reasoning,
        parallelism_limit,
        max_tool_calls,
        context_budget_tokens,
        allow_fallback_model,
        reason,
    )) = policy
    else {
        return format!(
            "No resource policy in latest trace {}. Use /trace last for the full timeline.",
            &trace.trace_id[..8.min(trace.trace_id.len())]
        );
    };

    format!(
        "Resource Policy\n- trace: {}\n- latency: {} ({} ms)\n- cost ceiling: ${:.2}\n- reasoning: {}\n- parallelism: {}\n- max tool calls: {}\n- context budget: {} tokens\n- fallback model allowed: {}\n- reason: {}\n\nRuntime Inventory\n- skills: {}\n- agent definitions: {}\n- mcp servers: {}\n- evalsets: {}",
        &trace.trace_id[..8.min(trace.trace_id.len())],
        latency,
        target_ms,
        cost_ceiling_usd,
        reasoning,
        parallelism_limit,
        max_tool_calls,
        context_budget_tokens,
        allow_fallback_model,
        reason,
        app.skill_runtime.len(),
        crate::agent::profiles::load_definitions(
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
        )
        .len(),
        app.streaming_engine
            .as_ref()
            .and_then(|engine| engine.mcp_manager())
            .map(|mcp| mcp.health_diagnostics().len())
            .unwrap_or(0),
        crate::engine::evalset::load_evalsets_from_dir(
            std::env::current_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."))
                .join("evalsets")
        )
        .map(|sets| sets.len())
        .unwrap_or(0)
    )
}

/// /memory - Memory management (enhanced)
pub fn handle_memory(_app: &TuiApp) -> String {
    let root = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".priority-agent");
    let mem_path = root.join("memory");
    let project_exists = root.join("MEMORY.md").exists();
    let user_exists = root.join("USER.md").exists();
    let topic_files = count_files_with_ext(&mem_path, "md");
    let agent_files = count_files_with_ext(&mem_path.join("agents"), "json");

    if !project_exists && !user_exists && topic_files == 0 && agent_files == 0 {
        return "No memory entries saved. Start chatting to create memories.".to_string();
    }

    format!(
        "Memory namespaces:\n- project: {}\n- user: {}\n- topic files: {}\n- agent files: {}\n\nUse memory_load with a query to search across namespaces and show conflict hints.\nStored in: {}",
        if project_exists { "MEMORY.md" } else { "none" },
        if user_exists { "USER.md" } else { "none" },
        topic_files,
        agent_files,
        root.display()
    )
}

fn count_files_with_ext(dir: &std::path::Path, ext: &str) -> usize {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return 0,
    };
    entries
        .flatten()
        .filter(|entry| entry.path().extension().and_then(|value| value.to_str()) == Some(ext))
        .count()
}

/// /memory review - 展示记忆审查报告 (proposed/rejected/accepted)
pub async fn memory_review_report(app: &crate::tui::app::TuiApp) -> String {
    let memory_manager = if let Some(ref engine) = app.streaming_engine {
        engine.memory_manager_or_init()
    } else {
        None
    };
    let Some(memory_manager) = memory_manager else {
        return "Memory manager not available.".to_string();
    };
    let mem = memory_manager.lock().await;
    let decisions = mem.memory_decision_counts();
    let flushes = mem.memory_flush_summary();
    let conflicts = mem.memory_conflicts(8);
    let summary = mem.memory_summary();

    let mut lines = vec![
        "Memory Review".to_string(),
        "=============".to_string(),
        String::new(),
        format!(
            "Summary: {} project chars, {} user chars, {} session items, {} frozen",
            summary.project_memory_chars,
            summary.user_memory_chars,
            summary.session_memory_items,
            if summary.has_frozen_snapshot {
                "yes"
            } else {
                "no"
            },
        ),
        String::new(),
        format!(
            "Decisions: {} total (proposed={}, accepted={}, rejected={}, blocked={})",
            decisions.proposed + decisions.accepted + decisions.rejected + decisions.blocked,
            decisions.proposed,
            decisions.accepted,
            decisions.rejected,
            decisions.blocked,
        ),
        String::new(),
        format!(
            "Flushes: {} total (completed={}, pending={}, failed={})",
            flushes.total, flushes.completed, flushes.pending, flushes.failed,
        ),
        String::new(),
        "Conflicts:".to_string(),
    ];
    if conflicts.is_empty() {
        lines.push("  (none)".to_string());
    } else {
        for c in conflicts {
            lines.push(format!("  - {}", c));
        }
    }
    lines.push(String::new());
    lines.push("Accept proposals: /memory-proposals accept <index>".to_string());
    lines.push("Reject proposals: /memory-proposals reject <index>".to_string());
    lines.join("\n")
}

/// /memory files - 列出活跃的记忆文件及其大小
pub fn memory_files_report() -> String {
    let root = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".priority-agent");
    let project_path = root.join("MEMORY.md");
    let user_path = root.join("USER.md");
    let mem_dir = root.join("memory");
    let records_path = mem_dir.join("records.jsonl");

    let mut lines = vec![
        "Memory Files".to_string(),
        "============".to_string(),
        String::new(),
    ];

    // Project memory
    lines.push("Project:".to_string());
    if project_path.exists() {
        let size = std::fs::metadata(&project_path)
            .map(|m| m.len())
            .unwrap_or(0);
        lines.push(format!("  {} ({} bytes)", project_path.display(), size));
    } else {
        lines.push("  (no MEMORY.md)".to_string());
    }

    // User memory
    lines.push("User:".to_string());
    if user_path.exists() {
        let size = std::fs::metadata(&user_path).map(|m| m.len()).unwrap_or(0);
        lines.push(format!("  {} ({} bytes)", user_path.display(), size));
    } else {
        lines.push("  (no USER.md)".to_string());
    }

    // Topic files
    lines.push("Topic files:".to_string());
    if mem_dir.exists() {
        let mut files: Vec<_> = std::fs::read_dir(&mem_dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().extension().and_then(|v| v.to_str()) == Some("md"))
            .collect();
        files.sort_by_key(|e| e.file_name());
        if files.is_empty() {
            lines.push("  (no topic files)".to_string());
        } else {
            for entry in files {
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                lines.push(format!(
                    "  {} ({} bytes)",
                    entry.file_name().to_string_lossy(),
                    size
                ));
            }
        }
    } else {
        lines.push("  (no memory/ directory)".to_string());
    }

    // Records
    if records_path.exists() {
        let size = std::fs::metadata(&records_path)
            .map(|m| m.len())
            .unwrap_or(0);
        lines.push(format!(
            "\nRecords: {} ({} bytes)",
            records_path.display(),
            size
        ));
    }

    lines.join("\n")
}

/// /skills - List available skills
pub fn handle_skills(app: &TuiApp) -> String {
    let names = app
        .skill_runtime
        .names()
        .into_iter()
        .map(|name| format!("/{}", name))
        .collect::<Vec<_>>();
    format!(
        "Skills ({} available):\n{}\n\nInvoke directly with /<skill-name> <task>, or use skill_view for full skill content.",
        app.skill_runtime.len(),
        names.join(", ")
    )
}

// ═══════════════════════════════════════
// Phase 10 Final: profile, theme, shortcuts, quick, feedback
// ═══════════════════════════════════════

/// /profile - Edit user profile
pub fn handle_profile(_app: &mut TuiApp, args: &str) -> String {
    let args = args.trim();
    if args.is_empty() || args == "show" {
        return match load_profile() {
            Ok(map) if map.is_empty() => "Profile is empty.".to_string(),
            Ok(map) => {
                let mut keys: Vec<_> = map.keys().cloned().collect();
                keys.sort();
                let mut lines = vec!["Profile:".to_string()];
                for k in keys {
                    if let Some(v) = map.get(&k) {
                        lines.push(format!("- {} = {}", k, v));
                    }
                }
                lines.join("\n")
            }
            Err(e) => format!("Failed to load profile: {}", e),
        };
    }

    let mut parts = args.splitn(2, ' ');
    let action = parts.next().unwrap_or_default();
    let rest = parts.next().unwrap_or("").trim();
    match action {
        "show" => {
            let Some(key) = sanitize_profile_key(rest) else {
                return "Usage: /profile show <key>".to_string();
            };
            match load_profile() {
                Ok(map) => match map.get(&key) {
                    Some(v) => format!("{} = {}", key, v),
                    None => format!("Profile key '{}' not found.", key),
                },
                Err(e) => format!("Failed to load profile: {}", e),
            }
        }
        "set" => {
            let mut kv = rest.splitn(2, ' ');
            let raw_key = kv.next().unwrap_or_default();
            let value = kv.next().unwrap_or("").trim();
            let Some(key) = sanitize_profile_key(raw_key) else {
                return "Usage: /profile set <key> <value>".to_string();
            };
            if value.is_empty() {
                return "Usage: /profile set <key> <value>".to_string();
            }
            let mut map = match load_profile() {
                Ok(v) => v,
                Err(e) => return format!("Failed to load profile: {}", e),
            };
            map.insert(key.clone(), value.to_string());
            match save_profile(&map) {
                Ok(_) => format!("Profile updated: {} = {}", key, value),
                Err(e) => format!("Failed to save profile: {}", e),
            }
        }
        "unset" => {
            let Some(key) = sanitize_profile_key(rest) else {
                return "Usage: /profile unset <key>".to_string();
            };
            let mut map = match load_profile() {
                Ok(v) => v,
                Err(e) => return format!("Failed to load profile: {}", e),
            };
            if map.remove(&key).is_none() {
                return format!("Profile key '{}' not found.", key);
            }
            match save_profile(&map) {
                Ok(_) => format!("Profile key '{}' removed.", key),
                Err(e) => format!("Failed to save profile: {}", e),
            }
        }
        _ => "Usage: /profile [show [key]|set <key> <value>|unset <key>]".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trace(id: &str, turn_index: u64) -> crate::engine::trace::TurnTrace {
        let mut trace =
            crate::engine::trace::TurnTrace::new("session", turn_index, "inspect workspace");
        trace.trace_id = id.to_string();
        trace.finish(crate::engine::trace::TurnStatus::Completed);
        trace
    }

    #[test]
    fn merge_recent_traces_keeps_persisted_history_after_memory_entries() {
        let traces = merge_recent_traces(
            vec![trace("trace-3", 3), trace("trace-2", 2)],
            vec![trace("trace-3", 3), trace("trace-1", 1)],
            10,
        );

        assert_eq!(traces.len(), 3);
        assert_eq!(
            traces
                .iter()
                .map(|trace| trace.turn_index)
                .collect::<Vec<_>>(),
            vec![3, 2, 1]
        );
    }

    #[test]
    fn merge_recent_traces_applies_recent_limit_after_deduping() {
        let traces = merge_recent_traces(
            vec![trace("trace-4", 4)],
            vec![
                trace("trace-3", 3),
                trace("trace-2", 2),
                trace("trace-1", 1),
            ],
            2,
        );

        assert_eq!(traces.len(), 2);
        assert_eq!(traces[0].turn_index, 4);
        assert_eq!(traces[1].turn_index, 3);
    }

    #[test]
    fn eval_matrix_output_lists_phase_12_scenarios() {
        let output = crate::engine::scenario_matrix::format_scenario_matrix();

        assert!(output.contains("Phase 12 Deterministic Scenario Matrix"));
        assert!(output.contains("file_edit_rewind"));
        assert!(output.contains("mcp_auth_repair"));
        assert!(output.contains("External baseline: ready"));
        assert!(output.contains("/eval baseline-import"));
    }
}
