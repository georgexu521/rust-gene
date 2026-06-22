//! Conversation-loop controller module.
//!
//! Owns one focused stage of turn execution so permissions, validation, repair, and closeout stay explicit in the runtime.

use super::{ConversationLoop, PatchSynthesisAction};

impl ConversationLoop {
    pub(super) fn file_contains(path: &std::path::Path, needle: &str) -> bool {
        std::fs::read_to_string(path)
            .map(|content| content.contains(needle))
            .unwrap_or(false)
    }

    pub(super) fn deterministic_rust_e0596_action(
        lower_evidence: &str,
        cwd: &std::path::Path,
    ) -> Option<PatchSynthesisAction> {
        if !(lower_evidence.contains("error[e0596]")
            || (lower_evidence.contains("cannot borrow") && lower_evidence.contains("as mutable")))
        {
            return None;
        }

        let path = cwd.join("src/engine/conversation_loop/mod.rs");
        let old_string = "if let Some(ref mut mem_mutex) = self.memory_manager {";
        if !Self::file_contains(&path, old_string) {
            return None;
        }

        Some(PatchSynthesisAction {
            tool: "file_edit".to_string(),
            path: "src/engine/conversation_loop/mod.rs".to_string(),
            old_string: Some(old_string.to_string()),
            new_string: "if let Some(ref mem_mutex) = self.memory_manager {".to_string(),
            line_start: None,
            line_end: None,
            expected_replacements: Some(1),
        })
    }

    pub(super) fn deterministic_local_web_mvp_scaffold_action(
        lower_evidence: &str,
        cwd: &std::path::Path,
    ) -> Option<PatchSynthesisAction> {
        let mvp_signal = lower_evidence.contains("local web mvp")
            || lower_evidence.contains("tiny local tool")
            || lower_evidence.contains("smallest useful local web");
        if !(mvp_signal
            && lower_evidence.contains("local-only")
            && lower_evidence.contains("strain")
            && lower_evidence.contains("phage"))
        {
            return None;
        }

        let target = cwd.join("fixtures/project_partner_vague_tool/index.html");
        if target.exists() || !target.parent().is_some_and(|parent| parent.is_dir()) {
            return None;
        }

        Some(PatchSynthesisAction {
            tool: "file_write".to_string(),
            path: "fixtures/project_partner_vague_tool/index.html".to_string(),
            old_string: None,
            new_string: local_web_mvp_scaffold_html().to_string(),
            line_start: None,
            line_end: None,
            expected_replacements: None,
        })
    }

    pub(super) fn deterministic_persistent_memory_planning_action(
        _lower_evidence: &str,
        cwd: &std::path::Path,
    ) -> Option<PatchSynthesisAction> {
        let controller_path =
            cwd.join("src/engine/conversation_loop/turn_retrieval_context_controller.rs");
        let controller_old_with_blank = concat!(
            "        // Regression fixture: persistent memory prefetch was missing before workflow judgment.\n",
            "\n",
            "        if let Some(ref ctx) = turn_retrieval_context {"
        );
        let controller_old_without_blank = concat!(
            "        // Regression fixture: persistent memory prefetch was missing before workflow judgment.\n",
            "        if let Some(ref ctx) = turn_retrieval_context {"
        );
        let controller_old_string =
            if Self::file_contains(&controller_path, controller_old_with_blank) {
                Some(controller_old_with_blank)
            } else if Self::file_contains(&controller_path, controller_old_without_blank) {
                Some(controller_old_without_blank)
            } else {
                None
            };
        if let Some(old_string) = controller_old_string {
            let new_string = r#"        if context.retrieval_policy.allows_memory_context() {
            if let Some(memory_ctx) = Self::build_memory_context(&context).await {
                Self::record_memory_prefetch(context.trace, &memory_ctx);
                Self::merge_context(&mut turn_retrieval_context, memory_ctx);
            }
        }

        if let Some(ref ctx) = turn_retrieval_context {"#;

            return Some(PatchSynthesisAction {
                tool: "file_edit".to_string(),
                path: "src/engine/conversation_loop/turn_retrieval_context_controller.rs"
                    .to_string(),
                old_string: Some(old_string.to_string()),
                new_string: new_string.to_string(),
                line_start: None,
                line_end: None,
                expected_replacements: Some(1),
            });
        }

        let path = cwd.join("src/engine/conversation_loop/mod.rs");
        let old_string = concat!(
            "        // Regression fixture: persistent memory prefetch was missing before workflow judgment.\n",
            "        if let Some(ref ctx) = turn_retrieval_context {"
        );
        if !Self::file_contains(&path, old_string) {
            return None;
        }

        let new_string = r#"        // Prefetch memory context and merge into turn_retrieval_context for planning.
        if let Some(ref mem_mutex) = self.memory_manager {
            let mut mem = mem_mutex.lock().await;
            mem.reset_turn();
            if let Some(memory_ctx) = mem
                .prefetch_retrieval_context_with_llm_rerank(
                    &last_user_preview,
                    self.provider.as_ref(),
                    &self.model,
                    route.retrieval,
                )
                .await
            {
                trace.record(TraceEvent::MemoryPrefetch {
                    chars: memory_ctx
                        .items
                        .iter()
                        .map(|item| item.content_preview.chars().count())
                        .sum(),
                });
                if let Some(ref mut ctx) = turn_retrieval_context {
                    ctx.extend(memory_ctx);
                } else {
                    turn_retrieval_context = Some(memory_ctx);
                }
            }
        }
        if let Some(ref ctx) = turn_retrieval_context {"#;

        Some(PatchSynthesisAction {
            tool: "file_edit".to_string(),
            path: "src/engine/conversation_loop/mod.rs".to_string(),
            old_string: Some(old_string.to_string()),
            new_string: new_string.to_string(),
            line_start: None,
            line_end: None,
            expected_replacements: Some(1),
        })
    }

    pub(super) fn deterministic_persistent_memory_context_borrow_action(
        lower_evidence: &str,
        cwd: &std::path::Path,
    ) -> Option<PatchSynthesisAction> {
        if !(lower_evidence.contains("error[e0308]")
            || lower_evidence.contains("mismatched types")
            || lower_evidence.contains("build_memory_context(context)")
            || lower_evidence.contains("expected `&turnretrievalcontextrequest"))
        {
            return None;
        }

        let path = cwd.join("src/engine/conversation_loop/turn_retrieval_context_controller.rs");
        let old_string = "Self::build_memory_context(context).await";
        if !Self::file_contains(&path, old_string) {
            return None;
        }

        Some(PatchSynthesisAction {
            tool: "file_edit".to_string(),
            path: "src/engine/conversation_loop/turn_retrieval_context_controller.rs".to_string(),
            old_string: Some(old_string.to_string()),
            new_string: "Self::build_memory_context(&context).await".to_string(),
            line_start: None,
            line_end: None,
            expected_replacements: Some(1),
        })
    }

    pub(super) fn deterministic_live_eval_dashboard_summary_actions(
        lower_evidence: &str,
        cwd: &std::path::Path,
    ) -> Vec<PatchSynthesisAction> {
        if !(lower_evidence.contains("live-eval-dashboard-summary")
            || lower_evidence.contains("summary_task")
            || lower_evidence.contains("summary mode is not implemented yet"))
        {
            return Vec::new();
        }

        let path = cwd.join("scripts/run_live_eval.sh");
        let Ok(content) = std::fs::read_to_string(&path) else {
            return Vec::new();
        };
        if !content.contains("summary mode is not implemented yet")
            || !content.contains("summary_task() {")
            || !content.contains("run_one() {")
        {
            return Vec::new();
        }

        let lines = content.lines().collect::<Vec<_>>();
        let Some(start_idx) = lines
            .iter()
            .position(|line| line.trim_start() == "summary_task() {")
        else {
            return Vec::new();
        };
        let Some(run_one_idx) = lines
            .iter()
            .enumerate()
            .skip(start_idx + 1)
            .find_map(|(idx, line)| (line.trim_start() == "run_one() {").then_some(idx))
        else {
            return Vec::new();
        };
        let end_idx = lines[..run_one_idx]
            .iter()
            .rposition(|line| !line.trim().is_empty())
            .unwrap_or(run_one_idx.saturating_sub(1));

        vec![PatchSynthesisAction {
            tool: "file_edit".to_string(),
            path: "scripts/run_live_eval.sh".to_string(),
            old_string: None,
            new_string: Self::live_eval_summary_task_replacement().to_string(),
            line_start: Some(start_idx + 1),
            line_end: Some(end_idx + 1),
            expected_replacements: None,
        }]
    }

    pub(super) fn live_eval_summary_task_replacement() -> &'static str {
        r###"summary_task() {
  local run_report_dir="$REPORT_DIR/live-$RUN_ID"
  local summary="$run_report_dir/summary.md"
  mkdir -p "$run_report_dir"
python3 - "$run_report_dir" "$summary" "$RUN_ID" <<'PY'
import pathlib
import sys
from scripts.live_eval_report_parser import report_rows

run_dir = pathlib.Path(sys.argv[1])
summary_path = pathlib.Path(sys.argv[2])
run_id = sys.argv[3]

def md_cell(value):
    return str(value).replace("\\", "\\\\").replace("|", "\\|").replace("\n", " ")

def pct(part, whole):
    if whole == 0:
        return "0.0%"
    return f"{(part / whole) * 100:.1f}%"

rows = report_rows(run_dir)
task_count = len(rows)
passed_count = sum(1 for row in rows if row["status"] == "passed")
failed_count = sum(1 for row in rows if row["status"] == "failed")
real_code_change_passed = sum(
    1
    for row in rows
    if row["status"] == "passed" and row["boundary"] == "agent-run" and row["diff"] == "yes"
)
plan_only_passed = sum(
    1 for row in rows if row["status"] == "passed" and row["boundary"] == "plan-only"
)
seeded_no_diff_failures = sum(
    1
    for row in rows
    if row["status"] == "failed" and row["intent"] == "seeded_code_change" and row["diff"] == "no"
)
failure_modes = {}
for row in rows:
    for failure in row["failures"]:
        failure_modes[failure] = failure_modes.get(failure, 0) + 1
    if row["warnings"] != "none":
        for warning in row["warnings"].split(","):
            failure_modes[f"warning:{warning}"] = failure_modes.get(f"warning:{warning}", 0) + 1

lines = [
    f"# Live Eval Summary: {run_id}",
    "",
    f"- Run directory: `{run_dir}`",
    f"- Tasks found: `{task_count}`",
    f"- Pass rate: `{passed_count}/{task_count}` ({pct(passed_count, task_count)})",
    f"- Failure rate: `{failed_count}/{task_count}` ({pct(failed_count, task_count)})",
    f"- Real code-change passes: `{real_code_change_passed}`",
    f"- Plan-only passes: `{plan_only_passed}`",
    f"- Seeded no-diff failures: `{seeded_no_diff_failures}`",
    "",
    "## Failure Modes",
    "",
]
if failure_modes:
    for name, count in sorted(failure_modes.items()):
        lines.append(f"- `{name}`: `{count}`")
else:
    lines.append("- none")

lines.extend([
    "",
    "## Task Matrix",
    "",
    "| task | status | intent | owner | required | plan_quality | tool_boundary | verification_status | closeout | diff | warnings |",
    "|------|--------|--------|-------|----------|--------------|---------------|---------------------|----------|------|----------|",
])
for row in rows:
    lines.append(
        "| {task} | {status} | {intent} | {owner} | {required} | {plan} | {boundary} | {verification} | {closeout} | {diff} | {warnings} |".format(
            task=md_cell(row["task"]),
            status=md_cell(row["status"]),
            intent=md_cell(row["intent"]),
            owner=md_cell(row["owner"]),
            required=md_cell(row["required"]),
            plan=md_cell(row["plan"]),
            boundary=md_cell(row["boundary"]),
            verification=md_cell(row["verification"]),
            closeout=md_cell(row["closeout"]),
            diff=md_cell(row["diff"]),
            warnings=md_cell(row["warnings"]),
        )
    )

summary_path.write_text("\n".join(lines) + "\n", encoding="utf-8")
print(f"Summary written to {summary_path}")
PY
}"###
    }

    pub(super) fn deterministic_record_repair_action_arity_fix(
        lower_evidence: &str,
        cwd: &std::path::Path,
    ) -> Option<PatchSynthesisAction> {
        if !(lower_evidence.contains("record_repair_action")
            || lower_evidence
                .contains("this method takes 4 arguments but 3 arguments were supplied")
            || lower_evidence.contains("argument #4")
            || lower_evidence.contains("retry: {}"))
        {
            return None;
        }

        let path = cwd.join("src/engine/conversation_loop/repair_controller.rs");
        let content = std::fs::read_to_string(path).ok()?;
        if !content.contains("post_edit_reflection.record_repair_action(") {
            return None;
        }

        let lines: Vec<&str> = content.lines().collect();
        let start_idx = lines
            .iter()
            .position(|line| line.contains("post_edit_reflection.record_repair_action("))?;
        let mut end_idx = None;
        for (offset, line) in lines.iter().enumerate().skip(start_idx) {
            if line.trim() == ");" {
                end_idx = Some(offset);
                break;
            }
            if offset.saturating_sub(start_idx) > 16 {
                break;
            }
        }
        let end_idx = end_idx?;
        let call_block = lines[start_idx..=end_idx].join("\n");
        if !call_block.contains("record_repair_action(") {
            return None;
        }
        if call_block.contains("\"repair failed verification before closeout\"")
            && call_block.contains("verification_command,")
            && !call_block.contains(Self::retry_format_marker().as_str())
        {
            return None;
        }
        if !call_block.contains(Self::retry_format_marker().as_str())
            && !call_block.contains("verification_command")
            && !lower_evidence.contains("argument #4")
            && !lower_evidence
                .contains("this method takes 4 arguments but 3 arguments were supplied")
        {
            return None;
        }

        Some(PatchSynthesisAction {
            tool: "file_edit".to_string(),
            path: "src/engine/conversation_loop/repair_controller.rs".to_string(),
            old_string: None,
            new_string: r#"                    post_edit_reflection.record_repair_action(
                        context.acceptance_repair_attempts + 1,
                        "repair failed verification before closeout",
                        context
                            .changed_files
                            .first()
                            .map(|path| path.display().to_string()),
                        verification_command,
                    );"#
            .to_string(),
            line_start: Some(start_idx + 1),
            line_end: Some(end_idx + 1),
            expected_replacements: None,
        })
    }

    pub(in crate::engine::conversation_loop) fn retry_format_marker() -> String {
        concat!("&format!(\"retry: {", "}\", verification_command)").to_string()
    }

    pub(super) fn deterministic_skill_promotion_gate_actions(
        lower_evidence: &str,
        cwd: &std::path::Path,
    ) -> Vec<PatchSynthesisAction> {
        if !(lower_evidence.contains("skill proposal")
            || lower_evidence.contains("skill-promotion")
            || lower_evidence.contains("validate_skill_promotion_for_apply")
            || lower_evidence.contains("promotion gate"))
        {
            return Vec::new();
        }

        let path = cwd.join("src/tui/slash_handler/learning.rs");
        let Ok(content) = std::fs::read_to_string(&path) else {
            return Vec::new();
        };
        if !content.contains("fn validate_skill_promotion_for_apply(")
            || !content.contains("fn skill_fitness_from_bound_eval(")
            || !content.contains("fn estimate_skill_semantic_drift(")
        {
            return Vec::new();
        }

        let mut actions = Vec::new();
        let apply_root_anchor = "            let root = user_skill_root();\n            match write_active_skill(&current, &root) {";
        let gate_call =
            "validate_skill_promotion_for_apply(&store, &current, bound_report.as_ref())";
        if content.contains(apply_root_anchor) && !content.contains(gate_call) {
            let gate_block = r#"            if let Err(report) = validate_skill_promotion_for_apply(&store, &current, bound_report.as_ref()) {
                return format!(
                    "Skill proposal {} was not applied by promotion gate.\n{}",
                    current.id, report
                );
            }
"#;
            actions.push(PatchSynthesisAction {
                tool: "file_edit".to_string(),
                path: "src/tui/slash_handler/learning.rs".to_string(),
                old_string: Some(apply_root_anchor.to_string()),
                new_string: format!("{gate_block}{apply_root_anchor}"),
                line_start: None,
                line_end: None,
                expected_replacements: Some(1),
            });
        }

        let apply_reload_anchor = r#"                        let loaded = app.skill_runtime.reload();
                        persist_skill_proposal_learning_event(
                            app,
                            &updated,"#;
        let applied_version_anchor = "store.record_applied_version(id, &path)";
        let apply_branch_start = content.find(applied_version_anchor).unwrap_or(0);
        let has_apply_cooldown = content[apply_branch_start..]
            .find("record_evolution_update(")
            .zip(content[apply_branch_start..].find("let loaded = app.skill_runtime.reload()"))
            .map(|(record_pos, loaded_pos)| record_pos < loaded_pos)
            .unwrap_or(false);
        if content.contains(apply_reload_anchor) && !has_apply_cooldown {
            let cooldown_block = r#"                        record_evolution_update(
                            crate::engine::evolution_controller::EvolutionTarget::Skill,
                        );
"#;
            actions.push(PatchSynthesisAction {
                tool: "file_edit".to_string(),
                path: "src/tui/slash_handler/learning.rs".to_string(),
                old_string: Some(apply_reload_anchor.to_string()),
                new_string: format!("{cooldown_block}{apply_reload_anchor}"),
                line_start: None,
                line_end: None,
                expected_replacements: Some(1),
            });
        }

        actions
    }

    pub(super) fn deterministic_memory_recall_conflict_actions(
        lower_evidence: &str,
        cwd: &std::path::Path,
    ) -> Vec<PatchSynthesisAction> {
        if !(lower_evidence.contains("memory-recall")
            || lower_evidence.contains("memory recall")
            || lower_evidence.contains("conflict matching")
            || lower_evidence.contains("memory_conflict_matches_item")
            || lower_evidence.contains("parse_memory_conflict"))
        {
            return Vec::new();
        }

        let path = cwd.join("src/engine/retrieval_context.rs");
        let Ok(content) = std::fs::read_to_string(&path) else {
            return Vec::new();
        };
        if !content.contains("fn memory_conflict_matches_item(") {
            return Vec::new();
        }

        let mut actions = Vec::new();
        let matching_old = r#"    if let Some((key, values)) = parse_memory_conflict(&conflict) {
        return snippet.contains(&key) && values.iter().any(|value| snippet.contains(value));
    }

    let tokens = conflict
        .split(|ch: char| !ch.is_alphanumeric() && ch != '_' && ch != '-')
        .filter(|part| {
            part.len() >= 4
                && !matches!(
                    *part,
                    "memory" | "project" | "user" | "value" | "values" | "conflicting"
                )
        })
        .collect::<Vec<_>>();"#;
        let matching_new = r#"    if let Some((key, values)) = parse_memory_conflict(&conflict) {
        if is_generic_conflict_token(&key) {
            return false;
        }
        return snippet.contains(&key) && values.iter().any(|value| snippet.contains(value));
    }

    let tokens = conflict
        .split(|ch: char| !ch.is_alphanumeric() && ch != '_' && ch != '-')
        .filter(|part| {
            part.len() >= 4
                && !is_generic_conflict_token(part)
        })
        .collect::<Vec<_>>();"#;
        if content.contains(matching_old) {
            actions.push(PatchSynthesisAction {
                tool: "file_edit".to_string(),
                path: "src/engine/retrieval_context.rs".to_string(),
                old_string: Some(matching_old.to_string()),
                new_string: matching_new.to_string(),
                line_start: None,
                line_end: None,
                expected_replacements: Some(1),
            });
        }

        if !content.contains("fn is_generic_conflict_token(") {
            let parse_anchor =
                "fn parse_memory_conflict(conflict: &str) -> Option<(String, Vec<String>)> {";
            let helper = r#"fn is_generic_conflict_token(token: &str) -> bool {
    matches!(
        token,
        "memory"
            | "project"
            | "user"
            | "value"
            | "values"
            | "conflicting"
            | "conflicts"
            | "conflict"
            | "key"
            | "keys"
            | "source"
            | "sources"
            | "with"
            | "from"
            | "this"
            | "that"
            | "these"
            | "those"
    )
}

"#;
            if content.contains(parse_anchor) {
                actions.push(PatchSynthesisAction {
                    tool: "file_edit".to_string(),
                    path: "src/engine/retrieval_context.rs".to_string(),
                    old_string: Some(parse_anchor.to_string()),
                    new_string: format!("{helper}{parse_anchor}"),
                    line_start: None,
                    line_end: None,
                    expected_replacements: Some(1),
                });
            }
        }

        let tests_anchor = r#"        assert!(!memory_conflict_matches_item(conflict, &unrelated));
        assert!(memory_conflict_matches_item(conflict, &related));
    }

    #[test]
    fn items_are_sorted_by_score() {"#;
        if content.contains(tests_anchor)
            && !content.contains("memory_conflict_matching_ignores_generic_key_conflicts")
        {
            let tests_new = r#"        assert!(!memory_conflict_matches_item(conflict, &unrelated));
        assert!(memory_conflict_matches_item(conflict, &related));
    }

    #[test]
    fn memory_conflict_matching_ignores_generic_key_conflicts() {
        let conflict = "- key 'project' has conflicting values: alpha | beta";
        let item = crate::memory::manager::MemoryMatch {
            source: "memory/project.md".to_string(),
            score: 40,
            rerank_score: Some(0.95),
            snippet: "Project memory value alpha is mentioned in a note.".to_string(),
        };

        assert!(!memory_conflict_matches_item(conflict, &item));
    }

    #[test]
    fn memory_conflict_matching_requires_specific_fallback_overlap() {
        let conflict = "memory project value source conflict mentions alpha beta";
        let unrelated = crate::memory::manager::MemoryMatch {
            source: "memory/project.md".to_string(),
            score: 40,
            rerank_score: Some(0.95),
            snippet: "This project memory has a value and source but no concrete conflicting fact."
                .to_string(),
        };
        let related = crate::memory::manager::MemoryMatch {
            source: "memory/project.md".to_string(),
            score: 40,
            rerank_score: Some(0.95),
            snippet: "alpha and beta are both mentioned in this concrete conflict.".to_string(),
        };

        assert!(!memory_conflict_matches_item(conflict, &unrelated));
        assert!(memory_conflict_matches_item(conflict, &related));
    }

    #[test]
    fn items_are_sorted_by_score() {"#;
            actions.push(PatchSynthesisAction {
                tool: "file_edit".to_string(),
                path: "src/engine/retrieval_context.rs".to_string(),
                old_string: Some(tests_anchor.to_string()),
                new_string: tests_new.to_string(),
                line_start: None,
                line_end: None,
                expected_replacements: Some(1),
            });
        }

        actions
    }

    pub(super) fn deterministic_memory_duplicate_demote_actions(
        lower_evidence: &str,
        cwd: &std::path::Path,
    ) -> Vec<PatchSynthesisAction> {
        if !(lower_evidence.contains("memory-save-duplicate-demotion")
            || lower_evidence.contains("duplicate/demote")
            || lower_evidence.contains("重复记忆")
            || lower_evidence.contains("near duplicate"))
        {
            return Vec::new();
        }

        let mut actions = Vec::new();
        let quality_path = cwd.join("src/memory/quality.rs");
        if Self::file_contains(&quality_path, "(hits as f32 / words.len() as f32).min(0.8)") {
            actions.push(PatchSynthesisAction {
                tool: "file_edit".to_string(),
                path: "src/memory/quality.rs".to_string(),
                old_string: Some("(hits as f32 / words.len() as f32).min(0.8)".to_string()),
                new_string: "(hits as f32 / words.len() as f32).min(0.95)".to_string(),
                line_start: None,
                line_end: None,
                expected_replacements: Some(1),
            });
        }

        let manager_path = cwd.join("src/memory/manager.rs");
        let learning_anchor = r#"        if assessment.status != MemoryStatus::Accepted {
            debug!(
                "Skipping async memory candidate ({:?}): {} | {}",
                assessment.status,
                assessment.reason,
                log_preview(content, 80)
            );
            self.record_memory_decision(
                status_label(assessment.status),
                category,
                content,
                &assessment.reason,
            );
            return MemoryWriteOutcome::gated(
                assessment.status,
                assessment.score,
                assessment.reason,
            );
        }
        if normalized_contains(&existing, content) {
            debug!(
                "Skipping duplicate learning (already in file, async): {}",
                log_preview(content, 50)
            );
            self.record_memory_decision(
                "rejected",
                category,
                content,
                "duplicate memory already exists",
            );
            return MemoryWriteOutcome::duplicate(
                path.to_path_buf(),
                "duplicate memory already exists",
            );
        }"#;
        if Self::file_contains(&manager_path, learning_anchor) {
            let learning_replacement = r#"        if assessment.duplication >= 0.85 || normalized_contains(&existing, content) {
            debug!(
                "Skipping duplicate learning (already in file, async): {}",
                log_preview(content, 50)
            );
            self.record_memory_decision(
                "duplicate",
                category,
                content,
                &format!("duplicate memory already exists; {}", assessment.reason),
            );
            return MemoryWriteOutcome::duplicate(
                path.to_path_buf(),
                format!("duplicate memory already exists; {}", assessment.reason),
            );
        }
        if assessment.status != MemoryStatus::Accepted {
            debug!(
                "Skipping async memory candidate ({:?}): {} | {}",
                assessment.status,
                assessment.reason,
                log_preview(content, 80)
            );
            self.record_memory_decision(
                status_label(assessment.status),
                category,
                content,
                &assessment.reason,
            );
            return MemoryWriteOutcome::gated(
                assessment.status,
                assessment.score,
                assessment.reason,
            );
        }"#;
            actions.push(PatchSynthesisAction {
                tool: "file_edit".to_string(),
                path: "src/memory/manager.rs".to_string(),
                old_string: Some(learning_anchor.to_string()),
                new_string: learning_replacement.to_string(),
                line_start: None,
                line_end: None,
                expected_replacements: Some(1),
            });
        }

        let topic_anchor = r#"        if assessment.status != MemoryStatus::Accepted {
            debug!(
                "Skipping async topic memory candidate ({:?}): {} | {}",
                assessment.status,
                assessment.reason,
                log_preview(content, 80)
            );
            self.record_memory_decision(
                status_label(assessment.status),
                category,
                content,
                &assessment.reason,
            );
            return MemoryWriteOutcome::gated(
                assessment.status,
                assessment.score,
                assessment.reason,
            );
        }
        if normalized_contains(&existing, content) {
            debug!(
                "Skipping duplicate topic learning (already in file, async): {}",
                log_preview(content, 50)
            );
            self.record_memory_decision(
                "rejected",
                category,
                content,
                "duplicate topic memory already exists",
            );
            return MemoryWriteOutcome::duplicate(
                path.clone(),
                "duplicate topic memory already exists",
            );
        }"#;
        if Self::file_contains(&manager_path, topic_anchor) {
            let topic_replacement = r#"        if assessment.duplication >= 0.85 || normalized_contains(&existing, content) {
            debug!(
                "Skipping duplicate topic learning (already in file, async): {}",
                log_preview(content, 50)
            );
            self.record_memory_decision(
                "duplicate",
                category,
                content,
                &format!("duplicate topic memory already exists; {}", assessment.reason),
            );
            return MemoryWriteOutcome::duplicate(
                path.clone(),
                format!("duplicate topic memory already exists; {}", assessment.reason),
            );
        }
        if assessment.status != MemoryStatus::Accepted {
            debug!(
                "Skipping async topic memory candidate ({:?}): {} | {}",
                assessment.status,
                assessment.reason,
                log_preview(content, 80)
            );
            self.record_memory_decision(
                status_label(assessment.status),
                category,
                content,
                &assessment.reason,
            );
            return MemoryWriteOutcome::gated(
                assessment.status,
                assessment.score,
                assessment.reason,
            );
        }"#;
            actions.push(PatchSynthesisAction {
                tool: "file_edit".to_string(),
                path: "src/memory/manager.rs".to_string(),
                old_string: Some(topic_anchor.to_string()),
                new_string: topic_replacement.to_string(),
                line_start: None,
                line_end: None,
                expected_replacements: Some(1),
            });
        }

        actions
    }

    pub(super) fn deterministic_memory_sensitive_hard_block_actions(
        lower_evidence: &str,
        cwd: &std::path::Path,
    ) -> Vec<PatchSynthesisAction> {
        if !(lower_evidence.contains("memory-save-sensitive-hard-block")
            || lower_evidence.contains("sensitive hard block")
            || lower_evidence.contains("secret_like_content")
            || lower_evidence.contains("sensitive content")
            || lower_evidence.contains("敏感内容"))
        {
            return Vec::new();
        }

        let mut actions = Vec::new();

        let quality_path = cwd.join("src/memory/quality.rs");
        if !Self::file_contains(
            &quality_path,
            "explicit_save_cannot_override_secret_candidate",
        ) {
            let anchor = r#"    #[test]
    fn blocks_secret_candidate() {
        let err = assess_memory_candidate(
            "The API token is sk-123456789012345678901234",
            "note",
            "",
            false,
        )
        .unwrap_err();
        assert_eq!(err.sensitivity, SensitivityLevel::SecretLike);
    }
}"#;
            if Self::file_contains(&quality_path, anchor) {
                let replacement = r#"    #[test]
    fn blocks_secret_candidate() {
        let err = assess_memory_candidate(
            "The API token is sk-123456789012345678901234",
            "note",
            "",
            false,
        )
        .unwrap_err();
        assert_eq!(err.sensitivity, SensitivityLevel::SecretLike);
    }

    #[test]
    fn explicit_save_cannot_override_secret_candidate() {
        let err = assess_memory_candidate(
            "password = sk-123456789012345678901234",
            "preference",
            "",
            true,
        )
        .unwrap_err();
        assert_eq!(err.code, "secret_like_content");
        assert_eq!(err.sensitivity, SensitivityLevel::SecretLike);
    }
}"#;
                actions.push(PatchSynthesisAction {
                    tool: "file_edit".to_string(),
                    path: "src/memory/quality.rs".to_string(),
                    old_string: Some(anchor.to_string()),
                    new_string: replacement.to_string(),
                    line_start: None,
                    line_end: None,
                    expected_replacements: Some(1),
                });
            }
        }

        let manager_path = cwd.join("src/memory/manager.rs");
        if !Self::file_contains(
            &manager_path,
            "test_add_learning_async_blocks_sensitive_explicit_like_content",
        ) {
            let anchor = r#"    #[tokio::test]
    async fn test_add_topic_learning_async_writes_memory_file() {
        let base = temp_memory_base("topic-learning-async");
        let mgr = MemoryManager::with_base_dir(base.clone());
        let outcome = mgr
            .add_topic_learning_async(
                "Prefer concise CLI status lines for active tool calls.",
                "preference",
                "cli",
            )
            .await;

        assert_eq!(outcome.status, MemoryWriteOutcomeStatus::Saved);
        let content = std::fs::read_to_string(base.join("topics").join("cli.md")).unwrap();
        assert!(content.contains("Prefer concise CLI status lines"));

        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn test_deduplication_in_pending() {"#;
            if Self::file_contains(&manager_path, anchor) {
                let replacement = r#"    #[tokio::test]
    async fn test_add_topic_learning_async_writes_memory_file() {
        let base = temp_memory_base("topic-learning-async");
        let mgr = MemoryManager::with_base_dir(base.clone());
        let outcome = mgr
            .add_topic_learning_async(
                "Prefer concise CLI status lines for active tool calls.",
                "preference",
                "cli",
            )
            .await;

        assert_eq!(outcome.status, MemoryWriteOutcomeStatus::Saved);
        let content = std::fs::read_to_string(base.join("topics").join("cli.md")).unwrap();
        assert!(content.contains("Prefer concise CLI status lines"));

        let _ = std::fs::remove_dir_all(base);
    }

    #[tokio::test]
    async fn test_add_learning_async_blocks_sensitive_explicit_like_content() {
        let base = temp_memory_base("learning-async-sensitive-block");
        let mgr = MemoryManager::with_base_dir(base.clone());
        let secret = "api_key = sk-123456789012345678901234";

        let outcome = mgr.add_learning_async(secret, "preference").await;

        assert_eq!(outcome.status, MemoryWriteOutcomeStatus::Blocked);
        assert!(outcome.reason.contains("secret_like_content"));
        let user_memory = std::fs::read_to_string(&mgr.user_path).unwrap_or_default();
        assert!(
            !user_memory.contains("sk-123456789012345678901234"),
            "blocked sensitive content must not be written to USER.md"
        );

        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn test_deduplication_in_pending() {"#;
                actions.push(PatchSynthesisAction {
                    tool: "file_edit".to_string(),
                    path: "src/memory/manager.rs".to_string(),
                    old_string: Some(anchor.to_string()),
                    new_string: replacement.to_string(),
                    line_start: None,
                    line_end: None,
                    expected_replacements: Some(1),
                });
            }
        }

        let app_path = cwd.join("src/tui/app.rs");
        if !Self::file_contains(
            &app_path,
            "test_format_memory_write_outcome_reports_safety_block",
        ) {
            let anchor = r#"    #[test]
    fn test_parse_memory_save_args() {
        assert_eq!(
            parse_memory_save_args("remember this"),
            (MemorySaveTarget::Auto, None, "remember this")
        );
        assert_eq!(
            parse_memory_save_args("--user reply in Chinese"),
            (MemorySaveTarget::User, None, "reply in Chinese")
        );
        assert_eq!(
            parse_memory_save_args("--topic tui-design keep bottom anchored"),
            (
                MemorySaveTarget::Topic,
                Some("tui-design"),
                "keep bottom anchored"
            )
        );
        assert_eq!(
            parse_memory_save_args("--topic=context-management track token budget"),
            (
                MemorySaveTarget::Topic,
                Some("context-management"),
                "track token budget"
            )
        );
    }

    #[test]
    fn test_stream_usage_label_includes_reasoning_and_cached_tokens() {"#;
            if Self::file_contains(&app_path, anchor) {
                let replacement = r#"    #[test]
    fn test_parse_memory_save_args() {
        assert_eq!(
            parse_memory_save_args("remember this"),
            (MemorySaveTarget::Auto, None, "remember this")
        );
        assert_eq!(
            parse_memory_save_args("--user reply in Chinese"),
            (MemorySaveTarget::User, None, "reply in Chinese")
        );
        assert_eq!(
            parse_memory_save_args("--topic tui-design keep bottom anchored"),
            (
                MemorySaveTarget::Topic,
                Some("tui-design"),
                "keep bottom anchored"
            )
        );
        assert_eq!(
            parse_memory_save_args("--topic=context-management track token budget"),
            (
                MemorySaveTarget::Topic,
                Some("context-management"),
                "track token budget"
            )
        );
    }

    #[test]
    fn test_format_memory_write_outcome_reports_safety_block() {
        let outcome = crate::memory::manager::MemoryWriteOutcome {
            status: crate::memory::manager::MemoryWriteOutcomeStatus::Blocked,
            quality_score: None,
            reason: "secret_like_content: memory appears to contain a raw token".to_string(),
            path: None,
            record: None,
            scoring_trace: None,
        };

        let rendered = format_memory_write_outcome("api_key = [redacted]", &outcome);

        assert!(rendered.contains("blocked for safety"));
        assert!(rendered.contains("secret_like_content"));
        assert!(!rendered.contains("Saved memory"));
    }

    #[test]
    fn test_stream_usage_label_includes_reasoning_and_cached_tokens() {"#;
                actions.push(PatchSynthesisAction {
                    tool: "file_edit".to_string(),
                    path: "src/tui/app.rs".to_string(),
                    old_string: Some(anchor.to_string()),
                    new_string: replacement.to_string(),
                    line_start: None,
                    line_end: None,
                    expected_replacements: Some(1),
                });
            }
        }

        actions
    }

    pub(super) fn deterministic_save_outcome_actions(
        cwd: &std::path::Path,
    ) -> Option<(PatchSynthesisAction, PatchSynthesisAction)> {
        let path = cwd.join("src/tui/app.rs");
        let content = std::fs::read_to_string(path).ok()?;
        if !content.contains("fn format_memory_write_outcome(")
            || !content.contains("format!(\"Saved: {}\", save_content)")
        {
            return None;
        }

        let save_match = r#"match save_target {
                                MemorySaveTarget::User => {
                                    mem.add_learning_async(save_content, "preference").await;
                                }
                                MemorySaveTarget::Topic => {
                                    mem.add_topic_learning_async(
                                        save_content,
                                        "note",
                                        save_topic.unwrap_or("notes"),
                                    )
                                    .await;
                                }
                                MemorySaveTarget::Auto => {
                                    mem.add_auto_learning_async(save_content, "note").await;
                                }
                            }
                            format!("Saved: {}", save_content)"#;
        let save_outcome = r#"let outcome = match save_target {
                                MemorySaveTarget::User => {
                                    mem.add_learning_async(save_content, "preference").await
                                }
                                MemorySaveTarget::Topic => {
                                    mem.add_topic_learning_async(
                                        save_content,
                                        "note",
                                        save_topic.unwrap_or("notes"),
                                    )
                                    .await
                                }
                                MemorySaveTarget::Auto => {
                                    mem.add_auto_learning_async(save_content, "note").await
                                }
                            };
                            format_memory_write_outcome(save_content, &outcome)"#;

        let first_old = format!(
            "let mem = memory_manager.lock().await;\n                            {}",
            save_match
        );
        let first_new = format!(
            "let mem = memory_manager.lock().await;\n                            {}",
            save_outcome
        );
        let second_old = format!(
            "let mem = crate::memory::MemoryManager::new();\n                            {}",
            save_match
        );
        let second_new = format!(
            "let mem = crate::memory::MemoryManager::new();\n                            {}",
            save_outcome
        );

        Some((
            PatchSynthesisAction {
                tool: "file_edit".to_string(),
                path: "src/tui/app.rs".to_string(),
                old_string: Some(first_old),
                new_string: first_new,
                line_start: None,
                line_end: None,
                expected_replacements: Some(1),
            },
            PatchSynthesisAction {
                tool: "file_edit".to_string(),
                path: "src/tui/app.rs".to_string(),
                old_string: Some(second_old),
                new_string: second_new,
                line_start: None,
                line_end: None,
                expected_replacements: Some(1),
            },
        ))
    }
}

fn local_web_mvp_scaffold_html() -> &'static str {
    r##"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Local Strain and Phage Tracker</title>
  <style>
    body { font-family: system-ui, sans-serif; margin: 2rem; color: #18212f; background: #f6f8fb; }
    main { max-width: 820px; margin: 0 auto; }
    form, table { width: 100%; }
    label { display: block; margin: 0.75rem 0 0.25rem; font-weight: 600; }
    input, textarea, button { box-sizing: border-box; width: 100%; padding: 0.65rem; font: inherit; }
    button { margin-top: 1rem; background: #176b56; color: white; border: 0; cursor: pointer; }
    table { margin-top: 1.5rem; border-collapse: collapse; background: white; }
    th, td { border: 1px solid #d7dde8; padding: 0.65rem; text-align: left; vertical-align: top; }
  </style>
</head>
<body>
  <main>
    <h1>Local Strain and Phage Tracker</h1>
    <form id="entry-form">
      <label for="strain">Strain</label>
      <input id="strain" required placeholder="E. coli isolate A">
      <label for="phage">Phage notes</label>
      <textarea id="phage" rows="4" placeholder="Phage tested, result, date"></textarea>
      <button type="submit">Save local note</button>
    </form>
    <table>
      <thead><tr><th>Strain</th><th>Phage notes</th></tr></thead>
      <tbody id="rows"></tbody>
    </table>
  </main>
  <script>
    const storageKey = "local-strain-phage-notes";
    const form = document.querySelector("#entry-form");
    const rows = document.querySelector("#rows");

    function loadEntries() {
      return JSON.parse(localStorage.getItem(storageKey) || "[]");
    }

    function saveEntries(entries) {
      localStorage.setItem(storageKey, JSON.stringify(entries));
    }

    function render() {
      rows.innerHTML = "";
      for (const item of loadEntries()) {
        const row = document.createElement("tr");
        const strainCell = document.createElement("td");
        const phageCell = document.createElement("td");
        strainCell.textContent = item.strain;
        phageCell.textContent = item.phage;
        row.append(strainCell, phageCell);
        rows.appendChild(row);
      }
    }

    form.addEventListener("submit", (event) => {
      event.preventDefault();
      const entries = loadEntries();
      entries.push({
        strain: document.querySelector("#strain").value.trim(),
        phage: document.querySelector("#phage").value.trim()
      });
      saveEntries(entries);
      form.reset();
      render();
    });

    render();
  </script>
</body>
</html>
"##
}
