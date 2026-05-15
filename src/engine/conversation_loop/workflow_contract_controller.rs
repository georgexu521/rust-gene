use super::{persist_workflow_learning_event, workflow_contract_enabled};
use crate::engine::code_change_workflow::CodeChangeWorkflowRunner;
use crate::engine::intent_router::IntentRoute;
use crate::engine::retrieval_context::RetrievalContext;
use crate::engine::task_context::TaskContextBundle;
use crate::engine::trace::{TraceCollector, TraceEvent};
use crate::engine::workflow_contract::{
    ProgrammingWorkflowJudgment, WorkflowContractAnalyzer, WorkflowContractPrompt,
};
use crate::services::api::{LlmProvider, Message};
use crate::session_store::{LearningEventRecord, SessionStore};
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, warn};

pub(super) struct WorkflowContractJudgmentContext<'a> {
    pub(super) provider: &'a dyn LlmProvider,
    pub(super) model: String,
    pub(super) session_store: Option<&'a Arc<SessionStore>>,
    pub(super) session_id: &'a str,
    pub(super) last_user_preview: &'a str,
    pub(super) route: &'a IntentRoute,
    pub(super) working_dir: &'a Path,
    pub(super) learning_events: &'a [LearningEventRecord],
    pub(super) retrieval_context: Option<&'a RetrievalContext>,
    pub(super) task_bundle: &'a mut TaskContextBundle,
    pub(super) code_workflow: &'a mut CodeChangeWorkflowRunner,
    pub(super) messages: &'a mut Vec<Message>,
    pub(super) trace: &'a TraceCollector,
}

pub(super) struct WorkflowContractJudgmentApplicationContext<'a> {
    pub(super) session_store: Option<&'a Arc<SessionStore>>,
    pub(super) session_id: &'a str,
    pub(super) learning_events: &'a [LearningEventRecord],
    pub(super) retrieval_context: Option<&'a RetrievalContext>,
    pub(super) task_bundle: &'a mut TaskContextBundle,
    pub(super) code_workflow: &'a mut CodeChangeWorkflowRunner,
    pub(super) messages: &'a mut Vec<Message>,
    pub(super) trace: &'a TraceCollector,
}

pub(super) struct WorkflowContractController;

impl WorkflowContractController {
    pub(super) async fn run(context: WorkflowContractJudgmentContext<'_>) {
        let prompt = WorkflowContractPrompt::new(
            context.last_user_preview,
            context.route.clone(),
            context.working_dir.display().to_string(),
        );
        if !context.code_workflow.should_request_workflow_judgment()
            || !prompt.should_ask_model()
            || !workflow_contract_enabled(context.provider)
        {
            return;
        }

        let analyzer = WorkflowContractAnalyzer::new(context.provider, context.model);
        match analyzer.analyze(prompt).await {
            Ok(judgment) => Self::apply_judgment(
                WorkflowContractJudgmentApplicationContext {
                    session_store: context.session_store,
                    session_id: context.session_id,
                    learning_events: context.learning_events,
                    retrieval_context: context.retrieval_context,
                    task_bundle: context.task_bundle,
                    code_workflow: context.code_workflow,
                    messages: context.messages,
                    trace: context.trace,
                },
                judgment,
            ),
            Err(err) => Self::record_analysis_error(context.trace, &err),
        }
    }

    pub(super) fn apply_judgment(
        context: WorkflowContractJudgmentApplicationContext<'_>,
        mut judgment: ProgrammingWorkflowJudgment,
    ) {
        let learning_audit = crate::engine::learning_planning::apply_learning_to_workflow_judgment(
            &mut judgment,
            context.learning_events,
            context.retrieval_context,
        );
        let context_note = judgment.to_turn_context();
        context.trace.record(TraceEvent::WorkflowJudgmentCompleted {
            task_type: judgment.task_type.clone(),
            complexity: format!("{:?}", judgment.complexity),
            risk: format!("{:?}", judgment.risk),
            plan_steps: judgment.plan.len(),
            acceptance_checks: judgment.acceptance.criteria.len(),
            questions: judgment.questions.len(),
            guided_reasoning: judgment.guided_reasoning_required,
        });
        let top_step = judgment.top_plan_step();
        context.trace.record(TraceEvent::WorkflowPlanProgress {
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
            reweighted: learning_audit.applied,
        });
        if learning_audit.applied {
            context.trace.record(TraceEvent::WorkflowLearningAdjusted {
                adjustments: learning_audit.adjustments.len(),
                before_top_step: learning_audit.before_top_step.clone(),
                after_top_step: learning_audit.after_top_step.clone(),
                reason: learning_audit.explanation.clone(),
            });
            persist_workflow_learning_event(
                context.session_store,
                context.session_id,
                "planning_adjustment",
                format!(
                    "Learning adjusted workflow plan with {} change(s)",
                    learning_audit.adjustments.len()
                ),
                0.85,
                serde_json::to_value(&learning_audit).unwrap_or_else(|_| serde_json::json!({})),
            );
        }
        persist_workflow_learning_event(
            context.session_store,
            context.session_id,
            "workflow_judgment",
            format!(
                "Workflow judgment task_type={} risk={:?} questions={} guided={}",
                judgment.task_type,
                judgment.risk,
                judgment.questions.len(),
                judgment.guided_reasoning_required
            ),
            0.8,
            serde_json::json!({
                "task_type": judgment.task_type.clone(),
                "complexity": format!("{:?}", judgment.complexity),
                "risk": format!("{:?}", judgment.risk),
                "requirement_complete_enough": judgment.requirement_complete_enough,
                "needs_user_questions": judgment.needs_user_questions,
                "question_reason": judgment.question_reason.clone(),
                "questions": judgment.questions.clone(),
                "assumptions": judgment.assumptions.clone(),
                "guided_reasoning_required": judgment.guided_reasoning_required,
                "guided_reasoning_triggers": judgment.guided_reasoning_triggers.iter().map(|trigger| format!("{:?}", trigger)).collect::<Vec<_>>(),
                "plan_steps": judgment.plan.len(),
                "weighted_plan": judgment.weighted_plan_summary(),
                "acceptance_checks": judgment.acceptance.criteria.len(),
            }),
        );
        context.task_bundle.apply_workflow_judgment(judgment);
        context.code_workflow.refresh_policy(context.task_bundle);
        let insert_at = context
            .messages
            .iter()
            .take_while(|message| matches!(message, Message::System { .. }))
            .count();
        context
            .messages
            .insert(insert_at, Message::system(context_note));
    }

    pub(super) fn record_analysis_error(trace: &TraceCollector, err: &anyhow::Error) {
        if crate::engine::workflow_contract::is_recoverable_workflow_judgment_parse_error(err) {
            debug!(
                "Workflow judgment skipped after non-JSON model response: {}",
                err
            );
            trace.record(TraceEvent::WorkflowFallback {
                error: "workflow judgment skipped after non-JSON model response".to_string(),
            });
        } else {
            warn!("Workflow judgment analysis failed: {}", err);
            trace.record(TraceEvent::WorkflowFallback {
                error: format!("workflow judgment analysis failed: {}", err),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::intent_router::IntentRouter;
    use crate::engine::trace::{TurnStatus, TurnTrace};
    use crate::engine::workflow_contract::{
        AcceptanceContract, PriorityLabel, TaskComplexity, WorkflowPlanStep,
    };

    fn trace() -> TraceCollector {
        TraceCollector::new(TurnTrace::new(
            "session".to_string(),
            1,
            "workflow-contract",
        ))
    }

    fn task_bundle_and_runner() -> (TaskContextBundle, CodeChangeWorkflowRunner) {
        let route = IntentRouter::new().route("修改 src/main.rs 并运行测试");
        let bundle = TaskContextBundle::new("修改 src/main.rs", ".", route, None);
        let runner = CodeChangeWorkflowRunner::new(&bundle);
        (bundle, runner)
    }

    fn judgment() -> ProgrammingWorkflowJudgment {
        ProgrammingWorkflowJudgment {
            task_type: "bug_fix".to_string(),
            complexity: TaskComplexity::Medium,
            risk: crate::engine::intent_router::RiskLevel::Medium,
            requirement_complete_enough: true,
            needs_user_questions: false,
            question_reason: None,
            questions: Vec::new(),
            assumptions: vec!["Preserve existing controller boundaries".to_string()],
            guided_reasoning_required: false,
            guided_reasoning_triggers: Vec::new(),
            plan: vec![WorkflowPlanStep {
                id: Some("inspect".to_string()),
                description: "Inspect current implementation".to_string(),
                priority: PriorityLabel::P0,
                weight: None,
                importance_score: Some(1.0),
                weight_share: Some(1.0),
                factors: None,
                override_adjustment: None,
                computation: None,
                reason: "Need current code context".to_string(),
                acceptance_criteria: vec!["Relevant code reviewed".to_string()],
            }],
            acceptance: AcceptanceContract::pending(
                "修改 src/main.rs 并运行测试",
                vec!["cargo test -q passes".to_string()],
                Vec::new(),
            ),
        }
    }

    #[test]
    fn apply_judgment_updates_task_bundle_trace_and_messages() {
        let trace = trace();
        let (mut task_bundle, mut code_workflow) = task_bundle_and_runner();
        let mut messages = vec![
            Message::system("base system"),
            Message::user("修改 src/main.rs 并运行测试"),
        ];

        WorkflowContractController::apply_judgment(
            WorkflowContractJudgmentApplicationContext {
                session_store: None,
                session_id: "session",
                learning_events: &[],
                retrieval_context: None,
                task_bundle: &mut task_bundle,
                code_workflow: &mut code_workflow,
                messages: &mut messages,
                trace: &trace,
            },
            judgment(),
        );

        assert!(task_bundle.workflow_judgment.is_some());
        assert!(task_bundle
            .constraints
            .iter()
            .any(|constraint| constraint.contains("Preserve existing controller boundaries")));
        assert!(task_bundle
            .acceptance_checks
            .iter()
            .any(|check| check == "cargo test -q passes"));
        assert!(matches!(messages[0], Message::System { .. }));
        assert!(matches!(messages[1], Message::System { .. }));
        assert!(matches!(messages[2], Message::User { .. }));
        let finished = trace.finish(TurnStatus::Completed);
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::WorkflowJudgmentCompleted {
                task_type,
                plan_steps: 1,
                acceptance_checks: 1,
                ..
            } if task_type == "bug_fix"
        )));
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::WorkflowPlanProgress {
                active_step: Some(step),
                reweighted: false,
                ..
            } if step == "Inspect current implementation"
        )));
    }

    #[test]
    fn recoverable_parse_error_records_skip_trace() {
        let trace = trace();
        let err = anyhow::anyhow!("workflow judgment response did not contain JSON");

        WorkflowContractController::record_analysis_error(&trace, &err);

        let finished = trace.finish(TurnStatus::Completed);
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::WorkflowFallback { error }
                if error == "workflow judgment skipped after non-JSON model response"
        )));
    }
}
