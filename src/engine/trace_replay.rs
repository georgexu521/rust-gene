//! Trace replay — lightweight event sourcing for debugging.
//!
//! Reconstructs a decision timeline from `TraceEvent`s collected during
//! a conversation turn. Provides JSON export for external analysis.

use crate::engine::trace::TraceEvent;
use serde::Serialize;

/// A reconstructed decision point from the trace.
#[derive(Debug, Clone, Serialize)]
pub struct DecisionPoint {
    pub event_type: String,
    pub summary: String,
}

/// A replayed timeline of decisions made during a conversation turn.
#[derive(Debug, Clone, Serialize)]
pub struct DecisionTimeline {
    pub turn_count: u64,
    pub decisions: Vec<DecisionPoint>,
}

impl DecisionTimeline {
    /// Reconstruct a decision timeline from a sequence of trace events.
    pub fn from_events(turn_count: u64, events: &[TraceEvent]) -> Self {
        let decisions: Vec<DecisionPoint> = events
            .iter()
            .filter_map(|event| {
                let (event_type, summary) = match event {
                    TraceEvent::IntentRouted {
                        intent,
                        risk,
                        reason,
                        ..
                    } => (
                        "intent_routed",
                        format!("Intent: {} (risk: {}) — {}", intent, risk, reason),
                    ),
                    TraceEvent::ResourcePolicySelected {
                        latency,
                        context_budget_tokens,
                        reason,
                        ..
                    } => (
                        "resource_policy",
                        format!(
                            "Policy: {} latency, {} token budget — {}",
                            latency, context_budget_tokens, reason
                        ),
                    ),
                    TraceEvent::ToolStarted { tool, call_id, .. } => (
                        "tool_started",
                        format!("Tool: {} (call: {})", tool, call_id),
                    ),
                    TraceEvent::ToolCompleted {
                        tool,
                        call_id,
                        success,
                        duration_ms,
                        output_chars,
                    } => (
                        "tool_completed",
                        format!(
                            "Tool done: {} (call: {}, {} chars, {:?}ms, {})",
                            tool,
                            call_id,
                            output_chars,
                            duration_ms,
                            if *success { "ok" } else { "failed" }
                        ),
                    ),
                    _ => return None,
                };
                Some(DecisionPoint {
                    event_type: event_type.to_string(),
                    summary,
                })
            })
            .collect();

        Self {
            turn_count,
            decisions,
        }
    }

    /// Export the timeline as pretty-printed JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// One-line summary of key decisions.
    pub fn summary(&self) -> String {
        let items: Vec<String> = self
            .decisions
            .iter()
            .filter(|d| {
                d.event_type == "intent_routed"
                    || d.event_type == "plan_submitted"
                    || d.event_type == "compression"
                    || d.event_type == "resource_policy"
            })
            .map(|d| d.summary.clone())
            .collect();
        if items.is_empty() {
            format!("Turn {}: no key decisions recorded", self.turn_count)
        } else {
            format!("Turn {}:\n  {}", self.turn_count, items.join("\n  "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_events_produces_empty_timeline() {
        let timeline = DecisionTimeline::from_events(1, &[]);
        assert!(timeline.decisions.is_empty());
    }

    #[test]
    fn tool_events_are_tracked() {
        let events = vec![
            TraceEvent::ToolStarted {
                tool: "file_read".into(),
                call_id: "c1".into(),
                parallel: false,
                pre_executed: false,
            },
            TraceEvent::ToolCompleted {
                tool: "file_read".into(),
                call_id: "c1".into(),
                success: true,
                duration_ms: Some(42),
                output_chars: 100,
            },
        ];
        let timeline = DecisionTimeline::from_events(1, &events);
        assert_eq!(timeline.decisions.len(), 2);
        assert!(timeline.decisions[0].summary.contains("file_read"));
    }

    #[test]
    fn timeline_json_is_valid() {
        let events = vec![TraceEvent::IntentRouted {
            agent_mode: None,
            intent: "code".into(),
            workflow: "coding".into(),
            retrieval: "semantic".into(),
            confidence: 0.9,
            risk: "low".into(),
            reason: "user asked to write code".into(),
        }];
        let timeline = DecisionTimeline::from_events(0, &events);
        let json = timeline.to_json();
        assert!(json.contains("intent_routed"));
    }
}
