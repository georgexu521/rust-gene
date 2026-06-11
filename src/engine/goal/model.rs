use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalRunStatus {
    Active,
    Paused,
    Completed,
    Blocked,
    Failed,
    NeedsUser,
    Cancelled,
}

impl GoalRunStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            GoalRunStatus::Completed | GoalRunStatus::Failed | GoalRunStatus::Cancelled
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalRun {
    pub id: String,
    pub session_id: String,
    pub objective: String,
    pub status: GoalRunStatus,
    pub stop_rules: GoalStopRules,
    pub budget: GoalBudget,
    pub turn_count: u32,
    pub created_at: String,
    pub updated_at: String,
    pub last_closeout_status: Option<String>,
    pub last_blocker: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalStep {
    pub id: String,
    pub goal_id: String,
    pub session_id: String,
    pub turn_index: u32,
    pub prompt: String,
    pub closeout_status: Option<String>,
    pub verification_status: Option<String>,
    pub changed_files: usize,
    pub validation_items: usize,
    pub decision: GoalDecision,
    pub summary: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalBudget {
    pub max_turns: u32,
    pub max_minutes: u32,
    pub max_tokens: Option<u64>,
    pub max_repeated_blockers: u32,
}

impl Default for GoalBudget {
    fn default() -> Self {
        Self {
            max_turns: 10,
            max_minutes: 30,
            max_tokens: None,
            max_repeated_blockers: 3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalStopRules {
    #[serde(default)]
    pub validation_commands: Vec<String>,
    #[serde(default)]
    pub success_markers: Vec<String>,
    #[serde(default)]
    pub require_clean_worktree: bool,
    #[serde(default)]
    pub require_verified_closeout: bool,
}

impl Default for GoalStopRules {
    fn default() -> Self {
        Self {
            validation_commands: Vec::new(),
            success_markers: Vec::new(),
            require_clean_worktree: false,
            require_verified_closeout: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalDecision {
    Continue,
    Complete,
    Pause,
    NeedsUser,
    Blocked,
    Failed,
}
