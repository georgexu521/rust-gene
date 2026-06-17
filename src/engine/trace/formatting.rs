//! 跟踪格式化
//!
//! 将跟踪数据格式化为人类可读的文本，用于：
//! - 调试输出
//! - 诊断报告
//! - 用户界面显示

use super::{
    action_review_trace_summary, control_loop_diagnostic, latest_runtime_diet_summary,
    latest_tool_record_count, latest_tool_record_evidence_summary, scoring_trace_summary, short_id,
    TurnTrace,
};

/// 格式化跟踪摘要
pub fn format_trace_summary(trace: &TurnTrace, max_events: usize) -> String {
    let duration = trace
        .duration_ms()
        .map(|ms| format!("{}ms", ms))
        .unwrap_or_else(|| "running".to_string());
    let mut lines = vec![format!(
        "Trace {}\nSession: {}\nTurn: {}\nStatus: {:?}\nDuration: {}\nPrompt: {}",
        short_id(&trace.trace_id),
        trace.session_id,
        trace.turn_index,
        trace.status,
        duration,
        trace.user_message_preview
    )];
    if let Some(diet) = latest_runtime_diet_summary(trace) {
        lines.push(format!("\nRuntime Diet: {}", diet));
    }
    if let Some(tool_record_evidence) = latest_tool_record_evidence_summary(trace) {
        lines.push(format!("\nTool Record Evidence: {}", tool_record_evidence));
    }
    lines.push(format!(
        "\nControl Loop: {}",
        control_loop_diagnostic(trace).compact_summary()
    ));
    if let Some(action_reviews) = action_review_trace_summary(trace) {
        lines.push(format!(
            "\nAction Reviews: {}",
            action_reviews.compact_summary()
        ));
    }
    if let Some(scoring) = scoring_trace_summary(trace) {
        lines.push(format!("\nScoring: {}", scoring.compact_summary()));
    }

    lines.push("\nEvents:".to_string());
    for (idx, event) in trace.events.iter().take(max_events).enumerate() {
        lines.push(format!(
            "{:>2}. {:<20} {}",
            idx + 1,
            event.label(),
            event.summary()
        ));
    }
    if trace.events.len() > max_events {
        lines.push(format!(
            "... {} more events",
            trace.events.len().saturating_sub(max_events)
        ));
    }

    lines.join("\n")
}

pub fn format_trace_recent_line(trace: &TurnTrace) -> String {
    let action_review_summary = action_review_trace_summary(trace)
        .map(|summary| format!(" action_reviews={}", summary.compact_summary()))
        .unwrap_or_default();
    format!(
        "- {} turn {} {:?} events={} tool_records={}{} prompt={}",
        short_id(&trace.trace_id),
        trace.turn_index,
        trace.status,
        trace.events.len(),
        latest_tool_record_count(trace).unwrap_or(0),
        action_review_summary,
        trace.user_message_preview
    )
}
