//! Shared action boundary and side-effect labels.
//!
//! This is a first shared vocabulary for action review. It is intentionally
//! descriptive: permission enforcement can adopt these labels incrementally.

use crate::services::api::ToolCall;
use crate::tools::{Tool, ToolOperationKind};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActionSideEffectProfile {
    pub schema: String,
    pub tool_family: ToolBoundaryFamily,
    pub paths: Vec<WorkspacePathVerdict>,
    pub network: NetworkPolicyVerdict,
    pub external_side_effect: ExternalSideEffect,
    pub mutates_local_workspace: bool,
    pub mutates_local_machine: bool,
    pub remote_side_effect: bool,
    pub summary: String,
}

impl ActionSideEffectProfile {
    pub fn from_tool_call(
        tool_call: &ToolCall,
        tool: Option<&dyn Tool>,
        working_dir: &Path,
    ) -> Self {
        let tool_family = ToolBoundaryFamily::from_tool_name(&tool_call.name);
        let path_inputs = path_inputs(tool_call, tool);
        let paths = path_inputs
            .iter()
            .map(|path| WorkspaceBoundaryPolicy::classify_path(path, working_dir))
            .collect::<Vec<_>>();
        let network = NetworkPolicyVerdict::from_tool_call(tool_call);
        let external_side_effect =
            ExternalSideEffect::from_tool_call(tool_call, tool, &paths, &network);
        let mutates_local_workspace = matches!(
            external_side_effect,
            ExternalSideEffect::LocalWorkspaceMutation
                | ExternalSideEffect::GitRemotePublication
                | ExternalSideEffect::DatabaseOrDeploy
        );
        let mutates_local_machine = matches!(
            external_side_effect,
            ExternalSideEffect::LocalMachineMutation | ExternalSideEffect::CredentialOrAuth
        ) || paths
            .iter()
            .any(|path| !path.inside_workspace && path.class != WorkspacePathClass::Unknown);
        let remote_side_effect = matches!(
            external_side_effect,
            ExternalSideEffect::NetworkWrite
                | ExternalSideEffect::GitRemotePublication
                | ExternalSideEffect::DatabaseOrDeploy
                | ExternalSideEffect::PluginOrMcpUnknown
        );
        let summary = format!(
            "family={:?} external_effect={:?} network={:?} paths={}",
            tool_family,
            external_side_effect,
            network.class,
            paths.len()
        );

        Self {
            schema: "action_side_effect_profile.v1".to_string(),
            tool_family,
            paths,
            network,
            external_side_effect,
            mutates_local_workspace,
            mutates_local_machine,
            remote_side_effect,
            summary,
        }
    }

    pub fn has_external_or_sensitive_path(&self) -> bool {
        self.paths.iter().any(|path| {
            !path.inside_workspace
                || matches!(
                    path.class,
                    WorkspacePathClass::System
                        | WorkspacePathClass::HomePrivate
                        | WorkspacePathClass::Credential
                        | WorkspacePathClass::RepoMetadata
                )
        })
    }

    pub fn boundary_warnings(&self) -> Vec<String> {
        let mut warnings = Vec::new();
        if matches!(
            self.network.class,
            NetworkAccessClass::PackageInstall
                | NetworkAccessClass::UnknownNetworkCommand
                | NetworkAccessClass::RemoteService
                | NetworkAccessClass::UntrustedDomain
        ) {
            push_unique(
                &mut warnings,
                format!("NETWORK_BOUNDARY: {}", self.network.reason),
            );
        }

        for path in &self.paths {
            let warning = match path.class {
                WorkspacePathClass::External => Some(format!(
                    "OUTSIDE_WORKSPACE: {} ({})",
                    path.normalized, path.reason
                )),
                WorkspacePathClass::System => Some(format!(
                    "SYSTEM_PATH: {} ({})",
                    path.normalized, path.reason
                )),
                WorkspacePathClass::HomePrivate => Some(format!(
                    "HOME_PRIVATE_PATH: {} ({})",
                    path.normalized, path.reason
                )),
                WorkspacePathClass::RepoMetadata => Some(format!(
                    "REPO_METADATA_PATH: {} ({})",
                    path.normalized, path.reason
                )),
                WorkspacePathClass::Dependency => Some(format!(
                    "DEPENDENCY_PATH: {} ({})",
                    path.normalized, path.reason
                )),
                WorkspacePathClass::Generated => Some(format!(
                    "GENERATED_PATH: {} ({})",
                    path.normalized, path.reason
                )),
                WorkspacePathClass::Credential => Some(format!(
                    "CREDENTIAL_PATH: {} ({})",
                    path.normalized, path.reason
                )),
                WorkspacePathClass::Workspace | WorkspacePathClass::Unknown => None,
            };
            if let Some(warning) = warning {
                push_unique(&mut warnings, warning);
            }
            if matches!(
                path.class,
                WorkspacePathClass::System
                    | WorkspacePathClass::HomePrivate
                    | WorkspacePathClass::Credential
            ) {
                push_unique(
                    &mut warnings,
                    format!("HIGH_RISK_PATH: {} ({})", path.normalized, path.reason),
                );
            }
        }

        match self.external_side_effect {
            ExternalSideEffect::PluginOrMcpUnknown => push_unique(
                &mut warnings,
                "PLUGIN_OR_MCP_BOUNDARY: plugin/MCP execution has unknown external side effects"
                    .to_string(),
            ),
            ExternalSideEffect::CredentialOrAuth => push_unique(
                &mut warnings,
                "AUTH_BOUNDARY: action can grant or mutate credential/auth state".to_string(),
            ),
            ExternalSideEffect::GitRemotePublication => push_unique(
                &mut warnings,
                "REMOTE_PUBLICATION: git action can publish to a remote".to_string(),
            ),
            ExternalSideEffect::DatabaseOrDeploy => push_unique(
                &mut warnings,
                "EXTERNAL_SERVICE_BOUNDARY: database/deploy action has external effects"
                    .to_string(),
            ),
            ExternalSideEffect::LocalMachineMutation => push_unique(
                &mut warnings,
                "LOCAL_MACHINE_MUTATION: action can mutate local machine state".to_string(),
            ),
            ExternalSideEffect::LocalWorkspaceMutation => push_unique(
                &mut warnings,
                "LOCAL_WORKSPACE_MUTATION: action can mutate workspace files".to_string(),
            ),
            ExternalSideEffect::NetworkWrite => push_unique(
                &mut warnings,
                "NETWORK_WRITE: action can mutate remote network state".to_string(),
            ),
            ExternalSideEffect::NetworkRead | ExternalSideEffect::None => {}
        }

        warnings
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolBoundaryFamily {
    File,
    Bash,
    Mcp,
    Plugin,
    Git,
    Network,
    LocalRuntime,
    Other,
}

impl ToolBoundaryFamily {
    fn from_tool_name(tool_name: &str) -> Self {
        match tool_name {
            "file_read" | "file_write" | "file_edit" | "file_patch" | "format" => Self::File,
            "bash" | "bash_output" | "bash_tasks" | "bash_cancel" | "powershell" => Self::Bash,
            "mcp_tool" | "mcp_auth" | "list_mcp_resources" | "read_mcp_resource" => Self::Mcp,
            "plugin" | "plugin_list" | "plugin_manage" | "plugin_runtime" => Self::Plugin,
            name if name.starts_with("plugin_") => Self::Plugin,
            "git" | "git_status" | "git_diff" => Self::Git,
            "web_fetch" | "web_search" | "github" | "remote_trigger" | "remote_dev" => {
                Self::Network
            }
            "install_dependencies" | "start_dev_server" | "config" | "memory_clear" => {
                Self::LocalRuntime
            }
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspacePathVerdict {
    pub path: String,
    pub normalized: String,
    pub class: WorkspacePathClass,
    pub inside_workspace: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkspacePathClass {
    Workspace,
    External,
    System,
    HomePrivate,
    RepoMetadata,
    Dependency,
    Generated,
    Credential,
    Unknown,
}

pub struct WorkspaceBoundaryPolicy;

impl WorkspaceBoundaryPolicy {
    pub fn classify_path(path: &str, working_dir: &Path) -> WorkspacePathVerdict {
        let normalized = normalize_path(path, working_dir);
        let lower = normalized.to_string_lossy().to_ascii_lowercase();
        let inside_workspace = trusted_workspace_roots(working_dir)
            .into_iter()
            .any(|root| normalized.starts_with(root));
        let class = if path.trim().is_empty() {
            WorkspacePathClass::Unknown
        } else if inside_workspace && is_live_eval_worktree_path(&normalized) {
            WorkspacePathClass::Workspace
        } else if lower.contains("/.git/") || lower.ends_with("/.git") {
            WorkspacePathClass::RepoMetadata
        } else if lower.contains("/node_modules/")
            || lower.contains("/target/")
            || lower.contains("/vendor/")
            || lower.contains("/.venv/")
        {
            WorkspacePathClass::Dependency
        } else if lower.contains("/dist/")
            || lower.contains("/build/")
            || lower.contains("/coverage/")
            || lower.contains("/.next/")
        {
            WorkspacePathClass::Generated
        } else if contains_credential_marker(&lower) {
            WorkspacePathClass::Credential
        } else if lower.starts_with("/etc/")
            || lower.starts_with("/usr/")
            || lower.starts_with("/bin/")
            || lower.starts_with("/sbin/")
            || lower.starts_with("/var/")
            || lower.starts_with("/dev/")
        {
            WorkspacePathClass::System
        } else if lower.contains("/.ssh/") || lower.contains("/.gnupg/") {
            WorkspacePathClass::HomePrivate
        } else if inside_workspace {
            WorkspacePathClass::Workspace
        } else {
            WorkspacePathClass::External
        };

        let reason = match class {
            WorkspacePathClass::Workspace => "path is inside a trusted workspace root",
            WorkspacePathClass::External => "path is outside the working directory",
            WorkspacePathClass::System => "path is under a system directory",
            WorkspacePathClass::HomePrivate => "path is in a private home credential area",
            WorkspacePathClass::RepoMetadata => "path targets repository metadata",
            WorkspacePathClass::Dependency => "path targets generated dependency storage",
            WorkspacePathClass::Generated => "path targets generated output",
            WorkspacePathClass::Credential => "path resembles a credential or secret",
            WorkspacePathClass::Unknown => "path is missing or unknown",
        }
        .to_string();

        WorkspacePathVerdict {
            path: path.to_string(),
            normalized: normalized.display().to_string(),
            class,
            inside_workspace,
            reason,
        }
    }
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NetworkPolicyVerdict {
    pub class: NetworkAccessClass,
    pub target: Option<String>,
    pub trusted: bool,
    pub reason: String,
}

impl NetworkPolicyVerdict {
    fn none() -> Self {
        Self {
            class: NetworkAccessClass::None,
            target: None,
            trusted: true,
            reason: "no network access detected".to_string(),
        }
    }

    fn from_tool_call(tool_call: &ToolCall) -> Self {
        match tool_call.name.as_str() {
            "start_dev_server" => Self {
                class: NetworkAccessClass::Localhost,
                target: tool_call
                    .arguments
                    .get("expected_url")
                    .and_then(serde_json::Value::as_str)
                    .or_else(|| {
                        tool_call
                            .arguments
                            .get("command")
                            .and_then(serde_json::Value::as_str)
                    })
                    .map(str::to_string),
                trusted: true,
                reason: "local development server task exposes a localhost-oriented service"
                    .to_string(),
            },
            "install_dependencies" => Self {
                class: NetworkAccessClass::PackageInstall,
                target: tool_call.arguments["manager"].as_str().map(str::to_string),
                trusted: false,
                reason: "dependency installation can download and execute external content"
                    .to_string(),
            },
            "bash" => bash_network_verdict(tool_call),
            "web_fetch" => {
                let url = tool_call.arguments["url"].as_str().unwrap_or_default();
                url_network_verdict(url, NetworkAccessClass::TrustedDomain)
            }
            "web_search" => Self {
                class: NetworkAccessClass::UntrustedDomain,
                target: tool_call.arguments["query"].as_str().map(str::to_string),
                trusted: false,
                reason: "web search reaches an external search provider".to_string(),
            },
            "github" => Self {
                class: NetworkAccessClass::RemoteService,
                target: tool_call.arguments["action"].as_str().map(str::to_string),
                trusted: false,
                reason: "GitHub action can contact a remote service".to_string(),
            },
            "remote_trigger" | "remote_dev" => Self {
                class: NetworkAccessClass::RemoteService,
                target: tool_call.arguments["action"].as_str().map(str::to_string),
                trusted: false,
                reason: "remote tool can contact or mutate remote state".to_string(),
            },
            "mcp_tool" | "mcp_auth" | "list_mcp_resources" | "read_mcp_resource" => Self {
                class: NetworkAccessClass::RemoteService,
                target: tool_call.arguments["server_name"]
                    .as_str()
                    .map(str::to_string),
                trusted: false,
                reason: "MCP access may execute or read outside the local runtime".to_string(),
            },
            _ => Self::none(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NetworkAccessClass {
    None,
    Localhost,
    TrustedDomain,
    UntrustedDomain,
    PackageInstall,
    UnknownNetworkCommand,
    RemoteService,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExternalSideEffect {
    None,
    LocalWorkspaceMutation,
    LocalMachineMutation,
    NetworkRead,
    NetworkWrite,
    GitRemotePublication,
    DatabaseOrDeploy,
    CredentialOrAuth,
    PluginOrMcpUnknown,
}

impl ExternalSideEffect {
    fn from_tool_call(
        tool_call: &ToolCall,
        tool: Option<&dyn Tool>,
        paths: &[WorkspacePathVerdict],
        network: &NetworkPolicyVerdict,
    ) -> Self {
        if paths
            .iter()
            .any(|path| path.class == WorkspacePathClass::Credential)
        {
            return Self::CredentialOrAuth;
        }

        match tool_call.name.as_str() {
            "file_write" | "file_edit" | "file_patch" => Self::LocalWorkspaceMutation,
            "format" => match tool_call.arguments["action"].as_str() {
                Some("check") => Self::None,
                _ => Self::LocalWorkspaceMutation,
            },
            "git" => match tool_call.arguments["action"].as_str() {
                Some("push") => Self::GitRemotePublication,
                Some("checkout" | "branch" | "add" | "commit") => Self::LocalWorkspaceMutation,
                _ => Self::None,
            },
            "start_dev_server" => Self::LocalMachineMutation,
            "install_dependencies" => Self::LocalMachineMutation,
            "bash" => bash_external_side_effect(tool_call, paths, network),
            "web_fetch" | "web_search" => Self::NetworkRead,
            "github" => match tool_call.arguments["action"].as_str() {
                Some("pr_create") => Self::NetworkWrite,
                _ => Self::NetworkRead,
            },
            "remote_trigger" | "remote_dev" => Self::NetworkWrite,
            "mcp_tool" => Self::PluginOrMcpUnknown,
            "mcp_auth" => Self::CredentialOrAuth,
            "list_mcp_resources" | "read_mcp_resource" => Self::NetworkRead,
            "plugin" | "plugin_runtime" => Self::PluginOrMcpUnknown,
            "plugin_list" => Self::None,
            "plugin_manage" => match tool_call.arguments["action"].as_str() {
                Some("list" | "status" | "validate") => Self::None,
                Some("run") => Self::PluginOrMcpUnknown,
                Some("enable" | "disable" | "reload" | "sign" | "generate_key") => {
                    Self::LocalMachineMutation
                }
                _ => Self::PluginOrMcpUnknown,
            },
            name if name.starts_with("plugin_") => Self::PluginOrMcpUnknown,
            "memory_clear" | "config" => Self::LocalMachineMutation,
            _ => match tool.map(|tool| tool.operation_kind(&tool_call.arguments)) {
                Some(
                    ToolOperationKind::Write | ToolOperationKind::Edit | ToolOperationKind::Patch,
                ) => Self::LocalWorkspaceMutation,
                Some(ToolOperationKind::Network) => Self::NetworkRead,
                _ => Self::None,
            },
        }
    }
}

fn path_inputs(tool_call: &ToolCall, tool: Option<&dyn Tool>) -> Vec<String> {
    let mut paths = tool
        .map(|tool| tool.input_paths(&tool_call.arguments))
        .unwrap_or_default();

    paths.extend(common_path_inputs(&tool_call.arguments));

    if tool_call.name == "bash" {
        if let Some(command) = tool_call.arguments["command"].as_str() {
            let classification =
                crate::tools::bash_tool::command_classifier::classify_command(command);
            paths.extend(classification.absolute_path_patterns);
            paths.extend(classification.mutation_paths);
            paths.extend(classification.path_patterns);
            paths.extend(shell_absolute_path_tokens(command));
        }
    }

    paths.sort();
    paths.dedup();
    paths
}

fn common_path_inputs(arguments: &serde_json::Value) -> Vec<String> {
    [
        "path",
        "file_path",
        "directory",
        "working_dir",
        "manifest_path",
    ]
    .iter()
    .filter_map(|key| arguments.get(*key).and_then(serde_json::Value::as_str))
    .filter(|value| !value.trim().is_empty())
    .map(str::to_string)
    .collect()
}

fn shell_absolute_path_tokens(command: &str) -> Vec<String> {
    let mut paths = Vec::new();
    for raw in command.split(|ch: char| {
        ch.is_whitespace()
            || matches!(
                ch,
                '"' | '\'' | '`' | '(' | ')' | '{' | '}' | '[' | ']' | ';' | '|' | '&'
            )
    }) {
        let token = raw.trim_matches(|ch: char| matches!(ch, '<' | '>' | ',' | ':' | '='));
        if token.is_empty() {
            continue;
        }
        if token.contains("://") {
            continue;
        }

        let candidate = if token.starts_with('/') {
            Some(token.to_string())
        } else if let Some((_, path)) = token.split_once("=/") {
            Some(format!("/{path}"))
        } else if let Some((_, path)) = token.split_once(":/") {
            Some(format!("/{path}"))
        } else {
            None
        };

        if let Some(path) = candidate {
            let trimmed = path.as_str().trim_matches(|ch: char| {
                matches!(ch, '<' | '>' | ',' | ':' | ')' | ']' | '}' | '.')
            });
            if trimmed.starts_with('/') {
                paths.push(trimmed.to_string());
            }
        }
    }
    paths
}

fn bash_network_verdict(tool_call: &ToolCall) -> NetworkPolicyVerdict {
    let command = tool_call.arguments["command"].as_str().unwrap_or_default();
    let classification = crate::tools::bash_tool::command_classifier::classify_command(command);
    if !classification.network_access {
        return NetworkPolicyVerdict::none();
    }
    if classification.category
        == crate::tools::bash_tool::command_classifier::ShellCommandCategory::PackageInstall
    {
        if is_local_only_package_install_command(command) {
            return NetworkPolicyVerdict::none();
        }
        return NetworkPolicyVerdict {
            class: NetworkAccessClass::PackageInstall,
            target: Some(classification.normalized_command),
            trusted: false,
            reason: "package installation can download and execute external content".to_string(),
        };
    }
    NetworkPolicyVerdict {
        class: NetworkAccessClass::UnknownNetworkCommand,
        target: Some(classification.normalized_command),
        trusted: false,
        reason: "shell command may access the network".to_string(),
    }
}

fn bash_external_side_effect(
    tool_call: &ToolCall,
    paths: &[WorkspacePathVerdict],
    network: &NetworkPolicyVerdict,
) -> ExternalSideEffect {
    let command = tool_call.arguments["command"].as_str().unwrap_or_default();
    let lower = command.to_ascii_lowercase();
    let classification = crate::tools::bash_tool::command_classifier::classify_command(command);
    if contains_database_or_deploy_marker(&lower) {
        return ExternalSideEffect::DatabaseOrDeploy;
    }
    if classification.command_kind
        == crate::tools::bash_tool::command_classifier::CommandKind::Dangerous
    {
        return ExternalSideEffect::LocalMachineMutation;
    }
    if network.class == NetworkAccessClass::PackageInstall {
        return ExternalSideEffect::LocalMachineMutation;
    }
    if paths.iter().any(|path| !path.inside_workspace) || classification.external_path_access {
        return ExternalSideEffect::LocalMachineMutation;
    }
    if !classification.mutation_paths.is_empty()
        || !classification.mutation_indicators.is_empty()
        || classification.command_plan.has_write_redirection
    {
        return ExternalSideEffect::LocalWorkspaceMutation;
    }
    if classification.network_access {
        return ExternalSideEffect::NetworkRead;
    }
    ExternalSideEffect::None
}

fn is_local_only_package_install_command(command: &str) -> bool {
    let normalized =
        crate::tools::bash_tool::command_classifier::normalize_command_for_match(command);
    if normalized.is_empty() {
        return false;
    }
    let tokens = normalized.split_whitespace().collect::<Vec<_>>();
    let install_index = match tokens.as_slice() {
        ["pip" | "pip3", "install", ..] => Some(2usize),
        ["python" | "python3", "-m", "pip", "install", ..] => Some(4usize),
        ["uv", "pip", "install", ..] => Some(3usize),
        _ => None,
    };
    let Some(mut index) = install_index else {
        return false;
    };
    let mut targets = Vec::new();
    while index < tokens.len() {
        let token = tokens[index];
        if token == "-e" || token == "--editable" {
            index += 1;
            if index < tokens.len() {
                targets.push(tokens[index]);
            }
            index += 1;
            continue;
        }
        if token.starts_with('-') {
            index += 1;
            continue;
        }
        targets.push(token);
        index += 1;
    }

    !targets.is_empty()
        && targets
            .iter()
            .all(|target| looks_like_local_install_target(target))
}

fn looks_like_local_install_target(target: &str) -> bool {
    if target.starts_with("http://")
        || target.starts_with("https://")
        || target.starts_with("git+")
        || target.starts_with("ssh://")
    {
        return false;
    }
    target == "."
        || target.starts_with("./")
        || target.starts_with("../")
        || target.starts_with('/')
        || target.contains('/')
}

fn contains_database_or_deploy_marker(command: &str) -> bool {
    [
        "migrate",
        "migration",
        "deploy",
        "kubectl ",
        "helm ",
        "terraform ",
        "psql ",
        "mysql ",
        "prisma migrate",
        "diesel migration",
    ]
    .iter()
    .any(|marker| command.contains(marker))
}

fn url_network_verdict(url: &str, trusted_class: NetworkAccessClass) -> NetworkPolicyVerdict {
    let Some(host) = url_host(url) else {
        return NetworkPolicyVerdict {
            class: NetworkAccessClass::UntrustedDomain,
            target: None,
            trusted: false,
            reason: "network URL has no parseable host".to_string(),
        };
    };
    if is_localhost(&host) {
        return NetworkPolicyVerdict {
            class: NetworkAccessClass::Localhost,
            target: Some(host),
            trusted: true,
            reason: "URL targets localhost".to_string(),
        };
    }
    let trusted = trusted_domain(&host);
    NetworkPolicyVerdict {
        class: if trusted {
            trusted_class
        } else {
            NetworkAccessClass::UntrustedDomain
        },
        target: Some(host),
        trusted,
        reason: if trusted {
            "URL host is in trusted domains".to_string()
        } else {
            "URL host is not in trusted domains".to_string()
        },
    }
}

fn normalize_path(path: &str, working_dir: &Path) -> PathBuf {
    let input = Path::new(path);
    let joined = if input.is_absolute() {
        input.to_path_buf()
    } else {
        working_dir.join(input)
    };
    normalize_existing_or_logical(&joined)
}

fn normalize_existing_or_logical(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

fn trusted_workspace_roots(working_dir: &Path) -> Vec<PathBuf> {
    let mut roots = vec![normalize_existing_or_logical(working_dir)];
    if let Ok(extra) = std::env::var("PRIORITY_AGENT_TRUSTED_WORKSPACES") {
        roots.extend(
            extra
                .split(':')
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(|part| normalize_existing_or_logical(Path::new(part))),
        );
    }
    roots
}

fn is_live_eval_worktree_path(path: &Path) -> bool {
    let lower = path.to_string_lossy().to_ascii_lowercase();
    lower.contains("/target/live-evals/")
        && (lower.contains("/worktree/") || lower.ends_with("/worktree"))
}

fn contains_credential_marker(path: &str) -> bool {
    [
        ".env",
        "id_rsa",
        "id_ed25519",
        "authorized_keys",
        "credentials",
        "token",
        "secret",
    ]
    .iter()
    .any(|marker| path.contains(marker))
}

fn url_host(url: &str) -> Option<String> {
    let after_scheme = url.split_once("://")?.1;
    let host_port = after_scheme.split('/').next()?.split('@').next_back()?;
    let host = if host_port.starts_with('[') {
        host_port
            .find(']')
            .map(|end| host_port[1..end].to_ascii_lowercase())?
    } else {
        host_port
            .split(':')
            .next()
            .unwrap_or(host_port)
            .to_ascii_lowercase()
    };
    (!host.is_empty()).then_some(host)
}

fn trusted_domain(host: &str) -> bool {
    std::env::var("PRIORITY_AGENT_TRUSTED_DOMAINS")
        .ok()
        .is_some_and(|trusted| {
            trusted
                .split(',')
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .any(|domain| host == domain || host.ends_with(&format!(".{domain}")))
        })
}

fn is_localhost(host: &str) -> bool {
    matches!(host, "localhost" | "127.0.0.1" | "::1")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::{ToolContext, ToolResult};
    use async_trait::async_trait;
    use serde_json::{json, Value};

    struct PathTool;

    #[async_trait]
    impl Tool for PathTool {
        fn name(&self) -> &str {
            "file_edit"
        }

        fn description(&self) -> &str {
            "path classifier test tool"
        }

        fn parameters(&self) -> Value {
            json!({"type": "object", "properties": {"path": {"type": "string"}}})
        }

        async fn execute(&self, _params: Value, _context: ToolContext) -> ToolResult {
            ToolResult::success("unused")
        }
    }

    fn call(name: &str, args: Value) -> ToolCall {
        ToolCall {
            id: "call".to_string(),
            name: name.to_string(),
            arguments: args,
        }
    }

    #[test]
    fn classifies_workspace_and_external_paths() {
        let tool = PathTool;
        let profile = ActionSideEffectProfile::from_tool_call(
            &call("file_edit", json!({"path": "src/lib.rs"})),
            Some(&tool),
            Path::new("/repo"),
        );
        assert_eq!(profile.paths[0].class, WorkspacePathClass::Workspace);
        assert!(profile.mutates_local_workspace);

        let external = WorkspaceBoundaryPolicy::classify_path("/etc/hosts", Path::new("/repo"));
        assert_eq!(external.class, WorkspacePathClass::System);
        assert!(!external.inside_workspace);
    }

    #[test]
    fn live_eval_worktree_paths_are_workspace_not_dependency_paths() {
        let working_dir = Path::new("/repo/target/live-evals/run-123/minimum-agent-loop/worktree");
        let verdict =
            WorkspaceBoundaryPolicy::classify_path("fixtures/mva_loop/calculator.py", working_dir);

        assert_eq!(verdict.class, WorkspacePathClass::Workspace);
        assert!(verdict.inside_workspace);
    }

    #[test]
    fn live_eval_worktree_root_path_is_workspace_not_dependency_path() {
        let working_dir = Path::new("/repo/target/live-evals/run-123/minimum-agent-loop/worktree");
        let verdict = WorkspaceBoundaryPolicy::classify_path(
            "/repo/target/live-evals/run-123/minimum-agent-loop/worktree",
            working_dir,
        );

        assert_eq!(verdict.class, WorkspacePathClass::Workspace);
        assert!(verdict.inside_workspace);
    }

    #[test]
    fn classifies_bash_network_command() {
        let profile = ActionSideEffectProfile::from_tool_call(
            &call("bash", json!({"command": "curl https://example.com"})),
            None,
            Path::new("/repo"),
        );
        assert_eq!(
            profile.network.class,
            NetworkAccessClass::UnknownNetworkCommand
        );
        assert_eq!(
            profile.external_side_effect,
            ExternalSideEffect::NetworkRead
        );
    }

    #[test]
    fn local_pip_install_is_not_classified_as_network_package_install() {
        let profile = ActionSideEffectProfile::from_tool_call(
            &call(
                "bash",
                json!({"command": "python -m pip install -q fixtures/core_quality/terminal_app"}),
            ),
            None,
            Path::new("/repo"),
        );
        assert_eq!(profile.network.class, NetworkAccessClass::None);
    }

    #[test]
    fn remote_pip_install_remains_package_install() {
        let profile = ActionSideEffectProfile::from_tool_call(
            &call("bash", json!({"command": "pip install requests"})),
            None,
            Path::new("/repo"),
        );
        assert_eq!(profile.network.class, NetworkAccessClass::PackageInstall);
    }

    #[test]
    fn classifies_git_push_as_remote_publication() {
        let profile = ActionSideEffectProfile::from_tool_call(
            &call("git", json!({"action": "push"})),
            None,
            Path::new("/repo"),
        );
        assert_eq!(
            profile.external_side_effect,
            ExternalSideEffect::GitRemotePublication
        );
        assert!(profile.remote_side_effect);
    }

    #[test]
    fn classifies_start_dev_server_as_local_machine_task() {
        let profile = ActionSideEffectProfile::from_tool_call(
            &call(
                "start_dev_server",
                json!({"command": "npm run dev", "expected_url": "http://localhost:5173"}),
            ),
            None,
            Path::new("/repo"),
        );

        assert_eq!(profile.network.class, NetworkAccessClass::Localhost);
        assert_eq!(
            profile.external_side_effect,
            ExternalSideEffect::LocalMachineMutation
        );
        assert!(profile.mutates_local_machine);
    }

    #[test]
    fn classifies_install_dependencies_as_package_install() {
        let profile = ActionSideEffectProfile::from_tool_call(
            &call("install_dependencies", json!({"manager": "npm"})),
            None,
            Path::new("/repo"),
        );

        assert_eq!(profile.network.class, NetworkAccessClass::PackageInstall);
        assert_eq!(
            profile.external_side_effect,
            ExternalSideEffect::LocalMachineMutation
        );
        assert!(profile.mutates_local_machine);
    }

    #[test]
    fn classifies_common_paths_without_tool_contract() {
        let profile = ActionSideEffectProfile::from_tool_call(
            &call(
                "file_write",
                json!({"path": "../outside.rs", "content": ""}),
            ),
            None,
            Path::new("/repo/project"),
        );

        assert_eq!(profile.tool_family, ToolBoundaryFamily::File);
        assert_eq!(profile.paths[0].class, WorkspacePathClass::External);
        assert!(profile.has_external_or_sensitive_path());
        assert!(profile
            .boundary_warnings()
            .iter()
            .any(|warning| warning.contains("OUTSIDE_WORKSPACE")));
    }

    #[test]
    fn classifies_mcp_and_plugin_boundaries_with_shared_vocabulary() {
        let mcp = ActionSideEffectProfile::from_tool_call(
            &call(
                "mcp_tool",
                json!({"server_name": "github", "tool_name": "create_issue"}),
            ),
            None,
            Path::new("/repo"),
        );
        assert_eq!(mcp.tool_family, ToolBoundaryFamily::Mcp);
        assert_eq!(mcp.network.class, NetworkAccessClass::RemoteService);
        assert_eq!(
            mcp.external_side_effect,
            ExternalSideEffect::PluginOrMcpUnknown
        );

        let plugin = ActionSideEffectProfile::from_tool_call(
            &call(
                "plugin_manage",
                json!({"action": "run", "plugin_id": "demo"}),
            ),
            None,
            Path::new("/repo"),
        );
        assert_eq!(plugin.tool_family, ToolBoundaryFamily::Plugin);
        assert_eq!(
            plugin.external_side_effect,
            ExternalSideEffect::PluginOrMcpUnknown
        );
        assert!(plugin
            .boundary_warnings()
            .iter()
            .any(|warning| warning.contains("PLUGIN_OR_MCP_BOUNDARY")));
    }
}
