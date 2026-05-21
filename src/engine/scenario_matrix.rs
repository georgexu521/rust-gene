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

pub const REQUIRED_PHASE_12_KINDS: &[ScenarioKind] = &[
    ScenarioKind::FileEditRewind,
    ScenarioKind::BashBackgroundTask,
    ScenarioKind::PermissionDenialRetry,
    ScenarioKind::CompactionBoundary,
    ScenarioKind::SubagentWorktreeWorker,
    ScenarioKind::McpAuthRepair,
];

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
        external_baseline: ExternalBaselineStatus::DeferredUntilReplayFixture,
        evidence: FILE_EDIT_REWIND_EVIDENCE,
    },
    DeterministicScenario {
        id: "bash_background_task",
        kind: ScenarioKind::BashBackgroundTask,
        title: "Bash background task",
        phase: "Phase 12",
        user_task: "start a long-running shell command, poll output, then cancel or close out",
        status: ReplayStatus::ReplayFixtureReady,
        external_baseline: ExternalBaselineStatus::DeferredUntilReplayFixture,
        evidence: BASH_BACKGROUND_EVIDENCE,
    },
    DeterministicScenario {
        id: "permission_denial_retry",
        kind: ScenarioKind::PermissionDenialRetry,
        title: "Permission denial and retry",
        phase: "Phase 12",
        user_task: "deny a risky tool call, explain recovery, then retry through an allowed path",
        status: ReplayStatus::ReplayFixtureReady,
        external_baseline: ExternalBaselineStatus::DeferredUntilReplayFixture,
        evidence: PERMISSION_DENIAL_EVIDENCE,
    },
    DeterministicScenario {
        id: "compaction_boundary",
        kind: ScenarioKind::CompactionBoundary,
        title: "Compaction boundary",
        phase: "Phase 12",
        user_task: "force context pressure, compact with provenance, then resume the task",
        status: ReplayStatus::ReplayFixtureReady,
        external_baseline: ExternalBaselineStatus::DeferredUntilReplayFixture,
        evidence: COMPACTION_BOUNDARY_EVIDENCE,
    },
    DeterministicScenario {
        id: "subagent_worktree_worker",
        kind: ScenarioKind::SubagentWorktreeWorker,
        title: "Subagent isolated worktree worker",
        phase: "Phase 12",
        user_task: "fork a child worker into an isolated worktree, review output, and clean up",
        status: ReplayStatus::ReplayFixtureReady,
        external_baseline: ExternalBaselineStatus::DeferredUntilReplayFixture,
        evidence: SUBAGENT_WORKTREE_EVIDENCE,
    },
    DeterministicScenario {
        id: "mcp_auth_repair",
        kind: ScenarioKind::McpAuthRepair,
        title: "MCP auth repair",
        phase: "Phase 12",
        user_task: "hit an MCP auth/server failure, surface repair, then retry after approval",
        status: ReplayStatus::RuntimeMapped,
        external_baseline: ExternalBaselineStatus::DeferredUntilReplayFixture,
        evidence: MCP_AUTH_REPAIR_EVIDENCE,
    },
];

pub fn deterministic_scenarios() -> &'static [DeterministicScenario] {
    SCENARIOS
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

pub fn format_scenario_matrix() -> String {
    let summary = scenario_matrix_summary();
    let mut lines = vec![
        "Phase 12 Deterministic Scenario Matrix".to_string(),
        format!(
            "Scenarios: {}  Replay-ready: {}  Required evidence: {}  External baseline: {}",
            summary.scenarios,
            summary.replay_ready,
            summary.required_evidence,
            if summary.external_baseline_ready {
                "ready"
            } else {
                "deferred"
            }
        ),
    ];

    for scenario in deterministic_scenarios() {
        lines.push(format!(
            "- {} [{}]: {}",
            scenario.id,
            scenario.status.label(),
            scenario.title
        ));
        let evidence = scenario
            .evidence
            .iter()
            .map(|item| format!("{}={}", item.surface.label(), item.target))
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(format!("  evidence: {}", evidence));
    }

    lines.push("Next gate: convert these mapped cases into deterministic replay fixtures before external Claude/Codex baseline runs.".to_string());
    lines.join("\n")
}

pub fn matrix_missing_required_kinds() -> Vec<ScenarioKind> {
    let covered = deterministic_scenarios()
        .iter()
        .map(|scenario| scenario.kind)
        .collect::<BTreeSet<_>>();
    REQUIRED_PHASE_12_KINDS
        .iter()
        .copied()
        .filter(|kind| !covered.contains(kind))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrix_covers_phase_12_required_scenarios() {
        assert_eq!(
            deterministic_scenarios().len(),
            REQUIRED_PHASE_12_KINDS.len()
        );
        assert!(
            matrix_missing_required_kinds().is_empty(),
            "missing required scenarios: {:?}",
            matrix_missing_required_kinds()
        );

        let ids = deterministic_scenarios()
            .iter()
            .map(|scenario| scenario.id)
            .collect::<BTreeSet<_>>();
        for kind in REQUIRED_PHASE_12_KINDS {
            assert!(ids.contains(kind.id()), "missing id {}", kind.id());
        }
    }

    #[test]
    fn each_scenario_requires_local_runtime_evidence() {
        for scenario in deterministic_scenarios() {
            assert!(
                scenario.evidence.iter().any(|evidence| {
                    evidence.required
                        && matches!(
                            evidence.surface,
                            EvidenceSurface::TraceEvent
                                | EvidenceSurface::RuntimePanel
                                | EvidenceSurface::RecoveryPlan
                                | EvidenceSurface::ToolMetadata
                                | EvidenceSurface::SessionStore
                        )
                }),
                "{} should require at least one local runtime evidence surface",
                scenario.id
            );
            assert!(
                scenario.evidence.len() >= 3,
                "{} should have enough evidence to avoid a one-bit smoke test",
                scenario.id
            );
        }
    }

    #[test]
    fn external_baseline_stays_deferred_until_replays_are_ready() {
        let summary = scenario_matrix_summary();
        assert_eq!(summary.replay_ready, 5);
        assert!(!summary.external_baseline_ready);
        assert!(deterministic_scenarios()
            .iter()
            .all(|scenario| scenario.external_baseline
                == ExternalBaselineStatus::DeferredUntilReplayFixture));
    }

    #[test]
    fn replay_ready_scenarios_are_tracked() {
        let replay_ready = deterministic_scenarios()
            .iter()
            .filter(|scenario| scenario.status == ReplayStatus::ReplayFixtureReady)
            .map(|scenario| scenario.kind)
            .collect::<Vec<_>>();

        assert_eq!(
            replay_ready,
            vec![
                ScenarioKind::FileEditRewind,
                ScenarioKind::BashBackgroundTask,
                ScenarioKind::PermissionDenialRetry,
                ScenarioKind::CompactionBoundary,
                ScenarioKind::SubagentWorktreeWorker,
            ]
        );
    }

    #[test]
    fn formatted_matrix_lists_all_cases_and_next_gate() {
        let rendered = format_scenario_matrix();
        for scenario in deterministic_scenarios() {
            assert!(rendered.contains(scenario.id), "missing {}", scenario.id);
        }
        assert!(rendered.contains("Next gate"));
        assert!(rendered.contains("External baseline: deferred"));
    }
}
