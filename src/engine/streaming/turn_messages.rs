use crate::services::api::Message;
use crate::tools::ToolRegistry;
use std::path::Path;
use std::sync::Arc;

pub(super) fn session_title_from_user_message(message: &str) -> String {
    let title = message.split_whitespace().collect::<Vec<_>>().join(" ");
    if title.is_empty() {
        return "New Session".to_string();
    }
    let mut out: String = title.chars().take(60).collect();
    if out.chars().count() < title.chars().count() {
        out.push('…');
    }
    out
}

pub(super) fn build_messages_for_turn(
    system_prompt: &str,
    user_msg: &str,
    history: &[Message],
    agent_mode: crate::engine::agent_mode::AgentMode,
    working_dir: Option<&Path>,
) -> Vec<Message> {
    let assembler = if let Some(working_dir) = working_dir {
        crate::engine::prompt_context::PromptContextAssembler::new(system_prompt, working_dir)
    } else {
        crate::engine::prompt_context::PromptContextAssembler::from_current_dir(system_prompt)
    };
    // Use assembly plan to keep task-focus out of the stable prefix (Phase 0 Risk 1)
    let plan = assembler.assembly_plan_for_turn(user_msg, history);
    let mut system_prompt = plan.stable_prefix.content.clone();
    if let Some(mode_context) = agent_mode.runtime_context() {
        system_prompt.push_str("\n\n");
        system_prompt.push_str(mode_context);
    }
    let mut msgs = vec![Message::system(system_prompt)];
    msgs.extend(history.to_vec());
    // Prepend task-focus to the last user message in history (dynamic tail)
    if !plan.task_state.content.is_empty() {
        if let Some(Message::User { content }) =
            msgs.iter_mut().rfind(|m| matches!(m, Message::User { .. }))
        {
            *content = format!(
                "<task-focus>\n{}\n</task-focus>\n\n{}",
                plan.task_state.content.trim(),
                content
            );
        }
    }
    msgs
}

pub(super) async fn reactive_context_retry_messages(
    history: Arc<tokio::sync::Mutex<Vec<Message>>>,
    compressor: Arc<tokio::sync::Mutex<crate::engine::context_compressor::ContextCompressor>>,
    system_prompt: &str,
    user_msg: &str,
    agent_mode: crate::engine::agent_mode::AgentMode,
    working_dir: Option<&Path>,
    session_id: Option<&str>,
) -> Option<Vec<Message>> {
    let compressed = {
        let hist = history.lock().await;
        if hist.is_empty() {
            return None;
        }
        let mut comp = compressor.lock().await;
        comp.set_llm_summary_stable_prefix(system_prompt.to_string());
        comp.compress_async_with_strategy(
            &hist,
            crate::engine::context_collapse::ContextCompactionStrategy::ReactiveCompact,
        )
        .await
    };

    {
        let mut hist = history.lock().await;
        if compressed.len() >= hist.len()
            && crate::engine::context_compressor::estimate_messages_tokens(&compressed)
                >= crate::engine::context_compressor::estimate_messages_tokens(&hist)
        {
            return None;
        }
        *hist = compressed;
        if let Some(session_id) = session_id {
            crate::tools::file_tool::clear_read_files(session_id);
        }
        Some(build_messages_for_turn(
            system_prompt,
            user_msg,
            &hist,
            agent_mode,
            working_dir,
        ))
    }
}

pub(super) fn estimate_registry_tool_schema_tokens(
    registry: &ToolRegistry,
) -> (usize, u64, String) {
    let manifest = crate::engine::cache_stability::registry_tool_schema_manifest(registry);
    (
        manifest.tool_count,
        manifest.estimated_tokens,
        manifest.fingerprint,
    )
}

pub(super) fn route_wants_agent_manager(route: &crate::engine::intent_router::IntentRoute) -> bool {
    route
        .recommended_tools
        .iter()
        .any(|tool| matches!(tool.as_str(), "agent" | "swarm" | "send_message"))
}
