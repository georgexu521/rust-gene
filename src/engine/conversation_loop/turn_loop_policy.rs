use crate::engine::intent_router::{IntentRoute, RetrievalPolicy, RiskLevel, WorkflowKind};
use crate::engine::streaming::StreamEvent;
use crate::engine::trace::{TraceCollector, TraceEvent};
use crate::services::api::{ChatRequest, LlmProvider, Message};
use regex::Regex;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

// ---------------------------------------------------------------------------
// MainLoopProfile
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MainLoopProfile {
    QuietDirect,
    Standard,
}

impl MainLoopProfile {
    /// Request-shaping only. QuietDirect still enters the model loop; it just
    /// suppresses optional tools/dynamic context for low-risk direct turns.
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
        _repair_attempts: usize,
    ) -> usize {
        if self.is_quiet_direct() {
            1
        } else {
            configured_max
        }
    }
}

// ---------------------------------------------------------------------------
// ForceSummary
// ---------------------------------------------------------------------------

const FORCE_SUMMARY_MAX_TOKENS: u32 = 8192;
const FORCE_SUMMARY_INSTRUCTION: &str = "The turn is being force-summarized because the runtime reached a stuck-state or context guard. Summarize in plain prose what you learned from the tool results above. Do NOT emit any tool calls, function-call markup, DSML invocations, or SEARCH/REPLACE edit blocks; they will be discarded. Just plain text.";

/// Why the turn was forcefully terminated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Aborted + ContextGuard are API-ready for future callers
pub enum ForceSummaryReason {
    /// User explicitly aborted (Esc / /abort).
    Aborted,
    /// Context token budget exceeded the guard threshold.
    ContextGuard,
    /// Iteration limit hit — model was stuck in a tool-call loop.
    Stuck,
}

impl ForceSummaryReason {
    /// Human-readable prefix for the summary message.
    pub fn prefix(&self) -> &'static str {
        match self {
            ForceSummaryReason::Aborted => {
                "The task was interrupted by the user. Below is a summary of progress so far."
            }
            ForceSummaryReason::ContextGuard => {
                "The context window is nearly full. Below is a summary of what was accomplished."
            }
            ForceSummaryReason::Stuck => {
                "The iteration limit was reached. Below is a summary of progress so far."
            }
        }
    }
}

/// Check whether the conversation should be forced to wrap up.
///
/// Returns `true` when the current iteration is within the last 2 allowed
/// iterations, meaning the model should stop starting new multi-step tasks
/// and instead produce a final summary.
pub fn should_force_summary(iteration: usize, max_iterations: usize) -> bool {
    iteration >= max_iterations.saturating_sub(2).max(1)
}

/// Generate the force-summary system message to inject into the conversation.
pub fn force_summary_message() -> Message {
    force_summary_message_with_reason(ForceSummaryReason::Stuck)
}

/// Generate a force-summary system message with a specific reason prefix.
pub fn force_summary_message_with_reason(reason: ForceSummaryReason) -> Message {
    let prefix = reason.prefix();
    Message::System {
        content: format!(
            r#"<wrap-up>
{prefix}
Do NOT start any new multi-step task or call tools that would require follow-up work. Instead:
1. Summarize what has been accomplished so far.
2. List any remaining work clearly so the user can continue in a new session.
3. If there are uncommitted changes, describe them.
4. End your response after this summary — limit tool calls to only those needed for the summary above.
</wrap-up>"#
        ),
    }
}

pub(super) struct ForceSummaryAfterLimitContext<'a> {
    pub(super) provider: Arc<dyn LlmProvider>,
    pub(super) model: &'a str,
    pub(super) messages: &'a mut Vec<Message>,
    pub(super) trace: &'a TraceCollector,
    pub(super) tx: Option<&'a mpsc::Sender<StreamEvent>>,
    pub(super) cost_tracker: &'a Arc<Mutex<crate::cost_tracker::CostTracker>>,
    pub(super) reason: ForceSummaryReason,
}

/// Make one final no-tools summarization call and append the result to the
/// turn content.
pub(super) async fn force_summary_after_iter_limit(
    context: ForceSummaryAfterLimitContext<'_>,
) -> String {
    context.trace.record(TraceEvent::WorkflowFallback {
        error: "iteration cap reached; forcing no-tools summary".to_string(),
    });

    let mut messages = context.messages.clone();
    messages.push(Message::user(FORCE_SUMMARY_INSTRUCTION));
    tracing::debug!(message_count = messages.len(), "forcing no-tools summary");

    let mut request = ChatRequest::new(context.model).with_messages(messages);
    request.max_tokens = Some(FORCE_SUMMARY_MAX_TOKENS);
    request.tools = None;
    request.tool_choice = None;

    match context.provider.chat(request).await {
        Ok(response) => {
            if let Some(usage) = &response.usage {
                {
                    let mut tracker = context.cost_tracker.lock().await;
                    tracker.record_api_call_with_cache_write(
                        context.model,
                        usage.prompt_tokens as u64,
                        usage.completion_tokens as u64,
                        usage.cached_tokens.map(|tokens| tokens as u64),
                        usage.cache_write_tokens.map(|tokens| tokens as u64),
                    );
                }
                if let Some(tx) = context.tx {
                    let _ = tx
                        .send(StreamEvent::Usage {
                            prompt_tokens: usage.prompt_tokens,
                            completion_tokens: usage.completion_tokens,
                            reasoning_tokens: usage.reasoning_tokens,
                            cached_tokens: usage.cached_tokens,
                            cache_write_tokens: usage.cache_write_tokens,
                        })
                        .await;
                }
            }
            if super::session_processor::finish_reason_indicates_length(
                response.finish_reason.as_deref(),
            ) {
                if let Some(tx) = context.tx {
                    let _ = tx.send(StreamEvent::OutputTruncated).await;
                }
            }

            let cleaned = strip_hallucinated_tool_markup(&response.content);
            tracing::debug!(
                cleaned_len = cleaned.len(),
                "force summary response received"
            );
            let summary = if cleaned.trim().is_empty() {
                "The model did not produce a usable summary after the runtime stopped the repeated tool loop.".to_string()
            } else {
                cleaned
            };
            let annotated = format!("{}\n\n{}", context.reason.prefix(), summary.trim());
            context.messages.push(Message::assistant(summary.clone()));
            context.trace.record(TraceEvent::WorkflowFallback {
                error: "forced no-tools summary completed".to_string(),
            });
            annotated
        }
        Err(err) => {
            let fallback = format!(
                "{}\n\n[Forced summary failed: {}. Review the last tool results and continue if the task is not complete.]",
                context.reason.prefix(),
                err
            );
            context.trace.record(TraceEvent::Error {
                message: format!("forced no-tools summary failed: {err}"),
            });
            fallback
        }
    }
}

fn strip_hallucinated_tool_markup(content: &str) -> String {
    let mut output = crate::services::api::sanitize_assistant_content(content);
    let patterns = [
        r"(?s)〈DSML｜function_calls〉.*?〈/DSML｜function_calls〉",
        r"(?s)<\|DSML\|function_calls>.*?</\|DSML\|function_calls>",
        r"(?is)<(?:minimax:)?tool_call\b[^>]*>.*?</(?:minimax:)?tool_call>",
    ];
    for pattern in patterns {
        let re = Regex::new(pattern).expect("valid force-summary cleanup regex");
        output = re.replace_all(&output, "").to_string();
    }
    output.trim().to_string()
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

    #[test]
    fn standard_profile_respects_configured_reasonix_cap_without_extra_repair_rounds() {
        assert_eq!(MainLoopProfile::Standard.max_loop_iterations(50, 9), 50);
    }

    #[test]
    fn force_summary_in_last_two_iterations() {
        assert!(!should_force_summary(0, 10));
        assert!(!should_force_summary(7, 10));
        assert!(should_force_summary(8, 10));
        assert!(should_force_summary(9, 10));
    }

    #[test]
    fn force_summary_small_max() {
        // max_iterations = 3 → last 2: iteration 1, 2
        assert!(!should_force_summary(0, 3));
        assert!(should_force_summary(1, 3));
        assert!(should_force_summary(2, 3));

        // max_iterations = 1 → only one chance, don't force-summarize on iteration 0
        assert!(!should_force_summary(0, 1));
    }

    #[test]
    fn force_summary_message_is_system() {
        let msg = force_summary_message();
        assert!(matches!(msg, Message::System { .. }));
        let content = match &msg {
            Message::System { content } => content,
            _ => "",
        };
        assert!(content.contains("wrap-up"));
    }

    #[test]
    fn force_summary_strips_hallucinated_tool_markup() {
        let cleaned = strip_hallucinated_tool_markup(
            "Done\n〈DSML｜function_calls〉call〈/DSML｜function_calls〉\n<tool_call>{}</tool_call>",
        );
        assert_eq!(cleaned, "Done");
    }

    #[test]
    fn force_summary_uses_full_answer_output_cap() {
        assert_eq!(FORCE_SUMMARY_MAX_TOKENS, 8192);
    }
}
