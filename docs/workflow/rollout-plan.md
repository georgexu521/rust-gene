# WorkflowEngine 上线与回滚计划（W12 交付物）

> 目标：保留 legacy WorkflowEngine 的历史 rollout 记录和回放口径。
> 状态：legacy/eval-only as of 2026-05-08
>
> 该计划不再描述 interactive CLI 的默认产品路径。默认路径已回到 direct
> conversation loop；legacy workflow 只通过显式环境变量开启。

---

## 1. 发布阶段

1. 阶段 A（本地验证）
- 条件：workflow 单测全绿、验收脚本通过。
- 操作：默认开启，开发环境全量使用。

2. 阶段 B（灰度 10%）
- 条件：A 阶段 3 天内无 P0/P1。
- 操作：通过 `PRIORITY_AGENT_LEGACY_WORKFLOW_ENABLED` 对会话灰度开启；
  `PRIORITY_AGENT_WORKFLOW_ENABLED` 仅作为历史兼容变量。

3. 阶段 C（灰度 50%）
- 条件：Mainline Hit Rate 稳定 > 70%。
- 操作：扩大流量并持续监控 Drift/Rework。

4. 阶段 D（全量）
- 条件：连续 7 天核心指标稳定。
- 操作：全量开启 Workflow，保留强制 Direct 开关。

---

## 2. 放量门禁

任一条件不满足则暂停放量：
1. Drift Interruption Rate > 15%
2. Rework Rate 较前一阶段恶化 > 20%
3. P0 故障 > 0
4. Gate 误判率 > 25%

---

## 3. 回滚策略

1. 一级回滚（配置回滚）
- 操作：`PRIORITY_AGENT_LEGACY_WORKFLOW_ENABLED=0`
- 目标：5 分钟内切回 Direct。

2. 二级回滚（功能开关回滚）
- 操作：关闭 M2 优化项（Drift 自适应、收益率控制）。
- 目标：15 分钟内恢复 M1 稳定版本。

3. 三级回滚（版本回滚）
- 操作：回退到上一个稳定 release tag。
- 目标：30 分钟内恢复服务。

---

## 4. 发布后检查

1. 30 分钟：检查 gate 决策分布是否异常。
2. 2 小时：检查失败重试/重构比例。
3. 24 小时：生成首份灰度日报，确认是否扩大流量。
