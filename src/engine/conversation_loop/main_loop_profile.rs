use crate::engine::intent_router::{IntentRoute, RetrievalPolicy, RiskLevel, WorkflowKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MainLoopProfile {
    QuietDirect,
    Standard,
}

impl MainLoopProfile {
    pub(super) fn from_turn(route: &IntentRoute, required_validation_commands: &[String]) -> Self {
        let simple_direct = route.workflow == WorkflowKind::Direct
            && matches!(
                route.retrieval,
                RetrievalPolicy::None | RetrievalPolicy::Light
            )
            && route.risk == RiskLevel::Low
            && route.recommended_tools.is_empty()
            && required_validation_commands.is_empty();

        if simple_direct {
            Self::QuietDirect
        } else {
            Self::Standard
        }
    }

    pub(super) fn is_quiet_direct(self) -> bool {
        matches!(self, Self::QuietDirect)
    }

    pub(super) fn emit_start_event(self) -> bool {
        !self.is_quiet_direct()
    }

    pub(super) fn expose_tools(self) -> bool {
        !self.is_quiet_direct()
    }

    pub(super) fn inject_dynamic_context(self) -> bool {
        !self.is_quiet_direct()
    }

    pub(super) fn max_loop_iterations(
        self,
        configured_max: usize,
        repair_attempts: usize,
    ) -> usize {
        if self.is_quiet_direct() {
            1
        } else {
            configured_max + repair_attempts.max(3)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::intent_router::IntentRouter;

    #[test]
    fn greeting_uses_quiet_direct_profile() {
        let route = IntentRouter::new().route("你好");

        assert_eq!(
            MainLoopProfile::from_turn(&route, &[]),
            MainLoopProfile::QuietDirect
        );
    }

    #[test]
    fn code_change_uses_standard_profile() {
        let route = IntentRouter::new().route("帮我做一个天气预报网页");

        assert_eq!(
            MainLoopProfile::from_turn(&route, &[]),
            MainLoopProfile::Standard
        );
    }

    #[test]
    fn direct_validation_request_uses_standard_profile() {
        let route = IntentRouter::new().route("运行 cargo test -q");
        let required = vec!["cargo test -q".to_string()];

        assert_eq!(
            MainLoopProfile::from_turn(&route, &required),
            MainLoopProfile::Standard
        );
    }
}
