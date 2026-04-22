# WorkflowEngine V1 设计冻结文档

> 状态：V1 Frozen（2026-04-22）
> 来源：`brainstorm.md` + `workflow-spec.md`
> 规则：本文档冻结后，W1-W12 实施期间术语与状态机不再变更。如需变更，走文档修订评审。

---

## 1. 术语表（Frozen）

| 术语 | 英文（代码名） | 定义 |
|------|-------------|------|
| 主动提问式深思 | Socratic / DeepQuestioning | AI 自动提出问题、自动回答、递进深挖，直到形成可执行结论的执行策略 |
| 主线目标 | Mainline Goal | 当前任务的最高优先级目标；所有步骤必须能映射到主线，否则受 DriftPenalty 惩罚 |
| 权重 | Weight / Score | 0-100 的整数，由源码规则引擎计算，决定执行顺序 |
| 闸门 | Gate | 判定用户请求走 DirectMode 还是 Workflow 的决策点 |
| 问题链 | Question Chain | 递进式 Q&A 序列，每个问题基于前一个答案生成 |
| 漂移 | Drift | 执行步骤偏离主线目标的程度 |
| 阻塞点 | Blocker | 当前必须解决才能推进后续任务的关键步骤 |
| 原子操作 | Atomic Step | 不可再拆分的最小执行单元（递归触底后强制平铺） |
| 直接模式 | Direct Mode | 现有对话流，不走 WorkflowEngine |
| 快速通道 | Fast Lane | Gate 中对简单请求的短路判断 |

---

## 2. 状态机（Frozen）

```
                    +-------------+
                    |    IDLE     |
                    +------+------+
                           |
                           v
                    +-------------+
                    | GATE_CHECK  |----(Direct)----> DIRECT_MODE ----> [现有对话流]
                    +------+------+
                           |
                        (Workflow)
                           v
                    +-------------+
                    |  THINKING   |  <-- 主动提问式深思（Socratic）
                    +------+------+
                           |
                           v
                    +-------------+
                    |  PLANNING   |  <-- 生成/递归拆分计划
                    +------+------+
                           |
                           v
                    +-------------+
                    |  WEIGHTING  |  <-- 源码规则引擎计算权重
                    +------+------+
                           |
                           v
                    +-------------+
                    |  EXECUTING  |  <-- 按权重排序执行
                    +------+------+
                           |
                           v
                    +-------------+
                    |  VERIFYING  |  <-- 验证执行结果
                    +------+------+
                           |
                           v
                    +-------------+
                    |  REWEIGHT   |
                    +------+------+
                           |
           +---------------+---------------+
           |                               |
    (有剩余任务)                      (全部完成)
           |                               |
           v                               v
      WEIGHTING (重算)                   DONE
                                           |
                                           v
                                    +-------------+
                                    |   REPORT    |  <-- 汇总 + 保存记忆
                                    +-------------+

任意状态 ----(budget 耗尽/错误/用户中断)----> FALLBACK_DIRECT ----> [现有对话流]
```

### 2.1 状态转换规则

| 从状态 | 到状态 | 触发条件 |
|--------|--------|---------|
| IDLE | GATE_CHECK | 收到用户请求 |
| GATE_CHECK | DIRECT_MODE | Gate 判定为简单请求 |
| GATE_CHECK | THINKING | Gate 判定为复杂任务 |
| THINKING | PLANNING | 问题链收敛（停止条件满足） |
| THINKING | FALLBACK_DIRECT | token budget 耗尽 |
| PLANNING | WEIGHTING | 计划生成完成（含递归拆分） |
| PLANNING | THINKING | LLM 判定需要更多思考 |
| WEIGHTING | EXECUTING | 权重计算完成 |
| EXECUTING | VERIFYING | 步骤执行完成 |
| VERIFYING | REWEIGHT | 验证完成（无论成功/失败） |
| REWEIGHT | WEIGHTING | 剩余任务 > 0 |
| REWEIGHT | DONE | 剩余任务 == 0 |
| DONE | REPORT | 自动生成 |
| * | FALLBACK_DIRECT | 用户中断 / 错误 / budget 耗尽 |

---

## 3. 指标字典（Frozen）

### 3.1 北极星指标（4 硬指标）

| 指标名 | 代码标识 | 定义 | 计算方式 | W1 目标 | W12 目标 |
|--------|---------|------|---------|--------|---------|
| 主线命中率 | `mainline_hit_rate` | Top-1 执行步骤是否命中真正 blocker | 命中次数 / 总任务数 | > 60% | > 70% |
| 漂移打断率 | `drift_interruption_rate` | 用户主动说"跑偏了"的比例 | 打断次数 / 总任务数 | < 25% | < 15% |
| 首轮计划覆盖率 | `first_plan_coverage` | 首轮计划包含的关键步骤占比 | 实际需做步骤 ∩ 计划步骤 / 实际需做步骤 | > 70% | > 80% |
| 返工率 | `rework_rate` | 平均每任务返工次数 | 返工次数 / 总任务数 | 基线 | 每阶段降 10-20% |

### 3.2 过程指标（埋点用）

| 指标名 | 代码标识 | 定义 | 用途 |
|--------|---------|------|------|
| Gate 误判率 | `gate_misclass_rate` | Gate 把复杂任务判为简单 / 反之 | 调优 Gate 阈值 |
| 问题链长度 | `question_chain_len` | 单次 THINKING 产生的问题数 | 控制提问深度 |
| 问题收敛轮数 | `thinking_rounds` | THINKING -> PLANNING 的迭代次数 | 监控思考效率 |
| 权重稳定性 | `weight_stability` | 相同输入多次运行的排序一致性 | 验证规则引擎 |
| 执行成功率 | `step_success_rate` | 步骤一次执行成功比例 | 评估执行质量 |
| 递归触发率 | `recursion_trigger_rate` | 触发递归拆分的任务占比 | 监控复杂度 |

---

## 4. 复杂任务判定标准（Frozen）

Gate 判定输入维度与阈值：

### 4.1 判定维度

| 维度 | 数据来源 | 权重 |
|------|---------|------|
| 语义复杂度 | LLM 轻量分类（1-5 级） | 0.30 |
| 预计改动范围 | 文件数预估 / 关键词匹配 | 0.25 |
| 风险动作 | 是否包含 write/edit/bash/network | 0.25 |
| 架构决策 | 是否涉及多方案权衡 / 新技术引入 | 0.20 |

### 4.2 判定规则

```
score = 语义复杂度 * 0.30 + 改动范围分 * 0.25 + 风险分 * 0.25 + 架构分 * 0.20

if score >= 2.5:
    -> Workflow
else:
    -> Direct
```

### 4.3 快速通道（Fast Lane）—— 硬规则短路

以下情况**直接**走 Direct，不走 LLM 判断：

| 模式 | 匹配规则 |
|------|---------|
| 帮助类 | `/help`、`/clear`、`/status`、`/doctor` |
| 只读查询 | `git status`、`ls`、`cat`（无重定向） |
| 单文件查看 | `file_read` 单个文件，无后续修改意图 |
| 问候闲聊 | "你好"、"谢谢"、"再见" |

### 4.4 误判回退策略

| 误判类型 | 检测方式 | 回退动作 |
|---------|---------|---------|
| 复杂判为简单 | 用户在 Direct 中 3 次要求"再想一下" | 自动升级至 Workflow，保留上下文 |
| 简单判为复杂 | Gate score < 1.5 但进了 Workflow | 允许用户一次 `/skip` 回到 Direct |

---

## 5. 权重计算规则（V1 冻结子集）

### 5.1 评分公式

```
Score = Risk + Impact + Complexity + BlockerValue - DependencyPenalty - DriftPenalty

其中每项为 0-20 的整数，总分范围 [-40, 80]，经 sigmoid 映射到 [0, 100]
```

### 5.2 维度定义与源码规则

| 维度 | 规则（源码硬编码） | 默认值 | 环境变量覆盖 |
|------|-------------------|--------|-------------|
| Risk | 涉及文件写入 +3，bash 执行 +4，网络请求 +2，删除操作 +5 | 0 | `WEIGHT_RISK_MUL` |
| Impact | 修改模块数 > 3 +3，公共接口变更 +4，配置文件变更 +2 | 0 | `WEIGHT_IMPACT_MUL` |
| Complexity | 预估代码行数 > 100 +2，> 500 +5；文件数 > 3 +2 | 0 | `WEIGHT_COMPLEXITY_MUL` |
| BlockerValue | 解锁后续任务数 * 2（最多 +10） | 0 | `WEIGHT_BLOCKER_MUL` |
| DependencyPenalty | 每个未满足依赖 -3（最多 -15） | 0 | `WEIGHT_DEPENDENCY_MUL` |
| DriftPenalty | 偏离 Mainline Goal -4（由规则引擎判定） | 0 | `WEIGHT_DRIFT_MUL` |

### 5.3 可解释性要求

每个步骤的权重必须附带 `explanation: String`，格式：
```
"Risk+4(涉及bash), Impact+3(改3模块), Complexity+2(约200行), Blocker+6(解锁3任务), Dep-3(等#2完成), Drift-0(对齐主线) => Score=72"
```

---

## 6. 主动提问式深思规则（Frozen）

### 6.1 问题类型覆盖（必须覆盖）

| 类型 | 目标 | 示例 |
|------|------|------|
| 目标澄清 | 确认到底要达成什么 | "用户说的'优化'是指性能还是可读性？" |
| 约束澄清 | 确认不能做什么 | "是否有不能修改的公共 API？" |
| 风险澄清 | 确认哪里最可能出错 | "这个改动会影响哪些已有测试？" |
| 顺序澄清 | 确认先做什么才对 | "应该先改数据层还是先改接口层？" |

### 6.2 停止条件（满足任一即停）

1. 关键不确定点 <= 1 个
2. 已形成可执行计划（有具体步骤 + 负责人/工具）
3. 达到回合预算（`SOCRATIC_MAX_ROUNDS`）
4. LLM 判断继续追问收益 < 阈值（默认 0.3）

### 6.3 Budget 控制

| 层级 | 限制 | 默认值 | 环境变量 |
|------|------|--------|---------|
| 每轮对话触发次数 | 最多 1 次 | 1 | — |
| 单次思考最大轮数 | N 轮 Q&A | 5 | `SOCRATIC_MAX_ROUNDS` |
| 单个答案 token 预算 | M tokens | 500 | `SOCRATIC_ANSWER_BUDGET` |
| 单次思考总 token 预算 | 轮数 * 答案预算 * 1.5（含问题生成） | 3750 | `SOCRATIC_TOTAL_BUDGET` |

---

## 7. 递归拆分规则（Frozen）

### 7.1 触发条件（LLM 判断）

满足任一即触发递归：
1. 任务涉及 >= 2 个不相关子领域
2. 预计修改文件数 >= 5
3. 包含高风险操作且无明确回滚方案
4. 依赖关系图深度 >= 3

### 7.2 递归深度限制

```
L0: 用户需求（最大深度 3）
L1: 子计划（最大深度 3）
L2: 孙子计划（最大深度 3）
L3: 强制平铺为原子操作，不再递归
```

环境变量：`WORKFLOW_MAX_DEPTH=3`

### 7.3 触底平铺规则

到达 `WORKFLOW_MAX_DEPTH` 后：
- 不再调用 LLM 判断是否"太复杂"
- 直接按工具类型拆分：每个 write/edit 一个原子步骤，read/grep 合并为一个原子步骤
- 输出警告日志：`"[Workflow] Depth limit reached, flattening remaining steps"`

---

## 8. 模块边界（Frozen）

### 8.1 新增模块

```
src/engine/workflow/
├── mod.rs       # WorkflowEngine 主入口 + 状态机
├── gate.rs      # Gate 判定（Direct vs Workflow）
├── questioning.rs  # 主动提问式深思引擎
├── weights.rs   # 权重计算规则引擎
├── planner.rs   # 计划生成与递归拆分
├── executor.rs  # 按权重执行与回写
└── metrics.rs   # 指标埋点与日志
```

### 8.2 改造模块

| 模块 | 改造点 | 接口约定 |
|------|--------|---------|
| `engine/conversation_loop.rs` | `run_inner()` 前插入 Gate 调用 | Gate 返回 `Direct` 或 `Workflow` |
| `engine/socratic.rs` | 从被动调用改造为主动触发 | 新增 `SocraticSession::auto_think()` |
| `engine/socratic_executor.rs` | 接入 weights.rs 替换简单权重 | 权重由 `WeightEngine::compute()` 提供 |
| `memory/manager.rs` | 保存"问题链 + 决策"到记忆 | 新增 `save_workflow_decision()` |

### 8.3 与现有系统的关系

| 现有系统 | 关系 | 说明 |
|---------|------|------|
| Plan Mode (`/plan`) | 共存 | Plan Mode 是用户手动、人工审批；WorkflowEngine 是 AI 自动 |
| Socratic Tool | 复用内核 | 现有 `socratic_analyze` 保留，Workflow 内部调用改造后的 `SocraticSession` |
| Agent DAG | 执行层可选 | 复杂步骤可委托 Agent 并行执行 |
| Memory | 保存成果 | 问题链、权重决策、执行日志均写入记忆 |

---

## 9. 配置汇总（V1）

| 环境变量 | 默认值 | 说明 |
|---------|--------|------|
| `PRIORITY_AGENT_WORKFLOW_ENABLED` | `true` | 是否启用 WorkflowEngine |
| `PRIORITY_AGENT_WORKFLOW_MAX_DEPTH` | `3` | 递归最大深度 |
| `PRIORITY_AGENT_SOCRATIC_MAX_ROUNDS` | `5` | 单次思考最大 Q&A 轮数 |
| `PRIORITY_AGENT_SOCRATIC_ANSWER_BUDGET` | `500` | 单个答案 token 预算 |
| `PRIORITY_AGENT_SOCRATIC_TOTAL_BUDGET` | `3750` | 单次思考总 token 预算 |
| `PRIORITY_AGENT_WEIGHT_RISK_MUL` | `1.0` | Risk 维度系数 |
| `PRIORITY_AGENT_WEIGHT_IMPACT_MUL` | `1.0` | Impact 维度系数 |
| `PRIORITY_AGENT_WEIGHT_DRIFT_MUL` | `1.0` | DriftPenalty 系数 |

---

## 10. 验收标准（W1 专项）

本文档通过评审的标志：

- [ ] 术语表被所有相关方认可（无歧义）
- [ ] 状态机覆盖所有异常路径（FALLBACK_DIRECT）
- [ ] 指标字典有可计算的公式（不是定性描述）
- [ ] 复杂任务判定标准有明确阈值（score >= 2.5）
- [ ] 权重规则有源码级伪代码（可直接翻译为 Rust）
- [ ] 递归规则有深度限制和触底行为
- [ ] 模块边界不引入循环依赖

---

## 11. 变更日志

| 版本 | 日期 | 变更 |
|------|------|------|
| V1 Frozen | 2026-04-22 | 初始冻结，基于 brainstorm.md + workflow-spec.md |

---

*本文档冻结后，W1-W12 实施期间以此为准。任何修订需走评审流程并更新变更日志。*
