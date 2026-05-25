use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentContext {
    #[default]
    Primary,
    Subagent,
    Cron,
    Flush,
    Eval,
    Test,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryScope {
    pub user_id: Option<String>,
    pub profile: String,
    pub project_root: Option<PathBuf>,
    pub session_id: String,
    pub parent_session_id: Option<String>,
    pub agent_context: AgentContext,
    pub platform: String,
}

impl MemoryScope {
    pub fn local(session_id: impl Into<String>) -> Self {
        Self {
            user_id: None,
            profile: "default".to_string(),
            project_root: std::env::current_dir().ok(),
            session_id: session_id.into(),
            parent_session_id: None,
            agent_context: AgentContext::Primary,
            platform: "cli".to_string(),
        }
    }
}

impl Default for MemoryScope {
    fn default() -> Self {
        Self::local("unknown")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryKind {
    UserPreference,
    ProjectFact,
    WorkflowConvention,
    ToolQuirk,
    FailurePattern,
    SuccessfulFix,
    Decision,
    SkillCandidate,
    Note,
}

impl MemoryKind {
    pub fn from_category(category: &str, content: &str) -> Self {
        let lower = content.to_lowercase();
        match category {
            "preference" | "user" => Self::UserPreference,
            "project_fact" | "fact" | "project" => Self::ProjectFact,
            "convention" | "design" => Self::WorkflowConvention,
            "context" | "learned" | "session" => Self::WorkflowConvention,
            "decision" | "workflow" => Self::Decision,
            "failure" | "failure_lesson" | "bug" => Self::FailurePattern,
            "fix" | "success" | "successful_fix" | "successful_strategy" | "strategy" => {
                Self::SuccessfulFix
            }
            "skill" | "skill_candidate" => Self::SkillCandidate,
            _ if lower.contains("error")
                || lower.contains("failed")
                || lower.contains("panic")
                || lower.contains("失败")
                || lower.contains("报错") =>
            {
                Self::FailurePattern
            }
            _ if lower.contains("tool")
                || lower.contains("bash")
                || lower.contains("mcp")
                || lower.contains("工具") =>
            {
                Self::ToolQuirk
            }
            _ if lower.contains("project")
                || lower.contains("repo")
                || lower.contains("项目")
                || lower.contains("仓库") =>
            {
                Self::ProjectFact
            }
            _ => Self::Note,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryStatus {
    Proposed,
    Accepted,
    Rejected,
    Superseded,
    Archived,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SensitivityLevel {
    Public,
    LocalOnly,
    SecretLike,
    Unsafe,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryProvenance {
    pub source: String,
    pub session_id: Option<String>,
    pub turn_index: Option<u64>,
    pub tool_name: Option<String>,
}

impl MemoryProvenance {
    pub fn local(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            session_id: None,
            turn_index: None,
            tool_name: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryEvidenceKind {
    UserStatement,
    ToolOutput,
    File,
    Trace,
    LearningEvent,
    RuntimeObservation,
    Inference,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryEvidenceRef {
    pub kind: MemoryEvidenceKind,
    pub source: String,
    pub summary: String,
    pub confidence: f32,
}

impl MemoryEvidenceRef {
    pub fn new(
        kind: MemoryEvidenceKind,
        source: impl Into<String>,
        summary: impl Into<String>,
        confidence: f32,
    ) -> Self {
        Self {
            kind,
            source: source.into(),
            summary: summary.into(),
            confidence: confidence.clamp(0.0, 1.0),
        }
    }

    pub fn inferred(source: impl Into<String>, summary: impl Into<String>) -> Self {
        Self::new(MemoryEvidenceKind::Inference, source, summary, 0.45)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryProjection {
    pub path: String,
    pub heading: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryStrategyMetadata {
    #[serde(default)]
    pub failed_strategy: Option<String>,
    #[serde(default)]
    pub better_strategy: Option<String>,
    #[serde(default)]
    pub context_tags: Vec<String>,
    #[serde(default)]
    pub failure_type: Option<String>,
    #[serde(default)]
    pub recovery_plan_id: Option<String>,
    #[serde(default)]
    pub risk_modifier: i8,
    #[serde(default)]
    pub value_modifier: i8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCandidate {
    pub id: String,
    pub scope: MemoryScope,
    pub kind: MemoryKind,
    pub category: String,
    pub content: String,
    pub provenance: MemoryProvenance,
    #[serde(default)]
    pub evidence: Vec<MemoryEvidenceRef>,
    pub confidence: f32,
    pub importance: u8,
    #[serde(default)]
    pub tags: Vec<String>,
    pub explicit: bool,
    #[serde(default)]
    pub source_experience_ids: Vec<String>,
    #[serde(default)]
    pub strategy: Option<MemoryStrategyMetadata>,
}

impl MemoryCandidate {
    pub fn new(
        content: impl Into<String>,
        category: impl Into<String>,
        scope: MemoryScope,
        provenance: MemoryProvenance,
    ) -> Self {
        let content = content.into();
        let category = category.into();
        let kind = MemoryKind::from_category(&category, &content);
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            scope,
            kind,
            category,
            content,
            provenance,
            evidence: Vec::new(),
            confidence: 0.5,
            importance: 3,
            tags: Vec::new(),
            explicit: false,
            source_experience_ids: Vec::new(),
            strategy: None,
        }
    }

    pub fn with_evidence(mut self, evidence: MemoryEvidenceRef) -> Self {
        self.evidence.push(evidence);
        self
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    pub fn explicit(mut self, explicit: bool) -> Self {
        self.explicit = explicit;
        self
    }

    pub fn confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    pub fn importance(mut self, importance: u8) -> Self {
        self.importance = importance.clamp(1, 5);
        self
    }

    pub fn strategy(mut self, strategy: MemoryStrategyMetadata) -> Self {
        self.strategy = Some(strategy);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRecord {
    pub id: String,
    pub scope: MemoryScope,
    pub kind: MemoryKind,
    pub content: String,
    pub summary: String,
    pub provenance: MemoryProvenance,
    pub confidence: f32,
    pub utility: f32,
    pub sensitivity: SensitivityLevel,
    pub status: MemoryStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(default = "default_importance")]
    pub importance: u8,
    #[serde(default)]
    pub evidence: Vec<MemoryEvidenceRef>,
    #[serde(default)]
    pub last_verified_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub last_used_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub use_count: u64,
    #[serde(default)]
    pub success_count: u64,
    #[serde(default)]
    pub failure_count: u64,
    #[serde(default)]
    pub supersedes: Vec<String>,
    #[serde(default)]
    pub superseded_by: Option<String>,
    #[serde(default)]
    pub projection: Option<MemoryProjection>,
    #[serde(default)]
    pub strategy: Option<MemoryStrategyMetadata>,
    pub tags: Vec<String>,
}

impl MemoryRecord {
    pub fn new(
        content: impl Into<String>,
        kind: MemoryKind,
        scope: MemoryScope,
        provenance: MemoryProvenance,
    ) -> Self {
        let content = content.into();
        let now = Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            scope,
            kind,
            summary: summarize_content(&content, 140),
            content,
            provenance,
            confidence: 0.5,
            utility: 0.5,
            sensitivity: SensitivityLevel::LocalOnly,
            status: MemoryStatus::Proposed,
            created_at: now,
            updated_at: now,
            expires_at: None,
            importance: default_importance(),
            evidence: Vec::new(),
            last_verified_at: None,
            last_used_at: None,
            use_count: 0,
            success_count: 0,
            failure_count: 0,
            supersedes: Vec::new(),
            superseded_by: None,
            projection: None,
            strategy: None,
            tags: Vec::new(),
        }
    }

    pub fn from_candidate(
        candidate: MemoryCandidate,
        status: MemoryStatus,
        confidence: f32,
        utility: f32,
        sensitivity: SensitivityLevel,
    ) -> Self {
        let content = candidate.content;
        let now = Utc::now();
        let last_verified_at = if candidate
            .evidence
            .iter()
            .any(|evidence| !matches!(evidence.kind, MemoryEvidenceKind::Inference))
        {
            Some(now)
        } else {
            None
        };
        let mut record = Self {
            id: candidate.id,
            scope: candidate.scope,
            kind: candidate.kind,
            summary: summarize_content(&content, 140),
            content,
            provenance: candidate.provenance,
            confidence: confidence.clamp(0.0, 1.0),
            utility: utility.clamp(0.0, 1.0),
            sensitivity,
            status,
            created_at: now,
            updated_at: now,
            expires_at: None,
            importance: candidate.importance.clamp(1, 5),
            evidence: candidate.evidence,
            last_verified_at,
            last_used_at: None,
            use_count: 0,
            success_count: 0,
            failure_count: 0,
            supersedes: Vec::new(),
            superseded_by: None,
            projection: None,
            strategy: candidate.strategy,
            tags: candidate.tags,
        };
        match record.kind {
            MemoryKind::FailurePattern => record.failure_count = 1,
            MemoryKind::SuccessfulFix => record.success_count = 1,
            _ => {}
        }
        record
    }
}

fn default_importance() -> u8 {
    3
}

fn summarize_content(content: &str, max_chars: usize) -> String {
    let compact = content.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= max_chars {
        compact
    } else {
        let mut out = compact.chars().take(max_chars).collect::<String>();
        out.push_str("...");
        out
    }
}
