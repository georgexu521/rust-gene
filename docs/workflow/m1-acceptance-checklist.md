# M1 验收清单（W9 交付物）

> 目标：定义 M1 可执行、可量化的验收标准，每个检查项都有明确的通过/失败判定。
> 状态：V1 Frozen

---

## 1. 验收方法

### 1.1 运行方式

```bash
# 全量验收
cargo test --test workflow_m1_acceptance

# 单条验收
cargo test test_gate_fast_lane

# 手动验收（使用样本集）
./scripts/workflow-m1-acceptance.sh
```

### 1.2 判定标准

| 结果 | 含义 |
|------|------|
| ✅ PASS | 检查项通过，无警告 |
| ⚠️ PARTIAL | 基本通过，有轻微问题 |
| ❌ FAIL | 未通过，需修复 |
| ⏭️ SKIP | 条件不满足，跳过 |

---

## 2. 模块级验收

### 2.1 Gate 模块（gate.rs）

| 编号 | 检查项 | 测试方法 | 通过标准 |
|------|--------|---------|---------|
| G-01 | Fast Lane 匹配 `/help` | 输入 `/help`，检查输出 | 返回 `Direct` |
| G-02 | Fast Lane 匹配 `git status` | 输入 `git status`，检查输出 | 返回 `Direct` |
| G-03 | Heuristic 识别高风险词 | 输入 "重构整个模块" | 返回 `Workflow` |
| G-04 | Heuristic 识别低风险词 | 输入 "修复 typo" | 返回 `Direct` |
| G-05 | 普通任务进入 Workflow | 输入 "新增用户认证" | 返回 `Workflow` |
| G-06 | Gate 输出包含原因 | 任意输入 | `reason` 字段非空 |
| G-07 | Gate 延迟 < 100ms（Fast Lane） | 计时 100 次 | P95 < 100ms |

### 2.2 权重模块（weights.rs）— 已验证，回归测试

| 编号 | 检查项 | 测试方法 | 通过标准 |
|------|--------|---------|---------|
| W-01 | 六维规则全部生效 | `test_engine_compute` | ✅ 已有测试通过 |
| W-02 | 排序稳定性 | `test_engine_sorting` | ✅ 已有测试通过 |
| W-03 | Sigmoid 边界正确 | `test_sigmoid_boundary` | ✅ 已有测试通过 |
| W-04 | 环境变量系数覆盖 | `test_env_multiplier` | ✅ 已有测试通过 |
| W-05 | 依赖完成后无惩罚 | `test_dependency_with_completion` | ✅ 已有测试通过 |

### 2.3 提问模块（questioning.rs）

| 编号 | 检查项 | 测试方法 | 通过标准 |
|------|--------|---------|---------|
| Q-01 | `auto_think()` 方法存在 | 编译检查 | 编译通过 |
| Q-02 | 产出 `ThinkingResult` | 调用 `auto_think()` | 返回非空结果 |
| Q-03 | 问题链长度 <= 5 | 调用后检查 | `question_chain.len() <= 5` |
| Q-04 | 包含 Problem Statement | 检查结果字段 | `problem_statement` 非空 |
| Q-05 | 包含 Decision Basis | 检查结果字段 | `decision_basis` 非空 |
| Q-06 | Budget 耗尽时停止 | 设置极小的 budget 调用 | 标记 `budget_exhausted` |
| Q-07 | 环境变量控制轮数 | `SOCRATIC_MAX_ROUNDS=2` | 只生成 2 轮 |

### 2.4 Planner 模块（planner.rs）

| 编号 | 检查项 | 测试方法 | 通过标准 |
|------|--------|---------|---------|
| P-01 | 从 ThinkingResult 提取步骤 | 调用 `plan()` | 返回 Plan，steps >= 1 |
| P-02 | 步骤有权重 | 检查 PlanStep | `weight` 字段有值 |
| P-03 | 步骤有权重解释 | 检查 PlanStep | `weight_explanation` 非空 |
| P-04 | 依赖识别正确 | 给定已知依赖的输入 | 依赖关系正确 |
| P-05 | 无循环依赖 | 任意输入 | Plan 中无循环依赖 |

### 2.5 Executor 模块（executor.rs）

| 编号 | 检查项 | 测试方法 | 通过标准 |
|------|--------|---------|---------|
| E-01 | 按权重排序执行 | 给定多步骤 Plan | 执行顺序与权重排序一致 |
| E-02 | 依赖未满足时跳过 | 构造有依赖的 Plan | 依赖步骤先执行 |
| E-03 | 执行后更新状态 | 执行一个步骤 | `StepStatus::Completed` |
| E-04 | 失败后重试（1 次） | mock 失败 1 次 | 状态回到 `Pending` |
| E-05 | 失败 2 次标记重构 | mock 失败 2 次 | 标记 `[重构]` |

---

## 3. 集成级验收

### 3.1 WorkflowEngine 端到端

| 编号 | 检查项 | 测试方法 | 通过标准 |
|------|--------|---------|---------|
| I-01 | 简单任务走 Direct | 输入 `/help` | 不进入 Workflow |
| I-02 | 复杂任务走 Workflow | 输入 "新增认证模块" | 进入 Workflow，产出结果 |
| I-03 | Workflow 产出报告 | 执行复杂任务 | 返回非空 `final_report` |
| I-04 | 不破坏现有对话 | 进入 Workflow 后回到 Direct | Direct 模式正常工作 |

### 3.2 样本任务验收

使用 W2 的 20 个样本中的 5 个：

| 编号 | 样本 | 期望行为 |
|------|------|---------|
| S-01 | S-01（修复 clippy） | Gate 返回 Direct |
| S-02 | M-01（新增 slash 命令） | Gate 返回 Workflow → 执行完成 |
| S-03 | M-05（新增权限模式） | Gate 返回 Workflow → 执行完成 |
| S-04 | C-02（Hook 集成） | Gate 返回 Workflow → 计划生成 |
| S-05 | C-07（WorkflowEngine 自身） | Gate 返回 Workflow → 计划生成 |

---

## 4. 非功能验收

| 编号 | 检查项 | 测试方法 | 通过标准 |
|------|--------|---------|---------|
| N-01 | 编译无警告 | `cargo build` | 0 warnings |
| N-02 | Clippy 通过 | `cargo clippy` | 0 warnings |
| N-03 | 测试全绿 | `cargo test` | 全部通过 |
| N-04 | 现有测试不减少 | 对比测试数量 | >= 现有数量 |
| N-05 | Gate 延迟达标 | 基准测试 | P95 < 100ms（Fast Lane） |

---

## 5. 验收脚本模板

```bash
#!/bin/bash
# scripts/workflow-m1-acceptance.sh

set -e

echo "=== M1 Acceptance Checklist ==="
echo ""

# 编译检查
echo "[CHECK] Compilation..."
cargo build --release 2>&1 | grep -i "warning" >& /dev/null && echo "⚠️ WARNINGS FOUND" || echo "✅ PASS"

# Clippy 检查
echo "[CHECK] Clippy..."
cargo clippy -- -D warnings 2>&1 && echo "✅ PASS" || echo "❌ FAIL"

# 测试运行
echo "[CHECK] Tests..."
TEST_OUTPUT=$(cargo test 2>&1)
echo "$TEST_OUTPUT" | grep "test result" | tail -1

# 模块测试
echo ""
echo "[CHECK] Module tests..."
cargo test workflow:: 2>&1 | grep "test result" | tail -1

# 基准延迟（如果存在）
if [ -f target/release/priority-agent ]; then
    echo ""
    echo "[CHECK] Gate latency (sample)..."
    # 简单基准测试
fi

echo ""
echo "=== M1 Acceptance Complete ==="
```

---

## 6. 验收报告模板

```markdown
# M1 验收报告

日期: YYYY-MM-DD
执行人: [name]
版本: [commit hash]

## 结果汇总

| 类别 | 总项 | 通过 | 失败 | 跳过 |
|------|------|------|------|------|
| Gate | 7 | N | N | N |
| 权重 | 5 | N | N | N |
| 提问 | 7 | N | N | N |
| Planner | 5 | N | N | N |
| Executor | 5 | N | N | N |
| 集成 | 4 | N | N | N |
| 样本 | 5 | N | N | N |
| 非功能 | 5 | N | N | N |
| **总计** | **43** | **N** | **N** | **N** |

## 失败项详情

| 编号 | 问题 | 根因 | 修复计划 |
|------|------|------|---------|
| | | | |

## 结论

[ ] M1 通过，可进入 M2
[ ] M1 未通过，需修复后重新验收
```

---

*本文档冻结后，验收检查项不再变更。测试实现可在编码阶段细化。*
