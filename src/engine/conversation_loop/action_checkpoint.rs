use super::ConversationLoop;
use std::collections::HashSet;

impl ConversationLoop {
    pub(super) const ACTION_CHECKPOINT_TARGETED_LOOKUP_BUDGET: usize = 2;

    pub(super) fn focused_repair_mode_prompt(
        exposed_names: &[String],
        targeted_lookups_used: usize,
    ) -> String {
        let remaining =
            Self::ACTION_CHECKPOINT_TARGETED_LOOKUP_BUDGET.saturating_sub(targeted_lookups_used);
        let lookup_rule = if remaining == 0 {
            "The targeted lookup budget has already been used; do not call file_read/grep again. Patch from the evidence already gathered."
        } else if targeted_lookups_used == 0 {
            "file_read/grep are allowed only for up to two targeted lookups of a specific symbol, test, or call site; do not repeat broad inspection."
        } else {
            "One targeted file_read/grep lookup remains for a specific missing line range, symbol, test, or call site; after that, patch from the evidence gathered."
        };
        format!(
            "Current tool mode: FOCUSED REPAIR. The exposed tools for this request are: {}. Patch files as soon as the target line is known, using file_edit/file_write or bash only for a mutating patch command. {} Do not call glob/project_list or any tool that is not in the exposed list. Do not use bash for read-only inspection; after a file changes, bash may run validation. If previous validation reported compile/type errors, fix those exact errors first using the latest verification source context. If you have line numbers from earlier grep/file_read/verification output, prefer file_edit with line_start/line_end or exact old_string copied from that current source context. Do not invent enum variants, struct fields, functions, or APIs not visible in prior tool output; reuse existing names exactly. If a scorer/decision object already returns a final status, use that status directly; do not wrap it with explicit/score checks that can bypass safety, volatility, or duplication hard stops.",
            exposed_names.join(", "),
            lookup_rule
        )
    }

    pub(super) fn file_edit_failure_repair_correction(
        failed_tool_evidence: &[String],
    ) -> Option<String> {
        let relevant = failed_tool_evidence
            .iter()
            .filter(|evidence| evidence.contains("file_edit"))
            .filter(|evidence| {
                evidence.contains("Expected 1 occurrence")
                    || evidence.contains("old_string cannot be empty")
                    || evidence.contains("old_string cannot be empty or whitespace-only")
                    || evidence.contains("Action checkpoint file_edit rejected")
                    || evidence.contains("unique edit anchor")
            })
            .take(2)
            .cloned()
            .collect::<Vec<_>>();

        if relevant.is_empty() {
            return None;
        }

        Some(format!(
            "File edit repair correction:\n{}\nNext action is still a patch, not closeout. The previous file_edit did not modify a file because its anchor was empty, whitespace-only, or non-unique. Use one of these safer forms:\n- If prior file_read/grep output shows the target line number, call file_edit with path, line_start, line_end, and new_string for that exact line.\n- Otherwise copy a multi-line old_string that includes the surrounding function call and is unique exactly once.\nDo not retry the same broad old_string. Do not close out until a file_edit/file_write succeeds and validation runs.",
            relevant.join("\n\n")
        ))
    }

    pub(super) fn should_retry_after_file_edit_failure_correction(
        action_checkpoint_active: bool,
        file_edit_failure_correction_added: bool,
        file_edit_failure_retry_used: bool,
        successful_write_tool: bool,
    ) -> bool {
        action_checkpoint_active
            && file_edit_failure_correction_added
            && !file_edit_failure_retry_used
            && !successful_write_tool
    }

    pub(super) fn action_checkpoint_unexposed_tool_message(
        tool_name: &str,
        exposed_tool_names: &HashSet<String>,
    ) -> String {
        let mut exposed = exposed_tool_names.iter().cloned().collect::<Vec<_>>();
        exposed.sort();
        format!(
            "Tool '{tool_name}' was not exposed in the current focused repair request and cannot be executed. Exposed tools: {}. Use file_edit/file_write or a mutating bash command for the patch. Use file_read or grep only for one targeted lookup of a missing symbol, test, or call site. Do not call glob/project_list or repeat broad inspection.",
            exposed.join(", ")
        )
    }

    pub(super) fn bash_allowed_at_action_checkpoint(
        arguments: &serde_json::Value,
        has_changes_before_tools: bool,
    ) -> bool {
        let command = arguments["command"]
            .as_str()
            .unwrap_or_default()
            .to_ascii_lowercase();
        if command.trim().is_empty() {
            return false;
        }
        let mutating_markers = [
            "apply_patch",
            "python",
            "python3",
            "perl -",
            "sed -i",
            "cat >",
            "cat <<",
            "tee ",
            ">>",
            "> ",
            "mv ",
            "cp ",
            "touch ",
        ];
        if mutating_markers
            .iter()
            .any(|marker| command.contains(marker))
        {
            return true;
        }
        let validation_markers = [
            "bash -n",
            "cargo test",
            "cargo check",
            "cargo fmt",
            "cargo clippy",
            "npm test",
            "npm run test",
            "pnpm test",
            "pytest",
            "make test",
            "scripts/run_live_eval.sh",
        ];
        has_changes_before_tools
            && validation_markers
                .iter()
                .any(|marker| command.contains(marker))
    }

    pub(super) fn action_checkpoint_file_edit_rejection(
        arguments: &serde_json::Value,
        cwd: &std::path::Path,
    ) -> Option<String> {
        let path = arguments["path"].as_str().unwrap_or_default().trim();
        if path.is_empty() {
            return Some("file_edit path is empty".to_string());
        }
        let raw_path = std::path::Path::new(path);
        for component in raw_path.components() {
            match component {
                std::path::Component::ParentDir => {
                    return Some(format!(
                        "file_edit path contains parent traversal: {}",
                        path
                    ));
                }
                std::path::Component::Normal(part)
                    if part == ".git" || part == "target" || part == "node_modules" =>
                {
                    return Some(format!(
                        "file_edit path targets ignored/generated directory: {}",
                        path
                    ));
                }
                _ => {}
            }
        }

        let expected_replacements = arguments["expected_replacements"]
            .as_u64()
            .map(|value| value as usize)
            .unwrap_or(1);
        if expected_replacements != 1 {
            return Some(format!(
                "action checkpoint only permits one replacement per file_edit call; got expected_replacements={}. Split the patch into single, reviewable edits.",
                expected_replacements
            ));
        }

        let new_string = arguments["new_string"].as_str().unwrap_or_default();
        if new_string.len() > 20_000 {
            return Some("file_edit new_string is too large for action checkpoint".to_string());
        }

        let old_string = arguments["old_string"].as_str();
        let insert_after = arguments["insert_after"].as_str();
        let insert_before = arguments["insert_before"].as_str();
        let line_start = arguments["line_start"].as_u64().map(|value| value as usize);
        let line_end = arguments["line_end"].as_u64().map(|value| value as usize);

        if let (Some(start), Some(end)) = (line_start, line_end) {
            if start == 0 || end == 0 || start > end {
                return Some(format!(
                    "file_edit line range is invalid: {}..={}",
                    start, end
                ));
            }
            if start != end {
                return Some(format!(
                    "action checkpoint line-range edits must touch exactly one line; got {}..={}. Use exact old_string for larger changes or split into single-line edits.",
                    start, end
                ));
            }
            if end.saturating_sub(start) + 1 > 40 {
                return Some(format!(
                    "action checkpoint line range is too large: {} lines. Use a smaller edit.",
                    end.saturating_sub(start) + 1
                ));
            }
            return None;
        }

        let has_edit_anchor =
            old_string.is_some() || insert_after.is_some() || insert_before.is_some();
        if !has_edit_anchor {
            return Some(
                "file_edit must use old_string, insert_after, insert_before, or line_start/line_end"
                    .to_string(),
            );
        }

        let candidate = if raw_path.is_absolute() {
            raw_path.to_path_buf()
        } else {
            cwd.join(raw_path)
        };
        let canonical_cwd = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
        let Ok(canonical_file) = candidate.canonicalize() else {
            return Some(format!("file_edit target does not exist: {}", path));
        };
        if !canonical_file.starts_with(&canonical_cwd) || !canonical_file.is_file() {
            return Some(format!(
                "file_edit target is outside the working tree: {}",
                path
            ));
        }
        let Ok(content) = std::fs::read_to_string(&canonical_file) else {
            return Some(format!("file_edit target is not readable: {}", path));
        };

        let anchor = old_string
            .or(insert_after)
            .or(insert_before)
            .unwrap_or_default();
        if anchor.trim().is_empty() {
            return Some("file_edit anchor is empty".to_string());
        }
        let count = content.matches(anchor).count();
        if count != 1 {
            return Some(format!(
                "action checkpoint requires a unique edit anchor; found {} occurrence(s). Use a more specific old_string or a small line_start/line_end range.",
                count
            ));
        }

        None
    }
}
