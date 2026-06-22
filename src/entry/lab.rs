//! LabRun CLI entrypoint wiring.
//!
//! Connects LabRun subcommands to the shared command handlers without mixing them into normal interactive startup.

use crate::engine::streaming::StreamingQueryEngine;
use crate::lab::store::LabStore;
use crate::shell::{self, ShellOptions};
use anyhow::Context;
use std::sync::Arc;

pub async fn run_cli(engine: Arc<StreamingQueryEngine>, no_footer: bool) -> anyhow::Result<()> {
    let project_root = std::env::current_dir().context("failed to resolve current project root")?;
    let store = LabStore::for_project(&project_root);
    store.record_app_lifecycle_startup("lab_cli")?;
    engine.set_lab_context_enabled(true);
    shell::run_shell_with_options(
        engine,
        ShellOptions {
            no_footer,
            lab_mode: true,
        },
    )
    .await
}

pub async fn run_command(command: &str) -> anyhow::Result<()> {
    let project_root = std::env::current_dir().context("failed to resolve current project root")?;
    let store = LabStore::for_project(&project_root);
    store.record_app_lifecycle_startup_for_command("lab_command")?;
    store.claim_latest_active_run_for_current_process()?;
    let command = command
        .trim()
        .strip_prefix("/lab")
        .unwrap_or(command)
        .trim();
    let output = crate::lab::commands::handle_lab_command(&project_root, None, command);
    println!("{output}");
    store.release_current_process_lease_without_pausing()?;
    Ok(())
}

pub async fn run_command_with_components(
    command: &str,
    components: crate::bootstrap::AppComponents,
) -> anyhow::Result<()> {
    let project_root = std::env::current_dir().context("failed to resolve current project root")?;
    let store = LabStore::for_project(&project_root);
    store.record_app_lifecycle_startup_for_command("lab_provider_command")?;
    store.claim_latest_active_run_for_current_process()?;
    components.streaming_engine.set_lab_context_enabled(true);
    let command = command
        .trim()
        .strip_prefix("/lab")
        .unwrap_or(command)
        .trim();
    let context = build_lab_tool_context(&project_root, "lab-provider-command", &components)?;
    let output = crate::lab::commands::handle_lab_command_with_context(
        &project_root,
        None,
        command,
        context,
    )
    .await;
    println!("{output}");
    store.release_current_process_lease_without_pausing()?;
    Ok(())
}

fn build_lab_tool_context(
    project_root: &std::path::Path,
    session_id: &str,
    components: &crate::bootstrap::AppComponents,
) -> anyhow::Result<crate::tools::ToolContext> {
    let session_store_path = project_root
        .join(".priority-agent")
        .join("lab")
        .join("sessions.db");
    let session_store = Arc::new(crate::session_store::SessionStore::open(
        session_store_path,
    )?);
    if session_store.get_session(session_id)?.is_none() {
        session_store.create_session(
            session_id,
            "Lab runtime command session",
            &components.model,
            Some(&project_root.to_string_lossy()),
        )?;
    }
    let mut context = crate::tools::ToolContext::new(project_root, session_id)
        .with_session_store(session_store)
        .with_file_cache(crate::tools::file_cache::GLOBAL_FILE_CACHE.clone())
        .with_llm_provider(components.provider.clone())
        .with_model(components.model.clone())
        .with_lsp_manager(components.lsp_manager.clone())
        .with_worktree_manager(components.worktree_manager.clone())
        .with_task_manager(crate::task_manager::GLOBAL_TASK_MANAGER.clone());
    if let Ok(provider_id) = std::env::var("PRIORITY_AGENT_DEFAULT_PROVIDER") {
        if !provider_id.trim().is_empty() {
            context
                .metadata
                .insert("provider_id".to_string(), provider_id.trim().to_string());
        }
    }
    if let Some(agent_manager) = components.streaming_engine.agent_manager_or_init() {
        context = context.with_agent_manager(agent_manager);
    }
    if let Some(mcp) = components.streaming_engine.mcp_manager() {
        context = context.with_mcp_manager(mcp);
    }
    Ok(context)
}

pub async fn run_daemon_worker(components: crate::bootstrap::AppComponents) -> anyhow::Result<()> {
    let project_root = std::env::current_dir().context("failed to resolve current project root")?;
    let store = LabStore::for_project(&project_root);
    store.record_app_lifecycle_startup("lab_daemon_worker")?;
    let session_store = crate::session_store::SessionStore::open(
        crate::session_store::SessionStore::default_path(),
    )?;
    let recovered = session_store.recover_interrupted_agent_task_states(None)?;
    if recovered > 0 {
        tracing::warn!(
            "Lab daemon startup recovered {} interrupted sub-agent task(s) as paused_restart",
            recovered
        );
    }
    let policy = store
        .load_daemon_state()?
        .ok_or_else(|| anyhow::anyhow!("no Lab daemon policy found"))?;
    if !policy.enabled {
        anyhow::bail!("Lab daemon policy is disabled");
    }
    let run = store
        .claim_latest_active_run_for_current_process()?
        .or(store.latest_run()?)
        .ok_or_else(|| anyhow::anyhow!("no LabRun found for daemon worker"))?;
    store.record_daemon_start_result(Some(&run.lab_run_id), None)?;

    let context = build_lab_tool_context(&project_root, "lab-provider-command", &components)?;

    let result: anyhow::Result<String> = match policy.mode {
        crate::lab::model::LabDaemonMode::Strict => {
            let orchestrator =
                crate::lab::orchestrator::LabOrchestrator::for_project(&project_root);
            let steps = orchestrator
                .run_scheduler_steps_latest_with_context(policy.max_steps, context)
                .await?;
            let final_stage = store
                .latest_run()?
                .map(|run| run.current_stage)
                .unwrap_or_else(|| "unknown".to_string());
            Ok(format!(
                "strict steps={} final_stage={}",
                steps.len(),
                final_stage
            ))
        }
        crate::lab::model::LabDaemonMode::Hybrid => {
            let outcome = crate::lab::draft::run_hybrid_lab_steps_until_boundary(
                &project_root,
                components.provider,
                components.model,
                policy.max_steps,
                &policy.instructions,
                context,
            )
            .await?;
            Ok(format!(
                "hybrid steps={} final_stage={} stop_reason={:?}",
                outcome.steps.len(),
                outcome.final_stage,
                outcome.stop_reason
            ))
        }
        crate::lab::model::LabDaemonMode::HybridCycles => {
            let outcome = crate::lab::draft::run_hybrid_lab_cycles_until_boundary(
                &project_root,
                components.provider,
                components.model,
                policy.max_steps,
                policy.max_steps_per_cycle,
                &policy.instructions,
                context,
            )
            .await?;
            Ok(format!(
                "hybrid-cycles cycles={} final_stage={} final_cycle_count={} stop_reason={:?}",
                outcome.cycles.len(),
                outcome.final_stage,
                outcome.final_cycle_count,
                outcome.stop_reason
            ))
        }
    };

    match result {
        Ok(summary) => {
            store.release_current_process_lease_without_pausing()?;
            println!(
                "Lab daemon worker completed {} mode={:?} {}",
                run.lab_run_id, policy.mode, summary
            );
            Ok(())
        }
        Err(err) => {
            let _ = store.record_daemon_start_result(Some(&run.lab_run_id), Some(&err.to_string()));
            let _ = store.release_current_process_lease_without_pausing();
            Err(err)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static CWD_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn lab_run_command_accepts_lab_prefix_without_provider() {
        let _guard = CWD_LOCK.lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let old_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();

        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(run_command("/lab propose Build scripted Lab command"));

        std::env::set_current_dir(old_dir).unwrap();
        result.unwrap();
        let store = LabStore::for_project(temp.path());
        let proposal = store.latest_proposal().unwrap().unwrap();
        assert_eq!(proposal.user_goal, "Build scripted Lab command");
    }
}
