use crate::engine::trace::{TraceCollector, TraceEvent};

pub(super) fn apply_workflow_feedback_and_trace(
    task_bundle: &mut crate::engine::task_context::TaskContextBundle,
    trace: &TraceCollector,
    feedback: crate::engine::workflow_contract::WeightFeedbackEvent,
) {
    let Some(judgment) = task_bundle.workflow_judgment.as_mut() else {
        return;
    };
    let Some(top_step) = judgment.top_plan_step() else {
        return;
    };
    let old_plan = judgment.plan.clone();
    let target_id = top_step.id.clone();
    let target_description = top_step.description.clone();

    let Some(step) =
        judgment
            .plan
            .iter_mut()
            .find(|step| match (target_id.as_deref(), step.id.as_deref()) {
                (Some(target), Some(id)) => target == id,
                _ => step.description == target_description,
            })
    else {
        return;
    };

    crate::engine::workflow_contract::apply_weight_feedback(step, &feedback);
    crate::engine::workflow_contract::normalize_weight_shares(&mut judgment.plan);

    if !crate::engine::workflow_contract::should_record_reweight(&old_plan, &judgment.plan) {
        return;
    }

    let top_step = judgment.top_plan_step();
    trace.record(TraceEvent::WorkflowPlanProgress {
        total_steps: judgment.plan.len(),
        completed_steps: 0,
        active_step: top_step.as_ref().map(|step| step.description.clone()),
        top_priority: top_step.as_ref().map(|step| format!("{:?}", step.priority)),
        top_importance_score: top_step.as_ref().map(|step| step.normalized_weight()),
        top_weight_share: top_step.as_ref().map(|step| step.computed_weight_share()),
        weight_source: top_step
            .as_ref()
            .and_then(|step| step.weight_source())
            .map(|source| format!("{:?}", source)),
        reweighted: true,
    });
}

pub(super) fn trace_stage_validation(
    trace: &TraceCollector,
    record: &crate::engine::code_change_workflow::StageValidationRecord,
) {
    trace.record(TraceEvent::StageValidationCompleted {
        step: record.step_description.clone(),
        status: record.status.label().to_string(),
        changed_files: record.changed_files.len(),
        evidence_items: record.evidence.len(),
    });
}

pub(super) fn trace_adaptive_workflow_trigger(
    trace: &TraceCollector,
    trigger: crate::engine::code_change_workflow::AdaptiveWorkflowTrigger,
    runner: &crate::engine::code_change_workflow::CodeChangeWorkflowRunner,
) {
    trace.record(TraceEvent::AdaptiveWorkflowTriggered {
        trigger: trigger.label().to_string(),
        depth: format!("{:?}", runner.policy.depth),
        require_workflow_judgment: runner.policy.require_workflow_judgment,
        require_stage_validation: runner.policy.require_stage_validation,
        max_repair_attempts: runner.policy.max_repair_attempts,
        reason: runner.policy.reason.clone(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::intent_router::IntentRouter;

    #[test]
    fn trace_stage_validation_records_compact_counts() {
        let trace = TraceCollector::new(crate::engine::trace::TurnTrace::new(
            "session".to_string(),
            1,
            "test",
        ));
        let route = IntentRouter::new().route("修改 CLI 状态栏");
        let bundle = crate::engine::task_context::TaskContextBundle::new(
            "修改 CLI 状态栏",
            ".",
            route,
            None,
        );
        let mut runner =
            crate::engine::code_change_workflow::CodeChangeWorkflowRunner::new(&bundle);
        let record = runner.record_stage_validation(
            &bundle,
            &[std::path::PathBuf::from("src/main.rs")],
            true,
            &["cargo check passed".to_string()],
        );

        trace_stage_validation(&trace, &record);

        let finished = trace.finish(crate::engine::trace::TurnStatus::Completed);
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::StageValidationCompleted {
                changed_files: 1,
                evidence_items: 1,
                ..
            }
        )));
    }
}
