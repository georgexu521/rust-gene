//! 任务引导块：≤4 行的运行时事实注入，帮助 LLM 聚焦当前最重要的步骤。
//!
//! 由 `PRIORITY_AGENT_TASK_GUIDANCE` 环境变量控制（默认关闭）。
//! 从 trace 事件中提取关键事实，不引入新的评分公式。
//!
//! 设计约束：
//! - 不超过 4 行，超过不注入。
//! - 不写"必须/禁止/应当"这类新规则，只给事实。
//! - 不暴露完整公式，避免 LLM 过拟合分数。

use crate::engine::trace::{TraceCollector, TraceEvent};
use crate::services::api::Message;

/// 是否启用了 task-guidance 注入。
pub fn task_guidance_enabled() -> bool {
    matches!(
        std::env::var("PRIORITY_AGENT_TASK_GUIDANCE")
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str(),
        "1" | "true" | "yes" | "on"
    )
}

/// 组装并注入 task-guidance 到请求消息。
///
/// 从 trace 事件中提取当前 stage、top plan step、风险等级、最近动作分数。
pub fn inject_task_guidance(messages: &mut Vec<Message>, trace: &TraceCollector) {
    if !task_guidance_enabled() {
        return;
    }

    let snapshot = trace.snapshot();
    let events = &snapshot.events;

    let stage = events.iter().rev().find_map(|event| {
        if let TraceEvent::ActionDecisionEvaluated {
            formula_stage,
            phase_aligned,
            ..
        } = event
        {
            let aligned = if *phase_aligned { "" } else { " misaligned" };
            Some(format!("{}{}", formula_stage, aligned))
        } else {
            None
        }
    });

    let top_plan_step = events.iter().rev().find_map(|event| {
        if let TraceEvent::WorkflowPlanProgress {
            active_step,
            top_priority,
            top_importance_score,
            top_weight_share,
            weight_source,
            ..
        } = event
        {
            let step = active_step.as_ref().or(top_priority.as_ref())?;
            let mut parts = vec![format!("\"{}\"", compact_fact(step, 96))];
            if let Some(score) = top_importance_score {
                parts.push(format!("importance={score:.2}"));
            }
            if let Some(share) = top_weight_share {
                parts.push(format!("share={share:.2}"));
            }
            if let Some(source) = weight_source.as_ref().filter(|source| !source.is_empty()) {
                parts.push(format!("source={}", compact_fact(source, 40)));
            }
            Some(parts.join(" "))
        } else {
            None
        }
    });

    let risk = events.iter().rev().find_map(|event| {
        if let TraceEvent::RiskSignalAssessed { level, reasons, .. } = event {
            if level != "ordinary" {
                let reason_text = if reasons.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", reasons.join("; "))
                };
                Some(format!("{}{}", level, reason_text))
            } else {
                None
            }
        } else {
            None
        }
    });

    let recent_action = events.iter().rev().find_map(|event| {
        if let TraceEvent::ActionDecisionEvaluated {
            tool,
            action_score,
            uncertainty_reduction,
            risk,
            reason,
            ..
        } = event
        {
            let mut action = format!("\"{}\" score={}", compact_fact(tool, 40), action_score);
            if *risk >= 7 {
                action.push_str(" high_risk");
            }
            if *uncertainty_reduction <= 2 {
                action.push_str(" low_evidence");
            }
            if !reason.trim().is_empty() {
                action.push_str(&format!(" \"{}\"", compact_fact(reason, 80)));
            }
            Some(action)
        } else {
            None
        }
    });

    // 记忆信号：最近一次写入决策 + 召回统计
    let memory_signal = events.iter().rev().find_map(|event| {
        if let TraceEvent::MemoryWriteScored {
            ref status,
            score,
            ref reason,
            ..
        } = event
        {
            let label = match status.as_str() {
                "rejected" => format!("rejected({:.2})", score),
                "proposed" => format!("proposed({:.2})", score),
                "accepted" => format!("accepted({:.2})", score),
                _ => format!("{}({:.2})", status, score),
            };
            Some(format!("memory_write={} {}", label, compact_fact(reason, 60)))
        } else {
            None
        }
    });

    let mut lines: Vec<String> = Vec::with_capacity(6);
    if let Some(s) = stage {
        lines.push(format!("stage={}", s));
    }
    if let Some(step) = top_plan_step {
        lines.push(format!("top_plan_step={}", step));
    }
    if let Some(r) = risk {
        lines.push(format!("risk={}", r));
    }
    if let Some(action) = recent_action {
        lines.push(format!("recent_action={}", action));
    }
    if let Some(mem) = memory_signal {
        lines.push(format!("memory={}", mem));
    }

    // 严格 ≤4 行：优先保留 stage + top_plan_step + risk，截断尾部
    if lines.len() > 4 {
        lines.truncate(4);
    }

    if lines.is_empty() {
        return;
    }

    let block = format!("<task-guidance>\n{}\n</task-guidance>", lines.join("\n"));

    let Some(Message::User { content }) = messages
        .iter_mut()
        .rev()
        .find(|message| matches!(message, Message::User { .. }))
    else {
        return;
    };
    let original = std::mem::take(content);
    *content = format!(
        "<recent_observation>\n{}\n</recent_observation>\n\n{}",
        block, original
    );
}

fn compact_fact(value: &str, max_chars: usize) -> String {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= max_chars {
        return compact;
    }
    let mut truncated = compact
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect::<String>();
    truncated.push_str("...");
    truncated
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::trace::TurnTrace;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn default_disabled_without_env() {
        let _guard = env_lock().lock().unwrap();
        let previous = std::env::var("PRIORITY_AGENT_TASK_GUIDANCE").ok();
        std::env::remove_var("PRIORITY_AGENT_TASK_GUIDANCE");
        assert!(
            !task_guidance_enabled(),
            "task guidance should be off unless explicitly enabled"
        );
        if let Some(value) = previous {
            std::env::set_var("PRIORITY_AGENT_TASK_GUIDANCE", value);
        }
    }

    #[test]
    fn enabled_injects_fact_block_into_last_user_message() {
        let _guard = env_lock().lock().unwrap();
        let previous = std::env::var("PRIORITY_AGENT_TASK_GUIDANCE").ok();
        std::env::set_var("PRIORITY_AGENT_TASK_GUIDANCE", "1");

        let trace = TraceCollector::new(TurnTrace::new("session".to_string(), 1, "test"));
        trace.record(TraceEvent::WorkflowPlanProgress {
            total_steps: 2,
            completed_steps: 0,
            active_step: Some("implement weighted trace recording".to_string()),
            top_priority: None,
            top_importance_score: Some(0.91),
            top_weight_share: Some(0.62),
            weight_source: Some("workflow_contract".to_string()),
            reweighted: true,
        });
        trace.record(TraceEvent::RiskSignalAssessed {
            phase: "implementation".to_string(),
            level: "elevated".to_string(),
            entry_contract: true,
            reasons: vec!["mutating code".to_string()],
        });
        trace.record(TraceEvent::ActionDecisionEvaluated {
            tool: "file_read".to_string(),
            call_id: "call-1".to_string(),
            stage: "implementation".to_string(),
            value: 4,
            risk: 1,
            uncertainty_reduction: 2,
            cost: 1,
            reversibility: 8,
            scope_fit: 5,
            action_score: 6,
            formula_stage: "Implementation".to_string(),
            formula_version: "v1".to_string(),
            phase_aligned: true,
            mutates_workspace: false,
            broad_shell: false,
            modifiers: Vec::new(),
            requires_confirmation: false,
            reason: "low uncertainty reduction".to_string(),
        });

        let mut messages = vec![Message::system("stable"), Message::user("Please continue")];

        inject_task_guidance(&mut messages, &trace);

        assert_eq!(
            messages.len(),
            2,
            "guidance should not add a system message"
        );
        let Message::User { content } = &messages[1] else {
            panic!("expected user message");
        };
        assert!(content.contains("<task-guidance>"));
        assert!(content.contains("stage=Implementation"));
        assert!(content.contains("top_plan_step=\"implement weighted trace recording\" importance=0.91 share=0.62 source=workflow_contract"));
        assert!(content.contains("risk=elevated (mutating code)"));
        assert!(content.contains("recent_action=\"file_read\" score=6 low_evidence \"low uncertainty reduction\""));

        if let Some(value) = previous {
            std::env::set_var("PRIORITY_AGENT_TASK_GUIDANCE", value);
        } else {
            std::env::remove_var("PRIORITY_AGENT_TASK_GUIDANCE");
        }
    }
}
