use super::*;

pub(super) fn handle_daemon_command(project_root: &Path, store: &LabStore, args: &str) -> String {
    let (action, rest) = split_once(args);
    match action {
        "" | "status" => match store.load_daemon_state() {
            Ok(Some(state)) => [
                format!("Lab daemon policy: {}", state.project_root),
                format!("Enabled: {}", state.enabled),
                format!("Mode: {:?}", state.mode),
                format!("Max steps: {}", state.max_steps),
                format!("Max steps per cycle: {}", state.max_steps_per_cycle),
                format!("Interval ms: {}", state.interval_ms),
                format!(
                    "Instructions: {}",
                    if state.instructions.is_empty() {
                        "none"
                    } else {
                        state.instructions.as_str()
                    }
                ),
                format!(
                    "Last enabled: {}",
                    state
                        .last_enabled_at
                        .map(|time| time.to_rfc3339())
                        .unwrap_or_else(|| "none".to_string())
                ),
                format!(
                    "Last disabled: {}",
                    state
                        .last_disabled_at
                        .map(|time| time.to_rfc3339())
                        .unwrap_or_else(|| "none".to_string())
                ),
                format!(
                    "Last started: {}",
                    state
                        .last_started_at
                        .map(|time| time.to_rfc3339())
                        .unwrap_or_else(|| "none".to_string())
                ),
                format!(
                    "Last started LabRun: {}",
                    state
                        .last_started_lab_run_id
                        .as_deref()
                        .unwrap_or("none")
                ),
                format!(
                    "Last start error: {}",
                    state.last_start_error.as_deref().unwrap_or("none")
                ),
                format!(
                    "Last message: {}",
                    state.last_message.unwrap_or_else(|| "none".to_string())
                ),
            ]
            .join("\n"),
            Ok(None) => "No Lab daemon policy found.".to_string(),
            Err(err) => format!("Failed to read Lab daemon policy: {err}"),
        },
        "enable" => {
            let (mode, max_steps, max_steps_per_cycle, interval_ms, instructions) =
                match parse_daemon_enable_args(rest) {
                Ok(parsed) => parsed,
                Err(message) => return message,
            };
            match store.enable_daemon_with_cycle_bound(
                mode,
                max_steps,
                max_steps_per_cycle,
                interval_ms,
                instructions,
            ) {
                Ok(state) => format!(
                    "Enabled Lab daemon policy.\nMode: {:?}\nMax steps: {}\nMax steps per cycle: {}\nInterval ms: {}\nInstructions: {}",
                    state.mode,
                    state.max_steps,
                    state.max_steps_per_cycle,
                    state.interval_ms,
                    if state.instructions.is_empty() {
                        "none"
                    } else {
                        state.instructions.as_str()
                    }
                ),
                Err(err) => format!("Failed to enable Lab daemon policy: {err}"),
            }
        }
        "start" => {
            "Use /lab daemon start from the interactive shell so the daemon can access the active provider and ToolContext."
                .to_string()
        }
        "health" => handle_daemon_health_command(project_root, store),
        "launchd" => handle_daemon_launchd_command(store, rest),
        "service" => handle_daemon_service_command(store, rest),
        "disable" => match store.disable_daemon(rest) {
            Ok(state) => format!(
                "Disabled Lab daemon policy.\nLast message: {}",
                state.last_message.unwrap_or_else(|| "none".to_string())
            ),
            Err(err) => format!("Failed to disable Lab daemon policy: {err}"),
        },
        _ => {
            "Usage: /lab daemon [status|health|enable [strict|hybrid|hybrid-cycles] [max_steps] [max_steps_per_cycle] [interval_ms] [instructions]|start|launchd [label]|service [status|install|uninstall|load|unload|restart|supervise|commands] [label]|disable [reason]]"
                .to_string()
        }
    }
}

fn handle_daemon_health_command(project_root: &Path, store: &LabStore) -> String {
    let daemon = match store.load_daemon_state() {
        Ok(Some(state)) => state,
        Ok(None) => return "Lab daemon health: no_policy\nNo Lab daemon policy found.".to_string(),
        Err(err) => return format!("Failed to read Lab daemon health: {err}"),
    };
    let scheduler_status = background_scheduler_status(project_root).ok();
    let persisted_scheduler = scheduler_status
        .as_ref()
        .and_then(|status| status.persisted.as_ref());
    let scheduler_label = persisted_scheduler
        .map(|state| format!("{:?}", state.status))
        .unwrap_or_else(|| "none".to_string());
    let running_in_process = scheduler_status
        .as_ref()
        .map(|status| status.running_in_process)
        .unwrap_or(false);
    let health = if !daemon.enabled {
        "disabled"
    } else if daemon.last_start_error.is_some() {
        "unhealthy_start_error"
    } else if running_in_process {
        "running_in_process"
    } else if let Some(state) = persisted_scheduler {
        match state.status {
            crate::lab::model::LabSchedulerStatus::Running => "running_persisted",
            crate::lab::model::LabSchedulerStatus::Stopping => "stopping",
            crate::lab::model::LabSchedulerStatus::PausedRestart => "paused_restart",
            crate::lab::model::LabSchedulerStatus::Blocked => "attention_blocked",
            crate::lab::model::LabSchedulerStatus::NeedsUser => "needs_user",
            crate::lab::model::LabSchedulerStatus::Failed => "unhealthy_failed",
            crate::lab::model::LabSchedulerStatus::Completed => "completed",
            crate::lab::model::LabSchedulerStatus::Stopped => "stopped",
            crate::lab::model::LabSchedulerStatus::Idle => "idle",
        }
    } else if daemon.last_started_at.is_none() {
        "enabled_not_started"
    } else {
        "enabled_no_scheduler_state"
    };
    let lifecycle = match store.load_app_lifecycle_state() {
        Ok(Some(state)) => state
            .last_message
            .unwrap_or_else(|| "lifecycle checkpoint recorded".to_string()),
        Ok(None) => "none".to_string(),
        Err(err) => format!("unavailable ({err})"),
    };
    let launchd_label = default_launchd_label(store);
    let launchd_plist = store.root().join("launchd").join(format!(
        "{}.plist",
        safe_launchd_label_component(&launchd_label)
    ));
    [
        format!("Lab daemon health: {health}"),
        format!(
            "Policy: enabled={} mode={:?} max_steps={} max_steps_per_cycle={} interval_ms={}",
            daemon.enabled,
            daemon.mode,
            daemon.max_steps,
            daemon.max_steps_per_cycle,
            daemon.interval_ms
        ),
        format!("Scheduler: running_in_process={running_in_process} persisted={scheduler_label}"),
        format!(
            "Last started: {}",
            daemon
                .last_started_at
                .map(|time| time.to_rfc3339())
                .unwrap_or_else(|| "none".to_string())
        ),
        format!(
            "Last started LabRun: {}",
            daemon.last_started_lab_run_id.as_deref().unwrap_or("none")
        ),
        format!(
            "Last start error: {}",
            daemon.last_start_error.as_deref().unwrap_or("none")
        ),
        format!(
            "Last message: {}",
            daemon.last_message.as_deref().unwrap_or("none")
        ),
        format!("Lifecycle: {lifecycle}"),
        format!("LaunchAgent plist: {}", launchd_plist.display()),
        format!("LaunchAgent exists: {}", launchd_plist.exists()),
    ]
    .join("\n")
}

fn handle_daemon_launchd_command(store: &LabStore, args: &str) -> String {
    let label = if args.trim().is_empty() {
        default_launchd_label(store)
    } else {
        safe_launchd_label_component(args.trim())
    };
    match write_daemon_launchd_plist(store, &label) {
        Ok(path) => format!(
            "Wrote Lab daemon LaunchAgent plist.\nLabel: {}\nPlist: {}\nInstall hint: launchctl bootstrap gui/$(id -u) {}\nRun hint: launchctl kickstart -k gui/$(id -u)/{}",
            label,
            path.display(),
            path.display(),
            label
        ),
        Err(err) => format!("Failed to write Lab daemon LaunchAgent plist: {err}"),
    }
}

fn handle_daemon_service_command(store: &LabStore, args: &str) -> String {
    let (action, rest) = split_once(args.trim());
    let action = if action.is_empty() { "status" } else { action };
    let label = if rest.trim().is_empty() {
        default_launchd_label(store)
    } else {
        safe_launchd_label_component(rest.trim())
    };

    match action {
        "status" => daemon_service_status(store, &label),
        "commands" => daemon_service_commands(store, &label),
        "install" => match install_daemon_service_plist(store, &label) {
            Ok(paths) => format!(
                "Installed Lab daemon LaunchAgent plist.\n{}",
                daemon_service_lines(
                    store,
                    &label,
                    &paths.generated_plist,
                    &paths.installed_plist
                )
                .join("\n")
            ),
            Err(err) => format!("Failed to install Lab daemon service plist: {err}"),
        },
        "uninstall" => match uninstall_daemon_service_plist(&label) {
            Ok(removed) => {
                let paths = daemon_service_paths(store, &label);
                format!(
                    "Uninstalled Lab daemon LaunchAgent plist.\nRemoved: {}\n{}",
                    removed,
                    daemon_service_lines(
                        store,
                        &label,
                        &paths.generated_plist,
                        &paths.installed_plist
                    )
                    .join("\n")
                )
            }
            Err(err) => format!("Failed to uninstall Lab daemon service plist: {err}"),
        },
        "load" => match load_daemon_service(store, &label) {
            Ok(result) => format!("Loaded Lab daemon service.\n{}", result.format()),
            Err(err) => format!("Failed to load Lab daemon service: {err}"),
        },
        "unload" => match unload_daemon_service(&label) {
            Ok(result) => format!("Unloaded Lab daemon service.\n{}", result.format()),
            Err(err) => format!("Failed to unload Lab daemon service: {err}"),
        },
        "restart" | "kickstart" => match restart_daemon_service(&label) {
            Ok(result) => format!("Restarted Lab daemon service.\n{}", result.format()),
            Err(err) => format!("Failed to restart Lab daemon service: {err}"),
        },
        "supervise" => match supervise_daemon_service(store, &label) {
            Ok(report) => report,
            Err(err) => format!("Failed to supervise Lab daemon service: {err}"),
        },
        _ => "Usage: /lab daemon service [status|install|uninstall|load|unload|restart|supervise|commands] [label]".to_string(),
    }
}

struct DaemonServicePaths {
    generated_plist: PathBuf,
    installed_plist: PathBuf,
}

fn daemon_service_status(store: &LabStore, label: &str) -> String {
    let paths = daemon_service_paths(store, label);
    [
        vec!["Lab daemon service status.".to_string()],
        daemon_service_lines(store, label, &paths.generated_plist, &paths.installed_plist),
    ]
    .concat()
    .join("\n")
}

fn daemon_service_commands(store: &LabStore, label: &str) -> String {
    let paths = daemon_service_paths(store, label);
    daemon_service_lines(store, label, &paths.generated_plist, &paths.installed_plist).join("\n")
}

fn install_daemon_service_plist(
    store: &LabStore,
    label: &str,
) -> anyhow::Result<DaemonServicePaths> {
    let generated_plist = write_daemon_launchd_plist(store, label)?;
    let installed_plist = launch_agent_install_path(label)?;
    if let Some(parent) = installed_plist.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(&generated_plist, &installed_plist)?;
    Ok(DaemonServicePaths {
        generated_plist,
        installed_plist,
    })
}

fn uninstall_daemon_service_plist(label: &str) -> anyhow::Result<bool> {
    let installed_plist = launch_agent_install_path(label)?;
    if installed_plist.exists() {
        fs::remove_file(installed_plist)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

fn load_daemon_service(store: &LabStore, label: &str) -> anyhow::Result<LaunchctlResult> {
    let paths = install_daemon_service_plist(store, label)?;
    let domain = launchctl_gui_domain()?;
    run_launchctl(&[
        "bootstrap".to_string(),
        domain,
        paths.installed_plist.display().to_string(),
    ])
}

fn unload_daemon_service(label: &str) -> anyhow::Result<LaunchctlResult> {
    let target = launchctl_label_target(label)?;
    run_launchctl(&["bootout".to_string(), target])
}

fn restart_daemon_service(label: &str) -> anyhow::Result<LaunchctlResult> {
    let target = launchctl_label_target(label)?;
    run_launchctl(&["kickstart".to_string(), "-k".to_string(), target])
}

fn print_daemon_service(label: &str) -> anyhow::Result<LaunchctlResult> {
    let target = launchctl_label_target(label)?;
    run_launchctl_status(&["print".to_string(), target])
}

fn supervise_daemon_service(store: &LabStore, label: &str) -> anyhow::Result<String> {
    let Some(policy) = store.load_daemon_state()? else {
        return Ok("Lab daemon service supervision skipped: no daemon policy.".to_string());
    };
    if !policy.enabled {
        return Ok("Lab daemon service supervision skipped: daemon policy disabled.".to_string());
    }
    let print = print_daemon_service(label)?;
    if print.success {
        return Ok(format!(
            "Lab daemon service supervision healthy.\n{}",
            print.format()
        ));
    }
    let load = load_daemon_service(store, label)?;
    Ok(format!(
        "Lab daemon service supervision repaired missing service.\nPrint check:\n{}\nRepair:\n{}",
        print.format(),
        load.format()
    ))
}

struct LaunchctlResult {
    command: String,
    success: bool,
    status_code: Option<i32>,
    stdout: String,
    stderr: String,
}

impl LaunchctlResult {
    fn format(&self) -> String {
        [
            format!("Command: {}", self.command),
            format!(
                "Exit status: {}",
                self.status_code
                    .map(|code| code.to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            ),
            format!("Stdout: {}", compact_command_output(&self.stdout)),
            format!("Stderr: {}", compact_command_output(&self.stderr)),
        ]
        .join("\n")
    }
}

fn run_launchctl(args: &[String]) -> anyhow::Result<LaunchctlResult> {
    let result = run_launchctl_status(args)?;
    if result.success {
        Ok(result)
    } else {
        anyhow::bail!("{}", result.format())
    }
}

fn run_launchctl_status(args: &[String]) -> anyhow::Result<LaunchctlResult> {
    let bin = launchctl_bin();
    let output = Command::new(&bin).args(args).output()?;
    let command = format!(
        "{} {}",
        bin.display(),
        args.iter()
            .map(|arg| shell_display_arg(arg))
            .collect::<Vec<_>>()
            .join(" ")
    );
    Ok(LaunchctlResult {
        command,
        success: output.status.success(),
        status_code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

fn launchctl_bin() -> PathBuf {
    std::env::var_os("PRIORITY_AGENT_LAUNCHCTL_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("launchctl"))
}

fn launchctl_gui_domain() -> anyhow::Result<String> {
    if let Ok(domain) = std::env::var("PRIORITY_AGENT_LAUNCHCTL_DOMAIN") {
        let trimmed = domain.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }
    #[cfg(unix)]
    {
        Ok(format!("gui/{}", unsafe { libc::getuid() }))
    }
    #[cfg(not(unix))]
    {
        let uid = std::env::var("UID")
            .map_err(|_| anyhow::anyhow!("UID is not set; cannot build launchctl gui domain"))?;
        Ok(format!("gui/{uid}"))
    }
}

fn launchctl_label_target(label: &str) -> anyhow::Result<String> {
    Ok(format!(
        "{}/{}",
        launchctl_gui_domain()?,
        safe_launchd_label_component(label)
    ))
}

fn daemon_service_paths(store: &LabStore, label: &str) -> DaemonServicePaths {
    DaemonServicePaths {
        generated_plist: store
            .root()
            .join("launchd")
            .join(format!("{}.plist", safe_launchd_label_component(label))),
        installed_plist: launch_agent_install_path(label).unwrap_or_else(|_| {
            PathBuf::from("~/Library/LaunchAgents")
                .join(format!("{}.plist", safe_launchd_label_component(label)))
        }),
    }
}

fn launch_agent_install_path(label: &str) -> anyhow::Result<PathBuf> {
    let dir = launch_agents_dir()?;
    Ok(dir.join(format!("{}.plist", safe_launchd_label_component(label))))
}

fn launch_agents_dir() -> anyhow::Result<PathBuf> {
    if let Some(path) = std::env::var_os("PRIORITY_AGENT_LAUNCH_AGENTS_DIR") {
        return Ok(PathBuf::from(path));
    }
    let home = std::env::var_os("HOME")
        .ok_or_else(|| anyhow::anyhow!("HOME is not set; cannot resolve ~/Library/LaunchAgents"))?;
    Ok(PathBuf::from(home).join("Library").join("LaunchAgents"))
}

fn daemon_service_lines(
    store: &LabStore,
    label: &str,
    generated_plist: &Path,
    installed_plist: &Path,
) -> Vec<String> {
    vec![
        format!("Label: {label}"),
        format!("Generated plist: {}", generated_plist.display()),
        format!("Generated exists: {}", generated_plist.exists()),
        format!("Installed plist: {}", installed_plist.display()),
        format!("Installed exists: {}", installed_plist.exists()),
        format!(
            "Install command: /lab daemon service install {}",
            safe_launchd_label_component(label)
        ),
        format!(
            "Uninstall command: launchctl bootout gui/$(id -u)/{} && /lab daemon service uninstall {}",
            safe_launchd_label_component(label),
            safe_launchd_label_component(label)
        ),
        format!(
            "Bootstrap command: launchctl bootstrap gui/$(id -u) {}",
            installed_plist.display()
        ),
        format!(
            "Kickstart command: launchctl kickstart -k gui/$(id -u)/{}",
            safe_launchd_label_component(label)
        ),
        format!(
            "Load command: /lab daemon service load {}",
            safe_launchd_label_component(label)
        ),
        format!(
            "Unload command: /lab daemon service unload {}",
            safe_launchd_label_component(label)
        ),
        format!(
            "Restart command: /lab daemon service restart {}",
            safe_launchd_label_component(label)
        ),
        format!(
            "Supervise command: /lab daemon service supervise {}",
            safe_launchd_label_component(label)
        ),
        format!(
            "Print command: launchctl print gui/$(id -u)/{}",
            safe_launchd_label_component(label)
        ),
        format!("Health command: /lab daemon health"),
        format!("Project root: {}", store.project_root().display()),
    ]
}

fn compact_command_output(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "none".to_string()
    } else {
        compact_message_line(trimmed, 240)
    }
}

fn shell_display_arg(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || "-_./:".contains(ch))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

fn write_daemon_launchd_plist(store: &LabStore, label: &str) -> anyhow::Result<PathBuf> {
    let exe = std::env::current_exe()?;
    let launchd_dir = store.root().join("launchd");
    fs::create_dir_all(&launchd_dir)?;
    let plist_path = launchd_dir.join(format!("{}.plist", safe_launchd_label_component(label)));
    let stdout_path = store.root().join("daemon.out.log");
    let stderr_path = store.root().join("daemon.err.log");
    let plist = render_launchd_plist(
        label,
        &exe,
        store.project_root(),
        &stdout_path,
        &stderr_path,
    );
    fs::write(&plist_path, plist)?;
    Ok(plist_path)
}

fn default_launchd_label(store: &LabStore) -> String {
    let project = store
        .project_root()
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("project");
    format!(
        "com.priority-agent.lab.{}",
        safe_launchd_label_component(project)
    )
}

fn safe_launchd_label_component(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut last_was_dash = false;
    for ch in value.chars() {
        let normalized = if ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' {
            last_was_dash = false;
            Some(ch.to_ascii_lowercase())
        } else if !last_was_dash {
            last_was_dash = true;
            Some('-')
        } else {
            None
        };
        if let Some(ch) = normalized {
            out.push(ch);
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "project".to_string()
    } else {
        trimmed.to_string()
    }
}

pub(super) fn render_launchd_plist(
    label: &str,
    executable: &Path,
    working_directory: &Path,
    stdout_path: &Path,
    stderr_path: &Path,
) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{}</string>
  <key>ProgramArguments</key>
  <array>
    <string>{}</string>
    <string>lab-daemon</string>
  </array>
  <key>WorkingDirectory</key>
  <string>{}</string>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <false/>
  <key>StandardOutPath</key>
  <string>{}</string>
  <key>StandardErrorPath</key>
  <string>{}</string>
</dict>
</plist>
"#,
        xml_escape(label),
        xml_escape(&executable.display().to_string()),
        xml_escape(&working_directory.display().to_string()),
        xml_escape(&stdout_path.display().to_string()),
        xml_escape(&stderr_path.display().to_string())
    )
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn parse_daemon_enable_args(
    args: &str,
) -> Result<(LabDaemonMode, usize, usize, u64, &str), String> {
    let trimmed = args.trim();
    let default_max_steps = default_background_max_steps();
    let default_max_steps_per_cycle = 5usize;
    let default_interval_ms = default_background_interval_ms();
    if trimmed.is_empty() {
        return Ok((
            LabDaemonMode::Strict,
            default_max_steps,
            default_max_steps_per_cycle,
            default_interval_ms,
            "",
        ));
    }

    let (first, rest) = split_once(trimmed);
    let (mode, rest) = match first.to_ascii_lowercase().as_str() {
        "strict" => (LabDaemonMode::Strict, rest.trim()),
        "hybrid" => (LabDaemonMode::Hybrid, rest.trim()),
        "hybrid-cycles" | "hybrid_cycles" | "cycles" => (LabDaemonMode::HybridCycles, rest.trim()),
        _ => (LabDaemonMode::Strict, trimmed),
    };
    if rest.is_empty() {
        return Ok((
            mode,
            default_max_steps,
            default_max_steps_per_cycle,
            default_interval_ms,
            "",
        ));
    }
    let (first_numeric, after_steps) = split_once(rest);
    let max_steps = match first_numeric.parse::<usize>() {
        Ok(value) if value > 0 => value,
        Ok(_) => return Err(
            "Usage: /lab daemon enable [strict|hybrid|hybrid-cycles] [max_steps] [max_steps_per_cycle] [interval_ms] [instructions]"
                .to_string(),
        ),
        Err(_) => {
            return Ok((
                mode,
                default_max_steps,
                default_max_steps_per_cycle,
                default_interval_ms,
                rest,
            ))
        }
    };
    let after_steps = after_steps.trim();
    if after_steps.is_empty() {
        return Ok((
            mode,
            max_steps,
            default_max_steps_per_cycle,
            default_interval_ms,
            "",
        ));
    }
    if mode == LabDaemonMode::HybridCycles {
        let (second_numeric, after_cycle_steps) = split_once(after_steps);
        let max_steps_per_cycle = match second_numeric.parse::<usize>() {
            Ok(value) if value > 0 => value,
            Ok(_) => return Err(
                "Usage: /lab daemon enable hybrid-cycles [max_cycles] [max_steps_per_cycle] [interval_ms] [instructions]"
                    .to_string(),
            ),
            Err(_) => {
                return Ok((
                    mode,
                    max_steps,
                    default_max_steps_per_cycle,
                    default_interval_ms,
                    after_steps,
                ))
            }
        };
        let after_cycle_steps = after_cycle_steps.trim();
        if after_cycle_steps.is_empty() {
            return Ok((
                mode,
                max_steps,
                max_steps_per_cycle,
                default_interval_ms,
                "",
            ));
        }
        let (third_numeric, instructions) = split_once(after_cycle_steps);
        let interval_ms = match third_numeric.parse::<u64>() {
            Ok(value) if value > 0 => value,
            Ok(_) => return Err(
                "Usage: /lab daemon enable hybrid-cycles [max_cycles] [max_steps_per_cycle] [interval_ms] [instructions]"
                    .to_string(),
            ),
            Err(_) => {
                return Ok((
                    mode,
                    max_steps,
                    max_steps_per_cycle,
                    default_interval_ms,
                    after_cycle_steps,
                ))
            }
        };
        return Ok((
            mode,
            max_steps,
            max_steps_per_cycle,
            interval_ms,
            instructions,
        ));
    }
    let (second_numeric, instructions) = split_once(after_steps);
    let interval_ms = match second_numeric.parse::<u64>() {
        Ok(value) if value > 0 => value,
        Ok(_) => return Err(
            "Usage: /lab daemon enable [strict|hybrid|hybrid-cycles] [max_steps] [interval_ms] [instructions]"
                .to_string(),
        ),
        Err(_) => {
            return Ok((
                mode,
                max_steps,
                default_max_steps_per_cycle,
                default_interval_ms,
                after_steps,
            ))
        }
    };
    Ok((
        mode,
        max_steps,
        default_max_steps_per_cycle,
        interval_ms,
        instructions,
    ))
}
