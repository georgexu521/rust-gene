use crate::engine::evidence_ledger::{EvidenceLedger, ToolExecutionRecord, ToolExecutionStatus};
use crate::engine::intent_router::{IntentKind, IntentRoute, WorkflowKind};
use crate::engine::verification_proof::{VerificationProof, VerificationProofStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FinalAnswerClaimKind {
    MutationCompleted,
    ValidationPassed,
    CommitCreated,
    Pushed,
    #[allow(dead_code)]
    FileInspected,
    TaskCompleted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalAnswerClaim {
    pub kind: FinalAnswerClaimKind,
    pub span_preview: String,
    pub command: Option<String>,
    pub path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FinalAnswerClaimGateDecision {
    Pass,
    Repair {
        observation: FinalAnswerClaimObservation,
    },
    Downgrade {
        observation: FinalAnswerClaimObservation,
        user_visible_status: &'static str,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalAnswerClaimObservation {
    pub unsupported_claims: Vec<FinalAnswerClaim>,
    pub runtime_evidence: FinalAnswerEvidenceSnapshot,
    pub route_workflow: String,
    pub required_validation_commands: Vec<String>,
}

impl FinalAnswerClaimObservation {
    pub fn to_recent_observation_text(&self) -> String {
        let mut text = String::from("Final answer claim gate failed.\n\nUnsupported claims:\n");
        for claim in &self.unsupported_claims {
            text.push_str(&format!(
                "- {}: {}\n",
                claim_kind_label(claim.kind),
                claim.span_preview
            ));
        }
        text.push_str("\nRuntime evidence:\n");
        text.push_str(&format!("- route={}\n", self.route_workflow));
        text.push_str(&format!(
            "- changed_files={}\n",
            self.runtime_evidence.changed_files.len()
        ));
        text.push_str(&format!(
            "- successful_mutation_tools={}\n",
            self.runtime_evidence.successful_mutation_tools
        ));
        text.push_str(&format!(
            "- validation_records={}\n",
            self.runtime_evidence.validation_records.len()
        ));
        text.push_str(&format!(
            "- verification_status={}\n",
            self.runtime_evidence.verification_status
        ));
        if !self.required_validation_commands.is_empty() {
            text.push_str(&format!(
                "- required_validation={:?}\n",
                self.required_validation_commands
            ));
        }
        text.push_str("\nRequired next action:\n");
        text.push_str("- Continue the task.\n");
        text.push_str("- Inspect the target files if needed.\n");
        text.push_str("- Make an actual focused change or explain why no change is required.\n");
        if !self.required_validation_commands.is_empty() {
            text.push_str("- Run or request the required validation.\n");
        }
        text.push_str("- Do not claim completion until the evidence supports it.\n");
        text
    }
}

fn claim_kind_label(kind: FinalAnswerClaimKind) -> &'static str {
    match kind {
        FinalAnswerClaimKind::MutationCompleted => "mutation_completed",
        FinalAnswerClaimKind::ValidationPassed => "validation_passed",
        FinalAnswerClaimKind::CommitCreated => "commit_created",
        FinalAnswerClaimKind::Pushed => "pushed",
        FinalAnswerClaimKind::FileInspected => "file_inspected",
        FinalAnswerClaimKind::TaskCompleted => "task_completed",
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalAnswerEvidenceSnapshot {
    pub changed_files: Vec<String>,
    pub successful_mutation_tools: usize,
    pub validation_records: Vec<ValidationEvidenceSnapshot>,
    pub verification_status: String,
    pub git_commit_records: Vec<String>,
    pub git_push_records: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationEvidenceSnapshot {
    pub command: Option<String>,
    pub passed: bool,
}

#[derive(Debug, Clone)]
pub struct FinalAnswerClaimGateInput<'a> {
    pub content: &'a str,
    pub route: &'a IntentRoute,
    pub evidence_ledger: &'a EvidenceLedger,
    pub verification_proof: &'a VerificationProof,
    pub required_validation_commands: &'a [String],
    pub repair_used: bool,
    pub iterations_used: usize,
    pub max_iterations: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub struct FinalAnswerClaimGateBudget {
    pub max_repairs_per_turn: u8,
    pub max_repairs_per_goal_step: u8,
}

impl Default for FinalAnswerClaimGateBudget {
    fn default() -> Self {
        Self {
            max_repairs_per_turn: 1,
            max_repairs_per_goal_step: 1,
        }
    }
}

pub struct FinalAnswerClaimGate;

impl FinalAnswerClaimGate {
    pub fn evaluate(input: FinalAnswerClaimGateInput<'_>) -> FinalAnswerClaimGateDecision {
        let claims = extract_claims(input.content);
        if claims.is_empty() {
            return FinalAnswerClaimGateDecision::Pass;
        }

        let evidence = build_evidence_snapshot(input.evidence_ledger, input.verification_proof);
        let unsupported: Vec<FinalAnswerClaim> = claims
            .into_iter()
            .filter(|claim| {
                !is_claim_supported(
                    claim,
                    &evidence,
                    input.route,
                    input.verification_proof,
                    input.required_validation_commands,
                )
            })
            .collect();

        if unsupported.is_empty() {
            return FinalAnswerClaimGateDecision::Pass;
        }

        let observation = FinalAnswerClaimObservation {
            unsupported_claims: unsupported,
            runtime_evidence: evidence,
            route_workflow: format!("{:?}", input.route.workflow),
            required_validation_commands: input.required_validation_commands.to_vec(),
        };

        if !should_attempt_repair(input) {
            return FinalAnswerClaimGateDecision::Downgrade {
                observation,
                user_visible_status: "not_verified",
            };
        }

        FinalAnswerClaimGateDecision::Repair { observation }
    }
}

fn extract_claims(content: &str) -> Vec<FinalAnswerClaim> {
    let lower = content.to_lowercase();
    let mut claims = Vec::new();

    // Mutation claims — English
    if contains_any(
        &lower,
        &[
            "i fixed",
            "i've fixed",
            "i changed",
            "i updated",
            "i created",
            "i implemented",
        ],
    ) {
        claims.push(FinalAnswerClaim {
            kind: FinalAnswerClaimKind::MutationCompleted,
            span_preview: extract_mutation_preview(content),
            command: None,
            path: None,
        });
    }

    // Mutation claims — Chinese
    if contains_any(
        content,
        &["我修好了", "已经修复", "已经修改", "我改了", "已经实现"],
    ) {
        claims.push(FinalAnswerClaim {
            kind: FinalAnswerClaimKind::MutationCompleted,
            span_preview: extract_mutation_preview(content),
            command: None,
            path: None,
        });
    }

    // Validation claims — English
    if contains_any(
        &lower,
        &[
            "tests passed",
            "test passed",
            "validation passed",
            "checks passed",
            "build passed",
        ],
    ) || lower.contains("cargo test") && lower.contains("passed")
        || lower.contains("clippy") && lower.contains("passed")
    {
        claims.push(FinalAnswerClaim {
            kind: FinalAnswerClaimKind::ValidationPassed,
            span_preview: extract_validation_preview(content),
            command: extract_command_mention(content),
            path: None,
        });
    }

    // Validation claims — Chinese
    if contains_any(content, &["测试通过", "验证通过", "跑过测试", "检查通过"])
        || (content.contains("cargo test") || content.contains("clippy"))
            && content.contains("通过")
    {
        claims.push(FinalAnswerClaim {
            kind: FinalAnswerClaimKind::ValidationPassed,
            span_preview: extract_validation_preview(content),
            command: extract_command_mention(content),
            path: None,
        });
    }

    // Commit claims — English
    if contains_any(&lower, &["committed", "created commit", "pushed to github"])
        || (lower.contains("pushed")
            && !lower.contains("you can push")
            && !lower.contains("should push"))
    {
        claims.push(FinalAnswerClaim {
            kind: if lower.contains("pushed") {
                FinalAnswerClaimKind::Pushed
            } else {
                FinalAnswerClaimKind::CommitCreated
            },
            span_preview: extract_commit_preview(content),
            command: None,
            path: None,
        });
    }

    // Commit claims — Chinese
    if contains_any(
        content,
        &["已经提交", "提交好了", "已经推送", "推送到 GitHub"],
    ) {
        claims.push(FinalAnswerClaim {
            kind: if content.contains("推送") || content.contains("推送到") {
                FinalAnswerClaimKind::Pushed
            } else {
                FinalAnswerClaimKind::CommitCreated
            },
            span_preview: extract_commit_preview(content),
            command: None,
            path: None,
        });
    }

    // Task completion claims — scoped to action workflows
    if (contains_any(&lower, &["done", "all set", "everything is fixed"])
        || contains_any(content, &["完成了", "搞定了", "全部完成"]))
        && !is_read_only_route_suggestion(&lower)
    {
        claims.push(FinalAnswerClaim {
            kind: FinalAnswerClaimKind::TaskCompleted,
            span_preview: "completion claim".to_string(),
            command: None,
            path: None,
        });
    }

    // Deduplicate by kind
    let mut seen = std::collections::HashSet::new();
    claims.retain(|claim| seen.insert(claim.kind));

    claims
}

fn is_read_only_route_suggestion(lower: &str) -> bool {
    // Avoid triggering on suggestions/plans
    lower.contains("you can fix")
        || lower.contains("should pass after")
        || lower.contains("would change")
        || lower.contains("cannot verify")
        || lower.contains("no changes were made")
        || lower.contains("this is a plan")
}

fn is_claim_supported(
    claim: &FinalAnswerClaim,
    evidence: &FinalAnswerEvidenceSnapshot,
    route: &IntentRoute,
    verification_proof: &VerificationProof,
    required_validation_commands: &[String],
) -> bool {
    match claim.kind {
        FinalAnswerClaimKind::MutationCompleted => {
            !evidence.changed_files.is_empty() || evidence.successful_mutation_tools > 0
        }
        FinalAnswerClaimKind::ValidationPassed => validation_claim_supported(
            claim,
            evidence,
            verification_proof,
            required_validation_commands,
        ),
        FinalAnswerClaimKind::CommitCreated => !evidence.git_commit_records.is_empty(),
        FinalAnswerClaimKind::Pushed => !evidence.git_push_records.is_empty(),
        FinalAnswerClaimKind::FileInspected => {
            // Read/inspection claims are allowed on read-only routes or when file evidence exists
            matches!(
                route.workflow,
                WorkflowKind::Direct | WorkflowKind::Research | WorkflowKind::Planning
            ) || !evidence.changed_files.is_empty()
        }
        FinalAnswerClaimKind::TaskCompleted => match route.workflow {
            WorkflowKind::Direct | WorkflowKind::Research | WorkflowKind::Planning => true,
            WorkflowKind::CodeChange | WorkflowKind::BugFix => {
                let has_mutation =
                    !evidence.changed_files.is_empty() || evidence.successful_mutation_tools > 0;
                let validation_ok = matches!(
                    verification_proof.status,
                    VerificationProofStatus::Verified | VerificationProofStatus::NotApplicable
                );
                has_mutation && validation_ok
            }
            WorkflowKind::Delegation => {
                matches!(
                    verification_proof.status,
                    VerificationProofStatus::Verified | VerificationProofStatus::NotApplicable
                )
            }
        },
    }
}

fn validation_claim_supported(
    claim: &FinalAnswerClaim,
    evidence: &FinalAnswerEvidenceSnapshot,
    verification_proof: &VerificationProof,
    required_validation_commands: &[String],
) -> bool {
    let passed_records = evidence
        .validation_records
        .iter()
        .filter(|record| record.passed)
        .collect::<Vec<_>>();

    let Some(claimed_command) = claim.command.as_deref() else {
        if !required_validation_commands.is_empty() {
            return matches!(verification_proof.status, VerificationProofStatus::Verified)
                && required_validation_commands.iter().all(|required| {
                    passed_records
                        .iter()
                        .any(|record| record.command_matches(required))
                });
        }

        let proof_verified = matches!(verification_proof.status, VerificationProofStatus::Verified);
        return (!passed_records.is_empty() && proof_verified)
            || verification_proof.validation_passed > 0;
    };

    passed_records
        .iter()
        .any(|record| record.command_matches(claimed_command))
}

fn should_attempt_repair(input: FinalAnswerClaimGateInput<'_>) -> bool {
    // Only repair on action workflows where continued work is expected
    let action_workflow = matches!(
        input.route.workflow,
        WorkflowKind::CodeChange | WorkflowKind::BugFix | WorkflowKind::Delegation
    );
    if !action_workflow {
        return false;
    }

    // Don't repair if budget exhausted
    if input.iterations_used >= input.max_iterations {
        return false;
    }

    // Don't repair if already used repair for this turn
    if input.repair_used {
        return false;
    }

    // Don't repair on lightweight side questions or read-only routes
    if matches!(
        input.route.workflow,
        WorkflowKind::Direct | WorkflowKind::Research | WorkflowKind::Planning
    ) && input.route.intent != IntentKind::CodeChange
    {
        return false;
    }

    true
}

fn build_evidence_snapshot(
    ledger: &EvidenceLedger,
    verification_proof: &VerificationProof,
) -> FinalAnswerEvidenceSnapshot {
    let changed_files = ledger.changed_files();
    let successful_mutation_tools = ledger
        .tool_execution_records()
        .iter()
        .filter(|record| is_effective_mutation_record(record))
        .count();

    let validation_records: Vec<ValidationEvidenceSnapshot> = ledger
        .validation_facts()
        .iter()
        .map(|fact| ValidationEvidenceSnapshot {
            command: fact.command.clone(),
            passed: fact.passed,
        })
        .collect();

    // Git commit/push records from command facts
    let git_commit_records: Vec<String> = ledger
        .tool_execution_records()
        .iter()
        .filter(|record| {
            record.status == crate::engine::evidence_ledger::ToolExecutionStatus::Completed
                && record.tool == "bash"
                && record
                    .command
                    .as_deref()
                    .is_some_and(|cmd| cmd.contains("git commit"))
        })
        .filter_map(|record| record.command.clone())
        .collect();

    let git_push_records: Vec<String> = ledger
        .tool_execution_records()
        .iter()
        .filter(|record| {
            record.status == crate::engine::evidence_ledger::ToolExecutionStatus::Completed
                && record.tool == "bash"
                && record
                    .command
                    .as_deref()
                    .is_some_and(|cmd| cmd.contains("git push"))
        })
        .filter_map(|record| record.command.clone())
        .collect();

    FinalAnswerEvidenceSnapshot {
        changed_files,
        successful_mutation_tools,
        validation_records,
        verification_status: verification_proof.status.label().to_string(),
        git_commit_records,
        git_push_records,
    }
}

fn is_effective_mutation_record(record: &ToolExecutionRecord) -> bool {
    if record.status != ToolExecutionStatus::Completed {
        return false;
    }

    if !record.changed_paths.is_empty() {
        return true;
    }

    matches!(
        record.tool.as_str(),
        "file_write" | "file_edit" | "file_patch"
    ) && record.read_only != Some(true)
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

fn extract_mutation_preview(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    for line in &lines {
        let lower = line.to_lowercase();
        if lower.contains("fixed")
            || lower.contains("changed")
            || lower.contains("updated")
            || lower.contains("created")
            || lower.contains("implemented")
            || line.contains("修复")
            || line.contains("修改")
            || line.contains("实现")
        {
            let trimmed = line.trim();
            if trimmed.len() > 120 {
                return format!("{}...", &trimmed[..120]);
            }
            return trimmed.to_string();
        }
    }
    "mutation claim".to_string()
}

fn extract_validation_preview(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    for line in &lines {
        let lower = line.to_lowercase();
        if lower.contains("test") && lower.contains("pass")
            || lower.contains("validation") && lower.contains("pass")
            || lower.contains("clippy") && lower.contains("pass")
            || lower.contains("build") && lower.contains("pass")
            || line.contains("测试") && line.contains("通过")
            || line.contains("验证") && line.contains("通过")
            || line.contains("检查") && line.contains("通过")
        {
            let trimmed = line.trim();
            if trimmed.len() > 120 {
                return format!("{}...", &trimmed[..120]);
            }
            return trimmed.to_string();
        }
    }
    "validation claim".to_string()
}

fn extract_commit_preview(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    for line in &lines {
        let lower = line.to_lowercase();
        if lower.contains("commit")
            || lower.contains("pushed")
            || line.contains("提交")
            || line.contains("推送")
        {
            let trimmed = line.trim();
            if trimmed.len() > 120 {
                return format!("{}...", &trimmed[..120]);
            }
            return trimmed.to_string();
        }
    }
    "commit claim".to_string()
}

fn extract_command_mention(content: &str) -> Option<String> {
    let lower = content.to_lowercase();
    if lower.contains("cargo test") {
        Some("cargo test".to_string())
    } else if lower.contains("clippy") {
        Some("clippy".to_string())
    } else if lower.contains("npm test") {
        Some("npm test".to_string())
    } else {
        None
    }
}

fn command_identity(command: &str) -> String {
    command
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn command_matches_record(recorded: &str, claimed_or_required: &str) -> bool {
    let recorded = command_identity(recorded);
    let expected = command_identity(claimed_or_required);
    recorded == expected || recorded.contains(&expected) || expected.contains(&recorded)
}

impl ValidationEvidenceSnapshot {
    fn command_matches(&self, claimed_or_required: &str) -> bool {
        self.command
            .as_deref()
            .is_some_and(|command| command_matches_record(command, claimed_or_required))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::intent_router::{
        IntentKind, IntentRoute, ReasoningPolicy, RetrievalPolicy, RiskLevel, WorkflowKind,
    };
    use crate::engine::verification_proof::VerificationProof;
    use crate::services::api::ToolCall;
    use crate::tools::ToolResult;

    fn code_change_route() -> IntentRoute {
        IntentRoute {
            intent: IntentKind::CodeChange,
            confidence: 0.90,
            workflow: WorkflowKind::CodeChange,
            retrieval: RetrievalPolicy::Project,
            reasoning: ReasoningPolicy::Medium,
            risk: RiskLevel::High,
            recommended_tools: Vec::new(),
            dependency_install_intent: false,
            mcp_auth_intent: false,
            reason: "code change".to_string(),
        }
    }

    fn direct_route() -> IntentRoute {
        IntentRoute {
            intent: IntentKind::DirectAnswer,
            confidence: 0.90,
            workflow: WorkflowKind::Direct,
            retrieval: RetrievalPolicy::Light,
            reasoning: ReasoningPolicy::Low,
            risk: RiskLevel::Low,
            recommended_tools: Vec::new(),
            dependency_install_intent: false,
            mcp_auth_intent: false,
            reason: "direct answer".to_string(),
        }
    }

    fn gate_input<'a>(
        content: &'a str,
        route: &'a IntentRoute,
        ledger: &'a EvidenceLedger,
        proof: &'a VerificationProof,
    ) -> FinalAnswerClaimGateInput<'a> {
        FinalAnswerClaimGateInput {
            content,
            route,
            evidence_ledger: ledger,
            verification_proof: proof,
            required_validation_commands: &[],
            repair_used: false,
            iterations_used: 1,
            max_iterations: 10,
        }
    }

    fn empty_ledger_and_proof() -> (EvidenceLedger, VerificationProof) {
        (
            EvidenceLedger::new(),
            VerificationProof::new(VerificationProofStatus::NotRun, "no validation"),
        )
    }

    fn tool_call(name: &str, args: serde_json::Value) -> ToolCall {
        ToolCall {
            id: "call_1".to_string(),
            name: name.to_string(),
            arguments: args,
        }
    }

    #[test]
    fn unsupported_mutation_claim_triggers_repair() {
        let route = code_change_route();
        let (ledger, proof) = empty_ledger_and_proof();
        let input = gate_input("I fixed the bug.", &route, &ledger, &proof);
        let decision = FinalAnswerClaimGate::evaluate(input);

        assert!(
            matches!(decision, FinalAnswerClaimGateDecision::Repair { .. }),
            "expected Repair, got {:?}",
            decision
        );
    }

    #[test]
    fn unsupported_validation_claim_triggers_repair() {
        let route = code_change_route();
        let (ledger, proof) = empty_ledger_and_proof();
        let input = gate_input("Tests passed.", &route, &ledger, &proof);
        let decision = FinalAnswerClaimGate::evaluate(input);

        assert!(
            matches!(decision, FinalAnswerClaimGateDecision::Repair { .. }),
            "expected Repair, got {:?}",
            decision
        );
    }

    #[test]
    fn unsupported_commit_claim_triggers_repair() {
        let route = code_change_route();
        let (ledger, proof) = empty_ledger_and_proof();
        let input = gate_input("I committed the changes.", &route, &ledger, &proof);
        let decision = FinalAnswerClaimGate::evaluate(input);

        assert!(
            matches!(decision, FinalAnswerClaimGateDecision::Repair { .. }),
            "expected Repair, got {:?}",
            decision
        );
    }

    #[test]
    fn no_claim_passes_through() {
        let route = code_change_route();
        let (ledger, proof) = empty_ledger_and_proof();
        let input = gate_input("Here is what I found.", &route, &ledger, &proof);
        let decision = FinalAnswerClaimGate::evaluate(input);

        assert_eq!(decision, FinalAnswerClaimGateDecision::Pass);
    }

    #[test]
    fn read_only_answer_with_no_local_state_passes() {
        let route = direct_route();
        let (ledger, proof) = empty_ledger_and_proof();
        let input = gate_input("The answer is 42.", &route, &ledger, &proof);
        let decision = FinalAnswerClaimGate::evaluate(input);

        assert_eq!(decision, FinalAnswerClaimGateDecision::Pass);
    }

    #[test]
    fn chinese_unsupported_claim_triggers_repair() {
        let route = code_change_route();
        let (ledger, proof) = empty_ledger_and_proof();
        let input = gate_input("已经修复，测试通过", &route, &ledger, &proof);
        let decision = FinalAnswerClaimGate::evaluate(input);

        assert!(
            matches!(decision, FinalAnswerClaimGateDecision::Repair { .. }),
            "expected Repair, got {:?}",
            decision
        );
    }

    #[test]
    fn repair_already_used_downgrades() {
        let route = code_change_route();
        let (ledger, proof) = empty_ledger_and_proof();
        let mut input = gate_input("I fixed it.", &route, &ledger, &proof);
        input.repair_used = true;
        let decision = FinalAnswerClaimGate::evaluate(input);

        assert!(
            matches!(decision, FinalAnswerClaimGateDecision::Downgrade { .. }),
            "expected Downgrade, got {:?}",
            decision
        );
    }

    #[test]
    fn supported_mutation_claim_passes() {
        let route = code_change_route();
        let mut ledger = EvidenceLedger::new();
        ledger.record_changed_files(&[std::path::PathBuf::from("src/foo.rs")]);

        let input = FinalAnswerClaimGateInput {
            content: "I fixed the bug.",
            route: &route,
            evidence_ledger: &ledger,
            verification_proof: &VerificationProof::new(
                VerificationProofStatus::Verified,
                "validation passed",
            ),
            required_validation_commands: &[],
            repair_used: false,
            iterations_used: 1,
            max_iterations: 10,
        };
        let decision = FinalAnswerClaimGate::evaluate(input);

        assert_eq!(decision, FinalAnswerClaimGateDecision::Pass);
    }

    #[test]
    fn read_only_bash_does_not_support_mutation_claim() {
        let route = code_change_route();
        let mut ledger = EvidenceLedger::new();
        ledger.record_tool_result(
            &tool_call("bash", serde_json::json!({"command": "ls -la"})),
            &ToolResult::success_with_data(
                "listed files",
                serde_json::json!({
                    "tool_summary": {
                        "operation_kind": "list",
                        "read_only": true
                    }
                }),
            ),
        );
        let proof = VerificationProof::new(VerificationProofStatus::Verified, "validation passed");

        let input = gate_input("I fixed the bug.", &route, &ledger, &proof);
        let decision = FinalAnswerClaimGate::evaluate(input);

        assert!(
            matches!(decision, FinalAnswerClaimGateDecision::Repair { .. }),
            "expected Repair, got {:?}",
            decision
        );
    }

    #[test]
    fn supported_validation_claim_passes() {
        let route = code_change_route();
        let mut ledger = EvidenceLedger::new();
        ledger.record_validation_result("bash", Some("cargo test -q"), true, "tests passed");

        let input = FinalAnswerClaimGateInput {
            content: "Tests passed.",
            route: &route,
            evidence_ledger: &ledger,
            verification_proof: &VerificationProof::new(
                VerificationProofStatus::Verified,
                "required validation passed",
            ),
            required_validation_commands: &["cargo test -q".to_string()],
            repair_used: false,
            iterations_used: 1,
            max_iterations: 10,
        };
        let decision = FinalAnswerClaimGate::evaluate(input);

        assert_eq!(decision, FinalAnswerClaimGateDecision::Pass);
    }

    #[test]
    fn validation_claim_requires_matching_command() {
        let route = code_change_route();
        let mut ledger = EvidenceLedger::new();
        ledger.record_validation_result("bash", Some("cargo test -q"), true, "tests passed");

        let input = FinalAnswerClaimGateInput {
            content: "Clippy passed.",
            route: &route,
            evidence_ledger: &ledger,
            verification_proof: &VerificationProof::new(
                VerificationProofStatus::Verified,
                "cargo test passed",
            ),
            required_validation_commands: &["cargo test -q".to_string()],
            repair_used: false,
            iterations_used: 1,
            max_iterations: 10,
        };
        let decision = FinalAnswerClaimGate::evaluate(input);

        assert!(
            matches!(decision, FinalAnswerClaimGateDecision::Repair { .. }),
            "expected Repair, got {:?}",
            decision
        );
    }

    #[test]
    fn not_applicable_proof_does_not_support_validation_claim() {
        let route = code_change_route();
        let ledger = EvidenceLedger::new();
        let proof = VerificationProof::new(
            VerificationProofStatus::NotApplicable,
            "no validation needed",
        );
        let input = gate_input("Tests passed.", &route, &ledger, &proof);
        let decision = FinalAnswerClaimGate::evaluate(input);

        assert!(
            matches!(decision, FinalAnswerClaimGateDecision::Repair { .. }),
            "expected Repair, got {:?}",
            decision
        );
    }

    #[test]
    fn vague_done_claim_is_task_completion_only() {
        let claims = extract_claims("Done.");

        assert!(claims
            .iter()
            .any(|claim| claim.kind == FinalAnswerClaimKind::TaskCompleted));
        assert!(!claims
            .iter()
            .any(|claim| claim.kind == FinalAnswerClaimKind::MutationCompleted));
    }

    #[test]
    fn no_changes_claim_with_empty_ledger_passes() {
        let route = code_change_route();
        let (ledger, proof) = empty_ledger_and_proof();
        let input = gate_input("I did not modify any files.", &route, &ledger, &proof);
        let decision = FinalAnswerClaimGate::evaluate(input);

        assert_eq!(decision, FinalAnswerClaimGateDecision::Pass);
    }

    #[test]
    fn iteration_budget_exhausted_downgrades() {
        let route = code_change_route();
        let (ledger, proof) = empty_ledger_and_proof();
        let mut input = gate_input("I fixed it.", &route, &ledger, &proof);
        input.iterations_used = 10;
        input.max_iterations = 10;
        let decision = FinalAnswerClaimGate::evaluate(input);

        assert!(
            matches!(decision, FinalAnswerClaimGateDecision::Downgrade { .. }),
            "expected Downgrade, got {:?}",
            decision
        );
    }

    #[test]
    fn observation_text_is_structured() {
        let route = code_change_route();
        let (ledger, proof) = empty_ledger_and_proof();
        let input = gate_input("I fixed it and tests passed.", &route, &ledger, &proof);
        let decision = FinalAnswerClaimGate::evaluate(input);

        if let FinalAnswerClaimGateDecision::Repair { observation } = decision {
            let text = observation.to_recent_observation_text();
            assert!(text.contains("Final answer claim gate failed"));
            assert!(text.contains("Unsupported claims:"));
            assert!(text.contains("Runtime evidence:"));
            assert!(text.contains("Required next action:"));
            assert!(text.contains("changed_files=0"));
        } else {
            panic!("expected Repair decision");
        }
    }
}
