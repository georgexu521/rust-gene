use super::*;

pub(super) fn user_skill_root() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".priority-agent")
        .join("skills")
}

#[derive(Debug, Clone)]
pub(super) struct DisabledSkillBackup {
    pub(super) skill_name: String,
    pub(super) backup_name: String,
    pub(super) path: std::path::PathBuf,
}

pub(super) fn is_safe_skill_dir_name(name: &str) -> bool {
    !name.is_empty()
        && !name.contains('/')
        && !name.contains('\\')
        && name != "."
        && name != ".."
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
}

pub(super) fn disabled_skill_backups(
    root: &std::path::Path,
    filter: Option<&str>,
) -> Vec<DisabledSkillBackup> {
    let mut backups = Vec::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return backups;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(backup_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some((skill_name, _suffix)) = backup_name.split_once(".disabled-") else {
            continue;
        };
        if filter.is_some_and(|needle| needle != skill_name) {
            continue;
        }
        backups.push(DisabledSkillBackup {
            skill_name: skill_name.to_string(),
            backup_name: backup_name.to_string(),
            path,
        });
    }
    backups.sort_by(|a, b| b.backup_name.cmp(&a.backup_name));
    backups
}

pub(super) fn resolve_disabled_skill_backup(
    root: &std::path::Path,
    skill_name: &str,
    backup_name: Option<&str>,
) -> Option<DisabledSkillBackup> {
    let backups = disabled_skill_backups(root, Some(skill_name));
    match backup_name {
        Some(name) => backups
            .into_iter()
            .find(|backup| backup.backup_name == name),
        None => backups.into_iter().next(),
    }
}

pub(super) fn format_skill_proposal_line(
    proposal: &crate::engine::skill_evolution::SkillProposal,
) -> String {
    format!(
        "- {} /{} v{} [{:?}/{:?}] score={:.2} events={} evalsets={}: {}",
        proposal.id,
        proposal.name,
        proposal.skill_version(),
        proposal.status,
        proposal.trust,
        proposal.creation_score,
        proposal.trigger_event_ids.len(),
        if proposal.evalset_bindings.is_empty() {
            "none".to_string()
        } else {
            proposal.evalset_bindings.join(",")
        },
        proposal.procedure
    )
}

pub(super) fn format_skill_proposal_detail(
    proposal: &crate::engine::skill_evolution::SkillProposal,
) -> String {
    format!(
        "Skill Proposal {}\n\nName: /{}\nVersion: {}\nStatus: {:?}\nTrust: {:?}\nScope: {}\nCreation score: {:.2}\nEvidence count: {}\nScope confidence: {:.2}\nEvalSets: {}\nRollback to: {}\nApplied path: {}\nEvents: {:?}\n\nProcedure:\n{}\n\nTriggers:\n{}\n\nWorkflow:\n{}\n\nValidation:\n{}\n\nTools:\n{}\n\nEvidence:\n{}",
        proposal.id,
        proposal.name,
        proposal.skill_version(),
        proposal.status,
        proposal.trust,
        proposal.scope,
        proposal.creation_score,
        proposal.evidence_count,
        proposal.scope_confidence,
        if proposal.evalset_bindings.is_empty() {
            "none".to_string()
        } else {
            proposal.evalset_bindings.join(", ")
        },
        proposal.rollback_to.as_deref().unwrap_or("none"),
        proposal.applied_path.as_deref().unwrap_or("none"),
        proposal.trigger_event_ids,
        proposal.procedure,
        proposal
            .trigger_conditions
            .iter()
            .map(|item| format!("- {}", item))
            .collect::<Vec<_>>()
            .join("\n"),
        proposal
            .workflow_steps
            .iter()
            .enumerate()
            .map(|(idx, item)| format!("{}. {}", idx + 1, item))
            .collect::<Vec<_>>()
            .join("\n"),
        proposal
            .validation
            .iter()
            .map(|item| format!("- {}", item))
            .collect::<Vec<_>>()
            .join("\n"),
        proposal.allowed_tools.join(", "),
        proposal
            .evidence
            .iter()
            .map(|item| format!("- {}", item))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

pub(super) struct BoundSkillEvalReport {
    pub(super) ok: bool,
    pub(super) summary: String,
    pub(super) total: usize,
    pub(super) passed: usize,
    pub(super) failed: usize,
    pub(super) run_id: String,
    pub(super) failure_owner: String,
}

pub(super) fn run_bound_skill_evalsets(
    proposal: &crate::engine::skill_evolution::SkillProposal,
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

pub(super) fn validate_skill_promotion_for_apply(
    store: &crate::engine::skill_evolution::SkillProposalStore,
    proposal: &crate::engine::skill_evolution::SkillProposal,
    bound_report: Option<&BoundSkillEvalReport>,
) -> Result<Option<crate::engine::skill_evolution::SkillPromotionGate>, String> {
    let records = store.version_records(&proposal.name);
    let active_exists = user_skill_root().join(&proposal.name).exists();
    if records.is_empty() && !active_exists {
        return Ok(None);
    }

    let latest = records.last().ok_or_else(|| {
        format!(
            "Active /{} exists but no version baseline is recorded; rollback or record a baseline before replacing it.",
            proposal.name
        )
    })?;
    let candidate_version = proposal.skill_version();
    if latest.version == candidate_version {
        return Err(format!(
            "Candidate version '{}' matches the active /{} version; cannot compare promotion fitness. Regenerate the proposal or record candidate usage under a distinct version.",
            candidate_version, proposal.name
        ));
    }

    let all_events = store.usage_events(&proposal.name);
    let old_events = all_events
        .iter()
        .filter(|event| event.skill_version == latest.version && !event.provisional)
        .cloned()
        .collect::<Vec<_>>();
    let old_snapshot =
        crate::engine::skill_evolution::skill_fitness_snapshot(&proposal.name, &old_events)
            .ok_or_else(|| {
                format!(
                    "Existing /{} version '{}' has no confirmed fitness baseline. Record usage before replacing it.",
                    proposal.name, latest.version
                )
            })?;

    let new_events = all_events
        .iter()
        .filter(|event| event.skill_version == candidate_version && !event.provisional)
        .cloned()
        .collect::<Vec<_>>();
    let new_snapshot =
        crate::engine::skill_evolution::skill_fitness_snapshot(&proposal.name, &new_events)
            .or_else(|| bound_report.and_then(|report| skill_fitness_from_bound_eval(proposal, report)))
            .ok_or_else(|| {
                format!(
                    "Candidate /{} version '{}' has no promotion evidence. Record at least 3 candidate outcomes or bind passing evalsets before replacing an active skill.",
                    proposal.name, candidate_version
                )
            })?;
    let regression_rate = if new_snapshot.events == 0 {
        1.0
    } else {
        new_snapshot.stats.failure_rate
    };
    let semantic_drift = estimate_skill_semantic_drift(proposal);
    let gate = crate::engine::skill_evolution::compare_skill_versions_for_promotion(
        old_snapshot.fitness,
        &new_snapshot,
        regression_rate,
        semantic_drift,
    );
    if gate.passed {
        Ok(Some(gate))
    } else {
        Err(format_skill_promotion_gate(&gate))
    }
}

pub(super) fn skill_fitness_from_bound_eval(
    proposal: &crate::engine::skill_evolution::SkillProposal,
    report: &BoundSkillEvalReport,
) -> Option<crate::engine::skill_evolution::SkillFitnessSnapshot> {
    if report.total == 0 {
        return None;
    }
    let pass_rate = report.passed as f32 / report.total as f32;
    let failure_rate = report.failed as f32 / report.total as f32;
    let stats = crate::engine::skill_evolution::SkillFitnessStats {
        task_success: pass_rate,
        acceptance_pass_rate: pass_rate,
        test_pass_rate: pass_rate,
        user_satisfaction: if report.ok { 0.75 } else { 0.35 },
        reuse_rate: (proposal.evidence_count as f32 / 10.0).clamp(0.0, 1.0),
        time_saved: 0.55,
        tool_efficiency: 0.55,
        failure_rate,
        cost: 0.20,
        risk_penalty: if report.ok { 0.05 } else { 0.30 },
    };
    Some(crate::engine::skill_evolution::SkillFitnessSnapshot {
        skill_name: proposal.name.clone(),
        skill_version: proposal.skill_version(),
        events: report.total,
        fitness: crate::engine::skill_evolution::compute_skill_fitness(stats),
        stats,
    })
}

pub(super) fn estimate_skill_semantic_drift(
    proposal: &crate::engine::skill_evolution::SkillProposal,
) -> f32 {
    let step_count = proposal.workflow_steps.len() as f32;
    let validation_count = proposal.validation.len() as f32;
    let evidence_count = proposal.evidence_count.max(1) as f32;
    let shape_risk =
        ((step_count - validation_count).abs() / (step_count.max(1.0) + 2.0)).clamp(0.0, 0.25);
    let specificity_risk = proposal.creation_factors.over_specificity * 0.50;
    let evidence_risk = (1.0 / evidence_count).min(0.25);
    (shape_risk + specificity_risk + evidence_risk).clamp(0.0, 1.0)
}

pub(super) fn format_skill_eval(eval: &crate::engine::skill_evolution::SkillEvalResult) -> String {
    let mut lines = vec![format!(
        "Skill Eval {}\nResult: {}",
        eval.proposal_id,
        if eval.passed { "pass" } else { "fail" }
    )];
    for check in &eval.quality.checks {
        lines.push(format!(
            "- {} {}: {}",
            if check.passed { "ok" } else { "fail" },
            check.name,
            check.detail
        ));
    }
    for note in &eval.notes {
        lines.push(format!("- note: {}", note));
    }
    lines.join("\n")
}

pub(super) fn format_skill_fitness(
    snapshot: &crate::engine::skill_evolution::SkillFitnessSnapshot,
) -> String {
    format!(
        "Skill Fitness /{}\nVersion: {}\nEvents: {}\nFitness: {:.2}\n\nFactors:\n- task_success: {:.2}\n- acceptance_pass_rate: {:.2}\n- test_pass_rate: {:.2}\n- user_satisfaction: {:.2}\n- reuse_rate: {:.2}\n- time_saved: {:.2}\n- tool_efficiency: {:.2}\n- failure_rate: {:.2}\n- cost: {:.2}\n- risk_penalty: {:.2}",
        snapshot.skill_name,
        snapshot.skill_version,
        snapshot.events,
        snapshot.fitness,
        snapshot.stats.task_success,
        snapshot.stats.acceptance_pass_rate,
        snapshot.stats.test_pass_rate,
        snapshot.stats.user_satisfaction,
        snapshot.stats.reuse_rate,
        snapshot.stats.time_saved,
        snapshot.stats.tool_efficiency,
        snapshot.stats.failure_rate,
        snapshot.stats.cost,
        snapshot.stats.risk_penalty
    )
}

pub(super) fn format_skill_promotion_gate(
    gate: &crate::engine::skill_evolution::SkillPromotionGate,
) -> String {
    let mut lines = vec![format!(
        "Skill Promotion Gate\nResult: {}\nOld fitness: {:.2}\nNew fitness: {:.2}\nDelta: {:.2}\nEval count: {}\nRegression rate: {:.2}\nRisk penalty: {:.2}\nSemantic drift: {:.2}",
        if gate.passed { "pass" } else { "blocked" },
        gate.old_fitness,
        gate.new_fitness,
        gate.delta,
        gate.eval_count,
        gate.regression_rate,
        gate.risk_penalty,
        gate.semantic_drift
    )];
    if !gate.reasons.is_empty() {
        lines.push("Reasons:".to_string());
        for reason in &gate.reasons {
            lines.push(format!("- {}", reason));
        }
    }
    lines.join("\n")
}

pub(super) fn persist_skill_proposal_learning_event(
    app: &mut TuiApp,
    proposal: &crate::engine::skill_evolution::SkillProposal,
    action: &str,
    applied_path: Option<String>,
) {
    let mut payload = serde_json::to_value(proposal).unwrap_or_else(|_| serde_json::json!({}));
    if let Some(path) = applied_path {
        payload["applied_path"] = serde_json::json!(path);
    }
    if action == "apply" {
        payload["evolution_gate"] =
            serde_json::to_value(skill_evolution_gate(proposal)).unwrap_or_default();
    }
    let _ = app.session_manager.add_learning_event(
        "skill_proposal",
        "skill_evolution",
        &format!("Skill proposal {} {}", proposal.id, action),
        0.9,
        &payload,
    );
}

const EVOLUTION_COOLDOWN_SECS: u64 = 300;

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct PersistentEvolutionState {
    #[serde(default)]
    last_update_turn:
        std::collections::HashMap<crate::engine::evolution_controller::EvolutionTarget, u64>,
}

pub(super) fn evolution_state_path() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".priority-agent")
        .join("evolution_state.json")
}

pub(super) fn now_evolution_turn() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

pub(super) fn load_evolution_state() -> PersistentEvolutionState {
    std::fs::read_to_string(evolution_state_path())
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_default()
}

pub(super) fn save_evolution_state(state: &PersistentEvolutionState) {
    let path = evolution_state_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(content) = serde_json::to_string_pretty(state) {
        let _ = std::fs::write(path, content);
    }
}

pub(super) fn load_evolution_controller() -> crate::engine::evolution_controller::EvolutionController
{
    crate::engine::evolution_controller::EvolutionController::new()
        .with_cooldown_turns(EVOLUTION_COOLDOWN_SECS)
        .with_last_updates(load_evolution_state().last_update_turn)
}

pub(super) fn record_evolution_update(
    target: crate::engine::evolution_controller::EvolutionTarget,
) {
    let mut state = load_evolution_state();
    state.last_update_turn.insert(target, now_evolution_turn());
    save_evolution_state(&state);
}

pub(super) fn improvement_target(
    proposal: &crate::engine::improvement::ImprovementProposal,
) -> crate::engine::evolution_controller::EvolutionTarget {
    match proposal.target {
        crate::engine::improvement::ImprovementTarget::Memory => {
            crate::engine::evolution_controller::EvolutionTarget::Memory
        }
        crate::engine::improvement::ImprovementTarget::Skill => {
            crate::engine::evolution_controller::EvolutionTarget::Skill
        }
        crate::engine::improvement::ImprovementTarget::Prompt => {
            crate::engine::evolution_controller::EvolutionTarget::PromptSection
        }
        crate::engine::improvement::ImprovementTarget::Routing => {
            crate::engine::evolution_controller::EvolutionTarget::WorkflowPolicy
        }
        crate::engine::improvement::ImprovementTarget::ToolGuidance => {
            crate::engine::evolution_controller::EvolutionTarget::ToolDescription
        }
    }
}

pub(super) fn improvement_evolution_gate(
    proposal: &crate::engine::improvement::ImprovementProposal,
) -> crate::engine::evolution_controller::EvolutionGateDecision {
    use crate::engine::evolution_controller::EvolutionTriggerFactors;
    let risk = risk_value(proposal.risk);
    let target = improvement_target(proposal);
    load_evolution_controller().gate(
        target,
        EvolutionTriggerFactors {
            repeated_failure: (proposal.trigger_event_ids.len() as f32 / 4.0).clamp(0.0, 1.0),
            reuse_frequency: 0.55,
            user_correction_frequency: if proposal
                .evidence
                .iter()
                .any(|item| item.to_lowercase().contains("correction"))
            {
                0.80
            } else {
                0.35
            },
            task_impact: if proposal.target == crate::engine::improvement::ImprovementTarget::Memory
            {
                0.55
            } else {
                0.75
            },
            optimization_potential: 0.70,
            evolution_cost: if matches!(
                proposal.target,
                crate::engine::improvement::ImprovementTarget::Prompt
                    | crate::engine::improvement::ImprovementTarget::Routing
            ) {
                0.65
            } else {
                0.35
            },
            risk,
        },
        now_evolution_turn(),
    )
}

pub(super) fn skill_evolution_gate(
    proposal: &crate::engine::skill_evolution::SkillProposal,
) -> crate::engine::evolution_controller::EvolutionGateDecision {
    use crate::engine::evolution_controller::{EvolutionTarget, EvolutionTriggerFactors};
    load_evolution_controller().gate(
        EvolutionTarget::Skill,
        EvolutionTriggerFactors {
            repeated_failure: 0.0,
            reuse_frequency: (proposal.evidence_count as f32 / 6.0).clamp(0.0, 1.0),
            user_correction_frequency: proposal.creation_factors.user_correction_value,
            task_impact: proposal.creation_factors.future_utility,
            optimization_potential: proposal.creation_score,
            evolution_cost: proposal.creation_factors.over_specificity.max(0.20),
            risk: 1.0 - proposal.scope_confidence,
        },
        now_evolution_turn(),
    )
}

pub(super) fn risk_value(risk: crate::engine::intent_router::RiskLevel) -> f32 {
    match risk {
        crate::engine::intent_router::RiskLevel::Low => 0.20,
        crate::engine::intent_router::RiskLevel::Medium => 0.50,
        crate::engine::intent_router::RiskLevel::High => 0.85,
    }
}

pub(super) fn format_evolution_gate(
    gate: &crate::engine::evolution_controller::EvolutionGateDecision,
) -> String {
    let mut lines = vec![format!(
        "Evolution gate: {:?} target={:?} score={:.2} auto_apply={}",
        gate.action, gate.target, gate.score, gate.auto_apply_allowed
    )];
    for reason in &gate.reasons {
        lines.push(format!("- {}", reason));
    }
    lines.join("\n")
}
