//! Tool reliability audit snapshots.
//!
//! Claude Code's tool layer treats tools as product objects with explicit
//! safety, permission, display, and model-result semantics. This module gives
//! Priority Agent a similar side-effect-free audit surface so release-visible
//! tools cannot silently rely on broad defaults.

use super::{
    Tool, ToolInterruptBehavior, ToolOperationKind, ToolRegistry, ToolSearchOrReadSemantics,
    ToolUiRenderKind,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolReliabilitySample {
    pub label: String,
    pub params: Value,
}

impl ToolReliabilitySample {
    pub fn new(label: impl Into<String>, params: Value) -> Self {
        Self {
            label: label.into(),
            params,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolReliabilityIssue {
    pub severity: ToolReliabilityIssueSeverity,
    pub field: String,
    pub message: String,
}

impl ToolReliabilityIssue {
    fn warning(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: ToolReliabilityIssueSeverity::Warning,
            field: field.into(),
            message: message.into(),
        }
    }

    fn error(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: ToolReliabilityIssueSeverity::Error,
            field: field.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolReliabilityIssueSeverity {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolReliabilityProfile {
    pub tool_name: String,
    pub sample_label: String,
    pub sample_params: Value,
    pub operation_kind: ToolOperationKind,
    pub read_only: bool,
    pub concurrency_safe: bool,
    pub destructive: bool,
    pub open_world: bool,
    pub requires_user_interaction: bool,
    pub interrupt_behavior: ToolInterruptBehavior,
    pub max_result_size_chars: Option<usize>,
    pub permission_matcher_input: Option<String>,
    pub input_paths: Vec<String>,
    pub transcript_summary: Option<String>,
    pub ui_render_kind: ToolUiRenderKind,
    pub classifier_input: String,
    pub classifier_input_default_like: bool,
    pub search_or_read: ToolSearchOrReadSemantics,
    pub output_schema_present: bool,
    pub strict_schema: bool,
    pub should_defer: bool,
    pub always_load: bool,
    pub aliases: Vec<String>,
    pub search_hint: Option<String>,
    pub issues: Vec<ToolReliabilityIssue>,
}

impl ToolReliabilityProfile {
    pub fn has_errors(&self) -> bool {
        self.issues
            .iter()
            .any(|issue| issue.severity == ToolReliabilityIssueSeverity::Error)
    }
}

pub fn representative_tool_samples(tool_name: &str) -> Vec<ToolReliabilitySample> {
    match tool_name {
        "bash" => vec![
            ToolReliabilitySample::new("read_only", json!({ "command": "rg TODO src" })),
            ToolReliabilitySample::new("validation", json!({ "command": "cargo test -q" })),
            ToolReliabilitySample::new("mutation", json!({ "command": "cargo fmt" })),
            ToolReliabilitySample::new(
                "background",
                json!({
                    "command": "npm run dev",
                    "mode": "background"
                }),
            ),
            ToolReliabilitySample::new("destructive", json!({ "command": "rm -rf /" })),
        ],
        "run_tests" => vec![ToolReliabilitySample::new(
            "validation",
            json!({ "command": "cargo test -q" }),
        )],
        "start_dev_server" => vec![ToolReliabilitySample::new(
            "dev_server",
            json!({ "command": "npm run dev", "timeout_secs": 3600 }),
        )],
        "install_dependencies" => vec![ToolReliabilitySample::new(
            "project_install",
            json!({ "manager": "npm", "action": "install_project" }),
        )],
        "file_read" => vec![ToolReliabilitySample::new(
            "file",
            json!({ "path": "src/main.rs", "limit": 80 }),
        )],
        "file_write" => vec![ToolReliabilitySample::new(
            "write",
            json!({ "path": "tmp/reliability.txt", "content": "hello" }),
        )],
        "file_edit" => vec![ToolReliabilitySample::new(
            "replace",
            json!({
                "path": "src/main.rs",
                "old_string": "fn main",
                "new_string": "fn main"
            }),
        )],
        "file_patch" => vec![ToolReliabilitySample::new(
            "patch",
            json!({
                "operations": [{
                    "path": "src/main.rs",
                    "old_string": "fn main",
                    "new_string": "fn main"
                }]
            }),
        )],
        "grep" => vec![ToolReliabilitySample::new(
            "search",
            json!({ "pattern": "ToolReliability", "path": "src" }),
        )],
        "glob" => vec![ToolReliabilitySample::new(
            "search",
            json!({ "pattern": "src/**/*.rs" }),
        )],
        "agent" => vec![ToolReliabilitySample::new(
            "delegate",
            json!({
                "description": "Review file edit reliability",
                "prompt": "Inspect file edit reliability",
                "files": ["src/tools/file_tool/mod.rs"]
            }),
        )],
        "task_create" => vec![ToolReliabilitySample::new(
            "create",
            json!({ "description": "Review tool reliability", "task_type": "code" }),
        )],
        "task_get" | "task_update" | "task_stop" | "task_output" => {
            let mut params = json!({ "task_id": "task_12345678" });
            if tool_name == "task_update" {
                params["status"] = json!("running");
            } else if tool_name == "task_output" {
                params["action"] = json!("get");
            }
            vec![ToolReliabilitySample::new("task", params)]
        }
        "task_list" => vec![ToolReliabilitySample::new(
            "list",
            json!({ "status": "running" }),
        )],
        "web_fetch" => vec![ToolReliabilitySample::new(
            "fetch",
            json!({ "url": "https://example.com", "max_chars": 1200 }),
        )],
        "web_search" => vec![ToolReliabilitySample::new(
            "search",
            json!({ "query": "rust tool reliability", "num_results": 3 }),
        )],
        "git" => vec![
            ToolReliabilitySample::new("status", json!({ "action": "status" })),
            ToolReliabilitySample::new("push", json!({ "action": "push", "remote": "origin" })),
        ],
        "git_status" => vec![ToolReliabilitySample::new("status", json!({}))],
        "git_diff" => vec![ToolReliabilitySample::new(
            "diff",
            json!({ "path": "src/main.rs" }),
        )],
        "worktree" => vec![ToolReliabilitySample::new(
            "list",
            json!({ "action": "list" }),
        )],
        "mcp_tool" => vec![ToolReliabilitySample::new(
            "call",
            json!({ "server_name": "server", "tool_name": "tool", "arguments": {} }),
        )],
        "mcp_auth" => vec![ToolReliabilitySample::new(
            "auth",
            json!({ "server_name": "server" }),
        )],
        "list_mcp_resources" => vec![ToolReliabilitySample::new(
            "list",
            json!({ "server_name": "server" }),
        )],
        "read_mcp_resource" => vec![ToolReliabilitySample::new(
            "read",
            json!({ "server_name": "server", "uri": "file://resource" }),
        )],
        "remote_dev" => vec![ToolReliabilitySample::new(
            "detect",
            json!({ "action": "detect" }),
        )],
        "remote_trigger" => vec![ToolReliabilitySample::new(
            "dry_run",
            json!({ "action": "list" }),
        )],
        _ => vec![ToolReliabilitySample::new("default", json!({}))],
    }
}

pub fn audit_registry(registry: &ToolRegistry) -> Vec<ToolReliabilityProfile> {
    let mut profiles = Vec::new();
    let mut tools = registry.iter_tools().collect::<Vec<_>>();
    tools.sort_by_key(|tool| tool.name().to_string());

    for tool in tools {
        for sample in representative_tool_samples(tool.name()) {
            profiles.push(profile_tool(tool, sample));
        }
    }

    profiles
}

pub fn audit_release_tool_contracts(registry: &ToolRegistry) -> Vec<ToolReliabilityProfile> {
    audit_registry(registry)
        .into_iter()
        .filter(|profile| is_release_gate_tool(&profile.tool_name))
        .collect()
}

fn profile_tool(tool: &dyn Tool, sample: ToolReliabilitySample) -> ToolReliabilityProfile {
    let params = &sample.params;
    let operation_kind = tool.operation_kind(params);
    let read_only = tool.is_read_only(params);
    let concurrency_safe = tool.is_concurrency_safe(params);
    let destructive = tool.is_destructive(params);
    let open_world = tool.is_open_world(params);
    let requires_user_interaction = tool.requires_user_interaction();
    let interrupt_behavior = tool.interrupt_behavior();
    let max_result_size_chars = tool.max_result_size_chars();
    let permission_matcher_input = tool.permission_matcher_input(params);
    let input_paths = tool.input_paths(params);
    let transcript_summary = tool.transcript_summary(params);
    let ui_render_kind = tool.ui_render_kind(params);
    let classifier_input = tool.to_classifier_input(params);
    let classifier_input_default_like = classifier_input == default_classifier_input(tool, params);
    let search_or_read = tool.is_search_or_read_command(params);
    let output_schema_present = tool.output_schema().is_some();
    let strict_schema = tool.strict_schema();
    let should_defer = tool.should_defer();
    let always_load = tool.always_load();
    let aliases = tool
        .aliases()
        .iter()
        .map(|alias| alias.to_string())
        .collect();
    let search_hint = tool.search_hint().map(str::to_string);

    let mut profile = ToolReliabilityProfile {
        tool_name: tool.name().to_string(),
        sample_label: sample.label,
        sample_params: sample.params,
        operation_kind,
        read_only,
        concurrency_safe,
        destructive,
        open_world,
        requires_user_interaction,
        interrupt_behavior,
        max_result_size_chars,
        permission_matcher_input,
        input_paths,
        transcript_summary,
        ui_render_kind,
        classifier_input,
        classifier_input_default_like,
        search_or_read,
        output_schema_present,
        strict_schema,
        should_defer,
        always_load,
        aliases,
        search_hint,
        issues: Vec::new(),
    };
    profile.issues = audit_issues(&profile);
    profile
}

fn audit_issues(profile: &ToolReliabilityProfile) -> Vec<ToolReliabilityIssue> {
    let mut issues = Vec::new();

    if profile.read_only && !profile.concurrency_safe {
        issues.push(ToolReliabilityIssue::warning(
            "concurrency_safe",
            "read-only invocation is serial; keep only if ordering or side effects require it",
        ));
    }
    if !profile.read_only && profile.concurrency_safe {
        issues.push(ToolReliabilityIssue::error(
            "concurrency_safe",
            "mutating invocation cannot be concurrency-safe",
        ));
    }
    if matches!(
        profile.operation_kind,
        ToolOperationKind::Write | ToolOperationKind::Edit | ToolOperationKind::Patch
    ) && profile.input_paths.is_empty()
    {
        issues.push(ToolReliabilityIssue::error(
            "input_paths",
            "file mutation must expose affected paths",
        ));
    }
    if security_relevant(profile) && profile.permission_matcher_input.is_none() {
        issues.push(ToolReliabilityIssue::error(
            "permission_matcher_input",
            "security-relevant invocation needs stable permission matcher input",
        ));
    }
    if security_relevant(profile) && profile.classifier_input.trim().is_empty() {
        issues.push(ToolReliabilityIssue::error(
            "classifier_input",
            "security-relevant invocation needs classifier input",
        ));
    }
    if security_relevant(profile) && profile.classifier_input_default_like {
        issues.push(ToolReliabilityIssue::warning(
            "classifier_input",
            "classifier input looks like generic default output; prefer tool-specific security summary",
        ));
    }
    if profile.operation_kind == ToolOperationKind::Other
        && is_release_gate_tool(&profile.tool_name)
    {
        issues.push(ToolReliabilityIssue::error(
            "operation_kind",
            "release-visible tool must not use Other operation kind",
        ));
    }
    if profile.ui_render_kind == ToolUiRenderKind::Generic
        && is_release_gate_tool(&profile.tool_name)
        && profile.operation_kind != ToolOperationKind::Other
    {
        issues.push(ToolReliabilityIssue::error(
            "ui_render_kind",
            "release-visible tool needs a non-generic UI render lane",
        ));
    }
    if search_or_read_expected(profile) && !search_or_read_present(profile.search_or_read) {
        issues.push(ToolReliabilityIssue::error(
            "search_or_read",
            "read/search/list invocation must expose compact read/search semantics",
        ));
    }
    if requires_compact_result_budget(profile) && profile.max_result_size_chars.is_none() {
        issues.push(ToolReliabilityIssue::warning(
            "max_result_size_chars",
            "open-world or high-output read tool should declare provider-visible result budget",
        ));
    }
    if profile.transcript_summary.is_none() && is_summary_expected(profile) {
        issues.push(ToolReliabilityIssue::warning(
            "transcript_summary",
            "high-use tool should expose a compact transcript summary",
        ));
    }

    issues
}

fn default_classifier_input(tool: &dyn Tool, params: &Value) -> String {
    let keys: Vec<String> = params
        .as_object()
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default();
    format!("{}({})", tool.name(), keys.join(", "))
}

fn security_relevant(profile: &ToolReliabilityProfile) -> bool {
    profile.open_world
        || profile.destructive
        || matches!(
            profile.operation_kind,
            ToolOperationKind::Write
                | ToolOperationKind::Edit
                | ToolOperationKind::Patch
                | ToolOperationKind::Shell
                | ToolOperationKind::Task
                | ToolOperationKind::Network
        )
}

fn search_or_read_expected(profile: &ToolReliabilityProfile) -> bool {
    matches!(
        profile.operation_kind,
        ToolOperationKind::Read | ToolOperationKind::Search | ToolOperationKind::List
    )
}

fn search_or_read_present(semantics: ToolSearchOrReadSemantics) -> bool {
    semantics.is_search || semantics.is_read || semantics.is_list
}

fn requires_compact_result_budget(profile: &ToolReliabilityProfile) -> bool {
    profile.open_world
        || matches!(
            profile.operation_kind,
            ToolOperationKind::Search | ToolOperationKind::Network
        )
}

fn is_summary_expected(profile: &ToolReliabilityProfile) -> bool {
    is_release_gate_tool(&profile.tool_name)
        && !matches!(profile.sample_label.as_str(), "default")
        && matches!(
            profile.operation_kind,
            ToolOperationKind::Read
                | ToolOperationKind::Search
                | ToolOperationKind::List
                | ToolOperationKind::Write
                | ToolOperationKind::Edit
                | ToolOperationKind::Patch
                | ToolOperationKind::Shell
                | ToolOperationKind::Task
                | ToolOperationKind::Network
        )
}

fn is_release_gate_tool(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "bash"
            | "bash_output"
            | "bash_cancel"
            | "bash_tasks"
            | "run_tests"
            | "start_dev_server"
            | "install_dependencies"
            | "file_read"
            | "file_write"
            | "file_edit"
            | "file_patch"
            | "grep"
            | "glob"
            | "agent"
            | "task_create"
            | "task_get"
            | "task_list"
            | "task_update"
            | "task_stop"
            | "task_output"
            | "web_fetch"
            | "web_search"
            | "git"
            | "git_status"
            | "git_diff"
            | "worktree"
            | "mcp_tool"
            | "mcp_auth"
            | "list_mcp_resources"
            | "read_mcp_resource"
            | "remote_dev"
            | "remote_trigger"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::ToolRegistry;

    #[test]
    fn tool_reliability_audit_covers_representative_bash_modes() {
        let registry = ToolRegistry::default_registry();
        let bash_profiles = registry
            .reliability_audit()
            .into_iter()
            .filter(|profile| profile.tool_name == "bash")
            .collect::<Vec<_>>();

        assert!(bash_profiles
            .iter()
            .any(|profile| profile.sample_label == "read_only"
                && profile.read_only
                && profile.concurrency_safe
                && profile.ui_render_kind == ToolUiRenderKind::Search));
        assert!(bash_profiles
            .iter()
            .any(|profile| profile.sample_label == "destructive"
                && profile.destructive
                && !profile.read_only
                && !profile.concurrency_safe));
    }

    #[test]
    fn tool_reliability_release_tool_contracts_have_no_hard_errors() {
        let registry = ToolRegistry::default_registry();
        let failed = audit_release_tool_contracts(&registry)
            .into_iter()
            .filter(|profile| profile.has_errors())
            .collect::<Vec<_>>();

        assert!(
            failed.is_empty(),
            "release tool reliability failures:\n{}",
            failed
                .iter()
                .map(|profile| format!(
                    "{}:{} => {:?}",
                    profile.tool_name, profile.sample_label, profile.issues
                ))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
