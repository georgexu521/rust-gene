use super::{preview, TraceEvent, TurnTrace};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ControlLoopDiagnostic {
    pub phases: Vec<ControlLoopPhaseDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ControlLoopPhaseDiagnostic {
    pub phase: String,
    pub events: usize,
    pub latest_label: Option<String>,
    pub latest_summary: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActionReviewTraceSummary {
    pub total: usize,
    pub allowed: usize,
    pub ask_user: usize,
    pub denied: usize,
    pub revised: usize,
    pub checkpoint_required: usize,
    pub latest_tool: Option<String>,
    pub latest_decision: Option<String>,
    pub latest_reason: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScoringTraceSummary {
    pub latest_action: Option<String>,
    pub latest_candidate: Option<String>,
    pub latest_memory_recall: Option<String>,
    pub latest_memory_write: Option<String>,
    pub latest_memory_keep: Option<String>,
    pub latest_workflow: Option<String>,
}

impl ControlLoopDiagnostic {
    pub fn compact_summary(&self) -> String {
        self.phases
            .iter()
            .map(|phase| {
                let latest = phase
                    .latest_label
                    .as_deref()
                    .filter(|label| !label.is_empty())
                    .unwrap_or("none");
                format!("{}={} latest={}", phase.phase, phase.events, latest)
            })
            .collect::<Vec<_>>()
            .join(" -> ")
    }
}

impl ActionReviewTraceSummary {
    pub fn compact_summary(&self) -> String {
        let latest = match (
            self.latest_tool.as_deref(),
            self.latest_decision.as_deref(),
            self.latest_reason.as_deref(),
        ) {
            (Some(tool), Some(decision), Some(reason)) => {
                format!("{tool}:{decision}/{reason}")
            }
            _ => "none".to_string(),
        };
        format!(
            "total={} allow={} ask_user={} denied={} revised={} checkpoint_required={} latest={}",
            self.total,
            self.allowed,
            self.ask_user,
            self.denied,
            self.revised,
            self.checkpoint_required,
            latest
        )
    }
}

impl ScoringTraceSummary {
    pub fn compact_summary(&self) -> String {
        let mut parts = Vec::new();
        if let Some(action) = self.latest_action.as_ref() {
            parts.push(format!("action={action}"));
        }
        if let Some(candidate) = self.latest_candidate.as_ref() {
            parts.push(format!("candidate={candidate}"));
        }
        if let Some(recall) = self.latest_memory_recall.as_ref() {
            parts.push(format!("memory_recall={recall}"));
        }
        if let Some(write) = self.latest_memory_write.as_ref() {
            parts.push(format!("memory_write={write}"));
        }
        if let Some(keep) = self.latest_memory_keep.as_ref() {
            parts.push(format!("memory_keep={keep}"));
        }
        if let Some(workflow) = self.latest_workflow.as_ref() {
            parts.push(format!("workflow={workflow}"));
        }
        if parts.is_empty() {
            "none".to_string()
        } else {
            parts.join(" | ")
        }
    }
}

pub fn control_loop_diagnostic(trace: &TurnTrace) -> ControlLoopDiagnostic {
    let mut phases = CONTROL_LOOP_PHASES
        .iter()
        .map(|phase| ControlLoopPhaseDiagnostic {
            phase: (*phase).to_string(),
            events: 0,
            latest_label: None,
            latest_summary: None,
        })
        .collect::<Vec<_>>();

    for event in &trace.events {
        let Some(phase) = control_loop_phase_for_event(event) else {
            continue;
        };
        let Some(slot) = phases.iter_mut().find(|item| item.phase == phase) else {
            continue;
        };
        slot.events += 1;
        slot.latest_label = Some(event.label().to_string());
        slot.latest_summary = Some(event.summary());
    }

    ControlLoopDiagnostic { phases }
}

pub fn action_review_trace_summary(trace: &TurnTrace) -> Option<ActionReviewTraceSummary> {
    let mut summary = ActionReviewTraceSummary::default();
    for event in &trace.events {
        let TraceEvent::ActionReviewed {
            tool,
            decision,
            reason,
            checkpoint,
            ..
        } = event
        else {
            continue;
        };
        summary.total += 1;
        match decision.as_str() {
            "allow" => summary.allowed += 1,
            "ask_user" => summary.ask_user += 1,
            "deny" => summary.denied += 1,
            "revise" => summary.revised += 1,
            _ => {}
        }
        if action_review_checkpoint_required(checkpoint, reason) {
            summary.checkpoint_required += 1;
        }
        summary.latest_tool = Some(tool.clone());
        summary.latest_decision = Some(decision.clone());
        summary.latest_reason = Some(reason.clone());
    }
    (summary.total > 0).then_some(summary)
}

pub fn scoring_trace_summary(trace: &TurnTrace) -> Option<ScoringTraceSummary> {
    let mut summary = ScoringTraceSummary::default();
    for event in &trace.events {
        match event {
            TraceEvent::ActionDecisionEvaluated {
                tool,
                action_score,
                value,
                risk,
                uncertainty_reduction,
                scope_fit,
                ..
            } => {
                summary.latest_action = Some(format!(
                    "{} score={} value={} risk={} uncertainty={} scope_fit={}",
                    tool, action_score, value, risk, uncertainty_reduction, scope_fit
                ));
            }
            TraceEvent::CandidateActionsEvaluated {
                mode,
                candidate_count,
                selected_tool,
                selected_runtime_score,
                selected_model_score,
                runtime_selected_differs_from_model_order,
                ..
            } => {
                summary.latest_candidate = Some(format!(
                    "mode={} candidates={} selected={} runtime_score={} model_score={} differs={}",
                    mode,
                    candidate_count,
                    selected_tool.as_deref().unwrap_or("none"),
                    selected_runtime_score
                        .map(|score| score.to_string())
                        .unwrap_or_else(|| "n/a".to_string()),
                    selected_model_score
                        .map(|score| score.to_string())
                        .unwrap_or_else(|| "n/a".to_string()),
                    runtime_selected_differs_from_model_order
                ));
            }
            TraceEvent::MemoryRecallScored {
                item_count,
                injected,
                available,
                omitted,
                conflict_capped,
                top_score,
                budget_exhausted,
                ..
            } => {
                summary.latest_memory_recall = Some(format!(
                    "items={} injected={} available={} omitted={} conflict_capped={} top={:.2} budget_exhausted={}",
                    item_count,
                    injected,
                    available,
                    omitted,
                    conflict_capped,
                    top_score,
                    budget_exhausted
                ));
            }
            TraceEvent::MemoryWriteScored {
                kind,
                status,
                score,
                threshold,
                explicit,
                duplication,
                ..
            } => {
                summary.latest_memory_write = Some(format!(
                    "kind={} status={} score={:.2} threshold={:.2} explicit={} duplication={:.2}",
                    kind, status, score, threshold, explicit, duplication
                ));
            }
            TraceEvent::MemoryKeepScored {
                record_id,
                kind,
                action,
                score,
                contradiction_risk,
                redundancy,
                ..
            } => {
                summary.latest_memory_keep = Some(format!(
                    "{} kind={} action={} score={:.2} contradiction={:.2} redundancy={:.2}",
                    preview(record_id),
                    kind,
                    action,
                    score,
                    contradiction_risk,
                    redundancy
                ));
            }
            TraceEvent::WorkflowPlanProgress {
                active_step,
                top_priority,
                top_importance_score,
                top_weight_share,
                weight_source,
                reweighted,
                ..
            } => {
                summary.latest_workflow = Some(format!(
                    "step={} importance={} share={} source={} reweighted={}",
                    active_step
                        .as_deref()
                        .or(top_priority.as_deref())
                        .map(preview)
                        .unwrap_or_else(|| "none".to_string()),
                    top_importance_score
                        .map(|score| format!("{score:.2}"))
                        .unwrap_or_else(|| "n/a".to_string()),
                    top_weight_share
                        .map(|share| format!("{share:.2}"))
                        .unwrap_or_else(|| "n/a".to_string()),
                    weight_source.as_deref().unwrap_or("unknown"),
                    reweighted
                ));
            }
            _ => {}
        }
    }
    (summary.latest_action.is_some()
        || summary.latest_candidate.is_some()
        || summary.latest_memory_recall.is_some()
        || summary.latest_memory_write.is_some()
        || summary.latest_memory_keep.is_some()
        || summary.latest_workflow.is_some())
    .then_some(summary)
}

fn action_review_checkpoint_required(checkpoint: &str, reason: &str) -> bool {
    reason == "checkpoint_required"
        || matches!(
            checkpoint,
            "required_and_present" | "required_but_missing" | "unavailable"
        )
}

const CONTROL_LOOP_PHASES: [&str; 7] = [
    "context",
    "decision",
    "permission",
    "tool_execution",
    "state_update",
    "verification",
    "closeout",
];

fn control_loop_phase_for_event(event: &TraceEvent) -> Option<&'static str> {
    match event {
        TraceEvent::UserPromptSubmitted { .. }
        | TraceEvent::IntentRouted { .. }
        | TraceEvent::RouteCandidateEvaluated { .. }
        | TraceEvent::RouteCompetitionSummary { .. }
        | TraceEvent::ContextTokenBreakdown { .. }
        | TraceEvent::ResourcePolicySelected { .. }
        | TraceEvent::TaskContextBuilt { .. }
        | TraceEvent::TaskContractMaterialized { .. }
        | TraceEvent::ContextPackMaterialized { .. }
        | TraceEvent::MemorySnapshotInjected { .. }
        | TraceEvent::MemoryPrefetch { .. }
        | TraceEvent::ActiveMemoryEvaluated { .. }
        | TraceEvent::SelfEvolutionGuidanceInjected { .. }
        | TraceEvent::RetrievalContextBuilt { .. }
        | TraceEvent::ContextZonesMaterialized { .. }
        | TraceEvent::CacheStabilitySnapshot { .. }
        | TraceEvent::PromptCacheUsageRecorded { .. }
        | TraceEvent::MemoryBoundaryEvaluated { .. }
        | TraceEvent::MemorySynced { .. }
        | TraceEvent::ContextCompacted { .. }
        | TraceEvent::RuntimeDietReport { .. }
        | TraceEvent::ApiRequestStarted { .. }
        | TraceEvent::ProviderMessageSequenceNormalized { .. }
        | TraceEvent::ProviderToolCallRepairApplied { .. }
        | TraceEvent::StreamingToolExecutionShadow { .. }
        | TraceEvent::ProviderRequestStarted { .. }
        | TraceEvent::ProviderRequestRetrying { .. }
        | TraceEvent::ProviderRequestSlowWarning { .. }
        | TraceEvent::ProviderRequestCompleted { .. }
        | TraceEvent::ProviderRequestTimeout { .. }
        | TraceEvent::ProviderRequestCancelled { .. } => Some("context"),
        TraceEvent::ImplementationIntentRecorded { .. }
        | TraceEvent::WorkflowJudgmentCompleted { .. }
        | TraceEvent::WorkflowPlanProgress { .. }
        | TraceEvent::WorkflowLearningAdjusted { .. }
        | TraceEvent::WorkflowContractActivation { .. }
        | TraceEvent::RiskSignalAssessed { .. }
        | TraceEvent::AdaptiveWorkflowTriggered { .. }
        | TraceEvent::ActionDecisionEvaluated { .. }
        | TraceEvent::CandidateActionsEvaluated { .. }
        | TraceEvent::ActionReviewed { .. }
        | TraceEvent::MemoryRecallScored { .. }
        | TraceEvent::MemoryWriteScored { .. }
        | TraceEvent::MemoryKeepScored { .. }
        | TraceEvent::WorkflowRouted { .. } => Some("decision"),
        TraceEvent::GoalDriftDetected { .. }
        | TraceEvent::DestructiveScopeChecked { .. }
        | TraceEvent::PermissionRequested { .. }
        | TraceEvent::PermissionResolved { .. } => Some("permission"),
        TraceEvent::ApiRequestCompleted { .. }
        | TraceEvent::ToolStarted { .. }
        | TraceEvent::ToolCompleted { .. }
        | TraceEvent::HookCompleted { .. }
        | TraceEvent::SubagentStarted { .. }
        | TraceEvent::SubagentCompleted { .. }
        | TraceEvent::McpResourceAccessed { .. }
        | TraceEvent::RemoteBridgeAction { .. } => Some("tool_execution"),
        TraceEvent::SessionGoalUpdated { .. }
        | TraceEvent::AgentLoopStepEvaluated { .. }
        | TraceEvent::StopCheckEvaluated { .. }
        | TraceEvent::WorkflowFallback { .. }
        | TraceEvent::ToolObservationRecorded { .. }
        | TraceEvent::RecoveryApplied { .. }
        | TraceEvent::RecoveryPlan { .. } => Some("state_update"),
        TraceEvent::StageValidationCompleted { .. }
        | TraceEvent::ReflectionPassCompleted { .. }
        | TraceEvent::VerificationCompleted { .. }
        | TraceEvent::RequiredValidationHeartbeat { .. }
        | TraceEvent::AcceptanceReviewCompleted { .. }
        | TraceEvent::GuidedDebuggingCompleted { .. } => Some("verification"),
        TraceEvent::WorkflowCompleted { .. }
        | TraceEvent::AssistantResponded { .. }
        | TraceEvent::CompletionContractEvaluated { .. }
        | TraceEvent::FinalCloseoutPrepared { .. }
        | TraceEvent::ExecutionReportPrepared { .. }
        | TraceEvent::MemoryProposalPrepared { .. }
        | TraceEvent::CloseoutBackgroundStage { .. }
        | TraceEvent::Error { .. } => Some("closeout"),
    }
}

pub fn latest_runtime_diet_summary(trace: &TurnTrace) -> Option<String> {
    trace.events.iter().rev().find_map(|event| {
        if matches!(event, TraceEvent::RuntimeDietReport { .. }) {
            Some(event.summary())
        } else {
            None
        }
    })
}

pub fn latest_memory_proposal_summary(trace: &TurnTrace) -> Option<String> {
    trace.events.iter().rev().find_map(|event| match event {
        TraceEvent::MemoryProposalPrepared {
            status,
            candidates,
            candidate_kinds,
            evidence_items,
            write_policy,
            write_performed,
            reason,
            ..
        } => Some(format!(
            "{} candidates={} kinds={} evidence={} write_policy={} wrote={} reason={}",
            status,
            candidates,
            if candidate_kinds.is_empty() {
                "none".to_string()
            } else {
                candidate_kinds.join("+")
            },
            evidence_items,
            write_policy,
            write_performed,
            preview(reason)
        )),
        _ => None,
    })
}

pub fn latest_tool_record_count(trace: &TurnTrace) -> Option<usize> {
    trace.events.iter().rev().find_map(|event| match event {
        TraceEvent::FinalCloseoutPrepared { tool_records, .. } => Some(*tool_records),
        _ => None,
    })
}

pub fn latest_tool_record_evidence_summary(trace: &TurnTrace) -> Option<String> {
    trace.events.iter().rev().find_map(|event| match event {
        TraceEvent::FinalCloseoutPrepared {
            status,
            changed_files,
            validation_items,
            tool_records,
            tool_evidence,
            verification_proof_status,
            verification_proof_summary,
            acceptance_items,
            residual_risks,
            ..
        } if *tool_records > 0 || tool_evidence.as_ref().is_some_and(|s| !s.trim().is_empty()) => {
            Some(format!(
                "status={} records={} files={} validation={} acceptance={} risks={} proof={} proof_summary={} evidence={}",
                status,
                tool_records,
                changed_files,
                validation_items,
                acceptance_items,
                residual_risks,
                verification_proof_status.as_deref().unwrap_or("none"),
                verification_proof_summary.as_deref().unwrap_or("none"),
                tool_evidence.as_deref().unwrap_or("none")
            ))
        }
        _ => None,
    })
}
