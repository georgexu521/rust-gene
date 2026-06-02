# Workflow Spec: 权重驱动 + 主动提问式深思

> 目标：把 `brainstorm.md` 的理念落成可执行工程规格。  
> 状态：V1（可进入实现）

---

## 1. 问题定义

当前 AI 编程助手常见失败模式：
- 抓不住主线，陷入细枝末节。
- 不主动识别关键问题，用户不问就不想。
- 遇到复杂任务时执行顺序混乱，修修补补。

本项目要解决的是：
- 先把“最重要问题”找出来。
- 再通过“主动多提问题”把问题想透。
- 最后按权重执行并持续纠偏。

---

## 2. 核心原则

### 2.1 权重规则必须是源码约束

- 权重计算规则写在 Rust 代码里，不能只靠提示词。
- LLM 可以参与打分，但最终排序由规则引擎裁决。
- 支持环境变量调参，不支持用户绕过核心规则。

### 2.2 主动提问式深思（你说的“多提问题引导思考”）

- 这就是“苏格拉底式思考”的工程化版本。
- 本质不是哲学术语，而是一个执行策略：
  - 先问关键问题；
  - 再回答问题；
  - 回答后继续追问；
  - 直到形成可执行结论。

建议产品术语：
- 对外：`主动提问式深思`
- 对内（代码名）：`Socratic`

### 2.3 先找主线，再做细节

- 每轮执行前必须明确当前“主线目标（Mainline Goal）”。
- 所有步骤必须映射到主线目标；无法映射的步骤降权或延后。

---

## 3. 系统架构

## 3.1 模块划分

新增目录：`src/engine/workflow/`
- `mod.rs`: WorkflowEngine 入口与状态机
- `gate.rs`: 触发闸门（是否进入 workflow）
- `questioning.rs`: 主动提问式深思引擎
- `weights.rs`: 权重计算规则引擎
- `planner.rs`: 计划拆分/递归拆分
- `executor.rs`: 按权重执行与回写
- `metrics.rs`: 评估指标与日志

复用现有模块：
- `engine/conversation_loop.rs`: 增加入口闸门
- `engine/socratic.rs`: 复用并改造成主动模式
- `engine/socratic_executor.rs`: 复用执行骨架
- `memory/manager.rs`: 保存“问题链 + 决策”

---

## 4. 状态机（核心流程）

```text
IDLE
  -> GATE_CHECK
      -> DIRECT_MODE (简单请求)
      -> THINKING (复杂任务)
THINKING
  -> PLANNING
PLANNING
  -> WEIGHTING
WEIGHTING
  -> EXECUTING
EXECUTING
  -> VERIFYING
VERIFYING
  -> REWEIGHT
REWEIGHT
  -> EXECUTING (有剩余任务)
  -> DONE (全部完成)
ANY
  -> FALLBACK_DIRECT (budget/错误触发降级)
```

---

## 5. 触发闸门（Gate）

目标：决定“直接答复”还是“进入 workflow”。

判定输入：
- 用户请求语义复杂度（LLM 轻量分类）
- 预计改动范围（文件数/模块数）
- 风险动作（写文件、执行 shell、网络副作用）
- 架构决策需求（是否有多方案权衡）

输出：
- `Direct`: 走现有对话流
- `Workflow`: 进入主动提问式流程

---

## 6. 主动提问式深思（Questioning）

## 6.1 目标

在执行前产出三件东西：
- `Problem Statement`：问题本质
- `Key Uncertainties`：关键不确定点
- `Decision Basis`：决策依据

## 6.2 运行策略

- 每轮最多 `N` 个问题（默认 5）
- 每个问题回答预算 `M` tokens（默认 400-600）
- 问题按类型覆盖：
  - 目标澄清（到底要达成什么）
  - 约束澄清（不能做什么）
  - 风险澄清（哪里最可能出错）
  - 顺序澄清（先做什么才对）

## 6.3 停止条件

满足任一即停止追问：
- 关键不确定点 <= 1
- 已形成可执行计划
- 达到回合预算
- 判断继续追问收益低

---

## 7. 权重系统（Weights）

## 7.1 评分模型

每个步骤计算总分（0-100）：

`Score = Risk + Impact + Complexity + BlockerValue - DependencyPenalty - DriftPenalty`

默认维度（硬编码）：
- `Risk`: 风险越高越优先（先控风险）
- `Impact`: 影响面越大越优先
- `Complexity`: 高复杂先拆解/先验证
- `BlockerValue`: 解锁后续任务的价值
- `DependencyPenalty`: 依赖未满足时降权
- `DriftPenalty`: 偏离主线时降权

## 7.2 规则要求

- 必须可解释：每个分值都有理由文本。
- 必须可测试：同样输入得到稳定排序。
- 可调参数：仅系数通过 env 调整。

---

## 8. 执行与纠偏

每个步骤执行循环：
1. 执行前微思考（10-20 秒级）
2. 执行操作（工具调用/代码修改）
3. 验证结果（测试/静态检查/行为检查）
4. 更新状态（完成/失败/阻塞）
5. 重算剩余步骤权重

失败策略：
- 失败两次：触发“问题重构”而不是盲目重试
- 失败三次：自动升级为“人工确认节点”

---

## 9. 配置（第一版）

环境变量建议：
- `PRIORITY_AGENT_WORKFLOW_ENABLED=true`
- `PRIORITY_AGENT_WORKFLOW_MAX_DEPTH=3`
- `PRIORITY_AGENT_SOCRATIC_MAX_ROUNDS=5`
- `PRIORITY_AGENT_SOCRATIC_ANSWER_BUDGET=500`
- `PRIORITY_AGENT_WEIGHT_RISK_MUL=1.5`
- `PRIORITY_AGENT_WEIGHT_IMPACT_MUL=1.2`
- `PRIORITY_AGENT_WEIGHT_DRIFT_PENALTY=1.0`

---

## 10. 验收指标（必须量化）

MVP 必测指标：
- `Mainline Hit Rate`：Top-1 是否命中真正 blocker（目标 > 70%）
- `Drift Interruption Rate`：用户“你跑偏了”打断率（目标 < 15%）
- `First Plan Coverage`：首轮计划关键步骤覆盖率（目标 > 80%）
- `Rework Rate`：返工次数/任务（目标持续下降）

---

## 11. 里程碑

### M1：可运行 MVP
- Gate + Questioning + Weights + Executor 最小闭环
- 能输出“问题链 + 权重排序 + 执行日志”

### M2：质量增强
- 增加 drift penalty 与 blocker value
- 接入记忆反馈（历史失败点加权）

### M3：产品化
- TUI 可视化：当前主线、问题链、权重解释
- 支持中断/跳过/人工改权

---

## 12. 风险与防护

关键风险：
- 过度提问导致 token 成本高
- 规则过硬导致灵活性不足
- 递归拆分过深导致流程拖慢

防护策略：
- 预算硬上限 + 自动降级 direct mode
- 保留人工覆盖入口（确认/跳过/强制执行）
- 最大深度限制 + 底层平铺

---

## 13. 对你当前思路的优化建议（结论）

你现在的想法已经对了，建议补这三点：
- 把“提问数量”改成“提问收益率”目标：问题越少越好，但必须击中关键不确定性。
- 在权重里加入 `DriftPenalty`（偏离主线惩罚），这会显著降低“纠结细节”。
- 给每一步输出“为什么现在做它”一句话解释，避免系统自己也失焦。

---

## 14. V1 实施入口

第一批改造文件：
- `src/engine/conversation_loop.rs`
- `src/engine/socratic.rs`
- `src/engine/socratic_executor.rs`
- `src/engine/workflow/{mod,gate,questioning,weights,planner,executor,metrics}.rs`

建议先从 `gate.rs + weights.rs` 开始实现。
