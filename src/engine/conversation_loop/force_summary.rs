//! Force summary / wrap-up controller.
//!
//! When the conversation is approaching the iteration or token budget limit,
//! inject a wrap-up instruction that tells the model to summarize its work
//! and avoid calling new tools unless strictly necessary for the summary.
//!
//! Mirrors Reasonix's `ForceSummaryReason` enum in `loop/force-summary.ts`.

use crate::services::api::Message;

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
    // Floor at 1 so single-iteration tasks don't get force-summarized
    // before their first (and only) attempt.
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
