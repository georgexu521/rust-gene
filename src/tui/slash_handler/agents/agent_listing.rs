use crate::tools::Tool;
use crate::tui::app::TuiApp;

pub async fn handle_agents(app: &TuiApp, args: &str) -> String {
    let args = args.trim();
    if !args.is_empty() {
        return handle_agent_worktree_command(app, args).await;
    }

    let working_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let definitions = crate::agent::profiles::load_definitions(&working_dir);
    let profile_line = if definitions.is_empty() {
        "Agent definitions: none".to_string()
    } else {
        format!(
            "Agent definitions ({}): {}",
            definitions.len(),
            definitions
                .iter()
                .map(|definition| definition.summary_line())
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    let recent_artifacts = app
        .session_manager
        .recent_agent_artifacts(8)
        .unwrap_or_default();
    let recent_task_states = app
        .session_manager
        .recent_agent_task_states(8)
        .unwrap_or_default();
    let task_state_lines = format_agent_task_state_lines(&recent_task_states);
    let artifact_lines = if recent_artifacts.is_empty() {
        vec!["Recent artifacts: none for current session".to_string()]
    } else {
        let mut lines = vec![format!("Recent artifacts ({}):", recent_artifacts.len())];
        for artifact in recent_artifacts {
            let preview = artifact
                .output
                .lines()
                .next()
                .unwrap_or("")
                .chars()
                .take(96)
                .collect::<String>();
            lines.push(format!(
                "- {} [{}] profile={} role={} {}",
                artifact.agent_id,
                artifact.status,
                artifact.profile.as_deref().unwrap_or("none"),
                artifact.role,
                if preview.is_empty() {
                    artifact.description
                } else {
                    preview
                }
            ));
        }
        lines
    };

    if let Some(manager) = app
        .streaming_engine
        .as_ref()
        .and_then(|e| e.agent_manager())
    {
        let agents = manager.list_agents().await;
        if agents.is_empty() {
            let mut lines = vec!["No running agents found.".to_string(), profile_line];
            lines.push(String::new());
            lines.extend(task_state_lines);
            lines.push(String::new());
            lines.extend(artifact_lines);
            lines.join("\n")
        } else {
            let mut lines = vec![format!("Agents ({}):", agents.len())];
            for handle in agents.iter().take(30) {
                let status = *handle.status.borrow();
                lines.push(format!(
                    "- {} [{:?}] [{}] {}",
                    handle.id,
                    status,
                    handle.config.role.display_name(),
                    handle.config.name
                ));
            }
            lines.push(String::new());
            lines.push(profile_line);
            lines.push(String::new());
            lines.extend(task_state_lines);
            lines.push(String::new());
            lines.extend(artifact_lines);
            lines.join("\n")
        }
    } else {
        let mut lines = vec![
            "Agent manager unavailable (no engine connected).".to_string(),
            profile_line,
            String::new(),
        ];
        lines.extend(task_state_lines);
        lines.push(String::new());
        lines.extend(artifact_lines);
        lines.join("\n")
    }
}

pub(super) fn format_agent_task_state_lines(
    states: &[crate::session_store::AgentTaskStateRecord],
) -> Vec<String> {
    if states.is_empty() {
        return vec!["Durable task states: none for current session".to_string()];
    }

    let mut lines = vec![format!("Durable task states ({}):", states.len())];
    for state in states {
        let artifact = state
            .result_artifact_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| "none".to_string());
        let cleanup = if state.cleanup_hooks.is_empty() {
            "none".to_string()
        } else {
            state.cleanup_hooks.join(",")
        };
        lines.push(format!(
            "- {} [{}] profile={} role={} artifact={} tools={} permissions={} cleanup={} {}",
            state.agent_id,
            state.status,
            state.profile.as_deref().unwrap_or("none"),
            state.role,
            artifact,
            state.tool_ids_in_progress.len(),
            state.permission_requests.len(),
            cleanup,
            state.description
        ));
        if let Some(worktree) = state.payload.get("isolated_worktree") {
            let path = worktree
                .get("path")
                .and_then(|value| value.as_str())
                .unwrap_or("unknown");
            let branch = worktree
                .get("branch")
                .and_then(|value| value.as_str())
                .unwrap_or("unknown");
            lines.push(format!("  worktree: {} ({})", path, branch));
        }
        if let Some(fork) = state.payload.get("fork_context") {
            let message_count = fork
                .get("message_count")
                .and_then(|value| value.as_u64())
                .unwrap_or(0);
            let placeholder_complete = fork
                .get("placeholder_complete")
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            lines.push(format!(
                "  fork_context: messages={} placeholder_complete={}",
                message_count, placeholder_complete
            ));
        }
    }
    lines
}

async fn handle_agent_worktree_command(app: &TuiApp, args: &str) -> String {
    let mut parts = args.split_whitespace();
    if parts.next() != Some("worktree") {
        return agent_worktree_usage();
    }
    let Some(command) = parts.next() else {
        return agent_worktree_usage();
    };
    let Some(agent_id) = parts.next() else {
        return agent_worktree_usage();
    };
    let flags: Vec<&str> = parts.collect();
    let yes = flags.contains(&"--yes");
    let force = flags.contains(&"--force");
    let delete_branch = flags.contains(&"--delete-branch");
    let cleanup = flags.contains(&"--cleanup");
    let allow_dirty_parent = flags.contains(&"--allow-dirty-parent");
    let action = match command {
        "review" => "agent_review",
        "merge" => {
            if !yes {
                return "Agent worktree merge mutates the target worktree.\nUsage: /agents worktree merge <agent_id> --yes [--cleanup] [--delete-branch] [--force] [--allow-dirty-parent]".to_string();
            }
            "agent_merge"
        }
        "cleanup" => {
            if !yes {
                return "Agent worktree cleanup removes a git worktree.\nUsage: /agents worktree cleanup <agent_id> --yes [--force] [--delete-branch]".to_string();
            }
            "agent_cleanup"
        }
        _ => return agent_worktree_usage(),
    };
    let params = serde_json::json!({
        "action": action,
        "agent_id": agent_id,
        "force": force,
        "delete_branch": delete_branch,
        "cleanup": cleanup,
        "allow_dirty_parent": allow_dirty_parent,
    });
    let tool = crate::tools::WorktreeTool;
    let result = tool.execute(params, app.build_tool_context().await).await;
    if result.success {
        result.content
    } else {
        result
            .error
            .unwrap_or_else(|| "Agent worktree command failed".to_string())
    }
}

fn agent_worktree_usage() -> String {
    "Usage:\n  /agents\n  /agents worktree review <agent_id>\n  /agents worktree merge <agent_id> --yes [--cleanup] [--delete-branch] [--force] [--allow-dirty-parent]\n  /agents worktree cleanup <agent_id> --yes [--force] [--delete-branch]".to_string()
}

/// /agent — product-facing agent profile listing and selection.
pub async fn handle_agent_list(app: &mut TuiApp, args: &str) -> String {
    let parts: Vec<&str> = args.split_whitespace().collect();

    if parts.is_empty() || parts[0] == "list" {
        let profiles = crate::agent::profiles::product_profiles();
        if profiles.is_empty() {
            return "No built-in agent profiles available.".to_string();
        }
        let mut out = String::from("Available agent profiles:\n\n");
        for p in &profiles {
            let risk = match p
                .risk_policy
                .unwrap_or(crate::agent::profiles::AgentRiskPolicy::ReadOnly)
            {
                crate::agent::profiles::AgentRiskPolicy::CodeChange => "write",
                crate::agent::profiles::AgentRiskPolicy::ReadOnly => "read",
                crate::agent::profiles::AgentRiskPolicy::VerifyOnly => "verify",
            };
            let perm = match p
                .permission_mode
                .unwrap_or(crate::agent::profiles::AgentPermissionMode::ReadOnly)
            {
                crate::agent::profiles::AgentPermissionMode::ReadOnly => "read-only",
                crate::agent::profiles::AgentPermissionMode::Bubble => "bubble",
                crate::agent::profiles::AgentPermissionMode::IsolatedWrite => "isolated",
            };
            let tools = if p.allowed_tools.is_empty() {
                "all".to_string()
            } else {
                format!("{} tools", p.allowed_tools.len())
            };
            out.push_str(&format!(
                "  {} — {} (risk={risk}, perm={perm}, tools={tools})\n",
                p.name, p.description,
            ));
        }
        out.push_str("\nUse /agent <name> to see profile detail.\n");
        out.push_str("Use /agent switch <mode> to set the main session mode.\n");
        out.push_str("Use /agent run <profile> <prompt> to spawn a sub-agent.\n");
        return out;
    }

    if parts[0] == "switch" {
        let Some(name) = parts.get(1) else {
            return "Usage: /agent switch <auto|build|plan|explore|review>".to_string();
        };
        let Some(mode) = agent_mode_for_profile(name) else {
            return format!(
                "Agent profile '{}' is run-only or unknown. Switchable modes: auto, build, plan, explore, review.",
                name
            );
        };
        app.set_agent_mode(mode);
        return format!(
            "Agent mode switched to {}. Future turns use the {} runtime surface.",
            app.current_agent_mode_label(),
            app.current_agent_mode_label()
        );
    }

    if parts[0] == "run" {
        return handle_agent_run(app, args).await;
    }

    let name = parts[0];
    let profiles = crate::agent::profiles::product_profiles();
    if let Some(p) = profiles.into_iter().find(|p| p.name == name) {
        let mut out = format!("Agent profile: {}\n\n", p.name);
        out.push_str(&format!("  Description: {}\n", p.description));
        out.push_str(&format!(
            "  Permission: {:?}\n",
            p.permission_mode
                .unwrap_or(crate::agent::profiles::AgentPermissionMode::ReadOnly)
        ));
        out.push_str(&format!(
            "  Context: {:?}\n",
            p.context
                .unwrap_or(crate::agent::profiles::AgentContextMode::InheritedSummary)
        ));
        if !p.allowed_tools.is_empty() {
            out.push_str(&format!("  Tools: {}\n", p.allowed_tools.join(", ")));
        }
        out.push_str(&format!(
            "  Max turns: {}\n",
            p.max_turns
                .map(|t| t.to_string())
                .unwrap_or_else(|| "unlimited".into())
        ));
        out.push_str("\nActions:\n");
        if agent_mode_for_profile(&p.name).is_some() {
            out.push_str(&format!("  /agent switch {}\n", p.name));
        }
        out.push_str(&format!("  /agent run {} <prompt>\n", p.name));
        return out;
    }

    format!(
        "Unknown agent profile '{}'. Run /agent list to see available profiles.",
        name
    )
}

fn agent_mode_for_profile(name: &str) -> Option<crate::engine::agent_mode::AgentMode> {
    match name.trim().to_ascii_lowercase().as_str() {
        "auto" | "default" => Some(crate::engine::agent_mode::AgentMode::Auto),
        "build" | "implement" | "implementer" => Some(crate::engine::agent_mode::AgentMode::Build),
        "plan" | "planner" => Some(crate::engine::agent_mode::AgentMode::Plan),
        "explore" | "explorer" => Some(crate::engine::agent_mode::AgentMode::Explore),
        "review" | "audit" => Some(crate::engine::agent_mode::AgentMode::Review),
        _ => None,
    }
}

async fn handle_agent_run(app: &TuiApp, args: &str) -> String {
    let mut parts = args.trim().splitn(3, char::is_whitespace);
    let _run = parts.next();
    let Some(profile) = parts.next().map(str::trim).filter(|s| !s.is_empty()) else {
        return "Usage: /agent run <profile> <prompt>".to_string();
    };
    let Some(prompt) = parts.next().map(str::trim).filter(|s| !s.is_empty()) else {
        return "Usage: /agent run <profile> <prompt>".to_string();
    };

    let profiles = crate::agent::profiles::product_profiles();
    if !profiles.iter().any(|p| p.name == profile) {
        return format!(
            "Unknown agent profile '{}'. Run /agent list to see available profiles.",
            profile
        );
    }

    let params = serde_json::json!({
        "profile": profile,
        "description": prompt.chars().take(120).collect::<String>(),
        "prompt": prompt,
        "timeout_secs": 300,
    });
    let tool = crate::tools::AgentTool;
    let result = tool.execute(params, app.build_tool_context().await).await;
    if result.success {
        result.content
    } else {
        result
            .error
            .unwrap_or_else(|| "Agent run failed".to_string())
    }
}
