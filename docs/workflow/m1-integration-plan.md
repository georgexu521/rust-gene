# M1 MVP 联调计划（W8 交付物）

> 目标：明确 M1（最小可运行版本）的链路、接口和阻断清单，为编码阶段提供清晰的执行地图。
> 状态：V1 Frozen

---

## 1. M1 定义

M1 是 WorkflowEngine 的最小可运行闭环，必须能：
1. 接收用户请求，通过 Gate 判定进入 Workflow
2. 执行主动提问式深思（Socratic），产出 ThinkingResult
3. 生成 Plan，计算权重，排序执行
4. 输出执行报告

**M1 不包含**：递归拆分、失败重构、人工确认节点、TUI 可视化、指标持久化。

---

## 2. M1 模块边界

```
src/engine/workflow/
├── mod.rs           ✅ M1: WorkflowEngine 主入口 + 状态机
├── gate.rs          ✅ M1: Fast Lane + Heuristic（LLM Classifier 可选）
├── questioning.rs   ✅ M1: 固定模板 fallback + LLM 动态生成（简化版）
├── weights.rs       ✅ M1: 六维权重 + Sigmoid（已完成）
├── planner.rs       ✅ M1: 简单步骤提取 + 依赖识别（简化版）
├── executor.rs      ✅ M1: 按权重顺序串行执行
└── metrics.rs       ❌ M1: 暂不实现（用 tracing 日志替代）
```

---

## 3. 接口契约

### 3.1 WorkflowEngine 入口

```rust
pub struct WorkflowEngine {
    llm_provider: Arc<dyn LlmProvider>,
    weight_engine: WeightEngine,
    config: WorkflowConfig,
}

impl WorkflowEngine {
    pub fn new(
        llm_provider: Arc<dyn LlmProvider>,
        config: WorkflowConfig,
    ) -> Self;

    /// M1 核心方法：运行完整 workflow
    pub async fn run(
        &self,
        task: &str,
        mainline_goal: &str,
    ) -> Result<WorkflowResult, WorkflowError>;
}

pub struct WorkflowResult {
    pub thinking_result: ThinkingResult,
    pub plan: Plan,
    pub execution_log: Vec<ExecutionRecord>,
    pub final_report: String,
}
```

### 3.2 Gate 输出

```rust
pub enum GateDecision {
    Direct { reason: String },
    Workflow { reason: String, confidence: f64 },
}
```

### 3.3 ThinkingResult 输出

```rust
pub struct ThinkingResult {
    pub problem_statement: String,
    pub key_uncertainties: Vec<String>,
    pub decision_basis: String,
    pub question_chain: Vec<QuestionNode>,
}
```

### 3.4 与 ConversationLoop 的集成点

```rust
// engine/conversation_loop.rs
async fn run_inner(&mut self, input: &str, ...) -> Result<...> {
    // === M1 插入点 ===
    if self.should_use_workflow(input) {
        let engine = WorkflowEngine::new(self.llm_provider.clone(), WorkflowConfig::default());
        let result = engine.run(input, &extract_mainline(input)).await?;
        return Ok(self.format_workflow_result(result));
    }
    // === 原有逻辑 ===
    self.run_direct(input).await
}
```

---

## 4. 依赖关系

```
workflow/mod.rs
    ├── depends on: workflow/gate.rs
    ├── depends on: workflow/questioning.rs
    ├── depends on: workflow/weights.rs      ✅ 已完成
    ├── depends on: workflow/planner.rs
    ├── depends on: workflow/executor.rs
    ├── depends on: engine/socratic.rs        ✅ 已有，需改造
    ├── depends on: engine/plan_mode.rs       ✅ 已有，需扩展
    └── depends on: services/api/mod.rs       ✅ 已有（LlmProvider）

workflow/gate.rs
    └── depends on: services/api/mod.rs       ✅ 已有

workflow/questioning.rs
    ├── depends on: engine/socratic.rs        ✅ 已有
    └── depends on: services/api/mod.rs       ✅ 已有

workflow/planner.rs
    ├── depends on: engine/plan_mode.rs       ✅ 已有
    └── depends on: workflow/weights.rs       ✅ 已完成

workflow/executor.rs
    ├── depends on: tools/mod.rs              ✅ 已有（ToolRegistry）
    ├── depends on: engine/plan_mode.rs       ✅ 已有
    └── depends on: workflow/weights.rs       ✅ 已完成
```

**无循环依赖** ✅

---

## 5. 编码顺序

```
Phase 1（第 1-2 天）
├── gate.rs          # Fast Lane + Heuristic
├── questioning.rs   # 改造 SocraticSession，新增 auto_think()
└── planner.rs       # 简单步骤提取 + 权重计算

Phase 2（第 3-4 天）
├── executor.rs      # 串行执行 + 验证
├── mod.rs           # WorkflowEngine 状态机
└── integration      # ConversationLoop 插入点

Phase 3（第 5 天）
├── 单元测试         # 每个模块 >= 5 个测试
├── 集成测试         # 5 个样本任务端到端
└── 修复阻断项
```

---

## 6. 阻断清单

| 编号 | 阻断项 | 严重程度 | 解决方案 | 负责人 |
|------|--------|---------|---------|--------|
| B1 | `socratic.rs` 无 `auto_think()` 方法 | 🔴 阻断 | 新增方法，复用现有 Q&A 结构 | - |
| B2 | `plan_mode.rs` PlanStep 无 weight 字段 | 🟡 高 | 扩展 PlanStep 结构体 | - |
| B3 | Gate 的 LLM Classifier 延迟问题 | 🟡 高 | M1 先用 Heuristic，LLM 可选 | - |
| B4 | 无 `ThinkingResult` 数据结构 | 🔴 阻断 | 新增结构体 | - |
| B5 | ConversationLoop 无法暂停让 Workflow 接管 | 🟡 高 | 新增状态分支 | - |
| B6 | 环境变量读取在测试中冲突 | 🟢 中 | 使用 test_env_guard 模式 | - |

---

## 7. 风险与缓解

| 风险 | 影响 | 缓解 |
|------|------|------|
| M1 范围过大 | 延期 | 严格按"不包含"列表裁剪 |
| LLM 调用不稳定 | 测试失败 | 使用 mock provider 做单元测试 |
| 与现有 Plan Mode 冲突 | 功能异常 | 保持两套系统独立，不互相干扰 |
| 性能问题 | Gate 延迟 | Fast Lane 覆盖 80% 请求 |

---

## 8. 验收标准（M1）

- [ ] Gate 能正确区分 Direct 和 Workflow（Fast Lane + Heuristic）
- [ ] 进入 Workflow 后能产出 ThinkingResult
- [ ] ThinkingResult 包含 Problem Statement 和 Decision Basis
- [ ] 能生成 Plan 并按权重排序
- [ ] 能按权重顺序执行步骤（至少 1 步）
- [ ] 输出包含执行报告
- [ ] 单元测试 >= 20 个，全部通过
- [ ] 集成测试：5 个样本任务中有 3 个完成闭环
- [ ] 不破坏现有 Direct Mode 功能

---

*本文档冻结后，M1 范围不再扩展。如需新增功能，归入 M2。*
