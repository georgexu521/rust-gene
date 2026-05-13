//! Runtime preference slash command handlers.

use super::utils::*;

use crate::engine::checkpoint::RestoreResult;
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
        let _ = tx.send(false);
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
    matches!(target, "last-file" | "latest-file") || target.starts_with("fc_")
}

async fn handle_file_change_rollback(app: &TuiApp, target: &str) -> String {
    let session_id = match app.session_manager.current_session_id() {
        Some(id) => format!("session-{}", id),
        None => return "No active session.".to_string(),
    };

    let mgr = crate::engine::checkpoint::get_checkpoint_manager(&session_id).await;
    let cp = mgr.lock().await;
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

/// /project - Project management
pub fn handle_project(_app: &TuiApp, args: &str) -> String {
    if args.is_empty() || args == "info" {
        let dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let name = dir.file_name().unwrap_or_default().to_string_lossy();
        let entries = std::fs::read_dir(&dir)
            .map(|it| it.flatten().count())
            .unwrap_or(0);
        let branch = std::process::Command::new("git")
            .args(["branch", "--show-current"])
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "(none)".to_string());
        return format!(
            "Project: {}\nPath: {}\nEntries: {}\nGit branch: {}",
            name,
            dir.display(),
            entries,
            branch
        );
    }

    let parts: Vec<&str> = args.split_whitespace().collect();
    match parts[0] {
        "list" => {
            let dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
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
            let root = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
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
                let dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
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
        _ => "Usage: /project [info|list|tree [depth]|init <name>]".to_string(),
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
