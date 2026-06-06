//! Socratic 提问引擎
//!
//! 核心理念：高密度思考 = 高密度提问-解答循环
//!
//! 工作流程：
//! 1. 用户提出任务
//! 2. Socratic 引擎自动生成一系列探索性问题
//! 3. 每个问题由 LLM 回答
//! 4. 答案触发新问题（递归深化）
//! 5. 所有 Q&A 汇总为深度推理链
//! 6. 基于推理链生成最终执行计划

use serde::{Deserialize, Serialize};
use serde_json::json;

/// 问题类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum QuestionType {
    /// 目标澄清
    GoalClarification,
    /// 前提检查
    PrerequisiteCheck,
    /// 风险评估
    RiskAssessment,
    /// 方案优化
    SolutionOptimization,
    /// 反例检验
    CounterExample,
    /// 反思总结
    Reflection,
}

impl QuestionType {
    pub fn label(&self) -> &'static str {
        match self {
            QuestionType::GoalClarification => "目标",
            QuestionType::PrerequisiteCheck => "前提",
            QuestionType::RiskAssessment => "风险",
            QuestionType::SolutionOptimization => "优化",
            QuestionType::CounterExample => "反例",
            QuestionType::Reflection => "反思",
        }
    }

    /// 问题类型的探索顺序
    pub fn exploration_sequence() -> Vec<QuestionType> {
        vec![
            QuestionType::GoalClarification,
            QuestionType::PrerequisiteCheck,
            QuestionType::RiskAssessment,
            QuestionType::SolutionOptimization,
            QuestionType::CounterExample,
            QuestionType::Reflection,
        ]
    }
}

/// 一个 Q&A 对
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QaPair {
    pub question: String,
    pub answer: String,
    pub question_type: QuestionType,
    pub depth: usize,
    pub leads_to: Vec<String>, // 这个答案引出的新问题
}

/// Socratic 会话
#[derive(Debug, Clone)]
pub struct SocraticSession {
    /// 原始任务
    pub task: String,
    /// Q&A 链
    pub qa_chain: Vec<QaPair>,
    /// 当前深度
    pub current_depth: usize,
    /// 最大深度
    pub max_depth: usize,
    /// 每层最大问题数
    pub questions_per_level: usize,
    /// 待探索的问题
    pub pending_questions: Vec<(String, QuestionType, usize)>,
}

impl SocraticSession {
    pub fn new(task: String) -> Self {
        Self {
            task,
            qa_chain: Vec::new(),
            current_depth: 0,
            max_depth: 3,
            questions_per_level: 3,
            pending_questions: Vec::new(),
        }
    }

    pub fn with_depth(mut self, depth: usize) -> Self {
        self.max_depth = depth;
        self
    }

    pub fn with_questions_per_level(mut self, count: usize) -> Self {
        self.questions_per_level = count;
        self
    }

    /// 自动思考模式（active-triggering）
    ///
    /// 根据任务特征自动判断是否启动 Socratic 分析：
    /// - 如果满足触发条件，生成初始问题并返回 true
    /// - 否则返回 false，跳过分析
    ///
    /// 触发条件可通过环境变量覆盖：
    /// - `PRIORITY_AGENT_AUTO_THINK=0` — 禁用自动触发
    pub fn auto_think(&mut self) -> bool {
        if Self::should_auto_trigger(&self.task) {
            self.generate_initial_questions();
            true
        } else {
            false
        }
    }

    /// 判断任务是否应该自动触发 Socratic 分析
    ///
    /// 触发条件（满足任一）：
    /// 1. 任务描述长度 > 50 字符
    /// 2. 包含复杂关键词（设计/重构/架构/分析/评估/优化）
    /// 3. 涉及多个子领域（数据库+前端+后端等）
    /// 4. 包含不确定/模糊词汇（"看看"/"研究"/"调研"）
    pub fn should_auto_trigger(task: &str) -> bool {
        // 环境变量禁用
        if std::env::var("PRIORITY_AGENT_AUTO_THINK")
            .ok()
            .map(|v| v == "0")
            .unwrap_or(false)
        {
            return false;
        }

        let t = task.to_lowercase();

        // 条件 1：长度
        if t.chars().count() > 50 {
            return true;
        }

        // 条件 2：复杂关键词
        let complex_keywords = [
            "设计",
            "重构",
            "架构",
            "分析",
            "评估",
            "优化",
            "design",
            "refactor",
            "architecture",
            "analyze",
            "evaluate",
            "optimize",
            "系统",
            "模块",
            "集成",
            "方案",
            "策略",
            "system",
            "module",
            "integration",
            "strategy",
        ];
        if complex_keywords.iter().any(|kw| t.contains(kw)) {
            return true;
        }

        // 条件 3：多领域暗示
        let domains = ["数据库", "前端", "后端", "api", "ui", "测试", "deploy"];
        let domain_hits = domains.iter().filter(|d| t.contains(*d)).count();
        if domain_hits >= 2 {
            return true;
        }

        // 条件 4：模糊/探索性词汇
        let fuzzy_keywords = [
            "看看",
            "研究",
            "调研",
            "调查",
            "review",
            "research",
            "investigate",
        ];
        if fuzzy_keywords.iter().any(|kw| t.contains(kw)) {
            return true;
        }

        false
    }

    /// 生成初始问题
    pub fn generate_initial_questions(&mut self) {
        let seq = QuestionType::exploration_sequence();

        for qtype in seq.iter().take(self.questions_per_level) {
            let question = self.generate_question_for_type(&self.task, qtype, 0);
            self.pending_questions.push((question, qtype.clone(), 0));
        }
    }

    /// 根据类型生成问题
    fn generate_question_for_type(
        &self,
        context: &str,
        qtype: &QuestionType,
        depth: usize,
    ) -> String {
        let prefix = if depth > 0 {
            format!("[深度 {}] ", depth)
        } else {
            String::new()
        };

        match qtype {
            QuestionType::GoalClarification => {
                format!(
                    "{}这个任务「{}」的最核心目标是什么？去掉所有表面需求后，本质上要解决什么问题？",
                    prefix,
                    truncate(context, 100)
                )
            }
            QuestionType::PrerequisiteCheck => {
                format!(
                    "{}要完成这个目标，需要哪些前提条件？哪些是必须先搞定的？",
                    prefix
                )
            }
            QuestionType::RiskAssessment => {
                format!("{}做这件事最大的风险是什么？最可能在哪里出错？", prefix)
            }
            QuestionType::SolutionOptimization => {
                format!("{}有没有更简单/更高效的方法来达到同样的目标？", prefix)
            }
            QuestionType::CounterExample => {
                format!("{}什么情况下这个方案会失败？有没有反例？", prefix)
            }
            QuestionType::Reflection => {
                format!(
                    "{}基于以上分析，最终的执行方案是什么？有什么需要调整的？",
                    prefix
                )
            }
        }
    }

    /// 从答案中提取 follow-up 问题
    fn extract_followups(&self, answer: &str, qtype: &QuestionType) -> Vec<String> {
        let mut followups = Vec::new();

        // 简单启发式：从答案中提取需要进一步探索的点
        let lines: Vec<&str> = answer.lines().collect();
        for line in &lines {
            let trimmed = line.trim();
            // 寻找包含"需要"/"可能"/"风险"/"问题"等关键词的行
            if (trimmed.contains("需要") || trimmed.contains("风险") || trimmed.contains("问题"))
                && trimmed.len() > 20
                && followups.len() < 2
            {
                followups.push(format!(
                    "[深度 {}] 能否更详细地说明：{}",
                    self.current_depth + 1,
                    truncate(trimmed, 80)
                ));
            }
        }

        // 如果是风险评估，从答案中提取具体风险点作为 follow-up
        if *qtype == QuestionType::RiskAssessment {
            for line in &lines {
                if line.contains('-') || line.contains('*') || line.contains('•') {
                    let cleaned = line.trim_start_matches(|c: char| {
                        c == '-' || c == '*' || c == '•' || c == ' ' || c == '\t'
                    });
                    if cleaned.len() > 10 && followups.len() < 3 {
                        followups.push(format!(
                            "[深度 {}] 对于这个风险「{}」，有什么具体的缓解措施？",
                            self.current_depth + 1,
                            truncate(cleaned, 60)
                        ));
                    }
                }
            }
        }

        followups
    }

    /// 记录一个 Q&A
    pub fn add_qa(
        &mut self,
        question: String,
        answer: String,
        qtype: QuestionType,
        new_questions: Vec<String>,
    ) {
        // 先推断子问题类型（需要 qtype 的引用）
        let child_questions: Vec<(String, QuestionType)> = if self.current_depth < self.max_depth {
            new_questions
                .into_iter()
                .map(|q| {
                    let child_type = Self::infer_question_type(&q, &qtype);
                    (q, child_type)
                })
                .collect()
        } else {
            Vec::new()
        };

        let qa = QaPair {
            question,
            answer: answer.clone(),
            question_type: qtype,
            depth: self.current_depth,
            leads_to: child_questions.iter().map(|(q, _)| q.clone()).collect(),
        };
        self.qa_chain.push(qa);

        // 新问题加入待探索队列（深度 +1）
        for (q, child_type) in child_questions {
            self.pending_questions
                .push((q, child_type, self.current_depth + 1));
        }
    }

    /// 从问题文本推断类型（启发式）
    fn infer_question_type(question: &str, parent_type: &QuestionType) -> QuestionType {
        let q_lower = question.to_lowercase();
        if q_lower.contains("风险") || q_lower.contains("出错") || q_lower.contains("失败") {
            QuestionType::RiskAssessment
        } else if q_lower.contains("前提") || q_lower.contains("依赖") || q_lower.contains("先决")
        {
            QuestionType::PrerequisiteCheck
        } else if q_lower.contains("优化") || q_lower.contains("更好") || q_lower.contains("更高效")
        {
            QuestionType::SolutionOptimization
        } else if q_lower.contains("反例") || q_lower.contains("不会") {
            QuestionType::CounterExample
        } else if q_lower.contains("总结") || q_lower.contains("最终") || q_lower.contains("方案")
        {
            QuestionType::Reflection
        } else {
            // 继承父问题类型
            parent_type.clone()
        }
    }

    /// 获取下一个待探索的问题
    pub fn next_question(&mut self) -> Option<(String, QuestionType, usize)> {
        self.pending_questions.pop()
    }

    /// 是否还有待探索的问题
    #[cfg(test)]
    pub fn has_pending(&self) -> bool {
        !self.pending_questions.is_empty()
    }

    /// 获取所有答案的摘要
    pub fn synthesis(&self) -> String {
        if self.qa_chain.is_empty() {
            return String::new();
        }

        let mut output = String::new();
        output.push_str(&format!("## Socratic Analysis: {}\n\n", self.task));

        let mut by_type: std::collections::HashMap<&str, Vec<&QaPair>> =
            std::collections::HashMap::new();
        for qa in &self.qa_chain {
            by_type
                .entry(qa.question_type.label())
                .or_default()
                .push(qa);
        }

        for qtype in QuestionType::exploration_sequence() {
            let label = qtype.label();
            if let Some(qas) = by_type.get(label) {
                output.push_str(&format!("### {}\n", label));
                for qa in qas {
                    output.push_str(&format!("Q: {}\n", qa.question));
                    output.push_str(&format!("A: {}\n\n", truncate(&qa.answer, 300)));
                }
            }
        }

        output.push_str("---\n");
        output.push_str(&format!("Total questions: {}\n", self.qa_chain.len()));
        output.push_str(&format!(
            "Max depth reached: {}\n",
            self.qa_chain.iter().map(|q| q.depth).max().unwrap_or(0)
        ));

        output
    }

    /// 统计信息
    pub fn stats(&self) -> SocraticStats {
        let by_type: std::collections::HashMap<String, usize> =
            self.qa_chain
                .iter()
                .fold(std::collections::HashMap::new(), |mut acc, qa| {
                    *acc.entry(qa.question_type.label().to_string()).or_insert(0) += 1;
                    acc
                });

        SocraticStats {
            total_questions: self.qa_chain.len(),
            max_depth: self.qa_chain.iter().map(|q| q.depth).max().unwrap_or(0),
            by_type,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SocraticStats {
    pub total_questions: usize,
    pub max_depth: usize,
    pub by_type: std::collections::HashMap<String, usize>,
}

// ── 工具接口 ──────────────────────────────────────────

/// Socratic 工具 - 让 agent 使用 Socratic 方法分析任务
/// 真正接入 LLM 回答每个问题
pub struct SocraticTool;

impl SocraticTool {
    /// 用 LLM 回答单个问题
    async fn answer_question(
        provider: &dyn crate::services::api::LlmProvider,
        model: &str,
        task: &str,
        question: &str,
        previous_qa: &[QaPair],
    ) -> String {
        use crate::services::api::{ChatRequest, Message};

        // 构建上下文：之前的 Q&A 链
        let mut context = String::new();
        if !previous_qa.is_empty() {
            context.push_str("之前的分析：\n");
            for qa in previous_qa.iter().rev().take(3).rev() {
                context.push_str(&format!(
                    "Q [{}]: {}\nA: {}\n\n",
                    qa.question_type.label(),
                    qa.question,
                    truncate(&qa.answer, 200)
                ));
            }
        }

        let system_prompt = "你是一个深度思考助手。用户会给你一个任务和一个探索性问题，请用简洁但深刻的中文回答。直接回答问题，不要废话。";
        let user_message = format!(
            "任务：{}\n\n{}\n问题：{}\n\n请直接回答：",
            task, context, question
        );

        let request = ChatRequest::new(model)
            .with_messages(vec![
                Message::system(system_prompt),
                Message::user(&user_message),
            ])
            .with_temperature(0.7);

        match provider.chat(request).await {
            Ok(response) => response.content,
            Err(e) => format!("(LLM 调用失败: {})", e),
        }
    }
}

#[async_trait::async_trait]
impl crate::tools::Tool for SocraticTool {
    fn name(&self) -> &str {
        "socratic_analyze"
    }

    fn operation_kind(&self, _params: &serde_json::Value) -> crate::tools::ToolOperationKind {
        crate::tools::ToolOperationKind::Task
    }

    fn description(&self) -> &str {
        "Analyze a task using the Socratic method - generate deep questions and answer them \
         with the LLM to create a thorough reasoning chain. Use this for complex tasks that \
         need deep analysis before execution. Requires LLM provider to be available."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "task": {
                    "type": "string",
                    "description": "The task or question to analyze"
                },
                "depth": {
                    "type": "integer",
                    "description": "How deep to explore (1-3, default: 2)",
                    "default": 2
                },
                "questions_per_level": {
                    "type": "integer",
                    "description": "Questions to ask per depth level (1-5, default: 3)",
                    "default": 3
                }
            },
            "required": ["task"]
        })
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        context: crate::tools::ToolContext,
    ) -> crate::tools::ToolResult {
        let task = params["task"].as_str().unwrap_or("");
        if task.is_empty() {
            return crate::tools::ToolResult::error("Task cannot be empty");
        }

        let depth = params["depth"].as_u64().unwrap_or(2) as usize;
        let qpl = params["questions_per_level"].as_u64().unwrap_or(3) as usize;

        // 检查 LLM Provider 是否可用
        let provider = match &context.llm_provider {
            Some(p) => p.as_ref(),
            None => {
                return crate::tools::ToolResult::error(format!(
                    "SocraticTool requires LLM provider. Set one provider key: {}.",
                    crate::services::api::provider::provider_key_env_hint()
                ));
            }
        };
        let model = if context.model.is_empty() {
            "kimi-k2.5"
        } else {
            &context.model
        };

        let mut session = SocraticSession::new(task.to_string())
            .with_depth(depth.min(3))
            .with_questions_per_level(qpl.min(5));

        session.generate_initial_questions();

        let mut output = String::new();
        output.push_str(&format!(
            "## Socratic Analysis (depth={}, questions/level={})\n\n",
            depth, qpl
        ));

        // 逐个回答问题
        while let Some((question, qtype, _d)) = session.next_question() {
            output.push_str(&format!("### [{}] {}\n", qtype.label(), question));

            // 调用 LLM 回答
            let answer =
                Self::answer_question(provider, model, task, &question, &session.qa_chain).await;

            output.push_str(&format!("{}\n\n", answer));

            // 从答案中提取 follow-up 问题
            let followups = session.extract_followups(&answer, &qtype);

            // 记录 Q&A
            session.add_qa(question, answer, qtype, followups);
        }

        // 添加综合分析
        let synthesis = session.synthesis();
        let stats = session.stats();

        output.push_str(&synthesis);
        output.push_str("\n---\n");
        output.push_str(&format!(
            "分析完成：{} 个问题，最大深度 {}\n",
            stats.total_questions, stats.max_depth
        ));

        crate::tools::ToolResult::success_with_data(
            output,
            serde_json::to_value(&stats).unwrap_or(serde_json::Value::Null),
        )
    }

    fn is_available(&self, context: &crate::tools::ToolContext) -> bool {
        context.llm_provider.is_some()
    }

    fn unavailable_reason(&self, _context: &crate::tools::ToolContext) -> Option<String> {
        Some("LLM provider not configured".to_string())
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", s.chars().take(max).collect::<String>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_socratic_session() {
        let mut session = SocraticSession::new("Implement user auth".to_string())
            .with_depth(2)
            .with_questions_per_level(2);

        session.generate_initial_questions();
        assert!(session.has_pending());

        let mut count = 0;
        while session.next_question().is_some() {
            count += 1;
        }
        assert_eq!(count, 2);
    }

    #[test]
    fn test_question_types() {
        let seq = QuestionType::exploration_sequence();
        assert_eq!(seq.len(), 6);
        assert_eq!(seq[0], QuestionType::GoalClarification);
        assert_eq!(seq[5], QuestionType::Reflection);
    }

    #[test]
    fn test_synthesis() {
        let mut session = SocraticSession::new("Test task".to_string());
        session.add_qa(
            "What is the goal?".to_string(),
            "The goal is X".to_string(),
            QuestionType::GoalClarification,
            vec![],
        );

        let synthesis = session.synthesis();
        assert!(synthesis.contains("目标"));
        assert!(synthesis.contains("The goal is X"));
    }

    #[test]
    fn test_stats() {
        let mut session = SocraticSession::new("Test".to_string());
        session.add_qa(
            "Q1".into(),
            "A1".into(),
            QuestionType::GoalClarification,
            vec![],
        );
        session.add_qa(
            "Q2".into(),
            "A2".into(),
            QuestionType::RiskAssessment,
            vec![],
        );

        let stats = session.stats();
        assert_eq!(stats.total_questions, 2);
        assert_eq!(stats.by_type.get("目标"), Some(&1));
        assert_eq!(stats.by_type.get("风险"), Some(&1));
    }

    #[test]
    fn test_extract_followups() {
        let session = SocraticSession::new("Test".to_string());
        let answer = "这个任务的风险是数据丢失。需要做好备份。主要问题是并发处理。";
        let followups = session.extract_followups(answer, &QuestionType::RiskAssessment);
        assert!(!followups.is_empty());
    }

    // ============================================================================
    // Auto-think 测试
    // ============================================================================

    #[test]
    fn test_should_auto_trigger_long_task() {
        let task = "这是一个非常长的任务描述，涉及多个模块的设计和实现，需要仔细分析";
        assert!(SocraticSession::should_auto_trigger(task));
    }

    #[test]
    fn test_should_auto_trigger_complex_keyword() {
        assert!(SocraticSession::should_auto_trigger("重构用户认证模块"));
        assert!(SocraticSession::should_auto_trigger("优化数据库查询性能"));
        assert!(SocraticSession::should_auto_trigger("design new api"));
    }

    #[test]
    fn test_should_auto_trigger_multi_domain() {
        assert!(SocraticSession::should_auto_trigger(
            "设计数据库表并编写前端界面"
        ));
    }

    #[test]
    fn test_should_not_auto_trigger_simple_task() {
        assert!(!SocraticSession::should_auto_trigger("修 bug"));
        assert!(!SocraticSession::should_auto_trigger("hello"));
    }

    #[test]
    fn test_auto_think_triggers_when_appropriate() {
        let mut session = SocraticSession::new("重构整个认证系统并优化数据库查询".to_string());
        let triggered = session.auto_think();
        assert!(triggered, "Complex task should trigger auto-think");
        assert!(!session.pending_questions.is_empty());
    }

    #[test]
    fn test_auto_think_skips_simple_task() {
        let mut session = SocraticSession::new("hi".to_string());
        let triggered = session.auto_think();
        assert!(!triggered, "Simple greeting should not trigger auto-think");
        assert!(session.pending_questions.is_empty());
    }
}
