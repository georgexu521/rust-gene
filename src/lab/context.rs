use crate::engine::context_compressor::estimate_tokens;
use crate::engine::prompt_context::stable_fingerprint;
use crate::lab::model::{
    LabCompressionAction, LabCompressionDecision, LabCostSummary, LabEvidenceRef, LabRole, LabRun,
    LabValidationRetry, LAB_SCHEMA_VERSION,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabContextEvidenceRefGroup {
    pub source: String,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabContextStability {
    StablePrefix,
    DynamicTail,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabContextLayer {
    pub layer: String,
    pub label: String,
    pub stability: LabContextStability,
    pub estimated_tokens: u64,
    pub content: String,
}

impl LabContextLayer {
    pub fn new(
        layer: impl Into<String>,
        label: impl Into<String>,
        stability: LabContextStability,
        content: impl Into<String>,
    ) -> Self {
        let content = content.into();
        Self {
            layer: layer.into(),
            label: label.into(),
            stability,
            estimated_tokens: estimate_tokens(&content) as u64,
            content,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabContextPacket {
    pub schema_version: u32,
    pub lab_run_id: String,
    pub role: LabRole,
    pub created_at: DateTime<Utc>,
    pub stable_prefix_fingerprint: String,
    pub dynamic_tail_fingerprint: String,
    pub stable_prefix_tokens: u64,
    pub dynamic_tail_tokens: u64,
    pub total_estimated_tokens: u64,
    pub layers: Vec<LabContextLayer>,
}

impl LabContextPacket {
    pub fn stable_layers(&self) -> impl Iterator<Item = &LabContextLayer> {
        self.layers
            .iter()
            .filter(|layer| layer.stability == LabContextStability::StablePrefix)
    }

    pub fn dynamic_layers(&self) -> impl Iterator<Item = &LabContextLayer> {
        self.layers
            .iter()
            .filter(|layer| layer.stability == LabContextStability::DynamicTail)
    }
}

pub fn build_lab_context_packet(
    run: &LabRun,
    role: LabRole,
    cost_summary: &LabCostSummary,
) -> LabContextPacket {
    build_lab_context_packet_with_evidence(run, role, cost_summary, &[])
}

pub fn build_lab_context_packet_with_evidence(
    run: &LabRun,
    role: LabRole,
    cost_summary: &LabCostSummary,
    evidence_refs: &[LabEvidenceRef],
) -> LabContextPacket {
    build_lab_context_packet_with_evidence_and_retries(run, role, cost_summary, evidence_refs, &[])
}

pub fn build_lab_context_packet_with_evidence_and_retries(
    run: &LabRun,
    role: LabRole,
    cost_summary: &LabCostSummary,
    evidence_refs: &[LabEvidenceRef],
    validation_retries: &[LabValidationRetry],
) -> LabContextPacket {
    build_lab_context_packet_with_evidence_retries_and_artifact_refs(
        run,
        role,
        cost_summary,
        evidence_refs,
        validation_retries,
        &[],
    )
}

pub fn build_lab_context_packet_with_evidence_retries_and_artifact_refs(
    run: &LabRun,
    role: LabRole,
    cost_summary: &LabCostSummary,
    evidence_refs: &[LabEvidenceRef],
    validation_retries: &[LabValidationRetry],
    artifact_gate_evidence_refs: &[LabContextEvidenceRefGroup],
) -> LabContextPacket {
    let layers = vec![
        LabContextLayer::new(
            "L0",
            "role-profile-and-project-charter",
            LabContextStability::StablePrefix,
            stable_role_project_context(run, role),
        ),
        LabContextLayer::new(
            "L1",
            "cost-policy",
            LabContextStability::StablePrefix,
            stable_cost_policy_context(run),
        ),
        LabContextLayer::new(
            "L2",
            "current-labrun-state",
            LabContextStability::DynamicTail,
            dynamic_run_state_context(run),
        ),
        LabContextLayer::new(
            "L3",
            "cost-and-cache-summary",
            LabContextStability::DynamicTail,
            dynamic_cost_context(cost_summary),
        ),
        LabContextLayer::new(
            "L4",
            "refs-only-evidence-index",
            LabContextStability::DynamicTail,
            dynamic_evidence_context(evidence_refs),
        ),
        LabContextLayer::new(
            "L5",
            "validation-retry-history",
            LabContextStability::DynamicTail,
            dynamic_validation_retry_context(validation_retries),
        ),
        LabContextLayer::new(
            "L6",
            "artifact-and-gate-evidence-refs",
            LabContextStability::DynamicTail,
            dynamic_artifact_gate_evidence_context(artifact_gate_evidence_refs),
        ),
    ];
    let stable_prefix = layers
        .iter()
        .filter(|layer| layer.stability == LabContextStability::StablePrefix)
        .map(|layer| layer.content.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");
    let dynamic_tail = layers
        .iter()
        .filter(|layer| layer.stability == LabContextStability::DynamicTail)
        .map(|layer| layer.content.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");
    let stable_prefix_tokens = layers
        .iter()
        .filter(|layer| layer.stability == LabContextStability::StablePrefix)
        .map(|layer| layer.estimated_tokens)
        .sum();
    let dynamic_tail_tokens = layers
        .iter()
        .filter(|layer| layer.stability == LabContextStability::DynamicTail)
        .map(|layer| layer.estimated_tokens)
        .sum();

    LabContextPacket {
        schema_version: LAB_SCHEMA_VERSION,
        lab_run_id: run.lab_run_id.clone(),
        role,
        created_at: Utc::now(),
        stable_prefix_fingerprint: stable_fingerprint(&stable_prefix),
        dynamic_tail_fingerprint: stable_fingerprint(&dynamic_tail),
        stable_prefix_tokens,
        dynamic_tail_tokens,
        total_estimated_tokens: stable_prefix_tokens.saturating_add(dynamic_tail_tokens),
        layers,
    }
}

pub fn evaluate_lab_context_compression(
    run: &LabRun,
    packet: &LabContextPacket,
) -> LabCompressionDecision {
    let budget = context_budget_for_role(run, packet.role);
    let usage_ratio_percent = if budget == 0 {
        0.0
    } else {
        (packet.total_estimated_tokens as f64 / budget as f64) * 100.0
    };
    let action = if usage_ratio_percent >= 80.0 {
        LabCompressionAction::Required
    } else if usage_ratio_percent >= 65.0 {
        LabCompressionAction::Recommend
    } else {
        LabCompressionAction::None
    };
    let reason = match action {
        LabCompressionAction::Required => {
            "context packet is at or above 80% of the role budget".to_string()
        }
        LabCompressionAction::Recommend => {
            "context packet is above 65% of the role budget; compress after this cycle".to_string()
        }
        LabCompressionAction::None => {
            "context packet is within the role budget threshold".to_string()
        }
    };

    LabCompressionDecision {
        schema_version: LAB_SCHEMA_VERSION,
        decision_id: String::new(),
        lab_run_id: run.lab_run_id.clone(),
        created_at: Utc::now(),
        role: packet.role,
        action,
        reason,
        context_budget_tokens: budget,
        packet_tokens: packet.total_estimated_tokens,
        usage_ratio_percent,
        stable_prefix_fingerprint: packet.stable_prefix_fingerprint.clone(),
        dynamic_tail_fingerprint: packet.dynamic_tail_fingerprint.clone(),
        cycle_id: Some(run.cycle_count.to_string()),
    }
}

fn context_budget_for_role(run: &LabRun, role: LabRole) -> u64 {
    match role {
        LabRole::Professor => run.cost_policy.professor_context_budget,
        LabRole::Postdoc => run.cost_policy.postdoc_context_budget,
        LabRole::Graduate => run.cost_policy.graduate_context_budget,
        LabRole::Runtime => run.cost_policy.postdoc_context_budget,
    }
}

fn stable_role_project_context(run: &LabRun, role: LabRole) -> String {
    let role_profile = match role {
        LabRole::Professor => &run.roles.professor,
        LabRole::Postdoc => &run.roles.postdoc,
        LabRole::Graduate => &run.roles.graduate,
        LabRole::Runtime => &run.roles.postdoc,
    };
    format!(
        "top_level_mode: {}\nrole: {:?}\nprofile: {}\nprompt_version: {}\nmodel_policy: {}\nproject_root: {}\nuser_goal: {}",
        run.top_level_mode,
        role,
        role_profile.profile,
        role_profile.prompt_version,
        role_profile.model_policy,
        run.project_root,
        run.user_goal
    )
}

fn stable_cost_policy_context(run: &LabRun) -> String {
    format!(
        "cost_policy.mode: {}\nmax_cycle_tokens: {}\nprofessor_context_budget: {}\npostdoc_context_budget: {}\ngraduate_context_budget: {}\nmeeting_context_budget: {}\nauto_compress_after_cycle: {}\nevidence_default: {}",
        run.cost_policy.mode,
        run.cost_policy.max_cycle_tokens,
        run.cost_policy.professor_context_budget,
        run.cost_policy.postdoc_context_budget,
        run.cost_policy.graduate_context_budget,
        run.cost_policy.meeting_context_budget,
        run.cost_policy.auto_compress_after_cycle,
        run.cost_policy.evidence_default
    )
}

fn dynamic_run_state_context(run: &LabRun) -> String {
    format!(
        "status: {:?}\ncurrent_stage: {}\ninternal_owner: {:?}\nneeds_user: {}\ncycle_count: {}\nfailure_count: {}\nactive_artifact_id: {}\nartifact_ids: {}\nopen_task_ids: {}",
        run.status,
        run.current_stage,
        run.internal_owner,
        run.needs_user,
        run.cycle_count,
        run.failure_count,
        run.resume_cursor.active_artifact_id.as_deref().unwrap_or("none"),
        run.artifact_ids.join(","),
        run.open_task_ids.join(",")
    )
}

fn dynamic_cost_context(summary: &LabCostSummary) -> String {
    format!(
        "requests: {}\ntotal_tokens: {}\nprompt_tokens: {}\ncompletion_tokens: {}\nreasoning_tokens: {}\ncached_tokens: {}\ncache_write_tokens: {}\ncache_miss_tokens: {}\ncache_hit_rate_percent: {:.1}\nestimated_cost_usd: {:.6}",
        summary.requests,
        summary.total_tokens,
        summary.prompt_tokens,
        summary.completion_tokens,
        summary.reasoning_tokens,
        summary.cached_tokens,
        summary.cache_write_tokens,
        summary.cache_miss_tokens,
        summary.cache_hit_rate_percent(),
        summary.estimated_cost_usd
    )
}

fn dynamic_evidence_context(evidence_refs: &[LabEvidenceRef]) -> String {
    if evidence_refs.is_empty() {
        return "evidence_refs: none".to_string();
    }
    let mut lines = vec![format!("evidence_ref_count: {}", evidence_refs.len())];
    for evidence in evidence_refs.iter().rev().take(20).rev() {
        lines.push(format!(
            "- id={} kind={:?} role={:?} ref={} summary={} hash={}",
            evidence.evidence_id,
            evidence.kind,
            evidence.role,
            evidence.reference,
            evidence.summary,
            evidence.metadata_hash.as_deref().unwrap_or("none")
        ));
    }
    lines.join("\n")
}

fn dynamic_validation_retry_context(validation_retries: &[LabValidationRetry]) -> String {
    if validation_retries.is_empty() {
        return "validation_retries: none".to_string();
    }
    let escalated = validation_retries
        .iter()
        .filter(|retry| retry.escalated)
        .count();
    let mut lines = vec![
        format!("validation_retry_count: {}", validation_retries.len()),
        format!("validation_retry_escalated_count: {}", escalated),
    ];
    for retry in validation_retries.iter().rev().take(20).rev() {
        lines.push(format!(
            "- id={} task={} attempt={} repair={} escalated={} summary={}",
            retry.retry_id,
            retry.task_id,
            retry.attempt,
            retry.repair_task_id.as_deref().unwrap_or("none"),
            retry.escalated,
            retry.validation_summary
        ));
    }
    lines.join("\n")
}

fn dynamic_artifact_gate_evidence_context(groups: &[LabContextEvidenceRefGroup]) -> String {
    if groups.is_empty() {
        return "artifact_gate_evidence_refs: none".to_string();
    }
    let mut lines = vec![format!(
        "artifact_gate_evidence_ref_group_count: {}",
        groups.len()
    )];
    for group in groups.iter().rev().take(30).rev() {
        let mut refs = group.evidence_refs.clone();
        refs.sort();
        refs.dedup();
        lines.push(format!(
            "- source={} refs={}",
            group.source,
            refs.iter()
                .rev()
                .take(20)
                .rev()
                .cloned()
                .collect::<Vec<_>>()
                .join(",")
        ));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lab::model::{
        LabCostUsage, LabEvidenceKind, LabEvidenceRef, LabProposal, LabValidationRetry,
    };

    #[test]
    fn dynamic_state_changes_only_dynamic_fingerprint() {
        let now = Utc::now();
        let proposal = LabProposal::new(
            "proposal_test".to_string(),
            "/tmp/project".to_string(),
            None,
            "Build LabRun".to_string(),
            now,
        );
        let mut run = LabRun::from_proposal("labrun_test".to_string(), &proposal, now);
        let cost = LabCostSummary::empty(&run.lab_run_id);

        let before = build_lab_context_packet(&run, LabRole::Professor, &cost);
        run.current_stage = "postdoc_plan".to_string();
        run.internal_owner = LabRole::Postdoc;
        let after = build_lab_context_packet(&run, LabRole::Professor, &cost);

        assert_eq!(
            before.stable_prefix_fingerprint,
            after.stable_prefix_fingerprint
        );
        assert_ne!(
            before.dynamic_tail_fingerprint,
            after.dynamic_tail_fingerprint
        );
    }

    #[test]
    fn cost_usage_changes_dynamic_fingerprint() {
        let now = Utc::now();
        let proposal = LabProposal::new(
            "proposal_test".to_string(),
            "/tmp/project".to_string(),
            None,
            "Build LabRun".to_string(),
            now,
        );
        let run = LabRun::from_proposal("labrun_test".to_string(), &proposal, now);
        let mut cost = LabCostSummary::empty(&run.lab_run_id);
        let before = build_lab_context_packet(&run, LabRole::Postdoc, &cost);
        cost.add_usage(&LabCostUsage {
            schema_version: LAB_SCHEMA_VERSION,
            usage_id: "usage_test".to_string(),
            lab_run_id: run.lab_run_id.clone(),
            created_at: now,
            role: LabRole::Postdoc,
            cycle_id: Some("0".to_string()),
            meeting_id: None,
            model: "test-model".to_string(),
            prompt_tokens: 100,
            completion_tokens: 20,
            reasoning_tokens: 5,
            cached_tokens: 60,
            cache_write_tokens: 10,
            cache_miss_tokens: 40,
            total_tokens: 125,
            estimated_cost_usd: 0.001,
            note: None,
        });
        let after = build_lab_context_packet(&run, LabRole::Postdoc, &cost);

        assert_eq!(
            before.stable_prefix_fingerprint,
            after.stable_prefix_fingerprint
        );
        assert_ne!(
            before.dynamic_tail_fingerprint,
            after.dynamic_tail_fingerprint
        );
    }

    #[test]
    fn evidence_refs_live_in_dynamic_tail() {
        let now = Utc::now();
        let proposal = LabProposal::new(
            "proposal_test".to_string(),
            "/tmp/project".to_string(),
            None,
            "Build LabRun".to_string(),
            now,
        );
        let run = LabRun::from_proposal("labrun_test".to_string(), &proposal, now);
        let cost = LabCostSummary::empty(&run.lab_run_id);
        let before = build_lab_context_packet_with_evidence(&run, LabRole::Postdoc, &cost, &[]);
        let evidence = vec![LabEvidenceRef {
            schema_version: LAB_SCHEMA_VERSION,
            evidence_id: "evidence_test".to_string(),
            lab_run_id: run.lab_run_id.clone(),
            created_at: now,
            kind: LabEvidenceKind::Log,
            role: LabRole::Postdoc,
            reference: "target/check.log".to_string(),
            summary: "cargo check passed".to_string(),
            artifact_id: None,
            cycle_id: Some("0".to_string()),
            metadata_hash: Some("hash".to_string()),
            estimated_summary_tokens: 4,
        }];
        let after =
            build_lab_context_packet_with_evidence(&run, LabRole::Postdoc, &cost, &evidence);

        assert_eq!(
            before.stable_prefix_fingerprint,
            after.stable_prefix_fingerprint
        );
        assert_ne!(
            before.dynamic_tail_fingerprint,
            after.dynamic_tail_fingerprint
        );
        assert!(after
            .layers
            .iter()
            .any(|layer| { layer.layer == "L4" && layer.content.contains("cargo check passed") }));
    }

    #[test]
    fn validation_retries_live_in_dynamic_tail() {
        let now = Utc::now();
        let proposal = LabProposal::new(
            "proposal_test".to_string(),
            "/tmp/project".to_string(),
            None,
            "Build LabRun".to_string(),
            now,
        );
        let run = LabRun::from_proposal("labrun_test".to_string(), &proposal, now);
        let cost = LabCostSummary::empty(&run.lab_run_id);
        let before = build_lab_context_packet_with_evidence_and_retries(
            &run,
            LabRole::Postdoc,
            &cost,
            &[],
            &[],
        );
        let retries = vec![LabValidationRetry {
            schema_version: LAB_SCHEMA_VERSION,
            retry_id: "retry_test".to_string(),
            lab_run_id: run.lab_run_id.clone(),
            task_id: "gradtask_test".to_string(),
            created_at: now,
            attempt: 1,
            validation_summary: "cargo check failed".to_string(),
            repair_task_id: Some("gradtask_repair".to_string()),
            escalated: false,
        }];
        let after = build_lab_context_packet_with_evidence_and_retries(
            &run,
            LabRole::Postdoc,
            &cost,
            &[],
            &retries,
        );

        assert_eq!(
            before.stable_prefix_fingerprint,
            after.stable_prefix_fingerprint
        );
        assert_ne!(
            before.dynamic_tail_fingerprint,
            after.dynamic_tail_fingerprint
        );
        assert!(after
            .layers
            .iter()
            .any(|layer| { layer.layer == "L5" && layer.content.contains("cargo check failed") }));
    }

    #[test]
    fn artifact_gate_evidence_refs_live_in_dynamic_tail() {
        let now = Utc::now();
        let proposal = LabProposal::new(
            "proposal_test".to_string(),
            "/tmp/project".to_string(),
            None,
            "Build LabRun".to_string(),
            now,
        );
        let run = LabRun::from_proposal("labrun_test".to_string(), &proposal, now);
        let cost = LabCostSummary::empty(&run.lab_run_id);
        let before = build_lab_context_packet_with_evidence_retries_and_artifact_refs(
            &run,
            LabRole::Professor,
            &cost,
            &[],
            &[],
            &[],
        );
        let artifact_gate_refs = vec![LabContextEvidenceRefGroup {
            source: "artifact:artifact_postdoc_summary".to_string(),
            evidence_refs: vec![
                "evidence_test".to_string(),
                "gate:postdoc_review".to_string(),
            ],
        }];
        let after = build_lab_context_packet_with_evidence_retries_and_artifact_refs(
            &run,
            LabRole::Professor,
            &cost,
            &[],
            &[],
            &artifact_gate_refs,
        );

        assert_eq!(
            before.stable_prefix_fingerprint,
            after.stable_prefix_fingerprint
        );
        assert_ne!(
            before.dynamic_tail_fingerprint,
            after.dynamic_tail_fingerprint
        );
        assert!(after.layers.iter().any(|layer| {
            layer.layer == "L6"
                && layer.content.contains("artifact:artifact_postdoc_summary")
                && layer.content.contains("gate:postdoc_review")
        }));
    }

    #[test]
    fn compression_decision_uses_role_budget_thresholds() {
        let now = Utc::now();
        let proposal = LabProposal::new(
            "proposal_test".to_string(),
            "/tmp/project".to_string(),
            None,
            "Build LabRun".to_string(),
            now,
        );
        let mut run = LabRun::from_proposal("labrun_test".to_string(), &proposal, now);
        run.cost_policy.professor_context_budget = 10;
        let cost = LabCostSummary::empty(&run.lab_run_id);
        let packet = build_lab_context_packet(&run, LabRole::Professor, &cost);

        let decision = evaluate_lab_context_compression(&run, &packet);

        assert_eq!(decision.action, LabCompressionAction::Required);
        assert_eq!(decision.context_budget_tokens, 10);
        assert_eq!(decision.packet_tokens, packet.total_estimated_tokens);
        assert_eq!(
            decision.stable_prefix_fingerprint,
            packet.stable_prefix_fingerprint
        );
    }
}
