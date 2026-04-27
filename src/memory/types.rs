use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentContext {
    Primary,
    Subagent,
    Cron,
    Flush,
    Eval,
    Test,
}

impl Default for AgentContext {
    fn default() -> Self {
        Self::Primary
    }
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
            "convention" | "design" => Self::WorkflowConvention,
            "context" | "learned" | "session" => Self::WorkflowConvention,
            "decision" | "workflow" => Self::Decision,
            "failure" | "bug" => Self::FailurePattern,
            "fix" | "success" => Self::SuccessfulFix,
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
            tags: Vec::new(),
        }
    }
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
