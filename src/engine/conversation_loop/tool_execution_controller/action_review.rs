//! Tool execution controller support.
//!
//! Separates execution gates, runtime context, and batch state from the conversation-loop control flow.

use super::super::tool_metadata::merge_tool_result_metadata;
use crate::engine::action_decision::ActionDecision;
use crate::engine::action_review::ActionReview;
use crate::engine::trace::{TraceCollector, TraceEvent};
use crate::services::api::ToolCall;
use crate::tools::ToolResult;

pub(super) fn record_action_decision_if_needed(
    trace: &Option<TraceCollector>,
    tool_call: &ToolCall,
    decision: &ActionDecision,
) {
    if !decision.trace_recommended && !mva_runtime_profile_enabled() {
        return;
    }
    if let Some(trace) = trace {
        trace.record(TraceEvent::ActionDecisionEvaluated {
            tool: tool_call.name.clone(),
            call_id: tool_call.id.clone(),
            stage: format!("{:?}", decision.action.stage),
            value: decision.scores.value,
            risk: decision.scores.risk,
            uncertainty_reduction: decision.scores.uncertainty_reduction,
            cost: decision.scores.cost,
            reversibility: decision.scores.reversibility,
            scope_fit: decision.scores.scope_fit,
            action_score: decision.scores.action_score,
            formula_stage: decision
                .score_computation
                .formula_stage
                .as_str()
                .to_string(),
            formula_version: decision.score_computation.formula_version.clone(),
            phase_aligned: decision.action.phase_aligned,
            mutates_workspace: decision.action.mutates_workspace,
            broad_shell: decision.action.broad_shell,
            modifiers: decision
                .score_computation
                .modifiers
                .iter()
                .filter_map(|modifier| serde_json::to_value(modifier).ok())
                .collect(),
            requires_confirmation: decision.requires_confirmation,
            reason: decision.reason_summary.clone(),
        });
    }
}

fn mva_runtime_profile_enabled() -> bool {
    crate::services::config::runtime_config().is_mva_profile()
}

pub(super) fn attach_action_review_metadata(result: &mut ToolResult, review: &ActionReview) {
    let observed_checkpoint_id = result
        .data
        .as_ref()
        .and_then(|data| data.get("checkpoint"))
        .and_then(|checkpoint| checkpoint.get("id"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let mut metadata = review.metadata();
    if let Some(checkpoint_id) = observed_checkpoint_id {
        if let Some(checkpoint) = metadata
            .get_mut("checkpoint")
            .and_then(serde_json::Value::as_object_mut)
        {
            checkpoint.insert(
                "status".to_string(),
                serde_json::Value::String("required_and_present".to_string()),
            );
            checkpoint.insert(
                "checkpoint_id".to_string(),
                serde_json::Value::String(checkpoint_id),
            );
            checkpoint.insert(
                "observed_result_checkpoint".to_string(),
                serde_json::Value::Bool(true),
            );
        }
    }
    merge_tool_result_metadata(result, "action_review", metadata);
}

pub(super) fn record_action_review(trace: &Option<TraceCollector>, review: &ActionReview) {
    if let Some(trace) = trace {
        let network = serde_json::to_value(review.side_effects.network.class)
            .ok()
            .and_then(|value| value.as_str().map(str::to_string))
            .unwrap_or_else(|| format!("{:?}", review.side_effects.network.class));
        let external_effect = serde_json::to_value(review.side_effects.external_side_effect)
            .ok()
            .and_then(|value| value.as_str().map(str::to_string))
            .unwrap_or_else(|| format!("{:?}", review.side_effects.external_side_effect));
        trace.record(TraceEvent::ActionReviewed {
            tool: review.tool.clone(),
            call_id: review.call_id.clone(),
            decision: review.decision.as_str().to_string(),
            reason: review.primary_reason.as_str().to_string(),
            permission: review.permission.decision.clone(),
            scope_allowed: review.scope.allowed,
            budget_allowed: review.budget.allowed,
            checkpoint: review.checkpoint.status.clone(),
            network,
            external_effect,
            recovery: review.model_recovery.clone(),
        });
    }
}

pub(super) fn record_tool_observation(
    trace: &Option<TraceCollector>,
    tool_call: &ToolCall,
    result: &ToolResult,
) {
    let Some(trace) = trace else {
        return;
    };
    let Some(observation) = result
        .data
        .as_ref()
        .and_then(|data| data.get("tool_observation"))
    else {
        return;
    };
    let files_read = observation
        .get("files_read")
        .and_then(serde_json::Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    let files_changed = observation
        .get("files_changed")
        .and_then(serde_json::Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    let key_findings = observation
        .get("key_findings")
        .and_then(serde_json::Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    let evidence_items = observation
        .get("evidence")
        .and_then(serde_json::Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    let quality_warning_labels = observation
        .get("quality_warnings")
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let quality_warnings = quality_warning_labels.len();
    let context_policy = result
        .data
        .as_ref()
        .and_then(|data| data.get("tool_result_context_policy"));
    trace.record(TraceEvent::ToolObservationRecorded {
        tool: tool_call.name.clone(),
        call_id: tool_call.id.clone(),
        status: observation
            .get("status")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(if result.success { "success" } else { "failed" })
            .to_string(),
        result_kind: observation
            .get("result_kind")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("generic")
            .to_string(),
        model_visibility: context_policy
            .and_then(|policy| policy.get("model_visibility"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
        include_in_next_context: observation
            .get("include_in_next_context")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true),
        store_in_state: observation
            .get("store_in_state")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true),
        key_findings,
        evidence_items,
        failure_type: observation
            .get("failure_type")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        recovery_plan_id: observation
            .get("recovery_plan_id")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        recovery_kind: observation
            .get("recovery_kind")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        raw_result_ref: observation
            .get("raw_result_ref")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        quality_warnings,
        quality_warning_labels,
        files_read,
        files_changed,
        checkpoint_id: observation
            .get("checkpoint_id")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        summary: observation
            .get("summary")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string(),
    });
}
