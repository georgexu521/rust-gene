use super::*;

pub(super) fn collect_desktop_diagnostics(
    selected_project: &Path,
    settings_path: &Path,
    diagnostic_logs_path: &Path,
) -> Vec<DesktopDiagnostic> {
    vec![
        provider_key_diagnostic(),
        shell_diagnostic(),
        command_diagnostic("git", "Git command", "git"),
        command_diagnostic("cargo", "Rust toolchain", "cargo"),
        command_diagnostic("corepack", "Node package manager bridge", "corepack"),
        xcode_tools_diagnostic(),
        project_access_diagnostic(selected_project),
        settings_access_diagnostic(settings_path),
        diagnostic_logs_access_diagnostic(diagnostic_logs_path),
    ]
}

pub(super) fn provider_setup_info_value() -> ProviderSetupInfo {
    ProviderSetupInfo {
        shell_profile_path: shell_profile_path().display().to_string(),
        provider_env_vars: priority_agent::services::api::provider::DEFAULT_PROVIDER_ENV_SPECS
            .iter()
            .flat_map(|spec| spec.key_env_vars.iter().copied())
            .collect(),
        example: "export MINIMAX_API_KEY=\"your-key-here\"",
    }
}

pub(super) fn provider_key_diagnostic() -> DesktopDiagnostic {
    let configured: Vec<&str> = priority_agent::services::api::provider::DEFAULT_PROVIDER_ENV_SPECS
        .iter()
        .filter_map(|spec| {
            spec.key_env_vars
                .iter()
                .any(|env| env_is_set(env))
                .then_some(spec.label)
        })
        .collect();

    if configured.is_empty() {
        DesktopDiagnostic {
            id: "provider_keys",
            label: "Provider keys",
            status: DiagnosticStatus::Error,
            detail: format!(
                "No provider key found. Set one of {} before running real agent sessions.",
                priority_agent::services::api::provider::provider_key_env_hint()
            ),
        }
    } else {
        DesktopDiagnostic {
            id: "provider_keys",
            label: "Provider keys",
            status: DiagnosticStatus::Ok,
            detail: format!("Configured providers: {}", configured.join(", ")),
        }
    }
}

pub(super) fn command_diagnostic(
    id: &'static str,
    label: &'static str,
    command: &str,
) -> DesktopDiagnostic {
    if command_available(command) {
        DesktopDiagnostic {
            id,
            label,
            status: DiagnosticStatus::Ok,
            detail: format!("`{command}` is available on PATH."),
        }
    } else {
        DesktopDiagnostic {
            id,
            label,
            status: DiagnosticStatus::Warning,
            detail: format!("`{command}` was not found on PATH."),
        }
    }
}

pub(super) fn shell_diagnostic() -> DesktopDiagnostic {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    if Path::new(&shell).exists() {
        DesktopDiagnostic {
            id: "shell",
            label: "Shell",
            status: DiagnosticStatus::Ok,
            detail: format!("Using shell: {shell}"),
        }
    } else {
        DesktopDiagnostic {
            id: "shell",
            label: "Shell",
            status: DiagnosticStatus::Warning,
            detail: format!("Configured shell does not exist: {shell}"),
        }
    }
}

pub(super) fn shell_profile_path() -> PathBuf {
    let home = std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    if shell.ends_with("bash") {
        home.join(".bash_profile")
    } else {
        home.join(".zshrc")
    }
}

pub(super) fn xcode_tools_diagnostic() -> DesktopDiagnostic {
    match Command::new("xcode-select").arg("-p").output() {
        Ok(output) if output.status.success() => DesktopDiagnostic {
            id: "xcode_select",
            label: "Xcode command line tools",
            status: DiagnosticStatus::Ok,
            detail: format!(
                "Developer tools path: {}",
                String::from_utf8_lossy(&output.stdout).trim()
            ),
        },
        _ => DesktopDiagnostic {
            id: "xcode_select",
            label: "Xcode command line tools",
            status: DiagnosticStatus::Warning,
            detail: "Xcode command line tools are not configured; run `xcode-select --install` if builds fail.".to_string(),
        },
    }
}

pub(super) fn project_access_diagnostic(project: &Path) -> DesktopDiagnostic {
    if !project.exists() {
        return DesktopDiagnostic {
            id: "project_access",
            label: "Project access",
            status: DiagnosticStatus::Error,
            detail: format!("Project path does not exist: {}", project.display()),
        };
    }
    if !project.is_dir() {
        return DesktopDiagnostic {
            id: "project_access",
            label: "Project access",
            status: DiagnosticStatus::Error,
            detail: format!("Project path is not a directory: {}", project.display()),
        };
    }
    if std::fs::read_dir(project).is_err() {
        return DesktopDiagnostic {
            id: "project_access",
            label: "Project access",
            status: DiagnosticStatus::Error,
            detail: format!("Project path is not readable: {}", project.display()),
        };
    }

    if directory_writable(project) {
        DesktopDiagnostic {
            id: "project_access",
            label: "Project access",
            status: DiagnosticStatus::Ok,
            detail: format!(
                "Project path is readable and writable: {}",
                project.display()
            ),
        }
    } else {
        DesktopDiagnostic {
            id: "project_access",
            label: "Project access",
            status: DiagnosticStatus::Warning,
            detail: format!(
                "Project path is readable but may not be writable: {}",
                project.display()
            ),
        }
    }
}

pub(super) fn settings_access_diagnostic(settings_path: &Path) -> DesktopDiagnostic {
    let Some(parent) = settings_path.parent() else {
        return DesktopDiagnostic {
            id: "settings_access",
            label: "Settings storage",
            status: DiagnosticStatus::Error,
            detail: format!(
                "Settings path has no parent directory: {}",
                settings_path.display()
            ),
        };
    };

    if directory_writable(parent)
        || std::fs::create_dir_all(parent).is_ok() && directory_writable(parent)
    {
        DesktopDiagnostic {
            id: "settings_access",
            label: "Settings storage",
            status: DiagnosticStatus::Ok,
            detail: format!("Settings can be stored at {}", settings_path.display()),
        }
    } else {
        DesktopDiagnostic {
            id: "settings_access",
            label: "Settings storage",
            status: DiagnosticStatus::Error,
            detail: format!("Settings directory is not writable: {}", parent.display()),
        }
    }
}

pub(super) fn diagnostic_logs_access_diagnostic(log_path: &Path) -> DesktopDiagnostic {
    let Some(parent) = log_path.parent() else {
        return DesktopDiagnostic {
            id: "diagnostic_logs",
            label: "Diagnostic logs",
            status: DiagnosticStatus::Error,
            detail: format!(
                "Diagnostic log path has no parent directory: {}",
                log_path.display()
            ),
        };
    };

    if directory_writable(parent)
        || std::fs::create_dir_all(parent).is_ok() && directory_writable(parent)
    {
        DesktopDiagnostic {
            id: "diagnostic_logs",
            label: "Diagnostic logs",
            status: DiagnosticStatus::Ok,
            detail: format!("Desktop logs can be written at {}", log_path.display()),
        }
    } else {
        DesktopDiagnostic {
            id: "diagnostic_logs",
            label: "Diagnostic logs",
            status: DiagnosticStatus::Warning,
            detail: format!(
                "Diagnostic log directory is not writable: {}",
                parent.display()
            ),
        }
    }
}

pub(super) fn env_is_set(name: &str) -> bool {
    std::env::var(name)
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

pub(super) fn command_available(command: &str) -> bool {
    Command::new("/bin/sh")
        .arg("-lc")
        .arg(format!("command -v {command} >/dev/null 2>&1"))
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

pub(super) fn directory_writable(path: &Path) -> bool {
    let test_path = path.join(format!(".priority-agent-write-test-{}", std::process::id()));
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&test_path)
    {
        Ok(_) => {
            let _ = std::fs::remove_file(test_path);
            true
        }
        Err(_) => false,
    }
}

pub(super) fn open_path(path: &Path) -> Result<(), String> {
    Command::new("open")
        .arg(path)
        .status()
        .map_err(|err| err.to_string())
        .and_then(|status| {
            status
                .success()
                .then_some(())
                .ok_or_else(|| format!("failed to open {}", path.display()))
        })
}
