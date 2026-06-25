//! Explicit LabRun context-maintenance side effects for request preparation.

use crate::engine::trace::{TraceCollector, TraceEvent};
use tracing::debug;

pub(super) struct LabContextMaintenanceOutcome {
    pub(super) recorded_decision: Option<crate::lab::model::LabCompressionDecision>,
    pub(super) auto_compression_artifact_id: Option<String>,
}

pub(super) fn maybe_record_lab_context_maintenance(
    store: &crate::lab::store::LabStore,
    working_dir: &std::path::Path,
    run: &crate::lab::model::LabRun,
    packet: &crate::lab::context::LabContextPacket,
    trace: &TraceCollector,
) -> LabContextMaintenanceOutcome {
    let compression_decision = crate::lab::context::evaluate_lab_context_compression(run, packet);
    let recorded_decision = store.record_compression_decision(compression_decision).ok();
    let auto_compression_artifact_id = recorded_decision.as_ref().and_then(|decision| {
        if matches!(
            decision.action,
            crate::lab::model::LabCompressionAction::None
        ) {
            return None;
        }
        match crate::lab::orchestrator::LabOrchestrator::for_project(working_dir)
            .auto_create_compression_summary_for_decision(decision)
        {
            Ok(Some(created)) => Some(created.artifact.artifact_id().to_string()),
            Ok(None) => None,
            Err(err) => {
                debug!(
                    target: "lab",
                    error = %err,
                    lab_run_id = %decision.lab_run_id,
                    "failed to auto-create LabRun compression summary"
                );
                None
            }
        }
    });

    if let Some(decision) = recorded_decision.as_ref() {
        if let Err(err) = store.record_run_event(
            &decision.lab_run_id,
            "lab_context_maintenance",
            serde_json::json!({
                "decision_id": decision.decision_id,
                "action": format!("{:?}", decision.action),
                "artifact_id": auto_compression_artifact_id.clone(),
                "source": "request_preparation.lab_context_maintenance",
            }),
        ) {
            debug!(
                target: "lab",
                error = %err,
                lab_run_id = %decision.lab_run_id,
                "failed to record LabRun context maintenance event"
            );
        }
        trace.record(TraceEvent::LabContextMaintenanceRecorded {
            lab_run_id: decision.lab_run_id.clone(),
            decision_id: Some(decision.decision_id.clone()),
            action: format!("{:?}", decision.action),
            artifact_id: auto_compression_artifact_id.clone(),
            source: "request_preparation.lab_context_maintenance".to_string(),
        });
    }

    LabContextMaintenanceOutcome {
        recorded_decision,
        auto_compression_artifact_id,
    }
}
