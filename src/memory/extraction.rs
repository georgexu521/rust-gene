//! 记忆提取管道
//!
//! LLM 驱动的记忆提取编排：启发式规则 + LLM 增强提取 + 后台 fork + trailing run。
//! 所有方法都是 `MemoryManager` 的薄编排，调用 `ingest_learnings` 和
//! `submit_candidate_with_provider_notifications` 完成实际的记忆写入。

use super::manager::{
    kind_label, memory_llm_timeout, MemoryManager, MemoryWriteTarget,
    MAX_LEARNINGS_PER_SESSION_EXTRACT, MAX_LEARNINGS_PER_TURN,
};
use super::provider::MemoryProviderCallStatus;
use super::types::{
    MemoryCandidate, MemoryEvidenceKind, MemoryEvidenceRef, MemoryKind, MemoryProvenance,
    MemoryScope, MemoryStrategyMetadata,
};
use crate::engine::task_contract::{
    MemoryProposal, MemoryProposalCandidate, MemoryProposalReviewStore, MemoryProposalStatus,
};
use crate::services::api::{ChatRequest, LlmProvider, Message};
use std::sync::Arc;
use tracing::{debug, info, warn};

// ---------------------------------------------------------------------------
// impl MemoryManager 方法
// ---------------------------------------------------------------------------

impl MemoryManager {
    /// 同步：保存本轮对话中学习到的内容（启发式提取）
    pub fn sync_turn(&mut self, user: &str, assistant: &str) {
        let learnings = extract_learnings_from_turn(user, assistant);
        self.ingest_learnings(learnings, MAX_LEARNINGS_PER_TURN);
    }

    /// 同步：保存本轮对话中学习到的内容（支持 LLM 增强提取）
    pub async fn sync_turn_llm(
        &mut self,
        user: &str,
        assistant: &str,
        provider: Option<&dyn LlmProvider>,
        model: &str,
    ) {
        self.mark_llm_extraction_started();

        // 先尝试启发式提取
        let heuristic = extract_learnings_from_turn(user, assistant);
        self.ingest_learnings(heuristic.clone(), MAX_LEARNINGS_PER_TURN);

        // 若启发式无结果且启用了 LLM 提取，则调用 LLM
        if heuristic.is_empty() {
            if let Some(p) = provider {
                let llm_candidates = self
                    .extract_memory_candidates_with_llm(user, assistant, p, model)
                    .await;
                for candidate in llm_candidates.into_iter().take(MAX_LEARNINGS_PER_TURN) {
                    self.submit_candidate_with_provider_notifications(
                        candidate,
                        MemoryWriteTarget::Auto,
                    )
                    .await;
                }
            }
        }
    }

    /// 后台 LLM 记忆提取（不阻塞主对话循环）
    ///
    /// 使用 `spawn` 在后台 fork 一个 task 进行 LLM 调用，
    /// 主对话循环不会被 LLM 延迟阻塞。
    pub fn sync_turn_llm_background(
        &self,
        user: String,
        assistant: String,
        provider: Arc<dyn LlmProvider>,
        model: String,
    ) {
        // 在 spawn 之前提取需要的字段，避免生命周期问题
        let forked_mode = self.forked_mode;
        let active_scope = self.active_scope.clone();

        tokio::spawn(async move {
            // 小延迟，让主对话先完成响应
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;

            let heuristic = extract_learnings_from_turn(&user, &assistant);

            // 在 forked 模式下，先写启发式结果作为 cache hit，再调用 LLM 增强
            // 在默认模式下，只有启发式无结果时才调用 LLM
            if forked_mode && !heuristic.is_empty() {
                let candidates = heuristic
                    .iter()
                    .take(MAX_LEARNINGS_PER_TURN)
                    .map(|learning| MemoryProposalCandidate {
                        kind: "note".to_string(),
                        scope: "project".to_string(),
                        content: learning.clone(),
                        evidence: vec![
                            "background_heuristic".to_string(),
                            format!("session_scope: {}", active_scope.session_id),
                        ],
                    })
                    .collect::<Vec<_>>();
                let source_task = format!(
                    "{}-background-heuristic-{}",
                    active_scope.session_id,
                    chrono::Utc::now().timestamp_millis()
                );
                upsert_background_memory_proposal(
                    &source_task,
                    candidates,
                    "background heuristic produced review-required proposal candidates",
                );
                debug!(
                    "Forked mode: proposed {} heuristic memory bullets for review",
                    heuristic.len()
                );
            }

            // 然后调用 LLM 进行增强提取（forked 模式）或备用提取（默认模式）
            let should_llm_extract = heuristic.is_empty() || forked_mode;

            if should_llm_extract {
                let system_prompt = "You are a memory extraction assistant. \
Analyze the conversation turn and propose up to 3 long-term memory candidates only. \
Return JSON: {\"memory_candidates\":[{\"type\":\"project_fact|user_preference|strategy|failure_lesson|note\",\"content\":\"...\",\"evidence\":\"...\",\"confidence\":0.0,\"importance\":1,\"tags\":[\"...\"]}]}. \
Only include facts supported by the turn. Do not save task progress, command history, or repeatable procedures; procedures belong in skills. Return exactly NONE if there is nothing critical to remember.";

                let content = format!(
                    "User:\n{}\n\nAssistant:\n{}\n",
                    user,
                    assistant.chars().take(4000).collect::<String>()
                );

                let request = ChatRequest::new(&model).with_messages(vec![
                    Message::system(system_prompt),
                    Message::user(&content),
                ]);

                if let Ok(Ok(response)) =
                    tokio::time::timeout(memory_llm_timeout(), provider.chat(request)).await
                {
                    let text = response.content.trim();
                    if !text.eq_ignore_ascii_case("NONE") && !text.is_empty() {
                        let candidates = parse_llm_memory_candidates(
                            text,
                            active_scope.clone(),
                            MemoryProvenance::local("background_llm"),
                        );
                        debug!(
                            "Background LLM extracted {} memory candidates (forked: {})",
                            candidates.len(),
                            forked_mode
                        );
                        let proposal_candidates = candidates
                            .into_iter()
                            .take(MAX_LEARNINGS_PER_TURN)
                            .map(memory_candidate_to_proposal_candidate)
                            .collect::<Vec<_>>();
                        let source_task = format!(
                            "{}-background-llm-{}",
                            active_scope.session_id,
                            chrono::Utc::now().timestamp_millis()
                        );
                        upsert_background_memory_proposal(
                            &source_task,
                            proposal_candidates,
                            "background LLM produced review-required proposal candidates",
                        );
                    }
                }
            }
        });
    }

    /// 使用 LLM 从对话中提取记忆
    pub(super) async fn extract_memory_candidates_with_llm(
        &self,
        user: &str,
        assistant: &str,
        provider: &dyn LlmProvider,
        model: &str,
    ) -> Vec<MemoryCandidate> {
        let system_prompt = "You are a memory extraction assistant. \
Analyze the conversation turn and propose up to 3 long-term memory candidates only. \
Return JSON: {\"memory_candidates\":[{\"type\":\"project_fact|user_preference|strategy|failure_lesson|note\",\"content\":\"...\",\"evidence\":\"...\",\"confidence\":0.0,\"importance\":1,\"tags\":[\"...\"]}]}. \
Only include facts supported by the turn. Do not save task progress, command history, or repeatable procedures; procedures belong in skills. Return exactly NONE if there is nothing critical to remember.";

        let content = format!(
            "User:\n{}\n\nAssistant:\n{}\n",
            user,
            assistant.chars().take(4000).collect::<String>()
        );

        let request = ChatRequest::new(model).with_messages(vec![
            Message::system(system_prompt),
            Message::user(&content),
        ]);

        match tokio::time::timeout(memory_llm_timeout(), provider.chat(request)).await {
            Ok(Ok(response)) => {
                let text = response.content.trim();
                if text.eq_ignore_ascii_case("NONE") || text.is_empty() {
                    return Vec::new();
                }
                let candidates = parse_llm_memory_candidates(
                    text,
                    self.active_scope.clone(),
                    MemoryProvenance::local("turn_llm_memory_extraction"),
                );
                debug!("LLM extracted {} memory candidates", candidates.len());
                candidates
            }
            Ok(Err(e)) => {
                warn!("LLM memory extraction failed: {}", e);
                Vec::new()
            }
            Err(_) => {
                warn!(
                    "LLM memory extraction timed out after {}s",
                    memory_llm_timeout().as_secs()
                );
                Vec::new()
            }
        }
    }

    /// Trailing run：会话结束时执行最终记忆提取
    ///
    /// 在 trailing_mode 启用时，会话结束后调用此方法进行最终 LLM 提取。
    /// 这确保对话结束后仍有一次记忆提取机会，捕获会话中学到的关键信息。
    pub async fn trailing_run(
        &mut self,
        messages: &[Message],
        provider: Option<&dyn LlmProvider>,
        model: &str,
    ) {
        if !self.trailing_mode {
            return;
        }
        if self.trailing_completed {
            debug!("Trailing run already completed, skipping");
            return;
        }

        info!(
            "Running trailing memory extraction for {} messages",
            messages.len()
        );
        let provider_outcomes = self
            .provider_registry
            .on_session_end_all(messages, &self.active_scope)
            .await;
        for outcome in provider_outcomes {
            if outcome.status != MemoryProviderCallStatus::Ok {
                debug!(
                    "Memory provider session-end hook {:?}: provider={} error={:?}",
                    outcome.status, outcome.provider, outcome.error
                );
            }
        }

        // 收集会话中的 user/assistant 对话内容
        let mut conversation_context = String::new();
        for msg in messages.iter().rev().take(20) {
            // 取最近 20 条消息
            match msg {
                Message::User { content } => {
                    conversation_context.push_str(&format!("User: {}\n", content));
                }
                Message::Assistant { content, .. } => {
                    conversation_context.push_str(&format!("Assistant: {}\n", content));
                }
                _ => {}
            }
        }

        if conversation_context.len() < 50 {
            debug!("Not enough conversation context for trailing extraction");
            return;
        }

        if let Some(p) = provider {
            let system_prompt = "You are a memory extraction assistant. \
Analyze this entire conversation session and propose up to 6 critical long-term memory candidates. \
Critical context includes: API keys or paths, architecture decisions, user preferences, \
specific error messages and their fixes, project conventions, important configuration values, \
or key decisions made during the session. \
Return JSON: {\"memory_candidates\":[{\"type\":\"project_fact|user_preference|strategy|failure_lesson|note\",\"content\":\"...\",\"evidence\":\"...\",\"confidence\":0.0,\"importance\":1,\"tags\":[\"...\"]}]}. \
Do not save task progress, command history, or repeatable procedures; procedures belong in skills. Return exactly the word NONE if there is nothing critical to remember.";

            let request = ChatRequest::new(model).with_messages(vec![
                Message::system(system_prompt),
                Message::user(&conversation_context),
            ]);

            match tokio::time::timeout(memory_llm_timeout(), p.chat(request)).await {
                Ok(Ok(response)) => {
                    let text = response.content.trim();
                    if !text.eq_ignore_ascii_case("NONE") && !text.is_empty() {
                        let candidates = parse_llm_memory_candidates(
                            text,
                            self.active_scope.clone(),
                            MemoryProvenance::local("trailing_llm_memory_extraction"),
                        );

                        debug!(
                            "Trailing run extracted {} memory candidates",
                            candidates.len()
                        );
                        let proposal_candidates = candidates
                            .into_iter()
                            .take(MAX_LEARNINGS_PER_SESSION_EXTRACT)
                            .map(memory_candidate_to_proposal_candidate)
                            .collect::<Vec<_>>();
                        let source_task = format!(
                            "{}-trailing-llm-{}",
                            self.active_scope.session_id,
                            chrono::Utc::now().timestamp_millis()
                        );
                        upsert_background_memory_proposal(
                            &source_task,
                            proposal_candidates,
                            "trailing LLM extraction produced review-required proposal candidates",
                        );
                    }
                }
                Ok(Err(e)) => {
                    warn!("Trailing run LLM extraction failed: {}", e);
                }
                Err(_) => {
                    warn!(
                        "Trailing run LLM extraction timed out after {}s",
                        memory_llm_timeout().as_secs()
                    );
                }
            }
        }

        self.trailing_completed = true;
    }
}

// ---------------------------------------------------------------------------
// 自由函数
// ---------------------------------------------------------------------------

fn upsert_background_memory_proposal(
    source_task: &str,
    candidates: Vec<MemoryProposalCandidate>,
    reason: impl Into<String>,
) {
    if candidates.is_empty() {
        return;
    }
    let proposal = MemoryProposal {
        task_id: format!("background-{source_task}"),
        source: "background".to_string(),
        status: MemoryProposalStatus::Proposed,
        candidates,
        write_policy: "review_required".to_string(),
        write_performed: false,
        reason: reason.into(),
    };
    let _ = MemoryProposalReviewStore::default().upsert(&proposal);
}

fn memory_candidate_to_proposal_candidate(candidate: MemoryCandidate) -> MemoryProposalCandidate {
    MemoryProposalCandidate {
        kind: kind_label(candidate.kind).to_string(),
        scope: "project".to_string(),
        content: candidate.content,
        evidence: candidate
            .evidence
            .into_iter()
            .map(|evidence| {
                format!(
                    "{:?}: {} ({})",
                    evidence.kind, evidence.summary, evidence.source
                )
            })
            .collect(),
    }
}

pub(super) fn parse_llm_memory_candidates(
    content: &str,
    scope: MemoryScope,
    provenance: MemoryProvenance,
) -> Vec<MemoryCandidate> {
    let trimmed = content.trim();
    if trimmed.eq_ignore_ascii_case("NONE") || trimmed.is_empty() {
        return Vec::new();
    }
    if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
        if start <= end {
            let json = &trimmed[start..=end];
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(json) {
                if let Some(candidates) = value
                    .get("memory_candidates")
                    .and_then(serde_json::Value::as_array)
                {
                    return candidates
                        .iter()
                        .filter_map(|candidate| {
                            memory_candidate_from_json(candidate, &scope, &provenance)
                        })
                        .take(6)
                        .collect();
                }
            }
        }
    }

    trimmed
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            let content = line.strip_prefix("- ").unwrap_or(line).trim();
            if content.is_empty() {
                return None;
            }
            let mut candidate =
                MemoryCandidate::new(content, "note", scope.clone(), provenance.clone())
                    .confidence(0.45)
                    .with_tags(infer_memory_tags(content, "note"));
            candidate.evidence.push(MemoryEvidenceRef::inferred(
                provenance.source.clone(),
                "legacy free-form LLM memory bullet",
            ));
            Some(candidate)
        })
        .take(6)
        .collect()
}

fn memory_candidate_from_json(
    value: &serde_json::Value,
    scope: &MemoryScope,
    provenance: &MemoryProvenance,
) -> Option<MemoryCandidate> {
    let content = value
        .get("content")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|content| !content.is_empty())?;
    let raw_type = value
        .get("type")
        .or_else(|| value.get("kind"))
        .or_else(|| value.get("category"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("note");
    let category = normalize_llm_memory_category(raw_type);
    let confidence = value
        .get("confidence")
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(0.55) as f32;
    let importance = value
        .get("importance")
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u8::try_from(value).ok())
        .unwrap_or(3);
    let mut tags = value
        .get("tags")
        .and_then(serde_json::Value::as_array)
        .map(|tags| {
            tags.iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|tag| !tag.is_empty())
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    tags.extend(infer_memory_tags(content, &category));
    tags.sort();
    tags.dedup();

    let mut candidate =
        MemoryCandidate::new(content, category.clone(), scope.clone(), provenance.clone())
            .confidence(confidence)
            .importance(importance)
            .with_tags(tags)
            .explicit(category == "preference");
    let evidence_summary = value
        .get("evidence")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|evidence| !evidence.is_empty())
        .unwrap_or("LLM proposed this candidate from conversation context");
    candidate.evidence.push(MemoryEvidenceRef::new(
        llm_memory_evidence_kind(&category, provenance),
        provenance.source.clone(),
        evidence_summary,
        confidence.clamp(0.0, 1.0),
    ));

    if matches!(
        candidate.kind,
        MemoryKind::FailurePattern | MemoryKind::SuccessfulFix
    ) {
        let failed_strategy = json_string(value, "failed_strategy");
        let better_strategy = json_string(value, "better_strategy").or_else(|| {
            if candidate.kind == MemoryKind::SuccessfulFix {
                Some(content.to_string())
            } else {
                None
            }
        });
        let failure_type = json_string(value, "failure_type");
        let recovery_plan_id = json_string(value, "recovery_plan_id");
        let context_tags = candidate.tags.clone();
        candidate.strategy = Some(MemoryStrategyMetadata {
            failed_strategy,
            better_strategy,
            context_tags,
            failure_type,
            recovery_plan_id,
            risk_modifier: if candidate.kind == MemoryKind::FailurePattern {
                1
            } else {
                0
            },
            value_modifier: if candidate.kind == MemoryKind::SuccessfulFix {
                1
            } else {
                0
            },
        });
    }
    Some(candidate)
}

fn normalize_llm_memory_category(raw_type: &str) -> String {
    match raw_type.trim().to_ascii_lowercase().as_str() {
        "user_preference" | "preference" | "user" => "preference",
        "project_fact" | "fact" | "project" => "project_fact",
        "failure_lesson" | "failure" | "failure_pattern" => "failure",
        "successful_strategy" | "successful_fix" | "success" | "strategy" => "success",
        "workflow_convention" | "convention" => "convention",
        "decision" | "workflow" => "decision",
        "tool_quirk" | "tool" => "tool",
        "skill" | "skill_candidate" => "skill",
        _ => "note",
    }
    .to_string()
}

fn llm_memory_evidence_kind(category: &str, provenance: &MemoryProvenance) -> MemoryEvidenceKind {
    if category == "preference" {
        return MemoryEvidenceKind::UserStatement;
    }
    let source = provenance.source.to_ascii_lowercase();
    if source.contains("trace") || source.contains("stop") || source.contains("recovery") {
        MemoryEvidenceKind::RuntimeObservation
    } else {
        MemoryEvidenceKind::Inference
    }
}

fn json_string(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
}

/// 从单轮对话中提取学习内容
fn extract_learnings_from_turn(user: &str, assistant: &str) -> Vec<String> {
    let mut learnings = Vec::new();

    // 检测用户偏好信号
    let user_lower = user.to_lowercase();
    if user_lower.contains("我喜欢")
        || user_lower.contains("i prefer")
        || user_lower.contains("我更喜欢")
    {
        learnings.push(format!("User preference: {}", user));
    }

    // 检测问题解决模式
    let assistant_lower = assistant.to_lowercase();
    if assistant_lower.contains("解决方案")
        || assistant_lower.contains("solution")
        || assistant_lower.contains("修复方法")
        || assistant_lower.contains("workaround")
    {
        // 提取解决方案段落
        for line in assistant.lines() {
            let line_lower = line.to_lowercase();
            if (line_lower.contains("解决")
                || line_lower.contains("fix")
                || line_lower.contains("方法")
                || line_lower.contains("approach"))
                && line.len() > 20
                && line.len() < 500
            {
                learnings.push(format!("Solution: {}", line.trim()));
            }
        }
    }

    // 检测错误和教训
    if assistant_lower.contains("error")
        || assistant_lower.contains("错误")
        || assistant_lower.contains("失败")
        || assistant_lower.contains("failed")
    {
        for line in assistant.lines() {
            if line.len() > 30
                && line.len() < 300
                && (line.to_lowercase().contains("error") || line.contains("错误"))
            {
                learnings.push(format!("Lesson: {}", line.trim()));
            }
        }
    }

    learnings
}

/// 从完整会话中提取学习内容
pub(super) fn extract_session_learnings(messages: &[Message]) -> Vec<String> {
    let mut learnings = Vec::new();

    // 统计工具使用频率
    let mut tool_usage: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for msg in messages {
        if let Message::Assistant {
            tool_calls: Some(calls),
            ..
        } = msg
        {
            for tc in calls {
                *tool_usage.entry(tc.name.clone()).or_insert(0) += 1;
            }
        }
    }

    // 记录高频工具
    for (tool, count) in &tool_usage {
        if *count >= 3 {
            learnings.push(format!("Frequently used tool: {} ({} times)", tool, count));
        }
    }

    // 检测成功的模式
    let all_content: String = messages
        .iter()
        .filter_map(|m| match m {
            Message::Assistant { content, .. } => Some(content.as_str()),
            Message::Tool { content, .. } => Some(content.as_str()),
            _ => None,
        })
        .collect();

    if all_content.contains("✅") || all_content.contains("success") || all_content.contains("完成")
    {
        // 找到成功的上下文
        for msg in messages.iter().rev().take(5) {
            if let Message::User { content } = msg {
                if content.len() > 20 && content.len() < 200 {
                    learnings.push(format!("Successful task pattern: {}", content.trim()));
                    break;
                }
            }
        }
    }

    learnings
}

pub(super) fn infer_memory_tags(content: &str, category: &str) -> Vec<String> {
    let lower = content.to_lowercase();
    let mut tags = vec![category.to_lowercase()];
    for (tag, markers) in [
        ("testing", &["test", "cargo test", "pytest", "测试"][..]),
        ("rust", &["rust", "cargo", ".rs", "crate"][..]),
        ("memory", &["memory", "remember", "记忆"][..]),
        ("tool", &["tool", "bash", "mcp", "工具"][..]),
        ("failure", &["error", "failed", "失败", "错误"][..]),
        (
            "strategy",
            &["strategy", "solution", "fix", "策略", "修复"][..],
        ),
        ("preference", &["prefer", "preference", "偏好", "喜欢"][..]),
        ("project", &["project", "repo", "项目", "仓库"][..]),
    ] {
        if markers.iter().any(|marker| lower.contains(marker)) {
            tags.push(tag.to_string());
        }
    }
    tags.sort();
    tags.dedup();
    tags
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_turn_learnings_from_preferences_and_solution_text() {
        let learnings = extract_learnings_from_turn(
            "I prefer using async/await",
            "Sure, here's the solution using async/await...",
        );

        assert!(!learnings.is_empty());
    }
}
