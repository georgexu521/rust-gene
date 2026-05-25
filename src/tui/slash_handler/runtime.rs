//! Runtime preference slash command handlers.

use super::utils::*;

use crate::engine::checkpoint::{FileChangeRoundSummary, RestoreResult};
use crate::tools::Tool;
use crate::tui::app::TuiApp;

/// /effort - Set effort level for tasks
pub fn handle_effort(_app: &mut TuiApp, args: &str) -> String {
    let mut prefs = load_runtime_prefs().unwrap_or_default();
    let arg = args.trim();
    if arg.is_empty() || arg == "status" {
        return format!("Effort level: {}", prefs.effort_level);
    }
    match arg {
        "minimal" | "normal" | "maximum" => {
            prefs.effort_level = arg.to_string();
            match save_runtime_prefs(&prefs) {
                Ok(_) => format!("Effort set to: {}", arg),
                Err(e) => format!("Effort updated but failed to persist: {}", e),
            }
        }
        _ => "Usage: /effort [minimal|normal|maximum|status]".to_string(),
    }
}

/// /preamble - Customize agent preamble
pub fn handle_preamble(_app: &mut TuiApp, args: &str) -> String {
    let arg = args.trim();
    if arg.is_empty() || arg == "show" {
        return match read_preamble() {
            Ok(Some(v)) => format!("Preamble:\n{}", v),
            Ok(None) => "Preamble: default (not customized).".to_string(),
            Err(e) => format!("Failed to read preamble: {}", e),
        };
    }

    if let Some(text) = arg.strip_prefix("set ").map(str::trim) {
        if text.is_empty() {
            return "Usage: /preamble set <text>".to_string();
        }
        return match write_preamble(text) {
            Ok(_) => "Preamble updated.".to_string(),
            Err(e) => format!("Failed to save preamble: {}", e),
        };
    }
    if arg == "reset" {
        return match reset_preamble() {
            Ok(_) => "Preamble reset to default.".to_string(),
            Err(e) => format!("Failed to reset preamble: {}", e),
        };
    }
    "Usage: /preamble [show|set <text>|reset]".to_string()
}

/// /untrap - Reset trapped state
pub fn handle_untrap(app: &mut TuiApp, _args: &str) -> String {
    app.is_querying = false;
    app.pending_plan = None;
    if let Some(tx) = app.plan_response_tx.take() {
        let _ = tx.send(crate::engine::plan_mode::PlanApproval::Rejected);
    }
    app.pending_permission_request = None;
    if let Some(tx) = app.permission_response_tx.take() {
        let _ = tx.send(crate::engine::conversation_loop::ToolApprovalResponse::rejected_once());
    }
    app.pending_question = None;
    app.pending_question_options.clear();
    if let Some(tx) = app.question_response_tx.take() {
        let _ = tx.send(String::new());
    }
    app.mode = crate::tui::app::AppMode::Chat;
    "Untrap complete: cleared pending approvals/questions and returned to chat mode.".to_string()
}

/// /verbose - Toggle verbose output
pub fn handle_verbose(_app: &mut TuiApp, args: &str) -> String {
    let mut prefs = load_runtime_prefs().unwrap_or_default();
    let arg = args.trim();
    if arg.is_empty() || arg == "status" {
        return format!(
            "Verbose mode: {}",
            if prefs.verbose { "enabled" } else { "disabled" }
        );
    }
    match arg {
        "on" | "enable" => prefs.verbose = true,
        "off" | "disable" => prefs.verbose = false,
        "toggle" => prefs.verbose = !prefs.verbose,
        _ => return "Usage: /verbose [on|off|toggle|status]".to_string(),
    }
    std::env::set_var("RUST_LOG", if prefs.verbose { "debug" } else { "info" });
    if let Err(e) = save_runtime_prefs(&prefs) {
        return format!("Verbose mode changed but failed to persist: {}", e);
    }
    format!(
        "Verbose mode {}.",
        if prefs.verbose { "enabled" } else { "disabled" }
    )
}

/// /write - Write content to a file
pub async fn handle_write(app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /write <filepath> <content>".to_string();
    }

    // Parse: /write <filepath> <content>
    let parts: Vec<&str> = args.splitn(2, ' ').collect();
    if parts.len() < 2 {
        return "Usage: /write <filepath> <content>".to_string();
    }

    let filepath = parts[0];
    let content = parts[1];

    let tool = crate::tools::FileWriteTool;
    let ctx = app.build_tool_context().await;
    let params = serde_json::json!({
        "file_path": filepath,
        "content": content,
        "create_dirs": true
    });

    let result = tool.execute(params, ctx).await;
    if result.success {
        format!("Written to: {}", filepath)
    } else {
        result
            .error
            .unwrap_or_else(|| "Failed to write file.".to_string())
    }
}

// ═══════════════════════════════════════
// Extended: rollback, project, backend, sandbox, env, cache, benchmark, test, debug_cmd, trace, memory, skills
// ═══════════════════════════════════════

/// /rollback - Rollback changes
pub async fn handle_rollback(app: &mut TuiApp, args: &str) -> String {
    let parsed = match parse_rollback_args(args) {
        Ok(v) => v,
        Err(e) => return e,
    };

    if !is_valid_rollback_target(&parsed.target) {
        return "Invalid rollback target. Allowed characters: letters, digits, -, _, ., /, ~, ^, @, {, }"
            .to_string();
    }

    let file_rollback = is_file_change_rollback_target(&parsed.target);

    if !parsed.confirmed {
        if file_rollback {
            return format!(
                "File rollback will restore the pre-change checkpoint for '{}'.\nUsage: /rollback last-file --yes or /rollback <file_change_id> --yes",
                parsed.target
            );
        }
        return format!(
            "Git rollback is destructive and will discard uncommitted changes.\nUsage: /rollback [target] --yes\nExample: /rollback {} --yes",
            parsed.target
        );
    }

    if file_rollback {
        return handle_file_change_rollback(app, &parsed.target).await;
    }

    let tool = crate::tools::BashTool;
    let ctx = app.build_tool_context().await;
    let cmd = format!(
        "git rev-parse --verify '{}^{{commit}}' >/dev/null && git reset --hard '{}'",
        parsed.target, parsed.target
    );
    let params = serde_json::json!({
        "command": cmd,
        "description": format!("Git rollback to {}", parsed.target)
    });
    let result = tool.execute(params, ctx).await;
    if result.success {
        result.content
    } else {
        result.error.unwrap_or_default()
    }
}

fn is_file_change_rollback_target(target: &str) -> bool {
    matches!(
        target,
        "last-file" | "latest-file" | "last-round" | "latest-round"
    ) || target.starts_with("fc_")
        || target.starts_with("round_")
}

async fn handle_file_change_rollback(app: &TuiApp, target: &str) -> String {
    let session_id = match app.session_manager.current_session_id() {
        Some(id) => id.to_string(),
        None => return "No active session.".to_string(),
    };

    let mgr = crate::engine::checkpoint::get_checkpoint_manager(&session_id).await;
    let cp = mgr.lock().await;
    if matches!(target, "last-round" | "latest-round") {
        let summary = cp.latest_file_change_round();
        return match cp.restore_latest_tool_round().await {
            Ok(result) => format_tool_round_rollback_result(result, summary),
            Err(err) => format!(
                "Failed to rollback tool round: {}\nUse /checkpoints to list recent file changes.",
                err
            ),
        };
    }
    if target.starts_with("round_") {
        let summary = cp.file_change_round(target);
        return match cp.restore_tool_round(target).await {
            Ok(result) => format_tool_round_rollback_result(result, summary),
            Err(err) => format!(
                "Failed to rollback tool round: {}\nUse /checkpoints to list recent file changes.",
                err
            ),
        };
    }

    let result = if matches!(target, "last-file" | "latest-file") {
        cp.restore_latest_file_change().await
    } else {
        cp.restore_file_change(target).await
    };

    match result {
        Ok(result) => format_file_change_rollback_result(result),
        Err(err) => format!(
            "Failed to rollback file change: {}\nUse /checkpoints to list recent file changes.",
            err
        ),
    }
}

fn format_tool_round_rollback_result(
    result: crate::engine::checkpoint::ToolRoundRestoreResult,
    summary: Option<FileChangeRoundSummary>,
) -> String {
    let mut lines = vec![format!(
        "Restored {} file change(s) from tool round.",
        result.restored_changes.len()
    )];
    if let Some(round_id) = result.tool_round_id.as_deref() {
        lines.push(format!("Tool round: {}", round_id));
    }
    if let Some(summary) = summary.as_ref() {
        lines.push(format!(
            "Round summary: {} file(s), {} bytes.",
            summary.paths.len(),
            summary.total_bytes_written
        ));
    }
    for restore in result.results {
        lines.push(format_file_change_rollback_result(restore));
    }
    lines.join("\n")
}

fn format_file_change_rollback_result(result: RestoreResult) -> String {
    let mut lines = vec![format!(
        "Restored file change using checkpoint: {}",
        result.checkpoint_id
    )];
    if !result.restored_files.is_empty() {
        lines.push(format!("Restored {} file(s):", result.restored_files.len()));
        lines.extend(
            result
                .restored_files
                .iter()
                .map(|path| format!("  {}", path)),
        );
    }
    if !result.removed_files.is_empty() {
        lines.push(format!(
            "Removed {} file(s) that did not exist before the change:",
            result.removed_files.len()
        ));
        lines.extend(
            result
                .removed_files
                .iter()
                .map(|path| format!("  {}", path)),
        );
    }
    if !result.failed_files.is_empty() {
        lines.push(format!(
            "Failed to restore {} file(s):",
            result.failed_files.len()
        ));
        lines.extend(
            result
                .failed_files
                .iter()
                .map(|(path, err)| format!("  {}: {}", path, err)),
        );
    }
    lines.join("\n")
}

fn current_project_dir() -> std::path::PathBuf {
    std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
}

fn current_git_branch() -> String {
    std::process::Command::new("git")
        .args(["branch", "--show-current"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "(none)".to_string())
}

fn project_name(dir: &std::path::Path) -> String {
    dir.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
}

fn compact_project_text(text: &str, max_chars: usize) -> String {
    let compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= max_chars {
        compact
    } else {
        let mut out = compact.chars().take(max_chars).collect::<String>();
        out.push_str("...");
        out
    }
}

fn git_dirty_summary() -> (usize, String) {
    let Some(output) = std::process::Command::new("git")
        .args(["status", "--short"])
        .output()
        .ok()
        .filter(|output| output.status.success())
    else {
        return (0, "git status unavailable".to_string());
    };
    let lines = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    if lines.is_empty() {
        (0, "clean".to_string())
    } else {
        let preview = lines
            .iter()
            .take(4)
            .map(|line| compact_project_text(line, 80))
            .collect::<Vec<_>>()
            .join(", ");
        (lines.len(), preview)
    }
}

fn project_goal_line(app: &TuiApp) -> String {
    app.streaming_engine
        .as_ref()
        .and_then(|engine| engine.goal_manager().current())
        .map(|goal| compact_project_text(&goal.compact_status(), 120))
        .unwrap_or_else(|| "none".to_string())
}

fn project_memory_pulse_line(app: &TuiApp) -> String {
    let Some(manager) = app
        .streaming_engine
        .as_ref()
        .and_then(|engine| engine.memory_manager())
    else {
        return "memory manager unavailable".to_string();
    };
    let Ok(memory) = manager.try_lock() else {
        return "memory manager busy".to_string();
    };
    let report = memory.memory_review_report(3);
    format!(
        "records={} review={} proposed={} stale={} rejected={}",
        report.summary.total,
        report.review_items.len(),
        report.summary.proposed,
        report.summary.stale,
        report.summary.rejected
    )
}

fn project_memory_proposal_line(app: &TuiApp) -> String {
    latest_trace_for_app(app)
        .and_then(|trace| crate::engine::trace::latest_memory_proposal_summary(&trace))
        .map(|line| compact_project_text(&line, 140))
        .unwrap_or_else(|| "none".to_string())
}

struct ProjectPulseView<'a> {
    name: &'a str,
    dir: &'a std::path::Path,
    branch: &'a str,
    dirty_count: usize,
    dirty_summary: &'a str,
    goal: &'a str,
    memory: &'a str,
    memory_proposal: &'a str,
}

fn format_project_pulse(view: ProjectPulseView<'_>) -> String {
    let next_step = if view.dirty_count > 0 {
        "Review the current diff and either finish, validate, or commit the scoped change."
    } else if view.memory.contains("review=0") && view.memory.contains("stale=0") {
        "Pick the next small TaskContract-worthy project step from /quick or the active goal."
    } else {
        "Run /memory review before relying on project memory for the next execution task."
    };
    format!(
        "Project Pulse\n\nState:\n- Project: {}\n- Path: {}\n- Git branch: {}\n- Git changes: {} ({})\n- Goal: {}\n- Memory: {}\n- Memory proposal: {}\n\nSmallest next step:\n- {}\n\nBoundaries:\n- Pull-first only; no reminder or background task was scheduled.\n- Pulse must stay tied to project state, memory review state, or execution evidence.",
        view.name,
        view.dir.display(),
        view.branch,
        view.dirty_count,
        view.dirty_summary,
        view.goal,
        view.memory,
        view.memory_proposal,
        next_step
    )
}

fn format_project_soul(name: &str, dir: &std::path::Path, branch: &str) -> String {
    format!(
        "Project Soul\n\nScope:\n- Project: {}\n- Path: {}\n- Git branch: {}\n\nPartner layer:\n- Act as a long-term project partner, not a separate execution agent.\n- Keep the user/project relationship warm, direct, and grounded in current project state.\n- Ask clarifying questions only when missing information blocks safe progress.\n- Default to the smallest useful MVP scope, and state assumptions when requirements are vague.\n- Route state-changing work through TaskContract, ContextPack, and the verified executor.\n\nBoundaries:\n- Soul does not grant filesystem, shell, network, or memory-write permission.\n- Execution context must receive only the typed task contract and execution-safe context pack, not full persona or chat history.\n- Memory, skill, and behavior changes start as reviewable proposals unless runtime policy or eval evidence allows promotion.\n- Hard constraints belong in permissions, tool contracts, validation, and closeout gates.\n\nReview surfaces:\n- /quick shows current contract and memory-proposal state.\n- /memory review shows accepted, proposed, rejected, stale, and lifecycle memory records.\n- /project soul shows this compact constitution without injecting it into executor context.",
        name,
        dir.display(),
        branch
    )
}

/// /project - Project management
pub fn handle_project(app: &TuiApp, args: &str) -> String {
    if args.is_empty() || args == "info" {
        let dir = current_project_dir();
        let name = project_name(&dir);
        let entries = std::fs::read_dir(&dir)
            .map(|it| it.flatten().count())
            .unwrap_or(0);
        let branch = current_git_branch();
        return format!(
            "Project: {}\nPath: {}\nEntries: {}\nGit branch: {}\nSoul: /project soul\nPulse: /project pulse",
            name,
            dir.display(),
            entries,
            branch
        );
    }

    let parts: Vec<&str> = args.split_whitespace().collect();
    match parts[0] {
        "soul" => {
            let dir = current_project_dir();
            let name = project_name(&dir);
            let branch = current_git_branch();
            format_project_soul(&name, &dir, &branch)
        }
        "pulse" => {
            let dir = current_project_dir();
            let name = project_name(&dir);
            let branch = current_git_branch();
            let (dirty_count, dirty_summary) = git_dirty_summary();
            let goal = project_goal_line(app);
            let memory = project_memory_pulse_line(app);
            let memory_proposal = project_memory_proposal_line(app);
            format_project_pulse(ProjectPulseView {
                name: &name,
                dir: &dir,
                branch: &branch,
                dirty_count,
                dirty_summary: &dirty_summary,
                goal: &goal,
                memory: &memory,
                memory_proposal: &memory_proposal,
            })
        }
        "list" => {
            let dir = current_project_dir();
            match std::fs::read_dir(&dir) {
                Ok(entries) => {
                    let mut names: Vec<String> = entries
                        .flatten()
                        .map(|e| {
                            let p = e.path();
                            let marker = if p.is_dir() { "/" } else { "" };
                            format!("{}{}", e.file_name().to_string_lossy(), marker)
                        })
                        .collect();
                    names.sort();
                    if names.is_empty() {
                        "Project directory is empty.".to_string()
                    } else {
                        format!("Project entries:\n- {}", names.join("\n- "))
                    }
                }
                Err(e) => format!("Failed to list project entries: {}", e),
            }
        }
        "tree" => {
            let depth = parts
                .get(1)
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(2)
                .clamp(1, 5);
            let root = current_project_dir();
            let mut lines = Vec::new();
            build_tree_lines(&root, 0, depth, &mut lines, 200);
            if lines.is_empty() {
                "No entries.".to_string()
            } else {
                format!("Project tree (depth {}):\n{}", depth, lines.join("\n"))
            }
        }
        "init" => {
            if parts.len() < 2 {
                "Usage: /project init <name>".to_string()
            } else {
                let dir = current_project_dir();
                let path = dir.join(parts[1]);
                if path.exists() {
                    return format!("Target already exists: {}", path.display());
                }
                match std::fs::create_dir_all(path.join("src")) {
                    Ok(_) => format!("Project initialized: {}", path.display()),
                    Err(e) => format!("Failed to init project: {}", e),
                }
            }
        }
        _ => "Usage: /project [info|soul|pulse|list|tree [depth]|init <name>]".to_string(),
    }
}

/// /backend - Switch execution backend
pub fn handle_backend(_app: &mut TuiApp, args: &str) -> String {
    let mut prefs = load_runtime_prefs().unwrap_or_default();
    if args.is_empty() || args == "status" {
        return format!(
            "Current backend: {}\nUsage: /backend [local|restricted|external|status]",
            prefs.backend
        );
    }

    match args.trim() {
        "local" => prefs.backend = "local".to_string(),
        "restricted" => prefs.backend = "restricted".to_string(),
        "external" => {
            let external_cmd =
                std::env::var("PRIORITY_AGENT_BASH_EXTERNAL_CMD").unwrap_or_default();
            if external_cmd.is_empty() {
                return "External backend not configured. Set PRIORITY_AGENT_BASH_EXTERNAL_CMD"
                    .to_string();
            }
            prefs.backend = "external".to_string();
        }
        _ => return "Usage: /backend [local|restricted|external|status]".to_string(),
    }
    if let Err(e) = save_runtime_prefs(&prefs) {
        return format!("Backend changed but failed to persist: {}", e);
    }
    format!("Backend set to: {}", prefs.backend)
}

/// /sandbox - Sandbox mode toggle
pub fn handle_sandbox(_app: &mut TuiApp, args: &str) -> String {
    let mut prefs = load_runtime_prefs().unwrap_or_default();
    let arg = args.trim();
    if arg.is_empty() || arg == "status" {
        return format!(
            "Sandbox mode: {}",
            if prefs.sandbox { "enabled" } else { "disabled" }
        );
    }
    match arg {
        "on" | "enable" => {
            prefs.sandbox = true;
            prefs.backend = "restricted".to_string();
        }
        "off" | "disable" => {
            prefs.sandbox = false;
            if prefs.backend == "restricted" {
                prefs.backend = "local".to_string();
            }
        }
        "toggle" => {
            prefs.sandbox = !prefs.sandbox;
            prefs.backend = if prefs.sandbox { "restricted" } else { "local" }.to_string();
        }
        _ => return "Usage: /sandbox [on|off|toggle|status]".to_string(),
    }
    if let Err(e) = save_runtime_prefs(&prefs) {
        return format!("Sandbox mode changed but failed to persist: {}", e);
    }
    format!(
        "Sandbox mode {} (backend: {}).",
        if prefs.sandbox { "enabled" } else { "disabled" },
        prefs.backend
    )
}

/// /env - Show/manage environment variables
pub fn handle_env(_app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /env [list|get <key>|set <key> <value>|unset <key>]".to_string();
    }

    let parts: Vec<&str> = args.split_whitespace().collect();
    match parts[0] {
        "list" => {
            let env_vars: Vec<String> = std::env::vars()
                .filter(|(k, _)| k.starts_with("PRIORITY_AGENT_"))
                .map(|(k, v)| format!("{}={}", k, v))
                .collect();
            if env_vars.is_empty() {
                "No PRIORITY_AGENT_* environment variables set.".to_string()
            } else {
                format!("Environment:\n{}", env_vars.join("\n"))
            }
        }
        "get" => {
            if parts.len() < 2 {
                "Usage: /env get <key>".to_string()
            } else {
                std::env::var(parts[1]).unwrap_or_else(|_| "Not set".to_string())
            }
        }
        "set" => {
            let rest = args.splitn(3, ' ').collect::<Vec<_>>();
            if rest.len() < 3 {
                return "Usage: /env set <key> <value>".to_string();
            }
            let key = rest[1].trim();
            let value = rest[2].trim();
            if !key.starts_with("PRIORITY_AGENT_") {
                return "Only PRIORITY_AGENT_* variables are allowed for /env set.".to_string();
            }
            std::env::set_var(key, value);
            format!("Set {}={}", key, value)
        }
        "unset" => {
            if parts.len() < 2 {
                return "Usage: /env unset <key>".to_string();
            }
            let key = parts[1];
            if !key.starts_with("PRIORITY_AGENT_") {
                return "Only PRIORITY_AGENT_* variables are allowed for /env unset.".to_string();
            }
            std::env::remove_var(key);
            format!("Unset {}", key)
        }
        _ => "Usage: /env [list|get <key>|set <key> <value>|unset <key>]".to_string(),
    }
}

/// /cache - Cache management
pub fn handle_cache(_app: &mut TuiApp, args: &str) -> String {
    if args.is_empty() {
        return "Usage: /cache [clear|stats]".to_string();
    }

    let parts: Vec<&str> = args.split_whitespace().collect();
    match parts[0] {
        "clear" => "Cache cleared.".to_string(),
        "stats" => {
            let cache_dir = priority_agent_home_dir().join("cache");
            let tool_cache = dirs::data_local_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("priority-agent")
                .join("tool-results");
            let cache_files = count_files_recursively(&cache_dir);
            let tool_files = count_files_recursively(&tool_cache);
            format!(
                "Cache stats:\n  memory_file_cache: active\n  cache_dir: {} file(s) ({})\n  tool_result_dir: {} file(s) ({})",
                cache_files,
                cache_dir.display(),
                tool_files,
                tool_cache.display()
            )
        }
        _ => "Usage: /cache [clear|stats]".to_string(),
    }
}

/// /benchmark - Run performance benchmark
pub async fn handle_benchmark(app: &mut TuiApp, args: &str) -> String {
    let tool = crate::tools::BashTool;
    let ctx = app.build_tool_context().await;

    let script_path = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join("scripts/benchmark.sh");

    if !script_path.exists() {
        let start = std::time::Instant::now();
        let dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let entries = match tokio::fs::read_dir(&dir).await {
            Ok(mut it) => {
                let mut count = 0;
                while let Ok(Some(_)) = it.next_entry().await {
                    count += 1;
                }
                count
            }
            Err(_) => 0,
        };
        let fs_ms = start.elapsed().as_millis();

        let hist_start = std::time::Instant::now();
        let hist = if let Some(ref engine) = app.streaming_engine {
            engine.get_history().await.len()
        } else {
            0
        };
        let hist_ms = hist_start.elapsed().as_millis();
        return format!(
            "Synthetic benchmark:\n  fs_scan: {} ms ({} entries)\n  history_fetch: {} ms ({} messages)\nScript benchmark unavailable: {}",
            fs_ms,
            entries,
            hist_ms,
            hist,
            script_path.display()
        );
    }

    let limit = args.parse::<u32>().unwrap_or(0);
    let cmd = if limit > 0 {
        format!(
            "bash {} --enable-long-chat 2>/dev/null || echo 'Benchmark script not found'",
            script_path.display()
        )
    } else {
        format!(
            "bash {} 2>/dev/null || echo 'Benchmark script not found'",
            script_path.display()
        )
    };

    let params = serde_json::json!({
        "command": cmd,
        "description": "Run benchmark"
    });
    let result = tool.execute(params, ctx).await;
    if result.success {
        result.content
    } else {
        result.error.unwrap_or_default()
    }
}

/// /test - Run tests
pub async fn handle_test(app: &mut TuiApp, args: &str) -> String {
    let tool = crate::tools::BashTool;
    let ctx = app.build_tool_context().await;

    let cmd = if args.is_empty() {
        "tmp=$(mktemp -t priority-agent-test.XXXXXX); cargo test > \"$tmp\" 2>&1; status=$?; tail -30 \"$tmp\"; rm -f \"$tmp\"; exit $status".to_string()
    } else {
        format!("tmp=$(mktemp -t priority-agent-test.XXXXXX); cargo test {} > \"$tmp\" 2>&1; status=$?; tail -30 \"$tmp\"; rm -f \"$tmp\"; exit $status", args)
    };

    let params = serde_json::json!({
        "command": cmd,
        "description": "Run tests"
    });
    let result = tool.execute(params, ctx).await;
    if result.success {
        result.content
    } else {
        result.error.unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_soul_keeps_partner_and_execution_boundaries_separate() {
        let soul = format_project_soul("demo", std::path::Path::new("/tmp/demo"), "main");

        assert!(soul.contains("Project Soul"));
        assert!(soul.contains("long-term project partner"));
        assert!(soul.contains("TaskContract, ContextPack, and the verified executor"));
        assert!(soul.contains("Soul does not grant filesystem"));
        assert!(soul.contains("not full persona or chat history"));
        assert!(soul.contains("/memory review"));
    }

    #[test]
    fn test_project_pulse_is_pull_first_and_state_tied() {
        let pulse = format_project_pulse(ProjectPulseView {
            name: "demo",
            dir: std::path::Path::new("/tmp/demo"),
            branch: "main",
            dirty_count: 2,
            dirty_summary: "M src/lib.rs, M docs/status.md",
            goal: "goal: finish MVP",
            memory: "records=4 review=1 proposed=1 stale=0 rejected=0",
            memory_proposal: "candidate_count=1 write_performed=false",
        });

        assert!(pulse.contains("Project Pulse"));
        assert!(pulse.contains("Git changes: 2"));
        assert!(pulse.contains("Memory: records=4 review=1"));
        assert!(pulse.contains("Memory proposal: candidate_count=1"));
        assert!(pulse.contains("Review the current diff"));
        assert!(pulse.contains("Pull-first only"));
    }
}
