use super::pseudo_tool_text;
use super::tool_execution::safe_prefix_by_bytes;
use crate::engine::trace::{TraceCollector, TraceEvent};
use crate::services::api::Message;
use std::collections::HashSet;

pub(super) struct AssistantResponseRetryRequest<'a> {
    pub(super) content: &'a str,
    pub(super) exposed_tool_names: &'a HashSet<String>,
    pub(super) tool_calls_made: bool,
    pub(super) is_local_filesystem_inspection_route: bool,
    pub(super) unsupported_filesystem_claims: Vec<String>,
    pub(super) pseudo_tool_retry_used: bool,
    pub(super) filesystem_grounding_retry_used: bool,
}

pub(super) struct AssistantResponseRetryDecision {
    pub(super) fallback_error: String,
    pub(super) assistant_message: Message,
    pub(super) correction_message: Message,
    pub(super) mark_pseudo_tool_retry_used: bool,
    pub(super) mark_filesystem_grounding_retry_used: bool,
}

pub(super) struct AssistantResponseRetryApplicationContext<'a> {
    pub(super) decision: AssistantResponseRetryDecision,
    pub(super) pseudo_tool_retry_used: &'a mut bool,
    pub(super) filesystem_grounding_retry_used: &'a mut bool,
    pub(super) trace: &'a TraceCollector,
    pub(super) messages: &'a mut Vec<Message>,
}

pub(super) struct AssistantResponseRetryController;

impl AssistantResponseRetryController {
    pub(super) fn evaluate(
        request: AssistantResponseRetryRequest<'_>,
    ) -> Option<AssistantResponseRetryDecision> {
        let needs_bash_tool_retry = pseudo_tool_text::contains_unexecuted_tool_command(
            request.content,
            request.exposed_tool_names,
        ) || pseudo_tool_text::contains_false_bash_unavailable_claim(
            request.content,
            request.exposed_tool_names,
        );
        let needs_filesystem_tool_retry = !request.tool_calls_made
            && request.is_local_filesystem_inspection_route
            && pseudo_tool_text::contains_local_filesystem_claim_without_tool(
                request.content,
                request.exposed_tool_names,
            );
        let needs_filesystem_grounding_retry = !request.unsupported_filesystem_claims.is_empty();

        let should_retry = (!request.pseudo_tool_retry_used
            && (needs_bash_tool_retry || needs_filesystem_tool_retry))
            || (!request.filesystem_grounding_retry_used && needs_filesystem_grounding_retry);
        if !should_retry {
            return None;
        }

        let (fallback_error, correction, mark_filesystem_grounding_retry_used) =
            if needs_filesystem_grounding_retry {
                (
                    format!(
                        "assistant included unsupported filesystem claim(s): {}; retrying with evidence-grounded correction",
                        request.unsupported_filesystem_claims.join(", ")
                    ),
                    "Your previous answer added filesystem metadata that was not explicitly supported by tool output. \
Re-answer from the evidence already gathered. Do not state size, item count, creation time, or exact contents unless the tool output directly contains that fact. \
If the user did not ask for those metadata fields, omit them.",
                    true,
                )
            } else if needs_filesystem_tool_retry {
                (
                    "assistant answered local filesystem state without a tool; retrying with explicit filesystem tool-use correction".to_string(),
                    "file_read and glob are currently exposed to you as callable tools. \
The user asked for current local filesystem state, so do not answer from memory or inference. \
Inspect the requested path with file_read or glob now, then answer only from that tool output. \
Do not invent size, item count, creation time, or contents that are not present in tool output.",
                    false,
                )
            } else {
                (
                    "assistant emitted an unexecuted or false-unavailable shell response; retrying with explicit bash tool-use correction".to_string(),
                    "Bash is currently exposed to you as a callable tool. \
The user asked for current local/runtime state, so do not answer from an unexecuted command and do not claim bash is unavailable. \
If a command appears in a code block or your answer asks the user to run a shell command manually, execute it with the bash tool now. \
Only report a tool as unavailable when it is not exposed in the current tool list.",
                    false,
                )
            };

        Some(AssistantResponseRetryDecision {
            fallback_error,
            assistant_message: Message::assistant(safe_prefix_by_bytes(request.content, 1200)),
            correction_message: Message::system(correction),
            mark_pseudo_tool_retry_used: !mark_filesystem_grounding_retry_used,
            mark_filesystem_grounding_retry_used,
        })
    }

    pub(super) fn apply_decision(context: AssistantResponseRetryApplicationContext<'_>) {
        let decision = context.decision;
        if decision.mark_filesystem_grounding_retry_used {
            *context.filesystem_grounding_retry_used = true;
        }
        if decision.mark_pseudo_tool_retry_used {
            *context.pseudo_tool_retry_used = true;
        }
        context.trace.record(TraceEvent::WorkflowFallback {
            error: decision.fallback_error,
        });
        context.messages.push(decision.assistant_message);
        context.messages.push(decision.correction_message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::trace::TurnTrace;

    fn exposed(names: &[&str]) -> HashSet<String> {
        names.iter().map(|name| (*name).to_string()).collect()
    }

    fn trace() -> TraceCollector {
        TraceCollector::new(TurnTrace::new("test", 1, "assistant-retry"))
    }

    #[test]
    fn retries_unexecuted_bash_command_when_bash_is_exposed() {
        let decision = AssistantResponseRetryController::evaluate(AssistantResponseRetryRequest {
            content: "```bash\npython3 app.py\n```",
            exposed_tool_names: &exposed(&["bash"]),
            tool_calls_made: false,
            is_local_filesystem_inspection_route: false,
            unsupported_filesystem_claims: Vec::new(),
            pseudo_tool_retry_used: false,
            filesystem_grounding_retry_used: false,
        })
        .expect("bash command should trigger retry");

        assert!(decision.mark_pseudo_tool_retry_used);
        assert!(!decision.mark_filesystem_grounding_retry_used);
        assert!(decision
            .fallback_error
            .contains("explicit bash tool-use correction"));
        assert!(matches!(
            decision.correction_message,
            Message::System { ref content } if content.contains("Bash is currently exposed")
        ));
    }

    #[test]
    fn retries_local_filesystem_claim_without_tool_on_first_answer() {
        let decision = AssistantResponseRetryController::evaluate(AssistantResponseRetryRequest {
            content: "桌面上没有 gex 文件夹。",
            exposed_tool_names: &exposed(&["file_read", "glob"]),
            tool_calls_made: false,
            is_local_filesystem_inspection_route: true,
            unsupported_filesystem_claims: Vec::new(),
            pseudo_tool_retry_used: false,
            filesystem_grounding_retry_used: false,
        })
        .expect("local filesystem answer without tool should trigger retry");

        assert!(decision.mark_pseudo_tool_retry_used);
        assert!(matches!(
            decision.correction_message,
            Message::System { ref content } if content.contains("file_read and glob")
        ));
    }

    #[test]
    fn retries_unsupported_filesystem_metadata_with_grounding_correction() {
        let decision = AssistantResponseRetryController::evaluate(AssistantResponseRetryRequest {
            content: "创建时间：2024 年 5 月 8 日",
            exposed_tool_names: &exposed(&["file_read"]),
            tool_calls_made: true,
            is_local_filesystem_inspection_route: true,
            unsupported_filesystem_claims: vec!["creation_time".to_string()],
            pseudo_tool_retry_used: false,
            filesystem_grounding_retry_used: false,
        })
        .expect("unsupported metadata should trigger grounding retry");

        assert!(!decision.mark_pseudo_tool_retry_used);
        assert!(decision.mark_filesystem_grounding_retry_used);
        assert!(decision.fallback_error.contains("creation_time"));
        assert!(matches!(
            decision.correction_message,
            Message::System { ref content } if content.contains("not explicitly supported")
        ));
    }

    #[test]
    fn does_not_retry_after_relevant_retry_was_used() {
        let decision = AssistantResponseRetryController::evaluate(AssistantResponseRetryRequest {
            content: "```bash\npython3 app.py\n```",
            exposed_tool_names: &exposed(&["bash"]),
            tool_calls_made: false,
            is_local_filesystem_inspection_route: false,
            unsupported_filesystem_claims: Vec::new(),
            pseudo_tool_retry_used: true,
            filesystem_grounding_retry_used: false,
        });

        assert!(decision.is_none());
    }

    #[test]
    fn apply_decision_updates_retry_state_trace_and_messages() {
        let trace = trace();
        let mut pseudo_tool_retry_used = false;
        let mut filesystem_grounding_retry_used = false;
        let mut messages = Vec::new();
        let decision = AssistantResponseRetryController::evaluate(AssistantResponseRetryRequest {
            content: "```bash\npython3 app.py\n```",
            exposed_tool_names: &exposed(&["bash"]),
            tool_calls_made: false,
            is_local_filesystem_inspection_route: false,
            unsupported_filesystem_claims: Vec::new(),
            pseudo_tool_retry_used,
            filesystem_grounding_retry_used,
        })
        .expect("bash command should trigger retry");

        AssistantResponseRetryController::apply_decision(
            AssistantResponseRetryApplicationContext {
                decision,
                pseudo_tool_retry_used: &mut pseudo_tool_retry_used,
                filesystem_grounding_retry_used: &mut filesystem_grounding_retry_used,
                trace: &trace,
                messages: &mut messages,
            },
        );

        assert!(pseudo_tool_retry_used);
        assert!(!filesystem_grounding_retry_used);
        assert_eq!(messages.len(), 2);
        assert!(matches!(messages[0], Message::Assistant { .. }));
        assert!(matches!(messages[1], Message::System { .. }));
        let finished = trace.finish(crate::engine::trace::TurnStatus::Completed);
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::WorkflowFallback { error }
                if error.contains("explicit bash tool-use correction")
        )));
    }
}
