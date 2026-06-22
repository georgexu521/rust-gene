//! Engine support module.
//!
//! Holds focused runtime support code used by the query engine and conversation loop.

use crate::engine::trace::{TraceEvent, TurnStatus, TurnTrace};
use crate::services::api::ToolCall;
use crate::tools::ToolResult;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExperienceRecord {
    pub task_type: String,
    pub risk: String,
    pub workflow: String,
    #[serde(default)]
    pub plan_before: Vec<String>,
    #[serde(default)]
    pub plan_after: Vec<String>,
    #[serde(default)]
    pub tools_used: Vec<ToolUseSummary>,
    #[serde(default)]
    pub tool_failures: Vec<ToolFailureSummary>,
    #[serde(default)]
    pub tests: Vec<ValidationSummary>,
    pub acceptance_status: String,
    pub repair_attempts: u32,
    pub cost: ExperienceCost,
    pub user_feedback: Option<String>,
    #[serde(default)]
    pub candidate_memories: Vec<CandidateMemoryRef>,
    #[serde(default)]
    pub candidate_skills: Vec<CandidateSkillRef>,
    pub final_outcome: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolUseSummary {
    pub name: String,
    pub success: bool,
    pub duration_ms: Option<u64>,
    pub output_chars: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolFailureSummary {
    pub tool: String,
    pub error_code: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ValidationSummary {
    pub command: Option<String>,
    pub passed: Option<bool>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExperienceCost {
    pub tokens: Option<u64>,
    pub duration_ms: Option<i64>,
    pub tool_calls: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CandidateMemoryRef {
    pub summary: String,
    pub score: Option<f32>,
    #[serde(default)]
    pub failed_strategy: Option<String>,
    #[serde(default)]
    pub better_strategy: Option<String>,
    #[serde(default)]
    pub context_tags: Vec<String>,
    #[serde(default)]
    pub failure_type: Option<String>,
    #[serde(default)]
    pub recovery_plan_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CandidateSkillRef {
    pub proposal_id: Option<String>,
    pub title: String,
    pub score: Option<f32>,
}

impl ExperienceRecord {
    pub fn from_turn_trace(trace: &TurnTrace) -> Self {
        let intent = trace.events.iter().find_map(|event| match event {
            TraceEvent::IntentRouted { intent, .. } => Some(intent.clone()),
            _ => None,
        });
        let latest_stop = trace.events.iter().rev().find_map(|event| match event {
            TraceEvent::StopCheckEvaluated {
                terminal_status,
                action,
                reason,
                summary,
                failure_type,
                recovery_plan_id,
                next_action,
                ..
            } => Some((
                terminal_status.clone(),
                action.clone(),
                reason.clone(),
                summary.clone(),
                failure_type.clone(),
                recovery_plan_id.clone(),
                next_action.clone(),
            )),
            _ => None,
        });
        let closeout_terminal = trace.events.iter().rev().find_map(|event| match event {
            TraceEvent::FinalCloseoutPrepared {
                terminal_status, ..
            } => terminal_status.clone(),
            _ => None,
        });
        let tool_calls = trace
            .events
            .iter()
            .filter(|event| matches!(event, TraceEvent::ToolCompleted { .. }))
            .count();
        let failed_tools = trace
            .events
            .iter()
            .filter_map(|event| match event {
                TraceEvent::ToolCompleted {
                    tool,
                    success: false,
                    ..
                } => Some(ToolFailureSummary {
                    tool: tool.clone(),
                    error_code: None,
                    error: Some("tool completed unsuccessfully".to_string()),
                }),
                _ => None,
            })
            .collect::<Vec<_>>();
        let terminal_status = latest_stop
            .as_ref()
            .and_then(|(terminal_status, ..)| terminal_status.clone())
            .or(closeout_terminal);
        let mut candidate_memories = Vec::new();
        if let Some((
            _terminal_status,
            action,
            reason,
            summary,
            failure_type,
            recovery_plan_id,
            next_action,
        )) = latest_stop.as_ref()
        {
            if failure_type
                .as_ref()
                .is_some_and(|value| !value.trim().is_empty())
            {
                candidate_memories.push(CandidateMemoryRef {
                    failed_strategy: failure_type.clone(),
                    better_strategy: next_action.clone().or_else(|| Some(action.clone())),
                    context_tags: vec![
                        "stop_check".to_string(),
                        "recovery".to_string(),
                        failure_type
                            .clone()
                            .unwrap_or_else(|| "unknown_failure".to_string()),
                    ],
                    failure_type: failure_type.clone(),
                    recovery_plan_id: recovery_plan_id.clone(),
                    summary: format!(
                        "failed_strategy={} reason={} better_strategy={} recovery_plan_id={}",
                        failure_type.as_deref().unwrap_or(reason),
                        summary,
                        next_action.as_deref().unwrap_or(action),
                        recovery_plan_id.as_deref().unwrap_or("none")
                    ),
                    score: Some(0.8),
                });
            }
        }

        Self {
            task_type: intent.unwrap_or_else(|| "unknown".to_string()),
            risk: "unknown".to_string(),
            workflow: "conversation_turn".to_string(),
            acceptance_status: if trace.status == TurnStatus::Completed {
                "passed".to_string()
            } else {
                "unknown".to_string()
            },
            repair_attempts: 0,
            cost: ExperienceCost {
                tokens: None,
                duration_ms: trace.duration_ms(),
                tool_calls,
            },
            final_outcome: terminal_status.unwrap_or_else(|| format!("{:?}", trace.status)),
            tool_failures: failed_tools,
            candidate_memories,
            ..Default::default()
        }
    }

    pub fn from_tool_outcome(tool_call: &ToolCall, result: &ToolResult) -> Self {
        let tool_use = ToolUseSummary {
            name: tool_call.name.clone(),
            success: result.success,
            duration_ms: result.duration_ms,
            output_chars: result.content.chars().count(),
        };
        let tool_failures = if result.success {
            Vec::new()
        } else {
            vec![ToolFailureSummary {
                tool: tool_call.name.clone(),
                error_code: None,
                error: result.error.clone(),
            }]
        };

        Self {
            task_type: "tool_use".to_string(),
            risk: "unknown".to_string(),
            workflow: "tool_outcome".to_string(),
            tools_used: vec![tool_use],
            tool_failures,
            acceptance_status: if result.success { "passed" } else { "failed" }.to_string(),
            repair_attempts: 0,
            cost: ExperienceCost {
                tokens: None,
                duration_ms: result.duration_ms.map(|v| v as i64),
                tool_calls: 1,
            },
            final_outcome: if result.success {
                "completed"
            } else {
                "failed"
            }
            .to_string(),
            ..Default::default()
        }
    }
}

pub fn attach_experience_payload(
    mut payload: serde_json::Value,
    record: ExperienceRecord,
) -> serde_json::Value {
    let Ok(record_value) = serde_json::to_value(record) else {
        return payload;
    };
    match &mut payload {
        serde_json::Value::Object(object) => {
            object.insert("experience".to_string(), record_value);
            payload
        }
        _ => serde_json::json!({ "value": payload, "experience": record_value }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attaches_experience_without_removing_existing_payload() {
        let payload = serde_json::json!({ "tool": "bash", "success": true });
        let enriched = attach_experience_payload(
            payload,
            ExperienceRecord {
                task_type: "tool_use".to_string(),
                workflow: "tool_outcome".to_string(),
                final_outcome: "completed".to_string(),
                ..Default::default()
            },
        );
        assert_eq!(enriched["tool"], "bash");
        assert_eq!(enriched["experience"]["workflow"], "tool_outcome");
    }
}
