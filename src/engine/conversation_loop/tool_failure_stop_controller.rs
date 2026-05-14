use super::READ_ONLY_TOOLS;
use std::collections::HashMap;

pub(super) struct ToolFailureStopRequest<'a> {
    pub(super) any_tool_success: bool,
    pub(super) repeated_failed_tools: &'a [String],
    pub(super) failed_tool_names: &'a HashMap<String, usize>,
}

pub(super) struct ToolFailureStopDecision {
    pub(super) message: String,
}

pub(super) struct ToolFailureStopController;

impl ToolFailureStopController {
    pub(super) fn decide(request: ToolFailureStopRequest<'_>) -> Option<ToolFailureStopDecision> {
        if request.any_tool_success {
            return None;
        }

        if let Some(message) = Self::repeated_failed_tools_message(request.repeated_failed_tools) {
            return Some(ToolFailureStopDecision { message });
        }

        Self::noisy_retry_message(request.failed_tool_names)
            .map(|message| ToolFailureStopDecision { message })
    }

    fn repeated_failed_tools_message(repeated_failed_tools: &[String]) -> Option<String> {
        let mut repeated = repeated_failed_tools.to_vec();
        repeated.sort();
        repeated.dedup();
        if repeated.is_empty() {
            return None;
        }
        Some(format!(
            "[Stopped repeated failed tool attempts: {}]",
            repeated.join(", ")
        ))
    }

    fn noisy_retry_message(failed_tool_names: &HashMap<String, usize>) -> Option<String> {
        let mut noisy_by_name = failed_tool_names
            .iter()
            .filter(|(name, count)| **count >= 2 && !READ_ONLY_TOOLS.contains(&name.as_str()))
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>();
        noisy_by_name.sort();
        noisy_by_name.dedup();
        if noisy_by_name.is_empty() {
            return None;
        }
        Some(format!(
            "[Stopped noisy retries after repeated failures: {}]",
            noisy_by_name.join(", ")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_failed_tools_take_precedence_and_are_sorted() {
        let failed_tool_names = HashMap::from([("file_write".to_string(), 3)]);

        let decision = ToolFailureStopController::decide(ToolFailureStopRequest {
            any_tool_success: false,
            repeated_failed_tools: &[
                "file_edit".to_string(),
                "bash".to_string(),
                "file_edit".to_string(),
            ],
            failed_tool_names: &failed_tool_names,
        })
        .expect("repeated failures should stop");

        assert_eq!(
            decision.message,
            "[Stopped repeated failed tool attempts: bash, file_edit]"
        );
    }

    #[test]
    fn noisy_non_read_only_retries_stop_when_no_tool_succeeded() {
        let failed_tool_names = HashMap::from([
            ("file_write".to_string(), 2),
            ("file_read".to_string(), 5),
            ("grep".to_string(), 3),
        ]);

        let decision = ToolFailureStopController::decide(ToolFailureStopRequest {
            any_tool_success: false,
            repeated_failed_tools: &[],
            failed_tool_names: &failed_tool_names,
        })
        .expect("noisy write retries should stop");

        assert_eq!(
            decision.message,
            "[Stopped noisy retries after repeated failures: file_write]"
        );
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
