use std::collections::HashMap;

pub(super) struct ToolFailureStopRequest<'a> {
    pub(super) any_tool_success: bool,
    pub(super) repeated_failed_tools: &'a [String],
    pub(super) failed_tool_names: &'a HashMap<String, usize>,
}

pub(super) struct ToolFailureStopDecision;

pub(super) struct ToolFailureStopController;

impl ToolFailureStopController {
    pub(super) fn decide(request: ToolFailureStopRequest<'_>) -> Option<ToolFailureStopDecision> {
        let _ = (
            request.any_tool_success,
            request.repeated_failed_tools,
            request.failed_tool_names,
        );
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_failed_tools_are_advisory_only() {
        let failed_tool_names = HashMap::from([("file_write".to_string(), 3)]);

        let decision = ToolFailureStopController::decide(ToolFailureStopRequest {
            any_tool_success: false,
            repeated_failed_tools: &[
                "file_edit".to_string(),
                "bash".to_string(),
                "file_edit".to_string(),
            ],
            failed_tool_names: &failed_tool_names,
        });

        assert!(decision.is_none());
    }

    #[test]
    fn noisy_non_read_only_retries_are_advisory_only() {
        let failed_tool_names = HashMap::from([
            ("file_write".to_string(), 2),
            ("file_read".to_string(), 5),
            ("grep".to_string(), 3),
        ]);

        let decision = ToolFailureStopController::decide(ToolFailureStopRequest {
            any_tool_success: false,
            repeated_failed_tools: &[],
            failed_tool_names: &failed_tool_names,
        });

        assert!(decision.is_none());
    }

    #[test]
    fn success_or_read_only_failures_do_not_stop() {
        let failed_tool_names = HashMap::from([("file_write".to_string(), 3)]);

        assert!(ToolFailureStopController::decide(ToolFailureStopRequest {
            any_tool_success: true,
            repeated_failed_tools: &["file_edit".to_string()],
            failed_tool_names: &failed_tool_names,
        })
        .is_none());

        let read_only_failures =
            HashMap::from([("file_read".to_string(), 3), ("grep".to_string(), 2)]);
        assert!(ToolFailureStopController::decide(ToolFailureStopRequest {
            any_tool_success: false,
            repeated_failed_tools: &[],
            failed_tool_names: &read_only_failures,
        })
        .is_none());
    }
}
