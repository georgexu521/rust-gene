use crate::engine::workflow_contract::AcceptanceReview;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepairSpec {
    pub failed_commands: Vec<String>,
    pub failed_command_evidence: Vec<String>,
    pub tool_record_evidence: Vec<String>,
    pub residual_command_matches: Vec<String>,
    pub failed_tests: Vec<String>,
    pub required_next_patch: Vec<String>,
    pub forbidden_fixes: Vec<String>,
    pub validation_commands: Vec<String>,
}

impl RepairSpec {
    pub fn from_failure(
        failed_commands: &[String],
        evidence: &[String],
        review: Option<&AcceptanceReview>,
    ) -> Self {
        Self::from_failure_with_tool_records(failed_commands, evidence, &[], review)
    }

    pub fn from_failure_with_tool_records(
        failed_commands: &[String],
        evidence: &[String],
        tool_record_evidence: &[String],
        review: Option<&AcceptanceReview>,
    ) -> Self {
        let failed_tests = extract_failed_tests(evidence);
        let failed_command_evidence = extract_failed_command_evidence(failed_commands, evidence);
        let residual_command_matches =
            extract_residual_command_matches(failed_commands, &failed_command_evidence);
        let tool_record_evidence = unique_strings(tool_record_evidence.iter().cloned());
        let mut required_next_patch = Vec::new();
        let mut forbidden_fixes = Vec::new();

        if !failed_tests.is_empty() {
            required_next_patch.push(format!(
                "Fix the failing tests before closeout: {}",
                failed_tests.join(", ")
            ));
        }
        for command in failed_commands {
            push_unique(
                &mut required_next_patch,
                format!(
                    "Before editing, explain why the current diff still fails `{command}` using failed_command_evidence"
                ),
            );
            push_unique(
                &mut required_next_patch,
                format!("Rerun and satisfy `{command}` after the next patch"),
            );
        }
        if !tool_record_evidence.is_empty() {
            push_unique(
                &mut required_next_patch,
                "Use tool_record_evidence to identify the latest failed/changed tool calls before choosing the next patch".to_string(),
            );
        }
        for item in &residual_command_matches {
            push_unique(
                &mut required_next_patch,
                format!("Eliminate or explicitly justify residual required-command match: {item}"),
            );
        }

        if let Some(review) = review {
            for item in &review.unresolved_items {
                push_unique(&mut required_next_patch, item.clone());
            }
            for item in &review.residual_risks {
                push_unique(&mut required_next_patch, item.clone());
            }
        }

        for item in extract_forbidden_fixes(evidence) {
            push_unique(&mut forbidden_fixes, item);
        }
        if forbidden_fixes.is_empty() {
            forbidden_fixes.push(
                "Do not edit tests or acceptance fixtures just to make validation pass".to_string(),
            );
        }

        Self {
            failed_commands: unique_strings(failed_commands.iter().cloned()),
            failed_command_evidence,
            tool_record_evidence,
            residual_command_matches,
            failed_tests,
            required_next_patch,
            forbidden_fixes,
            validation_commands: unique_strings(failed_commands.iter().cloned()),
        }
    }

    pub fn format_for_prompt(&self) -> String {
        let mut out = String::new();
        out.push_str("RepairSpec:\n");
        push_section(&mut out, "failed_commands", &self.failed_commands);
        push_section(
            &mut out,
            "failed_command_evidence",
            &self.failed_command_evidence,
        );
        push_section(&mut out, "tool_record_evidence", &self.tool_record_evidence);
        push_section(
            &mut out,
            "residual_command_matches",
            &self.residual_command_matches,
        );
        push_section(&mut out, "failed_tests", &self.failed_tests);
        push_section(&mut out, "required_next_patch", &self.required_next_patch);
        push_section(&mut out, "forbidden_fixes", &self.forbidden_fixes);
        push_section(&mut out, "validation_commands", &self.validation_commands);
        out.push_str(
            "Instruction: first state the concrete mismatch shown by failed_command_evidence, tool_record_evidence, and residual_command_matches, then make the smallest code patch that satisfies this spec, then rerun validation. Do not close out until failed_commands pass or you name a concrete blocker.\n",
        );
        out
    }
}

fn extract_failed_command_evidence(failed_commands: &[String], evidence: &[String]) -> Vec<String> {
    let mut snippets = Vec::new();
    for command in failed_commands {
        let command = command.trim();
        if command.is_empty() {
            continue;
        }
        for text in evidence {
            if !text.contains(command) {
                continue;
            }
            push_unique(
                &mut snippets,
                format!("`{}` output:\n{}", command, compact_evidence(text, 1800)),
            );
        }
    }

    if snippets.is_empty() {
        for text in evidence {
            if text.contains("[required verification]") || text.contains("required command failed")
            {
                push_unique(
                    &mut snippets,
                    format!(
                        "required validation output:\n{}",
                        compact_evidence(text, 1800)
                    ),
                );
            }
        }
    }

    snippets
}

fn extract_residual_command_matches(
    failed_commands: &[String],
    failed_command_evidence: &[String],
) -> Vec<String> {
    let mut matches = Vec::new();
    for command in failed_commands {
        let normalized = command.trim();
        if normalized.is_empty() || !is_required_negative_rg_command(normalized) {
            continue;
        }
        for evidence in failed_command_evidence {
            if !evidence.contains(normalized) {
                continue;
            }
            for line in evidence.lines() {
                if let Some(match_line) = parse_required_match_line(line) {
                    push_unique(
                        &mut matches,
                        format!("`{}` matched `{}`", normalized, match_line),
                    );
                }
            }
        }
    }
    matches
}

fn is_required_negative_rg_command(command: &str) -> bool {
    let trimmed = command.trim_start();
    trimmed.starts_with("! rg ") || trimmed.starts_with("! grep ")
}

fn parse_required_match_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if let Some(rest) = trimmed.strip_prefix("[error] ") {
        let (_, message) = rest.split_once(": ")?;
        let message = message.trim();
        return (!message.is_empty()).then(|| message.to_string());
    }
    if trimmed.is_empty()
        || trimmed.contains("` output:")
        || trimmed.starts_with('[')
        || trimmed.starts_with('$')
        || trimmed.starts_with("Result:")
        || trimmed.starts_with("required ")
        || trimmed.starts_with("... [truncated]")
    {
        return None;
    }
    if trimmed.contains("[required command failed") || trimmed.contains("found 0 warning") {
        return None;
    }
    Some(trimmed.to_string())
}

fn extract_failed_tests(evidence: &[String]) -> Vec<String> {
    let mut tests = Vec::new();
    for text in evidence {
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.ends_with("--- FAILED") {
                push_unique(
                    &mut tests,
                    trimmed.trim_end_matches("--- FAILED").trim().to_string(),
                );
            } else if let Some(rest) = trimmed.strip_prefix("---- ") {
                if let Some((name, _)) = rest.split_once(" stdout ----") {
                    push_unique(&mut tests, name.trim().to_string());
                }
            }
        }
    }
    tests
}

fn extract_forbidden_fixes(evidence: &[String]) -> Vec<String> {
    let joined = evidence.join("\n").to_lowercase();
    let mut forbidden = Vec::new();
    if joined.contains("format!(\"saved: {}") || joined.contains("format!(\"**saved:** {}") {
        forbidden.push("Do not leave unconditional Saved output for /save; surface the real memory write outcome".to_string());
    }
    if joined.contains("explicit || score") {
        forbidden.push("Do not restore `explicit || score >= threshold`; explicit saves cannot bypass hard quality gates".to_string());
    }
    if joined.contains("duplicate") {
        forbidden.push("Do not accept duplicate memories by threshold; duplicate hard stops must remain rejected".to_string());
    }
    forbidden
}

fn compact_evidence(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    let mut out = trimmed.chars().take(max_chars).collect::<String>();
    out.push_str("\n... [truncated]");
    out
}

fn push_section(out: &mut String, title: &str, items: &[String]) {
    out.push_str(title);
    out.push_str(":\n");
    if items.is_empty() {
        out.push_str("- none\n");
    } else {
        for item in items {
            out.push_str("- ");
            out.push_str(item);
            out.push('\n');
        }
    }
}

fn unique_strings(items: impl Iterator<Item = String>) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        push_unique(&mut result, item);
    }
    result
}

fn push_unique(items: &mut Vec<String>, item: String) {
    let item = item.trim();
    if item.is_empty() {
        return;
    }
    if !items.iter().any(|existing| existing == item) {
        items.push(item.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repair_spec_extracts_failed_tests_and_forbidden_fixes() {
        let evidence = vec![r#"
memory::quality::tests::explicit_does_not_accept_low_quality_note --- FAILED
---- memory::calibration::tests::built_in_calibration_samples_pass stdout ----
let status = if explicit || score >= 0.65 { MemoryStatus::Accepted } else { write_decision.status };
format!("Saved: {}", save_content)
duplicate_project_fact expected Rejected actual Accepted
"#
        .to_string()];
        let spec = RepairSpec::from_failure(
            &["cargo test -q memory -- --test-threads=1".to_string()],
            &evidence,
            None,
        );

        assert!(spec.failed_tests.contains(
            &"memory::quality::tests::explicit_does_not_accept_low_quality_note".to_string()
        ));
        assert!(spec.failed_tests.contains(
            &"memory::calibration::tests::built_in_calibration_samples_pass".to_string()
        ));
        assert!(spec
            .forbidden_fixes
            .iter()
            .any(|item| item.contains("explicit || score")));
        assert!(spec
            .forbidden_fixes
            .iter()
            .any(|item| item.contains("unconditional Saved")));
        assert!(spec.format_for_prompt().contains("RepairSpec:"));
        assert!(spec
            .format_for_prompt()
            .contains("first state the concrete mismatch"));
    }

    #[test]
    fn repair_spec_promotes_required_command_output_to_hard_evidence() {
        let command = r#"! rg '&format!\("retry: \{\}", verification_command\)' src/engine/conversation_loop/mod.rs"#;
        let evidence = vec![format!(
            r#"[required verification] {command} found 1 error(s), 0 warning(s):
  [error] unknown:                         &format!("retry: {{}}", verification_command),
  [required command failed: {command}]"#
        )];

        let spec = RepairSpec::from_failure(&[command.to_string()], &evidence, None);
        let prompt = spec.format_for_prompt();

        assert!(prompt.contains("failed_command_evidence"));
        assert!(prompt.contains("residual_command_matches"));
        assert!(prompt.contains(r#"&format!("retry: {}", verification_command)"#));
        assert!(prompt.contains("Eliminate or explicitly justify residual required-command match"));
        assert!(prompt.contains("Before editing, explain why the current diff still fails"));
        assert!(prompt.contains(command));
    }

    #[test]
    fn repair_spec_carries_tool_record_evidence_separately() {
        let spec = RepairSpec::from_failure_with_tool_records(
            &["cargo test -q".to_string()],
            &["required command failed: cargo test -q".to_string()],
            &[
                "tool record evidence: tool=file_edit status=completed changed_paths=src/lib.rs"
                    .to_string(),
                "tool record evidence: tool=bash status=failed command=cargo test -q".to_string(),
            ],
            None,
        );
        let prompt = spec.format_for_prompt();

        assert_eq!(spec.tool_record_evidence.len(), 2);
        assert!(prompt.contains("tool_record_evidence"));
        assert!(prompt.contains("tool=file_edit"));
        assert!(prompt.contains("Use tool_record_evidence"));
    }

    #[test]
    fn repair_spec_extracts_multiple_required_rg_matches() {
        let command = r#"! rg '&format!\("retry: \{\}", verification_command\)' src/engine/conversation_loop/mod.rs"#;
        let evidence = vec![format!(
            r#"[required verification] {command} found 1 error(s), 0 warning(s):
  [error] unknown:                         &format!("retry: {{}}", verification_command),
  [error] unknown:             concat!("&format!(\"retry: {{", "}}\", verification_command)").to_string(),
  [required command failed: {command}]"#
        )];

        let spec = RepairSpec::from_failure(&[command.to_string()], &evidence, None);

        assert_eq!(spec.residual_command_matches.len(), 2);
        assert!(spec
            .required_next_patch
            .iter()
            .any(|item| item.contains("residual required-command match")));
    }
}
