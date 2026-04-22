# 权重规则规格（W4 交付物）

> 目标：冻结六维权重模型、系数调参边界与可解释输出格式。
> 状态：V1 Frozen
> 对应实现：`src/engine/workflow/weights.rs`

---

## 1. 评分模型

原始分值（可正可负）：

`RawScore = Risk + Impact + Complexity + BlockerValue - DependencyPenalty - DriftPenalty`

归一化分值（0-100）：

`Score = sigmoid(RawScore)`

约束：
- 同输入必须稳定输出同排序。
- 排序结果必须包含维度解释文本。
- 仅允许通过环境变量调整系数，不允许替换维度本身。

---

## 2. 六维定义

| 维度 | 作用 | 取值方向 | 备注 |
|------|------|----------|------|
| Risk | 风险优先处置 | 越高越优先 | 危险操作/高破坏性任务前置 |
| Impact | 影响面优先 | 越高越优先 | 跨模块/公共接口变更优先 |
| Complexity | 复杂项优先拆解 | 越高越优先 | 复杂任务先明确边界 |
| BlockerValue | 解锁价值优先 | 越高越优先 | 能解锁后续步骤的任务前置 |
| DependencyPenalty | 依赖未满足惩罚 | 越高越后置 | 未满足依赖时自动降权 |
| DriftPenalty | 偏离主线惩罚 | 越高越后置 | 与主线目标无关的任务降权 |

---

## 3. 调参边界（ENV）

| 环境变量 | 默认值 | 说明 |
|---------|--------|------|
| `PRIORITY_AGENT_WEIGHT_RISK_MUL` | 1.0 | Risk 系数 |
| `PRIORITY_AGENT_WEIGHT_IMPACT_MUL` | 1.0 | Impact 系数 |
| `PRIORITY_AGENT_WEIGHT_COMPLEXITY_MUL` | 1.0 | Complexity 系数 |
| `PRIORITY_AGENT_WEIGHT_BLOCKER_MUL` | 1.0 | BlockerValue 系数 |
| `PRIORITY_AGENT_WEIGHT_DEPENDENCY_MUL` | 1.0 | DependencyPenalty 系数 |
| `PRIORITY_AGENT_WEIGHT_DRIFT_MUL` | 1.0 | DriftPenalty 系数 |

调参规则：
1. 线上仅允许 `[0.5, 2.0]` 的系数范围。
2. 一次只调一个系数，保持可归因。
3. 调参必须附带 20 条样本回放对比结果。

---

## 4. 可解释输出格式

每个步骤必须输出：
1. 最终分值 `normalized_score`。
2. 6 个维度的 `raw_score/weighted_score`。
3. 一句理由：`why_now`（为什么当前先做它）。

示例：

```json
{
  "step": "迁移数据库 schema",
  "normalized_score": 86,
  "dimension_scores": [
    {"dimension":"Risk","raw":8,"weighted":8.0,"explanation":"Risk+8 (migration)"},
    {"dimension":"Impact","raw":7,"weighted":7.0,"explanation":"Impact+7 (cross-module)"}
  ],
  "why_now": "该步骤高风险且解锁后续 3 个步骤，需优先完成。"
}
```

---

## 5. 验收项

1. `cargo test workflow::weights` 全绿。
2. 同一输入运行 100 次，排序一致率 100%。
3. 每个步骤都包含非空 explanation。
4. 对齐主线时 DriftPenalty 必须显著低于跑偏步骤。
