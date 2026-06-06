//! Tool settlement ledger.
//!
//! Gap 7 (opencode core alignment): every tool call that starts must
//! settle as completed, failed, cancelled, or provider-executed before
//! a turn can close. Incomplete settlement blocks verified closeout.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Settlement status for a tool call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolSettlement {
    Pending,
    Completed,
    Failed,
    Cancelled,
    ProviderExecuted,
}

impl ToolSettlement {
    pub fn is_settled(self) -> bool {
        !matches!(self, Self::Pending)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::ProviderExecuted => "provider_executed",
        }
    }
}

/// Per-turn ledger of tool settlements.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolSettlementLedger {
    /// Tool calls that started this turn, keyed by tool_call_id.
    entries: HashMap<String, ToolSettlementEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSettlementEntry {
    pub tool_call_id: String,
    pub tool_name: String,
    pub settlement: ToolSettlement,
    pub error: Option<String>,
}

impl ToolSettlementLedger {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a tool input start.
    pub fn start_tool(&mut self, tool_call_id: &str, tool_name: &str) {
        self.entries.insert(
            tool_call_id.to_string(),
            ToolSettlementEntry {
                tool_call_id: tool_call_id.to_string(),
                tool_name: tool_name.to_string(),
                settlement: ToolSettlement::Pending,
                error: None,
            },
        );
    }

    /// Settle a tool as completed.
    pub fn complete_tool(&mut self, tool_call_id: &str) {
        if let Some(entry) = self.entries.get_mut(tool_call_id) {
            entry.settlement = ToolSettlement::Completed;
        }
    }

    /// Settle a tool as failed.
    pub fn fail_tool(&mut self, tool_call_id: &str, error: &str) {
        if let Some(entry) = self.entries.get_mut(tool_call_id) {
            entry.settlement = ToolSettlement::Failed;
            entry.error = Some(error.to_string());
        }
    }

    /// Settle a tool as cancelled (interrupted, timeout).
    pub fn cancel_tool(&mut self, tool_call_id: &str) {
        if let Some(entry) = self.entries.get_mut(tool_call_id) {
            entry.settlement = ToolSettlement::Cancelled;
        }
    }

    /// Settle a tool as provider-executed (model called external tool).
    pub fn provider_executed(&mut self, tool_call_id: &str) {
        if let Some(entry) = self.entries.get_mut(tool_call_id) {
            entry.settlement = ToolSettlement::ProviderExecuted;
        }
    }

    /// Whether all started tools have settled.
    pub fn all_settled(&self) -> bool {
        self.entries
            .values()
            .all(|entry| entry.settlement.is_settled())
    }

    /// Count of unsettled tools.
    pub fn unsettled_count(&self) -> usize {
        self.entries
            .values()
            .filter(|entry| !entry.settlement.is_settled())
            .count()
    }

    /// List unsettled tool IDs (for diagnostics).
    pub fn unsettled_tools(&self) -> Vec<String> {
        self.entries
            .iter()
            .filter(|(_, entry)| !entry.settlement.is_settled())
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Settlement gap summary for closeout.
    pub fn gap_summary(&self) -> String {
        let unsettled = self.unsettled_tools();
        if unsettled.is_empty() {
            "all tools settled".to_string()
        } else {
            format!(
                "{} unsettled tool(s): {}",
                unsettled.len(),
                unsettled.join(", ")
            )
        }
    }

    /// Clear the ledger for the next turn.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracks_tool_lifecycle() {
        let mut ledger = ToolSettlementLedger::new();
        assert!(ledger.all_settled());

        ledger.start_tool("call-1", "bash");
        assert!(!ledger.all_settled());
        assert_eq!(ledger.unsettled_count(), 1);

        ledger.complete_tool("call-1");
        assert!(ledger.all_settled());
    }

    #[test]
    fn reports_unsettled_gaps() {
        let mut ledger = ToolSettlementLedger::new();
        ledger.start_tool("call-1", "bash");
        ledger.start_tool("call-2", "grep");
        ledger.complete_tool("call-1");

        assert!(!ledger.all_settled());
        assert_eq!(ledger.unsettled_count(), 1);
        let summary = ledger.gap_summary();
        assert!(summary.contains("unsettled"));
        assert!(summary.contains("call-2"));
    }

    #[test]
    fn clears_for_next_turn() {
        let mut ledger = ToolSettlementLedger::new();
        ledger.start_tool("call-1", "bash");
        ledger.clear();
        assert!(ledger.all_settled());
    }
}
