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
/// 从 trace 事件中提取当前 stage、风险等级、最近动作分数。
pub fn inject_task_guidance(
    messages: &mut Vec<Message>,
    trace: &TraceCollector,
) {
    if !task_guidance_enabled() {
        return;
    }

    let snapshot = trace.snapshot();
    let events = &snapshot.events;

    let stage = events
        .iter()
        .rev()
        .find_map(|event| {
            if let TraceEvent::ActionDecisionEvaluated {
                formula_stage, phase_aligned, ..
            } = event
            {
                let aligned = if *phase_aligned { "" } else { " misaligned" };
                Some(format!("{}{}", formula_stage, aligned))
            } else {
                None
            }
        });

    let risk = events
        .iter()
        .rev()
        .find_map(|event| {
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

    let mut lines: Vec<String> = Vec::with_capacity(3);
    if let Some(s) = stage {
        lines.push(format!("stage={}", s));
    }
    if let Some(r) = risk {
        lines.push(format!("risk={}", r));
    }

    if lines.is_empty() {
        return;
    }

    let block = format!(
        "<task-guidance>\n{}\n</task-guidance>",
        lines.join("\n")
    );

    let insert_pos = messages
        .iter()
        .position(|msg| matches!(msg, Message::User { .. }))
        .unwrap_or(0);
    messages.insert(
        insert_pos.min(messages.len()),
        Message::system(format!(
            "<recent_observation>\n{}\n</recent_observation>",
            block
        )),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_disabled_without_env() {
        assert!(
            !task_guidance_enabled()
                || std::env::var("PRIORITY_AGENT_TASK_GUIDANCE").is_ok()
        );
    }
}
