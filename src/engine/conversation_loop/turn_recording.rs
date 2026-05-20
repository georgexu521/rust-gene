use crate::engine::hooks::HookRunRecord;
use crate::engine::trace::{TraceCollector, TraceEvent, TurnStatus};
use crate::services::api::ToolCall;
use crate::tools::ToolResult;

pub(super) fn persist_turn_learning_event(
    store: &crate::session_store::SessionStore,
    trace: &crate::engine::trace::TurnTrace,
) -> rusqlite::Result<i64> {
    let intent = trace.events.iter().find_map(|event| match event {
        TraceEvent::IntentRouted { intent, .. } => Some(intent.as_str()),
        _ => None,
    });
    let goal = trace.events.iter().find_map(|event| match event {
        TraceEvent::SessionGoalUpdated { title, .. } => Some(title.as_str()),
        _ => None,
    });
    let tool_count = trace
        .events
        .iter()
        .filter(|event| matches!(event, TraceEvent::ToolCompleted { .. }))
        .count();
    let summary = match (goal, intent) {
        (Some(goal), Some(intent)) => format!("Turn {:?}: {} ({})", trace.status, goal, intent),
        (Some(goal), None) => format!("Turn {:?}: {}", trace.status, goal),
        (None, Some(intent)) => format!("Turn {:?}: intent {}", trace.status, intent),
        (None, None) => format!("Turn {:?}: no routed intent", trace.status),
    };
    let payload = serde_json::json!({
        "trace_id": trace.trace_id,
        "turn_index": trace.turn_index,
        "status": format!("{:?}", trace.status),
        "intent": intent,
        "goal": goal,
        "tool_count": tool_count,
        "event_count": trace.events.len(),
        "duration_ms": trace.duration_ms(),
    });
    let payload = crate::engine::experience_ledger::attach_experience_payload(
        payload,
        crate::engine::experience_ledger::ExperienceRecord::from_turn_trace(trace),
    );
    let confidence = if trace.status == TurnStatus::Completed {
        1.0
    } else {
        0.45
    };
    store.add_learning_event(
        &trace.session_id,
        "turn_outcome",
        "conversation_loop",
        &summary,
        confidence,
        &payload,
    )
}

pub(super) fn record_recovery_plan(
    trace: &TraceCollector,
    plan: &crate::engine::recovery_plan::RecoveryPlan,
) {
    trace.record(TraceEvent::RecoveryPlan {
        plan_id: plan.id.clone(),
        source: plan.source.clone(),
        category: plan.category.clone(),
        action: plan.action.clone(),
        retryable: plan.retryable,
        safe_retry: plan.safe_retry,
        suggested_command: plan.suggested_command.clone(),
        status: format!("{:?}", plan.status),
    });
    trace.record(TraceEvent::RecoveryApplied {
        error: plan.primary_error.clone(),
        action: plan.trace_action(),
    });
}

pub(super) fn record_goal_drift_if_needed(
    trace: &Option<TraceCollector>,
    goal: Option<&crate::engine::session_goal::SessionGoal>,
    tool_call: &ToolCall,
) {
    let (Some(trace), Some(goal)) = (trace, goal) else {
        return;
    };
    let check = crate::engine::goal_drift::GoalDriftDetector::new().check(goal, tool_call);
    if check.should_trace() {
        trace.record(TraceEvent::GoalDriftDetected {
            goal_id: goal.id.clone(),
            tool: tool_call.name.clone(),
            call_id: tool_call.id.clone(),
            level: format!("{:?}", check.level),
            reason: check.reason,
            suggested_action: check.suggested_action,
        });
    }
}

pub(super) fn record_mcp_resource_trace(
    trace: &Option<TraceCollector>,
    tool_call: &ToolCall,
    result: &ToolResult,
) {
    let Some(trace) = trace else {
        return;
    };
    let action = match tool_call.name.as_str() {
        "list_mcp_resources" => "list",
        "read_mcp_resource" => "read",
        _ => return,
    };
    let server = tool_call.arguments["server_name"]
        .as_str()
        .filter(|value| !value.is_empty())
        .unwrap_or("all")
        .to_string();
    let uri = tool_call.arguments["uri"]
        .as_str()
        .filter(|value| !value.is_empty())
        .unwrap_or("*")
        .to_string();

    trace.record(TraceEvent::McpResourceAccessed {
        server: server.clone(),
        uri: uri.clone(),
        action: action.to_string(),
        success: result.success,
        content_chars: result.content.chars().count(),
    });
    trace.record(TraceEvent::RetrievalContextBuilt {
        policy: "Mcp".to_string(),
        sources: vec!["Mcp".to_string()],
        items: usize::from(result.success),
        estimated_tokens: crate::engine::retrieval_context::estimate_tokens(&result.content),
        provenance: vec![format!("mcp.resource:{}:{}", server, uri)],
        conflicts: 0,
    });
}

pub(super) fn record_hook_traces(trace: &Option<TraceCollector>, records: &[HookRunRecord]) {
    let Some(trace) = trace else {
        return;
    };
    for record in records {
        trace.record(TraceEvent::HookCompleted {
            event: record.event.to_string(),
            provider: record.provider.as_str().to_string(),
            hook_name: record.hook_name.clone(),
            call_id: record.tool_call_id.clone(),
            tool: record.tool_name.clone(),
            success: record.success,
            blocked: record.blocked,
            duration_ms: record.duration_ms,
            error: record.error.clone(),
            output_preview: record.output_preview.clone(),
        });
    }
}

pub(super) fn record_web_retrieval_trace(
    trace: &Option<TraceCollector>,
    tool_call: &ToolCall,
    result: &ToolResult,
) {
    let Some(trace) = trace else {
        return;
    };
    let (title, provenance) = match tool_call.name.as_str() {
        "web_search" => (
            "Web search results",
            tool_call.arguments["query"]
                .as_str()
                .map(|query| format!("web.search:{}", query))
                .unwrap_or_else(|| "web.search".to_string()),
        ),
        "web_fetch" => (
            "Web fetched content",
            tool_call.arguments["url"]
                .as_str()
                .map(|url| format!("web.fetch:{}", url))
                .unwrap_or_else(|| "web.fetch".to_string()),
        ),
        _ => return,
    };
    if let Some(ctx) = crate::engine::retrieval_context::RetrievalContext::from_web_result(
        &provenance,
        title,
        &result.content,
        provenance.clone(),
        crate::engine::intent_router::RetrievalPolicy::Web,
    ) {
        trace.record(TraceEvent::RetrievalContextBuilt {
            policy: format!("{:?}", ctx.policy),
            sources: ctx
                .items
                .iter()
                .map(|item| format!("{:?}", item.source))
                .collect(),
            items: ctx.items.len(),
            estimated_tokens: ctx.token_estimate,
            provenance: ctx.provenance_summaries(),
            conflicts: ctx.conflict_count(),
        });
    }
}
