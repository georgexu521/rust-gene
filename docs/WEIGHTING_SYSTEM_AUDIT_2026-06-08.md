# Weighting System Audit — 2026-06-08

Priority Agent 的特色权重系统全面分析：有哪些评分系统、它们之间的关
系、哪些有效、哪些无效、如何改进。

---

## 1. 全景：20 个评分系统，分 4 层

### Layer 1: Action-Taking Spine（每个工具调用都经过）

| 模块 | 文件 | 行数 | 作用 |
|------|------|------|------|
| `CandidateAction` | `src/engine/candidate_action.rs` | 594 | 模型提议候选，自评分 |
| `ActionDecision` | `src/engine/action_decision.rs` | 981 | 运行时按 Stage 加权评分 |
| `ActionReview` | `src/engine/action_review.rs` | 1162 | 硬安全门 + 建议评分 |

**数据流**：
```
CandidateAction (模型打分)
  → ActionDecision (运行时打分 + Stage 加权 + 8 种 Modifier 源)
    → ActionReview (安全门: 不可用/未暴露/无效参数/预算/权限/破坏范围/检查点)
      → Allow / Deny / Revise / AskUser
```

`ActionDecision` 的 Stage 加权公式（6 套系数，按任务阶段切换）：

| 权重维度 | Diagnosis | Planning | Implementation | Verification | Recovery | Closeout |
|---------|-----------|----------|---------------|-------------|----------|----------|
| value | 10 | 10 | 13 | 13 | 12 | 14 |
| risk | -10 | -8 | -12 | -8 | -12 | -8 |
| uncertainty_reduction | 14 | 12 | 7 | 12 | 13 | 5 |
| cost | -8 | -8 | -8 | -9 | -8 | -10 |
| reversibility | 4 | 5 | 5 | 5 | 7 | 3 |
| scope_fit | 12 | 12 | 13 | 10 | 12 | 10 |

公式：
```
action_score = (value*w + uncertainty_reduction*w + reversibility*w + scope_fit*w
               - risk*w - cost*w) / 10
```

结果会 clamp 到 `[-30, 40]`，所以这个分数更像相对排序和 trace
信号，不是绝对概率。

### Layer 2: Memory Lifecycle（记忆写入 / 召回 / 维护 三段）

| 阶段 | 模块 | 公式 | 决策 |
|------|------|------|------|
| **写入** | `src/memory/quality.rs` + `scoring.rs` | 8因子: relevance×0.25 + reuse×0.20 + stability×0.15 + trust×0.15 + novelty×0.10 + risk_reduction×0.10 − token_cost×0.15 − sensitivity_risk×0.20 | Accept≥0.65, Propose≥0.45, Reject<0.45 |
| **召回** | `src/memory/recall.rs` | 7因子: match_quality×0.30 + scope_match×0.20 + recency×0.15 + trust×0.15 + usefulness×0.10 + criticality×0.10 − token_cost×0.15 | Inject≥0.70, Available≥0.50, Omit<0.50, ConflictCapped(0.55x) |
| **维护** | `src/memory/scoring.rs` (keep) | 7因子: recent_use×0.25 + usefulness×0.25 + trust×0.20 + stability×0.15 + scope×0.15 − contradiction×0.20 − redundancy×0.15 | KeepActive≥0.65, CompressOrDemote≥0.40, ArchiveCandidate<0.40 |

**辅助系统**：
- `src/memory/ranking.rs` — 本地关键词匹配 + 语义别名评分
- `src/memory/retrieval.rs` — LLM Rerank + Dialectic Multi-pass
- `src/memory/contradiction.rs` — 矛盾检测（keyword_overlap × (1 − content_similarity)）

### Layer 3: Workflow Planning（计划步骤权重）

⚠️ **两个独立系统在给计划步骤排序，但入口不同**：

| | `workflow_contract.rs` | `workflow/weights.rs` |
|---|---|---|
| 主导方 | **模型**（LLM 打分） | **规则**（确定性） |
| 行数 | 1336 | 884 |
| 因子 | dependency, user_value, risk_reduction, uncertainty_reduction, blocking, cost | Risk, Impact, Complexity, BlockerValue, DependencyPenalty, DriftPenalty, HistoricalFailure |
| 公式 | dependency×0.25 + user_value×0.25 + risk_reduction×0.20 + uncertainty_reduction×0.15 + blocking×0.15 − cost×0.10 | Sigma 归一化 [−120, +120] → [0, 100] |
| 动态调整 | ✅ `learning_planning.rs` 根据失败/恢复/记忆调整 | ❌ 固定规则，env-var 调乘数 |
| 模型覆盖 | ✅ `WeightOverride` (置信≥0.70, Δ≤0.25) | ❌ 无 |

**学习-规划桥接** (`src/engine/learning_planning.rs`, 696行)：
- 收集 failed_tools, failed_workflows, recovery_plans, high_confidence_memories
- 调整目标步骤的 WeightFactors（增量 0.03-0.12）
- 记忆-步骤相关性：Jaccard×0.60 + memory_score×0.40

### Layer 4: Evolution & Risk（辅助控制）

| 模块 | 行数 | 作用 |
|------|------|------|
| `risk_signal_controller.rs` | 574 | 4级风险（Ordinary→Elevated→High），影响 ActionDecision 的 risk 维度 |
| `evolution_controller.rs` | 231 | 7因子进化门控（repeated_failure×0.30 + reuse×0.25 + ...），AutoAccept需≥0.70+risk<0.45 |
| `skill_evolution.rs` | 1367 | 技能创建评分（repeatability×0.25 + complexity×0.25 + ...）|
| `companion_context.rs` | 574 | 同行文件评分（subject tokens + task tokens） |
| `cost_tracker/mod.rs` | 1263 | 工具执行质量（success_rate×0.5 + latency×0.2 + satisfaction×0.3） |
| `questioning.rs` | 673 | 提问相关性（Jaccard 字符集相似度） |
| `workflow/metrics.rs` | 762 | 目标函数（MainlineHit×0.4 + FirstPassQuality×0.35 + CostEfficiency×0.25） |
| `priority/mod.rs` | 181 | 外部 `priority_core::weight_engine` 集成 |

---

## 2. 架构问题

### 问题 1: 两个 Workflow 权重系统语义重叠

`workflow_contract.rs` 和 `workflow/weights.rs` 都会给计划步骤排优先级，但当前
入口不同：前者服务模型主导的 workflow contract，后者仍通过旧
`WorkflowPlanner` / `Plan::reweight` 路径使用。它们不是同一调用链里同时
仲裁同一批 step，但确实形成两套计划权重语言。结果是：
- 当旧 planner 和 workflow contract 产生不同排序时，没有统一解释层
- `workflow/weights.rs` 固定规则无法从失败中学习，而 `workflow_contract.rs` 可以
- 两条路径的系数都硬编码了，没有交叉验证

### 问题 2: CandidateAction 默认模式是纯开销

```
model_led_weighting_enabled()  → 默认 true
CandidateActionMode::from_env() → 默认 "shadow"
```

在 **shadow** 模式下的流程：
1. 模型在回复的 JSON 里带了 `candidate_actions`（3个候选+每个6个0-10分数）— **浪费 token**
2. `ActionDecision` 对每个候选算一遍运行时分数 — **浪费 CPU**
3. `rank_candidate_actions` 对比模型和运行时排名 — **记录但不干预**
4. 实际工具调用走原有的路由 — **对行为零影响**

`ActionDecision` 的分数进入 `ActionReview`，但只作为**建议**，不做硬拦截：
```rust
// ActionReview 中 score concerns are advisory unless they
// map to an explicit safety/evidence gate
```
单纯低分不会驱动 `StopChecker` 停止；只有当低分映射到明确的安全、证据、
权限、范围、预算或检查点问题时，才会变成 revise / deny / ask-user。

只有在 **gated** 模式下才会过滤：
```
Gated mode: 高风险/卡住 → 只执行排名第一的候选
```

但 gated 模式不是 runtime 接管语义选择。当前实现明确保留模型权重和模型顺序
的权威性，runtime 排名只记录 advisory calibration：
```
model_order = cmp(model_score) > cmp(original_index)
runtime_order = cmp(action_score) > cmp(scope_fit) > cmp(-risk) > cmp(original_index)
```
最终选择走 `model_order`；`runtime_order` 只用来记录
`runtime_selected_differs_from_model_order` 和 delta。因此 CandidateAction 的问题
不是“runtime 说了算”，而是默认 shadow 下 token/CPU 开销只换来 trace 校准，
没有进入正常行为闭环。

### 问题 3: 调试地狱

20 个评分系统，13,000+ 行代码。当 agent 行为异常时：
- 是 ActionDecision 的系数歪了？
- 是 CandidateAction 的某个分数没对上？
- 是 LearningPlanning 的不当调整？
- 是 WorkflowContract 和 WeightsEngine 互相矛盾？

Action spine 已有 trace：`ActionDecisionEvaluated`、`CandidateActionsEvaluated`
和 `ActionReviewed` 都会记录关键字段。但 trace 还不统一：Memory recall/write/keep
没有对应的 scoring trace 事件，也没有 dashboard 把 action、memory、workflow 的
评分输出放在同一个视图里。

---

## 3. 哪些有效、哪些无效

### ✅ 真正有效的

| 系统 | 为什么有效 |
|------|-----------|
| `ActionReview` 安全门 | 每个工具调用的硬拦截（权限/预算/破坏范围/无效参数），不依赖权重分数 |
| `score_memory_write` | 真正 gate 了记忆入库，7层评估阻止低质/危险记忆 |
| `score_recall` | 决定了记忆上下文注入量，配合 RetrievalContext budget |
| `risk_signal_controller` | 风险等级影响 closeout 合约、validation 要求，行为上有实质差异 |
| `score_evolution_trigger` | 门控了自动进化，cooldown 机制防止频繁触发 |

### 🟡 理论上对但目前无法验证的

| 系统 | 问题 |
|------|------|
| `learning_planning.rs` | 从失败中学习调整权重——逻辑合理，但缺少端到端测试或 eval case 证明实际效果 |
| `ActionDecision` stage 加权 | 系数选择（Diagnosis 重 uncertainty、Implementation 重 value）符合直觉，但没有校准数据 |
| `workflow_contract.rs` WeightOverride | 模型可以覆盖权重——理论上强，但需要模型给出准确的 confidence 估计 |

### ❌ 无效或净开销的

| 系统 | 问题 |
|------|------|
| `CandidateAction` shadow 模式 | 默认开启时消耗 token + CPU，结果主要进入校准 trace；应默认关闭，只在明确 loop 场景用 gated |
| `workflow/weights.rs` | 和 `workflow_contract.rs` 语义重叠，仍被旧 planner 使用；应先收敛入口，再迁移为 fallback 或删除 |
| `companion_context.rs` | Token 相似度打分对文件选择的作用有限，大模型自己更擅长判断 |

---

## 4. 推荐收敛方案

### 设计约束

这次收敛不应该把 runtime 重新做重。原则是：
- LLM 继续负责语义判断、工程取舍和修复选择。
- Runtime 负责确定性筛查、证据记录、权限/风险/预算/检查点、失败反馈和
  verified closeout。
- 软权重优先进入 trace / diagnostic；只有当它映射到明确 safety/evidence gate
  时才影响 allow / revise / deny。
- 给 LLM 的 guidance 应该短、事实化、可删；不能新增大段 always-on prompt 规则。

### 目标：从 20 个系统收敛到 3 个主干

```
主干 1: Action 安全门
  ActionReview 硬门（保留）
  ActionDecision stage 加权（保留但系数需校准）
  CandidateAction → 默认关掉 shadow，gated 只用于 repeated revision / uncertainty loop

主干 2: Memory 质量门
  score_memory_write → 记忆入库门控
  score_recall → 记忆召回选择
  score_memory_keep → 记忆维护清理
  contradiction → 已接入 maintain_memory()

主干 3: Workflow 权重
  先画清 workflow_contract.rs + workflow/weights.rs 的入口图
  冻结旧 WorkflowPlanner 新能力
  保留 workflow_contract 的学习能力（learning_planning + WeightOverride）
  再吸收 workflow/weights 的规则引擎作为 fallback（模型不参与时用）
```

### 具体行动

**P0: Scoring observability（先看清，不改行为）**
1. 补 memory scoring trace：`RecallScored`、`MemoryWriteScored`、
   `MemoryKeepScored`；`ActionDecisionEvaluated` 已存在。
2. 在 `/trace` 或 `/diagnostic` 增加一个 scoring summary，把 action、
   candidate、memory、workflow 的最新分数放在同一屏。
3. 输出字段只放结果、主因子、decision、source，不把完整公式塞进普通 prompt。

**P1: CandidateAction 默认降噪**
4. CandidateAction 默认关闭 shadow——`model_led_weighting_enabled()` 默认改为
   `false`，保留 env 显式开启 `shadow` / `gated`。
5. gated 继续只在 repeated revision / uncertainty loop 中启用，并保持模型顺序
   权威，runtime 只做 calibration evidence。

**P2: Workflow 入口收敛**
6. 画出 `workflow_contract.rs`、`workflow/weights.rs`、`WorkflowPlanner` 的实际
   入口图，标出哪些路径仍在生产调用、哪些只是测试/legacy。
7. 冻结旧 `WorkflowPlanner` 新能力；迁移前不删除 `workflow/weights.rs`。
8. 将 `workflow/weights.rs` 中仍有价值的 Risk/Impact/Dependency 规则迁移为
   `workflow_contract.rs` 的 fallback source，并保留 trace 字段说明 source。

**P3: Minimal task-guidance experiment**
9. 只从现有 task state / workflow plan / risk signal 生成一个 4 行以内的
   `<task-guidance>`，不引入新评分公式。
10. guidance 只放具体事实：当前阶段、top plan step、最近风险、最近一次低效动作。
11. 用 env 开关或 experiment flag 控制；默认先在 eval/replay 中打开，不直接全量。

**P4: 校准和回归**
12. 设计 10-20 个 behavior case，而不是只回测公式：提前 edit 被 revise、验证后
   closeout、高风险 bash ask/deny、无效重复动作减少、记忆召回不过量。
13. 为 `learning_planning` 写端到端 case，证明失败/恢复/记忆信号能改变 top step。
14. 系数 AB 只能生成建议，不做自动调参；任何系数变更必须带 trace/eval 证据和
   人工 review。

---

## 5. 总结

**权重系统是 Priority Agent 最有特色的部分**，但当前的状态是：

- **广度有余，深度不足**：20 个系统覆盖了所有角落，但核心链路（ActionDecision → ActionReview）评分仅作建议
- **默认模式太保守**：最有野心的模型-运行时校准（CandidateAction）在 shadow 模式下是纯开销
- **两个 workflow 权重系统语义重叠**：旧 planner 仍使用规则权重，新 contract 使用模型/学习权重，需要收敛入口
- **缺少验证闭环**：13,000 行量级的评分代码，缺少证明"有这套系统比没有更好"的校准集和端到端 case

**3 主干收敛**后：先不追求一次性删代码，而是把评分归到 action、memory、
workflow 三个可解释主干里。代码量可以逐步下降，调试路径会先变短，同时保留
已经有效的硬门能力。

---

## 6. 信息闭环分析：运行时算分能否指导 LLM？

### 核心发现：闭环低带宽、滞后、不可操作

运行时计算了大量权重和评分，但 **LLM 主要只能看到历史摘要、风险标签和
workflow plan 权重**。这些信息不是完全不可见，但缺少“下一步应该怎么用”的
前瞻 guidance。追踪每个评分系统到 LLM 可见上下文的路径：

### 逐系统追踪

| 系统 | 运行时算了什么 | LLM 看到了什么 | 能指导行为？ |
|------|--------------|---------------|-------------|
| `ActionDecision` | action_score, value, risk, uncertainty, scope_fit (6维×Stage加权) | 单次完整分数主要在 `result.data["action_decision"]` 和 `ActionDecisionEvaluated` trace 中；LLM 只看到后续 task-state 的历史摘要 | 🟡 低带宽、滞后 |
| `ActionDecision` 历史 | 过去 3 个 action 的分数摘要 | `Recent action scores: grep score=14 value=7 risk=2` — 在 task-state zone 中。`src/engine/task_context/state.rs:1050` | 🟡 事后回顾，不指导下一步 |
| `CandidateAction` | 模型自评 vs 运行时排名的 delta | **完全看不到**。所有 ranking 数据只进 `TraceEvent::CandidateActionsEvaluated`，不入消息。`src/engine/conversation_loop/turn_model_step_controller.rs:216` | ❌ |
| `WorkflowContract` | 运行时通过 learning_planning 调整 importance/share | LLM 在 plan 文本中看到 `importance=0.90 share=0.50`；这些值可能已经被 runtime recompute / learning 调整，但文本没有解释为什么被调高或调低 | 🟡 可见但解释不足 |
| `LearningPlanning` | 根据失败/恢复/记忆调整因子 | 调整结果写入 judgment，`to_turn_context()` 后以新的 importance/share 间接可见；缺少 before/after guidance 文本 | 🟡 间接生效 |
| `RiskSignal` | risk level (Ordinary/Elevated/High) | `Risks: risk_signal=elevated` 和少量 reason — 是风险标签，不是明确行动建议。`src/engine/conversation_loop/risk_signal_controller.rs:155` | 🟡 弱指导 |
| `ActionReview` 拒绝 | deny/revise 决策 + user_reason | ✅ `Action rejected: low_value_action. Inspect the target with file_read first...` — 工具错误消息。`src/engine/conversation_loop/tool_execution_controller/gate.rs:157` | ✅ 有效，但是事后纠正 |
| `EvolutionController` | evolution_score + 决策 | **看不到**。只用于运行时门控，不告诉 LLM | ❌ |

### 结论

当 LLM 即将选择下一个工具时，它的可见上下文是：

```
[记忆快照] [任务状态区（含过去 action 分数回顾）] [工具列表] [对话历史]
```

其中有一些运行时优先级信息，但形式偏弱：历史 action score、plan 的
importance/share、risk 标签。运行时刚算出的 `action_score=18`、`risk=2`、
`scope_fit=9` 通常不会以“下一步建议”的形式进入 LLM 视野。

### 当前架构的本质

```
LLM 决定做什么 ──→ 运行时在边界做二值判断（过 / 不过）
                     ↓
                 主要做 gate，少量做 guide
                 trace 很多，前瞻 guidance 很少
```

运行时用了 13,000 行量级代码计算优先级，但正常 LLM 决策路径主要得到的是
allow/deny/revise、历史摘要和少量风险/计划标签。它还不是一个高质量的实时
决策引导系统。

### 根本目的 vs 实际效果

| 想达到的效果 | 实际效果 |
|-------------|---------|
| 运行时区分任务重要性 | ✅ 算了，算了很多 |
| 引导 LLM 做重要的事 | 🟡 历史和计划权重可见，但缺少下一步 guidance |
| 防止在细枝末节上跑偏 | 🟡 只能事后 deny，不能事前引导 |
| 不钻牛角尖 | 🟡 明确失败、预算、revision 会触发；score-only loops 仍保持 advisory |

**一句话**：权重系统已经是一个不错的**事后分析框架**，但还不是一个清晰的
**实时决策引导系统**。问题不是“分数完全没有流入上下文”，而是流入方式太
弱、太滞后、太难让 LLM 直接行动。

---

## 7. 核心争论：算这些数字有用吗？还是让 LLM 自己判断？

### 双方的论据

**"算数字有用"的理由**：
- 运行时可以访问 LLM 看不到的信息（git 状态、文件变化量、工具使用频率、历史失败模式）
- 运行时可以做机器级的精度计算（6 个维度加权、历史趋势、冲突检测），LLM 做不到
- 运行时可以强制硬拦截（deny），LLM 自己无法约束自己
- ActionReview 的 deny/revise 文本确实在纠正 LLM 的错误——这是验证过的有效机制

**"让 LLM 自己判断"的理由**：
- LLM 对语义、意图、上下文的理解深度远超任何固定公式
- 任何硬编码的权重系数都基于假设，没有校准数据
- 20 个评分系统的维护成本已经超过它们产生的价值
- 最有效的部分（ActionReview deny）本质上不是"评分"，而是"规则检查"

### 我的看法

**两者不矛盾，但当前的比例严重失衡。** 应该反过来：

```
当前：90% 算分 + 10% 引导
目标：20% 算分 + 80% 引导
```

具体来说：

**应该保留的（运行时算，有用）**：

| 功能 | 为什么有用 |
|------|-----------|
| ActionReview 硬门 | deny 工具是运行时独有的能力，LLM 无法自我约束 |
| 记忆质量门控 | 安全扫描、去重、敏感信息检测——这些 LLM 做不到或不可靠 |
| 风险信号 | 综合多种信号判断风险等级，影响行为模式切换 |
| 矛盾检测 | O(n²) 实体比较是机器擅长的，LLM 注意力有限 |

**应该精简的（算太多，但用不上）**：

| 功能 | 为什么没用 |
|------|-----------|
| ActionDecision 6 维评分 | 单次分数主要进入 metadata/trace，LLM 只看到历史摘要；系数没有校准 |
| CandidateAction 排名比较 | shadow 模式下主要是校准 trace；默认开启时成本大于行为收益 |
| Workflow Weights Engine | 和 WorkflowContract 语义重叠，固定规则无学习能力，入口需要收敛 |
| Companion Context 文件评分 | token 相似度远不如 LLM 自己判断 |

**应该新增的（把算出来的数喂给 LLM）**：

不要算一堆数然后藏起来，也不要把公式解释塞给 LLM。更好的方式是把 runtime
已经知道的少量关键事实低噪音地反馈给 LLM：

```
实验开启时，在 LLM 调用前可注入：
<task-guidance>
stage=implementation
top_plan_step="实现核心逻辑" importance=0.95 source=workflow_contract
risk=elevated reason="mutating code after failed validation"
recent_action="file_read score=6 reduced_uncertainty=false"
</task-guidance>
```

这个 block 的约束：
- 不超过 4 行，超过就不注入。
- 不写“必须/禁止/应当”这类新规则，只给事实。
- 不暴露完整公式，避免 LLM 过拟合分数。
- 如果 task-state 已经有同等信息，就不重复注入。

核心原则：**运行时算关键事实，LLM 做语义判断，运行时用硬门兜底**。各司其职，
而不是运行时算一堆 LLM 看不到的冗余分数，也不是把 runtime 变成新的提示词
控制层。
