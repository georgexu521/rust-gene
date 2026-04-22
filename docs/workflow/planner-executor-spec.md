# Planner/Executor 与递归拆分策略规格（W6 交付物）

> 目标：定义"思考成果 → 可执行计划 → 按权重执行 → 验证 → 重算"的完整闭环。
> 状态：V1 Frozen
> 依赖：workflow-v1-design.md、weights.rs、questioning-spec.md

---

## 1. 架构总览

```
THINKING 产出 ThinkingResult
        │
        ▼
┌─────────────────┐
│    PLANNER      │ ← 将思考成果转化为 Plan
│                 │
│  - 提取执行步骤  │
│  - 识别依赖关系  │
│  - 计算权重     │
│  - 递归拆分判断  │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│   WEIGHTING     │ ← 六维权重引擎排序
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│    EXECUTOR     │ ← 按权重顺序执行
│                 │
│  - 微思考       │
│  - 执行操作     │
│  - 验证结果     │
│  - 更新状态     │
│  - 重算权重     │
└────────┬────────┘
         │
    ┌────┴────┐
    ▼         ▼
未完成    全部完成
    │         │
    ▼         ▼
WEIGHTING   DONE
```

---

## 2. Planner（计划生成）

### 2.1 输入

```rust
pub struct PlannerInput {
    pub thinking_result: ThinkingResult,    // 思考成果
    pub mainline_goal: String,              // 主线目标
    pub available_tools: Vec<String>,       // 可用工具列表
    pub max_depth: usize,                   // 递归深度限制
}
```

### 2.2 输出

```rust
pub struct PlannedResult {
    pub plan: Plan,                         // 执行计划（复用 plan_mode::Plan）
    pub step_contexts: Vec<StepContext>,    // 每个步骤的权重计算上下文
    pub needs_recursion: Vec<usize>,        // 需要递归拆分的步骤索引
    pub explanation: String,                // 计划解释
}
```

### 2.3 计划生成流程

```
输入: ThinkingResult
    │
    ▼
Step 1: 提取候选步骤
    - 从 Problem Statement 提取关键动作
    - 从 Key Uncertainties 提取验证步骤
    - 从 Decision Basis 提取决策依赖
    │
    ▼
Step 2: 为每个候选步骤分配工具
    - file_read/write/edit → 文件操作
    - bash/powershell → 命令执行
    - agent → 委托子 agent
    - 无明确工具 → 标记为 "analyze"
    │
    ▼
Step 3: 识别依赖关系
    - 读取必须在写入之前
    - 设计必须在实现之前
    - 测试必须在实现之后
    - 使用 LLM 轻量判断依赖
    │
    ▼
Step 4: 递归拆分判断
    - 对每个步骤：LLM 判断是否太复杂
    - 是 → 标记为 needs_recursion
    - 否 → 保留为原子步骤
    │
    ▼
Step 5: 计算权重
    - 为每个原子步骤构建 StepContext
    - 调用 WeightEngine::compute()
    - 生成 WeightedStep
    │
    ▼
输出: PlannedResult
```

### 2.4 依赖识别规则

```rust
fn infer_dependencies(steps: &[PlanStep]) -> Vec<(usize, usize)> {
    let mut deps = Vec::new();
    
    for (i, step_i) in steps.iter().enumerate() {
        for (j, step_j) in steps.iter().enumerate() {
            if i == j { continue; }
            
            // 规则 1: 读取在写入之前
            if step_i.tool == Some("file_read") && step_j.tool == Some("file_edit") {
                if step_i.description.contains(&extract_filename(&step_j.description)) {
                    deps.push((j, i)); // j 依赖 i
                }
            }
            
            // 规则 2: 设计在实现之前
            if step_i.description.contains("设计") && step_j.description.contains("实现") {
                deps.push((j, i));
            }
            
            // 规则 3: 实现在测试之前
            if step_i.description.contains("实现") && step_j.description.contains("测试") {
                deps.push((j, i));
            }
        }
    }
    
    deps
}
```

### 2.5 与 Plan Mode 的复用

```
Plan Mode (/plan)          WorkflowEngine
     │                           │
     ▼                           ▼
┌─────────┐               ┌─────────┐
│ 用户输入 │               │ LLM 生成 │
│ 计划     │               │ 计划     │
└────┬────┘               └────┬────┘
     │                         │
     ▼                         ▼
┌─────────┐               ┌─────────┐
│人工审批  │               │自动执行  │
│(每步)   │               │(可中断) │
└────┬────┘               └────┬────┘
     │                         │
     └──→ 共用 Plan 数据结构 ←─┘
```

复用 `plan_mode.rs` 中的 `Plan` 和 `PlanStep`，新增字段：

```rust
// plan_mode.rs 扩展
impl PlanStep {
    pub weight: Option<u32>,           // 权重分数
    pub weight_explanation: Option<String>, // 权重解释
    pub dependent_steps: Vec<usize>,   // 依赖的步骤索引
    pub unlocks_steps: Vec<usize>,     // 解锁的步骤索引
}
```

---

## 3. 递归拆分（Recursion）

### 3.1 触发条件

```rust
fn should_split(step: &PlanStep, depth: usize, max_depth: usize) -> bool {
    // 已到达最大深度，强制不拆分
    if depth >= max_depth {
        return false;
    }
    
    // LLM 判断条件（满足任一）
    let indicators = [
        step.description.contains("多个") || step.description.contains("5 个文件"),
        step.description.contains("架构") || step.description.contains("重构"),
        step.tool == Some("agent"),
        step.description.contains("递归") || step.description.contains("拆分"),
    ];
    
    indicators.iter().filter(|&&x| x).count() >= 2
}
```

### 3.2 拆分策略

```
原步骤: "实现完整的用户认证系统"
    │
    ▼
┌──────────────────────────────┐
│ LLM 拆分判断                  │
│ "涉及登录/注册/权限/会话管理" │
└──────────────┬───────────────┘
               │
    ┌──────────┼──────────┐
    ▼          ▼          ▼
"设计数据库    "实现登录    "实现注册
 schema"      API"        API"
    │          │          │
    └──────────┼──────────┘
               │
               ▼
        "实现权限检查"
               │
               ▼
        "实现会话管理"
```

### 3.3 递归流程

```rust
async fn plan_with_recursion(
    input: PlannerInput,
    depth: usize,
) -> Result<Vec<PlanStep>, String> {
    let mut all_steps = Vec::new();
    
    for step in input.thinking_result.extract_steps() {
        if should_split(&step, depth, input.max_depth) {
            // 递归拆分
            let sub_input = PlannerInput {
                thinking_result: ThinkingResult::from_step(&step),
                max_depth: input.max_depth,
                ..input.clone()
            };
            let sub_steps = Box::pin(plan_with_recursion(sub_input, depth + 1)).await?;
            all_steps.extend(sub_steps);
        } else {
            // 原子步骤
            all_steps.push(step);
        }
    }
    
    Ok(all_steps)
}
```

### 3.4 触底平铺

到达 `WORKFLOW_MAX_DEPTH` 后：

```rust
fn flatten_to_atomic(step: &PlanStep) -> Vec<PlanStep> {
    let desc = &step.description;
    let mut atoms = Vec::new();
    
    // 按工具类型拆分
    if desc.contains("file_write") || desc.contains("file_edit") {
        atoms.push(PlanStep {
            description: format!("[原子] {}", desc),
            tool: step.tool.clone(),
            ..Default::default()
        });
    } else {
        // 读操作合并为一个原子步骤
        atoms.push(PlanStep {
            description: format!("[原子] 读取相关文件: {}", desc),
            tool: Some("file_read".into()),
            ..Default::default()
        });
    }
    
    atoms
}
```

---

## 4. Executor（执行器）

### 4.1 执行循环

```
while 还有未完成的步骤:
    1. 获取最高权重的待执行步骤
    2. 检查依赖是否满足
    3. 执行前微思考（10-20 秒级）
    4. 执行操作（工具调用）
    5. 验证结果
    6. 更新步骤状态
    7. 重算剩余步骤权重
```

### 4.2 微思考（Pre-execution Micro-think）

每个步骤执行前，快速自问：

```
问题："我现在要执行 [步骤描述]，是否与主线 [主线目标] 对齐？"
回答：LLM 轻量判断（1 轮，~100 tokens）

如果判断为 "不对齐"：
    - 发出警告
    - 降低该步骤权重
    - 记录 drift 事件
```

### 4.3 验证策略

| 验证类型 | 触发条件 | 方法 |
|---------|---------|------|
| 编译检查 | 涉及代码修改 | `cargo check` |
| 测试验证 | 涉及功能变更 | `cargo test`（相关模块） |
| 静态检查 | 新增/修改文件 | `cargo clippy` |
| 行为检查 | 工具执行 | 检查结果是否包含预期内容 |

### 4.4 失败处理

```rust
enum StepOutcome {
    Success,
    Failure { reason: String, retryable: bool },
    Blocked { dependency: usize },
}

fn handle_failure(step: &mut PlanStep, failure_count: usize) {
    match failure_count {
        1 => {
            // 第一次失败：重试
            step.status = StepStatus::Pending;
        }
        2 => {
            // 第二次失败：触发问题重构
            step.status = StepStatus::Pending;
            step.description = format!("[重构] {}", step.description);
            // 标记需要重新 THINKING
        }
        3 => {
            // 第三次失败：升级为人工确认节点
            step.status = StepStatus::Failed("需要人工确认".into());
            // 暂停执行，等待用户输入
        }
        _ => {
            // 更多失败：跳过，记录
            step.status = StepStatus::Skipped;
        }
    }
}
```

---

## 5. 权重重算（Reweight）

### 5.1 触发时机

- 每完成一个步骤后
- 发现新依赖时
- 步骤失败后（重构时）

### 5.2 重算逻辑

```rust
fn reweight_steps(plan: &mut Plan, completed: &[usize]) {
    let engine = WeightEngine::default();
    
    for (i, step) in plan.steps.iter_mut().enumerate() {
        if step.status != StepStatus::Pending {
            continue;
        }
        
        let ctx = StepContext {
            step_index: i,
            completed_steps: completed.to_vec(),
            dependent_steps: step.dependent_steps.clone(),
            ..// 其他字段
        };
        
        let weighted = engine.compute(&ctx);
        step.weight = Some(weighted.normalized_score);
        step.weight_explanation = Some(weighted.explanation);
    }
}
```

---

## 6. 状态机映射

```
WorkflowState::PLANNING
    │
    ├── 调用 Planner::plan()
    │
    ├── 如果需要递归：
    │   └── 进入递归拆分
    │       ├── 每个子步骤生成 ThinkingResult
    │       ├── 递归调用 Planner
    │       └── 合并所有原子步骤
    │
    └── 输出 Plan + WeightedSteps
            │
            ▼
WorkflowState::WEIGHTING
    │
    ├── 调用 WeightEngine::compute_and_sort()
    │
    └── 输出按权重排序的步骤队列
            │
            ▼
WorkflowState::EXECUTING
    │
    ├── 取出队列中的最高权重步骤
    ├── 检查依赖满足
    ├── 微思考
    ├── 执行工具调用
    ├── 验证结果
    ├── 更新状态
    │
    └── 还有未完成步骤？
        ├── 是 → WorkflowState::REWEIGHT
        └── 否 → WorkflowState::VERIFYING
                │
                └── WorkflowState::DONE
```

---

## 7. 与现有代码的复用

### 7.1 复用组件

| 现有组件 | 复用方式 |
|---------|---------|
| `plan_mode::Plan` | 扩展字段（weight、dependent_steps） |
| `plan_mode::PlanStep` | 复用，新增 weight 相关字段 |
| `plan_mode::StepStatus` | 直接复用 |
| `socratic_executor::SocraticPlanExecutor` | 复用执行骨架，替换权重计算 |
| `conversation_loop::ConversationLoop` | 在 run_inner() 前插入闸门 |

### 7.2 新增组件

```
src/engine/workflow/
├── planner.rs      # Planner 实现
├── executor.rs     # Executor 实现
└── mod.rs          # WorkflowEngine 入口（整合 gate/planner/executor）
```

---

## 8. 异常路径

| 异常 | 处理 |
|------|------|
| Planner 无法提取步骤 | 降级：ThinkingResult.problem_statement 作为单一步骤 |
| 递归深度超限 | 强制平铺，记录 warning |
| 循环依赖 | 检测并打破（按索引顺序） |
| 执行工具不存在 | 标记失败，尝试替代工具 |
| 验证失败（编译/测试） | 按失败策略处理（重试/重构/人工确认） |
| 全部步骤被阻塞 | 暂停执行，提示用户解除阻塞 |

---

## 9. 验收标准

- [ ] Planner 能从 ThinkingResult 产出 Plan
- [ ] 依赖识别准确率 >= 70%（在 20 个样本上人工验证）
- [ ] 递归拆分触发合理（不过度拆分、不遗漏需拆分步骤）
- [ ] Executor 按权重顺序执行
- [ ] 失败 2 次触发重构、失败 3 次触发人工确认
- [ ] 权重重算后排序变化可解释
- [ ] 单元测试 >= 10 个，全部通过
- [ ] 异常路径全部覆盖

---

*本文档冻结后，Planner 的拆分策略和 Executor 的失败策略不再变更。工具映射规则可在 W7+ 迭代优化。*
