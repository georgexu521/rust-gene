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
