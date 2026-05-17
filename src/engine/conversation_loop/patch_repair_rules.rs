use super::{ConversationLoop, PatchSynthesisAction};
use crate::services::api::ToolCall;

type RepairRuleApply = fn(&ConversationLoop, &str, &std::path::Path) -> Vec<PatchSynthesisAction>;

struct RepairRule {
    id: &'static str,
    owner: &'static str,
    review_after: &'static str,
    apply: RepairRuleApply,
}

pub(super) fn deterministic_patch_tool_calls(
    loop_state: &ConversationLoop,
    evidence: &str,
    cwd: &std::path::Path,
) -> Vec<ToolCall> {
    let lower_evidence = evidence.to_lowercase();
    let mut actions = Vec::new();

    for rule in repair_rules() {
        let before = actions.len();
        actions.extend((rule.apply)(loop_state, &lower_evidence, cwd));
        if actions.len() > before {
            tracing::debug!(
                rule_id = rule.id,
                owner = rule.owner,
                review_after = rule.review_after,
                added_actions = actions.len() - before,
                "deterministic patch repair rule matched"
            );
        }
    }

    actions
        .iter()
        .filter_map(|action| loop_state.validate_patch_synthesis_action(action, cwd).ok())
        .take(6)
        .collect()
}

fn repair_rules() -> &'static [RepairRule] {
    &[
        RepairRule {
            id: "rust-e0596-ref-mut",
            owner: "conversation-loop",
            review_after: "2026-06-01",
            apply: rust_e0596_rule,
        },
        RepairRule {
            id: "persistent-memory-planning-prefetch",
            owner: "memory-routing",
            review_after: "2026-06-01",
            apply: persistent_memory_planning_rule,
        },
        RepairRule {
            id: "live-eval-dashboard-summary",
            owner: "live-eval",
            review_after: "2026-06-01",
            apply: live_eval_dashboard_summary_rule,
        },
        RepairRule {
            id: "record-repair-action-arity",
            owner: "workflow-repair",
            review_after: "2026-06-01",
            apply: record_repair_action_arity_rule,
        },
        RepairRule {
            id: "skill-promotion-gate",
            owner: "skill-evolution",
            review_after: "2026-06-01",
            apply: skill_promotion_gate_rule,
        },
        RepairRule {
            id: "memory-recall-conflict-precision",
            owner: "memory-recall",
            review_after: "2026-06-01",
            apply: memory_recall_conflict_rule,
        },
        RepairRule {
            id: "memory-duplicate-demotion",
            owner: "memory-quality",
            review_after: "2026-06-01",
            apply: memory_duplicate_demote_rule,
        },
        RepairRule {
            id: "memory-sensitive-hard-block",
            owner: "memory-quality",
            review_after: "2026-06-01",
            apply: memory_sensitive_hard_block_rule,
        },
        RepairRule {
            id: "memory-quality-gate",
            owner: "memory-quality",
            review_after: "2026-06-01",
            apply: memory_quality_gate_rule,
        },
    ]
}

fn rust_e0596_rule(
    _loop_state: &ConversationLoop,
    lower_evidence: &str,
    cwd: &std::path::Path,
) -> Vec<PatchSynthesisAction> {
    ConversationLoop::deterministic_rust_e0596_action(lower_evidence, cwd)
        .into_iter()
        .collect()
}

fn persistent_memory_planning_rule(
    _loop_state: &ConversationLoop,
    lower_evidence: &str,
    cwd: &std::path::Path,
) -> Vec<PatchSynthesisAction> {
    let mut actions = Vec::new();
    actions.extend(
        ConversationLoop::deterministic_persistent_memory_planning_action(lower_evidence, cwd),
    );
    actions.extend(
        ConversationLoop::deterministic_persistent_memory_context_borrow_action(
            lower_evidence,
            cwd,
        ),
    );
    actions
}

fn live_eval_dashboard_summary_rule(
    _loop_state: &ConversationLoop,
    lower_evidence: &str,
    cwd: &std::path::Path,
) -> Vec<PatchSynthesisAction> {
    ConversationLoop::deterministic_live_eval_dashboard_summary_actions(lower_evidence, cwd)
}

fn record_repair_action_arity_rule(
    _loop_state: &ConversationLoop,
    lower_evidence: &str,
    cwd: &std::path::Path,
) -> Vec<PatchSynthesisAction> {
    ConversationLoop::deterministic_record_repair_action_arity_fix(lower_evidence, cwd)
        .into_iter()
        .collect()
}

fn skill_promotion_gate_rule(
    _loop_state: &ConversationLoop,
    lower_evidence: &str,
    cwd: &std::path::Path,
) -> Vec<PatchSynthesisAction> {
    ConversationLoop::deterministic_skill_promotion_gate_actions(lower_evidence, cwd)
}

fn memory_recall_conflict_rule(
    _loop_state: &ConversationLoop,
    lower_evidence: &str,
    cwd: &std::path::Path,
) -> Vec<PatchSynthesisAction> {
    ConversationLoop::deterministic_memory_recall_conflict_actions(lower_evidence, cwd)
}

fn memory_duplicate_demote_rule(
    _loop_state: &ConversationLoop,
    lower_evidence: &str,
    cwd: &std::path::Path,
) -> Vec<PatchSynthesisAction> {
    ConversationLoop::deterministic_memory_duplicate_demote_actions(lower_evidence, cwd)
}

fn memory_sensitive_hard_block_rule(
    _loop_state: &ConversationLoop,
    lower_evidence: &str,
    cwd: &std::path::Path,
) -> Vec<PatchSynthesisAction> {
    ConversationLoop::deterministic_memory_sensitive_hard_block_actions(lower_evidence, cwd)
}

fn memory_quality_gate_rule(
    _loop_state: &ConversationLoop,
    lower_evidence: &str,
    cwd: &std::path::Path,
) -> Vec<PatchSynthesisAction> {
    if !(lower_evidence.contains("memory-save-quality-gate")
        || lower_evidence.contains("memory quality gate")
        || lower_evidence.contains("memory save quality")
        || lower_evidence.contains("quality gate")
        || lower_evidence.contains("quality gates"))
    {
        return Vec::new();
    }

    let mut actions = Vec::new();
    let memory_tool = cwd.join("src/tools/memory_tool/mod.rs");
    if ConversationLoop::file_contains(
        &memory_tool,
        "assess_memory_candidate(content, category, &existing, true)",
    ) {
        actions.push(PatchSynthesisAction {
            tool: "file_edit".to_string(),
            path: "src/tools/memory_tool/mod.rs".to_string(),
            old_string: Some(
                "assess_memory_candidate(content, category, &existing, true)".to_string(),
            ),
            new_string: "assess_memory_candidate(content, category, &existing, false)".to_string(),
            line_start: None,
            line_end: None,
            expected_replacements: Some(1),
        });
    }

    let quality = cwd.join("src/memory/quality.rs");
    if ConversationLoop::file_contains(
        &quality,
        "let status = if explicit || score >= 0.65 { MemoryStatus::Accepted } else { write_decision.status };",
    ) {
        actions.push(PatchSynthesisAction {
            tool: "file_edit".to_string(),
            path: "src/memory/quality.rs".to_string(),
            old_string: Some(
                "let status = if explicit || score >= 0.65 { MemoryStatus::Accepted } else { write_decision.status };"
                    .to_string(),
            ),
            new_string: "let status = write_decision.status;".to_string(),
            line_start: None,
            line_end: None,
            expected_replacements: Some(1),
        });
    }
    if ConversationLoop::file_contains(
        &quality,
        "let status = if score >= 0.65 { MemoryStatus::Accepted } else { write_decision.status };",
    ) {
        actions.push(PatchSynthesisAction {
            tool: "file_edit".to_string(),
            path: "src/memory/quality.rs".to_string(),
            old_string: Some(
                "let status = if score >= 0.65 { MemoryStatus::Accepted } else { write_decision.status };"
                    .to_string(),
            ),
            new_string: "let status = write_decision.status;".to_string(),
            line_start: None,
            line_end: None,
            expected_replacements: Some(1),
        });
    }
    let explicit_proposed_status = r#"let status = if score >= 0.65 {
        MemoryStatus::Accepted
    } else if explicit && score >= 0.45 {
        // Explicit override lowers threshold but still respects hard limits from score_memory_write
        MemoryStatus::Proposed
    } else {
        write_decision.status
    };"#;
    if ConversationLoop::file_contains(&quality, explicit_proposed_status) {
        actions.push(PatchSynthesisAction {
            tool: "file_edit".to_string(),
            path: "src/memory/quality.rs".to_string(),
            old_string: Some(explicit_proposed_status.to_string()),
            new_string: "let status = write_decision.status;".to_string(),
            line_start: None,
            line_end: None,
            expected_replacements: Some(1),
        });
    }

    if let Some((first, second)) = ConversationLoop::deterministic_save_outcome_actions(cwd) {
        actions.push(first);
        actions.push(second);
    }

    actions
}
