//! Conversation-loop controller module.
//!
//! Owns one focused stage of turn execution so permissions, validation, repair, and closeout stay explicit in the runtime.

use super::*;
use crate::services::api::{ChatRequest, Message, ToolCall};
use anyhow::Result;
use serde::Deserialize;
use tracing::warn;

#[derive(Debug, Deserialize)]
pub(super) struct PatchSynthesisPlan {
    #[serde(default)]
    pub(super) can_patch: bool,
    #[serde(default)]
    pub(super) reason: String,
    #[serde(default)]
    pub(super) actions: Vec<PatchSynthesisAction>,
}

#[derive(Debug, Deserialize)]
pub(super) struct PatchSynthesisAction {
    #[serde(default)]
    pub(super) tool: String,
    pub(super) path: String,
    #[serde(default)]
    pub(super) old_string: Option<String>,
    pub(super) new_string: String,
    #[serde(default)]
    pub(super) line_start: Option<usize>,
    #[serde(default)]
    pub(super) line_end: Option<usize>,
    #[serde(default)]
    pub(super) expected_replacements: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PatchSynthesisSource {
    ModelJson,
    ModelToolFallback,
    DeterministicFallback,
}

impl PatchSynthesisSource {
    pub(super) fn label(self) -> &'static str {
        match self {
            PatchSynthesisSource::ModelJson => "model_json",
            PatchSynthesisSource::ModelToolFallback => "model_tool_fallback",
            PatchSynthesisSource::DeterministicFallback => "deterministic_fallback",
        }
    }
}

#[derive(Debug)]
pub(super) struct PatchSynthesisOutcome {
    pub(super) tool_calls: Vec<ToolCall>,
    pub(super) source: PatchSynthesisSource,
    pub(super) fallback_reason: Option<String>,
}

impl PatchSynthesisOutcome {
    fn model_json(tool_calls: Vec<ToolCall>) -> Self {
        Self {
            tool_calls,
            source: PatchSynthesisSource::ModelJson,
            fallback_reason: None,
        }
    }

    fn model_tool_fallback(tool_calls: Vec<ToolCall>) -> Self {
        Self {
            tool_calls,
            source: PatchSynthesisSource::ModelToolFallback,
            fallback_reason: None,
        }
    }

    fn deterministic_fallback(tool_calls: Vec<ToolCall>, reason: impl Into<String>) -> Self {
        Self {
            tool_calls,
            source: PatchSynthesisSource::DeterministicFallback,
            fallback_reason: Some(reason.into()),
        }
    }
}

impl ConversationLoop {
    pub(super) async fn synthesize_patch_tool_calls(
        &self,
        messages: &[Message],
        task_preview: &str,
    ) -> Result<PatchSynthesisOutcome> {
        let evidence = Self::patch_synthesis_evidence(messages);
        let deterministic_seed = if task_preview.trim().is_empty() {
            evidence.clone()
        } else if evidence.trim().is_empty() {
            format!("TASK:\n{task_preview}")
        } else {
            format!("TASK:\n{task_preview}\n\nEVIDENCE:\n{evidence}")
        };

        if deterministic_seed.trim().is_empty() {
            return Err(anyhow::anyhow!("no usable evidence for patch synthesis"));
        }

        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let deterministic_reason = if evidence.trim().is_empty() {
            "model patch synthesis skipped: no usable evidence"
        } else {
            "deterministic patch repair rule matched before model synthesis"
        };
        if let Some(outcome) =
            self.deterministic_patch_fallback(&deterministic_seed, &cwd, deterministic_reason)
        {
            return Ok(outcome);
        }

        if evidence.trim().is_empty() {
            return Err(anyhow::anyhow!(deterministic_reason));
        }

        let system = r#"You are a controlled patch synthesis engine for a coding agent.
You receive prior read/search/tool evidence from the current task.
Return ONLY one JSON object. Do not use markdown. Do not explain outside JSON.
Only propose small, evidence-backed file_edit actions.
If you cannot patch from the evidence, return {"can_patch":false,"reason":"...","actions":[]}."#;
        let user = format!(
            r#"Task:
{task_preview}

Evidence from prior tool results:
{evidence}

Return this exact JSON shape:
{{
  "can_patch": true,
  "reason": "why this patch is safe from the evidence",
  "actions": [
    {{
      "tool": "file_edit",
      "path": "relative/path.rs",
      "old_string": "exact text to replace",
      "new_string": "replacement text",
      "expected_replacements": 1
    }}
  ]
}}

Rules:
- Only use tool="file_edit".
- Prefer old_string/new_string exact replacement when the evidence contains the original code.
- You may use line_start/line_end only when evidence gives a precise bounded line range; do not combine line_start/line_end with old_string.
- Do not invent paths. Use paths shown in evidence.
- Do not invent enum variants, struct fields, functions, or APIs not visible in evidence. Reuse existing names exactly; if a decision object already computes status, prefer that status over reimplementing gates.
- For quality/scoring fixes, if a scorer/decision object already encodes explicit override plus safety/duplication hard stops, assign from decision.status directly. Never re-promote Rejected/Proposed decisions to Accepted with a second explicit_override or score check in the caller.
- Keep actions minimal. Return one to six actions when the evidence shows multiple independent acceptance-critical bypasses or one Rust type change that requires updating every initializer/pattern. Otherwise return one safest next edit. Every action must have expected_replacements=1.
- For Rust compiler errors like "missing field `x` in initializer" or "pattern does not mention field `x`", fix every constructor and match pattern shown in the validation evidence, not just the enum definition.
- For memory quality gate tasks, if evidence shows both a model-facing save tool path and a quality/status override path, fix both paths in the same plan.
- For Python heredocs inside shell scripts, remember they run from stdin. Do not rely on pathlib.Path(__file__).parent. For repo-local modules under scripts/, import them as package paths such as `from scripts.live_eval_report_parser import report_rows`.
- Tool evidence may mark search hits with Markdown emphasis like **symbol**. Those asterisks are display highlighting, not source code; never copy ** markers into a patch.
- Never edit .git, target, cache, generated benchmark output, or files outside the working tree."#
        );

        let mut synthesis_messages = vec![Message::system(system), Message::user(user.clone())];
        let mut last_content = String::new();
        let mut last_validation_errors = Vec::new();

        for attempt in 0..2 {
            let request = ChatRequest::new(&self.model)
                .with_messages(synthesis_messages.clone())
                .with_temperature(0.0);
            let session_step = self.call_api(request).await?;
            let content = session_step.assistant_text;
            last_content = content.clone();

            if let Some(plan) = Self::parse_patch_synthesis_plan(&content) {
                if !plan.can_patch {
                    let reason = plan.reason.trim();
                    last_validation_errors.push(if reason.is_empty() {
                        "patch synthesis declined without a reason".to_string()
                    } else {
                        format!("patch synthesis declined: {}", reason)
                    });
                    if attempt == 0 {
                        synthesis_messages
                            .push(Message::assistant(safe_prefix_by_bytes(&content, 1200)));
                        synthesis_messages.push(Message::user(format!(
                            "The previous patch plan declined instead of editing: {}. If the evidence names a concrete missing code block, compile error, assertion failure, or regression marker, return corrected JSON with the smallest file_edit action. Return can_patch=false only when there is no concrete editable file or old_string evidence.",
                            last_validation_errors.join("; ")
                        )));
                        continue;
                    }
                    break;
                }

                let mut calls = Vec::new();
                let mut validation_errors = Vec::new();
                for action in plan.actions.iter().take(6) {
                    match self.validate_patch_synthesis_action(action, &cwd) {
                        Ok(call) => calls.push(call),
                        Err(err) => validation_errors.push(err.to_string()),
                    }
                }
                if !calls.is_empty() {
                    return Ok(PatchSynthesisOutcome::model_json(calls));
                }
                last_validation_errors = validation_errors;
                if last_validation_errors.is_empty() {
                    last_validation_errors
                        .push("patch plan did not include a valid file_edit action".to_string());
                }
            } else {
                last_validation_errors.push("response was not valid patch JSON".to_string());
            }

            if attempt == 0 {
                synthesis_messages.push(Message::assistant(safe_prefix_by_bytes(&content, 1200)));
                synthesis_messages.push(Message::user(format!(
                    "The previous patch plan was rejected: {}. Return corrected JSON only. Use one to six file_edit actions when multiple independent acceptance-critical bypasses or Rust missing-field/pattern compile errors are visible; otherwise use one action. Use either old_string or line_start/line_end, never both. Do not call tools. Reuse only paths, enum variants, fields, and functions shown in evidence or validation feedback.",
                    last_validation_errors.join("; ")
                )));
            }
        }

        if !last_validation_errors.is_empty() {
            warn!(
                "Patch synthesis JSON actions were not directly applicable: {}",
                last_validation_errors.join("; ")
            );
        }

        let Some(file_edit_tool) = self.tool_registry.get("file_edit") else {
            return Err(anyhow::anyhow!(
                "file_edit tool is unavailable for patch synthesis"
            ));
        };
        let file_edit_schema = crate::services::api::Tool {
            name: file_edit_tool.name().to_string(),
            description: file_edit_tool.description().to_string(),
            parameters: file_edit_tool.parameters(),
            strict_schema: file_edit_tool.strict_schema(),
        };
        let tool_system = r#"You are now in forced patch application mode.
Use the file_edit tool to apply the smallest safe patch from the evidence.
Do not call read/search tools.
Do not invent enum variants, struct fields, functions, or APIs not visible in evidence.
If a scorer/decision object already returns final status, use that status directly; do not re-promote with explicit_override or score checks in the caller.
For Python heredocs inside shell scripts, import repo-local scripts modules with package paths such as `from scripts.live_eval_report_parser import report_rows`; do not rely on pathlib.Path(__file__).parent under python stdin.
Tool evidence may contain Markdown search highlighting like **symbol**; strip those markers before using text as source code.
Do not answer in prose unless no safe patch exists."#;
        let tool_request = ChatRequest::new(&self.model)
            .with_messages(vec![
                Message::system(tool_system),
                Message::user(user),
                Message::assistant(format!(
                    "The previous JSON-only patch synthesis response was rejected: {}. It began with: {}",
                    last_validation_errors.join("; "),
                    safe_prefix_by_bytes(&last_content, 800)
                )),
            ])
            .with_tools(vec![file_edit_schema])
            .with_temperature(0.0);
        let fallback_step = self.call_api(tool_request).await?;
        let fallback_content = fallback_step.assistant_text;
        let fallback_tool_calls = fallback_step.tool_calls;
        let mut calls = Vec::new();
        let mut validation_errors = Vec::new();
        for tool_call in fallback_tool_calls.into_iter().take(6) {
            match self.validate_synthesized_tool_call(tool_call, &cwd) {
                Ok(call) => calls.push(call),
                Err(err) => validation_errors.push(err.to_string()),
            }
        }
        if calls.is_empty() {
            let failure_reason = format!(
                "model patch synthesis failed: validation_errors=[{}]; text began with: {}",
                validation_errors.join("; "),
                safe_prefix_by_bytes(&fallback_content, 800)
            );
            if let Some(outcome) =
                self.deterministic_patch_fallback(&deterministic_seed, &cwd, failure_reason.clone())
            {
                return Ok(outcome);
            }
            return Err(anyhow::anyhow!(
                "patch synthesis did not return valid JSON or file_edit calls; validation_errors=[{}]; text began with: {}",
                validation_errors.join("; "),
                safe_prefix_by_bytes(&fallback_content, 800)
            ));
        }
        Ok(PatchSynthesisOutcome::model_tool_fallback(calls))
    }

    pub(super) fn deterministic_patch_tool_calls(
        &self,
        evidence: &str,
        cwd: &std::path::Path,
    ) -> Vec<ToolCall> {
        patch_repair_rules::deterministic_patch_tool_calls(self, evidence, cwd)
    }

    pub(super) fn deterministic_patch_fallback(
        &self,
        evidence: &str,
        cwd: &std::path::Path,
        reason: impl Into<String>,
    ) -> Option<PatchSynthesisOutcome> {
        if !Self::deterministic_patch_synthesis_enabled() {
            return None;
        }
        let tool_calls = self.deterministic_patch_tool_calls(evidence, cwd);
        if tool_calls.is_empty() {
            None
        } else {
            Some(PatchSynthesisOutcome::deterministic_fallback(
                tool_calls, reason,
            ))
        }
    }

    pub(super) fn patch_synthesis_enabled() -> bool {
        crate::services::config::runtime_config().patch_synthesis_enabled()
    }

    pub(super) fn deterministic_patch_synthesis_enabled() -> bool {
        crate::services::config::runtime_config().deterministic_patch_synthesis_enabled()
    }

    pub(super) fn patch_synthesis_evidence(messages: &[Message]) -> String {
        let mut chunks = Vec::new();
        let mut total = 0usize;
        for message in messages.iter().rev() {
            let chunk = match message {
                Message::User { content } => {
                    format!("USER:\n{}", safe_prefix_by_bytes(content, 3000))
                }
                Message::Tool { content, .. } => {
                    if content.contains("[File unchanged since last read:") {
                        continue;
                    }
                    let relevant_failure = !content.starts_with("Result: OK")
                        && (content.contains("error[")
                            || content.contains("could not compile")
                            || content.contains("AssertionError")
                            || content.contains("[exit status:")
                            || content.contains("failed_commands"));
                    if !content.starts_with("Result: OK") && !relevant_failure {
                        continue;
                    }
                    let label = if content.starts_with("Result: OK") {
                        "TOOL RESULT"
                    } else {
                        "FAILED TOOL RESULT"
                    };
                    format!(
                        "{}:\n{}",
                        label,
                        Self::patch_synthesis_tool_excerpt(content)
                    )
                }
                Message::Assistant { content, .. } if !content.trim().is_empty() => {
                    format!("ASSISTANT:\n{}", safe_prefix_by_bytes(content, 1200))
                }
                _ => continue,
            };
            total += chunk.len();
            chunks.push(chunk);
            if total >= 18_000 {
                break;
            }
        }
        chunks.reverse();
        chunks.join("\n\n---\n\n")
    }

    pub(super) fn patch_synthesis_tool_excerpt(content: &str) -> String {
        if content.len() <= 7_000 {
            return content.to_string();
        }

        let mut sections = vec![format!(
            "[start of tool result]\n{}",
            safe_prefix_by_bytes(content, 1_800)
        )];
        for window in Self::patch_synthesis_relevant_windows(content, 4) {
            sections.push(format!("[relevant excerpt]\n{}", window));
        }
        sections.push(format!(
            "[end of tool result]\n{}",
            safe_suffix_by_bytes(content, 2_400)
        ));

        let joined = sections.join("\n\n[...]\n\n");
        if joined.len() <= 10_000 {
            joined
        } else {
            format!(
                "{}\n\n[...]\n\n{}",
                safe_prefix_by_bytes(&joined, 7_000),
                safe_suffix_by_bytes(&joined, 2_500)
            )
        }
    }

    pub(super) fn patch_synthesis_relevant_windows(
        content: &str,
        max_windows: usize,
    ) -> Vec<String> {
        const KEYWORDS: &[&str] = &[
            "todo",
            "fixme",
            "stub",
            "not implemented",
            "unimplemented",
            "panic!",
            "summary_task",
            "required_commands",
            "plan_quality",
            "tool_boundary",
            "verification_status",
            "memory_save",
            "assess_memory_candidate",
            "memorystatus::accepted",
            "duplicate",
            "sensitive",
            "assertionerror",
            "[exit status:",
            "error",
            "failed",
        ];

        let lines = content.lines().collect::<Vec<_>>();
        let mut matches = lines
            .iter()
            .enumerate()
            .filter_map(|(idx, line)| {
                let lower = line.to_ascii_lowercase();
                KEYWORDS
                    .iter()
                    .any(|keyword| lower.contains(keyword))
                    .then_some(idx)
            })
            .collect::<Vec<_>>();
        matches.sort_unstable();
        matches.dedup();

        let mut windows = Vec::new();
        let mut covered_until = 0usize;
        for idx in matches {
            let start = idx.saturating_sub(10);
            let end = (idx + 24).min(lines.len());
            if !windows.is_empty() && start <= covered_until {
                covered_until = covered_until.max(end);
                continue;
            }
            windows.push(lines[start..end].join("\n"));
            covered_until = end;
            if windows.len() >= max_windows {
                break;
            }
        }
        windows
    }

    pub(super) fn parse_patch_synthesis_plan(content: &str) -> Option<PatchSynthesisPlan> {
        let trimmed = content.trim();
        if let Ok(plan) = serde_json::from_str::<PatchSynthesisPlan>(trimmed) {
            return Some(plan);
        }

        let without_fence = trimmed
            .strip_prefix("```json")
            .or_else(|| trimmed.strip_prefix("```"))
            .and_then(|s| s.strip_suffix("```"))
            .map(str::trim)
            .unwrap_or(trimmed);
        if let Ok(plan) = serde_json::from_str::<PatchSynthesisPlan>(without_fence) {
            return Some(plan);
        }

        for (start, ch) in without_fence.char_indices() {
            if ch != '{' {
                continue;
            }
            if let Some(end) = Self::matching_json_object_end(without_fence, start) {
                if let Ok(plan) =
                    serde_json::from_str::<PatchSynthesisPlan>(&without_fence[start..end])
                {
                    return Some(plan);
                }
            }
        }
        None
    }

    pub(super) fn matching_json_object_end(input: &str, start: usize) -> Option<usize> {
        let mut depth = 0usize;
        let mut in_string = false;
        let mut escaped = false;
        for (offset, ch) in input[start..].char_indices() {
            if in_string {
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == '"' {
                    in_string = false;
                }
                continue;
            }

            match ch {
                '"' => in_string = true,
                '{' => depth += 1,
                '}' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return Some(start + offset + ch.len_utf8());
                    }
                }
                _ => {}
            }
        }
        None
    }

    pub(super) fn validate_patch_synthesis_action(
        &self,
        action: &PatchSynthesisAction,
        cwd: &std::path::Path,
    ) -> Result<ToolCall> {
        let action_tool = if action.tool.trim().is_empty() {
            "file_edit"
        } else {
            action.tool.trim()
        };
        if !matches!(action_tool, "file_edit" | "file_write") {
            return Err(anyhow::anyhow!(
                "unsupported synthesized patch tool: {}",
                action.tool
            ));
        }
        if action.path.trim().is_empty() {
            return Err(anyhow::anyhow!("synthesized patch path is empty"));
        }
        let raw_path = std::path::Path::new(action.path.trim());
        for component in raw_path.components() {
            match component {
                std::path::Component::ParentDir => {
                    return Err(anyhow::anyhow!(
                        "synthesized patch path contains parent traversal: {}",
                        action.path
                    ));
                }
                std::path::Component::Normal(part)
                    if part == ".git" || part == "target" || part == "node_modules" =>
                {
                    return Err(anyhow::anyhow!(
                        "synthesized patch path targets ignored/generated directory: {}",
                        action.path
                    ));
                }
                _ => {}
            }
        }
        if action.new_string.len() > 20_000 {
            return Err(anyhow::anyhow!(
                "synthesized patch replacement is too large"
            ));
        }
        if action_tool == "file_write" {
            return self.validate_patch_synthesis_file_write_action(action, raw_path, cwd);
        }

        let (canonical_candidate, tool_path) =
            match Self::resolve_synthesized_patch_path(raw_path, cwd) {
                Ok(resolved) => resolved,
                Err(path_error) => {
                    if let Some(old_string) = action.old_string.as_ref() {
                        Self::resolve_synthesized_patch_path_by_old_string(old_string, cwd)
                            .unwrap_or_else(|| Err(path_error))?
                    } else {
                        return Err(path_error);
                    }
                }
            };

        let is_rust_file =
            canonical_candidate.extension().and_then(|ext| ext.to_str()) == Some("rs");
        let mut normalized_new_string = action.new_string.clone();
        let mut params = serde_json::json!({
            "path": tool_path,
        });
        if action.line_start.is_some() || action.line_end.is_some() {
            let (Some(line_start), Some(line_end)) = (action.line_start, action.line_end) else {
                return Err(anyhow::anyhow!(
                    "synthesized patch line_start and line_end must be provided together"
                ));
            };
            if line_start == 0 || line_end == 0 || line_start > line_end {
                return Err(anyhow::anyhow!(
                    "synthesized patch line range is invalid: {}..={}",
                    line_start,
                    line_end
                ));
            }
            let file_content = std::fs::read_to_string(&canonical_candidate).unwrap_or_default();
            let line_count = file_content.lines().count();
            if line_start > line_count || line_end > line_count {
                return Err(anyhow::anyhow!(
                    "synthesized patch line range {}..={} is outside {} line file",
                    line_start,
                    line_end,
                    line_count
                ));
            }
            let line_span = line_end - line_start + 1;
            let max_line_span = Self::max_synthesized_line_range(&canonical_candidate);
            if line_span > max_line_span {
                return Err(anyhow::anyhow!(
                    "synthesized patch line range is too broad: {}..={} ({} lines, max {})",
                    line_start,
                    line_end,
                    line_span,
                    max_line_span
                ));
            }
            if canonical_candidate.extension().and_then(|ext| ext.to_str()) == Some("sh") {
                Self::validate_shell_line_range_scope(&file_content, line_start, line_end)?;
            }
            if is_rust_file && !Self::balanced_delimiters_rough(&normalized_new_string) {
                return Err(anyhow::anyhow!(
                    "synthesized patch replacement has unbalanced delimiters"
                ));
            }
            params["line_start"] = serde_json::json!(line_start);
            params["line_end"] = serde_json::json!(line_end);
        } else if let Some(old_string) = action.old_string.as_ref() {
            if old_string.trim().is_empty() {
                return Err(anyhow::anyhow!(
                    "synthesized patch old_string is empty without a line range"
                ));
            }
            if old_string.len() > 12_000 {
                return Err(anyhow::anyhow!("synthesized patch old_string is too large"));
            }
            let (normalized_old_string, replacement) =
                Self::normalize_synthesized_replacement_anchor(
                    old_string,
                    &normalized_new_string,
                    &canonical_candidate,
                )?;
            normalized_new_string = replacement;
            if is_rust_file
                && Self::balanced_delimiters_rough(&normalized_old_string)
                && !Self::balanced_delimiters_rough(&normalized_new_string)
            {
                return Err(anyhow::anyhow!(
                    "synthesized patch replacement has unbalanced delimiters"
                ));
            }
            params["old_string"] = serde_json::json!(normalized_old_string);
            if let Some(expected) = action.expected_replacements {
                if expected != 1 {
                    return Err(anyhow::anyhow!(
                        "synthesized patch expected_replacements must be exactly 1, got {}",
                        expected
                    ));
                }
                params["expected_replacements"] = serde_json::json!(expected);
            } else {
                params["expected_replacements"] = serde_json::json!(1);
            }
        } else {
            return Err(anyhow::anyhow!(
                "synthesized patch must include old_string or line_start/line_end"
            ));
        }
        params["new_string"] = serde_json::json!(normalized_new_string);

        if let Some(tool) = self.tool_registry.get("file_edit") {
            if let Some(err) = tool.validate_params(&params) {
                return Err(anyhow::anyhow!(
                    "synthesized patch failed tool schema validation: {}",
                    err
                ));
            }
        }
        if is_rust_file {
            Self::validate_rust_patch_semantics(&canonical_candidate, &action.new_string)?;
            if let Some(err) = Self::unknown_rust_enum_variant_in_patch(&action.new_string, cwd) {
                return Err(anyhow::anyhow!("{}", err));
            }
        } else if canonical_candidate.extension().and_then(|ext| ext.to_str()) == Some("sh") {
            Self::validate_shell_patch_semantics(&canonical_candidate, &normalized_new_string)?;
        }

        Ok(ToolCall {
            id: format!("patch_synthesis_{}", uuid::Uuid::new_v4().simple()),
            name: "file_edit".to_string(),
            arguments: params,
        })
    }

    fn validate_patch_synthesis_file_write_action(
        &self,
        action: &PatchSynthesisAction,
        raw_path: &std::path::Path,
        cwd: &std::path::Path,
    ) -> Result<ToolCall> {
        if action.old_string.is_some() || action.line_start.is_some() || action.line_end.is_some() {
            return Err(anyhow::anyhow!(
                "synthesized file_write must not include old_string or line ranges"
            ));
        }
        let (canonical_candidate, tool_path) = Self::resolve_synthesized_write_path(raw_path, cwd)?;
        if canonical_candidate.exists() {
            return Err(anyhow::anyhow!(
                "synthesized file_write target already exists: {}",
                raw_path.display()
            ));
        }
        let params = serde_json::json!({
            "path": tool_path,
            "content": action.new_string,
        });
        if let Some(tool) = self.tool_registry.get("file_write") {
            if let Some(err) = tool.validate_params(&params) {
                return Err(anyhow::anyhow!(
                    "synthesized file_write failed tool schema validation: {}",
                    err
                ));
            }
        }
        Ok(ToolCall {
            id: format!("patch_synthesis_{}", uuid::Uuid::new_v4().simple()),
            name: "file_write".to_string(),
            arguments: params,
        })
    }

    pub(super) fn max_synthesized_line_range(path: &std::path::Path) -> usize {
        let normalized_path = path.to_string_lossy();
        if normalized_path.ends_with("scripts/run_live_eval.sh") {
            return 96;
        }

        match path.extension().and_then(|ext| ext.to_str()) {
            Some("rs") => 25,
            Some("sh" | "py" | "md" | "toml" | "yaml" | "yml") => 80,
            _ => 40,
        }
    }

    pub(super) fn validate_shell_line_range_scope(
        file_content: &str,
        line_start: usize,
        line_end: usize,
    ) -> Result<()> {
        let selected = file_content
            .lines()
            .enumerate()
            .filter(|(idx, _)| {
                let line_no = idx + 1;
                line_no >= line_start && line_no <= line_end
            })
            .map(|(_, line)| line)
            .collect::<Vec<_>>();
        let Some(first_fn_index) = selected
            .iter()
            .position(|line| Self::shell_top_level_function_name(line).is_some())
        else {
            return Ok(());
        };
        let first_fn = Self::shell_top_level_function_name(selected[first_fn_index])
            .unwrap_or("function")
            .to_string();
        if let Some(next_fn) = selected
            .iter()
            .skip(first_fn_index + 1)
            .find_map(|line| Self::shell_top_level_function_name(line))
        {
            return Err(anyhow::anyhow!(
                "synthesized shell patch line range crosses function boundary: {} into {}",
                first_fn,
                next_fn
            ));
        }
        Ok(())
    }

    pub(super) fn shell_top_level_function_name(line: &str) -> Option<&str> {
        if line.starts_with(' ') || line.starts_with('\t') {
            return None;
        }
        let trimmed = line.trim_end();
        let name = trimmed.strip_suffix("() {")?;
        let mut chars = name.chars();
        let first = chars.next()?;
        if !(first == '_' || first.is_ascii_alphabetic()) {
            return None;
        }
        if chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric()) {
            Some(name)
        } else {
            None
        }
    }

    pub(super) fn validate_shell_patch_semantics(
        path: &std::path::Path,
        new_string: &str,
    ) -> Result<()> {
        let normalized_path = path.to_string_lossy();
        if !normalized_path.ends_with("scripts/run_live_eval.sh") {
            return Ok(());
        }

        if new_string.contains("**") {
            return Err(anyhow::anyhow!(
                "run_live_eval.sh patch contains Markdown emphasis markers (`**`), likely copied from highlighted tool output rather than source code"
            ));
        }

        let uses_bare_parser_import = new_string.contains("from live_eval_report_parser import")
            || new_string.contains("import live_eval_report_parser");
        let uses_package_parser_import = new_string
            .contains("from scripts.live_eval_report_parser import")
            || new_string.contains("import scripts.live_eval_report_parser");
        let adds_scripts_to_python_path = new_string.contains("sys.path")
            && new_string.contains("scripts")
            && new_string.contains("live_eval_report_parser");
        if uses_bare_parser_import && !uses_package_parser_import && !adds_scripts_to_python_path {
            return Err(anyhow::anyhow!(
                "run_live_eval.sh Python heredocs execute from stdin; import live_eval_report_parser as `from scripts.live_eval_report_parser import ...` or add the scripts directory to sys.path explicitly"
            ));
        }

        if new_string.contains("Path(__file__).parent")
            && new_string.contains("live_eval_report_parser")
            && !uses_package_parser_import
            && !adds_scripts_to_python_path
        {
            return Err(anyhow::anyhow!(
                "run_live_eval.sh Python heredocs must not rely on pathlib.Path(__file__).parent for repo-local imports; use the scripts package path"
            ));
        }

        Ok(())
    }

    pub(super) fn validate_rust_patch_semantics(
        path: &std::path::Path,
        new_string: &str,
    ) -> Result<()> {
        let normalized_path = path.to_string_lossy();
        if normalized_path.ends_with("src/memory/types.rs")
            && (new_string.contains("Duplicate") || new_string.contains("Demoted"))
            && new_string.contains("MemoryStatus")
        {
            return Err(anyhow::anyhow!(
                "memory duplicate/demote must be represented as MemoryWriteOutcomeStatus or quality decision output; do not extend MemoryStatus with Duplicate/Demoted"
            ));
        }
        if normalized_path.ends_with("src/memory/quality.rs")
            && new_string.contains("let status = if score >= 0.65")
            && new_string.contains("MemoryStatus::Accepted")
        {
            return Err(anyhow::anyhow!(
                "memory quality status must preserve score_memory_write hard gates; use write_decision.status instead of re-promoting score >= 0.65 to Accepted"
            ));
        }
        if normalized_path.ends_with("src/engine/conversation_loop/mod.rs")
            && new_string.contains("prefetch_retrieval_context_with_llm_rerank")
        {
            if new_string.contains("futures::executor::block_on") {
                return Err(anyhow::anyhow!(
                    "persistent memory prefetch in conversation_loop must use async lock().await, not futures::executor::block_on"
                ));
            }
            if new_string.contains("self.provider.as_ref().and_then") {
                return Err(anyhow::anyhow!(
                    "persistent memory prefetch in conversation_loop must pass the existing model string directly, not derive a preferred model from provider"
                ));
            }
            if new_string.contains("self.provider.as_ref().map") {
                return Err(anyhow::anyhow!(
                    "persistent memory prefetch in conversation_loop must pass self.provider.as_ref() directly, not treat it as an Option"
                ));
            }
            if !new_string.contains(".lock().await") {
                return Err(anyhow::anyhow!(
                    "persistent memory prefetch in conversation_loop must lock the Arc<Mutex<MemoryManager>> before calling prefetch"
                ));
            }
            if !new_string.contains("&self.model") {
                return Err(anyhow::anyhow!(
                    "persistent memory prefetch in conversation_loop must pass &self.model"
                ));
            }
        }
        Ok(())
    }

    pub(super) fn normalize_synthesized_replacement_anchor(
        old_string: &str,
        new_string: &str,
        path: &std::path::Path,
    ) -> Result<(String, String)> {
        let content = std::fs::read_to_string(path)?;
        let exact_count = content.matches(old_string).count();
        if exact_count == 1 {
            return Ok((old_string.to_string(), new_string.to_string()));
        }
        if exact_count > 1 {
            return Err(anyhow::anyhow!(
                "synthesized patch old_string is not unique in {}",
                path.display()
            ));
        }
        if new_string.lines().count() > 1 {
            if path.extension().and_then(|ext| ext.to_str()) == Some("sh") {
                if let Some((recovered_old, recovered_new)) =
                    Self::recover_shell_function_replacement_anchor(
                        old_string, new_string, &content, path,
                    )?
                {
                    return Ok((recovered_old, recovered_new));
                }
            }
            return Err(anyhow::anyhow!(
                "synthesized patch old_string was not found exactly in {}; refusing inexact multi-line replacement",
                path.display()
            ));
        }

        let Some(binding_name) = Self::synthesized_assignment_binding(old_string)
            .or_else(|| Self::synthesized_assignment_binding(new_string))
        else {
            return Err(anyhow::anyhow!(
                "synthesized patch old_string was not found exactly in {}",
                path.display()
            ));
        };

        let prefix = format!("let {binding_name} =");
        let matches = content
            .lines()
            .filter(|line| line.trim_start().starts_with(&prefix))
            .map(str::to_string)
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(anyhow::anyhow!(
                "synthesized patch old_string was not found exactly and assignment anchor `{}` matched {} lines in {}",
                binding_name,
                matches.len(),
                path.display()
            ));
        }

        let recovered_old = matches[0].clone();
        let recovered_new = if new_string.lines().count() <= 1 {
            let indent = recovered_old
                .chars()
                .take_while(|ch| ch.is_whitespace())
                .collect::<String>();
            format!("{}{}", indent, new_string.trim())
        } else {
            new_string.to_string()
        };
        Ok((recovered_old, recovered_new))
    }

    pub(super) fn recover_shell_function_replacement_anchor(
        old_string: &str,
        new_string: &str,
        content: &str,
        path: &std::path::Path,
    ) -> Result<Option<(String, String)>> {
        let Some(function_name) = Self::shell_function_name_in_text(old_string)
            .or_else(|| Self::shell_function_name_in_text(new_string))
        else {
            return Ok(None);
        };
        if let Some(new_function_name) = Self::shell_function_name_in_text(new_string) {
            if new_function_name != function_name {
                return Err(anyhow::anyhow!(
                    "synthesized shell patch tries to replace function `{}` with `{}`",
                    function_name,
                    new_function_name
                ));
            }
        } else {
            return Err(anyhow::anyhow!(
                "synthesized shell patch for `{}` must include the replacement function header",
                function_name
            ));
        }

        let Some(recovered_old) = Self::shell_function_block(content, &function_name) else {
            return Err(anyhow::anyhow!(
                "synthesized shell patch could not recover function `{}` in {}",
                function_name,
                path.display()
            ));
        };
        Ok(Some((recovered_old, new_string.to_string())))
    }

    pub(super) fn shell_function_name_in_text(input: &str) -> Option<String> {
        input.lines().find_map(|line| {
            let line = line.replace("**", "");
            let stripped = Self::strip_tool_line_prefix(line.trim_start());
            Self::shell_top_level_function_name(stripped).map(str::to_string)
        })
    }

    pub(super) fn strip_tool_line_prefix(line: &str) -> &str {
        let stripped_digits = line.trim_start_matches(|ch: char| ch.is_ascii_digit());
        let stripped_digits = stripped_digits.trim_start();
        stripped_digits
            .strip_prefix(':')
            .or_else(|| stripped_digits.strip_prefix('|'))
            .map(str::trim_start)
            .unwrap_or(line)
    }

    pub(super) fn shell_function_block(content: &str, function_name: &str) -> Option<String> {
        let lines = content.lines().collect::<Vec<_>>();
        let start = lines
            .iter()
            .position(|line| Self::shell_top_level_function_name(line) == Some(function_name))?;
        let mut end = None;
        for (idx, line) in lines.iter().enumerate().skip(start + 1) {
            if line.starts_with(' ') || line.starts_with('\t') {
                continue;
            }
            if line.trim_end() == "}" {
                end = Some(idx);
                break;
            }
            if Self::shell_top_level_function_name(line).is_some() {
                return None;
            }
        }
        let end = end?;
        Some(lines[start..=end].join("\n") + "\n")
    }

    pub(super) fn balanced_delimiters_rough(input: &str) -> bool {
        let mut stack = Vec::new();
        let mut in_string = false;
        let mut in_char = false;
        let mut escaped = false;

        for ch in input.chars() {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' && (in_string || in_char) {
                escaped = true;
                continue;
            }
            if in_string {
                if ch == '"' {
                    in_string = false;
                }
                continue;
            }
            if in_char {
                if ch == '\'' {
                    in_char = false;
                }
                continue;
            }

            match ch {
                '"' => in_string = true,
                '\'' => in_char = true,
                '(' | '[' | '{' => stack.push(ch),
                ')' => {
                    if stack.pop() != Some('(') {
                        return false;
                    }
                }
                ']' if stack.pop() != Some('[') => {
                    return false;
                }
                '}' if stack.pop() != Some('{') => {
                    return false;
                }
                _ => {}
            }
        }

        !in_string && !in_char && stack.is_empty()
    }

    pub(super) fn synthesized_assignment_binding(input: &str) -> Option<String> {
        let re = regex::Regex::new(r"(?m)^\s*let\s+([A-Za-z_][A-Za-z0-9_]*)\s*=").ok()?;
        re.captures(input)
            .and_then(|captures| captures.get(1).map(|m| m.as_str().to_string()))
    }

    pub(super) fn resolve_synthesized_patch_path(
        raw_path: &std::path::Path,
        cwd: &std::path::Path,
    ) -> Result<(std::path::PathBuf, String)> {
        let canonical_cwd = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
        let mut candidates = Vec::new();
        if raw_path.is_absolute() {
            candidates.push(raw_path.to_path_buf());
            if let Ok(stripped) = raw_path.strip_prefix(std::path::Path::new("/")) {
                candidates.push(cwd.join(stripped));
            }
        } else {
            candidates.push(cwd.join(raw_path));
        }

        let normal_components = raw_path
            .components()
            .filter_map(|component| match component {
                std::path::Component::Normal(part) => part.to_str().map(str::to_string),
                _ => None,
            })
            .collect::<Vec<_>>();
        for anchor in ["src", "tests", "benches", "examples"] {
            if let Some(idx) = normal_components.iter().position(|part| part == anchor) {
                let mut anchored = std::path::PathBuf::new();
                for part in &normal_components[idx..] {
                    anchored.push(part);
                }
                candidates.push(cwd.join(anchored));
            }
        }

        if let Some(match_path) = Self::unique_git_path_suffix_match(raw_path, cwd) {
            candidates.push(cwd.join(match_path));
        }

        for candidate in candidates {
            let Ok(canonical_candidate) = candidate.canonicalize() else {
                continue;
            };
            if !canonical_candidate.starts_with(&canonical_cwd) || !canonical_candidate.is_file() {
                continue;
            }
            let relative = canonical_candidate
                .strip_prefix(&canonical_cwd)
                .ok()
                .map(|path| path.to_string_lossy().to_string())
                .unwrap_or_else(|| canonical_candidate.to_string_lossy().to_string());
            return Ok((canonical_candidate, relative));
        }

        Err(anyhow::anyhow!(
            "synthesized patch path is not editable: {}",
            raw_path.display()
        ))
    }

    pub(super) fn resolve_synthesized_write_path(
        raw_path: &std::path::Path,
        cwd: &std::path::Path,
    ) -> Result<(std::path::PathBuf, String)> {
        let canonical_cwd = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
        let candidate = if raw_path.is_absolute() {
            raw_path.to_path_buf()
        } else {
            cwd.join(raw_path)
        };
        let parent = candidate.parent().ok_or_else(|| {
            anyhow::anyhow!(
                "synthesized file_write target has no parent: {}",
                raw_path.display()
            )
        })?;
        let canonical_parent = parent.canonicalize().map_err(|_| {
            anyhow::anyhow!(
                "synthesized file_write parent is not writable: {}",
                raw_path.display()
            )
        })?;
        if !canonical_parent.starts_with(&canonical_cwd) {
            return Err(anyhow::anyhow!(
                "synthesized file_write target is outside workspace: {}",
                raw_path.display()
            ));
        }
        let file_name = candidate.file_name().ok_or_else(|| {
            anyhow::anyhow!(
                "synthesized file_write target has no file name: {}",
                raw_path.display()
            )
        })?;
        let canonical_candidate = canonical_parent.join(file_name);
        let relative = canonical_candidate
            .strip_prefix(&canonical_cwd)
            .ok()
            .map(|path| path.to_string_lossy().to_string())
            .unwrap_or_else(|| canonical_candidate.to_string_lossy().to_string());
        Ok((canonical_candidate, relative))
    }

    pub(super) fn resolve_synthesized_patch_path_by_old_string(
        old_string: &str,
        cwd: &std::path::Path,
    ) -> Option<Result<(std::path::PathBuf, String)>> {
        if old_string.trim().is_empty() || old_string.len() > 12_000 {
            return None;
        }
        let canonical_cwd = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
        let mut matches = Vec::new();
        for relative in Self::candidate_patch_files(cwd).into_iter().take(5_000) {
            let candidate = cwd.join(&relative);
            let Ok(canonical_candidate) = candidate.canonicalize() else {
                continue;
            };
            if !canonical_candidate.starts_with(&canonical_cwd) || !canonical_candidate.is_file() {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(&canonical_candidate) else {
                continue;
            };
            if content.contains(old_string) {
                let tool_path = canonical_candidate
                    .strip_prefix(&canonical_cwd)
                    .ok()
                    .map(|path| path.to_string_lossy().to_string())
                    .unwrap_or_else(|| canonical_candidate.to_string_lossy().to_string());
                matches.push((canonical_candidate, tool_path));
            }
            if matches.len() > 1 {
                return None;
            }
        }
        matches.pop().map(Ok)
    }

    pub(super) fn candidate_patch_files(cwd: &std::path::Path) -> Vec<std::path::PathBuf> {
        let output = std::process::Command::new("git")
            .args(["ls-files"])
            .current_dir(cwd)
            .output();
        if let Ok(output) = output {
            if output.status.success() {
                let files = String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty())
                    .map(std::path::PathBuf::from)
                    .collect::<Vec<_>>();
                if !files.is_empty() {
                    return files;
                }
            }
        }

        let mut files = Vec::new();
        let mut stack = vec![cwd.to_path_buf()];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let file_name = entry.file_name();
                if path.is_dir() {
                    if matches!(
                        file_name.to_str(),
                        Some(".git" | "target" | "node_modules" | ".next" | "dist")
                    ) {
                        continue;
                    }
                    stack.push(path);
                    continue;
                }
                if path.is_file() {
                    if let Ok(relative) = path.strip_prefix(cwd) {
                        files.push(relative.to_path_buf());
                    }
                }
                if files.len() >= 5_000 {
                    return files;
                }
            }
        }
        files
    }

    pub(super) fn unknown_rust_enum_variant_in_patch(
        new_string: &str,
        cwd: &std::path::Path,
    ) -> Option<String> {
        let re = regex::Regex::new(r"\b([A-Z][A-Za-z0-9_]*)::([A-Z][A-Za-z0-9_]*)\b").ok()?;
        for captures in re.captures_iter(new_string) {
            let type_name = captures.get(1)?.as_str();
            let variant = captures.get(2)?.as_str();
            let Some(known_variants) = Self::known_rust_enum_variants(cwd, type_name) else {
                continue;
            };
            if !known_variants.contains(variant) {
                let mut known = known_variants.into_iter().collect::<Vec<_>>();
                known.sort();
                return Some(format!(
                    "synthesized patch uses unknown enum variant {}::{}; known variants: {}",
                    type_name,
                    variant,
                    known.join(", ")
                ));
            }
        }
        None
    }

    pub(super) fn known_rust_enum_variants(
        cwd: &std::path::Path,
        type_name: &str,
    ) -> Option<HashSet<String>> {
        for relative in Self::candidate_patch_files(cwd).into_iter().take(5_000) {
            if relative.extension().and_then(|ext| ext.to_str()) != Some("rs") {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(cwd.join(&relative)) else {
                continue;
            };
            let Some(body) = Self::extract_rust_enum_body(&content, type_name) else {
                continue;
            };
            let variants = body
                .lines()
                .filter_map(Self::rust_enum_variant_from_line)
                .collect::<HashSet<_>>();
            if !variants.is_empty() {
                return Some(variants);
            }
        }
        None
    }

    pub(super) fn extract_rust_enum_body(content: &str, type_name: &str) -> Option<String> {
        let needle = format!("enum {}", type_name);
        let start = content.find(&needle)?;
        let brace_start = content[start..].find('{')? + start;
        let mut depth = 0usize;
        for (offset, ch) in content[brace_start..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        let end = brace_start + offset;
                        return Some(content[brace_start + 1..end].to_string());
                    }
                }
                _ => {}
            }
        }
        None
    }

    pub(super) fn rust_enum_variant_from_line(line: &str) -> Option<String> {
        let trimmed = line.split("//").next().unwrap_or("").trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("}") {
            return None;
        }
        let ident = trimmed
            .chars()
            .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
            .collect::<String>();
        if ident
            .chars()
            .next()
            .map(|ch| ch.is_ascii_uppercase())
            .unwrap_or(false)
        {
            Some(ident)
        } else {
            None
        }
    }

    pub(super) fn unique_git_path_suffix_match(
        raw_path: &std::path::Path,
        cwd: &std::path::Path,
    ) -> Option<std::path::PathBuf> {
        let output = std::process::Command::new("git")
            .args(["ls-files"])
            .current_dir(cwd)
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let raw = raw_path
            .to_string_lossy()
            .trim_start_matches('/')
            .to_string();
        let file_name = raw_path.file_name()?.to_string_lossy().to_string();
        let mut matches = Vec::new();
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if line == raw || line.ends_with(&raw) || line.ends_with(&format!("/{}", file_name)) {
                matches.push(std::path::PathBuf::from(line));
            }
        }
        if matches.len() == 1 {
            matches.pop()
        } else {
            None
        }
    }

    pub(super) fn validate_synthesized_tool_call(
        &self,
        tool_call: ToolCall,
        cwd: &std::path::Path,
    ) -> Result<ToolCall> {
        if !matches!(tool_call.name.as_str(), "file_edit" | "file_write") {
            return Err(anyhow::anyhow!(
                "patch synthesis fallback returned unsupported tool: {}",
                tool_call.name
            ));
        }
        let action = PatchSynthesisAction {
            tool: tool_call.name.clone(),
            path: tool_call.arguments["path"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            old_string: tool_call.arguments["old_string"]
                .as_str()
                .map(str::to_string),
            new_string: tool_call.arguments["new_string"]
                .as_str()
                .or_else(|| tool_call.arguments["content"].as_str())
                .unwrap_or_default()
                .to_string(),
            line_start: tool_call.arguments["line_start"]
                .as_u64()
                .map(|value| value as usize),
            line_end: tool_call.arguments["line_end"]
                .as_u64()
                .map(|value| value as usize),
            expected_replacements: tool_call.arguments["expected_replacements"]
                .as_u64()
                .map(|value| value as usize),
        };
        self.validate_patch_synthesis_action(&action, cwd)
    }
}
