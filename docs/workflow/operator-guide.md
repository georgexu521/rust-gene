# WorkflowEngine 运维手册（W12 交付物）

> 目标：提供 legacy WorkflowEngine 的验证、故障排查、应急处理流程。
> 状态：legacy/eval-only as of 2026-05-08
>
> 默认 interactive CLI 不再启用这一套 legacy WorkflowEngine。正常代码任务走
> `conversation_loop` + `CodeChangeWorkflowRunner`。只有回放历史 workflow
> benchmark 或专项调试时才显式开启。

---

## 1. 常用开关

```bash
# legacy/eval-only 总开关。旧变量仍兼容，但新脚本优先使用这个名字。
export PRIORITY_AGENT_LEGACY_WORKFLOW_ENABLED=true
export PRIORITY_AGENT_WORKFLOW_ENABLED=true

# 深度与预算
export PRIORITY_AGENT_WORKFLOW_MAX_DEPTH=3
export PRIORITY_AGENT_SOCRATIC_MAX_ROUNDS=5
export PRIORITY_AGENT_SOCRATIC_ANSWER_BUDGET=500

# 权重调参
export PRIORITY_AGENT_WEIGHT_RISK_MUL=1.0
export PRIORITY_AGENT_WEIGHT_IMPACT_MUL=1.0
export PRIORITY_AGENT_WEIGHT_DRIFT_MUL=1.0
```

---

## 2. 日常巡检

1. 运行 `cargo test workflow --quiet`。
2. 运行 `scripts/workflow-m1-acceptance.sh`。
3. 检查最近 24h 指标趋势（主线命中/跑偏打断/返工率）。
4. 检查是否出现大量 `Gate decided Direct mode` 错误。

---

## 3. 故障排查流程

### 3.1 症状：频繁跑偏
1. 检查 `DriftPenalty` 系数是否被异常下调。
2. 抽样回放 Top-1 步骤是否偏离 mainline。
3. 临时提升 `PRIORITY_AGENT_WEIGHT_DRIFT_MUL` 到 1.2 观察。

### 3.2 症状：提问过多/太慢
1. 检查 `SOCRATIC_MAX_ROUNDS` 与 `ANSWER_BUDGET`。
2. 将 `MAX_ROUNDS` 从 5 下调至 3 做灰度。
3. 比较计划覆盖率是否恶化。

### 3.3 症状：Workflow 未触发
1. 检查 `PRIORITY_AGENT_LEGACY_WORKFLOW_ENABLED` 或兼容变量
   `PRIORITY_AGENT_WORKFLOW_ENABLED` 是否为 `0/false`。
2. 检查输入是否命中 Fast Lane。
3. 检查 `Gate::decide()` reason 输出。

---

## 4. 应急处理

1. 发现 P0：立即关闭 Workflow 总开关，切回 Direct。
2. 记录事故窗口、触发任务、关键日志与回退耗时。
3. 24 小时内提交 RCA（根因 + 修复 + 防复发）。
