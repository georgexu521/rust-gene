# 复杂真实场景测试计划
Status: Active

> 日期：2026-06-01
> 背景：agent 已通过小规模编码测试（读文件、简单编辑、cargo check）。
> 目标：用真实规模的编程任务验证 agent 的端到端能力。

---

## 测试哲学

小规模测试验证了「工具能调用、代码能编辑」，但真实编程场景的难点在于：

| 难点 | 小测试表现 | 需要验证 |
|---|---|---|
| 多文件跨模块编辑 | 未测 | 改一个 API 需要同步改 N 个文件 |
| bash 执行循环 | 未测 | cargo test → 失败 → 读错误 → 修复 → 重试 |
| 上下文管理 | 未测 | 大任务需要读很多文件，上下文会不会溢出 |
| 自我修复 | 未测 | 编辑出错后能否通过编译错误定位并修复 |
| 长时间运行 | 未测 | 多轮迭代中 token 预算、force summary 表现 |
| 真实代码库理解 | 未测 | 理解项目结构、找到正确的修改位置 |

---

## 测试场景

### 场景 1：重构 — 合并重复代码

**难度**：⭐⭐⭐ | **文件数**：3-5 | **预估轮数**：5-8

**任务**：
```
In src/engine/conversation_loop/tool_execution_controller.rs,
there is repeated logic for collecting read-only results that appears
in 3 places (around the flush points). Refactor this into a single
helper method called flush_read_only_batch that takes the read_only_jobs
vector and returns the collected results. Update all 3 call sites.
Run cargo check after the refactor.
```

**验证点**：
- [ ] 正确识别 3 处重复代码
- [ ] 创建 `flush_read_only_batch` 方法，签名合理
- [ ] 替换所有调用点
- [ ] cargo check 通过

---

### 场景 2：Bug 修复 — 修复并验证

**难度**：⭐⭐⭐⭐ | **文件数**：1-2 | **预估轮数**：6-12

**任务**：
```
I've introduced a bug: in src/engine/repair/storm.rs, I changed the
normalize_value function to NOT sort object keys. This means the hash
is non-deterministic and some tests fail intermittently.

Run: cargo test engine::repair::storm::tests::  -- --nocapture
Observe the failures (tests may pass or fail unpredictably).
Fix the normalize_value function to sort keys again.
Run the tests again to verify the fix.
```

**验证点**：
- [ ] 运行 cargo test 并观察结果
- [ ] 正确定位 `normalize_value` 函数
- [ ] 修复排序逻辑
- [ ] 重新运行测试，全部通过
- [ ] 如果测试不稳定，解释原因

---

### 场景 3：跨模块 API 变更

**难度**：⭐⭐⭐⭐ | **文件数**：4-6 | **预估轮数**：8-15

**任务**：
```
The AgentManager::emit_progress method in src/agent/manager.rs takes
&AgentId as a parameter. Change it to also accept &str by implementing
From<&str> for AgentId. Then update all call sites across the codebase
that currently construct AgentId("...".to_string()) to use the simpler
string form. Run cargo check and cargo test agent:: after the change.
```

**验证点**：
- [ ] 实现 `From<&str> for AgentId`
- [ ] 找到所有 call site（grep 或 code search）
- [ ] 更新每个 call site
- [ ] cargo check + cargo test 通过

---

### 场景 4：新 Feature 开发

**难度**：⭐⭐⭐⭐⭐ | **文件数**：3-5 | **预估轮数**：10-20

**任务**：
```
Add a new feature to the repair pipeline: a "Rate Limiter" module.
Create src/engine/repair/rate_limit.rs with a RateLimitState struct
that tracks how many times each tool has been called in the current
turn, and returns RateLimitDecision::Allow or RateLimitDecision::Block
if a tool exceeds a configurable per-turn limit (default: 10).

Register the module in src/engine/repair/mod.rs.
Add a unit test that verifies the 11th call to the same tool is blocked.
Wire it into tool_execution_controller.rs so it's checked after the
storm breaker but before the gate evaluation.
```

**验证点**：
- [ ] 创建 rate_limit.rs，结构体和方法正确
- [ ] 注册到 mod.rs
- [ ] 添加测试
- [ ] 集成到 tool_execution_controller
- [ ] cargo test 全部通过

---

### 场景 5：真实项目 — 给本项目加一个 CLI flag

**难度**：⭐⭐⭐ | **文件数**：2-3 | **预估轮数**：5-10

**任务**：
```
Add a --max-iterations flag to the CLI (src/main.rs) that overrides the
default max_iterations value. This flag should be usable in all modes
(--cli, --tui, --eval-run).

Steps:
1. Add the flag to StartupMode parsing
2. Pass it through to StreamingQueryEngine / ConversationLoop
3. Verify it works by running with --eval-run --max-iterations 1 against
   a prompt that would normally need more iterations (you'll see force_summary trigger)
```

**验证点**：
- [ ] 正确解析 --max-iterations flag
- [ ] 传递到引擎层
- [ ] 实际限制迭代次数
- [ ] 与 force_summary 配合工作

---

### 场景 6：压力测试 — 大规模重构提示

**难度**：⭐⭐⭐⭐⭐ | **文件数**：10+ | **预估轮数**：15-30

**任务**：
```
The src/memory/ module has several sub-modules that each define their
own error handling with String errors. Audit all files under src/memory/
and replace ad-hoc String-based errors with proper anyhow::Result or
custom error types where appropriate. Focus on:

1. src/memory/manager.rs — submit_candidate returns MemoryWriteOutcome
   with String reasons; improve error clarity
2. src/memory/extraction.rs — parse_llm_memory_candidates silently
   returns empty Vec on parse failure; add proper error logging
3. src/memory/persistence.rs — append_flush_record ignores write errors;
   add error propagation

For each file: read, identify weak error handling, improve it, verify
with cargo check.
```

**验证点**：
- [ ] 逐个文件检查和改进
- [ ] cargo check 无新增警告
- [ ] 改进后错误信息更清晰
- [ ] 不破坏现有测试

---

## 运行方式

```bash
cd /Users/georgexu/Desktop/rust-agent
cargo build -q

ENV="env MINIMAX_API_KEY=sk-cp-... MINIMAX_BASE_URL=https://api.minimaxi.com/v1 MINIMAX_MODEL=MiniMax-M2.7"
BIN="./target/debug/priority-agent --eval-run"

# 每个场景
echo "<prompt>" > /tmp/eval-s1.txt
$ENV $BIN --prompt-file /tmp/eval-s1.txt --output /tmp/eval-out-s1.txt --events /tmp/eval-ev-s1.jsonl
```

---

## 评分标准

| 维度 | 权重 | 说明 |
|---|---|---|
| 任务完成度 | 30% | 是否产出正确的代码变更 |
| 工具使用 | 20% | file_read/edit/bash/test 使用是否恰当 |
| 代码质量 | 20% | 生成的代码风格、正确性 |
| 自我验证 | 15% | 是否主动 cargo check/test |
| 修复能力 | 15% | 失败后能否自愈 |
| **总分** | **100%** | |

---

## 实施优先级

| 顺序 | 场景 | 理由 |
|---|---|---|
| 1 | 场景 5：CLI flag | 简单，熟悉代码结构 |
| 2 | 场景 1：重构合并 | 中等，验证多文件编辑 |
| 3 | 场景 2：Bug 修复 | 验证 bash+test 循环 |
| 4 | 场景 3：跨模块 API | 验证全局搜索替换 |
| 5 | 场景 4：新 Feature | 验证从零开发 |
| 6 | 场景 6：大规模重构 | 压力测试全套能力 |

## 与 Reasonix 的最终对比

完成本计划后，agent 应能证明：

| 能力 | Reasonix | Priority Agent |
|---|---|---|
| 简单读/分析 | ✅ | ✅ |
| 文件编辑 | ✅ | ✅ |
| 多文件重构 | ✅ | ? 待验证 |
| bash+test 循环 | ✅ | ? 待验证 |
| 自我修复 | ✅ | ? 待验证 |
| 新功能开发 | ✅ | ? 待验证 |
| 大规模压力 | ? | ? 待验证 |
