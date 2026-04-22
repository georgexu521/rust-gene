# 主动提问式深思策略规格（W5 交付物）

> 目标：将现有被动式 `SocraticTool` 改造为主动触发、动态生成、自问自答的提问式深思引擎。
> 状态：V1 Frozen
> 依赖：workflow-v1-design.md（术语、状态机、预算控制）、engine/socratic.rs（现有实现）

---

## 1. 改造概述

### 1.1 现有实现的问题

| 问题 | 说明 |
|------|------|
| 被动触发 | 用户必须手动调用 `socratic_analyze` 工具 |
| 固定模板 | 6 种问题类型是硬编码模板，与任务上下文无关 |
| 无自问自答 | 问题生成和答案是分离的，不是闭环 |
| 无预算控制 | 深度和问题数可配置但无 token 上限 |
| 无主线意识 | 问题链不与主线目标对齐 |

### 1.2 目标形态

```
用户提出复杂需求
        │
        ▼
┌─────────────────────────┐
│  WorkflowEngine::think() │ ← 自动触发，非手动
│                         │
│  1. 分析任务语义          │
│  2. 提取主线关键词        │
│  3. 生成问题链（动态）    │
│  4. LLM 自问自答         │
│  5. 检查收敛条件          │
│  6. 输出思考成果          │
└─────────────────────────┘
        │
        ▼
  Problem Statement
  Key Uncertainties
  Decision Basis
```

### 1.3 改造范围

- **复用**：`QaPair`、`QuestionType`、`SocraticStats` 数据结构
- **改造**：`SocraticSession` 新增 `auto_think()`、`dynamic_question()`、`evaluate_convergence()`
- **新增**：`ActiveQuestioningEngine` 结构体（管理问题生成、回答、收敛判断）
- **删除**：固定模板式问题生成（保留为 fallback）

---

## 2. 核心数据结构

### 2.1 问题链（Question Chain）

```rust
/// 问题节点（改造后的 QaPair）
pub struct QuestionNode {
    pub id: String,                    // 唯一标识（如 "Q-1-2"）
    pub question: String,              // 问题文本
    pub answer: String,                // LLM 自答内容
    pub question_type: QuestionType,   // 问题类型
    pub depth: usize,                  // 在问题树中的深度
    pub parent_id: Option<String>,     // 父问题 ID（递进关系）
    pub child_ids: Vec<String>,        // 子问题 ID
    pub mainline_relevance: f64,       // 与主线目标的相关度 [0, 1]
    pub uncertainty_reduced: bool,     // 是否减少了不确定性
    pub token_cost: usize,             // 本问题消耗的 token 数
}

/// 思考成果
pub struct ThinkingResult {
    pub problem_statement: String,     // 问题本质
    pub key_uncertainties: Vec<String>, // 剩余关键不确定点
    pub decision_basis: String,        // 决策依据
    pub question_chain: Vec<QuestionNode>, // 完整问题链
    pub total_token_cost: usize,       // 总 token 消耗
    pub convergence_reason: String,    // 收敛原因（为什么停了）
}

/// 问题生成上下文
pub struct QuestionContext {
    pub task_description: String,      // 原始任务描述
    pub mainline_goal: String,         // 主线目标
    pub codebase_context: Vec<String>, // 相关文件/模块列表
    pub memory_hints: Vec<String>,     // 记忆中的相关提示
    pub previous_answers: Vec<QuestionNode>, // 已回答的问题
    pub remaining_budget: usize,       // 剩余 token 预算
}
```

### 2.2 问题类型扩展

保留现有 6 种类型，新增 2 种：

| 类型 | 用途 | 优先级 |
|------|------|--------|
| GoalClarification | 目标澄清 | 高 |
| PrerequisiteCheck | 前提检查 | 高 |
| RiskAssessment | 风险评估 | 高 |
| SolutionOptimization | 方案优化 | 中 |
| CounterExample | 反例检验 | 中 |
| Reflection | 反思总结 | 低 |
| **ScopeClarification** | **范围澄清**（新增） | 高 |
| **SequenceClarification** | **顺序澄清**（新增） | 高 |

---

## 3. 动态问题生成策略

### 3.1 生成流程

```
输入: QuestionContext
    │
    ▼
Step 1: 分析上下文
    - 提取任务关键词
    - 识别代码库相关文件
    - 读取记忆系统中的相关经验
    │
    ▼
Step 2: 确定问题类型优先级
    - 首次：GoalClarification → ScopeClarification → PrerequisiteCheck → RiskAssessment
    - 深度 1+：基于前序答案推断最需要的类型
    │
    ▼
Step 3: 生成问题（LLM）
    - 使用 prompt 模板注入上下文
    - 要求 LLM 生成 1 个针对性问题
    - 问题必须与主线目标相关
    │
    ▼
Step 4: LLM 自答
    - 同一 LLM 调用回答刚生成的问题
    - 答案简洁，不超过预算
    │
    ▼
Step 5: 评估收敛
    - 不确定性是否减少？
    - 是否形成可执行计划？
    - 预算是否耗尽？
    │
    ▼
  未收敛 ──→ 回到 Step 2（下一轮）
  已收敛 ──→ 输出 ThinkingResult
```

### 3.2 问题生成 Prompt 模板

```
你是一个主动提问式深思引擎。你的任务是帮助想透一个编程任务。

【主线目标】
{mainline_goal}

【当前任务】
{task_description}

【已探索的问题】
{previous_qa_summary}

【相关代码文件】
{codebase_files}

【记忆中的相关经验】
{memory_hints}

---

要求：
1. 基于以上上下文，提出 1 个最关键的问题。
2. 问题必须有助于推进主线目标的实现。
3. 问题应该具体，不能是"你有什么想法？"这种泛泛的问题。
4. 优先关注：目标是否清晰、前提是否满足、风险在哪里、执行顺序是什么。

请只输出问题本身，不要解释为什么问这个问题。
```

### 3.3 答案生成 Prompt 模板

```
请回答以下问题。回答要简洁、有洞察力，不超过 {answer_budget} tokens。

任务：{task_description}
问题：{question}

请直接给出你的分析和结论：
```

---

## 4. 停止条件

### 4.1 硬停止（不可逾越）

| 条件 | 触发值 | 行为 |
|------|--------|------|
| 最大轮数 | `SOCRATIC_MAX_ROUNDS` (默认 5) | 停止，标记为 "budget_exhausted" |
| 单个答案 token 上限 | `SOCRATIC_ANSWER_BUDGET` (默认 500) | 截断答案，发出警告 |
| 单次思考总 token 上限 | `SOCRATIC_TOTAL_BUDGET` (默认 3750) | 停止，标记为 "budget_exhausted" |
| 递归深度 | `WORKFLOW_MAX_DEPTH` (默认 3) | 停止，平铺为原子步骤 |

### 4.2 软停止（LLM 判断）

```rust
fn should_stop(question_chain: &[QuestionNode]) -> Option<String> {
    // 条件 1：关键不确定点 <= 1
    let uncertainties = extract_uncertainties(question_chain);
    if uncertainties.len() <= 1 {
        return Some("uncertainties_low".into());
    }
    
    // 条件 2：已形成可执行计划
    if has_executable_plan(question_chain) {
        return Some("executable_plan_formed".into());
    }
    
    // 条件 3：连续 2 轮没有减少不确定性
    let last_two = question_chain.iter().rev().take(2);
    if last_two.all(|q| !q.uncertainty_reduced) {
        return Some("diminishing_returns".into());
    }
    
    // 条件 4：问题开始重复或变得太抽象
    if is_question_repeating(question_chain) {
        return Some("question_repetition".into());
    }
    
    None
}
```

### 4.3 优雅降级

当因 budget 耗尽而停止时：

```
1. 将未回答完的问题合并进执行计划
2. 在执行阶段 "边做边想"
3. 标记："思考因 budget 限制提前结束，关键问题将在执行中处理"
```

---

## 5. 预算控制

### 5.1 三层 Budget

```
┌─────────────────────────────────────────┐
│ Layer 1: 每轮对话最多触发 1 次思考流程      │  硬编码，不可配置
│   → 防止一轮对话无限思考                    │
├─────────────────────────────────────────┤
│ Layer 2: 单次思考最大 N 轮 Q&A             │  SOCRATIC_MAX_ROUNDS=5
│   → 控制问题链长度                         │
├─────────────────────────────────────────┤
│ Layer 3: 单个答案最多 M tokens              │  SOCRATIC_ANSWER_BUDGET=500
│   → 控制单次回答成本                        │
├─────────────────────────────────────────┤
│ Layer 4: 单次思考总 token 预算              │  SOCRATIC_TOTAL_BUDGET=3750
│   → 兜底总成本                             │
└─────────────────────────────────────────┘
```

### 5.2 Budget 跟踪

```rust
pub struct BudgetTracker {
    pub max_rounds: usize,
    pub max_answer_tokens: usize,
    pub max_total_tokens: usize,
    pub used_rounds: usize,
    pub used_tokens: usize,
}

impl BudgetTracker {
    pub fn can_proceed(&self) -> bool {
        self.used_rounds < self.max_rounds
            && self.used_tokens < self.max_total_tokens
    }
    
    pub fn remaining_answer_budget(&self) -> usize {
        let remaining = self.max_total_tokens.saturating_sub(self.used_tokens);
        remaining.min(self.max_answer_tokens)
    }
}
```

---

## 6. 与现有 SocraticSession 的复用

### 6.1 复用部分

| 组件 | 复用方式 |
|------|---------|
| `QaPair` | 升级为 `QuestionNode`（增加 id/parent/child/relevance） |
| `QuestionType` | 保留 6 种，新增 2 种 |
| `SocraticStats` | 直接复用，新增 token_cost 字段 |
| `truncate()` | 直接复用 |
| `synthesis()` | 改造为输出 `ThinkingResult` |

### 6.2 改造部分

```rust
impl SocraticSession {
    /// 新增：主动思考入口
    pub async fn auto_think(
        &mut self,
        llm_provider: &dyn LlmProvider,
        ctx: &QuestionContext,
    ) -> Result<ThinkingResult, String> {
        // 1. 初始化预算追踪器
        let mut budget = BudgetTracker::from_env();
        
        // 2. 循环生成问题-回答-评估
        while budget.can_proceed() {
            // 生成问题
            let question = self.dynamic_question(ctx, &budget).await?;
            
            // LLM 自答
            let answer = self.self_answer(llm_provider, &question, &budget).await?;
            
            // 记录并评估
            let node = self.record_qa(question, answer);
            budget.consume(node.token_cost);
            
            // 检查收敛
            if let Some(reason) = should_stop(&self.question_nodes) {
                return Ok(self.build_result(reason, budget));
            }
        }
        
        // Budget 耗尽
        Ok(self.build_result("budget_exhausted".into(), budget))
    }
    
    /// 新增：动态问题生成（替代固定模板）
    async fn dynamic_question(
        &self,
        ctx: &QuestionContext,
        budget: &BudgetTracker,
    ) -> Result<String, String> {
        let prompt = build_question_prompt(ctx, budget);
        // 调用 LLM 生成问题
        // ...
    }
    
    /// 新增：LLM 自答
    async fn self_answer(
        &self,
        llm_provider: &dyn LlmProvider,
        question: &str,
        budget: &BudgetTracker,
    ) -> Result<String, String> {
        let prompt = build_answer_prompt(question, budget.remaining_answer_budget());
        // 调用 LLM 回答
        // ...
    }
}
```

### 6.3 保留的被动接口

```rust
/// 保留：用户仍可手动调用 socratic_analyze
/// 但内部实现改为调用 auto_think()
impl SocraticTool {
    async fn execute(...) -> ToolResult {
        let mut session = SocraticSession::new(task);
        let ctx = QuestionContext::from_task(task);
        let result = session.auto_think(provider, &ctx).await?;
        ToolResult::success(result.format_output())
    }
}
```

---

## 7. 主线对齐检查

### 7.1 每个问题的主线相关度

```rust
fn compute_mainline_relevance(question: &str, mainline: &str) -> f64 {
    // 使用与 weights.rs 中 DriftPenaltyRule 相同的关键词匹配逻辑
    let matched_keywords = count_matching_keywords(question, mainline);
    let ratio = matched_keywords as f64 / total_mainline_keywords(mainline) as f64;
    ratio.clamp(0.0, 1.0)
}
```

### 7.2 低相关度处理

```
if relevance < 0.3:
    1. 发出警告："[Workflow] 当前问题可能偏离主线"
    2. 要求 LLM 重新生成与主线更相关的问题
    3. 连续 2 次低相关度 → 触发收敛，避免无限循环
```

---

## 8. 输出格式

### 8.1 ThinkingResult 文本输出

```markdown
## 主动思考成果

### 问题本质
{problem_statement}

### 剩余不确定点
{key_uncertainties}

### 决策依据
{decision_basis}

### 思考过程
{question_chain_formatted}

---
消耗: {total_token_cost} tokens, {question_count} 个问题
状态: {convergence_reason}
```

### 8.2 JSON 输出（供后续步骤消费）

```json
{
  "problem_statement": "...",
  "key_uncertainties": ["...", "..."],
  "decision_basis": "...",
  "question_chain": [
    {
      "id": "Q-1",
      "question": "...",
      "answer": "...",
      "type": "GoalClarification",
      "depth": 0,
      "mainline_relevance": 0.85
    }
  ],
  "total_token_cost": 1200,
  "convergence_reason": "executable_plan_formed"
}
```

---

## 9. 异常处理

| 异常 | 处理 |
|------|------|
| LLM 调用失败 | 重试 1 次，仍失败则降级到固定模板问题 |
| 问题生成超时 | 使用预设的 fallback 问题列表 |
| 答案为空/无效 | 标记该轮无效，不计入收敛判断 |
| 预算中途耗尽 | 基于已有问题链输出结果，标记 "budget_exhausted" |
| 全部问题低相关度 | 触发收敛，标记 "drift_detected" |

---

## 10. 测试策略

### 10.1 单元测试

| 测试 | 验证 |
|------|------|
| `test_dynamic_question_generation` | 动态生成的问题与上下文相关 |
| `test_budget_enforcement` | 超出 budget 后停止 |
| `test_convergence_uncertainties_low` | 不确定点 <=1 时正确收敛 |
| `test_convergence_executable_plan` | 形成可执行计划时收敛 |
| `test_mainline_relevance_filter` | 低相关度问题被过滤 |
| `test_fallback_on_llm_failure` | LLM 失败时使用固定模板 |

### 10.2 集成测试

使用 W2 的 20 个样本任务，验证：
- 每个任务的问题链长度 <= 5
- 问题链覆盖 4 种核心问题类型（目标/范围/前提/风险）
- 输出包含可执行的 Problem Statement
- Token 消耗 <= 3750

---

## 11. 与 Workflow 状态机的映射

```
WorkflowState::THINKING
    │
    ├── 调用 SocraticSession::auto_think()
    │
    ├── 内部循环：
    │   ├── generate_question()  ← LLM 动态生成
    │   ├── self_answer()        ← LLM 自问自答
    │   ├── evaluate_convergence()
    │   └── budget_tracker.consume()
    │
    └── 返回 ThinkingResult
            │
            ▼
    WorkflowState::PLANNING
```

---

## 12. 验收标准

- [ ] `auto_think()` 方法可用，能产出 `ThinkingResult`
- [ ] 问题由 LLM 动态生成，不是固定模板
- [ ] Budget 三层控制生效（轮数/答案/总预算）
- [ ] 收敛检测覆盖 4 种停止条件
- [ ] 主线相关度检查过滤偏离问题
- [ ] LLM 失败时有 fallback 模板
- [ ] 单元测试 >= 10 个，全部通过
- [ ] 集成测试：20 个样本任务中 80% 产出可执行结论

---

*本文档冻结后，问题类型、停止条件、预算层级不再变更。Prompt 模板可在 W7+ 迭代优化。*
