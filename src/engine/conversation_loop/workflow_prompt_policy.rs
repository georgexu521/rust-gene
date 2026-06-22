//! Conversation-loop controller module.
//!
//! Owns one focused stage of turn execution so permissions, validation, repair, and closeout stay explicit in the runtime.

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
        let mut in_allowed_tools = false;
        let mut in_forbidden_tools = false;
        let mut saw_allowed_tools_section = false;
        let mut allowed_code_write_tools = false;
        let mut forbidden_code_write_tools = std::collections::HashSet::new();

        for line in prompt.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("## ") {
                in_allowed_tools = trimmed.eq_ignore_ascii_case("## Allowed tools");
                in_forbidden_tools = trimmed.eq_ignore_ascii_case("## Forbidden tools")
                    || trimmed.eq_ignore_ascii_case("## Disallowed tools");
                if in_allowed_tools {
                    saw_allowed_tools_section = true;
                }
                continue;
            }
            if !(in_allowed_tools || in_forbidden_tools) || !trimmed.starts_with("- ") {
                continue;
            }
            let tool = trimmed
                .trim_start_matches("- ")
                .trim()
                .trim_matches('`')
                .to_ascii_lowercase();
            if matches!(tool.as_str(), "file_edit" | "file_write" | "file_patch") {
                if in_allowed_tools {
                    allowed_code_write_tools = true;
                } else if in_forbidden_tools {
                    forbidden_code_write_tools.insert(tool);
                }
            }
        }

        if allowed_code_write_tools {
            return false;
        }

        if saw_allowed_tools_section {
            return true;
        }

        if forbidden_code_write_tools.contains("file_edit")
            && (forbidden_code_write_tools.contains("file_write")
                || forbidden_code_write_tools.contains("file_patch"))
        {
            return true;
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
    fn prompt_does_not_forbid_code_writes_when_file_edit_is_allowed() {
        let prompt = r#"
## Allowed tools
- grep
- file_read
- file_edit
- bash

## Forbidden tools
- file_write
- file_patch
- git_push
"#;

        assert!(!WorkflowPromptPolicy::forbids_code_write_tools(prompt));
    }

    #[test]
    fn prompt_forbids_code_writes_when_allowed_tool_section_has_no_write_tool() {
        let prompt = r#"
## Allowed tools
- grep
- file_read
- bash

## Forbidden tools
- file_edit
- file_write
- file_patch
"#;

        assert!(WorkflowPromptPolicy::forbids_code_write_tools(prompt));
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
