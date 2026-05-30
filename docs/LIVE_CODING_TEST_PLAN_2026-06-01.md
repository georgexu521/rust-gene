# Agent 实际编码能力测试

> 日期：2026-06-01
> 模型：MiniMax M2.7
> 运行方式：`cargo build && env MINIMAX_API_KEY=... ./target/debug/priority-agent --eval-run --prompt-file <task>`

---

## 测试矩阵

| # | 类型 | 难度 | 测试目标 |
|---|---|---|---|
| T1 | 分析 | 低 | 基础 read + 理解 |
| T2 | Bug 修复 | 中 | read → edit → 正确修改 |
| T3 | 添加测试 | 中 | read → edit 多位置 → 验证逻辑 |
| T4 | 多文件编辑 | 中 | read 多文件 → 跨文件 edit |
| T5 | 验证+修复循环 | 高 | edit → cargo check → 根据错误修复 |
| T6 | 边界压力 | 高 | 小迭代上限 → 触发 force summary |

---

## T1 — 基础分析：理解一个模块

**Prompt**：
```
Read src/engine/repair/mod.rs and src/engine/repair/truncation.rs.
Explain what the repair pipeline does and how the three modules
(storm, truncation, rollback) fit together.
```

**预期**：agent 读取两个文件，准确描述修复流水线的三层结构。

**测试目标**：基础 read、多文件 read、代码理解

---

## T2 — Bug 修复：修正 force_summary 边界条件

**Prompt**：
```
There is a bug in src/engine/conversation_loop/force_summary.rs:
when max_iterations is 1, the should_force_summary function returns
true on iteration 0, which means the model gets a wrap-up instruction
before its first response.

The function currently reads:
  iteration >= max_iterations.saturating_sub(2)

Fix it so that single-iteration tasks don't get force-summarized on
their first attempt. Add a test case for max_iterations = 1.
```

**预期**：agent 修改函数 + 添加测试。如果它读到的是我们刚改过的代码，它会发现 bug 已被修复。

**注意**：我们的代码已经修好了这个 bug。如果 agent 发现 bug 已修复，它应该报告"已经修好了，这是已有的测试"而不是再次修改。

**测试目标**：read → 理解 → 判断 → 决定是否 edit

---

## T3 — 添加测试：给 storm.rs 补一个边界测试

**Prompt**：
```
Read src/engine/repair/storm.rs and its test module at the bottom.
Add a new test called storm_different_arg_types_not_confused that
verifies that calling the same tool with different argument TYPES
(e.g., string "5" vs number 5) is NOT treated as a repeated call,
since the normalized hash should differ.
```

**预期**：agent 读取 storm.rs，在 `#[cfg(test)]` 块中添一个新测试。

**测试目标**：read → edit 精确位置 → 正确理解 normalize 逻辑 → 写出有效测试

---

## T4 — 多文件编辑：给两个模块加代码示例

**Prompt**：
```
Read src/engine/conversation_loop/force_summary.rs and
src/engine/repair/storm.rs.

Add a "Usage Example" doc comment section to both files showing
how each module is used in the tool execution pipeline. The example
should be realistic and reference the actual function signatures.
```

**预期**：agent 读两个文件，理解各自的 API，在两个位置分别追加文档。

**测试目标**：多文件 read → 多位置 edit → 文档质量

---

## T5 — 验证+修复循环：改错代码然后修复

**Prompt**：
```
In src/engine/repair/storm.rs, find the normalize_and_hash_args function.
Temporarily introduce a bug: change "keys.sort();" to "// keys.sort();"
(comment out the sort line). Then run "cargo test engine::repair::storm::"
and observe that some tests fail because the hash is no longer
deterministic. Fix the bug by restoring the sort line. Explain what
happened.
```

**预期**：agent edit → cargo test → 发现失败 → 分析原因 → 修复 → cargo test 通过。

**测试目标**：完整的 edit → verify → repair 循环。这是最核心的编码能力测试。

---

## T6 — Force Summary 边界压力测试

**Prompt**：
```
Explain the full architecture of the Priority Agent conversation loop.
Cover: how messages flow, how tools are dispatched, how repair works,
how context is managed, and how the session ends.
```

**配置**：设置 `PRIORITY_AGENT_MAX_ITERATIONS=3`（环境变量）

**预期**：任务太大，3 轮迭代不够。Agent 应在第 2-3 轮触发 force summary，输出一个结构化的总结而不是失败。

**测试目标**：force summary 在实际压力下的表现

---

## 运行计划

```bash
cd /Users/georgexu/Desktop/rust-agent
cargo build -q

ENV="env MINIMAX_API_KEY=sk-cp-... MINIMAX_BASE_URL=https://api.minimaxi.com/v1 MINIMAX_MODEL=MiniMax-M2.7"
BIN="./target/debug/priority-agent --eval-run"

# T1
echo "<prompt>" > /tmp/eval-t1.txt
$ENV $BIN --prompt-file /tmp/eval-t1.txt --output /tmp/eval-out-t1.txt --events /tmp/eval-ev-t1.jsonl

# T2-T6 similarly...
```

---

## 评分标准

| 维度 | 满分 | 说明 |
|---|---|---|
| 任务完成度 | 5 | 是否完成了任务要求 |
| 工具使用正确性 | 5 | file_read/edit/bash 使用是否恰当 |
| 代码质量 | 5 | 生成的代码是否正确、符合风格 |
| 自我验证 | 5 | 是否主动运行了 cargo test/check |
| 修复能力 | 5 | 出错后是否能自行修复 |
| 新 feature 触发 | 5 | 是否触发了 read-before-edit/storm/truncation 等 |

**总分**：30 分/任务
