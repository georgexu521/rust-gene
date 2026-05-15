use super::validation_runner::{verification_source_context, RequiredValidationController};
use crate::engine::auto_verify::VerificationResult;
use crate::engine::evidence_ledger::{changed_files_diff_evidence, EvidenceLedger};
use crate::engine::lsp::LspManager;
use crate::engine::trace::{TraceCollector, TraceEvent};
use crate::services::api::Message;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tracing::debug;

pub(super) struct PostEditVerificationContext<'a> {
    pub(super) working_dir: &'a Path,
    pub(super) changed_files: &'a [PathBuf],
    pub(super) lsp_manager: Option<&'a LspManager>,
    pub(super) required_validation_commands: &'a [String],
    pub(super) successful_validation_commands: &'a [String],
    pub(super) successful_required_validation_commands: &'a mut HashSet<String>,
    pub(super) evidence_ledger: &'a mut EvidenceLedger,
    pub(super) tool_results_text: &'a mut String,
    pub(super) messages: &'a mut Vec<Message>,
}

pub(super) struct PostEditVerificationOutcome {
    pub(super) check_passed: bool,
    pub(super) effective_check_passed: bool,
    pub(super) effective_tests_passed: bool,
    pub(super) required_validation_passed: bool,
    pub(super) review_success: bool,
    pub(super) verify_passed: bool,
    pub(super) failed_commands: Vec<String>,
    pub(super) post_edit_evidence: Vec<String>,
    pub(super) acceptance_evidence: Vec<String>,
}

pub(super) struct PostEditVerificationTraceOutcome {
    pub(super) should_closeout_after_verified_change: bool,
}

pub(super) struct PostEditVerificationController;

impl PostEditVerificationController {
    pub(super) async fn run(
        context: PostEditVerificationContext<'_>,
    ) -> PostEditVerificationOutcome {
        let mut post_edit_evidence = Vec::new();
        let mut acceptance_evidence = Vec::new();
        let mut failed_commands = Vec::new();

        let verify_results = crate::engine::auto_verify::verify_file_changes(
            context.working_dir,
            context.changed_files,
        )
        .await;
        let check_passed = verify_results.iter().all(|result| result.success);
        Self::append_source_context_on_failure(
            context.working_dir,
            &verify_results,
            check_passed,
            &mut post_edit_evidence,
            &mut *context.tool_results_text,
            &mut *context.messages,
        );
        Self::apply_verification_results(
            verify_results,
            VerificationResultApplication {
                source: "auto_verify",
                ignore_failed_results: false,
                required_validation_covers_tests: false,
                evidence_ledger: &mut *context.evidence_ledger,
                acceptance_evidence: &mut acceptance_evidence,
                post_edit_evidence: &mut post_edit_evidence,
                failed_commands: &mut failed_commands,
                tool_results_text: &mut *context.tool_results_text,
                messages: &mut *context.messages,
            },
        );

        if let Some(lsp_text) =
            Self::lsp_diagnostics_text(context.lsp_manager, context.changed_files).await
        {
            post_edit_evidence.push(lsp_text.clone());
            Self::append_system_text(
                &mut *context.tool_results_text,
                &mut *context.messages,
                lsp_text,
            );
        }

        let mut required_validation_passed = true;
        if !context.required_validation_commands.is_empty() {
            let required_run = RequiredValidationController::run_pending_commands(
                context.working_dir,
                context.required_validation_commands,
                context.successful_validation_commands,
                &*context.successful_required_validation_commands,
            )
            .await;
            let required_application =
                RequiredValidationController::application_for_run(required_run);
            required_validation_passed = required_application.passed;
            acceptance_evidence.extend(required_application.acceptance_evidence);
            post_edit_evidence.extend(required_application.post_edit_evidence.clone());
            failed_commands.extend(required_application.failed_commands);
            for command in required_application.successful_commands {
                context
                    .successful_required_validation_commands
                    .insert(command);
            }
            for record in required_application.ledger_records {
                context.evidence_ledger.record_validation_result(
                    "required_validation",
                    Some(&record.command),
                    record.success,
                    &record.dialog_text,
                );
                if record.success {
                    debug!("{}", record.dialog_text);
                }
            }
            for text in required_application.post_edit_evidence {
                Self::append_system_text(
                    &mut *context.tool_results_text,
                    &mut *context.messages,
                    text,
                );
            }
        }
        let required_validation_covers_tests =
            !context.required_validation_commands.is_empty() && required_validation_passed;

        let manual_validation_after_changes = !context.successful_validation_commands.is_empty();
        let test_results = if RequiredValidationController::should_run_default_auto_tests(
            context.required_validation_commands,
        ) {
            crate::engine::auto_verify::run_tests(
                context.working_dir,
                context.changed_files,
                check_passed,
            )
            .await
        } else {
            Vec::new()
        };
        let tests_passed = required_validation_covers_tests
            || test_results.iter().all(|result| result.success)
            || (manual_validation_after_changes && check_passed);
        Self::append_source_context_on_failure(
            context.working_dir,
            &test_results,
            tests_passed,
            &mut post_edit_evidence,
            &mut *context.tool_results_text,
            &mut *context.messages,
        );
        Self::apply_verification_results(
            test_results,
            VerificationResultApplication {
                source: "auto_test",
                ignore_failed_results: manual_validation_after_changes,
                required_validation_covers_tests,
                evidence_ledger: &mut *context.evidence_ledger,
                acceptance_evidence: &mut acceptance_evidence,
                post_edit_evidence: &mut post_edit_evidence,
                failed_commands: &mut failed_commands,
                tool_results_text: &mut *context.tool_results_text,
                messages: &mut *context.messages,
            },
        );

        if manual_validation_after_changes {
            let manual_text = format!(
                "[Manual validation passed after code changes]\n{}",
                context
                    .successful_validation_commands
                    .iter()
                    .map(|cmd| format!("  $ {}", cmd))
                    .collect::<Vec<_>>()
                    .join("\n")
            );
            acceptance_evidence.push(manual_text.clone());
            post_edit_evidence.push(manual_text.clone());
            for command in context.successful_validation_commands {
                context.evidence_ledger.record_validation_result(
                    "manual_validation",
                    Some(command),
                    true,
                    &manual_text,
                );
            }
            debug!("{}", manual_text);
        }

        if let Some(diff_text) =
            changed_files_diff_evidence(context.working_dir, context.changed_files).await
        {
            acceptance_evidence.push(diff_text.clone());
            post_edit_evidence.push(diff_text.clone());
            context
                .evidence_ledger
                .record_validation_result("diff", None, true, &diff_text);
            debug!("{}", diff_text);
        }

        let review_result = crate::engine::code_review::review_changed_files(
            context.working_dir,
            context.changed_files,
        );
        let review_dialog = review_result.to_dialog_text();
        context.evidence_ledger.record_validation_result(
            "code_review",
            None,
            review_result.success,
            &review_dialog,
        );
        acceptance_evidence.push(review_dialog);
        if !review_result.success {
            let review_text = review_result.to_dialog_text();
            post_edit_evidence.push(review_text.clone());
            Self::append_system_text(
                &mut *context.tool_results_text,
                &mut *context.messages,
                review_text,
            );
        }

        let effective_check_passed = check_passed || required_validation_covers_tests;
        let effective_tests_passed = tests_passed || required_validation_covers_tests;
        let summary = Self::summarize(
            effective_check_passed,
            effective_tests_passed,
            required_validation_passed,
            review_result.success,
        );

        PostEditVerificationOutcome {
            check_passed,
            effective_check_passed,
            effective_tests_passed,
            required_validation_passed,
            review_success: review_result.success,
            verify_passed: summary.verify_passed,
            failed_commands,
            post_edit_evidence,
            acceptance_evidence,
        }
    }

    pub(super) fn record_trace(
        trace: &TraceCollector,
        changed_files: &[PathBuf],
        verification: &PostEditVerificationOutcome,
    ) -> PostEditVerificationTraceOutcome {
        trace.record(TraceEvent::VerificationCompleted {
            changed_files: changed_files.len(),
            passed: verification.verify_passed,
            check_passed: verification.effective_check_passed,
            tests_passed: verification.effective_tests_passed,
            review_passed: verification.review_success,
            failed_commands: verification.failed_commands.clone(),
        });
        PostEditVerificationTraceOutcome {
            should_closeout_after_verified_change: verification.verify_passed,
        }
    }

    fn summarize(
        effective_check_passed: bool,
        effective_tests_passed: bool,
        required_validation_passed: bool,
        review_success: bool,
    ) -> PostEditVerificationSummary {
        PostEditVerificationSummary {
            verify_passed: effective_check_passed
                && effective_tests_passed
                && required_validation_passed
                && review_success,
        }
    }

    fn apply_verification_results(
        results: Vec<VerificationResult>,
        context: VerificationResultApplication<'_>,
    ) {
        for result in results {
            let dialog_text = result.to_dialog_text();
            context.acceptance_evidence.push(dialog_text.clone());
            context.evidence_ledger.record_validation_result(
                context.source,
                Some(&result.command),
                result.success,
                &dialog_text,
            );
            if !result.success {
                if context.ignore_failed_results || context.required_validation_covers_tests {
                    debug!(
                        "Ignoring stale automatic test failure after successful required/manual validation command: {}",
                        result.command
                    );
                } else {
                    context.failed_commands.push(result.command.clone());
                    context.post_edit_evidence.push(dialog_text.clone());
                    Self::append_system_text(
                        context.tool_results_text,
                        context.messages,
                        dialog_text,
                    );
                }
            } else {
                debug!("{}", dialog_text);
            }
        }
    }

    fn append_source_context_on_failure(
        working_dir: &Path,
        results: &[VerificationResult],
        passed: bool,
        post_edit_evidence: &mut Vec<String>,
        tool_results_text: &mut String,
        messages: &mut Vec<Message>,
    ) {
        if passed {
            return;
        }
        if let Some(source_context) = verification_source_context(working_dir, results) {
            post_edit_evidence.push(source_context.clone());
            Self::append_system_text(tool_results_text, messages, source_context);
        }
    }

    async fn lsp_diagnostics_text(
        lsp_manager: Option<&LspManager>,
        changed_files: &[PathBuf],
    ) -> Option<String> {
        let lsp_manager = lsp_manager?;
        let mut lsp_issues = Vec::new();
        for path in changed_files {
            let uri = crate::engine::lsp::path_to_uri(path);
            for name in lsp_manager.server_names() {
                if let Some(client) = lsp_manager.get_client(&name) {
                    let diagnostics = client.get_diagnostics(&uri).await;
                    for diagnostic in diagnostics {
                        let severity = match diagnostic.severity {
                            Some(1) => "error",
                            Some(2) => "warning",
                            Some(3) => "info",
                            Some(4) => "hint",
                            _ => "diagnostic",
                        };
                        lsp_issues.push(format!(
                            "  [{}] {}:{}: {}",
                            severity,
                            path.display(),
                            diagnostic.range.start.line + 1,
                            diagnostic.message.replace('\n', " ")
                        ));
                    }
                }
            }
        }
        if lsp_issues.is_empty() {
            None
        } else {
            Some(format!(
                "[LSP diagnostics for modified files]:\n{}",
                lsp_issues.join("\n")
            ))
        }
    }

    fn append_system_text(
        tool_results_text: &mut String,
        messages: &mut Vec<Message>,
        text: String,
    ) {
        tool_results_text.push('\n');
        tool_results_text.push_str(&text);
        messages.push(Message::system(text));
    }
}

struct PostEditVerificationSummary {
    verify_passed: bool,
}

struct VerificationResultApplication<'a> {
    source: &'a str,
    ignore_failed_results: bool,
    required_validation_covers_tests: bool,
    evidence_ledger: &'a mut EvidenceLedger,
    acceptance_evidence: &'a mut Vec<String>,
    post_edit_evidence: &'a mut Vec<String>,
    failed_commands: &'a mut Vec<String>,
    tool_results_text: &'a mut String,
    messages: &'a mut Vec<Message>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::trace::{TurnStatus, TurnTrace};

    #[test]
    fn summary_requires_all_validation_channels_to_pass() {
        assert!(PostEditVerificationController::summarize(true, true, true, true).verify_passed);
        assert!(!PostEditVerificationController::summarize(false, true, true, true).verify_passed);
        assert!(!PostEditVerificationController::summarize(true, false, true, true).verify_passed);
        assert!(!PostEditVerificationController::summarize(true, true, false, true).verify_passed);
        assert!(!PostEditVerificationController::summarize(true, true, true, false).verify_passed);
    }

    fn verification_outcome(verify_passed: bool) -> PostEditVerificationOutcome {
        PostEditVerificationOutcome {
            check_passed: verify_passed,
            effective_check_passed: verify_passed,
            effective_tests_passed: true,
            required_validation_passed: true,
            review_success: true,
            verify_passed,
            failed_commands: vec!["cargo test -q".to_string()],
            post_edit_evidence: Vec::new(),
            acceptance_evidence: Vec::new(),
        }
    }

    #[test]
    fn record_trace_maps_verification_fields_to_trace_and_closeout_flag() {
        let trace = TraceCollector::new(TurnTrace::new("session", 1, "change code"));
        let changed_files = vec![PathBuf::from("src/lib.rs"), PathBuf::from("src/main.rs")];
        let verification = verification_outcome(false);

        let outcome =
            PostEditVerificationController::record_trace(&trace, &changed_files, &verification);

        assert!(!outcome.should_closeout_after_verified_change);
        let finished = trace.finish(TurnStatus::Completed);
        let event = finished
            .events
            .iter()
            .find_map(|event| match event {
                TraceEvent::VerificationCompleted {
                    changed_files,
                    passed,
                    check_passed,
                    tests_passed,
                    review_passed,
                    failed_commands,
                } => Some((
                    *changed_files,
                    *passed,
                    *check_passed,
                    *tests_passed,
                    *review_passed,
                    failed_commands.clone(),
                )),
                _ => None,
            })
            .expect("verification trace event");

        assert_eq!(event.0, 2);
        assert!(!event.1);
        assert!(!event.2);
        assert!(event.3);
        assert!(event.4);
        assert_eq!(event.5, vec!["cargo test -q".to_string()]);
    }
}
