use crate::engine::intent_router::IntentRoute;
use crate::engine::session_goal::SessionGoalManager;
use crate::engine::trace::{TraceCollector, TraceEvent};
use std::sync::Arc;

pub(super) struct SessionGoalUpdateContext<'a> {
    pub(super) manager: Option<&'a Arc<SessionGoalManager>>,
    pub(super) last_user_preview: &'a str,
    pub(super) route: &'a IntentRoute,
    pub(super) trace: &'a TraceCollector,
}

pub(super) struct SessionGoalController;

impl SessionGoalController {
    pub(super) fn update(context: SessionGoalUpdateContext<'_>) {
        let Some(manager) = context.manager else {
            return;
        };
        if let Some(goal) =
            manager.update_from_user_message(context.last_user_preview, Some(context.route))
        {
            context.trace.record(TraceEvent::SessionGoalUpdated {
                goal_id: goal.id,
                title: goal.title,
                status: format!("{:?}", goal.status),
                reason: "user turn routed to trackable workflow".to_string(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::intent_router::IntentRouter;
    use crate::engine::trace::{TurnStatus, TurnTrace};

    fn trace() -> TraceCollector {
        TraceCollector::new(TurnTrace::new("session", 1, "session goal"))
    }

    #[test]
    fn tracks_goal_and_records_trace_for_trackable_route() {
        let trace = trace();
        let manager = Arc::new(SessionGoalManager::new());
        let route = IntentRouter::new().route("继续优化 CLI 体验，完善状态栏");

        SessionGoalController::update(SessionGoalUpdateContext {
            manager: Some(&manager),
            last_user_preview: "继续优化 CLI 体验，完善状态栏",
            route: &route,
            trace: &trace,
        });

        assert!(manager.current().is_some());
        let finished = trace.finish(TurnStatus::Completed);
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::SessionGoalUpdated { title, .. } if title.contains("CLI")
        )));
    }

    #[test]
    fn skips_when_goal_manager_is_absent() {
        let trace = trace();
        let route = IntentRouter::new().route("继续优化 CLI 体验，完善状态栏");

        SessionGoalController::update(SessionGoalUpdateContext {
            manager: None,
            last_user_preview: "继续优化 CLI 体验，完善状态栏",
            route: &route,
            trace: &trace,
        });

        let finished = trace.finish(TurnStatus::Completed);
        assert!(!finished
            .events
            .iter()
            .any(|event| matches!(event, TraceEvent::SessionGoalUpdated { .. })));
    }
}
