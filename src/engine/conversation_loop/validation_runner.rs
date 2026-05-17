use super::safe_prefix_by_bytes;
use super::workflow_trace::trace_adaptive_workflow_trigger;
use crate::engine::auto_verify::{VerificationIssue, VerificationResult};
use crate::engine::code_change_workflow::{AdaptiveWorkflowTrigger, CodeChangeWorkflowRunner};
use crate::engine::trace::{TraceCollector, TraceEvent};
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RequiredValidationApplication {
    pub(super) passed: bool,
    pub(super) ledger_records: Vec<RequiredValidationLedgerRecord>,
    pub(super) acceptance_evidence: Vec<String>,
    pub(super) post_edit_evidence: Vec<String>,
    pub(super) successful_commands: Vec<String>,
    pub(super) failed_commands: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RequiredValidationLedgerRecord {
    pub(super) command: String,
    pub(super) success: bool,
    pub(super) dialog_text: String,
}

pub(super) struct RequiredValidationTriggerContext<'a> {
    pub(super) commands: &'a [String],
    pub(super) code_workflow: &'a mut CodeChangeWorkflowRunner,
    pub(super) trace: &'a TraceCollector,
}

impl RequiredValidationController {
    pub(super) fn record_initial_trigger(context: RequiredValidationTriggerContext<'_>) -> bool {
        if context.commands.is_empty()
            || !context
                .code_workflow
                .activate_trigger(AdaptiveWorkflowTrigger::RequiredValidation)
        {
            return false;
        }

        trace_adaptive_workflow_trigger(
            context.trace,
            AdaptiveWorkflowTrigger::RequiredValidation,
            context.code_workflow,
        );
        context.trace.record(TraceEvent::WorkflowFallback {
            error: format!(
                "adaptive workflow trigger activated: required_validation commands={}",
                context.commands.len()
            ),
        });
        true
    }

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
            || is_safe_required_python_module_search_assertion(command)
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

    pub(super) fn application_for_run(run: RequiredValidationRun) -> RequiredValidationApplication {
        let mut application = RequiredValidationApplication {
            passed: run.passed,
            ledger_records: Vec::with_capacity(run.items.len()),
            acceptance_evidence: Vec::with_capacity(run.items.len()),
            post_edit_evidence: Vec::new(),
            successful_commands: Vec::new(),
            failed_commands: Vec::new(),
        };

        for item in run.items {
            application
                .acceptance_evidence
                .push(item.dialog_text.clone());
            application
                .ledger_records
                .push(RequiredValidationLedgerRecord {
                    command: item.command.clone(),
                    success: item.success,
                    dialog_text: item.dialog_text.clone(),
                });
            if item.success {
                application.successful_commands.push(item.command);
            } else {
                application.failed_commands.push(item.command);
                application.post_edit_evidence.push(item.dialog_text);
            }
        }

        application
    }

    pub(super) fn source_context_from_evidence(
        working_dir: &std::path::Path,
        evidence: &[String],
    ) -> Option<String> {
        let issues = extract_required_validation_source_issues(working_dir, evidence);
        if issues.is_empty() {
            return None;
        }

        let result = VerificationResult {
            language: "required".to_string(),
            command: "required validation".to_string(),
            success: false,
            issues,
            raw_output: evidence.join("\n"),
            summary: "required validation source context".to_string(),
        };
        verification_source_context(working_dir, &[result])
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

fn extract_required_validation_source_issues(
    working_dir: &std::path::Path,
    evidence: &[String],
) -> Vec<VerificationIssue> {
    let mut issues = Vec::new();
    let mut seen = HashSet::new();
    for text in evidence {
        for line in text.lines() {
            if let Some((file, line_number, message)) =
                parse_python_traceback_source_line(working_dir, line)
                    .or_else(|| parse_colon_source_line(working_dir, line))
            {
                let key = (file.clone(), line_number);
                if !seen.insert(key) {
                    continue;
                }
                issues.push(VerificationIssue {
                    severity: "error".to_string(),
                    file: Some(file),
                    line: Some(line_number as u32),
                    message,
                });
            }
            if issues.len() >= 12 {
                return issues;
            }
        }
    }
    issues
}

fn parse_python_traceback_source_line(
    working_dir: &std::path::Path,
    line: &str,
) -> Option<(String, usize, String)> {
    let trimmed = line.trim();
    let rest = trimmed.strip_prefix("File \"")?;
    let (file, rest) = rest.split_once("\", line ")?;
    let line_digits = rest
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    let line_number = line_digits.parse::<usize>().ok()?;
    normalize_source_file_for_issue(working_dir, file).map(|file| {
        (
            file,
            line_number,
            "required validation traceback frame".to_string(),
        )
    })
}

fn parse_colon_source_line(
    working_dir: &std::path::Path,
    line: &str,
) -> Option<(String, usize, String)> {
    for token in line.split_whitespace() {
        let token = token
            .trim_matches(|ch: char| matches!(ch, '(' | ')' | '"' | '\'' | ',' | ';' | '[' | ']'));
        if let Some((file, line_number)) = parse_colon_source_token(token) {
            if let Some(file) = normalize_source_file_for_issue(working_dir, &file) {
                return Some((
                    file,
                    line_number,
                    "required validation output location".to_string(),
                ));
            }
        }
    }
    None
}

fn parse_colon_source_token(token: &str) -> Option<(String, usize)> {
    let mut parts = token.rsplitn(3, ':');
    let last = parts.next()?;
    let second = parts.next()?;
    if second.chars().all(|ch| ch.is_ascii_digit()) {
        let file = parts.next()?;
        let line_number = second.parse::<usize>().ok()?;
        return Some((file.to_string(), line_number));
    }
    let line_number = last.parse::<usize>().ok()?;
    Some((second.to_string(), line_number))
}

fn normalize_source_file_for_issue(working_dir: &std::path::Path, file: &str) -> Option<String> {
    let raw_path = std::path::Path::new(file);
    let candidate = if raw_path.is_absolute() {
        raw_path.to_path_buf()
    } else {
        working_dir.join(raw_path)
    };
    let canonical_cwd = working_dir
        .canonicalize()
        .unwrap_or_else(|_| working_dir.to_path_buf());
    let canonical_file = candidate.canonicalize().ok()?;
    if !canonical_file.starts_with(&canonical_cwd) || !canonical_file.is_file() {
        return None;
    }
    canonical_file
        .strip_prefix(&canonical_cwd)
        .ok()
        .map(|path| path.display().to_string())
        .or_else(|| Some(canonical_file.display().to_string()))
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

fn is_safe_required_python_module_search_assertion(command: &str) -> bool {
    let normalized =
        crate::tools::bash_tool::command_classifier::normalize_command_for_match(command);
    if normalized.is_empty()
        || normalized.contains('\n')
        || normalized.contains(';')
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

    let Some((left, right)) = normalized.split_once('|') else {
        return false;
    };
    let right = right.trim_start();
    if !(right.starts_with("rg ") || right.starts_with("grep ")) {
        return false;
    }
    let left = left.trim();
    let python = left
        .strip_prefix(". .venv/bin/activate && ")
        .or_else(|| left.strip_prefix("source .venv/bin/activate && "))
        .unwrap_or(left)
        .trim_start();
    python.starts_with("python -m ") || python.starts_with("python3 -m ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::intent_router::IntentRouter;
    use crate::engine::task_context::TaskContextBundle;
    use crate::engine::trace::{TurnStatus, TurnTrace};

    fn code_workflow() -> CodeChangeWorkflowRunner {
        let route = IntentRouter::new().route("修改 src/main.rs 并运行验证");
        let bundle = TaskContextBundle::new("修改 src/main.rs", ".", route, None);
        CodeChangeWorkflowRunner::new(&bundle)
    }

    fn trace() -> TraceCollector {
        TraceCollector::new(TurnTrace::new(
            "session".to_string(),
            1,
            "required-validation",
        ))
    }

    #[test]
    fn initial_required_validation_trigger_records_once() {
        let trace = trace();
        let mut code_workflow = code_workflow();
        let commands = vec!["cargo test -q".to_string()];

        assert!(RequiredValidationController::record_initial_trigger(
            RequiredValidationTriggerContext {
                commands: &commands,
                code_workflow: &mut code_workflow,
                trace: &trace,
            },
        ));
        assert!(!RequiredValidationController::record_initial_trigger(
            RequiredValidationTriggerContext {
                commands: &commands,
                code_workflow: &mut code_workflow,
                trace: &trace,
            },
        ));

        assert_eq!(
            code_workflow.adaptive_trigger_labels(),
            vec!["required_validation"]
        );
        let finished = trace.finish(TurnStatus::Completed);
        assert_eq!(
            finished
                .events
                .iter()
                .filter(|event| matches!(
                    event,
                    TraceEvent::AdaptiveWorkflowTriggered { trigger, .. }
                    if trigger == "required_validation"
                ))
                .count(),
            1
        );
        assert!(finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::WorkflowFallback { error }
                if error == "adaptive workflow trigger activated: required_validation commands=1"
        )));
    }

    #[test]
    fn initial_required_validation_trigger_ignores_empty_commands() {
        let trace = trace();
        let mut code_workflow = code_workflow();

        assert!(!RequiredValidationController::record_initial_trigger(
            RequiredValidationTriggerContext {
                commands: &[],
                code_workflow: &mut code_workflow,
                trace: &trace,
            },
        ));

        assert!(code_workflow.adaptive_trigger_labels().is_empty());
        let finished = trace.finish(TurnStatus::Completed);
        assert!(!finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::AdaptiveWorkflowTriggered { trigger, .. }
                if trigger == "required_validation"
        )));
        assert!(!finished.events.iter().any(|event| matches!(
            event,
            TraceEvent::WorkflowFallback { error }
                if error.contains("required_validation")
        )));
    }

    #[test]
    fn required_validation_source_context_extracts_node_error_line() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join("fixtures/app")).expect("create fixture dir");
        let source = tmp.path().join("fixtures/app/app.js");
        std::fs::write(&source, "function demo() {\n  return 1;\n}\n}\n").expect("write source");
        let evidence = vec![format!(
            "$ node fixtures/app/test.cjs\n{}:4\n}}\n^\nSyntaxError: Unexpected token '}}'",
            source.display()
        )];

        let context =
            RequiredValidationController::source_context_from_evidence(tmp.path(), &evidence)
                .expect("node source context");

        assert!(context.contains("fixtures/app/app.js:4"));
        assert!(context.contains(">    4 | }"));
        assert!(context.contains("required validation output location"));
    }

    #[test]
    fn required_validation_source_context_extracts_python_traceback_line() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join("tests")).expect("create tests dir");
        let source = tmp.path().join("tests/test_api.py");
        std::fs::write(
            &source,
            "def test_route():\n    status = 404\n    assert status == 200\n",
        )
        .expect("write source");
        let evidence = vec![format!(
            "Traceback (most recent call last):\n  File \"{}\", line 3, in test_route\n    assert status == 200\nAssertionError: 404 != 200",
            source.display()
        )];

        let context =
            RequiredValidationController::source_context_from_evidence(tmp.path(), &evidence)
                .expect("python source context");

        assert!(context.contains("tests/test_api.py:3"));
        assert!(context.contains(">    3 |     assert status == 200"));
        assert!(context.contains("required validation traceback frame"));
    }
}
