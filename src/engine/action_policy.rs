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
            "external_effect={:?} network={:?} paths={}",
            external_side_effect,
            network.class,
            paths.len()
        );

        Self {
            schema: "action_side_effect_profile.v1".to_string(),
            paths,
            network,
            external_side_effect,
            mutates_local_workspace,
            mutates_local_machine,
            remote_side_effect,
            summary,
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
        let workspace = normalize_existing_or_logical(working_dir);
        let inside_workspace = normalized.starts_with(&workspace);
        let class = if path.trim().is_empty() {
            WorkspacePathClass::Unknown
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
            WorkspacePathClass::Workspace => "path is inside the working directory",
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
            "mcp_tool" => Self {
                class: NetworkAccessClass::RemoteService,
                target: tool_call.arguments["server_name"]
                    .as_str()
                    .map(str::to_string),
                trusted: false,
                reason: "MCP tool may execute outside the local runtime".to_string(),
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
            "plugin" | "mcp_tool" => Self::PluginOrMcpUnknown,
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

    if tool_call.name == "bash" {
        if let Some(command) = tool_call.arguments["command"].as_str() {
            let classification =
                crate::tools::bash_tool::command_classifier::classify_command(command);
            paths.extend(classification.absolute_path_patterns);
            paths.extend(classification.mutation_paths);
            paths.extend(classification.path_patterns);
        }
    }

    paths.sort();
    paths.dedup();
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
}
