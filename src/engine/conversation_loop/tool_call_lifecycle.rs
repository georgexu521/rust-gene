use crate::services::api::ToolCall;
use crate::tools::ToolResult;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ToolCallStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Denied,
    ProviderExecuted,
}

#[derive(Debug, Clone)]
pub(super) struct ToolCallLifecycleRecord {
    pub(super) status: ToolCallStatus,
    pub(super) tool_name: String,
    pub(super) parallel: bool,
    pub(super) pre_executed: bool,
}

#[derive(Debug, Default, Clone)]
pub(super) struct ToolCallLifecycle {
    records: HashMap<String, ToolCallLifecycleRecord>,
}

impl ToolCallLifecycle {
    pub(super) fn pending_batch(&mut self, tool_calls: &[ToolCall]) {
        for tool_call in tool_calls {
            if tool_call.id.is_empty() || tool_call.name.is_empty() {
                continue;
            }
            self.records.insert(
                tool_call.id.clone(),
                ToolCallLifecycleRecord {
                    status: ToolCallStatus::Pending,
                    tool_name: tool_call.name.clone(),
                    parallel: false,
                    pre_executed: false,
                },
            );
        }
    }

    pub(super) fn running(&mut self, tool_call: &ToolCall, parallel: bool, pre_executed: bool) {
        self.update(tool_call, ToolCallStatus::Running, parallel, pre_executed);
    }

    pub(super) fn completed(&mut self, tool_call: &ToolCall, result: &ToolResult) {
        let status = if result.success {
            ToolCallStatus::Completed
        } else {
            ToolCallStatus::Failed
        };
        let (parallel, pre_executed) = self
            .records
            .get(&tool_call.id)
            .map(|record| (record.parallel, record.pre_executed))
            .unwrap_or((false, false));
        self.update(tool_call, status, parallel, pre_executed);
    }

    pub(super) fn denied(&mut self, tool_call: &ToolCall) {
        self.update(tool_call, ToolCallStatus::Denied, false, false);
    }

    pub(super) fn provider_executed(&mut self, tool_call: &ToolCall, result: &ToolResult) {
        let status = if result.success {
            ToolCallStatus::ProviderExecuted
        } else {
            ToolCallStatus::Failed
        };
        self.update(tool_call, status, true, true);
    }

    #[cfg(test)]
    pub(super) fn snapshot(&self) -> Vec<(String, ToolCallLifecycleRecord)> {
        let mut records = self
            .records
            .iter()
            .map(|(id, record)| (id.clone(), record.clone()))
            .collect::<Vec<_>>();
        records.sort_by(|left, right| left.0.cmp(&right.0));
        records
    }

    pub(super) fn snapshot_for(
        &self,
        tool_calls: &[ToolCall],
    ) -> Vec<(String, ToolCallLifecycleRecord)> {
        let ids = tool_calls
            .iter()
            .map(|tool_call| tool_call.id.as_str())
            .collect::<HashSet<_>>();
        let mut records = self
            .records
            .iter()
            .filter(|(id, _)| ids.contains(id.as_str()))
            .map(|(id, record)| (id.clone(), record.clone()))
            .collect::<Vec<_>>();
        records.sort_by(|left, right| left.0.cmp(&right.0));
        records
    }

    fn update(
        &mut self,
        tool_call: &ToolCall,
        status: ToolCallStatus,
        parallel: bool,
        pre_executed: bool,
    ) {
        if tool_call.id.is_empty() || tool_call.name.is_empty() {
            return;
        }
        self.records.insert(
            tool_call.id.clone(),
            ToolCallLifecycleRecord {
                status,
                tool_name: tool_call.name.clone(),
                parallel,
                pre_executed,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool_call(id: &str, name: &str) -> ToolCall {
        ToolCall {
            id: id.to_string(),
            name: name.to_string(),
            arguments: serde_json::json!({}),
        }
    }

    #[test]
    fn tracks_pending_running_and_completed_tool_calls() {
        let call = tool_call("call_1", "bash");
        let mut lifecycle = ToolCallLifecycle::default();

        lifecycle.pending_batch(std::slice::from_ref(&call));
        assert_eq!(lifecycle.snapshot()[0].1.status, ToolCallStatus::Pending);

        lifecycle.running(&call, false, false);
        assert_eq!(lifecycle.snapshot()[0].1.status, ToolCallStatus::Running);

        lifecycle.completed(&call, &ToolResult::success("ok"));
        assert_eq!(lifecycle.snapshot()[0].1.status, ToolCallStatus::Completed);
    }

    #[test]
    fn tracks_denied_and_provider_executed_tool_calls() {
        let denied = tool_call("call_1", "file_write");
        let pre_executed = tool_call("call_2", "file_read");
        let mut lifecycle = ToolCallLifecycle::default();

        lifecycle.denied(&denied);
        lifecycle.provider_executed(&pre_executed, &ToolResult::success("read"));

        let snapshot = lifecycle.snapshot();
        assert_eq!(snapshot[0].1.status, ToolCallStatus::Denied);
        assert_eq!(snapshot[1].1.status, ToolCallStatus::ProviderExecuted);
        assert!(snapshot[1].1.parallel);
        assert!(snapshot[1].1.pre_executed);
    }

    #[test]
    fn snapshot_for_limits_records_to_current_batch() {
        let previous = tool_call("call_previous", "file_read");
        let current = tool_call("call_current", "grep");
        let mut lifecycle = ToolCallLifecycle::default();

        lifecycle.provider_executed(&previous, &ToolResult::success("cached"));
        lifecycle.pending_batch(std::slice::from_ref(&current));

        let snapshot = lifecycle.snapshot_for(std::slice::from_ref(&current));

        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].0, "call_current");
        assert_eq!(snapshot[0].1.status, ToolCallStatus::Pending);
    }
}
