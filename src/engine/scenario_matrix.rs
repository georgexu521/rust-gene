//! Deterministic parity scenarios that should graduate into replay fixtures.

use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScenarioKind {
    FileEditRewind,
    BashBackgroundTask,
    PermissionDenialRetry,
    CompactionBoundary,
    SubagentWorktreeWorker,
    McpAuthRepair,
}

impl ScenarioKind {
    pub const fn id(self) -> &'static str {
        match self {
            Self::FileEditRewind => "file_edit_rewind",
            Self::BashBackgroundTask => "bash_background_task",
            Self::PermissionDenialRetry => "permission_denial_retry",
            Self::CompactionBoundary => "compaction_boundary",
            Self::SubagentWorktreeWorker => "subagent_worktree_worker",
            Self::McpAuthRepair => "mcp_auth_repair",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceSurface {
    TraceEvent,
    RuntimePanel,
    RecoveryPlan,
    ToolMetadata,
    SlashCommand,
    SessionStore,
}

impl EvidenceSurface {
    pub const fn label(self) -> &'static str {
        match self {
            Self::TraceEvent => "trace",
            Self::RuntimePanel => "panel",
            Self::RecoveryPlan => "recovery",
            Self::ToolMetadata => "tool",
            Self::SlashCommand => "command",
            Self::SessionStore => "session",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayStatus {
    RuntimeMapped,
    ReplayFixtureReady,
}

impl ReplayStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::RuntimeMapped => "runtime_mapped",
            Self::ReplayFixtureReady => "replay_fixture_ready",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalBaselineStatus {
    DeferredUntilReplayFixture,
    Ready,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ScenarioEvidence {
    pub surface: EvidenceSurface,
    pub target: &'static str,
    pub required: bool,
    pub reason: &'static str,
}

impl ScenarioEvidence {
    pub const fn required(
        surface: EvidenceSurface,
        target: &'static str,
        reason: &'static str,
    ) -> Self {
        Self {
            surface,
            target,
            required: true,
            reason,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct DeterministicScenario {
    pub id: &'static str,
    pub kind: ScenarioKind,
    pub title: &'static str,
    pub phase: &'static str,
    pub user_task: &'static str,
    pub status: ReplayStatus,
    pub external_baseline: ExternalBaselineStatus,
    pub evidence: &'static [ScenarioEvidence],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScenarioMatrixSummary {
    pub phase: &'static str,
    pub scenarios: usize,
    pub replay_ready: usize,
    pub required_evidence: usize,
    pub external_baseline_ready: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeSpineCaseKind {
    DirectAnswer,
    ReadOnlyProjectAudit,
    SmallCodeChange,
    BugFix,
    UserForbidsWrites,
    UserSpecifiedValidation,
    ToolFailure,
    NoDiffAuditCloseout,
    PermissionRequired,
    TestFailureRepair,
    RouteMistakeRecovery,
    SubagentVerifier,
    IsolatedWorktreeImplementer,
    MemoryRetrievalConflict,
    SkillGuidance,
}

impl RuntimeSpineCaseKind {
    pub const fn id(self) -> &'static str {
        match self {
            Self::DirectAnswer => "runtime_spine_direct_answer",
            Self::ReadOnlyProjectAudit => "runtime_spine_read_only_project_audit",
            Self::SmallCodeChange => "runtime_spine_small_code_change",
            Self::BugFix => "runtime_spine_bug_fix",
            Self::UserForbidsWrites => "runtime_spine_user_forbids_writes",
            Self::UserSpecifiedValidation => "runtime_spine_user_specified_validation",
            Self::ToolFailure => "runtime_spine_tool_failure",
            Self::NoDiffAuditCloseout => "runtime_spine_no_diff_audit_closeout",
            Self::PermissionRequired => "runtime_spine_permission_required",
            Self::TestFailureRepair => "runtime_spine_test_failure_repair",
            Self::RouteMistakeRecovery => "runtime_spine_route_mistake_recovery",
            Self::SubagentVerifier => "runtime_spine_subagent_verifier",
            Self::IsolatedWorktreeImplementer => "runtime_spine_isolated_worktree_implementer",
            Self::MemoryRetrievalConflict => "runtime_spine_memory_retrieval_conflict",
            Self::SkillGuidance => "runtime_spine_skill_guidance",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeSpinePhase {
    P0aCore,
    P0bExtended,
}

impl RuntimeSpinePhase {
    pub const fn label(self) -> &'static str {
        match self {
            Self::P0aCore => "P0a core runtime spine",
            Self::P0bExtended => "P0b extended runtime spine",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeSpineTaskType {
    DirectAnswer,
    ReadOnlyAudit,
    CodeChange,
    BugFix,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpectedChangedFiles {
    None,
    NonEmpty,
    Either,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CloseoutStatusExpectation {
    Verified,
    NotApplicable,
    Partial,
    Failed,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeSpineGateOutcomeClass {
    ProtectiveBlock,
    RecoverableFriction,
    UnrecoveredBlock,
    SuspectedFalsePositive,
    PolicyCorrectButUxCostly,
    HarmlessPass,
}

impl RuntimeSpineGateOutcomeClass {
    pub const fn label(self) -> &'static str {
        match self {
            Self::ProtectiveBlock => "protective_block",
            Self::RecoverableFriction => "recoverable_friction",
            Self::UnrecoveredBlock => "unrecovered_block",
            Self::SuspectedFalsePositive => "suspected_false_positive",
            Self::PolicyCorrectButUxCostly => "policy_correct_but_ux_costly",
            Self::HarmlessPass => "harmless_pass",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureOwner {
    None,
    IntentRouter,
    ToolExposure,
    ActionReview,
    Permission,
    ModelPlanning,
    ToolRuntime,
    ValidationCommand,
    EvidenceLedger,
    Closeout,
    ContextAssembly,
    Subagent,
    UserBlocked,
    ExternalEnvironment,
    Harness,
}

impl FailureOwner {
    pub const fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::IntentRouter => "intent_router",
            Self::ToolExposure => "tool_exposure",
            Self::ActionReview => "action_review",
            Self::Permission => "permission",
            Self::ModelPlanning => "model_planning",
            Self::ToolRuntime => "tool_runtime",
            Self::ValidationCommand => "validation_command",
            Self::EvidenceLedger => "evidence_ledger",
            Self::Closeout => "closeout",
            Self::ContextAssembly => "context_assembly",
            Self::Subagent => "subagent",
            Self::UserBlocked => "user_blocked",
            Self::ExternalEnvironment => "external_environment",
            Self::Harness => "harness",
        }
    }

    pub fn from_label(label: &str) -> Option<Self> {
        match label.trim().to_ascii_lowercase().as_str() {
            "none" => Some(Self::None),
            "intent_router" => Some(Self::IntentRouter),
            "tool_exposure" => Some(Self::ToolExposure),
            "action_review" => Some(Self::ActionReview),
            "permission" => Some(Self::Permission),
            "model_planning" | "llm_reasoning" => Some(Self::ModelPlanning),
            "tool_runtime" => Some(Self::ToolRuntime),
            "validation_command" => Some(Self::ValidationCommand),
            "evidence_ledger" => Some(Self::EvidenceLedger),
            "closeout" => Some(Self::Closeout),
            "context_assembly" => Some(Self::ContextAssembly),
            "subagent" => Some(Self::Subagent),
            "user_blocked" => Some(Self::UserBlocked),
            "external_environment" => Some(Self::ExternalEnvironment),
            "harness" | "eval_harness" => Some(Self::Harness),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GoldenTraceSurface {
    RouteDecision,
    ToolExposurePlan,
    ActionReviewDecision,
    EvidenceLedgerSummary,
    VerificationProof,
    CompletionContract,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RuntimeSpineRouteExpectation {
    pub one_of: &'static [&'static str],
    pub min_confidence: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RuntimeSpineToolExpectation {
    pub must_expose: &'static [&'static str],
    pub may_expose: &'static [&'static str],
    pub must_not_expose: &'static [&'static str],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RuntimeSpineMutationExpectation {
    pub expected_changed_files: ExpectedChangedFiles,
    pub outside_workspace: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RuntimeSpineValidationExpectation {
    pub required: bool,
    pub accepted_families: &'static [&'static str],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RuntimeSpineCloseoutExpectation {
    pub allowed_status: &'static [CloseoutStatusExpectation],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RuntimeSpineFinalAnswerExpectation {
    pub must_mention: &'static [&'static str],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RuntimeSpineExpected {
    pub route: RuntimeSpineRouteExpectation,
    pub tools: RuntimeSpineToolExpectation,
    pub mutation: RuntimeSpineMutationExpectation,
    pub validation: RuntimeSpineValidationExpectation,
    pub closeout: RuntimeSpineCloseoutExpectation,
    pub final_answer: RuntimeSpineFinalAnswerExpectation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RuntimeSpineFrictionBudget {
    pub max_tool_rounds: usize,
    pub max_repeated_denied_attempts: usize,
    pub max_no_progress_rounds: usize,
    pub max_unnecessary_reads: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RuntimeSpineCaseSpec {
    pub id: &'static str,
    pub kind: RuntimeSpineCaseKind,
    pub phase: RuntimeSpinePhase,
    pub title: &'static str,
    pub task_type: RuntimeSpineTaskType,
    pub initial_prompt: &'static str,
    pub expected: RuntimeSpineExpected,
    pub failure_owner_if_failed: &'static [FailureOwner],
    pub expected_gate_outcomes: &'static [RuntimeSpineGateOutcomeClass],
    pub friction_budget: RuntimeSpineFrictionBudget,
    pub golden_trace: &'static [GoldenTraceSurface],
}

pub const REQUIRED_PHASE_12_KINDS: &[ScenarioKind] = &[
    ScenarioKind::FileEditRewind,
    ScenarioKind::BashBackgroundTask,
    ScenarioKind::PermissionDenialRetry,
    ScenarioKind::CompactionBoundary,
    ScenarioKind::SubagentWorktreeWorker,
    ScenarioKind::McpAuthRepair,
];

pub const REQUIRED_RUNTIME_SPINE_P0A_KINDS: &[RuntimeSpineCaseKind] = &[
    RuntimeSpineCaseKind::DirectAnswer,
    RuntimeSpineCaseKind::ReadOnlyProjectAudit,
    RuntimeSpineCaseKind::SmallCodeChange,
    RuntimeSpineCaseKind::BugFix,
    RuntimeSpineCaseKind::UserForbidsWrites,
    RuntimeSpineCaseKind::UserSpecifiedValidation,
    RuntimeSpineCaseKind::ToolFailure,
    RuntimeSpineCaseKind::NoDiffAuditCloseout,
];

pub const REQUIRED_RUNTIME_SPINE_P0B_KINDS: &[RuntimeSpineCaseKind] = &[
    RuntimeSpineCaseKind::PermissionRequired,
    RuntimeSpineCaseKind::TestFailureRepair,
    RuntimeSpineCaseKind::RouteMistakeRecovery,
    RuntimeSpineCaseKind::SubagentVerifier,
    RuntimeSpineCaseKind::IsolatedWorktreeImplementer,
    RuntimeSpineCaseKind::MemoryRetrievalConflict,
    RuntimeSpineCaseKind::SkillGuidance,
];

const CORE_GOLDEN_TRACE: &[GoldenTraceSurface] = &[
    GoldenTraceSurface::RouteDecision,
    GoldenTraceSurface::ToolExposurePlan,
    GoldenTraceSurface::ActionReviewDecision,
    GoldenTraceSurface::EvidenceLedgerSummary,
    GoldenTraceSurface::VerificationProof,
    GoldenTraceSurface::CompletionContract,
];

const LOW_FRICTION: RuntimeSpineFrictionBudget = RuntimeSpineFrictionBudget {
    max_tool_rounds: 2,
    max_repeated_denied_attempts: 0,
    max_no_progress_rounds: 0,
    max_unnecessary_reads: 1,
};

const CODE_CHANGE_FRICTION: RuntimeSpineFrictionBudget = RuntimeSpineFrictionBudget {
    max_tool_rounds: 5,
    max_repeated_denied_attempts: 1,
    max_no_progress_rounds: 1,
    max_unnecessary_reads: 2,
};

const EXTENDED_CONTROL_FRICTION: RuntimeSpineFrictionBudget = RuntimeSpineFrictionBudget {
    max_tool_rounds: 6,
    max_repeated_denied_attempts: 1,
    max_no_progress_rounds: 2,
    max_unnecessary_reads: 3,
};

const FILE_EDIT_REWIND_EVIDENCE: &[ScenarioEvidence] = &[
    ScenarioEvidence::required(
        EvidenceSurface::TraceEvent,
        "tool.start:file_edit",
        "the edit must be visible in the turn trace before rewind evidence is accepted",
    ),
    ScenarioEvidence::required(
        EvidenceSurface::ToolMetadata,
        "file_checkpoint",
        "rewind must refer to a concrete checkpoint or file snapshot, not a vague undo claim",
    ),
    ScenarioEvidence::required(
        EvidenceSurface::SlashCommand,
        "/rewind",
        "the user-facing recovery path must be discoverable from the CLI",
    ),
];

const BASH_BACKGROUND_EVIDENCE: &[ScenarioEvidence] = &[
    ScenarioEvidence::required(
        EvidenceSurface::ToolMetadata,
        "shell_result.backgrounded",
        "background execution must return a handle instead of blocking the turn",
    ),
    ScenarioEvidence::required(
        EvidenceSurface::RuntimePanel,
        "/panel tasks",
        "the long-running task must be inspectable while it is still active",
    ),
    ScenarioEvidence::required(
        EvidenceSurface::ToolMetadata,
        "bash_output",
        "bounded output reads must be available after the process is spawned",
    ),
];

const PERMISSION_DENIAL_EVIDENCE: &[ScenarioEvidence] = &[
    ScenarioEvidence::required(
        EvidenceSurface::TraceEvent,
        "permission.request",
        "approval prompts must be explicit in the trace",
    ),
    ScenarioEvidence::required(
        EvidenceSurface::TraceEvent,
        "permission.resolve",
        "the deny/approve decision must be distinguishable from a tool failure",
    ),
    ScenarioEvidence::required(
        EvidenceSurface::RecoveryPlan,
        "/permissions explain",
        "permission denial must produce the same recovery spine as other blocked runtime work",
    ),
];

const COMPACTION_BOUNDARY_EVIDENCE: &[ScenarioEvidence] = &[
    ScenarioEvidence::required(
        EvidenceSurface::TraceEvent,
        "context.compact",
        "compaction must record before/after size and boundary identity",
    ),
    ScenarioEvidence::required(
        EvidenceSurface::TraceEvent,
        "runtime.diet",
        "request-budget evidence must explain why compaction happened",
    ),
    ScenarioEvidence::required(
        EvidenceSurface::SessionStore,
        "boundary_id",
        "the persisted session must retain enough provenance to resume safely",
    ),
];

const SUBAGENT_WORKTREE_EVIDENCE: &[ScenarioEvidence] = &[
    ScenarioEvidence::required(
        EvidenceSurface::RuntimePanel,
        "/agents worktree review",
        "isolated worker output must have a review path before merge",
    ),
    ScenarioEvidence::required(
        EvidenceSurface::ToolMetadata,
        "isolated_worktree",
        "the forked execution path must identify the child worktree and branch",
    ),
    ScenarioEvidence::required(
        EvidenceSurface::SessionStore,
        "recursive_fork_guard",
        "child agents must carry a guard that prevents runaway recursive forking",
    ),
];

const MCP_AUTH_REPAIR_EVIDENCE: &[ScenarioEvidence] = &[
    ScenarioEvidence::required(
        EvidenceSurface::TraceEvent,
        "mcp.resource",
        "MCP access must be visible as a distinct external-resource event",
    ),
    ScenarioEvidence::required(
        EvidenceSurface::RuntimePanel,
        "/panel mcp",
        "auth and server health must be inspectable without rerunning the task",
    ),
    ScenarioEvidence::required(
        EvidenceSurface::RecoveryPlan,
        "/mcp approve",
        "auth repair must route to an explicit user-facing permission action",
    ),
];

const SCENARIOS: &[DeterministicScenario] = &[
    DeterministicScenario {
        id: "file_edit_rewind",
        kind: ScenarioKind::FileEditRewind,
        title: "File edit with rewind",
        phase: "Phase 12",
        user_task: "edit a file, verify checkpoint evidence, then rewind the edit",
        status: ReplayStatus::ReplayFixtureReady,
        external_baseline: ExternalBaselineStatus::Ready,
        evidence: FILE_EDIT_REWIND_EVIDENCE,
    },
    DeterministicScenario {
        id: "bash_background_task",
        kind: ScenarioKind::BashBackgroundTask,
        title: "Bash background task",
        phase: "Phase 12",
        user_task: "start a long-running shell command, poll output, then cancel or close out",
        status: ReplayStatus::ReplayFixtureReady,
        external_baseline: ExternalBaselineStatus::Ready,
        evidence: BASH_BACKGROUND_EVIDENCE,
    },
    DeterministicScenario {
        id: "permission_denial_retry",
        kind: ScenarioKind::PermissionDenialRetry,
        title: "Permission denial and retry",
        phase: "Phase 12",
        user_task: "deny a risky tool call, explain recovery, then retry through an allowed path",
        status: ReplayStatus::ReplayFixtureReady,
        external_baseline: ExternalBaselineStatus::Ready,
        evidence: PERMISSION_DENIAL_EVIDENCE,
    },
    DeterministicScenario {
        id: "compaction_boundary",
        kind: ScenarioKind::CompactionBoundary,
        title: "Compaction boundary",
        phase: "Phase 12",
        user_task: "force context pressure, compact with provenance, then resume the task",
        status: ReplayStatus::ReplayFixtureReady,
        external_baseline: ExternalBaselineStatus::Ready,
        evidence: COMPACTION_BOUNDARY_EVIDENCE,
    },
    DeterministicScenario {
        id: "subagent_worktree_worker",
        kind: ScenarioKind::SubagentWorktreeWorker,
        title: "Subagent isolated worktree worker",
        phase: "Phase 12",
        user_task: "fork a child worker into an isolated worktree, review output, and clean up",
        status: ReplayStatus::ReplayFixtureReady,
        external_baseline: ExternalBaselineStatus::Ready,
        evidence: SUBAGENT_WORKTREE_EVIDENCE,
    },
    DeterministicScenario {
        id: "mcp_auth_repair",
        kind: ScenarioKind::McpAuthRepair,
        title: "MCP auth repair",
        phase: "Phase 12",
        user_task: "hit an MCP auth/server failure, surface repair, then retry after approval",
        status: ReplayStatus::ReplayFixtureReady,
        external_baseline: ExternalBaselineStatus::Ready,
        evidence: MCP_AUTH_REPAIR_EVIDENCE,
    },
];

const RUNTIME_SPINE_P0A_CASES: &[RuntimeSpineCaseSpec] = &[
    RuntimeSpineCaseSpec {
        id: RuntimeSpineCaseKind::DirectAnswer.id(),
        kind: RuntimeSpineCaseKind::DirectAnswer,
        phase: RuntimeSpinePhase::P0aCore,
        title: "Direct answer stays lightweight",
        task_type: RuntimeSpineTaskType::DirectAnswer,
        initial_prompt: "解释这个错误信息是什么意思，不要修改文件",
        expected: RuntimeSpineExpected {
            route: RuntimeSpineRouteExpectation {
                one_of: &["DirectAnswer"],
                min_confidence: "medium",
            },
            tools: RuntimeSpineToolExpectation {
                must_expose: &[],
                may_expose: &["file_read"],
                must_not_expose: &["file_edit", "file_patch", "install_dependencies"],
            },
            mutation: RuntimeSpineMutationExpectation {
                expected_changed_files: ExpectedChangedFiles::None,
                outside_workspace: false,
            },
            validation: RuntimeSpineValidationExpectation {
                required: false,
                accepted_families: &[],
            },
            closeout: RuntimeSpineCloseoutExpectation {
                allowed_status: &[CloseoutStatusExpectation::NotApplicable],
            },
            final_answer: RuntimeSpineFinalAnswerExpectation {
                must_mention: &["status"],
            },
        },
        failure_owner_if_failed: &[
            FailureOwner::IntentRouter,
            FailureOwner::ToolExposure,
            FailureOwner::ContextAssembly,
            FailureOwner::ModelPlanning,
        ],
        expected_gate_outcomes: &[RuntimeSpineGateOutcomeClass::HarmlessPass],
        friction_budget: LOW_FRICTION,
        golden_trace: CORE_GOLDEN_TRACE,
    },
    RuntimeSpineCaseSpec {
        id: RuntimeSpineCaseKind::ReadOnlyProjectAudit.id(),
        kind: RuntimeSpineCaseKind::ReadOnlyProjectAudit,
        phase: RuntimeSpinePhase::P0aCore,
        title: "Read-only audit proves current state without mutation",
        task_type: RuntimeSpineTaskType::ReadOnlyAudit,
        initial_prompt: "审查当前实现是否满足要求，不要改文件",
        expected: RuntimeSpineExpected {
            route: RuntimeSpineRouteExpectation {
                one_of: &["CodeChange", "Research", "DirectAnswer"],
                min_confidence: "medium",
            },
            tools: RuntimeSpineToolExpectation {
                must_expose: &["file_read", "grep"],
                may_expose: &["glob", "git_diff", "git_status"],
                must_not_expose: &["file_edit", "file_patch", "install_dependencies"],
            },
            mutation: RuntimeSpineMutationExpectation {
                expected_changed_files: ExpectedChangedFiles::None,
                outside_workspace: false,
            },
            validation: RuntimeSpineValidationExpectation {
                required: false,
                accepted_families: &["manual_inspection", "static_read"],
            },
            closeout: RuntimeSpineCloseoutExpectation {
                allowed_status: &[
                    CloseoutStatusExpectation::Verified,
                    CloseoutStatusExpectation::NotApplicable,
                ],
            },
            final_answer: RuntimeSpineFinalAnswerExpectation {
                must_mention: &["evidence", "no_diff"],
            },
        },
        failure_owner_if_failed: &[
            FailureOwner::IntentRouter,
            FailureOwner::ToolExposure,
            FailureOwner::EvidenceLedger,
            FailureOwner::Closeout,
        ],
        expected_gate_outcomes: &[RuntimeSpineGateOutcomeClass::HarmlessPass],
        friction_budget: RuntimeSpineFrictionBudget {
            max_tool_rounds: 3,
            max_repeated_denied_attempts: 0,
            max_no_progress_rounds: 1,
            max_unnecessary_reads: 2,
        },
        golden_trace: CORE_GOLDEN_TRACE,
    },
    RuntimeSpineCaseSpec {
        id: RuntimeSpineCaseKind::SmallCodeChange.id(),
        kind: RuntimeSpineCaseKind::SmallCodeChange,
        phase: RuntimeSpinePhase::P0aCore,
        title: "Small code change reads, edits, validates, and closes out",
        task_type: RuntimeSpineTaskType::CodeChange,
        initial_prompt: "做一个小的代码修改，并运行指定测试",
        expected: RuntimeSpineExpected {
            route: RuntimeSpineRouteExpectation {
                one_of: &["CodeChange", "BugFix"],
                min_confidence: "medium",
            },
            tools: RuntimeSpineToolExpectation {
                must_expose: &["file_read", "grep"],
                may_expose: &["file_edit", "file_patch", "run_tests", "bash", "git_diff"],
                must_not_expose: &["install_dependencies"],
            },
            mutation: RuntimeSpineMutationExpectation {
                expected_changed_files: ExpectedChangedFiles::NonEmpty,
                outside_workspace: false,
            },
            validation: RuntimeSpineValidationExpectation {
                required: true,
                accepted_families: &["unit_test", "targeted_check", "format"],
            },
            closeout: RuntimeSpineCloseoutExpectation {
                allowed_status: &[CloseoutStatusExpectation::Verified],
            },
            final_answer: RuntimeSpineFinalAnswerExpectation {
                must_mention: &["changed_files", "validation"],
            },
        },
        failure_owner_if_failed: &[
            FailureOwner::IntentRouter,
            FailureOwner::ToolExposure,
            FailureOwner::ActionReview,
            FailureOwner::ModelPlanning,
            FailureOwner::EvidenceLedger,
            FailureOwner::Closeout,
        ],
        expected_gate_outcomes: &[
            RuntimeSpineGateOutcomeClass::HarmlessPass,
            RuntimeSpineGateOutcomeClass::RecoverableFriction,
        ],
        friction_budget: CODE_CHANGE_FRICTION,
        golden_trace: CORE_GOLDEN_TRACE,
    },
    RuntimeSpineCaseSpec {
        id: RuntimeSpineCaseKind::BugFix.id(),
        kind: RuntimeSpineCaseKind::BugFix,
        phase: RuntimeSpinePhase::P0aCore,
        title: "Bug fix inspects failure, patches, validates, and repairs if needed",
        task_type: RuntimeSpineTaskType::BugFix,
        initial_prompt: "修复这个 bug，并跑相关测试",
        expected: RuntimeSpineExpected {
            route: RuntimeSpineRouteExpectation {
                one_of: &["BugFix", "CodeChange"],
                min_confidence: "medium",
            },
            tools: RuntimeSpineToolExpectation {
                must_expose: &["file_read", "grep"],
                may_expose: &[
                    "symbol_query",
                    "file_edit",
                    "file_patch",
                    "run_tests",
                    "bash",
                    "git_diff",
                ],
                must_not_expose: &["install_dependencies"],
            },
            mutation: RuntimeSpineMutationExpectation {
                expected_changed_files: ExpectedChangedFiles::NonEmpty,
                outside_workspace: false,
            },
            validation: RuntimeSpineValidationExpectation {
                required: true,
                accepted_families: &["unit_test", "targeted_check", "regression_test"],
            },
            closeout: RuntimeSpineCloseoutExpectation {
                allowed_status: &[CloseoutStatusExpectation::Verified],
            },
            final_answer: RuntimeSpineFinalAnswerExpectation {
                must_mention: &["root_cause", "validation"],
            },
        },
        failure_owner_if_failed: &[
            FailureOwner::IntentRouter,
            FailureOwner::ToolExposure,
            FailureOwner::ActionReview,
            FailureOwner::ValidationCommand,
            FailureOwner::ModelPlanning,
            FailureOwner::Closeout,
        ],
        expected_gate_outcomes: &[
            RuntimeSpineGateOutcomeClass::HarmlessPass,
            RuntimeSpineGateOutcomeClass::RecoverableFriction,
        ],
        friction_budget: CODE_CHANGE_FRICTION,
        golden_trace: CORE_GOLDEN_TRACE,
    },
    RuntimeSpineCaseSpec {
        id: RuntimeSpineCaseKind::UserForbidsWrites.id(),
        kind: RuntimeSpineCaseKind::UserForbidsWrites,
        phase: RuntimeSpinePhase::P0aCore,
        title: "User-forbidden writes stay read-only",
        task_type: RuntimeSpineTaskType::ReadOnlyAudit,
        initial_prompt: "只分析，不要修改任何文件",
        expected: RuntimeSpineExpected {
            route: RuntimeSpineRouteExpectation {
                one_of: &["DirectAnswer", "Research", "CodeChange"],
                min_confidence: "medium",
            },
            tools: RuntimeSpineToolExpectation {
                must_expose: &["file_read"],
                may_expose: &["grep", "glob", "git_diff"],
                must_not_expose: &["file_edit", "file_patch", "file_write", "format"],
            },
            mutation: RuntimeSpineMutationExpectation {
                expected_changed_files: ExpectedChangedFiles::None,
                outside_workspace: false,
            },
            validation: RuntimeSpineValidationExpectation {
                required: false,
                accepted_families: &["manual_inspection"],
            },
            closeout: RuntimeSpineCloseoutExpectation {
                allowed_status: &[
                    CloseoutStatusExpectation::Verified,
                    CloseoutStatusExpectation::NotApplicable,
                    CloseoutStatusExpectation::Partial,
                ],
            },
            final_answer: RuntimeSpineFinalAnswerExpectation {
                must_mention: &["read_only", "not_modified"],
            },
        },
        failure_owner_if_failed: &[
            FailureOwner::ToolExposure,
            FailureOwner::ActionReview,
            FailureOwner::Closeout,
            FailureOwner::ModelPlanning,
        ],
        expected_gate_outcomes: &[
            RuntimeSpineGateOutcomeClass::ProtectiveBlock,
            RuntimeSpineGateOutcomeClass::RecoverableFriction,
        ],
        friction_budget: RuntimeSpineFrictionBudget {
            max_tool_rounds: 3,
            max_repeated_denied_attempts: 1,
            max_no_progress_rounds: 1,
            max_unnecessary_reads: 2,
        },
        golden_trace: CORE_GOLDEN_TRACE,
    },
    RuntimeSpineCaseSpec {
        id: RuntimeSpineCaseKind::UserSpecifiedValidation.id(),
        kind: RuntimeSpineCaseKind::UserSpecifiedValidation,
        phase: RuntimeSpinePhase::P0aCore,
        title: "User-specified validation is recognized and proven",
        task_type: RuntimeSpineTaskType::CodeChange,
        initial_prompt: "修改代码后必须运行 cargo test -q route_scoped_tools",
        expected: RuntimeSpineExpected {
            route: RuntimeSpineRouteExpectation {
                one_of: &["CodeChange", "BugFix"],
                min_confidence: "medium",
            },
            tools: RuntimeSpineToolExpectation {
                must_expose: &["file_read", "grep"],
                may_expose: &["file_edit", "file_patch", "run_tests", "bash"],
                must_not_expose: &["install_dependencies"],
            },
            mutation: RuntimeSpineMutationExpectation {
                expected_changed_files: ExpectedChangedFiles::Either,
                outside_workspace: false,
            },
            validation: RuntimeSpineValidationExpectation {
                required: true,
                accepted_families: &["required_command"],
            },
            closeout: RuntimeSpineCloseoutExpectation {
                allowed_status: &[CloseoutStatusExpectation::Verified],
            },
            final_answer: RuntimeSpineFinalAnswerExpectation {
                must_mention: &["required_validation", "cargo test -q route_scoped_tools"],
            },
        },
        failure_owner_if_failed: &[
            FailureOwner::ValidationCommand,
            FailureOwner::EvidenceLedger,
            FailureOwner::Closeout,
            FailureOwner::ModelPlanning,
        ],
        expected_gate_outcomes: &[RuntimeSpineGateOutcomeClass::HarmlessPass],
        friction_budget: CODE_CHANGE_FRICTION,
        golden_trace: CORE_GOLDEN_TRACE,
    },
    RuntimeSpineCaseSpec {
        id: RuntimeSpineCaseKind::ToolFailure.id(),
        kind: RuntimeSpineCaseKind::ToolFailure,
        phase: RuntimeSpinePhase::P0aCore,
        title: "Tool failure returns observation and bounded recovery",
        task_type: RuntimeSpineTaskType::BugFix,
        initial_prompt: "修复失败命令暴露的问题，并根据工具失败结果恢复",
        expected: RuntimeSpineExpected {
            route: RuntimeSpineRouteExpectation {
                one_of: &["BugFix", "CodeChange"],
                min_confidence: "medium",
            },
            tools: RuntimeSpineToolExpectation {
                must_expose: &["file_read", "grep"],
                may_expose: &["bash", "run_tests", "file_edit", "file_patch"],
                must_not_expose: &["install_dependencies"],
            },
            mutation: RuntimeSpineMutationExpectation {
                expected_changed_files: ExpectedChangedFiles::Either,
                outside_workspace: false,
            },
            validation: RuntimeSpineValidationExpectation {
                required: true,
                accepted_families: &["tool_failure_recovery", "targeted_check"],
            },
            closeout: RuntimeSpineCloseoutExpectation {
                allowed_status: &[
                    CloseoutStatusExpectation::Verified,
                    CloseoutStatusExpectation::Partial,
                    CloseoutStatusExpectation::Failed,
                ],
            },
            final_answer: RuntimeSpineFinalAnswerExpectation {
                must_mention: &["tool_failure", "recovery"],
            },
        },
        failure_owner_if_failed: &[
            FailureOwner::ToolRuntime,
            FailureOwner::ModelPlanning,
            FailureOwner::EvidenceLedger,
            FailureOwner::Closeout,
        ],
        expected_gate_outcomes: &[
            RuntimeSpineGateOutcomeClass::RecoverableFriction,
            RuntimeSpineGateOutcomeClass::UnrecoveredBlock,
        ],
        friction_budget: RuntimeSpineFrictionBudget {
            max_tool_rounds: 5,
            max_repeated_denied_attempts: 1,
            max_no_progress_rounds: 2,
            max_unnecessary_reads: 2,
        },
        golden_trace: CORE_GOLDEN_TRACE,
    },
    RuntimeSpineCaseSpec {
        id: RuntimeSpineCaseKind::NoDiffAuditCloseout.id(),
        kind: RuntimeSpineCaseKind::NoDiffAuditCloseout,
        phase: RuntimeSpinePhase::P0aCore,
        title: "No-diff audit closes with evidence instead of missed-edit failure",
        task_type: RuntimeSpineTaskType::ReadOnlyAudit,
        initial_prompt: "审查当前行为是否已经正确，如果正确不要改文件",
        expected: RuntimeSpineExpected {
            route: RuntimeSpineRouteExpectation {
                one_of: &["CodeChange", "Research", "DirectAnswer"],
                min_confidence: "medium",
            },
            tools: RuntimeSpineToolExpectation {
                must_expose: &["file_read", "grep"],
                may_expose: &["bash", "run_tests", "git_diff", "git_status"],
                must_not_expose: &["file_edit", "file_patch"],
            },
            mutation: RuntimeSpineMutationExpectation {
                expected_changed_files: ExpectedChangedFiles::None,
                outside_workspace: false,
            },
            validation: RuntimeSpineValidationExpectation {
                required: false,
                accepted_families: &["no_diff_audit", "manual_inspection", "targeted_check"],
            },
            closeout: RuntimeSpineCloseoutExpectation {
                allowed_status: &[
                    CloseoutStatusExpectation::Verified,
                    CloseoutStatusExpectation::NotApplicable,
                ],
            },
            final_answer: RuntimeSpineFinalAnswerExpectation {
                must_mention: &["no_diff", "evidence"],
            },
        },
        failure_owner_if_failed: &[
            FailureOwner::IntentRouter,
            FailureOwner::EvidenceLedger,
            FailureOwner::Closeout,
            FailureOwner::ModelPlanning,
        ],
        expected_gate_outcomes: &[RuntimeSpineGateOutcomeClass::HarmlessPass],
        friction_budget: RuntimeSpineFrictionBudget {
            max_tool_rounds: 4,
            max_repeated_denied_attempts: 0,
            max_no_progress_rounds: 1,
            max_unnecessary_reads: 2,
        },
        golden_trace: CORE_GOLDEN_TRACE,
    },
];

const RUNTIME_SPINE_P0B_CASES: &[RuntimeSpineCaseSpec] = &[
    RuntimeSpineCaseSpec {
        id: RuntimeSpineCaseKind::PermissionRequired.id(),
        kind: RuntimeSpineCaseKind::PermissionRequired,
        phase: RuntimeSpinePhase::P0bExtended,
        title: "Permission-required action is explicit and recoverable",
        task_type: RuntimeSpineTaskType::CodeChange,
        initial_prompt: "完成需要权限确认的发布准备动作，用户拒绝时要诚实停下",
        expected: RuntimeSpineExpected {
            route: RuntimeSpineRouteExpectation {
                one_of: &["CodeChange", "BugFix"],
                min_confidence: "medium",
            },
            tools: RuntimeSpineToolExpectation {
                must_expose: &["file_read", "grep"],
                may_expose: &["bash", "git", "file_edit", "file_patch"],
                must_not_expose: &["install_dependencies"],
            },
            mutation: RuntimeSpineMutationExpectation {
                expected_changed_files: ExpectedChangedFiles::Either,
                outside_workspace: false,
            },
            validation: RuntimeSpineValidationExpectation {
                required: true,
                accepted_families: &["permission_resolution", "targeted_check"],
            },
            closeout: RuntimeSpineCloseoutExpectation {
                allowed_status: &[
                    CloseoutStatusExpectation::Verified,
                    CloseoutStatusExpectation::Partial,
                    CloseoutStatusExpectation::Blocked,
                ],
            },
            final_answer: RuntimeSpineFinalAnswerExpectation {
                must_mention: &["permission", "recovery"],
            },
        },
        failure_owner_if_failed: &[
            FailureOwner::Permission,
            FailureOwner::ActionReview,
            FailureOwner::ToolExposure,
            FailureOwner::Closeout,
        ],
        expected_gate_outcomes: &[
            RuntimeSpineGateOutcomeClass::ProtectiveBlock,
            RuntimeSpineGateOutcomeClass::RecoverableFriction,
            RuntimeSpineGateOutcomeClass::UnrecoveredBlock,
        ],
        friction_budget: EXTENDED_CONTROL_FRICTION,
        golden_trace: CORE_GOLDEN_TRACE,
    },
    RuntimeSpineCaseSpec {
        id: RuntimeSpineCaseKind::TestFailureRepair.id(),
        kind: RuntimeSpineCaseKind::TestFailureRepair,
        phase: RuntimeSpinePhase::P0bExtended,
        title: "Failed validation re-enters context and triggers bounded repair",
        task_type: RuntimeSpineTaskType::BugFix,
        initial_prompt: "修复测试失败；失败结果必须进入下一轮决策，不能 false green",
        expected: RuntimeSpineExpected {
            route: RuntimeSpineRouteExpectation {
                one_of: &["BugFix", "CodeChange"],
                min_confidence: "medium",
            },
            tools: RuntimeSpineToolExpectation {
                must_expose: &["file_read", "grep", "bash"],
                may_expose: &["run_tests", "file_edit", "file_patch", "git_diff"],
                must_not_expose: &["install_dependencies"],
            },
            mutation: RuntimeSpineMutationExpectation {
                expected_changed_files: ExpectedChangedFiles::NonEmpty,
                outside_workspace: false,
            },
            validation: RuntimeSpineValidationExpectation {
                required: true,
                accepted_families: &["failed_validation_repair", "regression_test"],
            },
            closeout: RuntimeSpineCloseoutExpectation {
                allowed_status: &[
                    CloseoutStatusExpectation::Verified,
                    CloseoutStatusExpectation::Partial,
                    CloseoutStatusExpectation::Failed,
                ],
            },
            final_answer: RuntimeSpineFinalAnswerExpectation {
                must_mention: &["failed_validation", "repair", "rerun"],
            },
        },
        failure_owner_if_failed: &[
            FailureOwner::ValidationCommand,
            FailureOwner::ModelPlanning,
            FailureOwner::ToolRuntime,
            FailureOwner::Closeout,
        ],
        expected_gate_outcomes: &[
            RuntimeSpineGateOutcomeClass::RecoverableFriction,
            RuntimeSpineGateOutcomeClass::UnrecoveredBlock,
        ],
        friction_budget: EXTENDED_CONTROL_FRICTION,
        golden_trace: CORE_GOLDEN_TRACE,
    },
    RuntimeSpineCaseSpec {
        id: RuntimeSpineCaseKind::RouteMistakeRecovery.id(),
        kind: RuntimeSpineCaseKind::RouteMistakeRecovery,
        phase: RuntimeSpinePhase::P0bExtended,
        title: "Route drift upgrades understanding without silent mutation expansion",
        task_type: RuntimeSpineTaskType::CodeChange,
        initial_prompt: "先看起来像问答，但随后需要读项目并修一个小问题",
        expected: RuntimeSpineExpected {
            route: RuntimeSpineRouteExpectation {
                one_of: &["DirectAnswer", "Research", "CodeChange", "BugFix"],
                min_confidence: "low",
            },
            tools: RuntimeSpineToolExpectation {
                must_expose: &["file_read", "grep"],
                may_expose: &["glob", "bash", "file_edit", "file_patch"],
                must_not_expose: &["install_dependencies"],
            },
            mutation: RuntimeSpineMutationExpectation {
                expected_changed_files: ExpectedChangedFiles::Either,
                outside_workspace: false,
            },
            validation: RuntimeSpineValidationExpectation {
                required: true,
                accepted_families: &["route_recovery", "targeted_check"],
            },
            closeout: RuntimeSpineCloseoutExpectation {
                allowed_status: &[
                    CloseoutStatusExpectation::Verified,
                    CloseoutStatusExpectation::Partial,
                ],
            },
            final_answer: RuntimeSpineFinalAnswerExpectation {
                must_mention: &["route_recovery", "validation"],
            },
        },
        failure_owner_if_failed: &[
            FailureOwner::IntentRouter,
            FailureOwner::ToolExposure,
            FailureOwner::ActionReview,
            FailureOwner::ModelPlanning,
        ],
        expected_gate_outcomes: &[
            RuntimeSpineGateOutcomeClass::RecoverableFriction,
            RuntimeSpineGateOutcomeClass::PolicyCorrectButUxCostly,
        ],
        friction_budget: EXTENDED_CONTROL_FRICTION,
        golden_trace: CORE_GOLDEN_TRACE,
    },
    RuntimeSpineCaseSpec {
        id: RuntimeSpineCaseKind::SubagentVerifier.id(),
        kind: RuntimeSpineCaseKind::SubagentVerifier,
        phase: RuntimeSpinePhase::P0bExtended,
        title: "Subagent verifier claim requires parent runtime proof",
        task_type: RuntimeSpineTaskType::ReadOnlyAudit,
        initial_prompt: "让 verifier 子 agent 审查实现，但父 agent 必须复验证据再 closeout",
        expected: RuntimeSpineExpected {
            route: RuntimeSpineRouteExpectation {
                one_of: &["Research", "CodeChange", "DirectAnswer"],
                min_confidence: "medium",
            },
            tools: RuntimeSpineToolExpectation {
                must_expose: &["file_read", "grep"],
                may_expose: &["agent", "bash", "run_tests", "git_diff"],
                must_not_expose: &["file_edit", "file_patch", "file_write"],
            },
            mutation: RuntimeSpineMutationExpectation {
                expected_changed_files: ExpectedChangedFiles::None,
                outside_workspace: false,
            },
            validation: RuntimeSpineValidationExpectation {
                required: true,
                accepted_families: &["subagent_claim", "parent_runtime_verification"],
            },
            closeout: RuntimeSpineCloseoutExpectation {
                allowed_status: &[
                    CloseoutStatusExpectation::Verified,
                    CloseoutStatusExpectation::Partial,
                ],
            },
            final_answer: RuntimeSpineFinalAnswerExpectation {
                must_mention: &["subagent", "parent_verified", "proof"],
            },
        },
        failure_owner_if_failed: &[
            FailureOwner::Subagent,
            FailureOwner::EvidenceLedger,
            FailureOwner::Closeout,
            FailureOwner::ModelPlanning,
        ],
        expected_gate_outcomes: &[
            RuntimeSpineGateOutcomeClass::HarmlessPass,
            RuntimeSpineGateOutcomeClass::RecoverableFriction,
        ],
        friction_budget: EXTENDED_CONTROL_FRICTION,
        golden_trace: CORE_GOLDEN_TRACE,
    },
    RuntimeSpineCaseSpec {
        id: RuntimeSpineCaseKind::IsolatedWorktreeImplementer.id(),
        kind: RuntimeSpineCaseKind::IsolatedWorktreeImplementer,
        phase: RuntimeSpinePhase::P0bExtended,
        title: "Implementer subagent changes stay isolated until parent review",
        task_type: RuntimeSpineTaskType::CodeChange,
        initial_prompt: "委派 implementer 子 agent 在隔离 worktree 修改，再由父 agent 审查合并",
        expected: RuntimeSpineExpected {
            route: RuntimeSpineRouteExpectation {
                one_of: &["CodeChange", "BugFix"],
                min_confidence: "medium",
            },
            tools: RuntimeSpineToolExpectation {
                must_expose: &["agent", "file_read", "grep"],
                may_expose: &["worktree", "git_diff", "bash", "file_edit", "file_patch"],
                must_not_expose: &["install_dependencies"],
            },
            mutation: RuntimeSpineMutationExpectation {
                expected_changed_files: ExpectedChangedFiles::Either,
                outside_workspace: false,
            },
            validation: RuntimeSpineValidationExpectation {
                required: true,
                accepted_families: &["parent_reviewed_subagent_patch", "targeted_check"],
            },
            closeout: RuntimeSpineCloseoutExpectation {
                allowed_status: &[
                    CloseoutStatusExpectation::Verified,
                    CloseoutStatusExpectation::Partial,
                ],
            },
            final_answer: RuntimeSpineFinalAnswerExpectation {
                must_mention: &["isolated_worktree", "parent_review", "validation"],
            },
        },
        failure_owner_if_failed: &[
            FailureOwner::Subagent,
            FailureOwner::ToolRuntime,
            FailureOwner::EvidenceLedger,
            FailureOwner::Closeout,
        ],
        expected_gate_outcomes: &[
            RuntimeSpineGateOutcomeClass::ProtectiveBlock,
            RuntimeSpineGateOutcomeClass::RecoverableFriction,
        ],
        friction_budget: EXTENDED_CONTROL_FRICTION,
        golden_trace: CORE_GOLDEN_TRACE,
    },
    RuntimeSpineCaseSpec {
        id: RuntimeSpineCaseKind::MemoryRetrievalConflict.id(),
        kind: RuntimeSpineCaseKind::MemoryRetrievalConflict,
        phase: RuntimeSpinePhase::P0bExtended,
        title: "Conflicting memory is demoted below current workspace evidence",
        task_type: RuntimeSpineTaskType::ReadOnlyAudit,
        initial_prompt: "记忆里有旧说法，当前代码里可能不同；以当前 evidence 为准审查",
        expected: RuntimeSpineExpected {
            route: RuntimeSpineRouteExpectation {
                one_of: &["Research", "DirectAnswer", "CodeChange"],
                min_confidence: "medium",
            },
            tools: RuntimeSpineToolExpectation {
                must_expose: &["file_read", "grep"],
                may_expose: &["memory_load", "context", "glob"],
                must_not_expose: &["file_edit", "file_patch", "file_write"],
            },
            mutation: RuntimeSpineMutationExpectation {
                expected_changed_files: ExpectedChangedFiles::None,
                outside_workspace: false,
            },
            validation: RuntimeSpineValidationExpectation {
                required: false,
                accepted_families: &["current_workspace_evidence", "memory_conflict_review"],
            },
            closeout: RuntimeSpineCloseoutExpectation {
                allowed_status: &[
                    CloseoutStatusExpectation::Verified,
                    CloseoutStatusExpectation::Partial,
                    CloseoutStatusExpectation::NotApplicable,
                ],
            },
            final_answer: RuntimeSpineFinalAnswerExpectation {
                must_mention: &["memory_conflict", "current_evidence"],
            },
        },
        failure_owner_if_failed: &[
            FailureOwner::ContextAssembly,
            FailureOwner::EvidenceLedger,
            FailureOwner::ModelPlanning,
            FailureOwner::Closeout,
        ],
        expected_gate_outcomes: &[RuntimeSpineGateOutcomeClass::HarmlessPass],
        friction_budget: EXTENDED_CONTROL_FRICTION,
        golden_trace: CORE_GOLDEN_TRACE,
    },
    RuntimeSpineCaseSpec {
        id: RuntimeSpineCaseKind::SkillGuidance.id(),
        kind: RuntimeSpineCaseKind::SkillGuidance,
        phase: RuntimeSpinePhase::P0bExtended,
        title: "Skill guidance stays background and validation still owns closeout",
        task_type: RuntimeSpineTaskType::CodeChange,
        initial_prompt: "使用相关 skill 的建议完成小修改，但不要把 skill 当成更高优先级指令",
        expected: RuntimeSpineExpected {
            route: RuntimeSpineRouteExpectation {
                one_of: &["CodeChange", "BugFix", "Research"],
                min_confidence: "medium",
            },
            tools: RuntimeSpineToolExpectation {
                must_expose: &["file_read", "grep"],
                may_expose: &[
                    "file_edit",
                    "file_patch",
                    "bash",
                    "run_tests",
                    "skills_list",
                    "skill_view",
                ],
                must_not_expose: &["install_dependencies"],
            },
            mutation: RuntimeSpineMutationExpectation {
                expected_changed_files: ExpectedChangedFiles::Either,
                outside_workspace: false,
            },
            validation: RuntimeSpineValidationExpectation {
                required: true,
                accepted_families: &["targeted_check", "skill_guided_validation"],
            },
            closeout: RuntimeSpineCloseoutExpectation {
                allowed_status: &[
                    CloseoutStatusExpectation::Verified,
                    CloseoutStatusExpectation::Partial,
                ],
            },
            final_answer: RuntimeSpineFinalAnswerExpectation {
                must_mention: &["skill_guidance", "validation"],
            },
        },
        failure_owner_if_failed: &[
            FailureOwner::ContextAssembly,
            FailureOwner::ToolExposure,
            FailureOwner::EvidenceLedger,
            FailureOwner::Closeout,
        ],
        expected_gate_outcomes: &[
            RuntimeSpineGateOutcomeClass::HarmlessPass,
            RuntimeSpineGateOutcomeClass::RecoverableFriction,
        ],
        friction_budget: EXTENDED_CONTROL_FRICTION,
        golden_trace: CORE_GOLDEN_TRACE,
    },
];

pub fn deterministic_scenarios() -> &'static [DeterministicScenario] {
    SCENARIOS
}

pub fn runtime_spine_p0a_cases() -> &'static [RuntimeSpineCaseSpec] {
    RUNTIME_SPINE_P0A_CASES
}

pub fn runtime_spine_p0b_cases() -> &'static [RuntimeSpineCaseSpec] {
    RUNTIME_SPINE_P0B_CASES
}

pub fn scenario_matrix_summary() -> ScenarioMatrixSummary {
    let scenarios = deterministic_scenarios();
    ScenarioMatrixSummary {
        phase: "Phase 12",
        scenarios: scenarios.len(),
        replay_ready: scenarios
            .iter()
            .filter(|scenario| scenario.status == ReplayStatus::ReplayFixtureReady)
            .count(),
        required_evidence: scenarios
            .iter()
            .flat_map(|scenario| scenario.evidence)
            .filter(|evidence| evidence.required)
            .count(),
        external_baseline_ready: scenarios
            .iter()
            .all(|scenario| scenario.external_baseline == ExternalBaselineStatus::Ready),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuntimeSpineMatrixSummary {
    pub phase: &'static str,
    pub scenarios: usize,
    pub cases_with_validation: usize,
    pub cases_requiring_no_mutation: usize,
    pub golden_trace_surfaces: usize,
}

pub fn runtime_spine_p0a_summary() -> RuntimeSpineMatrixSummary {
    let cases = runtime_spine_p0a_cases();
    RuntimeSpineMatrixSummary {
        phase: RuntimeSpinePhase::P0aCore.label(),
        scenarios: cases.len(),
        cases_with_validation: cases
            .iter()
            .filter(|case| case.expected.validation.required)
            .count(),
        cases_requiring_no_mutation: cases
            .iter()
            .filter(|case| {
                case.expected.mutation.expected_changed_files == ExpectedChangedFiles::None
            })
            .count(),
        golden_trace_surfaces: CORE_GOLDEN_TRACE.len(),
    }
}

pub fn runtime_spine_p0b_summary() -> RuntimeSpineMatrixSummary {
    let cases = runtime_spine_p0b_cases();
    RuntimeSpineMatrixSummary {
        phase: RuntimeSpinePhase::P0bExtended.label(),
        scenarios: cases.len(),
        cases_with_validation: cases
            .iter()
            .filter(|case| case.expected.validation.required)
            .count(),
        cases_requiring_no_mutation: cases
            .iter()
            .filter(|case| {
                case.expected.mutation.expected_changed_files == ExpectedChangedFiles::None
            })
            .count(),
        golden_trace_surfaces: CORE_GOLDEN_TRACE.len(),
    }
}

pub fn runtime_spine_missing_p0a_kinds() -> Vec<RuntimeSpineCaseKind> {
    let covered = runtime_spine_p0a_cases()
        .iter()
        .map(|case| case.kind)
        .collect::<BTreeSet<_>>();
    REQUIRED_RUNTIME_SPINE_P0A_KINDS
        .iter()
        .copied()
        .filter(|kind| !covered.contains(kind))
        .collect()
}

pub fn runtime_spine_missing_p0b_kinds() -> Vec<RuntimeSpineCaseKind> {
    let covered = runtime_spine_p0b_cases()
        .iter()
        .map(|case| case.kind)
        .collect::<BTreeSet<_>>();
    REQUIRED_RUNTIME_SPINE_P0B_KINDS
        .iter()
        .copied()
        .filter(|kind| !covered.contains(kind))
        .collect()
}

mod report;
pub use report::*;

#[cfg(test)]
mod tests;
