
## 6. 上下文传递和缓存分析

### 每次 LLM 请求的实际结构

```
[系统提示] 稳定前缀（缓存命中, $0.0028/M tokens）
  包含: AGENTS.md, SOUL.md, TOOLS.md, working dir

[第 1 轮历史] user + assistant + tool outputs
[第 2 轮历史] user + assistant + tool outputs
...

[当前轮 user] 这条消息被 prepend 了动态上下文:
  <task-state> 当前任务状态、plan mode
  <task-contract> workflow contract + context pack
  <context_ledger> 最近读过的文件、改过的文件、验证结果
  <task-guidance> 当前 stage、risk、recent action score
  <relevant-memory> 记忆召回结果
  用户原始消息
```

### 缓存命中不是免费的——是两个概念

| | 缓存 hit | 缓存 miss |
|---|---|---|
| 价格（DeepSeek） | $0.0028 / 1M tokens | $0.14 / 1M tokens |
| 什么被 hit | 系统提示 + 前 N-1 轮历史 | 第 N 轮新内容 |
| 每增加一轮 | 不影响 | 多 miss 一轮 |

命中了便宜 50 倍，但不免费。每增加一轮对话，新内容（上一轮的 assistant/tool 回复）始终是 miss。

### 和 OpenCode 的核心差异

| | Priority Agent | OpenCode |
|---|---|---|
| 历史传递 | 全部原样 | 压缩后: 旧轮摘要 + 保留 2 轮 |
| 压缩触发 | 80% token 预算（被动） | model_limit - output - 20000（主动） |
| 动态上下文 | 注入到 user 消息（保缓存） | 全在 system prompt |
| 缓存策略 | MD5 指纹监控 | 依靠提供方原生缓存 |
| closeout 验证 | 需要原始 tool output 证据 | 不需要 |

### 全量历史的问题

第 20 轮时，即使前面 18 轮都 cache hit（便宜），最后 2 轮的 miss 部分（assistant 长回复 + tool output）也可能几千 token x $0.14，累积成本显著。

---

## 7. 选择性压缩方案

### 不能全抄 OpenCode

OpenCode 的压缩会丢掉 tool output 原文。但我们有 closeout verification——需要原始证据（cargo test 输出、rg 结果）来验证改动。

如果把 `cargo test` 结果压缩成 "tests passed"，closeout controller 会判定"证据不足"，触发 repair loop——反而多花 token。

### 方案: 分级压缩

```
keep: 系统提示 (stable prefix, 始终缓存命中)
keep: 所有 user/assistant 对话文本（语义信息，较短）
keep: 最后 2 轮的 tool output 原文 ← closeout 验证需要
compress: 前 N 轮的工具输出 → 摘要格式
         保留: 命令、exit code、关键行（失败行、匹配行）
         丢弃: 完整 stdout/stderr（build output 可能几千行）
```

### 摘要格式

```
[compressed tool output #3]
cmd: cargo test -q
exit: 0
lines: 2 passed, 0 failed
key: test score_memory_write ... ok
```

约 100 bytes，替代可能几 KB 的原始输出。

### 已有基础——复用现有压缩器

`src/engine/context_compressor.rs` 已有 Snip/MicroCompact/AutoCompact 三种策略和 Hermes 8-section 模板。不需要重写，只需:
1. 加一个 `SelectiveCompress` 策略（只压 tool output，保留对话文本）
2. 在 token 达到 50% 时就主动触发（不等 80%）
3. 加 `closeout_evidence_preserve` flag —— 标记哪些 tool output 是 required_validation 证据，不可压缩
# Routing System Analysis — 2026-06-08

Priority Agent 路由系统和 OpenCode 的对比分析，以及改进方向。

---

## 1. 当前路由架构

### 入口

`turn_setup_controller.rs:58` → `IntentRouter::new().route_with_learning(...)`

### 决策链

```
用户消息
  │
  ├── heuristics.rs: 20+ 个关键词检测函数
  │
  ├── intent_router.rs: cascade 优先级 if/else
  │
  └── IntentRoute { intent, workflow, retrieval, reasoning, risk, tools }
       │
       ├── tool_orchestrator.rs: 按 route 过滤工具面
       ├── resource_policy.rs: 按 reasoning 分配资源
       ├── route_recovery.rs: LLM 越权恢复
       └── 各 controller: 按 route 做不同逻辑
```

### 12 个 Intent 类别

| Intent | 触发条件 | Workflow | Tools |
|--------|---------|----------|-------|
| CodeChange | 重构/修改/实现 + 代码信号 | CodeChange | 全部编码工具 |
| Debugging | 修复/bug/报错 | BugFix | 编码 + lsp |
| Memory | 记住/回忆 无代码改动 | Direct | memory 工具 |
| Research | 搜索/查找 web | Research | web_search |
| Configuration | 安装/配置 | Direct | mcp/config |
| Delegation | 分派/委托 | Delegation | agent/swarm |
| Planning | 设计/计划 无改动 | Planning | read/plan |
| DirectAnswer | 解释/告诉/问答 | Direct | file_read/glob |

---

## 2. OpenCode 对比

### OpenCode 没有意图分类

用户手动选 build/plan agent。路由 = 权限 ruleset。没有自动分类。

### 我们的优势

- 自动意图分类：用户不需要选模式
- 检索策略分级：Light/Project/Memory/Web/Full
- 资源预算：按 reasoning depth 分配
- 路由恢复：LLM 越权时扩展 read、阻止 mutation
- 风险自动评估

### OpenCode 的优势

- 用户显式选 agent：不会误分类
- 子 agent 委托：task 工具简单
- 权限即路由：plan = deny 编辑工具

---

## 3. 四个问题

### 问题 1: 不读上下文 —— 第二轮丢 intent

```
第 1 轮: "帮我重构 scoring.rs" → CodeChange OK
第 2 轮: "还有那个文件也改一下" → ??? (没有关键词，fallback 到 DirectAnswer)
```

**解决**: 上一轮 closeout 未完成且是 CodeChange/Debugging → 本轮继承 intent，
除非检测到 topic switch。

### 问题 2: 优先级硬编码 —— 谁先谁赢

20 个 if/else cascade，先匹配到的赢，不是更准确的赢。

**解决**: heuristic 返回 `Option<(IntentKind, f32)>`，取最高 confidence。
多 intent 接近时给更多上下文让 LLM 判断。

### 问题 3: 不确定时硬选 Light

```
用户: "看看这个" → 无信号 → DirectAnswer + Light
LLM: 看不到文件内容，只能猜
```

**解决**: 所有 confidence < 0.3 → 至少保持 Project 级别上下文。

### 问题 4: 不会用 LLM 辅助分类

纯关键词匹配。LLM 的语义理解完全没有用到路由里。

**解决**: confidence 低时，在已有 context 中让 LLM 自报 intent。

---

## 4. 改进方向

### Phase 1: 基础（低成本）

1. 继承上一轮 intent（closeout 未完成时）
2. confidence-based 路由替代 cascade 优先级
3. 不确定时给更多上下文（至少 Project）

### Phase 2: LLM 辅助

4. confidence 低时让 LLM 自报 intent

### Phase 3: 校准

5. 用边缘消息测试路由准确性

---

## 5. 路由的价值：LLM 自选工具，为什么还要路由？

### 路由和工具选择是两层

```
路由决定:                      LLM 决定:
  你能用哪些工具                在这些工具里你用哪个
  你能看多少上下文              你看完后怎么回答
  你有多少时间和预算            你分几步做
```

路由输出的不是"选哪个工具"，而是"给多大的工具面"。

### 路由的 ROI

| 路由层 | 做什么 | 不做的代价 |
|--------|--------|-----------|
| 工具面控制 | 问答不给编辑工具 | LLM 在问答时可能改代码 |
| 上下文预算 | 问答不给 Project 级 | 每个请求浪费几千 token |
| 风险分级 | 代码改动标 High | 简单问答走完整验证 |
| 检索策略 | 问答不加载记忆 | 每个请求加载全量记忆 |

**结论**: 路由不是告诉 LLM 做什么，是限制 LLM 不能做什么、给多少资源。
LLM 在路由给的边界内自己选工具。最有价值的一步是 **第二轮继承上一轮的 intent**。
