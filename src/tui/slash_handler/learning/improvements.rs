//! Learning slash-command handler support.
//!
//! Renders learning goals, memory proposals, improvements, and skills without changing memory policy directly.

use super::*;

/// /improvements - Controlled self-evolution proposals
pub fn handle_improvements(app: &mut TuiApp, args: &str) -> String {
    use crate::engine::improvement::{
        ImprovementEffectOutcome, ImprovementStore, ProposalEvalStatus, ProposalStatus,
    };

    let mut parts = args.split_whitespace();
    let action = parts.next().unwrap_or("list");
    let store = ImprovementStore::default();

    match action {
        "scan" | "propose" => {
            let limit = parts
                .next()
                .and_then(|value| value.parse::<i64>().ok())
                .unwrap_or(50)
                .clamp(5, 200);
            let events = match app.session_manager.recent_learning_events(limit) {
                Ok(events) => events,
                Err(e) => return format!("Improvement scan failed: {}", e),
            };
            match store.propose_from_learning_events(&events) {
                Ok(proposals) if proposals.is_empty() => {
                    "Improvement scan complete: no new proposals.".to_string()
                }
                Ok(proposals) => {
                    let mut lines = vec![format!(
                        "Improvement scan complete: {} new proposal(s)",
                        proposals.len()
                    )];
                    for proposal in proposals {
                        lines.push(format_improvement_line(&proposal));
                    }
                    lines.join("\n")
                }
                Err(e) => format!("Improvement scan failed: {}", e),
            }
        }
        "list" | "" => {
            let proposals = store.list();
            if proposals.is_empty() {
                "Improvements\n- none yet\n\nRun /improvements scan to generate proposals from recent learning events.".to_string()
            } else {
                let mut lines = vec![format!("Improvements ({} total)", proposals.len())];
                for proposal in proposals.iter().take(20) {
                    lines.push(format_improvement_line(proposal));
                }
                lines.join("\n")
            }
        }
        "active" => format_applied_guidance_list(&store.applied_guidance_store().active()),
        "doctor" => format_improvement_doctor(&store),
        "show" => {
            let Some(id) = parts.next() else {
                return "Usage: /improvements show <id>".to_string();
            };
            match store.get(id) {
                Some(proposal) => format_improvement_detail_with_state(&proposal, &store),
                None => format!("No improvement proposal matching '{}'.", id),
            }
        }
        "effect" => {
            let Some(id) = parts.next() else {
                return "Usage: /improvements effect <id>".to_string();
            };
            let Some(proposal) = store.get(id) else {
                return format!("No improvement proposal matching '{}'.", id);
            };
            format_improvement_effect_summary(&store.effect_store().summary(&proposal.id))
        }
        "record-effect" => {
            let Some(id) = parts.next() else {
                return "Usage: /improvements record-effect <id> <positive|neutral|negative> <evalset> [reason]".to_string();
            };
            let Some(outcome) = parts.next().and_then(parse_improvement_effect_outcome) else {
                return "Usage: /improvements record-effect <id> <positive|neutral|negative> <evalset> [reason]".to_string();
            };
            let Some(evalset) = parts.next() else {
                return "Usage: /improvements record-effect <id> <positive|neutral|negative> <evalset> [reason]".to_string();
            };
            let Some(proposal) = store.get(id) else {
                return format!("No improvement proposal matching '{}'.", id);
            };
            let reason = parts.collect::<Vec<_>>().join(" ");
            let reason = if reason.trim().is_empty() {
                "manual effect record".to_string()
            } else {
                reason
            };
            match store.effect_store().record(
                proposal.id.clone(),
                evalset,
                format!("manual-{}", chrono::Utc::now().timestamp()),
                outcome,
                if outcome == ImprovementEffectOutcome::Negative {
                    "framework"
                } else {
                    "none"
                },
                reason,
            ) {
                Ok(record) => format!(
                    "Recorded improvement effect {}\n{}",
                    record.id,
                    format_improvement_effect_summary(&store.effect_store().summary(&proposal.id))
                ),
                Err(e) => format!("Failed to record improvement effect: {}", e),
            }
        }
        "deactivate" => {
            let Some(id) = parts.next() else {
                return "Usage: /improvements deactivate <id>".to_string();
            };
            match store.applied_guidance_store().deactivate(id) {
                Ok(Some(record)) => format!(
                    "Deactivated applied guidance {}\nproposal={} status={:?}",
                    record.id, record.proposal_id, record.status
                ),
                Ok(None) => format!("No applied guidance matching '{}'.", id),
                Err(e) => format!("Failed to deactivate applied guidance: {}", e),
            }
        }
        "bind-eval" => {
            let Some(id) = parts.next() else {
                return "Usage: /improvements bind-eval <id> <evalset-name>".to_string();
            };
            let Some(evalset) = parts.next() else {
                return "Usage: /improvements bind-eval <id> <evalset-name>".to_string();
            };
            match store.bind_evalset(id, evalset) {
                Ok(Some(updated)) => format!(
                    "Bound evalset '{}' to improvement proposal {}\n{}",
                    evalset,
                    updated.id,
                    format_improvement_line(&updated)
                ),
                Ok(None) => format!("No improvement proposal matching '{}'.", id),
                Err(e) => format!("Failed to bind evalset: {}", e),
            }
        }
        "eval" => {
            let Some(id) = parts.next() else {
                return "Usage: /improvements eval <id>".to_string();
            };
            let Some(current) = store.get(id) else {
                return format!("No improvement proposal matching '{}'.", id);
            };
            let eval = evaluate_improvement_proposal_for_apply(&current);
            match store.record_eval(
                id,
                if eval.passed {
                    ProposalEvalStatus::Passed
                } else {
                    ProposalEvalStatus::Failed
                },
                eval.summary.clone(),
            ) {
                Ok(Some(updated)) => {
                    persist_improvement_learning_event(app, &updated, "eval");
                    format!("{}\n\n{}", eval.summary, format_improvement_line(&updated))
                }
                Ok(None) => format!("No improvement proposal matching '{}'.", id),
                Err(e) => format!("Failed to record improvement eval: {}", e),
            }
        }
        "accept" | "reject" | "apply" | "rollback" => {
            let Some(id) = parts.next() else {
                return format!("Usage: /improvements {} <id>", action);
            };
            let desired = match action {
                "accept" => ProposalStatus::Accepted,
                "reject" => ProposalStatus::Rejected,
                "apply" => ProposalStatus::Applied,
                "rollback" => ProposalStatus::RolledBack,
                _ => unreachable!(),
            };
            let Some(current) = store.get(id) else {
                return format!("No improvement proposal matching '{}'.", id);
            };
            if desired == ProposalStatus::RolledBack && current.status != ProposalStatus::Applied {
                return format!(
                    "Proposal {} is {:?}. Only applied proposals can be rolled back.",
                    current.id, current.status
                );
            }
            if desired == ProposalStatus::Applied && current.status != ProposalStatus::Accepted {
                return format!(
                    "Proposal {} is {:?}. Accept it before applying. High-risk and behavior-changing proposals require explicit approval.",
                    current.id, current.status
                );
            }
            if desired == ProposalStatus::Applied && current.evalset_bindings.is_empty() {
                return format!(
                    "Proposal {} has no bound evalset. Run /improvements bind-eval {} <evalset> before eval/apply.",
                    current.id, current.id
                );
            }
            if desired == ProposalStatus::Applied
                && current.eval_status != ProposalEvalStatus::Passed
            {
                return format!(
                    "Proposal {} has eval={:?}. Run /improvements eval {} before applying.",
                    current.id, current.eval_status, current.id
                );
            }
            if desired == ProposalStatus::Applied {
                let gate = improvement_evolution_gate(&current);
                if matches!(
                    gate.action,
                    crate::engine::evolution_controller::EvolutionAction::Reject
                        | crate::engine::evolution_controller::EvolutionAction::Monitor
                ) {
                    return format!(
                        "Proposal {} was not applied by evolution gate.\n{}",
                        current.id,
                        format_evolution_gate(&gate)
                    );
                }
            }
            match store.update_status(id, desired) {
                Ok(Some(updated)) => {
                    if desired == ProposalStatus::Applied {
                        record_evolution_update(improvement_target(&updated));
                    }
                    if desired == ProposalStatus::RolledBack {
                        record_evolution_update(improvement_target(&updated));
                    }
                    persist_improvement_learning_event(app, &updated, action);
                    format!(
                        "Updated proposal {}\n{}",
                        updated.id,
                        format_improvement_line(&updated)
                    )
                }
                Ok(None) => format!("No improvement proposal matching '{}'.", id),
                Err(e) => format!("Failed to update proposal: {}", e),
            }
        }
        _ => {
            "Usage: /improvements [list|scan [limit]|active|doctor|show <id>|bind-eval <id> <evalset>|eval <id>|accept <id>|reject <id>|apply <id>|rollback <id>|effect <id>|record-effect <id> <positive|neutral|negative> <evalset> [reason]|deactivate <id>]"
                .to_string()
        }
    }
}

pub(super) fn format_improvement_line(
    proposal: &crate::engine::improvement::ImprovementProposal,
) -> String {
    format!(
        "- {} [{:?}/{:?}/{:?}] eval={:?} evalsets={} stage={} events={}: {}",
        proposal.id,
        proposal.status,
        proposal.target,
        proposal.risk,
        proposal.eval_status,
        if proposal.evalset_bindings.is_empty() {
            "none".to_string()
        } else {
            proposal.evalset_bindings.join(",")
        },
        proposal.lifecycle_stage(),
        proposal.trigger_event_ids.len(),
        proposal.proposed_change
    )
}

fn format_improvement_detail(proposal: &crate::engine::improvement::ImprovementProposal) -> String {
    format!(
        "Improvement Proposal {}\n\nStatus: {:?}\nStage: {}\nTarget: {:?}\nRisk: {:?}\nEval: {:?}\nEvalSets: {}\nEval summary: {}\nApplied ref: {}\nRollback ref: {}\nEvents: {:?}\n\nProposed change:\n{}\n\nExpected benefit:\n{}\n\nValidation plan:\n{}\n\nRollback plan:\n{}\n\nEvidence:\n{}",
        proposal.id,
        proposal.status,
        proposal.lifecycle_stage(),
        proposal.target,
        proposal.risk,
        proposal.eval_status,
        if proposal.evalset_bindings.is_empty() {
            "none".to_string()
        } else {
            proposal.evalset_bindings.join(", ")
        },
        proposal.eval_summary.as_deref().unwrap_or("none"),
        proposal.applied_ref.as_deref().unwrap_or("none"),
        proposal.rollback_ref.as_deref().unwrap_or("none"),
        proposal.trigger_event_ids,
        proposal.proposed_change,
        proposal.expected_benefit,
        proposal
            .validation
            .iter()
            .map(|item| format!("- {}", item))
            .collect::<Vec<_>>()
            .join("\n"),
        proposal.rollback_plan,
        proposal
            .evidence
            .iter()
            .map(|item| format!("- {}", item))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

pub(super) fn format_improvement_detail_with_state(
    proposal: &crate::engine::improvement::ImprovementProposal,
    store: &crate::engine::improvement::ImprovementStore,
) -> String {
    let mut detail = format_improvement_detail(proposal);
    let guidance = store.applied_guidance_store().get(&proposal.id);
    let effect = store.effect_store().summary(&proposal.id);
    detail.push_str("\n\nApplied guidance:\n");
    match guidance {
        Some(record) => detail.push_str(&format!(
            "- {} status={:?} activation={:?} scope={}:{} rollback={}",
            record.id,
            record.status,
            record.activation,
            record.scope.kind,
            record.scope.label,
            record.rollback_ref.as_deref().unwrap_or("none")
        )),
        None => detail.push_str("- none"),
    }
    detail.push_str("\n\nEffect summary:\n");
    detail.push_str(&format_improvement_effect_summary(&effect));
    detail
}

pub(super) fn format_applied_guidance_list(
    records: &[crate::engine::improvement::AppliedGuidanceRecord],
) -> String {
    if records.is_empty() {
        return "Active Applied Guidance\n- none".to_string();
    }
    let mut lines = vec![format!("Active Applied Guidance ({} total)", records.len())];
    for record in records.iter().take(20) {
        lines.push(format!(
            "- {} proposal={} target={:?} activation={:?} scope={}:{} evalsets={} updated={}",
            record.id,
            record.proposal_id,
            record.target,
            record.activation,
            record.scope.kind,
            record.scope.label,
            if record.evalsets.is_empty() {
                "none".to_string()
            } else {
                record.evalsets.join(",")
            },
            record.updated_at
        ));
        lines.push(format!(
            "  {}",
            record
                .content
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .chars()
                .take(220)
                .collect::<String>()
        ));
    }
    lines.join("\n")
}

pub(super) fn format_improvement_effect_summary(
    summary: &crate::engine::improvement::ImprovementEffectSummary,
) -> String {
    let mut lines = vec![format!(
        "Improvement Effect {}\n- total={} positive={} neutral={} negative={} rollback_recommended={}",
        summary.proposal_id,
        summary.total,
        summary.positive,
        summary.neutral,
        summary.negative,
        summary.rollback_recommended
    )];
    for record in &summary.recent {
        lines.push(format!(
            "- {} {:?} evalset={} run={} owner={} reason={}",
            record.created_at,
            record.outcome,
            record.evalset,
            record.run_id,
            record.failure_owner,
            record.reason
        ));
    }
    lines.join("\n")
}

fn parse_improvement_effect_outcome(
    value: &str,
) -> Option<crate::engine::improvement::ImprovementEffectOutcome> {
    match value.to_ascii_lowercase().as_str() {
        "positive" | "pass" | "passed" | "improved" => {
            Some(crate::engine::improvement::ImprovementEffectOutcome::Positive)
        }
        "neutral" | "same" => Some(crate::engine::improvement::ImprovementEffectOutcome::Neutral),
        "negative" | "fail" | "failed" | "regressed" => {
            Some(crate::engine::improvement::ImprovementEffectOutcome::Negative)
        }
        _ => None,
    }
}

fn format_improvement_doctor(store: &crate::engine::improvement::ImprovementStore) -> String {
    let proposals = store.list();
    let active = store.applied_guidance_store().active();
    let missing_evalsets = proposals
        .iter()
        .filter(|proposal| {
            proposal.status == crate::engine::improvement::ProposalStatus::Accepted
                && proposal.evalset_bindings.is_empty()
        })
        .count();
    let failed_eval = proposals
        .iter()
        .filter(|proposal| {
            proposal.eval_status == crate::engine::improvement::ProposalEvalStatus::Failed
        })
        .count();
    let rollback_recommended = proposals
        .iter()
        .filter(|proposal| {
            store
                .effect_store()
                .summary(&proposal.id)
                .rollback_recommended
        })
        .count();
    let last_eval = proposals
        .iter()
        .filter(|proposal| proposal.eval_summary.is_some())
        .max_by(|left, right| left.updated_at.cmp(&right.updated_at));
    format!(
        "Improvement Doctor\n- proposals={}\n- active_guidance={}\n- blocked_missing_evalsets={}\n- failed_eval={}\n- rollback_recommended={}\n- last_eval={}",
        proposals.len(),
        active.len(),
        missing_evalsets,
        failed_eval,
        rollback_recommended,
        last_eval
            .map(|proposal| format!("{} {:?}", proposal.id, proposal.eval_status))
            .unwrap_or_else(|| "none".to_string())
    )
}

pub(super) struct ImprovementEvalSummary {
    pub(super) passed: bool,
    pub(super) summary: String,
}

pub(super) fn evaluate_improvement_proposal_for_apply(
    proposal: &crate::engine::improvement::ImprovementProposal,
) -> ImprovementEvalSummary {
    let has_validation = !proposal.validation.is_empty();
    let has_evidence = !proposal.evidence.is_empty();
    let gate = improvement_evolution_gate(proposal);
    let gate_allows = !matches!(
        gate.action,
        crate::engine::evolution_controller::EvolutionAction::Reject
            | crate::engine::evolution_controller::EvolutionAction::Monitor
    );
    let bound_report = run_bound_improvement_evalsets(proposal);
    let has_bound_evalset = !proposal.evalset_bindings.is_empty();
    let bound_ok = bound_report
        .as_ref()
        .map(|report| report.ok)
        .unwrap_or(false);
    let passed = has_validation && has_evidence && gate_allows && bound_ok;
    let mut lines = vec![format!(
        "Improvement Eval {}: {}",
        proposal.id,
        if passed { "passed" } else { "failed" }
    )];
    lines.push(format!(
        "- validation_plan={} evidence={} gate={:?} score={:.2}",
        proposal.validation.len(),
        proposal.evidence.len(),
        gate.action,
        gate.score
    ));
    if !has_validation {
        lines.push("- missing validation plan".to_string());
    }
    if !has_evidence {
        lines.push("- missing evidence".to_string());
    }
    if !gate_allows {
        lines.push("- evolution gate did not allow apply".to_string());
    }
    if !has_bound_evalset {
        lines.push("- missing bound evalset; bind at least one evalset before apply".to_string());
        lines.push("- failure_owner=framework".to_string());
    }
    if let Some(report) = bound_report {
        lines.push(format!(
            "- bound_evalsets: {}/{} passed, failed={}, run_id={}, failure_owner={}",
            report.passed, report.total, report.failed, report.run_id, report.failure_owner
        ));
        if !report.ok {
            lines.push(report.summary);
        }
    }
    for reason in gate.reasons.iter().take(3) {
        lines.push(format!("- gate: {}", reason));
    }
    ImprovementEvalSummary {
        passed,
        summary: lines.join("\n"),
    }
}

fn run_bound_improvement_evalsets(
    proposal: &crate::engine::improvement::ImprovementProposal,
) -> Option<BoundSkillEvalReport> {
    if proposal.evalset_bindings.is_empty() {
        return None;
    }
    let eval_dir = std::env::current_dir().ok()?.join("evalsets");
    let mut summaries = Vec::new();
    let mut ok = true;
    let mut total = 0usize;
    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut failure_owner = "none".to_string();
    let run_id = format!("eval-{}", chrono::Utc::now().timestamp());
    for binding in &proposal.evalset_bindings {
        match crate::engine::evalset::run_evalsets_from_dir(&eval_dir, Some(binding)) {
            Ok(reports) if reports.is_empty() => {
                ok = false;
                failure_owner = "test_harness".to_string();
                summaries.push(format!("- {}: no matching evalset", binding));
            }
            Ok(reports) => {
                let binding_ok = reports.iter().all(|report| report.ok());
                ok &= binding_ok;
                for report in &reports {
                    total += report.total;
                    passed += report.passed;
                    failed += report.failed;
                }
                if !binding_ok {
                    failure_owner = "framework".to_string();
                }
                summaries.push(crate::engine::evalset::format_reports(&reports));
            }
            Err(e) => {
                ok = false;
                failure_owner = "test_harness".to_string();
                summaries.push(format!("- {}: {}", binding, e));
            }
        }
    }
    Some(BoundSkillEvalReport {
        ok,
        summary: summaries.join("\n\n"),
        total,
        passed,
        failed,
        run_id,
        failure_owner,
    })
}

fn persist_improvement_learning_event(
    app: &mut TuiApp,
    proposal: &crate::engine::improvement::ImprovementProposal,
    action: &str,
) {
    let mut payload = serde_json::to_value(proposal).unwrap_or_else(|_| serde_json::json!({}));
    if action == "apply" {
        payload["evolution_gate"] =
            serde_json::to_value(improvement_evolution_gate(proposal)).unwrap_or_default();
    }
    let _ = app.session_manager.add_learning_event(
        "improvement_proposal",
        "improvements",
        &format!("Improvement proposal {} {}", proposal.id, action),
        0.9,
        &payload,
    );
}
