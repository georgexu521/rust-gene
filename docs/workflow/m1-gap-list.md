# M1 缺口收敛清单（W10 交付物）

> 目标：基于 M1 验收演练输出缺口、优先级、修复排期。
> 状态：V1
> 数据来源：`m1-acceptance-checklist.md` + 当前代码巡检

---

## 1. 总览

| 缺口ID | 类别 | 优先级 | 状态 | 负责人 | 目标周 |
|--------|------|--------|------|--------|--------|
| GAP-01 | 文档交付缺失（W4/W10-W12） | P1 | ✅ 已补齐 | Core | W10 |
| GAP-02 | `WorkflowEngine::run_gate` 与 Gate 规则未对齐 | P1 | ✅ 已修复 | Core | W10 |
| GAP-03 | workflow 模块编译告警 | P2 | ✅ 已清理 | Core | W10 |
| GAP-04 | M1 验收脚本缺失 | P1 | ✅ 已补齐 | Core | W10 |
| GAP-05 | 周报模板缺失 | P2 | ✅ 已补齐 | PM | W10 |

---

## 2. 细项说明

### GAP-01 文档交付缺失
- 问题：`weights-spec.md`、`m1-gap-list.md`、`m2-optimization-plan.md`、`rollout-plan.md`、`operator-guide.md` 未落盘。
- 影响：W10-W12 无法按计划闭环验收。
- 处理：全部补齐到 `docs/workflow/`。

### GAP-02 Gate 行为不一致
- 问题：`WorkflowEngine` 内部 `run_gate()` 直接放行，和 `gate.rs` 规则不一致。
- 影响：外部直接调用 `WorkflowEngine::run()` 时可能绕过闸门策略。
- 处理：改为实际调用 `Gate::decide()`，Direct 决策直接返回错误并解释原因。

### GAP-03 编译告警
- 问题：workflow 相关模块存在未使用 import/变量与无效比较告警。
- 影响：质量门禁和后续 clippy 收敛效率下降。
- 处理：已清理并回归测试。

---

## 3. 当前判定

M1 当前状态：`Usable (Not Production-ready)`
- ✅ Gate / Questioning / Planner / Weights / Executor 主链路可运行
- ✅ Workflow 单测通过
- ⚠️ 仍需继续推进 M2（指标自动化、灰度策略、运维演练）
