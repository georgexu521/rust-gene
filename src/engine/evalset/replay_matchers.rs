use super::{EvalExpect, EvalReplay};
use crate::engine::trace::{TraceEvent, TurnTrace};

pub(super) fn trace_tool_sequence(trace: &TurnTrace) -> Vec<String> {
    trace
        .events
        .iter()
        .filter_map(|event| match event {
            TraceEvent::ToolStarted { tool, .. } => Some(tool.clone()),
            _ => None,
        })
        .collect()
}

pub(super) fn trace_has_failed_tool(trace: &TurnTrace, expected_tool: &str) -> bool {
    trace.events.iter().any(|event| {
        matches!(
            event,
            TraceEvent::ToolCompleted { tool, success, .. }
                if tool == expected_tool && !success
        )
    })
}

pub(super) fn trace_verification_status(trace: &TurnTrace) -> Option<bool> {
    trace.events.iter().rev().find_map(|event| match event {
        TraceEvent::VerificationCompleted { passed, .. } => Some(*passed),
        _ => None,
    })
}

pub(super) fn trace_last_reflection_status(trace: &TurnTrace) -> Option<String> {
    trace.events.iter().rev().find_map(|event| match event {
        TraceEvent::ReflectionPassCompleted { status, .. } => Some(status.clone()),
        _ => None,
    })
}

pub(super) fn trace_repair_required(trace: &TurnTrace) -> bool {
    trace.events.iter().any(|event| {
        matches!(
            event,
            TraceEvent::ReflectionPassCompleted {
                status,
                unresolved,
                ..
            } if status != "Passed" && *unresolved > 0
        )
    })
}

pub(super) fn trace_last_permission_approved(trace: &TurnTrace) -> Option<bool> {
    trace.events.iter().rev().find_map(|event| match event {
        TraceEvent::PermissionResolved { approved, .. } => Some(*approved),
        _ => None,
    })
}

pub(super) fn trace_last_permission_decision(trace: &TurnTrace) -> Option<String> {
    trace.events.iter().rev().find_map(|event| match event {
        TraceEvent::PermissionResolved { decision, .. } => decision.clone(),
        _ => None,
    })
}

pub(super) fn trace_last_permission_persistence_scope(trace: &TurnTrace) -> Option<String> {
    trace.events.iter().rev().find_map(|event| match event {
        TraceEvent::PermissionResolved {
            persistence_scope, ..
        } => persistence_scope.clone(),
        _ => None,
    })
}

pub(super) fn trace_has_matching_recovery_plan(
    trace: &TurnTrace,
    expected_category: Option<&str>,
    expected_suggested_command: Option<&str>,
    expected_safe_retry: Option<bool>,
) -> bool {
    trace.events.iter().any(|event| {
        let TraceEvent::RecoveryPlan {
            category,
            safe_retry,
            suggested_command,
            ..
        } = event
        else {
            return false;
        };

        expected_category.is_none_or(|expected| category == expected)
            && expected_suggested_command
                .is_none_or(|expected| suggested_command.as_deref() == Some(expected))
            && expected_safe_retry.is_none_or(|expected| *safe_retry == expected)
    })
}

pub(super) fn has_terminal_task_expectation(expect: &EvalExpect) -> bool {
    expect.terminal_task_id.is_some()
        || expect.terminal_task_status.is_some()
        || expect.terminal_task_read_tool.is_some()
        || expect.terminal_task_cancel_tool.is_some()
        || expect.terminal_task_output_path.is_some()
        || expect.backgrounded_tool.is_some()
}

pub(super) fn replay_has_matching_terminal_task(replay: &EvalReplay, expect: &EvalExpect) -> bool {
    replay.terminal_tasks.iter().any(|task| {
        expect
            .terminal_task_id
            .as_deref()
            .is_none_or(|expected| task.id == expected)
            && expect
                .terminal_task_status
                .as_deref()
                .is_none_or(|expected| task.status == expected)
            && expect
                .terminal_task_read_tool
                .as_deref()
                .is_none_or(|expected| task.read_tool.as_deref() == Some(expected))
            && expect
                .terminal_task_cancel_tool
                .as_deref()
                .is_none_or(|expected| task.cancel_tool.as_deref() == Some(expected))
            && expect
                .terminal_task_output_path
                .as_deref()
                .is_none_or(|expected| task.output_path.as_deref() == Some(expected))
            && expect
                .backgrounded_tool
                .as_deref()
                .is_none_or(|expected| task.backgrounded && task.source_tool == expected)
    })
}

pub(super) fn has_run_context_expectation(expect: &EvalExpect) -> bool {
    expect.context_attachment_type.is_some()
        || expect.context_attachment_label.is_some()
        || expect.context_attachment_file.is_some()
        || expect.context_attachment_patch_preview_min_chars.is_some()
}

pub(super) fn replay_has_matching_run_context(replay: &EvalReplay, expect: &EvalExpect) -> bool {
    replay.run_contexts.iter().any(|context| {
        expect
            .context_attachment_type
            .as_deref()
            .is_none_or(|expected| context.context_type == expected)
            && expect
                .context_attachment_label
                .as_deref()
                .is_none_or(|expected| context.label == expected)
            && expect
                .context_attachment_file
                .as_deref()
                .is_none_or(|expected| context.files.iter().any(|file| file == expected))
            && expect
                .context_attachment_patch_preview_min_chars
                .is_none_or(|expected| context.patch_preview.chars().count() >= expected)
    })
}

pub(super) fn has_file_checkpoint_expectation(expect: &EvalExpect) -> bool {
    expect.file_checkpoint_id.is_some()
        || expect.file_change_id.is_some()
        || expect.file_checkpoint_path.is_some()
}

pub(super) fn replay_has_matching_file_change(replay: &EvalReplay, expect: &EvalExpect) -> bool {
    replay.file_changes.iter().any(|change| {
        expect
            .file_checkpoint_id
            .as_deref()
            .is_none_or(|expected| change.checkpoint_id == expected)
            && expect
                .file_change_id
                .as_deref()
                .is_none_or(|expected| change.id == expected)
            && expect
                .file_checkpoint_path
                .as_deref()
                .is_none_or(|expected| change.path == expected)
    })
}

pub(super) fn has_rewind_expectation(expect: &EvalExpect) -> bool {
    expect.rewind_target.is_some()
        || expect.rewind_command.is_some()
        || expect.rewind_checkpoint_id.is_some()
        || expect.rewind_restored_files.is_some()
}

pub(super) fn replay_has_matching_rewind(replay: &EvalReplay, expect: &EvalExpect) -> bool {
    let Some(rewind) = &replay.rewind else {
        return false;
    };

    expect
        .rewind_target
        .as_deref()
        .is_none_or(|expected| rewind.target == expected)
        && expect
            .rewind_command
            .as_deref()
            .is_none_or(|expected| rewind.command == expected)
        && expect
            .rewind_checkpoint_id
            .as_deref()
            .is_none_or(|expected| rewind.checkpoint_id == expected)
        && expect
            .rewind_restored_files
            .is_none_or(|expected| rewind.restored_files.len() == expected)
        && rewind.failed_files.is_empty()
}

pub(super) fn has_context_compaction_expectation(expect: &EvalExpect) -> bool {
    expect.context_boundary_id.is_some()
        || expect.context_compaction_strategy.is_some()
        || expect.context_before_tokens.is_some()
        || expect.context_after_tokens.is_some()
        || expect.context_preserved_tail_count.is_some()
}

pub(super) fn trace_has_matching_context_compaction(
    trace: &TurnTrace,
    expect: &EvalExpect,
) -> bool {
    trace.events.iter().any(|event| {
        let TraceEvent::ContextCompacted {
            before_tokens,
            after_tokens,
            strategy,
            boundary_id,
            preserved_tail_count,
            ..
        } = event
        else {
            return false;
        };

        expect
            .context_boundary_id
            .as_deref()
            .is_none_or(|expected| boundary_id.as_deref() == Some(expected))
            && expect
                .context_compaction_strategy
                .as_deref()
                .is_none_or(|expected| strategy == expected)
            && expect
                .context_before_tokens
                .is_none_or(|expected| *before_tokens == expected)
            && expect
                .context_after_tokens
                .is_none_or(|expected| *after_tokens == expected)
            && expect
                .context_preserved_tail_count
                .is_none_or(|expected| *preserved_tail_count == Some(expected))
    })
}

pub(super) fn has_runtime_diet_expectation(expect: &EvalExpect) -> bool {
    expect.runtime_diet_total_request_tokens.is_some()
        || expect.runtime_diet_remaining_context_tokens.is_some()
        || expect.runtime_diet_route_scoped_tools.is_some()
        || expect.runtime_diet_workflow_context.is_some()
}

pub(super) fn trace_has_matching_runtime_diet(trace: &TurnTrace, expect: &EvalExpect) -> bool {
    trace.events.iter().any(|event| {
        let TraceEvent::RuntimeDietReport {
            total_request_tokens,
            remaining_context_tokens,
            route_scoped_tools,
            workflow_context,
            ..
        } = event
        else {
            return false;
        };

        expect
            .runtime_diet_total_request_tokens
            .is_none_or(|expected| *total_request_tokens == expected)
            && expect
                .runtime_diet_remaining_context_tokens
                .is_none_or(|expected| *remaining_context_tokens == Some(expected))
            && expect
                .runtime_diet_route_scoped_tools
                .is_none_or(|expected| *route_scoped_tools == expected)
            && expect
                .runtime_diet_workflow_context
                .as_deref()
                .is_none_or(|expected| workflow_context == expected)
    })
}

pub(super) fn has_subagent_expectation(expect: &EvalExpect) -> bool {
    expect.subagent_agent_id.is_some()
        || expect.subagent_profile.is_some()
        || expect.subagent_role.is_some()
        || expect.subagent_status.is_some()
        || expect.subagent_context_mode.is_some()
        || expect.subagent_allowed_tools.is_some()
        || expect.isolated_worktree_path.is_some()
        || expect.isolated_worktree_branch.is_some()
        || expect.recursive_fork_guard.is_some()
        || expect.fork_placeholder_complete.is_some()
        || expect.fork_message_count.is_some()
}

pub(super) fn replay_has_matching_subagent(replay: &EvalReplay, expect: &EvalExpect) -> bool {
    replay.subagents.iter().any(|subagent| {
        expect
            .subagent_agent_id
            .as_deref()
            .is_none_or(|expected| subagent.agent_id == expected)
            && expect
                .subagent_profile
                .as_deref()
                .is_none_or(|expected| subagent.profile.as_deref() == Some(expected))
            && expect
                .subagent_role
                .as_deref()
                .is_none_or(|expected| subagent.role == expected)
            && expect
                .subagent_status
                .as_deref()
                .is_none_or(|expected| subagent.status == expected)
            && expect
                .subagent_context_mode
                .as_deref()
                .is_none_or(|expected| subagent.context_mode.as_deref() == Some(expected))
            && expect
                .subagent_allowed_tools
                .is_none_or(|expected| subagent.allowed_tools == expected)
            && expect
                .isolated_worktree_path
                .as_deref()
                .is_none_or(|expected| subagent.worktree_path.as_deref() == Some(expected))
            && expect
                .isolated_worktree_branch
                .as_deref()
                .is_none_or(|expected| subagent.worktree_branch.as_deref() == Some(expected))
            && expect
                .recursive_fork_guard
                .is_none_or(|expected| subagent.recursive_fork_guard == expected)
            && expect
                .fork_placeholder_complete
                .is_none_or(|expected| subagent.placeholder_complete == expected)
            && expect
                .fork_message_count
                .is_none_or(|expected| subagent.fork_message_count == Some(expected))
    })
}

pub(super) fn replay_has_agent_worktree_action(
    replay: &EvalReplay,
    action: &str,
    expected_command: Option<&str>,
    expected_status: Option<&str>,
) -> bool {
    replay.agent_worktree_actions.iter().any(|record| {
        record.action == action
            && expected_command.is_none_or(|expected| record.command.as_deref() == Some(expected))
            && expected_status.is_none_or(|expected| record.status == expected)
    })
}

pub(super) fn replay_has_matching_agent_worktree_metadata(
    replay: &EvalReplay,
    expect: &EvalExpect,
) -> bool {
    let merge_matches = expect
        .agent_worktree_merge_kind
        .as_deref()
        .is_none_or(|expected| {
            replay.agent_worktree_actions.iter().any(|record| {
                record.action == "agent_merge" && record.merge_kind.as_deref() == Some(expected)
            })
        });
    let cleanup_matches = expect
        .agent_worktree_cleanup_deleted_branch
        .is_none_or(|expected| {
            replay
                .agent_worktree_actions
                .iter()
                .any(|record| record.action == "agent_cleanup" && record.delete_branch == expected)
        });

    merge_matches && cleanup_matches
}

pub(super) fn has_mcp_resource_expectation(expect: &EvalExpect) -> bool {
    expect.mcp_resource_server.is_some()
        || expect.mcp_resource_uri.is_some()
        || expect.mcp_resource_action.is_some()
        || expect.mcp_resource_success.is_some()
        || expect.mcp_resource_content_chars.is_some()
}

pub(super) fn trace_has_matching_mcp_resource(trace: &TurnTrace, expect: &EvalExpect) -> bool {
    trace.events.iter().any(|event| {
        let TraceEvent::McpResourceAccessed {
            server,
            uri,
            action,
            success,
            content_chars,
        } = event
        else {
            return false;
        };

        expect
            .mcp_resource_server
            .as_deref()
            .is_none_or(|expected| server == expected)
            && expect
                .mcp_resource_uri
                .as_deref()
                .is_none_or(|expected| uri == expected)
            && expect
                .mcp_resource_action
                .as_deref()
                .is_none_or(|expected| action == expected)
            && expect
                .mcp_resource_success
                .is_none_or(|expected| *success == expected)
            && expect
                .mcp_resource_content_chars
                .is_none_or(|expected| *content_chars == expected)
    })
}

pub(super) fn has_mcp_repair_expectation(expect: &EvalExpect) -> bool {
    expect.mcp_repair_server.is_some()
        || expect.mcp_repair_category.is_some()
        || expect.mcp_repair_command.is_some()
        || expect.mcp_repair_status.is_some()
        || expect.mcp_panel_command.is_some()
}

pub(super) fn replay_has_matching_mcp_repair(replay: &EvalReplay, expect: &EvalExpect) -> bool {
    replay.mcp_repairs.iter().any(|repair| {
        expect
            .mcp_repair_server
            .as_deref()
            .is_none_or(|expected| repair.server == expected)
            && expect
                .mcp_repair_category
                .as_deref()
                .is_none_or(|expected| repair.category == expected)
            && expect
                .mcp_repair_command
                .as_deref()
                .is_none_or(|expected| repair.command == expected)
            && expect
                .mcp_repair_status
                .as_deref()
                .is_none_or(|expected| repair.status == expected)
            && expect
                .mcp_panel_command
                .as_deref()
                .is_none_or(|expected| repair.panel_command == expected)
    })
}
