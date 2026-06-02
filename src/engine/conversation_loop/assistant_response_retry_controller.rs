use super::tool_execution::safe_prefix_by_bytes;
use super::{pseudo_tool_text, should_use_nonstreaming_tools};
use crate::engine::evidence_ledger::EvidenceLedger;
use crate::engine::intent_router::{IntentKind, IntentRoute, WorkflowKind};
use crate::engine::streaming::{emit_text_progressively, StreamEvent};
use crate::engine::trace::{TraceCollector, TraceEvent};
use crate::services::api::{LlmProvider, Message, Tool};
use std::collections::HashSet;
use tokio::sync::mpsc;

pub(super) struct AssistantResponseRetryRequest<'a> {
    pub(super) content: &'a str,
    pub(super) exposed_tool_names: &'a HashSet<String>,
    pub(super) tool_calls_made: bool,
    pub(super) is_local_filesystem_inspection_route: bool,
    pub(super) unsupported_filesystem_claims: Vec<String>,
    pub(super) pseudo_tool_retry_used: bool,
    pub(super) filesystem_grounding_retry_used: bool,
    pub(super) continuation_retry_used: bool,
}

pub(super) struct AssistantResponseRetryDecision {
    pub(super) fallback_error: String,
    pub(super) assistant_message: Message,
    pub(super) correction_message: Message,
    pub(super) mark_pseudo_tool_retry_used: bool,
    pub(super) mark_filesystem_grounding_retry_used: bool,
    pub(super) mark_continuation_retry_used: bool,
}

pub(super) struct AssistantResponseRetryApplicationContext<'a> {
    pub(super) decision: AssistantResponseRetryDecision,
    pub(super) pseudo_tool_retry_used: &'a mut bool,
    pub(super) filesystem_grounding_retry_used: &'a mut bool,
    pub(super) continuation_retry_used: &'a mut bool,
    pub(super) trace: &'a TraceCollector,
    pub(super) messages: &'a mut Vec<Message>,
}

pub(super) struct NoToolAssistantResponseContext<'a> {
    pub(super) content: &'a str,
    pub(super) route: &'a IntentRoute,
    pub(super) evidence_ledger: &'a EvidenceLedger,
    pub(super) exposed_tool_names: &'a HashSet<String>,
    pub(super) tool_calls_made: bool,
    pub(super) pseudo_tool_retry_used: &'a mut bool,
    pub(super) filesystem_grounding_retry_used: &'a mut bool,
    pub(super) continuation_retry_used: &'a mut bool,
    pub(super) provider: &'a dyn LlmProvider,
    pub(super) tools: &'a [Tool],
    pub(super) tx: Option<&'a mpsc::Sender<StreamEvent>>,
    pub(super) trace: &'a TraceCollector,
    pub(super) messages: &'a mut Vec<Message>,
}

pub(super) enum NoToolAssistantResponseFlow {
    Retry,
    Finish,
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
        let needs_continuation_retry =
            request.tool_calls_made && is_continuation_only_response(request.content);

        let should_retry = (!request.pseudo_tool_retry_used
            && (needs_bash_tool_retry || needs_filesystem_tool_retry))
            || (!request.filesystem_grounding_retry_used && needs_filesystem_grounding_retry)
            || (!request.continuation_retry_used && needs_continuation_retry);
        if !should_retry {
            return None;
        }

        let (
            fallback_error,
            correction,
            mark_filesystem_grounding_retry_used,
            mark_continuation_retry_used,
        ) = if needs_filesystem_grounding_retry {
            (
                format!(
                    "assistant included unsupported filesystem claim(s): {}; retrying with evidence-grounded correction",
                    request.unsupported_filesystem_claims.join(", ")
                ),
                "Your previous answer added filesystem metadata that was not explicitly supported by tool output. \
Re-answer from the evidence already gathered. Do not state size, item count, creation time, or exact contents unless the tool output directly contains that fact. \
If the user did not ask for those metadata fields, omit them.",
                true,
                false,
            )
        } else if needs_filesystem_tool_retry {
            (
                    "assistant answered local filesystem state without a tool; retrying with explicit filesystem tool-use correction".to_string(),
                    "file_read and glob are currently exposed to you as callable tools. \
The user asked for current local filesystem state, so do not answer from memory or inference. \
Inspect the requested path with file_read or glob now, then answer only from that tool output. \
Do not invent size, item count, creation time, or contents that are not present in tool output.",
                    false,
                    false,
                )
        } else if needs_continuation_retry {
            (
                    "assistant returned a continuation placeholder without tools; retrying with explicit closeout correction".to_string(),
                    "Your previous response said you would continue investigating, but it did not call a tool and it did not answer the user. \
Use the tool results already gathered. If the answer is knowable, provide the final answer now in the user's requested format. \
Only call a tool if one specific missing fact is required. Do not reply with planning prose such as \"I'll check\" or \"continuing\" without a tool call.",
                    false,
                    true,
                )
        } else {
            (
                    "assistant emitted an unexecuted or false-unavailable shell response; retrying with explicit bash tool-use correction".to_string(),
                    "Bash is currently exposed to you as a callable tool. \
The user asked for current local/runtime state, so do not answer from an unexecuted command and do not claim bash is unavailable. \
If a command appears in a code block or your answer asks the user to run a shell command manually, execute it with the bash tool now. \
Only report a tool as unavailable when it is not exposed in the current tool list.",
                    false,
                    false,
                )
        };

        Some(AssistantResponseRetryDecision {
            fallback_error,
            assistant_message: Message::assistant(safe_prefix_by_bytes(request.content, 1200)),
            correction_message: Message::system(format!(
                "<recent_observation>\n{}\n</recent_observation>",
                correction
            )),
            mark_pseudo_tool_retry_used: !mark_filesystem_grounding_retry_used
                && !mark_continuation_retry_used,
            mark_filesystem_grounding_retry_used,
            mark_continuation_retry_used,
        })
    }

    pub(super) fn apply_decision(context: AssistantResponseRetryApplicationContext<'_>) {
        let decision = context.decision;
        if decision.mark_filesystem_grounding_retry_used {
            *context.filesystem_grounding_retry_used = true;
        }
        if decision.mark_continuation_retry_used {
            *context.continuation_retry_used = true;
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

    pub(super) async fn handle_no_tool_response(
        context: NoToolAssistantResponseContext<'_>,
    ) -> NoToolAssistantResponseFlow {
        let is_local_filesystem_route = is_local_filesystem_inspection_route(context.route);
        let filesystem_grounding_gaps = if is_local_filesystem_route {
            context
                .evidence_ledger
                .unsupported_filesystem_claims(context.content)
        } else {
            Vec::new()
        };

        if let Some(retry_decision) = Self::evaluate(AssistantResponseRetryRequest {
            content: context.content,
            exposed_tool_names: context.exposed_tool_names,
            tool_calls_made: context.tool_calls_made,
            is_local_filesystem_inspection_route: is_local_filesystem_route,
            unsupported_filesystem_claims: filesystem_grounding_gaps,
            pseudo_tool_retry_used: *context.pseudo_tool_retry_used,
            filesystem_grounding_retry_used: *context.filesystem_grounding_retry_used,
            continuation_retry_used: *context.continuation_retry_used,
        }) {
            Self::apply_decision(AssistantResponseRetryApplicationContext {
                decision: retry_decision,
                pseudo_tool_retry_used: context.pseudo_tool_retry_used,
                filesystem_grounding_retry_used: context.filesystem_grounding_retry_used,
                continuation_retry_used: context.continuation_retry_used,
                trace: context.trace,
                messages: context.messages,
            });
            return NoToolAssistantResponseFlow::Retry;
        }

        if let Some(tx) = context.tx {
            if should_use_nonstreaming_tools(context.provider, context.tools)
                && !context.content.is_empty()
            {
                emit_text_progressively(tx, context.content.to_string()).await;
            }
        }

        NoToolAssistantResponseFlow::Finish
    }
}

pub(super) fn is_continuation_only_response(content: &str) -> bool {
    let trimmed = content.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 160 {
        return false;
    }
    let lower = trimmed.to_ascii_lowercase();
    contains_any(
        trimmed,
        &[
            "继续查找",
            "继续检查",
            "继续读取",
            "继续分析",
            "补齐",
            "补充",
            "还需要",
            "需要补",
        ],
    ) || lower.starts_with("i'll ")
        || lower.starts_with("i will ")
        || lower.starts_with("let me ")
        || lower.starts_with("continuing ")
        || lower.starts_with("continue ")
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

pub(super) fn is_local_filesystem_inspection_route(route: &IntentRoute) -> bool {
    matches!(route.intent, IntentKind::DirectAnswer)
        && matches!(route.workflow, WorkflowKind::Direct)
        && route
            .recommended_tools
            .iter()
            .any(|tool| matches!(tool.as_str(), "file_read" | "glob"))
        && !route
            .recommended_tools
            .iter()
            .any(|tool| tool.as_str() == "bash")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::intent_router::IntentRouter;
    use crate::engine::trace::{TurnStatus, TurnTrace};
    use crate::services::api::{ChatRequest, ChatResponse};
    use async_openai::types::ChatCompletionResponseStream;

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
            continuation_retry_used: false,
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
            continuation_retry_used: false,
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
            continuation_retry_used: false,
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
    fn retries_continuation_placeholder_after_tools() {
        let decision = AssistantResponseRetryController::evaluate(AssistantResponseRetryRequest {
            content: "继续查找路由枚举和工具定义。",
            exposed_tool_names: &exposed(&["file_read", "grep", "bash"]),
            tool_calls_made: true,
            is_local_filesystem_inspection_route: false,
            unsupported_filesystem_claims: Vec::new(),
            pseudo_tool_retry_used: false,
            filesystem_grounding_retry_used: false,
            continuation_retry_used: false,
        })
        .expect("continuation placeholder should trigger a closeout retry");

        assert!(!decision.mark_pseudo_tool_retry_used);
        assert!(!decision.mark_filesystem_grounding_retry_used);
        assert!(decision.mark_continuation_retry_used);
        assert!(decision.fallback_error.contains("continuation placeholder"));
    }

    #[test]
    fn retries_chinese_fill_in_placeholder_after_tools() {
        let decision = AssistantResponseRetryController::evaluate(AssistantResponseRetryRequest {
            content: "补齐关键未读区段：route_message 路由表。",
            exposed_tool_names: &exposed(&["file_read", "grep", "bash"]),
            tool_calls_made: true,
            is_local_filesystem_inspection_route: false,
            unsupported_filesystem_claims: Vec::new(),
            pseudo_tool_retry_used: false,
            filesystem_grounding_retry_used: false,
            continuation_retry_used: false,
        })
        .expect("Chinese fill-in placeholder should trigger a closeout retry");

        assert!(!decision.mark_pseudo_tool_retry_used);
        assert!(!decision.mark_filesystem_grounding_retry_used);
        assert!(decision.mark_continuation_retry_used);
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
            continuation_retry_used: false,
        });

        assert!(decision.is_none());
    }

    #[test]
    fn apply_decision_updates_retry_state_trace_and_messages() {
        let trace = trace();
        let mut pseudo_tool_retry_used = false;
        let mut filesystem_grounding_retry_used = false;
        let mut continuation_retry_used = false;
        let mut messages = Vec::new();
        let decision = AssistantResponseRetryController::evaluate(AssistantResponseRetryRequest {
            content: "```bash\npython3 app.py\n```",
            exposed_tool_names: &exposed(&["bash"]),
            tool_calls_made: false,
            is_local_filesystem_inspection_route: false,
            unsupported_filesystem_claims: Vec::new(),
            pseudo_tool_retry_used,
            filesystem_grounding_retry_used,
            continuation_retry_used,
        })
        .expect("bash command should trigger retry");

        AssistantResponseRetryController::apply_decision(
            AssistantResponseRetryApplicationContext {
                decision,
                pseudo_tool_retry_used: &mut pseudo_tool_retry_used,
                filesystem_grounding_retry_used: &mut filesystem_grounding_retry_used,
                continuation_retry_used: &mut continuation_retry_used,
                trace: &trace,
                messages: &mut messages,
            },
        );

        assert!(pseudo_tool_retry_used);
        assert!(!filesystem_grounding_retry_used);
        assert!(!continuation_retry_used);
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

    struct MockProvider {
        base_url: &'static str,
        model: &'static str,
    }

    #[async_trait::async_trait]
    impl LlmProvider for MockProvider {
        async fn chat(&self, _request: ChatRequest) -> anyhow::Result<ChatResponse> {
            Err(anyhow::anyhow!("chat not used in this test"))
        }

        async fn chat_stream(
            &self,
            _request: ChatRequest,
        ) -> anyhow::Result<ChatCompletionResponseStream> {
            Err(anyhow::anyhow!("stream not used in this test"))
        }

        fn base_url(&self) -> &str {
            self.base_url
        }

        fn default_model(&self) -> &str {
            self.model
        }
    }

    #[test]
    fn local_filesystem_inspection_route_is_distinct_from_terminal_route() {
        let local_route = IntentRouter::new().route("请帮我看看桌面有没有 gex 文件夹");
        let terminal_route =
            IntentRouter::new().route("帮我看看我电脑默认的python有没有安装pygame，帮我安装一下吧");

        assert!(is_local_filesystem_inspection_route(&local_route));
        assert!(!is_local_filesystem_inspection_route(&terminal_route));
    }

    #[tokio::test]
    async fn no_tool_response_retries_when_controller_decides_to_correct() {
        let route = IntentRouter::new().route("运行 python3 app.py 看看输出");
        let evidence_ledger = EvidenceLedger::new();
        let trace = trace();
        let provider = MockProvider {
            base_url: "mock://local",
            model: "mock-model",
        };
        let tools = vec![Tool::new("bash", "run shell command")];
        let exposed_tools = exposed(&["bash"]);
        let mut pseudo_tool_retry_used = false;
        let mut filesystem_grounding_retry_used = false;
        let mut continuation_retry_used = false;
        let mut messages = Vec::new();

        let flow = AssistantResponseRetryController::handle_no_tool_response(
            NoToolAssistantResponseContext {
                content: "```bash\npython3 app.py\n```",
                route: &route,
                evidence_ledger: &evidence_ledger,
                exposed_tool_names: &exposed_tools,
                tool_calls_made: false,
                pseudo_tool_retry_used: &mut pseudo_tool_retry_used,
                filesystem_grounding_retry_used: &mut filesystem_grounding_retry_used,
                continuation_retry_used: &mut continuation_retry_used,
                provider: &provider,
                tools: &tools,
                tx: None,
                trace: &trace,
                messages: &mut messages,
            },
        )
        .await;

        assert!(matches!(flow, NoToolAssistantResponseFlow::Retry));
        assert!(pseudo_tool_retry_used);
        assert!(!filesystem_grounding_retry_used);
        assert!(!continuation_retry_used);
        assert_eq!(messages.len(), 2);
        let finished = trace.finish(TurnStatus::Completed);
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::WorkflowFallback { error }
                if error.contains("explicit bash tool-use correction")
        )));
    }

    #[tokio::test]
    async fn no_tool_response_finishes_and_emits_nonstreaming_text_chunk() {
        let route = IntentRouter::new().route("say hello");
        let evidence_ledger = EvidenceLedger::new();
        let trace = trace();
        let provider = MockProvider {
            base_url: "https://api.minimaxi.com/v1",
            model: "MiniMax-M2.7",
        };
        let tools = vec![Tool::new("bash", "run shell command")];
        let exposed_tools = exposed(&["bash"]);
        let (tx, mut rx) = mpsc::channel(1);
        let mut pseudo_tool_retry_used = false;
        let mut filesystem_grounding_retry_used = false;
        let mut continuation_retry_used = false;
        let mut messages = Vec::new();

        let flow = AssistantResponseRetryController::handle_no_tool_response(
            NoToolAssistantResponseContext {
                content: "hello",
                route: &route,
                evidence_ledger: &evidence_ledger,
                exposed_tool_names: &exposed_tools,
                tool_calls_made: false,
                pseudo_tool_retry_used: &mut pseudo_tool_retry_used,
                filesystem_grounding_retry_used: &mut filesystem_grounding_retry_used,
                continuation_retry_used: &mut continuation_retry_used,
                provider: &provider,
                tools: &tools,
                tx: Some(&tx),
                trace: &trace,
                messages: &mut messages,
            },
        )
        .await;

        assert!(matches!(flow, NoToolAssistantResponseFlow::Finish));
        assert!(!pseudo_tool_retry_used);
        assert!(!filesystem_grounding_retry_used);
        assert!(!continuation_retry_used);
        assert!(messages.is_empty());
        assert!(matches!(
            rx.recv().await,
            Some(StreamEvent::TextChunk(content)) if content == "hello"
        ));
    }
}
