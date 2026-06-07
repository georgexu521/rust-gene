//! Permission DTOs — shared types for permission explain decisions.

use serde::{Deserialize, Serialize};

/// Product-facing permission explain result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionExplainResult {
    pub decision: String,
    pub confidence: f32,
    pub risk_level: String,
    pub reasons: Vec<String>,
    pub warnings: Vec<String>,
    pub rule_views: Vec<PermissionRuleViewDto>,
}

/// Single permission rule as seen by the user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRuleViewDto {
    pub scope: String,
    pub matcher_key: String,
    pub effect: String,
    pub source: String,
    pub expires: Option<String>,
    pub risk_reason: Option<String>,
}

/// Shell command view attached to a permission request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellCommandViewDto {
    pub primary_command: String,
    pub normalized: String,
    pub detected_paths: Vec<String>,
    pub cwd_changing: bool,
    pub cwd_targets: Vec<String>,
    pub mutation_family: String,
    pub has_write_redirection: bool,
    pub write_targets: Vec<String>,
    pub dynamic_segments: Vec<String>,
    pub recommended_tool: Option<String>,
    pub warnings: Vec<String>,
}
