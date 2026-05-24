//! Structured dependency-install facade.

use crate::tools::{Tool, ToolContext, ToolErrorCode, ToolOperationKind, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Instant;
use tokio::process::Command;
use tokio::time::{timeout, Duration};

pub struct InstallDependenciesTool;

#[derive(Debug, Clone, PartialEq, Eq)]
struct InstallCommand {
    program: String,
    args: Vec<String>,
    display: String,
    manifest_path: Option<String>,
    lockfile_policy: String,
}

#[async_trait]
impl Tool for InstallDependenciesTool {
    fn name(&self) -> &str {
        "install_dependencies"
    }

    fn description(&self) -> &str {
        "Install project dependencies through a structured package-manager contract. Generates safe package-manager argv from explicit parameters and requires approval because it can download and execute external content."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "manager": {
                    "type": "string",
                    "enum": ["npm", "pnpm", "yarn", "python_pip", "uv_pip", "cargo", "go"],
                    "description": "Package manager to use."
                },
                "action": {
                    "type": "string",
                    "enum": ["install_project", "add_packages"],
                    "default": "install_project",
                    "description": "Install project dependencies, or add explicit packages when supported."
                },
                "packages": {
                    "type": "array",
                    "items": { "type": "string", "minLength": 1 },
                    "description": "Packages to add/install for add_packages actions."
                },
                "dev_dependency": {
                    "type": "boolean",
                    "default": false,
                    "description": "Install packages as development dependencies for JS package managers."
                },
                "requirements_file": {
                    "type": "string",
                    "description": "Requirements file for python_pip or uv_pip install_project."
                },
                "working_dir": {
                    "type": "string",
                    "description": "Optional working directory inside the project."
                },
                "frozen": {
                    "type": "boolean",
                    "default": true,
                    "description": "Prefer lockfile-respecting install commands for project installs."
                },
                "timeout_secs": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 3600,
                    "default": 600,
                    "description": "Maximum runtime in seconds."
                }
            },
            "required": ["manager"],
            "additionalProperties": false
        })
    }

    fn requires_confirmation(&self, _params: &serde_json::Value) -> bool {
        true
    }

    fn operation_kind(&self, _params: &serde_json::Value) -> ToolOperationKind {
        ToolOperationKind::Network
    }

    fn is_concurrency_safe(&self, _params: &serde_json::Value) -> bool {
        false
    }

    fn is_open_world(&self, _params: &serde_json::Value) -> bool {
        true
    }

    fn strict_schema(&self) -> bool {
        true
    }

    fn max_result_size_chars(&self) -> Option<usize> {
        Some(12_000)
    }

    fn search_hint(&self) -> Option<&'static str> {
        Some("install project dependencies package manager")
    }

    fn input_paths(&self, params: &serde_json::Value) -> Vec<String> {
        let mut paths = Vec::new();
        if let Some(working_dir) = params["working_dir"]
            .as_str()
            .filter(|path| !path.trim().is_empty())
        {
            paths.push(working_dir.to_string());
        }
        if let Some(requirements_file) = params["requirements_file"]
            .as_str()
            .filter(|path| !path.trim().is_empty())
        {
            paths.push(requirements_file.to_string());
        }
        paths
    }

    fn permission_matcher_input(&self, params: &serde_json::Value) -> Option<String> {
        build_install_command(params)
            .ok()
            .map(|command| command.display)
    }

    fn to_classifier_input(&self, params: &serde_json::Value) -> String {
        build_install_command(params)
            .map(|command| format!("install_dependencies: {}", command.display))
            .unwrap_or_else(|_| "install_dependencies: invalid".to_string())
    }

    fn tool_use_summary(&self, params: &serde_json::Value) -> Option<String> {
        build_install_command(params)
            .ok()
            .map(|command| command.display)
    }

    async fn execute(&self, params: serde_json::Value, context: ToolContext) -> ToolResult {
        let install = match build_install_command(&params) {
            Ok(command) => command,
            Err(reason) => return invalid_install_request(reason),
        };
        let working_dir = match install_working_dir(&params, &context) {
            Ok(path) => path,
            Err(reason) => return invalid_install_request(reason),
        };
        let timeout_secs = params["timeout_secs"]
            .as_u64()
            .unwrap_or(600)
            .clamp(1, 3600);

        let started = Instant::now();
        let mut command = Command::new(&install.program);
        command
            .args(&install.args)
            .current_dir(&working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let output = timeout(Duration::from_secs(timeout_secs), command.output()).await;
        match output {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let content =
                    install_output(&install.display, output.status.code(), &stdout, &stderr);
                let data = install_metadata(
                    &install,
                    &working_dir,
                    InstallExecutionMetadata {
                        timeout_secs,
                        timed_out: false,
                        exit_code: output.status.code(),
                        stdout_bytes: output.stdout.len(),
                        stderr_bytes: output.stderr.len(),
                        duration_ms: started.elapsed().as_millis() as u64,
                    },
                );
                if output.status.success() {
                    ToolResult::success_with_data(content, data)
                } else {
                    let mut result = ToolResult::error(content);
                    result.error_code = Some(ToolErrorCode::ExecutionFailed);
                    result.data = Some(data);
                    result
                }
            }
            Ok(Err(error)) => {
                let mut result = ToolResult::error(format!(
                    "Failed to run dependency install command `{}`: {error}",
                    install.display
                ));
                result.error_code = Some(ToolErrorCode::ExecutionFailed);
                result.data = Some(install_metadata(
                    &install,
                    &working_dir,
                    InstallExecutionMetadata {
                        timeout_secs,
                        timed_out: false,
                        exit_code: None,
                        stdout_bytes: 0,
                        stderr_bytes: 0,
                        duration_ms: started.elapsed().as_millis() as u64,
                    },
                ));
                result
            }
            Err(_) => {
                let mut result = ToolResult::error(format!(
                    "Dependency install command timed out after {timeout_secs}s: {}",
                    install.display
                ));
                result.error_code = Some(ToolErrorCode::Timeout);
                result.data = Some(install_metadata(
                    &install,
                    &working_dir,
                    InstallExecutionMetadata {
                        timeout_secs,
                        timed_out: true,
                        exit_code: None,
                        stdout_bytes: 0,
                        stderr_bytes: 0,
                        duration_ms: started.elapsed().as_millis() as u64,
                    },
                ));
                result
            }
        }
    }
}

fn build_install_command(params: &serde_json::Value) -> Result<InstallCommand, String> {
    let manager = params["manager"]
        .as_str()
        .ok_or_else(|| "manager is required".to_string())?;
    let action = params["action"].as_str().unwrap_or("install_project");
    let packages = params["packages"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    validate_package_names(&packages)?;
    let dev_dependency = params["dev_dependency"].as_bool().unwrap_or(false);
    let frozen = params["frozen"].as_bool().unwrap_or(true);
    let requirements_file = params["requirements_file"]
        .as_str()
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .unwrap_or("requirements.txt");

    match (manager, action) {
        ("npm", "install_project") => {
            let args = if frozen && packages.is_empty() {
                vec!["ci".to_string()]
            } else {
                vec!["install".to_string()]
            };
            Ok(command(
                "npm",
                args,
                Some("package.json"),
                lock_policy(frozen),
            ))
        }
        ("npm", "add_packages") => {
            require_packages(&packages)?;
            let mut args = vec!["install".to_string()];
            if dev_dependency {
                args.push("--save-dev".to_string());
            }
            args.extend(packages);
            Ok(command(
                "npm",
                args,
                Some("package.json"),
                "updates_manifest",
            ))
        }
        ("pnpm", "install_project") => {
            let mut args = vec!["install".to_string()];
            if frozen {
                args.push("--frozen-lockfile".to_string());
            }
            Ok(command(
                "pnpm",
                args,
                Some("package.json"),
                lock_policy(frozen),
            ))
        }
        ("pnpm", "add_packages") => {
            require_packages(&packages)?;
            let mut args = vec!["add".to_string()];
            if dev_dependency {
                args.push("-D".to_string());
            }
            args.extend(packages);
            Ok(command(
                "pnpm",
                args,
                Some("package.json"),
                "updates_manifest",
            ))
        }
        ("yarn", "install_project") => {
            let mut args = vec!["install".to_string()];
            if frozen {
                args.push("--frozen-lockfile".to_string());
            }
            Ok(command(
                "yarn",
                args,
                Some("package.json"),
                lock_policy(frozen),
            ))
        }
        ("yarn", "add_packages") => {
            require_packages(&packages)?;
            let mut args = vec!["add".to_string()];
            if dev_dependency {
                args.push("--dev".to_string());
            }
            args.extend(packages);
            Ok(command(
                "yarn",
                args,
                Some("package.json"),
                "updates_manifest",
            ))
        }
        ("python_pip", "install_project") => {
            validate_relative_path(requirements_file)?;
            Ok(command(
                "python3",
                vec![
                    "-m".to_string(),
                    "pip".to_string(),
                    "install".to_string(),
                    "-r".to_string(),
                    requirements_file.to_string(),
                ],
                Some(requirements_file),
                "requirements_file",
            ))
        }
        ("python_pip", "add_packages") => {
            require_packages(&packages)?;
            let mut args = vec!["-m".to_string(), "pip".to_string(), "install".to_string()];
            args.extend(packages);
            Ok(command("python3", args, None, "direct_packages"))
        }
        ("uv_pip", "install_project") => {
            validate_relative_path(requirements_file)?;
            Ok(command(
                "uv",
                vec![
                    "pip".to_string(),
                    "install".to_string(),
                    "-r".to_string(),
                    requirements_file.to_string(),
                ],
                Some(requirements_file),
                "requirements_file",
            ))
        }
        ("uv_pip", "add_packages") => {
            require_packages(&packages)?;
            let mut args = vec!["pip".to_string(), "install".to_string()];
            args.extend(packages);
            Ok(command("uv", args, None, "direct_packages"))
        }
        ("cargo", "install_project") => Ok(command(
            "cargo",
            vec!["fetch".to_string()],
            Some("Cargo.toml"),
            "lockfile_respecting",
        )),
        ("cargo", "add_packages") => {
            Err("cargo add mutates Cargo.toml; edit the manifest intentionally instead".to_string())
        }
        ("go", "install_project") => Ok(command(
            "go",
            vec!["mod".to_string(), "download".to_string()],
            Some("go.mod"),
            "lockfile_respecting",
        )),
        ("go", "add_packages") => {
            Err("go get mutates go.mod; edit the manifest intentionally instead".to_string())
        }
        (_, _) => Err(format!(
            "unsupported dependency install request: {manager}/{action}"
        )),
    }
}

fn command(
    program: &str,
    args: Vec<String>,
    manifest_path: Option<&str>,
    lockfile_policy: &str,
) -> InstallCommand {
    let display = std::iter::once(program.to_string())
        .chain(args.iter().map(|arg| shell_word(arg)))
        .collect::<Vec<_>>()
        .join(" ");
    InstallCommand {
        program: program.to_string(),
        args,
        display,
        manifest_path: manifest_path.map(str::to_string),
        lockfile_policy: lockfile_policy.to_string(),
    }
}

fn lock_policy(frozen: bool) -> &'static str {
    if frozen {
        "lockfile_required"
    } else {
        "lockfile_preferred"
    }
}

fn require_packages(packages: &[String]) -> Result<(), String> {
    if packages.is_empty() {
        Err("packages are required for add_packages".to_string())
    } else {
        Ok(())
    }
}

fn validate_package_names(packages: &[String]) -> Result<(), String> {
    for package in packages {
        if package.trim().is_empty() {
            return Err("package names cannot be empty".to_string());
        }
        if package.starts_with('-')
            || package.contains(';')
            || package.contains('|')
            || package.contains('&')
            || package.contains('`')
            || package.contains('$')
            || package.contains('<')
            || package.contains('>')
            || package.contains('\n')
            || package.contains('\r')
        {
            return Err(format!("unsafe package name: {package}"));
        }
    }
    Ok(())
}

fn validate_relative_path(path: &str) -> Result<(), String> {
    let path = std::path::Path::new(path);
    if path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err("dependency manifest paths must be relative workspace paths".to_string());
    }
    Ok(())
}

fn install_working_dir(
    params: &serde_json::Value,
    context: &ToolContext,
) -> Result<PathBuf, String> {
    let Some(raw) = params["working_dir"]
        .as_str()
        .map(str::trim)
        .filter(|path| !path.is_empty())
    else {
        return Ok(context.working_dir.clone());
    };
    validate_relative_path(raw)?;
    Ok(context.working_dir.join(raw))
}

fn invalid_install_request(reason: String) -> ToolResult {
    let mut result = ToolResult::error(format!("Invalid dependency install request: {reason}"));
    result.error_code = Some(ToolErrorCode::InvalidParams);
    result.data = Some(json!({
        "tool": "install_dependencies",
        "failure": "invalid_dependency_install_request",
        "reason": reason,
    }));
    result
}

struct InstallExecutionMetadata {
    timeout_secs: u64,
    timed_out: bool,
    exit_code: Option<i32>,
    stdout_bytes: usize,
    stderr_bytes: usize,
    duration_ms: u64,
}

fn install_metadata(
    install: &InstallCommand,
    working_dir: &std::path::Path,
    execution: InstallExecutionMetadata,
) -> serde_json::Value {
    let classification =
        crate::tools::bash_tool::command_classifier::classify_command(&install.display);
    json!({
        "tool": "install_dependencies",
        "dependency_install": {
            "schema": "dependency_install.v1",
            "command": install.display,
            "manager": install.program,
            "args": install.args,
            "manifest_path": install.manifest_path,
            "lockfile_policy": install.lockfile_policy,
            "network_class": "package_install",
            "requires_approval": true,
        },
        "shell_result": {
            "command": install.display,
            "cwd": working_dir.display().to_string(),
            "exit_code": execution.exit_code,
            "stdout_bytes": execution.stdout_bytes,
            "stderr_bytes": execution.stderr_bytes,
            "timed_out": execution.timed_out,
            "duration_ms": execution.duration_ms,
            "timeout_secs": execution.timeout_secs,
        },
        "command_classification": classification,
    })
}

fn install_output(command: &str, exit_code: Option<i32>, stdout: &str, stderr: &str) -> String {
    let mut out = format!(
        "Dependency install command `{}` exited with {}",
        command,
        exit_code
            .map(|code| code.to_string())
            .unwrap_or_else(|| "unknown".to_string())
    );
    if !stdout.trim().is_empty() {
        out.push_str("\n\nstdout:\n");
        out.push_str(stdout.trim_end());
    }
    if !stderr.trim().is_empty() {
        out.push_str("\n\nstderr:\n");
        out.push_str(stderr.trim_end());
    }
    out
}

fn shell_word(value: &str) -> String {
    if value.chars().all(|ch| {
        ch == '_'
            || ch == '-'
            || ch == '.'
            || ch == '/'
            || ch == '@'
            || ch == ':'
            || ch.is_ascii_alphanumeric()
    }) {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\"'\"'"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_lockfile_respecting_npm_project_install() {
        let command = build_install_command(&json!({"manager": "npm"})).unwrap();

        assert_eq!(command.program, "npm");
        assert_eq!(command.args, vec!["ci"]);
        assert_eq!(command.manifest_path.as_deref(), Some("package.json"));
        assert_eq!(command.lockfile_policy, "lockfile_required");
    }

    #[test]
    fn builds_structured_package_add_without_shell_interpolation() {
        let command = build_install_command(&json!({
            "manager": "pnpm",
            "action": "add_packages",
            "packages": ["@types/node", "vite"],
            "dev_dependency": true
        }))
        .unwrap();

        assert_eq!(command.program, "pnpm");
        assert_eq!(command.args, vec!["add", "-D", "@types/node", "vite"]);
        assert_eq!(command.display, "pnpm add -D @types/node vite");
    }

    #[test]
    fn rejects_unsafe_package_names() {
        let err = build_install_command(&json!({
            "manager": "npm",
            "action": "add_packages",
            "packages": ["left-pad; rm -rf ."]
        }))
        .expect_err("unsafe package should fail");

        assert!(err.contains("unsafe package name"));
    }

    #[test]
    fn cargo_add_is_rejected_to_avoid_manifest_mutation() {
        let err = build_install_command(&json!({
            "manager": "cargo",
            "action": "add_packages",
            "packages": ["serde"]
        }))
        .expect_err("cargo add should fail");

        assert!(err.contains("mutates Cargo.toml"));
    }

    #[tokio::test]
    async fn execute_rejects_invalid_request_before_running_network_command() {
        let result = InstallDependenciesTool
            .execute(
                json!({
                    "manager": "go",
                    "action": "add_packages",
                    "packages": ["example.com/mod"]
                }),
                ToolContext::new(".", "test-install-dependencies"),
            )
            .await;

        assert!(!result.success);
        assert_eq!(result.error_code, Some(ToolErrorCode::InvalidParams));
        assert_eq!(
            result.data.unwrap()["failure"],
            "invalid_dependency_install_request"
        );
    }
}
