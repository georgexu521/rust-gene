//! 记忆系统类型定义
//!
//! 定义了记忆系统使用的所有核心类型，包括：
//! - 记忆范围和边界
//! - 记忆记录和元数据
//! - 记忆查询和过滤
//! - 记忆评估和质量

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Agent 上下文类型
///
/// 标识当前是在哪种 agent 上下文中操作记忆
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryScopeKind {
    User,
    Project,
    Topic,
    Session,
    Agent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryTrustBoundary {
    User,
    Project,
    Topic,
    Session,
    Agent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryScopeIdentity {
    pub kind: MemoryScopeKind,
    pub id: String,
    pub parent: Option<String>,
    pub labels: Vec<String>,
    pub trust_boundary: MemoryTrustBoundary,
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

    pub fn identity(&self) -> MemoryScopeIdentity {
        let profile_parent = Some(format!("profile:{}", self.profile));
        if let Some(project_root) = self.project_root.as_deref() {
            let project_identity = project_identity(project_root);
            return MemoryScopeIdentity {
                kind: MemoryScopeKind::Project,
                id: project_identity.id,
                parent: profile_parent,
                labels: project_identity.labels,
                trust_boundary: MemoryTrustBoundary::Project,
            };
        }

        if let Some(user_id) = self
            .user_id
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            return MemoryScopeIdentity {
                kind: MemoryScopeKind::User,
                id: format!("{}:profile:{}", user_id.trim(), self.profile),
                parent: None,
                labels: vec![self.profile.clone()],
                trust_boundary: MemoryTrustBoundary::User,
            };
        }

        if !self.session_id.trim().is_empty() {
            return MemoryScopeIdentity {
                kind: MemoryScopeKind::Session,
                id: self.session_id.trim().to_string(),
                parent: profile_parent,
                labels: vec![self.platform.clone()],
                trust_boundary: MemoryTrustBoundary::Session,
            };
        }

        MemoryScopeIdentity {
            kind: MemoryScopeKind::Agent,
            id: format!("{}:profile:{}", self.platform, self.profile),
            parent: None,
            labels: vec![self.platform.clone(), self.profile.clone()],
            trust_boundary: MemoryTrustBoundary::Agent,
        }
    }

    pub fn identity_label(&self) -> String {
        let identity = self.identity();
        let kind = match identity.kind {
            MemoryScopeKind::User => "user",
            MemoryScopeKind::Project => "project",
            MemoryScopeKind::Topic => "topic",
            MemoryScopeKind::Session => "session",
            MemoryScopeKind::Agent => "agent",
        };
        format!("{kind}:{}", identity.id)
    }

    pub fn topic_identity(&self, topic: &str) -> Option<MemoryScopeIdentity> {
        let topic_id = normalize_scope_component(topic)?;
        let parent = if self.project_root.is_some() {
            Some(self.identity().id)
        } else if let Some(user_id) = self
            .user_id
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            Some(format!("{}:profile:{}", user_id.trim(), self.profile))
        } else {
            Some(format!("profile:{}", self.profile))
        };
        let mut labels = vec![topic_id.clone()];
        if let Some(project_root) = self.project_root.as_deref() {
            labels.push(project_root.display().to_string());
        }
        Some(MemoryScopeIdentity {
            kind: MemoryScopeKind::Topic,
            id: match parent.as_deref() {
                Some(parent) => format!("{parent}:topic:{topic_id}"),
                None => format!("topic:{topic_id}"),
            },
            parent,
            labels,
            trust_boundary: MemoryTrustBoundary::Topic,
        })
    }

    pub fn topic_identity_label(&self, topic: &str) -> Option<String> {
        self.topic_identity(topic)
            .map(|identity| format!("topic:{}", identity.id))
    }
}

impl Default for MemoryScope {
    fn default() -> Self {
        Self::local("unknown")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProjectIdentity {
    id: String,
    labels: Vec<String>,
}

fn project_identity(project_root: &Path) -> ProjectIdentity {
    if let Some(git) = git_project_identity(project_root) {
        let mut id = format!("git:{}", git.remote);
        let mut labels = vec![
            format!("project_root:{}", project_root.display()),
            format!("git_remote:{}", git.remote),
            format!("git_root:{}", git.root.display()),
            format!("git_dir:{}", git.git_dir.display()),
        ];
        if let Some(branch) = git.branch {
            labels.push(format!("git_branch:{branch}"));
        }
        if let Some(subpath) = git.subpath {
            id.push_str(":subpath:");
            id.push_str(&subpath);
            labels.push(format!("monorepo_subpath:{subpath}"));
        }
        return ProjectIdentity { id, labels };
    }
    let stable_path = project_root
        .canonicalize()
        .unwrap_or_else(|_| project_root.to_path_buf());
    ProjectIdentity {
        id: format!("path:{}", stable_path.display()),
        labels: vec![
            format!("project_root:{}", project_root.display()),
            format!("path:{}", stable_path.display()),
        ],
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GitProjectIdentity {
    remote: String,
    root: PathBuf,
    git_dir: PathBuf,
    branch: Option<String>,
    subpath: Option<String>,
}

fn git_project_identity(project_root: &Path) -> Option<GitProjectIdentity> {
    let mut current = Some(project_root);
    while let Some(dir) = current {
        if let Some(metadata) = git_metadata_at(dir) {
            if let Ok(content) = std::fs::read_to_string(&metadata.config_path) {
                if let Some(remote) = parse_git_remote_identity(&content) {
                    let root = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
                    let project = project_root
                        .canonicalize()
                        .unwrap_or_else(|_| project_root.to_path_buf());
                    let subpath = project.strip_prefix(&root).ok().and_then(|relative| {
                        if relative.as_os_str().is_empty() {
                            None
                        } else {
                            normalize_scope_component(&relative.to_string_lossy())
                        }
                    });
                    return Some(GitProjectIdentity {
                        remote,
                        root,
                        git_dir: metadata.git_dir,
                        branch: metadata.branch,
                        subpath,
                    });
                }
            }
        }
        current = dir.parent();
    }
    None
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GitMetadata {
    git_dir: PathBuf,
    config_path: PathBuf,
    branch: Option<String>,
}

fn git_metadata_at(root: &Path) -> Option<GitMetadata> {
    let dot_git = root.join(".git");
    let git_dir = if dot_git.is_dir() {
        dot_git
    } else if dot_git.is_file() {
        let content = std::fs::read_to_string(&dot_git).ok()?;
        let gitdir = content.trim().strip_prefix("gitdir:")?.trim();
        let path = PathBuf::from(gitdir);
        if path.is_absolute() {
            path
        } else {
            root.join(path)
        }
    } else {
        return None;
    };
    let common_dir = git_common_dir(&git_dir);
    let branch = git_branch_name(&git_dir).or_else(|| git_branch_name(&common_dir));
    Some(GitMetadata {
        git_dir,
        config_path: common_dir.join("config"),
        branch,
    })
}

fn git_common_dir(git_dir: &Path) -> PathBuf {
    let common_dir_path = git_dir.join("commondir");
    if let Ok(content) = std::fs::read_to_string(common_dir_path) {
        let value = content.trim();
        if !value.is_empty() {
            let path = PathBuf::from(value);
            return if path.is_absolute() {
                path
            } else {
                git_dir.join(path)
            };
        }
    }
    git_dir.to_path_buf()
}

fn git_branch_name(git_dir: &Path) -> Option<String> {
    let head = std::fs::read_to_string(git_dir.join("HEAD")).ok()?;
    let head = head.trim();
    let branch = head.strip_prefix("ref: refs/heads/")?;
    normalize_scope_component(branch)
}

fn parse_git_remote_identity(content: &str) -> Option<String> {
    content.lines().find_map(|line| {
        let line = line.trim();
        let remote = line.strip_prefix("url =")?.trim();
        if remote.is_empty() {
            None
        } else {
            Some(sanitize_git_remote_identity(remote))
        }
    })
}

fn sanitize_git_remote_identity(remote: &str) -> String {
    let mut value = remote.trim().trim_end_matches(".git").to_string();
    if let Some(scheme_pos) = value.find("://") {
        let authority_start = scheme_pos + 3;
        let authority = &value[authority_start..];
        if let Some(at) = authority.find('@') {
            value = format!("{}{}", &value[..authority_start], &authority[at + 1..]);
        }
    }
    value
}

fn normalize_scope_component(value: &str) -> Option<String> {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in value.trim().chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_dash = false;
        } else if (ch == '-'
            || ch == '_'
            || ch == '.'
            || ch == '/'
            || ch == '\\'
            || ch.is_whitespace())
            && !last_dash
            && !out.is_empty()
        {
            out.push('-');
            last_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
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
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub stale_after: Option<DateTime<Utc>>,
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
    pub fn default_stale_after(kind: MemoryKind, base: DateTime<Utc>) -> Option<DateTime<Utc>> {
        let days = match kind {
            MemoryKind::UserPreference | MemoryKind::Decision => return None,
            MemoryKind::ProjectFact | MemoryKind::ToolQuirk => 90,
            MemoryKind::WorkflowConvention => 180,
            MemoryKind::FailurePattern | MemoryKind::SuccessfulFix => 90,
            MemoryKind::SkillCandidate => 90,
            MemoryKind::Note => 30,
        };
        Some(base + chrono::Duration::days(days))
    }

    pub fn default_expires_at(kind: MemoryKind, base: DateTime<Utc>) -> Option<DateTime<Utc>> {
        let days = match kind {
            MemoryKind::Note => 180,
            MemoryKind::SkillCandidate => 365,
            _ => return None,
        };
        Some(base + chrono::Duration::days(days))
    }

    pub fn apply_default_lifecycle(&mut self) {
        if self.stale_after.is_none() {
            let base = self.last_verified_at.unwrap_or(self.created_at);
            self.stale_after = Self::default_stale_after(self.kind, base);
        }
        if self.expires_at.is_none() {
            self.expires_at = Self::default_expires_at(self.kind, self.created_at);
        }
    }

    pub fn is_expired_at(&self, now: DateTime<Utc>) -> bool {
        self.expires_at
            .map(|expires_at| expires_at <= now)
            .unwrap_or(false)
    }

    pub fn is_expired(&self) -> bool {
        self.is_expired_at(Utc::now())
    }

    pub fn needs_revalidation_at(&self, now: DateTime<Utc>) -> bool {
        if !matches!(self.status, MemoryStatus::Accepted) {
            return false;
        }
        self.stale_after
            .map(|stale_after| stale_after <= now)
            .unwrap_or(false)
    }

    pub fn needs_revalidation(&self) -> bool {
        self.needs_revalidation_at(Utc::now())
    }

    pub fn new(
        content: impl Into<String>,
        kind: MemoryKind,
        scope: MemoryScope,
        provenance: MemoryProvenance,
    ) -> Self {
        let content = content.into();
        let now = Utc::now();
        let mut record = Self {
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
            stale_after: None,
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
        };
        record.apply_default_lifecycle();
        record
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
            stale_after: None,
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
        record.apply_default_lifecycle();
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

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_scope_dir(name: &str) -> PathBuf {
        let unique = format!("priority-agent-scope-{}-{}", name, uuid::Uuid::new_v4());
        let base = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&base).unwrap();
        base
    }

    #[test]
    fn memory_scope_identity_prefers_git_remote_without_credentials() {
        let base = temp_scope_dir("git-identity");
        std::fs::create_dir_all(base.join(".git")).unwrap();
        std::fs::write(
            base.join(".git").join("config"),
            "[remote \"origin\"]\n    url = https://token@example.com/gex/project.git\n",
        )
        .unwrap();
        std::fs::write(base.join(".git").join("HEAD"), "ref: refs/heads/main\n").unwrap();
        let mut scope = MemoryScope::local("scope-test");
        scope.project_root = Some(base.clone());

        let identity = scope.identity();

        assert_eq!(identity.kind, MemoryScopeKind::Project);
        assert_eq!(identity.id, "git:https://example.com/gex/project");
        assert_eq!(identity.trust_boundary, MemoryTrustBoundary::Project);
        assert!(identity
            .labels
            .iter()
            .any(|label| label == "git_remote:https://example.com/gex/project"));
        assert!(identity
            .labels
            .iter()
            .any(|label| label == "git_branch:main"));
        assert!(scope
            .identity_label()
            .starts_with("project:git:https://example.com/gex/project"));

        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn memory_scope_identity_reads_branch_from_linked_worktree_git_file() {
        let repo = temp_scope_dir("git-worktree-repo");
        let worktree = std::env::temp_dir().join(format!(
            "priority-agent-scope-git-worktree-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&worktree);
        std::fs::create_dir_all(repo.join(".git").join("worktrees").join("agent-wt")).unwrap();
        std::fs::create_dir_all(&worktree).unwrap();
        std::fs::write(
            repo.join(".git").join("config"),
            "[remote \"origin\"]\n    url = git@github.com:gex/priority-agent.git\n",
        )
        .unwrap();
        std::fs::write(
            worktree.join(".git"),
            format!(
                "gitdir: {}\n",
                repo.join(".git")
                    .join("worktrees")
                    .join("agent-wt")
                    .display()
            ),
        )
        .unwrap();
        std::fs::write(
            repo.join(".git")
                .join("worktrees")
                .join("agent-wt")
                .join("commondir"),
            "../..\n",
        )
        .unwrap();
        std::fs::write(
            repo.join(".git")
                .join("worktrees")
                .join("agent-wt")
                .join("HEAD"),
            "ref: refs/heads/feature/memory-scope\n",
        )
        .unwrap();
        let mut scope = MemoryScope::local("worktree-scope");
        scope.project_root = Some(worktree.clone());

        let identity = scope.identity();

        assert_eq!(identity.kind, MemoryScopeKind::Project);
        assert_eq!(identity.id, "git:git@github.com:gex/priority-agent");
        assert!(identity
            .labels
            .iter()
            .any(|label| label == "git_branch:feature-memory-scope"));
        assert!(identity
            .labels
            .iter()
            .any(|label| label.starts_with("git_dir:")));

        let _ = std::fs::remove_dir_all(repo);
        let _ = std::fs::remove_dir_all(worktree);
    }

    #[test]
    fn memory_scope_identity_includes_monorepo_subpath_for_git_projects() {
        let base = temp_scope_dir("git-monorepo");
        let crate_a = base.join("crates").join("agent-a");
        let crate_b = base.join("crates").join("agent-b");
        std::fs::create_dir_all(base.join(".git")).unwrap();
        std::fs::create_dir_all(&crate_a).unwrap();
        std::fs::create_dir_all(&crate_b).unwrap();
        std::fs::write(
            base.join(".git").join("config"),
            "[remote \"origin\"]\n    url = git@github.com:gex/priority-agent.git\n",
        )
        .unwrap();
        let mut scope_a = MemoryScope::local("scope-a");
        scope_a.project_root = Some(crate_a);
        let mut scope_b = MemoryScope::local("scope-b");
        scope_b.project_root = Some(crate_b);

        let identity_a = scope_a.identity();
        let identity_b = scope_b.identity();

        assert_eq!(identity_a.kind, MemoryScopeKind::Project);
        assert_eq!(identity_a.parent.as_deref(), Some("profile:default"));
        assert!(identity_a.id.ends_with(":subpath:crates-agent-a"));
        assert!(identity_a
            .labels
            .iter()
            .any(|label| label == "monorepo_subpath:crates-agent-a"));
        assert_ne!(identity_a.id, identity_b.id);

        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn memory_scope_identity_falls_back_to_session_without_project() {
        let mut scope = MemoryScope::local("session-only");
        scope.project_root = None;

        let identity = scope.identity();

        assert_eq!(identity.kind, MemoryScopeKind::Session);
        assert_eq!(identity.id, "session-only");
        assert_eq!(scope.identity_label(), "session:session-only");
    }

    #[test]
    fn memory_scope_topic_identity_is_project_bound_when_project_exists() {
        let base = temp_scope_dir("topic-project");
        let mut scope = MemoryScope::local("topic-session");
        scope.project_root = Some(base.clone());

        let identity = scope
            .topic_identity("Rust Workflow")
            .expect("topic identity");

        assert_eq!(identity.kind, MemoryScopeKind::Topic);
        assert_eq!(identity.trust_boundary, MemoryTrustBoundary::Topic);
        assert!(identity.id.contains("topic:rust-workflow"));
        assert_eq!(identity.parent, Some(scope.identity().id));
        let label = format!("topic:{}", identity.id);
        assert_eq!(
            scope.topic_identity_label("Rust Workflow").as_deref(),
            Some(label.as_str())
        );

        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn memory_scope_topic_identity_can_be_global_profile_bound() {
        let mut scope = MemoryScope::local("topic-session");
        scope.project_root = None;
        scope.profile = "gex".to_string();

        let identity = scope
            .topic_identity("../Context Management.md")
            .expect("topic identity");

        assert_eq!(identity.kind, MemoryScopeKind::Topic);
        assert_eq!(identity.id, "profile:gex:topic:context-management-md");
        assert_eq!(identity.parent.as_deref(), Some("profile:gex"));
        assert!(scope.topic_identity("!!!").is_none());
    }
}
