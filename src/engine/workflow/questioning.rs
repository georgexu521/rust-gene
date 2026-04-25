//! 主动提问式深思引擎
//!
//! 改造现有 SocraticSession，新增主动触发、动态生成、自问自答能力。
//! M1 范围：固定模板 fallback + 简化版 LLM 动态生成 + Budget 控制。

use super::policy::SocraticPolicy;
use crate::engine::socratic::QuestionType;
use crate::services::api::{ChatRequest, LlmProvider, Message};
use serde::{Deserialize, Serialize};

/// 问题节点（改造后的 QaPair）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionNode {
    pub id: String,
    pub question: String,
    pub answer: String,
    pub question_type: QuestionType,
    pub depth: usize,
    pub parent_id: Option<String>,
    pub child_ids: Vec<String>,
    pub mainline_relevance: f64,
    pub token_cost: usize,
    /// 从回答中提取的可执行证据项（命令/路径/断言）
    pub evidence_items: Vec<String>,
}

/// 思考成果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingResult {
    pub problem_statement: String,
    pub key_uncertainties: Vec<String>,
    pub decision_basis: String,
    pub question_chain: Vec<QuestionNode>,
    pub total_token_cost: usize,
    pub convergence_reason: String,
}

impl ThinkingResult {
    /// 格式化输出为 Markdown
    pub fn format_output(&self) -> String {
        let mut output = String::new();
        output.push_str("## 主动思考成果\n\n");
        output.push_str(&format!("### 问题本质\n{}\n\n", self.problem_statement));

        if !self.key_uncertainties.is_empty() {
            output.push_str("### 剩余不确定点\n");
            for u in &self.key_uncertainties {
                output.push_str(&format!("- {}\n", u));
            }
            output.push('\n');
        }

        output.push_str(&format!("### 决策依据\n{}\n\n", self.decision_basis));

        if !self.question_chain.is_empty() {
            output.push_str("### 思考过程\n");
            for node in &self.question_chain {
                output.push_str(&format!(
                    "**Q{}** [{}]: {}\n\n{}",
                    node.id,
                    node.question_type.label(),
                    node.question,
                    node.answer
                ));
                if !node.evidence_items.is_empty() {
                    output.push_str("\n证据项:\n");
                    for e in &node.evidence_items {
                        output.push_str(&format!("- {}\n", e));
                    }
                }
                output.push('\n');
            }
        }

        output.push_str(&format!(
            "\n---\n消耗: {} tokens, {} 个问题\n状态: {}\n",
            self.total_token_cost,
            self.question_chain.len(),
            self.convergence_reason
        ));
        output
    }

    /// 从思考成果中提取候选执行步骤（供 Planner 使用）
    pub fn extract_steps(&self) -> Vec<String> {
        let mut steps = Vec::new();

        // 从决策依据中提取动作
        let basis = &self.decision_basis;
        for line in basis.lines() {
            let trimmed = line.trim();
            let is_numbered = trimmed.len() >= 2
                && trimmed
                    .chars()
                    .next()
                    .map(|c| c.is_ascii_digit())
                    .unwrap_or(false)
                && trimmed.chars().nth(1) == Some('.');
            if is_numbered || trimmed.starts_with("-") || trimmed.starts_with("*") {
                let cleaned = trimmed
                    .trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == ' ')
                    .trim_start_matches(['-', '*', ' ']);
                if !cleaned.is_empty() && cleaned.len() > 10 {
                    steps.push(cleaned.to_string());
                }
            }
        }

        // 如果没有提取到步骤，使用问题本质作为单一步骤
        if steps.is_empty() {
            steps.push(self.problem_statement.clone());
        }

        steps
    }
}

/// Budget 追踪器
#[derive(Debug, Clone)]
pub struct BudgetTracker {
    pub max_rounds: usize,
    pub max_answer_tokens: usize,
    pub max_total_tokens: usize,
    pub used_rounds: usize,
    pub used_tokens: usize,
}

impl BudgetTracker {
    pub fn from_env() -> Self {
        Self {
            max_rounds: std::env::var("PRIORITY_AGENT_SOCRATIC_MAX_ROUNDS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(5),
            max_answer_tokens: std::env::var("PRIORITY_AGENT_SOCRATIC_ANSWER_BUDGET")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(500),
            max_total_tokens: std::env::var("PRIORITY_AGENT_SOCRATIC_TOTAL_BUDGET")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(3750),
            used_rounds: 0,
            used_tokens: 0,
        }
    }

    pub fn from_policy(policy: &SocraticPolicy) -> Self {
        Self {
            max_rounds: policy.max_rounds,
            max_answer_tokens: policy.max_answer_tokens,
            max_total_tokens: policy.max_total_tokens,
            used_rounds: 0,
            used_tokens: 0,
        }
    }

    pub fn can_proceed(&self) -> bool {
        self.used_rounds < self.max_rounds && self.used_tokens < self.max_total_tokens
    }

    pub fn remaining_answer_budget(&self) -> usize {
        let remaining = self.max_total_tokens.saturating_sub(self.used_tokens);
        remaining.min(self.max_answer_tokens)
    }

    pub fn consume(&mut self, tokens: usize) {
        self.used_rounds += 1;
        self.used_tokens += tokens;
    }

    pub fn is_exhausted(&self) -> bool {
        !self.can_proceed()
    }
}

/// 问题上下文
#[derive(Debug, Clone)]
pub struct QuestionContext {
    pub task_description: String,
    pub mainline_goal: String,
    pub previous_answers: Vec<QuestionNode>,
}

/// 主动提问式深思引擎
pub struct ActiveQuestioningEngine {
    task: String,
    mainline_goal: String,
    nodes: Vec<QuestionNode>,
    node_counter: usize,
}

#[derive(Debug, Clone)]
struct PendingQuestion {
    question: String,
    qtype: QuestionType,
    depth: usize,
    parent_id: Option<String>,
}

impl ActiveQuestioningEngine {
    pub fn new(task: String, mainline_goal: String) -> Self {
        Self {
            task,
            mainline_goal,
            nodes: Vec::new(),
            node_counter: 0,
        }
    }

    /// M1 核心方法：执行主动思考
    pub async fn think(
        &mut self,
        llm_provider: &dyn LlmProvider,
        model: &str,
    ) -> Result<ThinkingResult, String> {
        self.think_with_policy(llm_provider, model, &SocraticPolicy::default())
            .await
    }

    pub async fn think_with_policy(
        &mut self,
        llm_provider: &dyn LlmProvider,
        model: &str,
        policy: &SocraticPolicy,
    ) -> Result<ThinkingResult, String> {
        let mut budget = BudgetTracker::from_policy(policy);
        let max_depth = policy.max_depth;
        let mut queue = self.generate_seed_questions();

        while let Some(pending) = queue.pop_front() {
            if !budget.can_proceed() {
                break;
            }

            // LLM 自答
            let answer = self
                .self_answer(llm_provider, model, &pending.question, &budget)
                .await?;

            let cost = estimate_token_cost(&pending.question, &answer);
            budget.consume(cost);

            let node = self.record_qa(
                pending.question,
                answer,
                pending.qtype,
                pending.depth,
                pending.parent_id,
            );

            // 检查收敛
            if let Some(reason) = self.check_convergence() {
                return Ok(self.build_result(reason, &budget));
            }

            // 检查是否形成可执行计划
            if self.has_executable_plan(&node) {
                return Ok(self.build_result("executable_plan_formed".into(), &budget));
            }

            // 动态追问（递进式）
            if pending.depth < max_depth {
                if let Some(next) = self.generate_followup_question(&node, pending.depth + 1) {
                    queue.push_back(next);
                }
            }
        }

        // Budget 耗尽或问题列表遍历完
        let reason = if budget.is_exhausted() {
            "budget_exhausted".to_string()
        } else {
            "question_queue_exhausted".to_string()
        };
        Ok(self.build_result(reason, &budget))
    }

    fn generate_seed_questions(&self) -> std::collections::VecDeque<PendingQuestion> {
        let mut q = std::collections::VecDeque::new();
        q.push_back(PendingQuestion {
            question: format!(
                "任务「{}」的最核心目标是什么？去掉表面需求后，本质问题是什么？",
                truncate(&self.task, 80)
            ),
            qtype: QuestionType::GoalClarification,
            depth: 0,
            parent_id: None,
        });
        q.push_back(PendingQuestion {
            question: "要完成这个目标，哪些前提和约束必须先满足？".to_string(),
            qtype: QuestionType::PrerequisiteCheck,
            depth: 0,
            parent_id: None,
        });
        q.push_back(PendingQuestion {
            question: "最大的风险是什么？最可能出错的环节在哪里？".to_string(),
            qtype: QuestionType::RiskAssessment,
            depth: 0,
            parent_id: None,
        });
        if is_complex_task(&self.task) {
            q.push_back(PendingQuestion {
                question: "如果要避免跑偏，应该如何定义主线与非主线边界？".to_string(),
                qtype: QuestionType::GoalClarification,
                depth: 0,
                parent_id: None,
            });
        }
        q.push_back(PendingQuestion {
            question: "基于以上分析，最终执行方案是什么？请列出步骤。".to_string(),
            qtype: QuestionType::Reflection,
            depth: 0,
            parent_id: None,
        });
        q
    }

    /// LLM 自答
    async fn self_answer(
        &self,
        llm_provider: &dyn LlmProvider,
        model: &str,
        question: &str,
        budget: &BudgetTracker,
    ) -> Result<String, String> {
        let system_prompt = "你是一个深度思考助手。用户会给你一个编程任务和一个探索性问题，请用简洁但深刻的中文回答。直接回答问题，不要废话。";

        let user_message = format!(
            "任务：{}\n主线目标：{}\n\n问题：{}\n\n请直接给出你的分析和结论（不超过 {} tokens）：",
            self.task,
            self.mainline_goal,
            question,
            budget.remaining_answer_budget()
        );

        let request = ChatRequest::new(model)
            .with_messages(vec![
                Message::system(system_prompt),
                Message::user(&user_message),
            ])
            .with_temperature(0.7);

        match llm_provider.chat(request).await {
            Ok(response) => Ok(response.content),
            Err(e) => Err(format!("LLM 调用失败: {}", e)),
        }
    }

    /// 记录 Q&A
    fn record_qa(
        &mut self,
        question: String,
        answer: String,
        qtype: QuestionType,
        depth: usize,
        parent_id: Option<String>,
    ) -> QuestionNode {
        self.node_counter += 1;
        let id = format!("Q-{}", self.node_counter);

        let relevance = compute_mainline_relevance(&question, &self.mainline_goal);
        let cost = estimate_token_cost(&question, &answer);
        let evidence_items = extract_evidence_items(&answer);

        let node = QuestionNode {
            id: id.clone(),
            question,
            answer,
            question_type: qtype,
            depth,
            parent_id,
            child_ids: Vec::new(),
            mainline_relevance: relevance,
            token_cost: cost,
            evidence_items,
        };

        self.nodes.push(node.clone());
        node
    }

    fn generate_followup_question(
        &self,
        node: &QuestionNode,
        depth: usize,
    ) -> Option<PendingQuestion> {
        let answer = node.answer.to_lowercase();
        let followup = if answer.contains("不确定")
            || answer.contains("可能")
            || answer.contains("风险")
            || answer.contains("依赖")
        {
            Some((
                "针对你刚提到的不确定点，最小验证步骤是什么？".to_string(),
                QuestionType::RiskAssessment,
            ))
        } else if node.question_type == QuestionType::PrerequisiteCheck {
            Some((
                "这些前提里，哪个是当前真正的 blocker？为什么？".to_string(),
                QuestionType::GoalClarification,
            ))
        } else if node.question_type == QuestionType::GoalClarification && depth <= 2 {
            Some((
                "若只允许先做一件事，哪一步最能推动主线？".to_string(),
                QuestionType::Reflection,
            ))
        } else {
            None
        }?;

        Some(PendingQuestion {
            question: followup.0,
            qtype: followup.1,
            depth,
            parent_id: Some(node.id.clone()),
        })
    }

    /// 检查收敛条件
    fn check_convergence(&self) -> Option<String> {
        // 条件 1：关键不确定点 <= 1
        let uncertainties = extract_uncertainties(&self.nodes);
        if uncertainties.len() <= 1 && !self.nodes.is_empty() {
            return Some("uncertainties_low".into());
        }

        // 条件 2：连续 2 轮没有新信息
        if self.nodes.len() >= 2 {
            let last_two = &self.nodes[self.nodes.len() - 2..];
            if last_two.iter().all(|n| n.answer.len() < 20) {
                return Some("diminishing_returns".into());
            }
        }

        None
    }

    /// 检查是否形成可执行计划
    fn has_executable_plan(&self, node: &QuestionNode) -> bool {
        // Reflection 类型的问题如果答案包含步骤列表，视为可执行
        if node.question_type == QuestionType::Reflection {
            let a = &node.answer;
            (a.contains("1.") || a.contains("2.") || a.contains("-") || a.contains("*"))
                && !node.evidence_items.is_empty()
        } else {
            false
        }
    }

    /// 构建最终结果
    fn build_result(&self, reason: String, budget: &BudgetTracker) -> ThinkingResult {
        let problem_statement = self.extract_problem_statement();
        let uncertainties = extract_uncertainties(&self.nodes);
        let decision_basis = self.extract_decision_basis();

        ThinkingResult {
            problem_statement,
            key_uncertainties: uncertainties,
            decision_basis,
            question_chain: self.nodes.clone(),
            total_token_cost: budget.used_tokens,
            convergence_reason: reason,
        }
    }

    fn extract_problem_statement(&self) -> String {
        // 从第一个 GoalClarification 答案中提取
        self.nodes
            .iter()
            .find(|n| n.question_type == QuestionType::GoalClarification)
            .map(|n| n.answer.clone())
            .unwrap_or_else(|| self.task.clone())
    }

    fn extract_decision_basis(&self) -> String {
        // 从 Reflection 答案中提取
        self.nodes
            .iter()
            .find(|n| n.question_type == QuestionType::Reflection)
            .map(|n| n.answer.clone())
            .unwrap_or_else(|| "未形成明确决策".into())
    }
}

// ============================================================================
// 工具函数
// ============================================================================

/// 计算问题与主线的相关度
fn compute_mainline_relevance(question: &str, mainline: &str) -> f64 {
    let q_lower = question.to_lowercase();
    let m_lower = mainline.to_lowercase();

    // 简单字符匹配
    let q_chars: std::collections::HashSet<char> = q_lower.chars().collect();
    let m_chars: std::collections::HashSet<char> = m_lower.chars().collect();

    if q_chars.is_empty() || m_chars.is_empty() {
        return 0.0;
    }

    let intersection: std::collections::HashSet<_> = q_chars.intersection(&m_chars).collect();
    let ratio = intersection.len() as f64 / m_chars.len().max(q_chars.len()) as f64;
    ratio.clamp(0.0, 1.0)
}

/// 从答案中提取不确定性
fn extract_uncertainties(nodes: &[QuestionNode]) -> Vec<String> {
    let mut uncertainties = Vec::new();

    for node in nodes {
        if node.question_type == QuestionType::RiskAssessment
            || node.question_type == QuestionType::PrerequisiteCheck
        {
            for line in node.answer.lines() {
                let trimmed = line.trim();
                if (trimmed.contains("不确定")
                    || trimmed.contains("可能")
                    || trimmed.contains("风险")
                    || trimmed.contains("问题"))
                    && trimmed.len() > 10
                    && trimmed.len() < 200
                {
                    uncertainties.push(trimmed.to_string());
                }
            }
        }
    }

    // 去重
    uncertainties.sort();
    uncertainties.dedup();
    uncertainties.truncate(5);
    uncertainties
}

/// 估算 token 成本（简化为中文字符数 / 2 + 英文单词数）
fn estimate_token_cost(question: &str, answer: &str) -> usize {
    let text = format!("{} {}", question, answer);
    let chinese_chars = text
        .chars()
        .filter(|c| matches!(c, '\u{4e00}'..='\u{9fff}'))
        .count();
    let english_words = text.split_whitespace().count();
    (chinese_chars / 2 + english_words).max(1)
}

fn extract_evidence_items(answer: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in answer.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        let lower = t.to_lowercase();
        let has_cmd = lower.contains("cargo ")
            || lower.contains("git ")
            || lower.contains("bash ")
            || lower.contains("test ")
            || lower.contains("run ");
        let has_path = t.contains('/') || t.ends_with(".rs") || t.ends_with(".md");
        let has_assert = lower.contains("必须")
            || lower.contains("should")
            || lower.contains("assert")
            || lower.contains("验收");
        if has_cmd || has_path || has_assert {
            out.push(t.to_string());
        }
        if out.len() >= 5 {
            break;
        }
    }
    out
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", s.chars().take(max).collect::<String>())
    }
}

fn is_complex_task(task: &str) -> bool {
    let t = task.to_lowercase();
    let markers = [
        "重构",
        "架构",
        "迁移",
        "跨模块",
        "系统",
        "workflow",
        "refactor",
        "architecture",
        "migrate",
        "multi",
    ];
    markers.iter().any(|m| t.contains(m))
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thinking_result_extract_steps() {
        let result = ThinkingResult {
            problem_statement: "实现认证".into(),
            key_uncertainties: vec![],
            decision_basis: "1. 设计数据库\n2. 实现登录\n3. 实现注册".into(),
            question_chain: vec![],
            total_token_cost: 100,
            convergence_reason: "test".into(),
        };
        let steps = result.extract_steps();
        assert_eq!(steps.len(), 3);
        assert!(steps[0].contains("数据库"));
    }

    #[test]
    fn test_budget_tracker() {
        std::env::set_var("PRIORITY_AGENT_SOCRATIC_MAX_ROUNDS", "3");
        let bt = BudgetTracker::from_env();
        assert_eq!(bt.max_rounds, 3);
        assert!(bt.can_proceed());

        let mut bt = bt;
        bt.consume(100);
        bt.consume(100);
        bt.consume(100);
        assert!(!bt.can_proceed());

        std::env::remove_var("PRIORITY_AGENT_SOCRATIC_MAX_ROUNDS");
    }

    #[test]
    fn test_estimate_token_cost() {
        let cost = estimate_token_cost("问题", "答案是中文内容");
        assert!(cost > 0);
    }

    #[test]
    fn test_compute_mainline_relevance() {
        let r = compute_mainline_relevance("实现登录", "实现用户认证");
        assert!(r > 0.0);

        let r2 = compute_mainline_relevance("修复 typo", "实现用户认证");
        assert!(r2 < r);
    }

    #[test]
    fn test_extract_uncertainties() {
        let nodes = vec![QuestionNode {
            id: "Q1".into(),
            question: "风险？".into(),
            answer: "不确定数据库选型。可能有并发问题。".into(),
            question_type: QuestionType::RiskAssessment,
            depth: 0,
            parent_id: None,
            child_ids: vec![],
            mainline_relevance: 0.8,
            token_cost: 10,
            evidence_items: vec![],
        }];
        let u = extract_uncertainties(&nodes);
        assert!(!u.is_empty());
    }
}
