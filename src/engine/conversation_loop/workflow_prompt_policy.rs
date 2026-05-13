pub(super) struct WorkflowPromptPolicy;

impl WorkflowPromptPolicy {
    pub(super) fn allows_no_diff_audit_closeout(prompt: &str) -> bool {
        let lower = prompt.to_ascii_lowercase();
        lower.contains("eval intent: `audit_or_regression_check`")
            || lower.contains("eval intent: audit_or_regression_check")
            || lower.contains("eval intent: `stale_or_already_satisfied`")
            || lower.contains("eval intent: stale_or_already_satisfied")
            || lower.contains("if the requested behavior is already present")
            || lower.contains("do not force an arbitrary edit")
    }

    pub(super) fn forbids_code_write_tools(prompt: &str) -> bool {
        let mut in_forbidden_tools = false;
        for line in prompt.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("## ") {
                in_forbidden_tools = trimmed.eq_ignore_ascii_case("## Forbidden tools")
                    || trimmed.eq_ignore_ascii_case("## Disallowed tools");
                continue;
            }
            if !in_forbidden_tools || !trimmed.starts_with("- ") {
                continue;
            }
            let tool = trimmed
                .trim_start_matches("- ")
                .trim()
                .trim_matches('`')
                .to_ascii_lowercase();
            if matches!(tool.as_str(), "file_edit" | "file_write" | "file_patch") {
                return true;
            }
        }

        let lower = prompt.to_ascii_lowercase();
        lower.contains("do not edit files")
            || lower.contains("do not change files")
            || lower.contains("no file edits")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_eval_allows_no_diff_closeout() {
        let audit_prompt = r#"
# Live coding regression task: memory recall should demote only relevant conflicts
- Eval intent: `audit_or_regression_check`
## Closeout requirements
- This is an audit/regression evaluation. If the requested behavior is already present, prove it with direct evidence and required commands instead of forcing an arbitrary edit.
"#;

        assert!(WorkflowPromptPolicy::allows_no_diff_audit_closeout(
            audit_prompt
        ));
        assert!(!WorkflowPromptPolicy::allows_no_diff_audit_closeout(
            "- Eval intent: `seeded_code_change`\n- This is a real code-change evaluation."
        ));
    }

    #[test]
    fn prompt_forbids_code_write_tools_from_live_eval_block() {
        let prompt = r#"
## Forbidden tools
- file_edit
- file_write
- git_push
"#;

        assert!(WorkflowPromptPolicy::forbids_code_write_tools(prompt));
        assert!(!WorkflowPromptPolicy::forbids_code_write_tools(
            "## Forbidden tools\n- git_push\n"
        ));
    }

    #[test]
    fn prompt_forbids_code_write_tools_from_plain_text_constraints() {
        assert!(WorkflowPromptPolicy::forbids_code_write_tools(
            "Do not edit files; only inspect the project."
        ));
        assert!(WorkflowPromptPolicy::forbids_code_write_tools(
            "No file edits are allowed in this audit."
        ));
        assert!(!WorkflowPromptPolicy::forbids_code_write_tools(
            "Inspect first, then edit files if needed."
        ));
    }
}
