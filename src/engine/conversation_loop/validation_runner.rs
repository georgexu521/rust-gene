use super::safe_prefix_by_bytes;
use crate::engine::auto_verify::{VerificationIssue, VerificationResult};
use crate::services::api::ToolCall;
use std::collections::HashSet;
use tokio::io::AsyncReadExt;

fn required_validation_timeout() -> std::time::Duration {
    let secs = std::env::var("PRIORITY_AGENT_REQUIRED_VALIDATION_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(900)
        .clamp(30, 900);
    std::time::Duration::from_secs(secs)
}

fn sanitize_required_validation_env(cmd: &mut tokio::process::Command) {
    for key in [
        "PRIORITY_AGENT_A2A_TRANSCRIPT_PATH",
        "PRIORITY_AGENT_AUTO_REVIEW",
        "PRIORITY_AGENT_AUTO_TEST",
        "PRIORITY_AGENT_CLOSEOUT_VISIBILITY",
        "PRIORITY_AGENT_EVAL_EVENTS",
        "PRIORITY_AGENT_LEGACY_WORKFLOW_ENABLED",
        "PRIORITY_AGENT_LLM_MEMORY_EXTRACTION",
        "PRIORITY_AGENT_WORKFLOW_CONTRACT",
        "PRIORITY_AGENT_WORKFLOW_ENABLED",
    ] {
        cmd.env_remove(key);
    }
}

pub(super) async fn shell_output_with_timeout(
    command: &str,
    working_dir: &std::path::Path,
    timeout: std::time::Duration,
) -> std::io::Result<std::process::Output> {
    let mut cmd = tokio::process::Command::new("sh");
    cmd.arg("-lc").arg(command).current_dir(working_dir);
    sanitize_required_validation_env(&mut cmd);
    #[cfg(unix)]
    cmd.process_group(0);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    cmd.kill_on_drop(true);

    let mut child = cmd.spawn()?;
    let child_pid = child.id();
    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    let stdout_task = tokio::spawn(async move {
        let mut buffer = Vec::new();
        if let Some(ref mut stream) = stdout {
            stream.read_to_end(&mut buffer).await?;
        }
        Ok::<Vec<u8>, std::io::Error>(buffer)
    });
    let stderr_task = tokio::spawn(async move {
        let mut buffer = Vec::new();
        if let Some(ref mut stream) = stderr {
            stream.read_to_end(&mut buffer).await?;
        }
        Ok::<Vec<u8>, std::io::Error>(buffer)
    });

    let started_at = std::time::Instant::now();
    let mut heartbeat = tokio::time::interval(std::time::Duration::from_secs(30));
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let status = loop {
        tokio::select! {
            result = child.wait() => break result?,
            _ = heartbeat.tick() => {
                let elapsed = started_at.elapsed();
                if elapsed >= std::time::Duration::from_secs(30) {
                    eprintln!(
                        "[required validation still running after {}s] {}",
                        elapsed.as_secs(),
                        safe_prefix_by_bytes(command, 160)
                    );
                }
            }
            _ = tokio::time::sleep_until(tokio::time::Instant::from_std(started_at + timeout)) => {
                #[cfg(unix)]
                if let Some(pid) = child_pid {
                    unsafe {
                        libc::kill(-(pid as i32), libc::SIGKILL);
                    }
                }
                let _ = child.start_kill();
                let _ = child.wait().await;
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    format!("command timed out after {}s", timeout.as_secs()),
                ));
            }
        }
    };
    let stdout = stdout_task.await.map_err(std::io::Error::other)??;
    let stderr = stderr_task.await.map_err(std::io::Error::other)??;
    Ok(std::process::Output {
        status,
        stdout,
        stderr,
    })
}

pub(super) fn verification_source_context(
    working_dir: &std::path::Path,
    results: &[VerificationResult],
) -> Option<String> {
    let canonical_cwd = working_dir
        .canonicalize()
        .unwrap_or_else(|_| working_dir.to_path_buf());
    let mut snippets = Vec::new();
    let mut seen = HashSet::new();
    let mut total_chars = 0usize;

    for result in results {
        for issue in result.issues.iter().take(12) {
            let Some(file) = issue.file.as_deref() else {
                continue;
            };
            let raw_path = std::path::Path::new(file);
            let candidate = if raw_path.is_absolute() {
                raw_path.to_path_buf()
            } else {
                working_dir.join(raw_path)
            };
            let Ok(canonical_file) = candidate.canonicalize() else {
                continue;
            };
            if !canonical_file.starts_with(&canonical_cwd) || !canonical_file.is_file() {
                continue;
            }
            let line = issue.line.unwrap_or(1).max(1) as usize;
            let key = (canonical_file.clone(), line);
            if !seen.insert(key) {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(&canonical_file) else {
                continue;
            };
            let lines = content.lines().collect::<Vec<_>>();
            if lines.is_empty() {
                continue;
            }
            let start = line.saturating_sub(3).max(1);
            let end = (line + 3).min(lines.len());
            let relative = canonical_file
                .strip_prefix(&canonical_cwd)
                .ok()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| canonical_file.display().to_string());
            let mut snippet = format!(
                "[Verification source context] {}:{} ({})\n",
                relative, line, issue.message
            );
            for idx in start..=end {
                let marker = if idx == line { ">" } else { " " };
                let source_line = lines.get(idx - 1).copied().unwrap_or_default();
                snippet.push_str(&format!("{marker} {idx:>4} | {source_line}\n"));
            }
            total_chars += snippet.chars().count();
            snippets.push(snippet);
            if total_chars >= 12_000 {
                break;
            }
        }
        if total_chars >= 12_000 {
            break;
        }
    }

    if snippets.is_empty() {
        None
    } else {
        Some(format!(
            "{}\nUse this exact current source context to repair compile/validation errors before addressing broader acceptance gaps.",
            snippets.join("\n")
        ))
    }
}

pub(super) struct RequiredValidationController;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RequiredValidationRun {
    pub(super) passed: bool,
    pub(super) items: Vec<RequiredValidationResultItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RequiredValidationResultItem {
    pub(super) command: String,
    pub(super) success: bool,
    pub(super) dialog_text: String,
}

impl RequiredValidationController {
    pub(super) fn should_run_default_auto_tests(required_validation_commands: &[String]) -> bool {
        required_validation_commands.is_empty()
    }

    pub(super) fn successful_validation_command(
        tool_call: &ToolCall,
        success: bool,
    ) -> Option<String> {
        if !success || !Self::is_validation_tool_call(tool_call) {
            return None;
        }
        tool_call.arguments["command"]
            .as_str()
            .map(str::trim)
            .filter(|command| !command.is_empty())
            .map(ToString::to_string)
    }

    pub(super) fn command_matches_required(
        required_validation_commands: &[String],
        command: &str,
    ) -> bool {
        let normalized_command = Self::normalize_command_for_match(command);
        required_validation_commands
            .iter()
            .any(|required| Self::normalize_command_for_match(required) == normalized_command)
    }

    pub(super) fn pending_commands(
        required_validation_commands: &[String],
        successful_validation_commands: &[String],
        successful_required_validation_commands: &HashSet<String>,
    ) -> Vec<String> {
        let already_ran = successful_validation_commands
            .iter()
            .map(|cmd| Self::normalize_command_for_match(cmd))
            .chain(
                successful_required_validation_commands
                    .iter()
                    .map(|cmd| Self::normalize_command_for_match(cmd)),
            )
            .collect::<HashSet<_>>();

        required_validation_commands
            .iter()
            .filter(|cmd| !already_ran.contains(&Self::normalize_command_for_match(cmd)))
            .cloned()
            .collect()
    }

    pub(super) fn is_validation_tool_call(tool_call: &ToolCall) -> bool {
        if tool_call.name != "bash" {
            return false;
        }
        let Some(command) = tool_call.arguments["command"].as_str() else {
            return false;
        };
        crate::tools::bash_tool::command_classifier::classify_command(command).is_safe_validation()
    }

    pub(super) fn normalize_command_for_match(command: &str) -> String {
        crate::tools::bash_tool::command_classifier::normalize_command_for_match(command)
    }

    fn is_safe_validation_command(command: &str) -> bool {
        crate::tools::bash_tool::command_classifier::classify_command(command).is_safe_validation()
    }

    fn is_safe_required_validation_command(command: &str) -> bool {
        Self::is_safe_validation_command(command)
            || command.starts_with("python3 -c ")
            || command.starts_with("python -c ")
            || is_safe_required_search_assertion(command)
    }

    pub(super) fn extract_commands(prompt: &str) -> Vec<String> {
        let mut commands = Vec::new();
        for line in prompt.lines() {
            let trimmed = line.trim();
            if !trimmed.starts_with("- `") {
                continue;
            }
            let rest = &trimmed[3..];
            let Some(end) = rest.find('`') else {
                continue;
            };
            let command = rest[..end].trim();
            if command.is_empty() || command == "(none)" {
                continue;
            }
            if Self::is_safe_required_validation_command(command)
                && !commands.iter().any(|existing| existing == command)
            {
                commands.push(command.to_string());
            }
        }
        commands
    }

    pub(super) async fn run_pending_commands(
        working_dir: &std::path::Path,
        required_validation_commands: &[String],
        successful_validation_commands: &[String],
        successful_required_validation_commands: &HashSet<String>,
    ) -> RequiredValidationRun {
        let pending = Self::pending_commands(
            required_validation_commands,
            successful_validation_commands,
            successful_required_validation_commands,
        );
        Self::summarize_results(Self::run_commands(working_dir, &pending).await)
    }

    pub(super) fn summarize_results(results: Vec<VerificationResult>) -> RequiredValidationRun {
        let mut passed = true;
        let items = results
            .into_iter()
            .map(|result| {
                let dialog_text = result.to_dialog_text();
                if !result.success {
                    passed = false;
                }
                RequiredValidationResultItem {
                    command: result.command,
                    success: result.success,
                    dialog_text,
                }
            })
            .collect();

        RequiredValidationRun { passed, items }
    }

    pub(super) async fn run_commands(
        working_dir: &std::path::Path,
        commands: &[String],
    ) -> Vec<VerificationResult> {
        let mut results = Vec::new();
        for command in commands.iter().take(8) {
            let timeout = required_validation_timeout();
            let output = shell_output_with_timeout(command, working_dir, timeout).await;
            let result = match output {
                Ok(output) => {
                    let raw_output = format!(
                        "{}{}",
                        String::from_utf8_lossy(&output.stdout),
                        String::from_utf8_lossy(&output.stderr)
                    );
                    VerificationResult {
                        language: "required".to_string(),
                        command: command.clone(),
                        success: output.status.success(),
                        issues: if output.status.success() {
                            Vec::new()
                        } else {
                            vec![VerificationIssue {
                                severity: "error".to_string(),
                                file: None,
                                line: None,
                                message: safe_prefix_by_bytes(&raw_output, 1200).to_string(),
                            }]
                        },
                        raw_output,
                        summary: if output.status.success() {
                            format!("required command passed: {}", command)
                        } else {
                            format!("required command failed: {}", command)
                        },
                    }
                }
                Err(err) => {
                    let timed_out = err.kind() == std::io::ErrorKind::TimedOut;
                    let message = if timed_out {
                        format!("required command timed out after {}s", timeout.as_secs())
                    } else {
                        format!("failed to run required command: {}", err)
                    };
                    VerificationResult {
                        language: "required".to_string(),
                        command: command.clone(),
                        success: false,
                        issues: vec![VerificationIssue {
                            severity: "error".to_string(),
                            file: None,
                            line: None,
                            message,
                        }],
                        raw_output: err.to_string(),
                        summary: if timed_out {
                            format!("required command timed out: {}", command)
                        } else {
                            format!("required command failed to run: {}", command)
                        },
                    }
                }
            };
            results.push(result);
        }
        results
    }
}

fn is_safe_required_search_assertion(command: &str) -> bool {
    let normalized =
        crate::tools::bash_tool::command_classifier::normalize_command_for_match(command);
    if normalized.is_empty()
        || normalized.contains('\n')
        || normalized.contains(';')
        || normalized.contains('|')
        || normalized.contains("&&")
        || normalized.contains("||")
        || normalized.ends_with('&')
        || normalized.contains(" & ")
        || normalized.contains('`')
        || normalized.contains("$(")
        || normalized.contains('>')
        || normalized.contains('<')
    {
        return false;
    }

    let mut tokens = normalized.split_whitespace();
    let Some(tool) = tokens.next() else {
        return false;
    };
    if tool != "rg" && tool != "grep" {
        return false;
    }

    let positional = tokens
        .filter(|token| !token.starts_with('-'))
        .collect::<Vec<_>>();
    positional.len() >= 2
}
